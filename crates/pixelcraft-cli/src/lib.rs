//! CLI parsing over shared editor operations.
use pixelcraft_automation::{CLI_HELP, operation};
use serde_json::{Value, json};
pub fn run_cli(args: &[String]) -> Result<Value, String> {
    let op = args.first().ok_or(CLI_HELP)?;
    let v = cli_arguments(&args[1..])?;
    operation(op, &v)
}

fn cli_arguments(args: &[String]) -> Result<Value, String> {
    let mut v = json!({});
    if args.first().is_some_and(|s| s.starts_with('{')) {
        if args.len() != 1 {
            return Err("Unexpected extra arguments".into());
        }
        v = serde_json::from_str(&args[0]).map_err(|e| e.to_string())?;
    } else {
        let mut i = 0;
        while i < args.len() {
            let key = args[i]
                .strip_prefix("--")
                .ok_or("Expected --argument value")?;
            let key = if key == "dry-run" { "dry_run" } else { key };
            let value = args.get(i + 1).ok_or("Missing argument value")?;
            v[key] = if [
                "inputs",
                "options",
                "edits",
                "commands",
                "frame",
                "width",
                "height",
                "scale",
                "layers",
                "columns",
                "steps",
                "dry_run",
                "overwrite",
            ]
            .contains(&key)
            {
                serde_json::from_str(value).map_err(|e| e.to_string())?
            } else {
                json!(value)
            };
            i += 2;
        }
    }
    Ok(v)
}
