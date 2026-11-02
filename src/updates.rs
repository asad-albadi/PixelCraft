//! GitHub release lookup; all network work runs off the editor thread.
use semver::Version;
use std::{sync::mpsc, time::Duration};

pub const INSTALLED_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct Release {
    pub tag: String,
    version: Version,
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AboutInfo {
    pub developer: String,
    pub repository: String,
}
impl Default for AboutInfo {
    fn default() -> Self {
        Self {
            developer: option_env!("PIXELCRAFT_DEVELOPER")
                .unwrap_or("PixelCraft contributors")
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
    pub fn save(&self) -> Result<(), String> {
        let directory = crate::extensions::config_directory();
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        std::fs::write(
            directory.join("about.json"),
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn github_url(&self) -> Option<String> {
        repository_slug(&self.repository).map(|slug| format!("https://github.com/{slug}"))
    }
    pub fn releases_url(&self) -> Option<String> {
        self.github_url()
            .map(|url| format!("{url}/releases/latest"))
    }
}
