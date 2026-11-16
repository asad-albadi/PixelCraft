use super::{PreparedExport, prepare_output};
use serde_json::Value;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
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
        self.check_path(path, false)
    }

    pub(super) fn check_write(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, true)
    }

    fn check_path(&self, path: &Path, allow_missing: bool) -> Result<(), String> {
        let Some(root) = self.root else {
            return Ok(());
        };
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
        let prepared = prepare_output(staging_target, true, |_| Ok(Value::Null))?;
        let result = encode(&prepared.temporary)?;
        // Recheck all destinations before publishing in case a file appeared
        // while the encoder was running.
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
        Some(prepare_output(prepared.staging.clone(), true, |_| {
            Ok(Value::Null)
        })?)
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

    #[test]
    fn failed_overwrite_restores_previously_published_files() {
        let output =
            std::env::temp_dir().join(format!("pixelcraft-rollback-test-{}", std::process::id()));
        let prepared = prepare_output(output, true, |_| Ok(Value::Null)).unwrap();
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
