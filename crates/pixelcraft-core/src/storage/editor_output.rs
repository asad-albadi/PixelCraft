//! Explicit GUI output authorization over anchored native publication.
use super::{mcp_access, mcp_files::Files, mcp_io};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
pub struct EditorOutputPlan {
    output: PathBuf,
    files: Arc<Vec<PathBuf>>,
    directory: bool,
    existing: Vec<PathBuf>,
    root: Option<PathBuf>,
    access: Option<Arc<mcp_access::Project>>,
}
type OutputAccess = (PathBuf, Option<PathBuf>, Option<Arc<mcp_access::Project>>);
fn output_access(path: &Path, directory: bool) -> Result<OutputAccess, String> {
    let (parent, output) = if directory && path.file_name().is_none() {
        let output = path.canonicalize().map_err(|e| e.to_string())?;
        (output.clone(), output)
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let output = parent.join(path.file_name().ok_or("Invalid output filename")?);
        (parent, output)
    };
    #[cfg(any(windows, target_os = "linux"))]
    {
        let root = if let Some(root) = mcp_access::active_root() {
            root
        } else if directory {
            match std::fs::symlink_metadata(&output) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => output.clone(),
                Ok(_) => {
                    return Err("Choose a regular output folder, not a linked directory".into());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => parent,
                Err(e) => return Err(e.to_string()),
            }
        } else {
            parent
        };
        let access = mcp_access::Project::active(&root)
            .map_or_else(|| mcp_access::editor_project(&root), Ok)?;
        Ok((output, Some(root), Some(access)))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        // Platforms without the native anchored writer accept only private trees.
        verify_private_tree(&parent)?;
        if directory {
            match std::fs::symlink_metadata(&output) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
                    verify_private_tree(&output)?
                }
                Ok(_) => return Err("Choose a regular output folder".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok((output, None, None))
    }
}
#[cfg(not(any(windows, target_os = "linux")))]
fn verify_private_tree(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for ancestor in path.ancestors() {
            let meta = std::fs::metadata(ancestor).map_err(|e| e.to_string())?;
            if (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
                || (meta.mode() & 0o022 != 0 && meta.mode() & 0o1000 == 0)
            {
                return Err("Choose a private output folder for this export".into());
            }
            #[cfg(target_os = "macos")]
            if mac_acl_has_writer(ancestor)? {
                return Err("Choose a private output folder for this export".into());
            }
        }
    }
    let _ = path;
    Ok(())
}
impl EditorOutputPlan {
    pub fn new(output: PathBuf, files: Vec<PathBuf>, directory: bool) -> Result<Self, String> {
        Self::with_cancel(output, files, directory, || false)
    }
    pub fn with_cancel(
        output: PathBuf,
        files: Vec<PathBuf>,
        directory: bool,
        cancelled: impl Fn() -> bool + Send + Sync,
    ) -> Result<Self, String> {
        if files.is_empty() {
            return Err("Export has no output files".into());
        }
        let original_parent = if directory {
            output.as_path()
        } else {
            output.parent().ok_or("Invalid output path")?
        };
        let (output, root, access) = output_access(&output, directory)?;
        let target_parent = if directory {
            output.as_path()
        } else {
            output.parent().unwrap()
        };
        let mut unique = HashSet::new();
        let mut rebased = Vec::new();
        for file in files {
            if file.parent() != Some(original_parent) || !unique.insert(file.clone()) {
                return Err("Invalid or duplicate export output".into());
            }
            rebased.push(target_parent.join(file.file_name().ok_or("Invalid output filename")?));
        }
        let inspect = || {
            let mut existing = Vec::new();
            if directory {
                match mcp_io::metadata(&output) {
                    Ok(meta) if !meta.is_dir() => {
                        return Err("Choose a regular output folder".into());
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            for file in &rebased {
                if cancelled() {
                    return Err("Export cancelled".into());
                }
                match mcp_io::metadata(file) {
                    Ok(meta) if meta.is_file() => existing.push(file.clone()),
                    Ok(_) => {
                        return Err(format!("Output must be a regular file: {}", file.display()));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(existing)
        };
        let existing = if let Some(access) = &access {
            access.run(inspect)?
        } else {
            inspect()?
        };
        Ok(Self {
            output,
            files: Arc::new(rebased),
            directory,
            existing,
            root,
            access,
        })
    }
    pub fn output(&self) -> &Path {
        &self.output
    }
    pub fn existing(&self) -> &[PathBuf] {
        &self.existing
    }
    pub fn paths(&self) -> &[PathBuf] {
        self.files.as_slice()
    }
    pub fn shared_paths(&self) -> Arc<Vec<PathBuf>> {
        self.files.clone()
    }
    pub fn write<T: Send>(
        self,
        confirmed: bool,
        encode: impl FnOnce(&Path) -> Result<T, String> + Send,
    ) -> Result<T, String> {
        if !confirmed && !self.existing.is_empty() {
            return Err("Existing export outputs require confirmation".into());
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        if self.directory {
            match mcp_io::metadata(&self.output) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    mcp_io::create_staging_dir(&self.output).map_err(|e| e.to_string())?;
                }
                Ok(meta) if !meta.is_dir() => return Err("Choose a regular output folder".into()),
                Err(error) => return Err(error.to_string()),
                _ => {}
            }
            verify_private_tree(&self.output)?;
        }
        let approved = if confirmed {
            self.existing.into_iter().collect()
        } else {
            HashSet::new()
        };
        let operation = || {
            Files {
                root: self.root.as_deref(),
                overwrite: false,
            }
            .write_files_authorized(
                &self.output,
                &self.files,
                self.directory,
                Some(&approved),
                encode,
            )
        };
        if let Some(access) = &self.access {
            access.run(operation)
        } else {
            operation()
        }
    }
}
/// Encoders write under an anchored parent with exclusive, identity-checked staging.
pub fn write_editor_file<T: Send>(
    path: &Path,
    overwrite: bool,
    encode: impl FnOnce(&Path) -> Result<T, String> + Send,
) -> Result<T, String> {
    write_editor_file_authorized(path, overwrite, None, encode)
}
/// Confirmed Save As paths must retain the physical destination shown to the user.
pub fn write_editor_file_authorized<T: Send>(
    path: &Path,
    overwrite: bool,
    approved: Option<&Path>,
    encode: impl FnOnce(&Path) -> Result<T, String> + Send,
) -> Result<T, String> {
    let (output, root, access) = output_access(path, false)?;
    if approved.is_some_and(|approved| approved != output) {
        return Err("Save destination changed; choose the destination again".into());
    }
    let operation = || {
        Files {
            root: root.as_deref(),
            overwrite,
        }
        .write_file(&output, encode)
    };
    if let Some(access) = access {
        access.run(operation)
    } else {
        operation()
    }
}

// Darwin ACL constants/signatures: apple-oss-distributions/Libc/include/sys/acl.h.
#[cfg(target_os = "macos")]
fn mac_acl_has_writer(path: &Path) -> Result<bool, String> {
    use std::{
        ffi::{CString, c_void},
        os::unix::ffi::OsStrExt,
    };
    unsafe extern "C" {
        fn acl_get_link_np(path: *const libc::c_char, kind: libc::c_int) -> *mut c_void;
        fn acl_get_entry(
            acl: *mut c_void,
            kind: libc::c_int,
            entry: *mut *mut c_void,
        ) -> libc::c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut libc::c_int) -> libc::c_int;
        fn acl_get_permset(entry: *mut c_void, perms: *mut *mut c_void) -> libc::c_int;
        fn acl_get_perm_np(perms: *mut c_void, permission: libc::c_uint) -> libc::c_int;
        fn acl_free(acl: *mut c_void) -> libc::c_int;
    }
    struct OwnedAcl(*mut c_void);
    impl Drop for OwnedAcl {
        fn drop(&mut self) {
            unsafe {
                acl_free(self.0);
            }
        }
    }
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| "Invalid output path")?;
    let raw = unsafe { acl_get_link_np(path.as_ptr(), 0x100) };
    if raw.is_null() {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ENOENT) {
            Ok(false)
        } else {
            Err(error.to_string())
        };
    }
    let acl = OwnedAcl(raw);
    let mut entry = std::ptr::null_mut();
    for index in 0..1024 {
        if unsafe { acl_get_entry(acl.0, if index == 0 { 0 } else { -1 }, &mut entry) } != 0 {
            return Ok(false);
        }
        let mut tag = 0;
        if unsafe { acl_get_tag_type(entry, &mut tag) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if tag != 1 {
            continue;
        }
        let mut permissions = std::ptr::null_mut();
        if unsafe { acl_get_permset(entry, &mut permissions) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        for flag in [
            1 << 2,
            1 << 4,
            1 << 5,
            1 << 6,
            1 << 8,
            1 << 10,
            1 << 12,
            1 << 13,
        ] {
            match unsafe { acl_get_perm_np(permissions, flag) } {
                1 => return Ok(true),
                0 => {}
                _ => return Err(std::io::Error::last_os_error().to_string()),
            }
        }
    }
    Err("Output ACL exceeds its entry limit".into())
}
#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    use super::*;
    #[test]
    fn shared_write_acl_is_not_mistaken_for_a_private_parent() {
        let root = std::env::temp_dir().join(format!("pixelcraft-acl-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(!mac_acl_has_writer(&root).unwrap());
        let status = std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone allow add_file,delete_child"])
            .arg(&root)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(mac_acl_has_writer(&root).unwrap());
        let output = root.join("image.png");
        assert!(EditorOutputPlan::new(output.clone(), vec![output], false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod sequence_validation_tests {
    #[test]
    fn a_sequence_validates_its_document_once_not_once_per_frame() {
        let mut doc = crate::model::Document::new(1, 1);
        for _ in 0..63 {
            doc.add_frame(0, false);
        }
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-sequence-validation-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        crate::model::DOCUMENT_VALIDATIONS.set(0);
        crate::formats::encode_editor_sequence(&root, &doc, 1).unwrap();
        let validations = crate::model::DOCUMENT_VALIDATIONS.get();
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(
            validations, 1,
            "sequence validation must scale linearly with the document"
        );
    }
}

#[cfg(all(test, windows))]
mod root_output_tests {
    use super::*;
    #[test]
    fn an_existing_drive_root_is_a_valid_sequence_destination() {
        let current = std::env::current_dir().unwrap();
        let root = current.ancestors().last().unwrap().to_path_buf();
        let output = root.join(format!("pixelcraft-plan-only-{}.png", std::process::id()));
        assert!(!output.exists());
        let plan = EditorOutputPlan::new(root.clone(), vec![output.clone()], true).unwrap();
        assert!(plan.existing().is_empty());
        assert!(
            !output.exists(),
            "planning must not write at the drive root"
        );
    }
}
