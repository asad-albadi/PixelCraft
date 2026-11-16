//! Resolve each component from an already-open directory. A pathname check
//! followed by an independent open would reintroduce the reparse-point race.
use std::{
    ffi::{OsStr, OsString, c_void},
    fs::{self, File},
    io,
    mem::{offset_of, size_of},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle, RawHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
    ptr,
};

const FILE_READ_DATA: u32 = 0x0001;
const FILE_WRITE_DATA: u32 = 0x0002;
const FILE_TRAVERSE: u32 = 0x0020;
const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const DELETE: u32 = 0x0001_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const READ_ACCESS: u32 = FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
const DIRECTORY_ACCESS: u32 = READ_ACCESS | FILE_TRAVERSE;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const FILE_SHARE_DELETE: u32 = 4;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_DIRECTORY_FILE: u32 = 1;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0020;
const FILE_NON_DIRECTORY_FILE: u32 = 0x0040;
const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_OPEN: u32 = 1;
const FILE_CREATE: u32 = 2;
const FILE_OPEN_IF: u32 = 3;
const OBJ_CASE_INSENSITIVE: u32 = 0x0040;
const OBJ_DONT_REPARSE: u32 = 0x1000;
const FILE_RENAME_INFORMATION: u32 = 10;
const FILE_LINK_INFORMATION: u32 = 11;
const FILE_NAMES_INFORMATION: u32 = 12;
const FILE_DISPOSITION_INFORMATION: u32 = 13;
const STATUS_NO_MORE_FILES: i32 = 0x8000_0006u32 as i32;
const STATUS_NO_SUCH_FILE: i32 = 0xc000_000fu32 as i32;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: RawHandle,
    object_name: *const UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}

#[repr(C)]
#[derive(Default)]
struct FileInformation {
    attributes: u32,
    times: [[u32; 2]; 3],
    volume_serial: u32,
    size_high: u32,
    size_low: u32,
    number_of_links: u32,
    index_high: u32,
    index_low: u32,
}

// FileRenameInformation and FileLinkInformation have the same request layout.
#[repr(C)]
struct LinkName {
    replace_if_exists: u8,
    root_directory: RawHandle,
    file_name_length: u32,
    file_name: [u16; 1],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        handle: *mut RawHandle,
        desired_access: u32,
        attributes: *const ObjectAttributes,
        status: *mut IoStatusBlock,
        allocation_size: *const i64,
        file_attributes: u32,
        share_access: u32,
        create_disposition: u32,
        create_options: u32,
        ea_buffer: *const c_void,
        ea_length: u32,
    ) -> i32;

    fn NtSetInformationFile(
        handle: RawHandle,
        status: *mut IoStatusBlock,
        file_information: *const c_void,
        length: u32,
        information_class: u32,
    ) -> i32;

    fn NtQueryDirectoryFile(
        handle: RawHandle,
        event: RawHandle,
        apc_routine: *const c_void,
        apc_context: *const c_void,
        status: *mut IoStatusBlock,
        file_information: *mut c_void,
        length: u32,
        information_class: u32,
        single_entry: u8,
        file_name: *const UnicodeString,
        restart: u8,
    ) -> i32;

    fn RtlNtStatusToDosError(status: i32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn ReOpenFile(handle: RawHandle, access: u32, share: u32, flags: u32) -> RawHandle;
    fn GetFileInformationByHandle(handle: RawHandle, information: *mut FileInformation) -> i32;
}

fn reopen_directory(directory: &File) -> io::Result<File> {
    // SAFETY: ReOpenFile keeps the same directory object but sets up a
    // separate enumeration cursor. The existing handle remains open.
    let handle = unsafe {
        ReOpenFile(
            directory.as_raw_handle(),
            DIRECTORY_ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
        )
    };
    if handle as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: success returns a new handle owned by the caller.
    let file = unsafe { File::from_raw_handle(handle) };
    reject_reparse(&file)?;
    Ok(file)
}

fn file_information(file: &File) -> io::Result<FileInformation> {
    let mut info = FileInformation::default();
    // SAFETY: info is writable storage with the native FileInformation layout.
    let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info)
}

fn reject_hard_links(file: &File) -> io::Result<()> {
    if file_information(file)?.number_of_links != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Cannot replace a file with multiple hard links",
        ));
    }
    Ok(())
}

fn directory_identity(file: &File) -> io::Result<(u64, u64)> {
    let info = file_information(file)?;
    let index = u64::from(info.index_high) << 32 | u64::from(info.index_low);
    Ok((u64::from(info.volume_serial), index))
}

fn nt_result(status: i32) -> io::Result<()> {
    if status >= 0 {
        Ok(())
    } else {
        // SAFETY: this scalar conversion has no pointer arguments.
        Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ))
    }
}

fn valid_component(name: &OsStr) -> io::Result<usize> {
    let mut count = 0;
    for unit in name.encode_wide() {
        // Reject separators, NTFS streams, and Win32 device path syntax.
        if matches!(unit, 0 | 47 | 58 | 92) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid path component",
            ));
        }
        count += 1;
    }
    let spelling = name.to_string_lossy();
    let stem = spelling
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ');
    let reserved = matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.get(..3)
            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
            && stem.get(3..).is_some_and(|s| {
                matches!(
                    s,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
    });
    if count == 0 || count > 255 || spelling.ends_with([' ', '.']) || reserved {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid path component",
        ));
    }
    Ok(count)
}

fn reject_reparse(file: &File) -> io::Result<()> {
    if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows reparse points are not allowed in the project root",
        ));
    }
    Ok(())
}

fn open_child(
    parent: &File,
    name: &OsStr,
    access: u32,
    share: u32,
    disposition: u32,
    options: u32,
) -> io::Result<File> {
    let length = (valid_component(name)? * size_of::<u16>()) as u16;
    let wide: Vec<u16> = name.encode_wide().collect();
    let name = UnicodeString {
        length,
        maximum_length: length,
        buffer: wide.as_ptr(),
    };
    let attributes = ObjectAttributes {
        length: size_of::<ObjectAttributes>() as u32,
        root_directory: parent.as_raw_handle(),
        object_name: &name,
        attributes: OBJ_DONT_REPARSE | OBJ_CASE_INSENSITIVE,
        security_descriptor: ptr::null_mut(),
        security_quality_of_service: ptr::null_mut(),
    };
    let mut status = IoStatusBlock::default();
    let mut handle = ptr::null_mut();
    // SAFETY: all buffers are initialized and live through the synchronous call.
    // The directory handle and counted UTF-16 name refer to a single child.
    nt_result(unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut status,
            ptr::null(),
            0,
            share,
            disposition,
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    })?;
    // SAFETY: a successful NtCreateFile transfers ownership of the returned handle.
    let file = unsafe { File::from_raw_handle(handle) };
    reject_reparse(&file)?;
    Ok(file)
}

fn open_directory(parent: &File, name: &OsStr, create: bool) -> io::Result<File> {
    open_child(
        parent,
        name,
        DIRECTORY_ACCESS,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        if create { FILE_OPEN_IF } else { FILE_OPEN },
        FILE_DIRECTORY_FILE,
    )
}

fn directory_names(directory: &File) -> io::Result<Vec<OsString>> {
    let mut names = Vec::new();
    let mut restart = true;
    // The native FILE_NAMES_INFORMATION records require aligned storage.
    let mut buffer = [0u64; 1024];
    loop {
        let mut io_status = IoStatusBlock::default();
        // SAFETY: this synchronous handle and the output buffer remain valid
        // for the call. The filename mask, event, and APC parameters are unused.
        let status = unsafe {
            NtQueryDirectoryFile(
                directory.as_raw_handle(),
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                &mut io_status,
                buffer.as_mut_ptr().cast(),
                size_of_val(&buffer) as u32,
                FILE_NAMES_INFORMATION,
                0,
                ptr::null(),
                u8::from(restart),
            )
        };
        if status == STATUS_NO_MORE_FILES || status == STATUS_NO_SUCH_FILE {
            return Ok(names);
        }
        nt_result(status)?;
        restart = false;
        if io_status.information == 0 || io_status.information > size_of_val(&buffer) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid directory response",
            ));
        }
        // SAFETY: the reported length was checked against initialized storage.
        let mut data = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), io_status.information)
        };
        loop {
            if data.len() < 12 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid directory entry",
                ));
            }
            let next = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
            let length = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
            if !length.is_multiple_of(2)
                || length > data.len() - 12
                || (next != 0 && (next < 12 + length || next >= data.len()))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid directory entry",
                ));
            }
            let wide: Vec<u16> = data[12..12 + length]
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .map(u16::from_le_bytes)
                .collect();
            let name = OsString::from_wide(&wide);
            if name != "." && name != ".." {
                valid_component(&name)?;
                names.push(name);
            }
            if next == 0 {
                break;
            }
            data = &data[next..];
        }
    }
}

fn delete(file: File) -> io::Result<()> {
    let delete_file = 1u8;
    let mut status = IoStatusBlock::default();
    // SAFETY: file is a synchronous handle opened with DELETE, and the
    // one-byte FILE_DISPOSITION_INFORMATION remains live for the call.
    nt_result(unsafe {
        NtSetInformationFile(
            file.as_raw_handle(),
            &mut status,
            ptr::from_ref(&delete_file).cast(),
            1,
            FILE_DISPOSITION_INFORMATION,
        )
    })?;
    // Windows removes the marked directory entry when the handle is closed.
    drop(file);
    Ok(())
}

pub(super) struct Guard {
    root: PathBuf,
    _ancestors: Vec<File>,
    directory: File,
}

struct Directory {
    file: File,
    // A descendant handle alone should not be relied on to preserve the
    // location of every directory traversed on the way to it.
    _ancestors: Vec<File>,
}

impl Guard {
    /// The caller supplies a canonical Windows disk or UNC path.
    pub(super) fn new(root: &Path) -> io::Result<Self> {
        let invalid_root = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Expected an absolute Windows project root: {}",
                    root.display()
                ),
            )
        };
        let mut parts = root.components();
        let Some(Component::Prefix(prefix)) = parts.next() else {
            return Err(invalid_root());
        };
        if !matches!(
            prefix.kind(),
            Prefix::Disk(_)
                | Prefix::VerbatimDisk(_)
                | Prefix::UNC(_, _)
                | Prefix::VerbatimUNC(_, _)
        ) || parts.next() != Some(Component::RootDir)
        {
            return Err(invalid_root());
        }
        for part in parts.clone() {
            match part {
                Component::Normal(name) => {
                    valid_component(name)?;
                }
                _ => return Err(invalid_root()),
            }
        }
        let mut volume = PathBuf::from(prefix.as_os_str());
        volume.push(r"\");
        let mut directory = fs::OpenOptions::new()
            .access_mode(DIRECTORY_ACCESS)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(&volume)?;
        reject_reparse(&directory)?;
        let mut ancestors = Vec::new();
        for part in parts {
            let Component::Normal(name) = part else {
                unreachable!("root components were validated");
            };
            let next = open_directory(&directory, name, false)?;
            ancestors.push(std::mem::replace(&mut directory, next));
        }
        Ok(Self {
            root: root.into(),
            _ancestors: ancestors,
            directory,
        })
    }

    pub(super) fn identity(&self) -> io::Result<(u64, u64)> {
        directory_identity(&self.directory)
    }

    pub(super) fn directory_identity(&self, path: &Path) -> io::Result<(u64, u64)> {
        directory_identity(&self.walk(path, false)?.file)
    }

    fn suffix<'a>(&self, path: &'a Path) -> io::Result<&'a Path> {
        let outside = || {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Path escapes the project root: {}", path.display()),
            )
        };
        let suffix = path.strip_prefix(&self.root).map_err(|_| outside())?;
        for part in suffix.components() {
            if !matches!(part, Component::Normal(name) if valid_component(name).is_ok()) {
                return Err(outside());
            }
        }
        Ok(suffix)
    }

    fn walk(&self, path: &Path, create: bool) -> io::Result<Directory> {
        let suffix = self.suffix(path)?;
        let mut directory = self.directory.try_clone()?;
        let mut ancestors = Vec::new();
        for part in suffix.components() {
            let Component::Normal(name) = part else {
                unreachable!("component was validated");
            };
            let next = open_directory(&directory, name, create)?;
            ancestors.push(std::mem::replace(&mut directory, next));
        }
        Ok(Directory {
            file: directory,
            _ancestors: ancestors,
        })
    }

    fn parent<'a>(&self, path: &'a Path) -> io::Result<(Directory, &'a OsStr)> {
        if self.suffix(path)?.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Expected a child below the project root: {}",
                    path.display()
                ),
            ));
        }
        let name = path.file_name().unwrap();
        let parent = self.walk(path.parent().unwrap(), false)?;
        Ok((parent, name))
    }

    pub(super) fn open_read(&self, path: &Path) -> io::Result<File> {
        let (parent, name) = self.parent(path)?;
        open_child(
            &parent.file,
            name,
            READ_ACCESS,
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )
    }

    pub(super) fn ensure_directory(&self, path: &Path) -> io::Result<()> {
        self.walk(path, true).map(drop)
    }

    pub(super) fn create_dir(&self, path: &Path) -> io::Result<()> {
        let (parent, name) = self.parent(path)?;
        open_child(
            &parent.file,
            name,
            DIRECTORY_ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE,
        )
        .map(drop)
    }

    pub(super) fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        if self.suffix(path)?.as_os_str().is_empty() {
            reject_reparse(&self.directory)?;
            return self.directory.metadata();
        }
        let (parent, name) = self.parent(path)?;
        open_child(
            &parent.file,
            name,
            READ_ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            0,
        )?
        .metadata()
    }

    pub(super) fn read_dir(&self, path: &Path) -> io::Result<Vec<OsString>> {
        if self.suffix(path)?.as_os_str().is_empty() {
            return directory_names(&reopen_directory(&self.directory)?);
        }
        let directory = self.walk(path, false)?;
        directory_names(&directory.file)
    }

    pub(super) fn create_file(&self, path: &Path, exclusive: bool) -> io::Result<File> {
        let (parent, name) = self.parent(path)?;
        let open = |disposition| {
            open_child(
                &parent.file,
                name,
                READ_ACCESS | FILE_WRITE_DATA,
                FILE_SHARE_READ,
                disposition,
                FILE_NON_DIRECTORY_FILE,
            )
        };
        match open(FILE_CREATE) {
            Err(error) if !exclusive && error.kind() == io::ErrorKind::AlreadyExists => {
                let file = open(FILE_OPEN)?;
                // Check the opened object before mutating it. Do not use
                // FILE_OVERWRITE_IF, which truncates before checking attributes.
                reject_hard_links(&file)?;
                file.set_len(0)?;
                Ok(file)
            }
            result => result,
        }
    }

    pub(super) fn remove_file(&self, path: &Path) -> io::Result<()> {
        let (parent, name) = self.parent(path)?;
        delete(open_child(
            &parent.file,
            name,
            READ_ACCESS | DELETE,
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )?)
    }

    pub(super) fn remove_dir(&self, path: &Path) -> io::Result<()> {
        let (parent, name) = self.parent(path)?;
        delete(open_child(
            &parent.file,
            name,
            DIRECTORY_ACCESS | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?)
    }

    #[cfg(test)]
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let expected = self.directory_identity(path)?;
        self.remove_owned_dir_all(path, expected, None)
    }

    pub(super) fn remove_owned_dir_all(
        &self,
        path: &Path,
        expected: (u64, u64),
        known_child: Option<(&OsStr, (u64, u64))>,
    ) -> io::Result<()> {
        let (parent, name) = self.parent(path)?;
        let directory = open_child(
            &parent.file,
            name,
            DIRECTORY_ACCESS | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        if directory_identity(&directory)? != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The restricted export's staging directory changed",
            ));
        }
        let names = directory_names(&directory)?;
        let mut directories = vec![(directory, names.into_iter())];
        while let Some((directory, names)) = directories.last_mut() {
            if let Some(name) = names.next() {
                let child = open_child(
                    directory,
                    &name,
                    DIRECTORY_ACCESS | DELETE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    FILE_OPEN,
                    0,
                )?;
                if child.metadata()?.is_dir() {
                    let Some((allowed_name, expected)) =
                        known_child.filter(|_| directories.len() == 1)
                    else {
                        continue;
                    };
                    if name != allowed_name || directory_identity(&child)? != expected {
                        continue;
                    }
                    let names = directory_names(&child)?;
                    directories.push((child, names.into_iter()));
                } else {
                    delete(child)?;
                }
            } else {
                delete(directories.pop().unwrap().0)?;
            }
        }
        Ok(())
    }

    pub(super) fn rename(
        &self,
        source: &Path,
        destination: &Path,
        replace: bool,
    ) -> io::Result<()> {
        self.publish(source, destination, FILE_RENAME_INFORMATION, replace)
    }

    pub(super) fn hard_link(&self, source: &Path, destination: &Path) -> io::Result<()> {
        self.publish(source, destination, FILE_LINK_INFORMATION, false)
    }

    fn publish(
        &self,
        source: &Path,
        destination: &Path,
        information_class: u32,
        replace: bool,
    ) -> io::Result<()> {
        // Validate both paths before opening either of them.
        self.suffix(destination)?;
        let (source_parent, source_name) = self.parent(source)?;
        let (destination_parent, destination_name) = self.parent(destination)?;
        let access = READ_ACCESS
            | if information_class == FILE_RENAME_INFORMATION {
                DELETE
            } else {
                0
            };
        let file = open_child(
            &source_parent.file,
            source_name,
            access,
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )?;

        let wide: Vec<u16> = destination_name.encode_wide().collect();
        let name_bytes = wide.len() * size_of::<u16>();
        let name_offset = offset_of!(LinkName, file_name);
        let size = size_of::<LinkName>() + name_bytes;
        // usize storage provides the pointer alignment required by LinkName.
        let mut buffer = vec![0usize; size.div_ceil(size_of::<usize>())];
        let request = buffer.as_mut_ptr().cast::<LinkName>();
        let mut status = IoStatusBlock::default();
        // SAFETY: request points to aligned, initialized storage large enough
        // for the header and counted name. Both file handles live through the call.
        unsafe {
            (*request).replace_if_exists = u8::from(replace);
            (*request).root_directory = if source.parent() == destination.parent() {
                ptr::null_mut()
            } else {
                destination_parent.file.as_raw_handle()
            };
            (*request).file_name_length = name_bytes as u32;
            ptr::copy_nonoverlapping(
                wide.as_ptr(),
                request.cast::<u8>().add(name_offset).cast(),
                wide.len(),
            );
            nt_result(NtSetInformationFile(
                file.as_raw_handle(),
                &mut status,
                request.cast(),
                size as u32,
                information_class,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "pixelcraft-windows-guard-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn guarded_open_never_traverses_a_junction() {
        let fixture = Fixture::new();
        let root = fixture.0.join("project");
        let outside = fixture.0.join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("source.pixel"), "outside").unwrap();
        let junction = root.join("redirected");
        let result = std::process::Command::new("cmd")
            .current_dir(&fixture.0)
            .args(["/d", "/c", "mklink", "/j", r"project\redirected", "outside"])
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        assert!(junction.exists());

        let root = root.canonicalize().unwrap();
        let guard = Guard::new(&root).unwrap();
        assert!(
            guard
                .open_read(&root.join("redirected/source.pixel"))
                .is_err()
        );
        assert!(
            guard
                .ensure_directory(&root.join("redirected/output"))
                .is_err()
        );
        assert!(guard.remove_dir_all(&root.join("redirected")).is_err());
        assert_eq!(
            fs::read_to_string(outside.join("source.pixel")).unwrap(),
            "outside"
        );
        assert!(!outside.join("output").exists());
    }

    #[test]
    fn anchored_output_can_replace_without_releasing_the_parent() {
        let fixture = Fixture::new();
        let root = fixture.0.canonicalize().unwrap();
        let guard = Guard::new(&root).unwrap();
        let dir = root.join("nested").join("output");
        guard.ensure_directory(&dir).unwrap();
        assert!(fs::rename(&root, root.with_extension("moved")).is_err());

        let source = root.join("new.pixel");
        let target = dir.join("old.pixel");
        fs::write(&source, "new").unwrap();
        fs::write(&target, "old").unwrap();
        assert!(guard.rename(&source, &target, false).is_err());

        let mut previous = String::new();
        guard
            .open_read(&target)
            .unwrap()
            .read_to_string(&mut previous)
            .unwrap();
        assert_eq!(previous, "old");
        guard.rename(&source, &target, true).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!source.exists());
    }

    #[test]
    fn scoped_create_list_link_and_cleanup_use_the_anchored_tree() {
        let fixture = Fixture::new();
        let root = fixture.0.canonicalize().unwrap();
        let server = Guard::new(&root).unwrap();
        let request = Guard::new(&root).unwrap();
        assert_eq!(request.identity().unwrap(), server.identity().unwrap());

        let stage = root.join("stage");
        request.create_dir(&stage).unwrap();
        assert_eq!(
            request.create_dir(&stage).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        let assets = stage.join("assets");
        request.ensure_directory(&assets).unwrap();
        assert!(request.metadata(&assets).unwrap().is_dir());

        let source = assets.join("source.pixel");
        request
            .create_file(&source, true)
            .unwrap()
            .write_all(b"source")
            .unwrap();
        assert_eq!(
            request.create_file(&source, true).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists,
        );
        request
            .create_file(&source, false)
            .unwrap()
            .write_all(b"new")
            .unwrap();
        let output = root.join("published.pixel");
        request.hard_link(&source, &output).unwrap();
        assert_eq!(
            request.create_file(&source, false).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied,
        );
        assert_eq!(fs::read(&output).unwrap(), b"new");
        assert!(request.remove_dir(&stage).is_err());
        assert!(source.exists());
        assert_eq!(
            request.read_dir(&assets).unwrap(),
            vec![OsString::from("source.pixel")]
        );
        let expected = [OsString::from("published.pixel"), OsString::from("stage")];
        for guard in [&server, &request, &server] {
            let mut names = guard.read_dir(&root).unwrap();
            names.sort();
            assert_eq!(names, expected);
        }
        request.remove_dir_all(&stage).unwrap();
        assert!(!stage.exists());
        assert_eq!(fs::read(&output).unwrap(), b"new");
        request.remove_file(&output).unwrap();
        request.create_dir(&stage).unwrap();
        request.remove_dir(&stage).unwrap();
        assert!(request.read_dir(&root).unwrap().is_empty());
        assert_eq!(
            request.open_read(&output).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
}
