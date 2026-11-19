use super::{PreparedExport, mcp_io, prepare_output_under};
use serde_json::Value;
use std::{
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
        (Component::Normal(a), Component::Normal(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(not(windows))]
fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    a == b
}

pub(super) fn project_suffix<'a>(
    root: &Path,
    path: &'a Path,
) -> Result<std::path::Components<'a>, String> {
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

pub(super) struct Files<'a> {
    pub(super) root: Option<&'a Path>,
    pub(super) overwrite: bool,
}

impl Files<'_> {
    pub(super) fn resolve(root: Option<&Path>, path: PathBuf) -> PathBuf {
        let Some(root) = root else {
            return path;
        };
        #[cfg(windows)]
        if path.is_absolute()
            && let Ok(suffix) = project_suffix(root, &path)
        {
            return root.join(suffix.as_path());
        }
        root.join(path)
    }

    pub(super) fn check_read(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, false)?;
        if self.root.is_some() && !mcp_io::metadata(path).map_err(|e| e.to_string())?.is_file() {
            if mcp_io::open(path).is_ok() {
                return Ok(());
            }
            return Err(format!("Input must be a regular file: {}", path.display()));
        }
        Ok(())
    }

    pub(super) fn check_atlas_input(&self, path: &Path) -> Result<(), String> {
        self.check_path(path, false)?;
        if self.root.is_some() {
            let metadata = mcp_io::metadata(path).map_err(|e| e.to_string())?;
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
        #[cfg(not(any(windows, target_os = "linux")))]
        let _ = allow_missing;
        let Some(root) = self.root else {
            return Ok(());
        };
        let _suffix = project_suffix(root, path)?;
        #[cfg(windows)]
        if super::mcp_access::Project::active(root).is_some() {
            let mut ancestor = path;
            loop {
                match mcp_io::metadata(ancestor) {
                    Ok(_) => return Ok(()),
                    Err(e) if allow_missing && e.kind() == ErrorKind::NotFound => {
                        ancestor = ancestor.parent().ok_or_else(|| e.to_string())?;
                    }
                    Err(e) => {
                        return Err(format!(
                            "Could not access path inside the project root: {e}"
                        ));
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        if let Some(project) = super::mcp_linux::Project::active(root) {
            return project.check(_suffix.as_path(), allow_missing);
        }
        Err("Restricted MCP filesystem access was not established".into())
    }

    pub(super) fn check_file(&self, path: &Path) -> Result<(), String> {
        self.check_write(path)?;
        match mcp_io::metadata(path) {
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
        self.write_files_authorized(output, files, directory, None, encode)
    }
    pub(super) fn write_files_authorized<T>(
        &self,
        output: &Path,
        files: &[PathBuf],
        directory: bool,
        approved: Option<&std::collections::HashSet<PathBuf>>,
        encode: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        self.check_write(output)?;
        let mut names = std::collections::HashSet::new();
        for file in files {
            Files {
                root: self.root,
                overwrite: approved.map_or(self.overwrite, |set| set.contains(file)),
            }
            .check_file(file)?;
            if !names.insert(file) {
                return Err(format!("Duplicate output path: {}", file.display()));
            }
        }
        if directory
            && mcp_io::metadata(output).is_err_and(|error| error.kind() == ErrorKind::NotFound)
            && let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty())
        {
            mcp_io::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let staging_target =
            if directory && mcp_io::metadata(output).is_ok_and(|entry| entry.is_dir()) {
                output.join("files")
            } else {
                output.to_path_buf()
            };
        let prepared = prepare_output_under(staging_target, true, self.root, directory, |_| {
            Ok(Value::Null)
        })?;
        let result = encode(&prepared.temporary)?;
        // Recheck staging and destinations before publishing in case a path
        // changed while the encoder was running.
        self.check_write(&prepared.staging)?;
        prepared.check_staging()?;
        for file in files {
            Files {
                root: self.root,
                overwrite: approved.map_or(self.overwrite, |set| set.contains(file)),
            }
            .check_file(file)?;
        }
        if directory {
            self.check_write(output)?;
            mcp_io::create_dir_all(output).map_err(|e| e.to_string())?;
        }
        publish_files_authorized(&prepared, files, directory, self.overwrite, approved)?;
        Ok(result)
    }
}

pub(super) fn publish_files(
    prepared: &PreparedExport,
    files: &[PathBuf],
    directory: bool,
    replace: bool,
) -> Result<(), String> {
    publish_files_authorized(prepared, files, directory, replace, None)
}
fn publish_files_authorized(
    prepared: &PreparedExport,
    files: &[PathBuf],
    directory: bool,
    replace: bool,
    approved: Option<&std::collections::HashSet<PathBuf>>,
) -> Result<(), String> {
    let may_replace = |file: &Path| approved.map_or(replace, |set| set.contains(file));
    let staging = if directory {
        &prepared.temporary
    } else {
        &prepared.staging
    };
    let mut backups = if files.len() > 1 && files.iter().any(|file| may_replace(file)) {
        Some(prepare_output_under(
            prepared.staging.clone(),
            true,
            prepared.project_root.as_deref(),
            false,
            |_| Ok(Value::Null),
        )?)
    } else {
        None
    };
    let mut previous = vec![None; files.len()];
    if let Some(backups) = &backups {
        for (index, file) in files.iter().enumerate() {
            if !may_replace(file) {
                continue;
            }
            match mcp_io::metadata(file) {
                Ok(metadata) if metadata.is_file() => {
                    let backup = backups.staging.join(index.to_string());
                    mcp_io::hard_link(file, &backup)
                        .or_else(|_| mcp_io::copy(file, &backup))
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
        let written = if may_replace(file) {
            mcp_io::rename(&staged, file)
        } else {
            mcp_io::publish_new(&staged, file)
        };
        if let Err(e) = written {
            let mut error = format!("Could not publish {}: {e}", file.display());
            for (published, backup) in files[..index].iter().zip(&previous).rev() {
                let restored = match backup {
                    Some(backup) => mcp_io::rename(backup, published),
                    None => mcp_io::remove_file(published),
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
    use std::fs;

    #[cfg(target_os = "linux")]
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
        let canonical = root.canonicalize().unwrap();
        let prepared = super::super::mcp_access::run(Some(&canonical), None, || {
            prepare_output_under(
                parent.join("file.png"),
                true,
                Some(&canonical),
                false,
                |_| Ok(Value::Null),
            )
        })
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

    #[cfg(target_os = "linux")]
    #[test]
    fn restricted_cleanup_does_not_delete_a_replacement_directory_inside_the_project() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-staging-replacement-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let canonical = root.canonicalize().unwrap();
        let prepared = super::super::mcp_access::run(Some(&canonical), None, || {
            prepare_output_under(root.join("file.png"), true, Some(&canonical), false, |_| {
                Ok(Value::Null)
            })
        })
        .unwrap();
        let renamed = root.join("moved");
        fs::rename(&prepared.staging, &renamed).unwrap();
        fs::create_dir(&prepared.staging).unwrap();
        let marker = prepared.staging.join("other");
        fs::write(&marker, b"keep").unwrap();
        let result = prepared.write().unwrap_err();
        let contents = fs::read(marker);
        fs::remove_dir_all(root).unwrap();
        assert!(result.contains("staging directory changed"), "{result}");
        assert_eq!(contents.unwrap(), b"keep");
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn restricted_cleanup_preserves_unrelated_items_moved_into_staging() {
        let root =
            std::env::temp_dir().join(format!("pixelcraft-staging-graft-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let canonical = root.canonicalize().unwrap();
        let prepared = super::super::mcp_access::run(Some(&canonical), None, || {
            prepare_output_under(
                canonical.join("file.png"),
                true,
                Some(&canonical),
                true,
                |temporary| {
                    mcp_io::write(temporary.join("owned.png"), b"temporary output")
                        .map_err(|e| e.to_string())?;
                    mcp_io::write(temporary.join("replaced.png"), b"other temporary output")
                        .map_err(|e| e.to_string())?;
                    Ok(Value::Null)
                },
            )
        })
        .unwrap();
        let known = prepared.temporary.join("owned.png");
        let replaced = prepared.temporary.join("replaced.png");
        fs::rename(&replaced, canonical.join("moved-output.png")).unwrap();
        fs::write(&replaced, b"replacement, keep").unwrap();
        let injected = prepared.staging.join("injected.png");
        fs::write(&injected, b"untracked, keep").unwrap();
        let unrelated = canonical.join("unrelated");
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("keep"), b"keep").unwrap();
        let moved = prepared.temporary.join("moved");
        fs::rename(unrelated, &moved).unwrap();
        drop(prepared);
        let contents = fs::read(moved.join("keep"));
        let replacement_contents = fs::read(replaced);
        let injected_contents = fs::read(injected);
        let owned_gone = !known.exists();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(contents.unwrap(), b"keep");
        assert_eq!(replacement_contents.unwrap(), b"replacement, keep");
        assert_eq!(injected_contents.unwrap(), b"untracked, keep");
        assert!(owned_gone);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn restricted_publication_rejects_a_replaced_staged_file() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-staged-file-replacement-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let canonical = root.canonicalize().unwrap();
        let prepared = super::super::mcp_access::run(Some(&canonical), None, || {
            prepare_output_under(
                canonical.join("file.png"),
                true,
                Some(&canonical),
                false,
                |temporary| {
                    mcp_io::write(temporary, b"original export")
                        .map_err(|error| error.to_string())?;
                    Ok(Value::Null)
                },
            )
        })
        .unwrap();
        fs::rename(&prepared.temporary, canonical.join("moved.png")).unwrap();
        fs::write(&prepared.temporary, b"unrelated content").unwrap();
        let result = prepared.access.as_ref().unwrap().run(|| {
            mcp_io::publish_new(&prepared.temporary, &prepared.output)
                .map_err(|error| error.to_string())
        });
        let temporary = prepared.temporary.clone();
        let output = prepared.output.clone();
        drop(prepared);
        let kept = fs::read(temporary);
        let output_exists = output.exists();
        fs::remove_dir_all(root).unwrap();
        assert!(result.unwrap_err().contains("changed"));
        assert!(!output_exists);
        assert_eq!(kept.unwrap(), b"unrelated content");
    }

    #[test]
    fn failed_overwrite_restores_previously_published_files() {
        let output =
            std::env::temp_dir().join(format!("pixelcraft-rollback-test-{}", std::process::id()));
        let prepared =
            prepare_output_under(output, true, None, false, |_| Ok(Value::Null)).unwrap();
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
