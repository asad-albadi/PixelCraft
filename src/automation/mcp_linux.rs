//! Linux restricted MCP workers. Landlock guards each filesystem syscall;
//! openat2 bounds preflight lookups to the selected directory descriptor.
use std::{
    cell::RefCell,
    ffi::CString,
    fs::File,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

const PROJECT_RIGHTS: u64 = (1 << 1)
    | (1 << 2)
    | (1 << 3)
    | (1 << 4)
    | (1 << 5)
    | (1 << 7)
    | (1 << 8)
    | (1 << 13)
    | (1 << 14);
const HANDLED_RIGHTS: u64 = (1 << 15) - 1;
const LANDLOCK_CREATE_RULESET_VERSION: libc::c_uint = 1;
const LANDLOCK_RULE_PATH_BENEATH: libc::c_int = 1;
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: RawFd,
}

type RuntimeJob = Box<dyn FnOnce() + Send>;
thread_local! {
    static ACTIVE: RefCell<Option<(Arc<Project>, mpsc::Sender<RuntimeJob>)>> = const { RefCell::new(None) };
}

pub(super) struct Project {
    path: PathBuf,
    dir: File,
    rules: Option<File>,
    identity: (u64, u64),
}

fn os_result(result: libc::c_long) -> io::Result<libc::c_long> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

fn open(dir: RawFd, path: &Path, flags: libc::c_int, resolve: u64) -> io::Result<File> {
    let name = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let how = OpenHow {
        flags: (flags | libc::O_CLOEXEC) as u64,
        mode: if flags & libc::O_CREAT == 0 { 0 } else { 0o600 },
        resolve,
    };
    // SAFETY: name and how are valid for the duration of the call, and success
    // transfers ownership of a new descriptor to us.
    let fd = unsafe {
        os_result(libc::syscall(
            libc::SYS_openat2,
            dir,
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        ))?
    };
    Ok(unsafe { File::from_raw_fd(fd as RawFd) })
}

impl Project {
    pub(super) fn new(path: &Path) -> Result<Arc<Self>, String> {
        let unavailable =
            |e| format!("Restricted MCP requires Linux openat2 and Landlock ABI 3 or newer: {e}");
        let dir = open(
            libc::AT_FDCWD,
            path,
            libc::O_PATH | libc::O_DIRECTORY,
            RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
        )
        .map_err(unavailable)?;
        let metadata = dir.metadata().map_err(|e| e.to_string())?;
        // SAFETY: the version query accepts a null rule pointer; the subsequent
        // rule and path payloads are kernel ABI structs valid through each call.
        let rules = unsafe {
            let version = os_result(libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u64>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            ))
            .map_err(unavailable)?;
            if version < 3 {
                return Err(format!(
                    "Restricted MCP requires Linux Landlock ABI 3 or newer (found ABI {version})"
                ));
            }
            let fd = os_result(libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &HANDLED_RIGHTS,
                std::mem::size_of::<u64>(),
                0u32,
            ))
            .map_err(unavailable)?;
            let rules = File::from_raw_fd(fd as RawFd);
            let rule = PathBeneath {
                allowed_access: PROJECT_RIGHTS,
                parent_fd: dir.as_raw_fd(),
            };
            os_result(libc::syscall(
                libc::SYS_landlock_add_rule,
                rules.as_raw_fd(),
                LANDLOCK_RULE_PATH_BENEATH,
                &rule,
                0u32,
            ))
            .map_err(unavailable)?;
            rules
        };
        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            dir,
            rules: Some(rules),
            identity: (metadata.dev(), metadata.ino()),
        }))
    }

    /// Trusted editor encoders need anchored file I/O, not a new kernel sandbox requirement.
    pub(super) fn new_editor(path: &Path) -> Result<Arc<Self>, String> {
        let dir = open(
            libc::AT_FDCWD,
            path,
            libc::O_PATH | libc::O_DIRECTORY,
            RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
        )
        .map_err(|e| e.to_string())?;
        let metadata = dir.metadata().map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            dir,
            rules: None,
            identity: (metadata.dev(), metadata.ino()),
        }))
    }

    pub(super) fn active(path: &Path) -> Option<Arc<Self>> {
        ACTIVE.with_borrow(|active| {
            active
                .as_ref()
                .filter(|(project, _)| project.path == path)
                .map(|(project, _)| project.clone())
        })
    }

    pub(super) fn identity(&self) -> Option<(u64, u64)> {
        Some(self.identity)
    }

    pub(super) fn check(&self, relative: &Path, allow_missing: bool) -> Result<(), String> {
        let mut candidate = relative;
        loop {
            let path = if candidate.as_os_str().is_empty() {
                Path::new(".")
            } else {
                candidate
            };
            match open(
                self.dir.as_raw_fd(),
                path,
                libc::O_PATH,
                RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
            ) {
                Ok(_) => return Ok(()),
                Err(e) if allow_missing && e.kind() == io::ErrorKind::NotFound => {
                    candidate = candidate.parent().ok_or_else(|| e.to_string())?;
                }
                Err(e) => {
                    return Err(format!(
                        "Could not access path inside the project root: {e}"
                    ));
                }
            }
        }
    }

    pub(super) fn run<T: Send>(
        self: &Arc<Self>,
        operation: impl FnOnce() -> Result<T, String> + Send,
    ) -> Result<T, String> {
        if let Some(current) = ACTIVE.with_borrow(|active| active.as_ref().map(|(p, _)| p.clone()))
        {
            if current.identity != self.identity {
                return Err("Cannot change the root of an active restricted MCP worker".into());
            }
            return operation();
        }
        let project = self.clone();
        std::thread::scope(|scope| {
            let (sender, receiver) = mpsc::channel::<RuntimeJob>();
            let worker = scope.spawn(move || {
                // SAFETY: flags and the descriptor match the Linux syscalls.
                // Both restrictions apply only to this new worker and its children.
                if let Some(rules) = &project.rules {
                    unsafe {
                        os_result(
                            libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) as libc::c_long
                        )
                        .and_then(|_| {
                            os_result(libc::syscall(
                                libc::SYS_landlock_restrict_self,
                                rules.as_raw_fd(),
                                0u32,
                            ))
                        })
                        .map_err(|e| {
                            format!("Could not enable Linux restricted MCP filesystem access: {e}")
                        })?;
                    }
                }
                ACTIVE.set(Some((project, sender)));
                let result = operation();
                ACTIVE.set(None);
                result
            });
            // Fulfill only explicit runtime requests from our codecs (the FFmpeg
            // process receives pipes and no project paths, and WebP loads a library).
            for job in receiver {
                job();
            }
            worker
                .join()
                .map_err(|_| "Restricted MCP worker stopped unexpectedly".to_string())?
        })
    }
}

fn anchored_parent(project: &Project, path: &Path) -> io::Result<(File, CString)> {
    let relative = path.strip_prefix(&project.path).map_err(io::Error::other)?;
    let name = relative
        .file_name()
        .ok_or_else(|| io::Error::other("Cannot mutate the project root"))?;
    let parent = relative
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = open(
        project.dir.as_raw_fd(),
        parent,
        libc::O_PATH | libc::O_DIRECTORY,
        RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
    )?;
    Ok((
        directory,
        CString::new(name.as_bytes()).map_err(io::Error::other)?,
    ))
}
pub(super) fn create_directory(path: &Path, mode: libc::mode_t) -> Option<io::Result<(u64, u64)>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let (parent, name) = anchored_parent(project, path)?;
            os_result(
                unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), mode) } as libc::c_long,
            )?;
            let directory = open(
                parent.as_raw_fd(),
                Path::new(std::ffi::OsStr::from_bytes(name.as_bytes())),
                libc::O_PATH | libc::O_DIRECTORY,
                RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
            )?;
            let metadata = directory.metadata()?;
            Ok((metadata.dev(), metadata.ino()))
        })())
    })
}
pub(super) fn ensure_directory(path: &Path) -> Option<io::Result<()>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let relative = path.strip_prefix(&project.path).map_err(io::Error::other)?;
            let mut directory = project.dir.try_clone()?;
            for component in relative.components() {
                let std::path::Component::Normal(name) = component else {
                    return Err(io::Error::other("Invalid directory component"));
                };
                let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
                if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o777) } != 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                directory = open(
                    directory.as_raw_fd(),
                    Path::new(std::ffi::OsStr::from_bytes(name.as_bytes())),
                    libc::O_PATH | libc::O_DIRECTORY,
                    RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
                )?;
            }
            Ok(())
        })())
    })
}
pub(super) fn remove_entry(path: &Path, directory: bool) -> Option<io::Result<()>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let (parent, name) = anchored_parent(project, path)?;
            os_result(unsafe {
                libc::unlinkat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    if directory { libc::AT_REMOVEDIR } else { 0 },
                )
            } as libc::c_long)?;
            Ok(())
        })())
    })
}

pub(super) fn open_read(path: &Path) -> Option<io::Result<File>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let relative = path.strip_prefix(&project.path).map_err(io::Error::other)?;
            let input = open(
                project.dir.as_raw_fd(),
                relative,
                libc::O_RDONLY | libc::O_NONBLOCK,
                RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
            )?;
            let metadata = input.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Input must be a regular file",
                ));
            }
            if metadata.nlink() != 1 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Project-root access to files with hard links is disabled",
                ));
            }
            Ok(input)
        })())
    })
}

pub(super) fn create(path: &Path) -> Option<io::Result<File>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let relative = path.strip_prefix(&project.path).map_err(io::Error::other)?;
            open(
                project.dir.as_raw_fd(),
                relative,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
                RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
            )
        })())
    })
}

pub(super) fn metadata(path: &Path) -> Option<io::Result<std::fs::Metadata>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let relative = path.strip_prefix(&project.path).map_err(io::Error::other)?;
            let relative = if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                relative
            };
            open(
                project.dir.as_raw_fd(),
                relative,
                libc::O_PATH | libc::O_NOFOLLOW,
                RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
            )?
            .metadata()
        })())
    })
}

pub(super) fn directory_path(path: &Path) -> Option<io::Result<PathBuf>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let dir = project.open_directory(path)?;
            std::fs::read_link(format!("/proc/self/fd/{}", dir.as_raw_fd()))
        })())
    })
}

impl Project {
    fn open_directory(&self, path: &Path) -> io::Result<File> {
        let path = path.strip_prefix(&self.path).map_err(io::Error::other)?;
        let path = if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        };
        open(
            self.dir.as_raw_fd(),
            path,
            libc::O_PATH | libc::O_DIRECTORY,
            RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
        )
    }
}

fn directory_identity_of(dir: &File) -> io::Result<(u64, u64)> {
    let metadata = dir.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

pub(super) fn directory_identity(path: &Path) -> Option<io::Result<(u64, u64)>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some(
            project
                .open_directory(path)
                .and_then(|dir| directory_identity_of(&dir)),
        )
    })
}

fn unlink_child(parent: &File, name: &std::ffi::OsStr, directory: bool) -> io::Result<()> {
    let name = std::ffi::CString::new(name.as_bytes())?;
    // SAFETY: name and parent remain valid throughout the call.
    unsafe {
        os_result(libc::unlinkat(
            parent.as_raw_fd(),
            name.as_ptr(),
            if directory { libc::AT_REMOVEDIR } else { 0 },
        ) as libc::c_long)
    }
    .map(|_| ())
}

fn open_entry(directory: &File, name: &Path, flags: libc::c_int) -> io::Result<Option<File>> {
    match open(
        directory.as_raw_fd(),
        name,
        libc::O_PATH | libc::O_NOFOLLOW | flags,
        RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
    ) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn remove_tracked_files(
    directory: &File,
    known_child: Option<(&std::ffi::OsStr, (u64, u64))>,
    files: &[(PathBuf, (u64, u64))],
) -> io::Result<()> {
    let mut child = None;
    if let Some((name, expected)) = known_child
        && let Ok(Some(handle)) = open_entry(directory, Path::new(name), libc::O_DIRECTORY)
        && directory_identity_of(&handle)? == expected
    {
        child = Some((name, expected, handle));
    }
    for (relative, expected) in files {
        let Some(name) = relative.file_name() else {
            continue;
        };
        let parent = match relative.parent() {
            Some(parent) if parent.as_os_str().is_empty() => directory,
            Some(parent) => {
                let Some((child_name, _, handle)) = &child else {
                    continue;
                };
                if parent != Path::new(child_name) {
                    continue;
                }
                handle
            }
            None => continue,
        };
        if let Some(file) = open_entry(parent, Path::new(name), 0)? {
            let info = file.metadata()?;
            if info.is_file() && (info.dev(), info.ino()) == *expected {
                unlink_child(parent, name, false)?;
            }
        }
    }
    if let Some((name, expected, _held)) = child
        && let Some(current) = open_entry(directory, Path::new(name), libc::O_DIRECTORY)?
        && directory_identity_of(&current)? == expected
    {
        unlink_child(directory, name, true)?;
    }
    Ok(())
}

pub(super) fn remove_owned_dir_all(
    path: &Path,
    expected: (u64, u64),
    known_child: Option<(&std::ffi::OsStr, (u64, u64))>,
    files: &[(PathBuf, (u64, u64))],
) -> Option<io::Result<()>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let dir = project.open_directory(path)?;
            if directory_identity_of(&dir)? != expected {
                return Err(io::Error::other(
                    "The restricted export's staging directory changed",
                ));
            }
            remove_tracked_files(&dir, known_child, files)?;
            let parent =
                project.open_directory(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
            let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
            let current = open(
                parent.as_raw_fd(),
                Path::new(name),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS,
            )?;
            if directory_identity_of(&current)? != expected {
                return Err(io::Error::other(
                    "The restricted export's staging directory changed",
                ));
            }
            // The final rmdir never recurses into a replacement directory.
            unlink_child(&parent, name, true)
        })())
    })
}

pub(super) fn publish(
    source: &Path,
    destination: &Path,
    replace: bool,
    link: bool,
    expected: Option<(u64, u64)>,
) -> Option<io::Result<(u64, u64)>> {
    ACTIVE.with_borrow(|active| {
        let (project, _) = active.as_ref()?;
        Some((|| {
            let parent = |path: &Path| {
                let dir =
                    project.open_directory(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
                let leaf = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
                Ok::<_, io::Error>((dir, leaf.to_os_string()))
            };
            let (source, source_name) = parent(source)?;
            let (destination, destination_name) = parent(destination)?;
            let opened =
                open_entry(&source, Path::new(&source_name), 0)?.ok_or(io::ErrorKind::NotFound)?;
            let info = opened.metadata()?;
            let identity = (info.dev(), info.ino());
            if !info.is_file() || expected.is_some_and(|expected| identity != expected) {
                return Err(io::Error::other(
                    "The staged source file changed before publication",
                ));
            }
            let source_name = std::ffi::CString::new(source_name.as_bytes())?;
            let destination_name = std::ffi::CString::new(destination_name.as_bytes())?;
            // SAFETY: both directory descriptors and names remain valid
            // during the syscall. The kernel enforces Landlock again here if
            // either directory moved after it was opened. These syscalls still
            // resolve the leaf by name within the pinned source directory.
            unsafe {
                if link {
                    os_result(libc::linkat(
                        source.as_raw_fd(),
                        source_name.as_ptr(),
                        destination.as_raw_fd(),
                        destination_name.as_ptr(),
                        0,
                    ) as libc::c_long)
                } else {
                    os_result(libc::syscall(
                        libc::SYS_renameat2,
                        source.as_raw_fd(),
                        source_name.as_ptr(),
                        destination.as_raw_fd(),
                        destination_name.as_ptr(),
                        if replace { 0 } else { libc::RENAME_NOREPLACE },
                    ))
                }
                .map(|_| identity)
            }
        })())
    })
}

pub(crate) fn with_codec_runtime<T: Send + 'static>(
    runtime: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let Some(broker) =
        ACTIVE.with_borrow(|active| active.as_ref().map(|(_, broker)| broker.clone()))
    else {
        return Ok(runtime());
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    broker
        .send(Box::new(move || {
            let _ = sender.send(runtime());
        }))
        .map_err(|_| "The codec runtime worker stopped".to_string())?;
    receiver
        .recv()
        .map_err(|_| "The codec runtime worker stopped".to_string())
}

pub(crate) fn is_restricted() -> bool {
    ACTIVE.with_borrow(Option::is_some)
}

pub(super) fn active_root() -> Option<PathBuf> {
    ACTIVE.with_borrow(|active| active.as_ref().map(|(project, _)| project.path.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn restricted_worker_denies_symlink_swaps_and_publication_from_moved_directories() {
        let dir =
            std::env::temp_dir().join(format!("pixelcraft-landlock-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("project");
        let outside = dir.join("outside");
        fs::create_dir_all(root.join("destination")).unwrap();
        fs::create_dir_all(root.join("stage")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(root.join("stage/result"), b"new").unwrap();
        fs::write(outside.join("secret"), b"private").unwrap();
        let project = Project::new(&root).unwrap();
        let (ready, started) = mpsc::sync_channel(0);
        let (resume, changed) = mpsc::sync_channel(0);
        let outcome = std::thread::scope(|scope| {
            let project_root = &root;
            let test = scope.spawn(move || {
                project.run(move || {
                    let root = project_root;
                    let destination = root.join("destination");
                    let held_directory = open(
                        libc::AT_FDCWD,
                        &destination,
                        libc::O_PATH | libc::O_DIRECTORY,
                        0,
                    )
                    .unwrap();
                    ready.send(()).unwrap();
                    changed.recv().unwrap();
                    let read = fs::read(destination.join("secret"));
                    let publish = fs::rename(root.join("stage/result"), destination.join("result"));
                    let moved = open(
                        held_directory.as_raw_fd(),
                        Path::new("direct"),
                        libc::O_CREAT | libc::O_WRONLY,
                        0,
                    );
                    Ok((
                        read.err().unwrap().kind(),
                        publish.err().unwrap().kind(),
                        moved.err().unwrap().kind(),
                    ))
                })
            });
            started.recv().unwrap();
            fs::rename(root.join("destination"), outside.join("moved")).unwrap();
            symlink(&outside, root.join("destination")).unwrap();
            resume.send(()).unwrap();
            test.join().unwrap()
        });
        assert_eq!(
            outcome.unwrap(),
            (
                io::ErrorKind::PermissionDenied,
                io::ErrorKind::PermissionDenied,
                io::ErrorKind::PermissionDenied
            )
        );
        assert!(!outside.join("result").exists());
        assert!(!outside.join("moved/direct").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod editor_anchor_tests {
    use super::*;
    #[test]
    fn editor_directory_operations_reject_a_replaced_parent() {
        use std::os::unix::fs::symlink;
        let fixture =
            std::env::temp_dir().join(format!("pixelcraft-editor-dir-swap-{}", std::process::id()));
        let root = fixture.join("project");
        let outside = fixture.join("outside");
        std::fs::create_dir_all(root.join("parent")).unwrap();
        std::fs::create_dir_all(outside.join("keep")).unwrap();
        std::fs::write(outside.join("keep.txt"), b"untouched").unwrap();
        let root = root.canonicalize().unwrap();
        let editor = Project::new_editor(&root).unwrap();
        std::fs::rename(root.join("parent"), root.join("old-parent")).unwrap();
        symlink(&outside, root.join("parent")).unwrap();
        editor
            .run(|| {
                assert!(
                    create_directory(&root.join("parent/new"), 0o700)
                        .unwrap()
                        .is_err()
                );
                assert!(
                    ensure_directory(&root.join("parent/new/nested"))
                        .unwrap()
                        .is_err()
                );
                assert!(
                    remove_entry(&root.join("parent/keep"), true)
                        .unwrap()
                        .is_err()
                );
                assert!(
                    remove_entry(&root.join("parent/keep.txt"), false)
                        .unwrap()
                        .is_err()
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(
            std::fs::read(outside.join("keep.txt")).unwrap(),
            b"untouched"
        );
        assert!(outside.join("keep").is_dir());
        assert!(!outside.join("new").exists());
        drop(editor);
        std::fs::remove_dir_all(fixture).unwrap();
    }
    #[test]
    fn editor_anchor_does_not_require_or_disable_a_strict_mcp_ruleset() {
        let root =
            std::env::temp_dir().join(format!("pixelcraft-editor-anchor-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let editor = Project::new_editor(&root).unwrap();
        assert!(editor.rules.is_none());
        editor
            .run(|| {
                let output = root.join("owned.txt");
                super::super::mcp_io::write(&output, b"owned").map_err(|e| e.to_string())?;
                Ok(())
            })
            .unwrap();
        let strict = Project::new(&root).unwrap();
        assert!(strict.rules.is_some());
        drop(editor);
        drop(strict);
        std::fs::remove_dir_all(root).unwrap();
    }
}
