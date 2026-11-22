use super::{file_operation, file_operation_mode, mcp_files::Files, mcp_io};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

const MAX_STEPS: usize = 64;
const MAX_PLAN_PATH_BYTES: usize = 4_000_000;

pub(super) fn preview_paths(
    files: Option<&Files<'_>>,
    output: &Path,
    paths: Vec<PathBuf>,
    directory: bool,
) -> Result<Value, String> {
    let files = files.ok_or("Missing batch path policy")?;
    files.check_write(output)?;
    if directory && mcp_io::metadata(output).is_ok_and(|m| !m.is_dir()) {
        return Err(format!("Output must be a directory: {}", output.display()));
    }
    for path in &paths {
        files.check_file(path)?;
    }
    Ok(json!({"outputs": paths}))
}

fn boolean(args: &Value, key: &str, fallback: bool) -> Result<bool, String> {
    args.get(key).map_or(Ok(fallback), |arg| {
        arg.as_bool()
            .ok_or_else(|| format!("{key} must be a boolean"))
    })
}

pub(super) fn execute(args: &Value, policy: Option<&Files<'_>>) -> Result<Value, String> {
    let steps = args["steps"]
        .as_array()
        .ok_or("Batch expects a steps array")?;
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(format!("Choose between 1 and {MAX_STEPS} batch steps"));
    }
    let dry_run = boolean(args, "dry_run", false)?;
    let root = policy.and_then(|files| files.root);
    let overwrite = boolean(
        args,
        "overwrite",
        policy.is_some_and(|files| files.overwrite),
    )?;
    let mut jobs = Vec::with_capacity(steps.len());
    let mut destinations = BTreeSet::new();
    let mut path_bytes = 0usize;
    for (index, step) in steps.iter().enumerate() {
        let planned = (|| {
            let operation = step["operation"].as_str().ok_or("Missing operation")?;
            if !matches!(
                operation,
                "export" | "render" | "atlas" | "batch-tags" | "palette-export"
            ) {
                return Err(format!("Unsupported batch export operation: {operation}"));
            }
            let arguments = &step["arguments"];
            if !arguments.is_object() {
                return Err("Step arguments must be an object".into());
            }
            let files = Files {
                root,
                overwrite: boolean(arguments, "overwrite", overwrite)?,
            };
            let mut preview = file_operation_mode(operation, arguments, Some(&files), true)?;
            let outputs = preview["outputs"].take();
            for output in outputs.as_array().ok_or("Could not list planned outputs")? {
                let output = output.as_str().ok_or("Invalid planned output")?;
                path_bytes = path_bytes.saturating_add(output.len());
                if path_bytes > MAX_PLAN_PATH_BYTES {
                    return Err("Batch output plan exceeds its size limit".into());
                }
                let path = normalized_path(Path::new(output))?;
                if !destinations.insert(path) {
                    return Err(format!("Duplicate output path: {output}"));
                }
            }
            Ok((operation, arguments, files, outputs))
        })();
        jobs.push(planned.map_err(|error| format!("Batch step {}: {error}", index + 1))?);
    }
    for path in &destinations {
        if path
            .ancestors()
            .skip(1)
            .any(|ancestor| destinations.contains(ancestor))
        {
            return Err(format!(
                "An output also needs to be a directory: {}",
                path.display()
            ));
        }
    }

    let mut results = Vec::with_capacity(jobs.len());
    for (index, (operation, arguments, files, outputs)) in jobs.into_iter().enumerate() {
        let mut result = json!({"operation": operation, "outputs": outputs});
        if !dry_run {
            result["result"] =
                file_operation(operation, arguments, Some(&files)).map_err(|error| {
                    format!(
                        "Batch step {} failed after {index} completed step(s): {error}",
                        index + 1
                    )
                })?;
        }
        results.push(result);
    }
    Ok(json!({"dry_run": dry_run, "steps": results}))
}

fn normalized_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    #[cfg(windows)]
    let normalized = PathBuf::from(normalized.to_string_lossy().to_lowercase());
    Ok(normalized)
}
