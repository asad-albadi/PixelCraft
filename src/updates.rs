//! GitHub release lookup; all network work runs off the editor thread.
use semver::Version;
use std::{sync::mpsc, time::Duration};

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
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let path = directory.join(name);
            let temp = directory.join(format!("{name}.partial"));
            std::fs::write(&temp, &bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&temp, &path).map_err(|e| e.to_string())?;
            Ok(VerifiedDownload {
                path,
                digest,
                name: name.clone(),
            })
        })();
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}
/// Call only after the person confirms installation. Recheck disk bytes before use.
pub fn install(
    download: &VerifiedDownload,
    target: Option<&std::path::Path>,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&download.path).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes)) != download.digest {
        return Err("Downloaded installer changed; download it again".into());
    }
    #[cfg(target_os = "windows")]
    {
        let _ = target;
        std::process::Command::new(&download.path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let target = target.ok_or("Choose an AppImage installation path")?;
        if target == download.path {
            return Err("Choose an installation path outside the download cache".into());
        }
        let temp = target.with_extension("AppImage.installing");
        std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        if target.exists() {
            std::fs::copy(target, target.with_extension("AppImage.previous"))
                .map_err(|e| e.to_string())?;
        }
        std::fs::rename(temp, target).map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = target;
        Err("In-app installation is unavailable on this platform".into())
    }
}

#[cfg(test)]
mod installation_regressions {
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
        let download = VerifiedDownload {
            path: exe.clone(),
            digest: format!("{:x}", Sha256::digest(&bytes)),
            name: "installer.exe".into(),
        };
        install(&download, None).unwrap();
        let start = std::time::Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read(&marker).unwrap(), b"installed");
        // Wait until Windows releases the executable handle before replacing it.
        while std::fs::write(&exe, b"tampered").is_err() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(install(&download, None).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
