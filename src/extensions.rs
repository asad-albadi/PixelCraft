//! Independent local extensions. Never discovers or launches another editor.
use crate::{formats, model::Document};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};
pub fn data_directory() -> PathBuf {
    if let Some(p) = std::env::var_os("PIXELCRAFT_HOME") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".local/share")
        });
    base.join("pixelcraft")
}
pub fn config_directory() -> PathBuf {
    if let Some(p) = std::env::var_os("PIXELCRAFT_HOME") {
        return PathBuf::from(p).join("preferences");
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".config")
        })
        .join("pixelcraft")
}
pub fn projects_directory() -> Result<PathBuf, String> {
    let p = data_directory().join("projects");
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}
pub fn run_source(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
) -> Result<Document, String> {
    let json = crate::lua::execute(doc, source, frame, layer, command, false)?;
    let mut edited: Document =
        serde_json::from_str(&json).map_err(|e| format!("Invalid script result: {e}"))?;
    edited.validate()?;
    edited.set_color_mode(edited.color_mode);
    Ok(edited)
}
pub fn run_lua(
    doc: &Document,
    path: &Path,
    frame: usize,
    layer: usize,
    command: Option<&str>,
) -> Result<Document, String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    run_source(doc, &source, frame, layer, command)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contribution {
    pub path: String,
    #[serde(default)]
    pub id: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contributions {
    #[serde(default)]
    pub scripts: Vec<Contribution>,
    #[serde(default)]
    pub themes: Vec<Contribution>,
    #[serde(default)]
    pub palettes: Vec<Contribution>,
    #[serde(default)]
    pub shortcuts: Vec<Contribution>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub contributes: Contributions,
}
#[derive(Clone, Debug)]
pub struct Installed {
    pub id: String,
    pub directory: PathBuf,
    pub manifest: Manifest,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub script: PathBuf,
}
#[derive(Clone, Debug)]
pub struct Manager {
    pub root: PathBuf,
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && id != "."
        && id != ".."
}
fn safe_path(p: &str) -> Result<PathBuf, String> {
    let p = Path::new(p);
    if p.as_os_str().is_empty()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || p.to_string_lossy().contains('\\')
    {
        return Err("Extension path must stay inside its package".into());
    }
    Ok(p.to_path_buf())
}
impl Manager {
    pub fn new() -> Result<Self, String> {
        Self::at(data_directory())
    }
    pub fn at(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(root.join("extensions")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(root.join("projects")).map_err(|e| e.to_string())?;
        Ok(Self { root })
    }
    fn extension_dir(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Invalid extension ID".into());
        }
        Ok(self.root.join("extensions").join(id))
    }
    pub fn list(&self) -> Result<Vec<Installed>, String> {
        let mut list = vec![];
        for e in std::fs::read_dir(self.root.join("extensions")).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            if e.file_name().to_string_lossy().starts_with('.')
                || !e.file_type().map_err(|e| e.to_string())?.is_dir()
            {
                continue;
            }
            let directory = e.path();
            let m = std::fs::read(directory.join("package.json")).map_err(|e| e.to_string())?;
            let manifest: Manifest = serde_json::from_slice(&m).map_err(|e| e.to_string())?;
            let id = e.file_name().to_string_lossy().into_owned();
            list.push(Installed {
                id,
                directory: directory.clone(),
                manifest,
                enabled: !directory.join(".disabled").exists(),
            });
        }
        list.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(list)
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let dir = self.extension_dir(id)?;
        if !dir.join("package.json").is_file() {
            return Err("Extension not installed".into());
        }
        let p = dir.join(".disabled");
        if enabled {
            if p.exists() {
                std::fs::remove_file(p).map_err(|e| e.to_string())?
            }
        } else {
            std::fs::write(p, []).map_err(|e| e.to_string())?
        }
        Ok(())
    }
    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        std::fs::remove_dir_all(self.extension_dir(id)?).map_err(|e| e.to_string())
    }
    pub fn install(&self, source: &Path) -> Result<String, String> {
        let staging = self.root.join("extensions").join(format!(
            ".install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let result = (|| {
            if source.is_dir() {
                copy_directory(source, &staging, 0, &mut 0usize)?;
            } else if source.extension().and_then(|x| x.to_str()) == Some("lua") {
                let name = source
                    .file_stem()
                    .and_then(|x| x.to_str())
                    .ok_or("Invalid script filename")?;
                if !valid_id(name) {
                    return Err("Rename the script with letters, numbers or hyphens".into());
                }
                std::fs::copy(source, staging.join("main.lua")).map_err(|e| e.to_string())?;
                std::fs::write(
                    staging.join("package.json"),
                    serde_json::to_vec(&Manifest {
                        name: name.into(),
                        display_name: name.into(),
                        version: "1".into(),
                        contributes: Contributions {
                            scripts: vec![Contribution {
                                path: "main.lua".into(),
                                id: String::new(),
                            }],
                            ..Default::default()
                        },
                    })
                    .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            } else {
                unzip(&std::fs::read(source).map_err(|e| e.to_string())?, &staging)?;
            }
            let manifest_path = if staging.join("package.json").is_file() {
                staging.join("package.json")
            } else {
                let files: Vec<_> = std::fs::read_dir(&staging)
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .map(|e| e.path().join("package.json"))
                    .filter(|p| p.is_file())
                    .collect();
                if files.len() != 1 {
                    return Err("Package must contain one package.json".into());
                }
                files[0].clone()
            };
            let package_dir = manifest_path.parent().unwrap();
            let manifest: Manifest =
                serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if !valid_id(&manifest.name) {
                return Err("Invalid package name".into());
            }
            let dest = self.extension_dir(&manifest.name)?;
            if dest.exists() {
                return Err(
                    "This extension is already installed. Uninstall it before replacing it.".into(),
                );
            }
            let c = &manifest.contributes;
            let mut count = 0;
            for contribution in c
                .scripts
                .iter()
                .chain(&c.themes)
                .chain(&c.palettes)
                .chain(&c.shortcuts)
            {
                let p = safe_path(&contribution.path)?;
                if !package_dir.join(p).is_file() {
                    return Err(format!("Missing extension asset: {}", contribution.path));
                }
                count += 1;
            }
            if count == 0 {
                return Err(
                    "Package has no supported scripts, themes, palettes or shortcuts".into(),
                );
            }
            std::fs::rename(package_dir, &dest).map_err(|e| e.to_string())?;
            Ok(manifest.name)
        })();
        let _ = std::fs::remove_dir_all(staging);
        result
    }
    pub fn commands(&self) -> Result<Vec<PluginCommand>, String> {
        let mut out = vec![];
        for p in self.list()?.into_iter().filter(|p| p.enabled) {
            for s in &p.manifest.contributes.scripts {
                let script = p.directory.join(safe_path(&s.path)?);
                let source = std::fs::read_to_string(&script).map_err(|e| e.to_string())?;
                let json = crate::lua::execute(&Document::new(1, 1), &source, 0, 0, None, true)
                    .map_err(|e| format!("{}: {e}", p.id))?;
                let mut commands: Vec<PluginCommand> =
                    serde_json::from_str(&json).map_err(|e| e.to_string())?;
                if commands.is_empty() {
                    commands.push(PluginCommand {
                        id: String::new(),
                        title: script
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        package: String::new(),
                        script: PathBuf::new(),
                    });
                }
                for command in &mut commands {
                    command.package = p.id.clone();
                    command.script = script.clone();
                }
                out.extend(commands);
            }
        }
        Ok(out)
    }
    pub fn asset(&self, id: &str, asset: &Contribution) -> Result<PathBuf, String> {
        let dir = self.extension_dir(id)?;
        if dir.join(".disabled").exists() {
            return Err("Extension is disabled".into());
        }
        Ok(dir.join(safe_path(&asset.path)?))
    }
}
fn copy_directory(
    source: &Path,
    dest: &Path,
    depth: usize,
    total: &mut usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("Extension folder is nested too deeply".into());
    }
    if dest
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(source.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("Install a package from outside the extension data directory".into());
    }
    for e in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let ty = e.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            return Err("Extension symlinks are not supported".into());
        }
        let p = dest.join(e.file_name());
        if ty.is_dir() {
            std::fs::create_dir(&p).map_err(|e| e.to_string())?;
            copy_directory(&e.path(), &p, depth + 1, total)?;
        } else if ty.is_file() {
            *total = total.saturating_add(e.metadata().map_err(|e| e.to_string())?.len() as usize);
            if *total > 32 * 1024 * 1024 {
                return Err("Extension exceeds 32 MiB".into());
            }
            std::fs::copy(e.path(), p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn unzip(b: &[u8], dest: &Path) -> Result<(), String> {
    fn w(b: &[u8], p: usize) -> Result<usize, String> {
        Ok(
            u16::from_le_bytes(b.get(p..p + 2).ok_or("Truncated ZIP")?.try_into().unwrap())
                as usize,
        )
    }
    fn d(b: &[u8], p: usize) -> Result<usize, String> {
        Ok(
            u32::from_le_bytes(b.get(p..p + 4).ok_or("Truncated ZIP")?.try_into().unwrap())
                as usize,
        )
    }
    let end = (b.len().saturating_sub(65557)..b.len().saturating_sub(21))
        .rev()
        .find(|&i| b.get(i..i + 4) == Some(b"PK\x05\x06"))
        .ok_or("Invalid ZIP directory")?;
    let count = w(b, end + 10)?;
    let mut p = d(b, end + 16)?;
    let mut total = 0usize;
    for _ in 0..count {
        if b.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err("Invalid ZIP entry".into());
        }
        let flags = w(b, p + 8)?;
        let method = w(b, p + 10)?;
        let crc = d(b, p + 16)? as u32;
        let compressed = d(b, p + 20)?;
        let size = d(b, p + 24)?;
        let name_len = w(b, p + 28)?;
        let extra = w(b, p + 30)?;
        let comment = w(b, p + 32)?;
        let local = d(b, p + 42)?;
        let name = std::str::from_utf8(
            b.get(p + 46..p + 46 + name_len)
                .ok_or("Truncated ZIP name")?,
        )
        .map_err(|e| e.to_string())?;
        let relative = safe_path(name.trim_end_matches('/'))?;
        if flags & 1 != 0 {
            return Err("Encrypted extensions are unsupported".into());
        }
        if (d(b, p + 38)? >> 16) & 0xf000 == 0xa000 {
            return Err("ZIP symlinks are unsupported".into());
        }
        total = total.saturating_add(size);
        if total > 32 * 1024 * 1024 {
            return Err("Extension exceeds 32 MiB".into());
        }
        let path = dest.join(relative);
        if name.ends_with('/') {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        } else {
            if b.get(local..local + 4) != Some(b"PK\x03\x04") {
                return Err("Invalid local ZIP header".into());
            }
            let offset = local + 30 + w(b, local + 26)? + w(b, local + 28)?;
            let encoded = b
                .get(offset..offset + compressed)
                .ok_or("Truncated ZIP data")?;
            let bytes = match method {
                0 => encoded.to_vec(),
                8 => {
                    let mut out = vec![];
                    flate2::read::DeflateDecoder::new(encoded)
                        .take(size as u64 + 1)
                        .read_to_end(&mut out)
                        .map_err(|e| e.to_string())?;
                    out
                }
                _ => return Err("Unsupported ZIP compression".into()),
            };
            if bytes.len() != size {
                return Err("ZIP size mismatch".into());
            }
            let mut calculated = 0xffffffffu32;
            for byte in &bytes {
                calculated ^= *byte as u32;
                for _ in 0..8 {
                    calculated = (calculated >> 1) ^ (0xedb88320u32.wrapping_mul(calculated & 1));
                }
            }
            if !calculated != crc {
                return Err("ZIP checksum mismatch".into());
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?
            }
            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
        p += 46 + name_len + extra + comment;
    }
    Ok(())
}
// Temporary internal sessions keep asynchronous edits isolated and undoable.
#[derive(Clone, Debug)]
pub struct Session {
    pub directory: PathBuf,
}
impl Session {
    pub fn new(doc: &Document) -> Result<Self, String> {
        // Clock timestamps can repeat for concurrent calls on Windows.
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "pixelcraft-session-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let s = Self { directory };
        formats::save(&s.input_path(), doc)?;
        Ok(s)
    }
    pub fn input_path(&self) -> PathBuf {
        self.directory.join("input.pixel")
    }
    pub fn result_path(&self) -> PathBuf {
        self.directory.join("result.pixel")
    }
    pub fn load_result(&self) -> Result<Document, String> {
        formats::load(&self.result_path())
    }
}
