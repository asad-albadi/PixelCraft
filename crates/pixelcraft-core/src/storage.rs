//! Shared guarded I/O and anchored publication; no MCP transport.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
mod editor_output;
pub mod mcp_access;
pub mod mcp_files;
pub mod mcp_io;
#[cfg(target_os = "linux")]
pub(crate) mod mcp_linux;
#[cfg(windows)]
mod mcp_windows;
pub use editor_output::{EditorOutputPlan, write_editor_file, write_editor_file_authorized};
pub struct PreparedExport {
    pub output: PathBuf,
    staging: PathBuf,
    staging_identity: Option<(u64, u64)>,
    temporary_directory_identity: Option<(u64, u64)>,
    staged_files: Option<std::sync::Arc<mcp_io::StagedFiles>>,
    preserve_staging: bool,
    temporary: PathBuf,
    metadata: Value,
    replace: bool,
    sequence: Vec<String>,
    project_root: Option<PathBuf>,
    access: Option<std::sync::Arc<mcp_access::Project>>,
}

impl Drop for PreparedExport {
    fn drop(&mut self) {
        if self.preserve_staging {
            return;
        }
        let root = self.project_root.as_deref();
        let staging = &self.staging;
        let staging_identity = self.staging_identity;
        let temporary_directory = self
            .temporary
            .file_name()
            .zip(self.temporary_directory_identity);
        let cleanup = || {
            mcp_files::Files {
                root,
                overwrite: true,
            }
            .check_write(staging)?;
            mcp_io::remove_staging(
                staging,
                staging_identity,
                temporary_directory,
                self.staged_files.as_deref(),
            )
            .map_err(|error| error.to_string())
        };
        if let Some(access) = &self.access {
            let _ = access.run(cleanup);
        } else if root.is_none() {
            let _ = cleanup();
        }
    }
}

pub fn prepare_output(
    output: PathBuf,
    replace: bool,
    encode: impl FnOnce(&Path) -> Result<Value, String>,
) -> Result<PreparedExport, String> {
    let root = mcp_access::active_root();
    prepare_output_under(output, replace, root.as_deref(), false, encode)
}

pub fn prepare_output_under(
    output: PathBuf,
    replace: bool,
    root: Option<&Path>,
    directory: bool,
    encode: impl FnOnce(&Path) -> Result<Value, String>,
) -> Result<PreparedExport, String> {
    let files = mcp_files::Files {
        root,
        overwrite: replace,
    };
    let access = root
        .map(|root| {
            mcp_access::Project::active(root)
                .ok_or("Restricted MCP filesystem access was not established")
        })
        .transpose()?;
    files.check_write(&output)?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if !replace && mcp_io::exists(&output).map_err(|e| e.to_string())? {
        return Err("Output already exists. Choose a new path.".into());
    }
    let parent = output.parent().ok_or("Invalid output path")?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let resolved_parent;
    let parent = if root.is_some() {
        resolved_parent = mcp_io::directory_path(parent).map_err(|e| e.to_string())?;
        files.check_write(&resolved_parent)?;
        &resolved_parent
    } else {
        parent
    };
    let filename = output.file_name().ok_or("Invalid output filename")?;
    let (staging, staging_identity) = loop {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = parent.join(format!(".pixelcraft-output-{}-{n}", std::process::id()));
        match mcp_io::create_staging_dir(&dir) {
            Ok(identity) => break (dir, identity),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let staged_files = staging_identity.map(|_| mcp_io::track_staging(&staging));
    let temporary = staging.join(filename);
    let mut prepared = PreparedExport {
        output,
        staging,
        staging_identity,
        temporary_directory_identity: None,
        staged_files,
        preserve_staging: false,
        temporary,
        metadata: Value::Null,
        replace,
        sequence: vec![],
        project_root: root.map(Path::to_path_buf),
        access,
    };
    if directory && prepared.access.is_some() {
        prepared.temporary_directory_identity =
            mcp_io::create_staging_dir(&prepared.temporary).map_err(|error| error.to_string())?;
    }
    prepared.metadata = encode(&prepared.temporary)?;
    prepared.check_staging()?;
    Ok(prepared)
}

pub fn check_sequence_outputs(
    output: &Path,
    names: &[String],
    files: &mcp_files::Files<'_>,
) -> Result<Vec<PathBuf>, String> {
    files.check_write(output)?;
    match mcp_io::metadata(output) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(format!(
                "Sequence output must be a directory: {}",
                output.display()
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    names
        .iter()
        .map(|name| {
            let frame = output.join(name);
            files.check_file(&frame)?;
            Ok(frame)
        })
        .collect()
}

impl PreparedExport {
    pub fn with_sequence(mut self, output: PathBuf, names: Vec<String>) -> Self {
        self.output = output;
        self.sequence = names;
        self
    }

    fn check_staging(&self) -> Result<(), String> {
        if let Some(identity) = self.staging_identity
            && mcp_io::directory_identity(&self.staging).map_err(|error| error.to_string())?
                != identity
        {
            return Err("The restricted export's staging directory changed".into());
        }
        if let Some(identity) = self.temporary_directory_identity
            && mcp_io::directory_identity(&self.temporary).map_err(|error| error.to_string())?
                != identity
        {
            return Err("The restricted export's temporary directory changed".into());
        }
        Ok(())
    }

    pub fn write(self) -> Result<Value, String> {
        if let Some(access) = self.access.clone() {
            access.run(|| self.write_inner())
        } else if self.project_root.is_some() {
            Err("Restricted MCP filesystem access was not established".into())
        } else {
            self.write_inner()
        }
    }

    fn write_inner(mut self) -> Result<Value, String> {
        let guard = mcp_files::Files {
            root: self.project_root.as_deref(),
            overwrite: self.replace,
        };
        guard.check_write(&self.output)?;
        guard.check_write(&self.staging)?;
        self.check_staging()?;
        if !self.sequence.is_empty() && self.replace {
            let files = check_sequence_outputs(&self.output, &self.sequence, &guard)?;
            mcp_io::create_dir_all(&self.output).map_err(|e| e.to_string())?;
            mcp_files::publish_files(&self, &files, true, true)?;
        } else if !self.sequence.is_empty() {
            mcp_io::create_dir(&self.output).map_err(|e| e.to_string())?;
            for (index, name) in self.sequence.iter().enumerate() {
                if let Err(error) =
                    mcp_io::publish_new(&self.temporary.join(name), &self.output.join(name))
                {
                    for written in &self.sequence[..index] {
                        let _ = mcp_io::remove_file(&self.output.join(written));
                    }
                    let _ = mcp_io::remove_dir(&self.output);
                    return Err(error.to_string());
                }
            }
        } else {
            if self.project_root.is_some() {
                guard.check_file(&self.output)?;
            }
            if self.replace {
                mcp_io::rename(&self.temporary, &self.output)
            } else {
                mcp_io::publish_new(&self.temporary, &self.output)
            }
            .map_err(|e| e.to_string())?;
        }
        self.metadata["output"] = json!(self.output);
        Ok(std::mem::take(&mut self.metadata))
    }
}
