use super::{PreparedExport, prepare_output_under};
use serde_json::Value;
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

pub(super) fn project_root(path: &Path) -> Result<PathBuf, String> {
    let root = path
        .canonicalize()
        .map_err(|e| format!("Invalid project root {}: {e}", path.display()))?;
    if !root.is_dir() {
        return Err(format!(
            "Project root must be a directory: {}",
            root.display()
        ));
    }
    Ok(root)
}

#[cfg(windows)]
fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    use std::path::Prefix;
    match (a, b) {
        (Component::Prefix(a), Component::Prefix(b)) => match (a.kind(), b.kind()) {
            (
                Prefix::Disk(a) | Prefix::VerbatimDisk(a),
                Prefix::Disk(b) | Prefix::VerbatimDisk(b),
            ) => a.eq_ignore_ascii_case(&b),
            (
                Prefix::UNC(server_a, share_a) | Prefix::VerbatimUNC(server_a, share_a),
                Prefix::UNC(server_b, share_b) | Prefix::VerbatimUNC(server_b, share_b),
            ) => server_a.eq_ignore_ascii_case(server_b) && share_a.eq_ignore_ascii_case(share_b),
            _ => false,
        },
        (Component::Normal(a), Component::Normal(b)) => a.eq_ignore_ascii_case(b),
        _ => a == b,
    }
}

#[cfg(not(windows))]
fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    a == b
}

fn project_suffix<'a>(root: &Path, path: &'a Path) -> Result<std::path::Components<'a>, String> {
    let outside = || format!("Path escapes the project root: {}", path.display());
    let mut suffix = path.components();
    for part in root.components() {
        if !suffix.next().is_some_and(|p| same_component(p, part)) {
            return Err(outside());
        }
    }
    let mut depth = 0;
    for part in suffix.clone() {
        match part {
            Component::Normal(name) => {
                #[cfg(windows)]
                if name.to_string_lossy().ends_with([' ', '.'])
                    || name.to_string_lossy().contains(':')
                {
                    return Err(outside());
                }
                let _ = name;
                depth += 1;
            }
            Component::ParentDir if depth > 0 => depth -= 1,
            Component::CurDir => {}
            _ => return Err(outside()),
        }
    }
    Ok(suffix)
}

#[cfg(windows)]
fn check_reparse_points(root: &Path, parts: std::path::Components<'_>) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let check = |path: &Path| match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 => {
            Err(format!(
                "Project-root access to Windows reparse points is disabled: {}",
                path.display()
            ))
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Could not inspect {}: {e}", path.display())),
    };
    let ancestors: Vec<_> = root
        .ancestors()
        .filter(|path| path.file_name().is_some())
        .collect();
    for ancestor in ancestors.into_iter().rev() {
        if !check(ancestor)? {
            return Err(format!(
                "The project root no longer exists: {}",
                root.display()
            ));
        }
    }
    let mut current = root.to_path_buf();
    for part in parts {
        match part {
            Component::Normal(name) => current.push(name),
            Component::ParentDir => {
                current.pop();
                continue;
            }
            _ => continue,
        }
        if !check(&current)? {
            break;
        }
    }
    Ok(())
}

pub(super) struct Files<'a> {
    pub(super) root: Option<&'a Path>,
    pub(super) overwrite: bool,
}

impl Files<'_> {
    pub(super) fn resolve(root: Option<&Path>, path: PathBuf) -> PathBuf {
        match root {
            Some(root) => root.join(path),
            None => path,
        }
    }

    pub(super) fn check_read(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, false)?;
        if self.root.is_some() && !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
            return Err(format!("Input must be a regular file: {}", path.display()));
        }
        Ok(())
    }

    pub(super) fn check_atlas_input(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, false)?;
        if self.root.is_some() {
            let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(format!(
                    "Atlas input must be a file or directory: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check_write(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, true)
    }

    fn check_path(&self, path: &Path, allow_missing: bool) -> Result<(), String> {
        let Some(root) = self.root else {
            return Ok(());
        };
        let _suffix = project_suffix(root, path)?;
        #[cfg(windows)]
        check_reparse_points(root, _suffix)?;
        // Check the nearest existing ancestor for new paths. Use symlink metadata
        // so a dangling link is treated as a link rather than as a missing name.
        let mut ancestor = path;
        while allow_missing
            && ancestor.file_name().is_some()
            && fs::symlink_metadata(ancestor).is_err_and(|e| e.kind() == ErrorKind::NotFound)
        {
            ancestor = ancestor.parent().unwrap();
        }
        let canonical = ancestor.canonicalize().map_err(|e| {
            format!(
                "Could not resolve {} inside the project root: {e}",
                path.display()
            )
        })?;
        if !canonical.starts_with(root) {
            return Err(format!("Path escapes the project root: {}", path.display()));
        }
        Ok(())
    }

    pub(super) fn check_file(&self, path: &Path) -> Result<(), String> {
        self.check_write(path)?;
        match fs::symlink_metadata(path) {
            Ok(_) if !self.overwrite => Err(format!(
                "Output already exists: {}. Set overwrite to true to replace it.",
                path.display()
            )),
            Ok(metadata) if !metadata.is_file() => {
                Err(format!("Output must be a regular file: {}", path.display()))
            }
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Could not inspect {}: {e}", path.display())),
        }
    }

    pub(super) fn write_file<T>(
        &self,
        output: &Path,
        encode: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        self.write_files(output, &[output.to_path_buf()], false, encode)
    }

    // The encoder writes into a private staging directory. Validate the whole
    // output set up front and publish only after all the files have encoded.
    pub(super) fn write_files<T>(
        &self,
        output: &Path,
        files: &[PathBuf],
        directory: bool,
        encode: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        self.check_write(output)?;
        let mut names = std::collections::HashSet::new();
        for file in files {
            self.check_file(file)?;
            if !names.insert(file) {
                return Err(format!("Duplicate output path: {}", file.display()));
            }
        }
        if directory && let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let staging_target = if directory && output.is_dir() {
            output.join("files")
        } else {
            output.to_path_buf()
        };
        let prepared = prepare_output_under(staging_target, true, self.root, |_| Ok(Value::Null))?;
        let result = encode(&prepared.temporary)?;
        // Recheck staging and destinations before publishing in case a path
        // changed while the encoder was running.
        self.check_write(&prepared.staging)?;
        for file in files {
            self.check_file(file)?;
        }
        if directory {
            self.check_write(output)?;
            fs::create_dir_all(output).map_err(|e| e.to_string())?;
        }
        publish_files(&prepared, files, directory, self.overwrite)?;
        Ok(result)
    }
}

pub(super) fn publish_files(
    prepared: &PreparedExport,
    files: &[PathBuf],
    directory: bool,
    replace: bool,
) -> Result<(), String> {
    let staging = if directory {
        &prepared.temporary
    } else {
        &prepared.staging
    };
    let mut backups = if replace && files.len() > 1 {
        Some(prepare_output_under(
            prepared.staging.clone(),
            true,
            prepared.project_root.as_deref(),
            |_| Ok(Value::Null),
        )?)
    } else {
        None
    };
    let mut previous = vec![None; files.len()];
    if let Some(backups) = &backups {
        for (index, file) in files.iter().enumerate() {
            match fs::symlink_metadata(file) {
                Ok(metadata) if metadata.is_file() => {
                    let backup = backups.staging.join(index.to_string());
                    fs::hard_link(file, &backup)
                        .or_else(|_| fs::copy(file, &backup).map(|_| ()))
                        .map_err(|e| format!("Could not back up {}: {e}", file.display()))?;
                    previous[index] = Some(backup);
                }
                Ok(_) => return Err(format!("Output must be a regular file: {}", file.display())),
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Could not inspect {}: {e}", file.display())),
            }
        }
    }
    for (index, file) in files.iter().enumerate() {
        let staged = staging.join(file.file_name().ok_or("Invalid output filename")?);
        let written = if replace {
            fs::rename(staged, file)
        } else {
            fs::hard_link(staged, file)
        };
        if let Err(e) = written {
            let mut error = format!("Could not publish {}: {e}", file.display());
            for (published, backup) in files[..index].iter().zip(&previous).rev() {
                let restored = match backup {
                    Some(backup) => fs::rename(backup, published),
                    None => fs::remove_file(published),
                };
                if let Err(e) = restored {
                    error.push_str(&format!(
                        "
Could not restore {}: {e}",
                        published.display()
                    ));
                    if let (Some(backups), Some(backup)) = (&mut backups, backup) {
                        backups.preserve_staging = true;
                        error.push_str(&format!(
                            "
Backup retained at {}",
                            backup.display()
                        ));
                    }
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn restricted_cleanup_leaves_redirected_external_staging_untouched() {
        use std::os::unix::fs::symlink;

        let workspace =
            std::env::temp_dir().join(format!("pixelcraft-cleanup-test-{}", std::process::id()));
        let root = workspace.join("project");
        let parent = root.join("export");
        let outside = workspace.join("outside");
        fs::create_dir_all(&parent).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let prepared = prepare_output_under(
            parent.join("file.png"),
            true,
            Some(&root.canonicalize().unwrap()),
            |_| Ok(Value::Null),
        )
        .unwrap();
        let unrelated = outside
            .join(prepared.staging.file_name().unwrap())
            .join("keep");
        fs::create_dir(unrelated.parent().unwrap()).unwrap();
        fs::write(&unrelated, b"unrelated content").unwrap();
        fs::rename(&parent, root.join("moved")).unwrap();
        symlink(&outside, &parent).unwrap();
        drop(prepared);
        let contents = fs::read(&unrelated);
        fs::remove_dir_all(workspace).unwrap();
        assert_eq!(contents.unwrap(), b"unrelated content");
    }

    #[test]
    fn failed_overwrite_restores_previously_published_files() {
        let output =
            std::env::temp_dir().join(format!("pixelcraft-rollback-test-{}", std::process::id()));
        let prepared = prepare_output_under(output, true, None, |_| Ok(Value::Null)).unwrap();
        let published = prepared.staging.join("published");
        let staged = &prepared.temporary;
        fs::create_dir(&published).unwrap();
        fs::create_dir(staged).unwrap();
        let existing = published.join("existing.png");
        let new = published.join("new.png");
        let missing = published.join("missing.png");
        fs::write(&existing, b"original").unwrap();
        fs::write(staged.join("existing.png"), b"replacement").unwrap();
        fs::write(staged.join("new.png"), b"new").unwrap();
        assert!(
            publish_files(
                &prepared,
                &[existing.clone(), new.clone(), missing],
                true,
                true
            )
            .is_err()
        );
        assert_eq!(fs::read(existing).unwrap(), b"original");
        assert!(!new.exists());
    }
}
