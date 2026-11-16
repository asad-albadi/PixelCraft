//! GitHub release lookup; all network work runs off the editor thread.
use semver::Version;
use std::{
    fs,
    hash::{BuildHasher, RandomState},
    io::{self, Read, Write},
    path::Path,
    sync::mpsc,
    time::Duration,
};

pub const INSTALLED_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    version: Version,
    assets: Vec<(String, String)>,
}

impl Release {
    pub fn parse(body: &str) -> Result<Self, String> {
        let json: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        if json["draft"].as_bool() == Some(true) || json["prerelease"].as_bool() == Some(true) {
            return Err("No stable published release found.".into());
        }
        let tag = json["tag_name"]
            .as_str()
            .ok_or("No published release found.")?;
        let version = Version::parse(tag.trim_start_matches('v'))
            .map_err(|_| "Release tag is not a version number.")?;
        if !version.pre.is_empty() {
            return Err("No stable published release found.".into());
        }
        Ok(Self {
            tag: tag.into(),
            version,
            assets: json["assets"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| {
                            Some((
                                v["name"].as_str()?.to_owned(),
                                v["browser_download_url"].as_str()?.to_owned(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub fn is_newer_than(&self, installed: &str) -> bool {
        Version::parse(installed).is_ok_and(|current| self.version.cmp_precedence(&current).is_gt())
    }
}

pub fn check(
    repository: &str,
    ctx: eframe::egui::Context,
) -> mpsc::Receiver<Result<Release, String>> {
    let (tx, rx) = mpsc::channel();
    let endpoint = format!("https://api.github.com/repos/{repository}/releases/latest");
    std::thread::spawn(move || {
        let result = (|| {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                        .build(),
                )
                .timeout_global(Some(Duration::from_secs(15)))
                .build()
                .into();
            let mut response = agent
                .get(&endpoint)
                .header(
                    "User-Agent",
                    concat!("PixelCraft/", env!("CARGO_PKG_VERSION")),
                )
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .call()
                .map_err(|e| format!("Could not check GitHub: {e}"))?;
            let body = response
                .body_mut()
                .with_config()
                .limit(1024 * 1024)
                .read_to_string()
                .map_err(|e| format!("Could not read release: {e}"))?;
            Release::parse(&body)
        })();
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}

/// Accept only a GitHub owner/repository, never an arbitrary update endpoint.
pub fn repository_slug(input: &str) -> Option<String> {
    let slug = input.trim().trim_end_matches('/');
    let slug = slug.strip_prefix("https://github.com/").unwrap_or(slug);
    let slug = slug.strip_suffix(".git").unwrap_or(slug);
    let parts: Vec<_> = slug.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return None;
    }
    Some(slug.into())
}

// Legacy about.json files remain readable, but build identity is authoritative.
#[derive(serde::Deserialize)]
#[serde(default)]
pub struct AboutInfo {
    #[serde(skip_deserializing)]
    pub developer: String,
    #[serde(skip_deserializing)]
    pub repository: String,
}
impl Default for AboutInfo {
    fn default() -> Self {
        Self {
            developer: option_env!("PIXELCRAFT_DEVELOPER")
                .unwrap_or(env!("CARGO_PKG_AUTHORS"))
                .into(),
            repository: option_env!("PIXELCRAFT_REPOSITORY").unwrap_or("").into(),
        }
    }
}
impl AboutInfo {
    pub fn load() -> Self {
        std::fs::read(crate::extensions::config_directory().join("about.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn github_url(&self) -> Option<String> {
        repository_slug(&self.repository).map(|slug| format!("https://github.com/{slug}"))
    }
    pub fn releases_url(&self) -> Option<String> {
        self.github_url()
            .map(|url| format!("{url}/releases/latest"))
    }
}

#[derive(Debug)]
pub struct VerifiedDownload {
    path: std::path::PathBuf,
    digest: String,
    size: u64,
    pub name: String,
}
/// Parse the standard SHA-256 sidecar and bind it to the selected artifact.
pub fn verify_checksum(bytes: &[u8], sidecar: &str, name: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let expected = sidecar
        .lines()
        .find_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.len() == 2 && parts[1].trim_start_matches('*') == name {
                Some(parts[0])
            } else {
                None
            }
        })
        .ok_or("Checksum file does not name this download")?;
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid SHA-256 checksum".into());
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("Update checksum mismatch; download discarded".into());
    }
    Ok(actual)
}
impl Release {
    pub fn download_available(&self) -> bool {
        self.platform_asset().is_some()
    }
    fn platform_asset(&self) -> Option<&(String, String)> {
        if std::env::consts::ARCH != "x86_64" {
            return None;
        }
        let suffix = if cfg!(target_os = "windows") {
            "-x86_64-Setup.exe"
        } else if cfg!(target_os = "linux") {
            "-x86_64.AppImage"
        } else {
            return None;
        };
        self.assets.iter().find(|(name, _)| {
            name.starts_with("PixelCraft-")
                && name.ends_with(suffix)
                && !name.contains('/')
                && !name.contains('\\')
        })
    }
}
fn publish_file(
    destination: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let directory = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing update directory"))?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    for attempt in 0..16 {
        let nonce = RandomState::new().hash_one(attempt);
        let temporary = directory.join(format!(".pixelcraft-update-{nonce:016x}.tmp"));
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = write(&mut file).and_then(|()| file.sync_all());
        drop(file);
        let result = result.and_then(|()| publish_rename(&temporary, destination));
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Could not create a unique temporary update file",
    ))
}

fn publish_rename(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        const ERROR_SHARING_VIOLATION: i32 = 32;
        for _ in 0..5 {
            match fs::rename(source, destination) {
                Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return result,
            }
        }
    }
    fs::rename(source, destination)
}

fn cache_download(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<std::path::PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(name);
    publish_file(&path, |file| file.write_all(bytes))?;
    Ok(path)
}

pub fn download(
    release: Release,
    repository: String,
    directory: std::path::PathBuf,
    ctx: eframe::egui::Context,
) -> mpsc::Receiver<Result<VerifiedDownload, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let slug = repository_slug(&repository).ok_or("Invalid update repository")?;
            let (name, url) = release
                .platform_asset()
                .ok_or("No installer for this platform")?;
            let checksum_name = format!("{name}.sha256");
            let checksum_url = &release
                .assets
                .iter()
                .find(|(n, _)| n == &checksum_name)
                .ok_or("Release has no checksum for this installer")?
                .1;
            let prefix = format!("https://github.com/{slug}/releases/download/");
            if !url.starts_with(&prefix) || !checksum_url.starts_with(&prefix) {
                return Err(
                    "Release assets must belong to the configured GitHub repository".into(),
                );
            }
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(300)))
                .build()
                .into();
            let checksum = agent
                .get(checksum_url)
                .call()
                .map_err(|e| e.to_string())?
                .body_mut()
                .with_config()
                .limit(16384)
                .read_to_string()
                .map_err(|e| e.to_string())?;
            let bytes = agent
                .get(url)
                .call()
                .map_err(|e| e.to_string())?
                .body_mut()
                .with_config()
                .limit(512 * 1024 * 1024)
                .read_to_vec()
                .map_err(|e| e.to_string())?;
            let digest = verify_checksum(&bytes, &checksum, name)?;
            let path = cache_download(&directory, name, &bytes).map_err(|e| e.to_string())?;
            Ok(VerifiedDownload {
                path,
                digest,
                size: bytes.len() as u64,
                name: name.clone(),
            })
        })();
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}

fn open_installer(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The downloaded installer must be a regular file",
            ));
        }
    }
    Ok(file)
}

/// Call only after the person confirms installation. Recheck disk bytes before use.
pub fn install(
    download: &VerifiedDownload,
    target: Option<&std::path::Path>,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let mut installer = open_installer(&download.path).map_err(|e| e.to_string())?;
    let metadata = installer.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() != download.size {
        return Err("Downloaded installer changed; download it again".into());
    }
    let mut bytes = Vec::new();
    (&mut installer)
        .take(download.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != download.size
        || format!("{:x}", Sha256::digest(&bytes)) != download.digest
    {
        return Err("Downloaded installer changed; download it again".into());
    }
    #[cfg(target_os = "windows")]
    {
        let _ = target;
        // Keep the executable protected from writes and renames until the loader has opened it.
        let launch = std::process::Command::new(&download.path).spawn();
        drop(installer);
        launch.map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let target = target.ok_or("Choose an AppImage installation path")?;
        if target == download.path {
            return Err("Choose an installation path outside the download cache".into());
        }
        match fs::File::open(target) {
            Ok(mut old) => {
                let permissions = old.metadata().map_err(|e| e.to_string())?.permissions();
                publish_file(&target.with_extension("AppImage.previous"), |backup| {
                    io::copy(&mut old, backup)?;
                    backup.set_permissions(permissions)
                })
                .map_err(|e| e.to_string())?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        publish_file(target, |file| {
            file.write_all(&bytes)?;
            file.set_permissions(fs::Permissions::from_mode(0o755))
        })
        .map_err(|e| e.to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = target;
        Err("In-app installation is unavailable on this platform".into())
    }
}

#[cfg(test)]
mod installation_regressions {
    #[test]
    fn concurrent_update_cache_writes_publish_complete_files() {
        use std::{
            fs,
            sync::{Arc, Barrier},
        };

        let root =
            std::env::temp_dir().join(format!("pixelcraft-update-race-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let name = "PixelCraft-test-x86_64.AppImage";
        let barrier = Arc::new(Barrier::new(8));
        let payloads: Vec<_> = (0..8).map(|value| vec![value; 512 * 1024]).collect();
        let results = std::thread::scope(|scope| {
            let threads: Vec<_> = payloads
                .iter()
                .map(|bytes| {
                    let barrier = barrier.clone();
                    let root = &root;
                    scope.spawn(move || {
                        barrier.wait();
                        super::cache_download(root, name, bytes)
                    })
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        let entries = fs::read_dir(&root).unwrap().count();
        let cached = fs::read(root.join(name)).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert!(results.iter().all(Result::is_ok), "{results:?}");
        assert!(payloads.contains(&cached));
        assert_eq!(entries, 1);
    }

    #[cfg(unix)]
    #[test]
    fn update_cache_ignores_a_preexisting_partial_symlink() {
        use std::{fs, os::unix::fs::symlink};

        let root =
            std::env::temp_dir().join(format!("pixelcraft-update-cache-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let other = root.join("other-file");
        fs::write(&other, b"keep this file").unwrap();
        let name = "PixelCraft-test-x86_64.AppImage";
        symlink(&other, root.join(format!("{name}.partial"))).unwrap();

        let result = super::cache_download(&root, name, b"verified update").unwrap();
        let cached = fs::read(result).unwrap();
        let unrelated = fs::read(other).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert_eq!(cached, b"verified update");
        assert_eq!(unrelated, b"keep this file");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_update_install_preserves_backup_and_rejects_tampering() {
        use super::{VerifiedDownload, install};
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("pixelcraft-install-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("download.AppImage");
        let target = root.join("PixelCraft.AppImage");
        let bytes = b"new verified application";
        std::fs::write(&source, bytes).unwrap();
        std::fs::write(&target, b"old application").unwrap();
        let download = VerifiedDownload {
            path: source.clone(),
            digest: format!("{:x}", Sha256::digest(bytes)),
            size: bytes.len() as u64,
            name: "download.AppImage".into(),
        };
        assert!(install(&download, Some(&source)).is_err());
        install(&download, Some(&target)).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert_eq!(
            std::fs::read(target.with_extension("AppImage.previous")).unwrap(),
            b"old application"
        );
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        std::fs::write(&source, b"tampered").unwrap();
        assert!(install(&download, Some(&target)).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_update_install_replaces_backup_without_writing_through_links() {
        use super::{VerifiedDownload, install};
        use sha2::{Digest, Sha256};
        use std::{fs, os::unix::fs::symlink};

        let root =
            std::env::temp_dir().join(format!("pixelcraft-install-links-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("download.AppImage");
        let target = root.join("PixelCraft.AppImage");
        let other = root.join("other-file");
        let backup = target.with_extension("AppImage.previous");
        fs::write(&other, b"keep this file").unwrap();
        symlink(&other, target.with_extension("AppImage.installing")).unwrap();
        symlink(&other, &backup).unwrap();
        fs::write(&source, b"verified update").unwrap();
        fs::write(&target, b"old application").unwrap();
        let download = VerifiedDownload {
            path: source,
            digest: format!("{:x}", Sha256::digest(b"verified update")),
            size: b"verified update".len() as u64,
            name: "download.AppImage".into(),
        };

        install(&download, Some(&target)).unwrap();
        let installed = fs::read(&target).unwrap();
        let previous = fs::read(backup).unwrap();
        let unrelated = fs::read(other).unwrap();
        fs::remove_dir_all(root).unwrap();

        assert_eq!(installed, b"verified update");
        assert_eq!(previous, b"old application");
        assert_eq!(unrelated, b"keep this file");
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_installation_regressions {
    use super::*;
    #[test]
    fn windows_update_launches_verified_executable_and_rejects_tampering() {
        use sha2::{Digest, Sha256};
        let root = std::env::temp_dir().join(format!("pixelcraft-update-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let marker = root.join("launched.txt");
        let source = root.join("installer.rs");
        let exe = root.join("installer.exe");
        std::fs::write(
            &source,
            format!(
                "fn main() {{ std::fs::write({:?}, b\"installed\").unwrap(); }}",
                marker.to_str().unwrap()
            ),
        )
        .unwrap();
        assert!(
            std::process::Command::new("rustc")
                .arg(&source)
                .arg("-o")
                .arg(&exe)
                .status()
                .unwrap()
                .success()
        );
        let bytes = std::fs::read(&exe).unwrap();
        let held = open_installer(&exe).unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&exe).is_err());
        assert!(std::fs::rename(&exe, root.join("moved.exe")).is_err());
        drop(held);
        let download = VerifiedDownload {
            path: exe.clone(),
            digest: format!("{:x}", Sha256::digest(&bytes)),
            size: bytes.len() as u64,
            name: "installer.exe".into(),
        };
        install(&download, None).unwrap();
        let start = std::time::Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read(&marker).unwrap(), b"installed");
        // Wait until Windows releases the executable handle before replacing it.
        let mut tampered = bytes;
        tampered[0] ^= 1;
        while std::fs::write(&exe, &tampered).is_err() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(install(&download, None).is_err());
        let file = fs::OpenOptions::new().write(true).open(&exe).unwrap();
        file.set_len(download.size + 1).unwrap();
        drop(file);
        assert!(install(&download, None).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
