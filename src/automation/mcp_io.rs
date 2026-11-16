//! File I/O shared by codecs and MCP. The unrestricted editor and CLI use std.
use std::{
    fs::{self, File, Metadata},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) fn open(path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.open_read(path)) {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::open_read(path) {
        return result;
    }
    File::open(path)
}

pub(crate) fn create(path: impl AsRef<Path>) -> io::Result<File> {
    create_impl(path.as_ref(), false)
}

pub(crate) fn create_new(path: impl AsRef<Path>) -> io::Result<File> {
    create_impl(path.as_ref(), true)
}

fn create_impl(path: &Path, exclusive: bool) -> io::Result<File> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.create_file(path, true)) {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::create(path) {
        return result;
    }
    if exclusive {
        File::create_new(path)
    } else {
        File::create(path)
    }
}

pub(crate) fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let mut result = vec![];
    open(path)?.read_to_end(&mut result)?;
    Ok(result)
}

pub(crate) fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    create(path)?.write_all(bytes.as_ref())
}

pub(crate) fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let path = path.as_ref();
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.metadata(path)) {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::metadata(path) {
        return result;
    }
    fs::symlink_metadata(path)
}

pub(crate) fn children(path: &Path) -> io::Result<Vec<PathBuf>> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| {
        guard
            .read_dir(path)
            .map(|names| names.into_iter().map(|name| path.join(name)).collect())
    }) {
        return result;
    }
    fs::read_dir(path)?.map(|entry| Ok(entry?.path())).collect()
}

pub(crate) fn create_dir(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.create_dir(path)) {
        return result;
    }
    fs::create_dir(path)
}

pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        create_dir(path)
    }
}

pub(crate) fn create_dir_all(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.ensure_directory(path)) {
        return result;
    }
    fs::create_dir_all(path)
}

pub(crate) fn directory_identity(path: &Path) -> io::Result<(u64, u64)> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.directory_identity(path)) {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::directory_identity(path) {
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "No restricted directory handle",
    ))
}

pub(crate) fn remove_staging(
    path: &Path,
    expected: Option<(u64, u64)>,
    known_child: Option<(&std::ffi::OsStr, (u64, u64))>,
) -> io::Result<()> {
    if let Some(expected) = expected {
        #[cfg(windows)]
        if let Some(result) = super::mcp_access::windows(|guard| {
            guard.remove_owned_dir_all(path, expected, known_child)
        }) {
            return result;
        }
        #[cfg(target_os = "linux")]
        if let Some(result) = super::mcp_linux::remove_owned_dir_all(path, expected, known_child) {
            return result;
        }
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "No restricted directory handle",
        ));
    }
    fs::remove_dir_all(path)
}

pub(crate) fn remove_dir(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.remove_dir(path)) {
        return result;
    }
    fs::remove_dir(path)
}

pub(crate) fn remove_file(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.remove_file(path)) {
        return result;
    }
    fs::remove_file(path)
}

pub(crate) fn rename(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) =
        super::mcp_access::windows(|guard| guard.rename(source, destination, true))
    {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::publish(source, destination, true, false) {
        return result;
    }
    fs::rename(source, destination)
}

pub(crate) fn publish_new(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) =
        super::mcp_access::windows(|guard| guard.rename(source, destination, false))
    {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::publish(source, destination, false, false) {
        return result;
    }
    fs::hard_link(source, destination)
}

pub(crate) fn hard_link(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if let Some(result) = super::mcp_access::windows(|guard| guard.hard_link(source, destination)) {
        return result;
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::publish(source, destination, false, true) {
        return result;
    }
    fs::hard_link(source, destination)
}

pub(crate) fn exists(path: &Path) -> io::Result<bool> {
    match metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn directory_path(path: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(result) = super::mcp_linux::directory_path(path) {
        return result;
    }
    Ok(path.to_path_buf())
}

pub(crate) fn copy(source: &Path, destination: &Path) -> io::Result<()> {
    io::copy(&mut open(source)?, &mut create_new(destination)?).map(|_| ())
}

pub(crate) fn open_image(path: impl AsRef<Path>) -> image::ImageResult<image::DynamicImage> {
    let path = path.as_ref();
    image_reader(path)?.decode()
}

pub(crate) fn image_reader(path: &Path) -> io::Result<image::ImageReader<BufReader<File>>> {
    let mut reader = image::ImageReader::new(BufReader::new(open(path)?));
    if let Some(format) = path
        .extension()
        .and_then(image::ImageFormat::from_extension)
    {
        reader.set_format(format);
    }
    Ok(reader)
}

pub(crate) fn write_image(
    path: impl AsRef<Path>,
    image: &image::RgbaImage,
) -> image::ImageResult<()> {
    let path = path.as_ref();
    write_image_as(path, image, image::ImageFormat::from_path(path)?)
}

pub(crate) fn write_image_as(
    path: impl AsRef<Path>,
    pixels: &image::RgbaImage,
    format: image::ImageFormat,
) -> image::ImageResult<()> {
    let mut writer = io::BufWriter::new(create(path)?);
    pixels.write_to(&mut writer, format)?;
    writer.flush()?;
    Ok(())
}
