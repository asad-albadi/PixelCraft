use std::path::Path;
#[cfg(not(target_os = "linux"))]
use std::sync::Arc;

#[cfg(target_os = "linux")]
pub use super::mcp_linux::{Project, is_restricted};

#[cfg(windows)]
mod windows_access {
    use super::*;
    use crate::storage::mcp_windows::Guard;
    use std::{cell::RefCell, path::PathBuf};

    thread_local! {
        static ACTIVE: RefCell<Option<Arc<Project>>> = const { RefCell::new(None) };
    }

    pub struct Project {
        path: PathBuf,
        guard: Guard,
        identity: (u64, u64),
    }

    impl Project {
        pub fn new(path: &Path) -> Result<Arc<Self>, String> {
            let guard =
                Guard::new(path).map_err(|e| format!("Could not open the project root: {e}"))?;
            let identity = guard
                .identity()
                .map_err(|e| format!("Could not identify the project root: {e}"))?;
            Ok(Arc::new(Self {
                path: path.to_path_buf(),
                guard,
                identity,
            }))
        }

        pub fn active(path: &Path) -> Option<Arc<Self>> {
            ACTIVE.with_borrow(|active| active.as_ref().filter(|p| p.path == path).cloned())
        }

        pub fn identity(&self) -> Option<(u64, u64)> {
            Some(self.identity)
        }

        pub fn run<T: Send>(
            self: &Arc<Self>,
            operation: impl FnOnce() -> Result<T, String> + Send,
        ) -> Result<T, String> {
            if let Some(current) = ACTIVE.with_borrow(Clone::clone) {
                if current.identity != self.identity {
                    return Err(
                        "Cannot change the root of an active restricted MCP operation".into(),
                    );
                }
                return operation();
            }
            struct Reset;
            impl Drop for Reset {
                fn drop(&mut self) {
                    ACTIVE.set(None);
                }
            }
            let _reset = Reset;
            ACTIVE.set(Some(self.clone()));
            operation()
        }
    }

    pub fn is_restricted() -> bool {
        ACTIVE.with_borrow(Option::is_some)
    }

    pub fn active_root() -> Option<PathBuf> {
        ACTIVE.with_borrow(|active| active.as_ref().map(|project| project.path.clone()))
    }

    pub fn windows<T>(operation: impl FnOnce(&Guard) -> T) -> Option<T> {
        ACTIVE.with_borrow(|active| active.as_ref().map(|project| operation(&project.guard)))
    }
}

#[cfg(windows)]
pub(super) use windows_access::windows;
#[cfg(windows)]
pub use windows_access::{Project, is_restricted};

#[cfg(not(any(target_os = "linux", windows)))]
pub struct Project;

#[cfg(not(any(target_os = "linux", windows)))]
pub fn is_restricted() -> bool {
    false
}

#[cfg(not(any(target_os = "linux", windows)))]
impl Project {
    pub fn new(_: &Path) -> Result<Arc<Self>, String> {
        Err("Restricted MCP filesystem operations are unavailable on this platform".into())
    }

    pub fn run<T: Send>(
        self: &Arc<Self>,
        _operation: impl FnOnce() -> Result<T, String> + Send,
    ) -> Result<T, String> {
        Err("Restricted MCP filesystem operations are unavailable on this platform".into())
    }

    pub fn active(_: &Path) -> Option<Arc<Self>> {
        None
    }

    pub fn identity(&self) -> Option<(u64, u64)> {
        None
    }
}

pub fn restricted() -> bool {
    is_restricted()
}

pub fn active_root() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    return windows_access::active_root();
    #[cfg(target_os = "linux")]
    return super::mcp_linux::active_root();
    #[cfg(not(any(target_os = "linux", windows)))]
    None
}

pub fn find_ffmpeg(root: &Path) -> std::io::Result<std::path::PathBuf> {
    let command = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let paths = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&paths)
        .filter(|path| path.is_absolute())
        .filter_map(|path| path.join(command).canonicalize().ok())
        .find(|path| !path.starts_with(root) && path.is_file())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Restricted MCP requires FFmpeg on an absolute PATH entry outside the project",
            )
        })
}

pub fn run<T: Send>(
    root: Option<&Path>,
    identity: Option<(u64, u64)>,
    operation: impl FnOnce() -> Result<T, String> + Send,
) -> Result<T, String> {
    match root {
        Some(root) => {
            let project = Project::active(root).map_or_else(|| Project::new(root), Ok)?;
            if identity.is_some() && project.identity() != identity {
                return Err("The project root changed since the MCP session opened it".into());
            }
            project.run(operation)
        }
        None => operation(),
    }
}

#[cfg(any(windows, target_os = "linux"))]
pub fn editor_project(path: &Path) -> Result<std::sync::Arc<Project>, String> {
    #[cfg(windows)]
    {
        Project::new(path)
    }
    #[cfg(target_os = "linux")]
    {
        Project::new_editor(path)
    }
}
