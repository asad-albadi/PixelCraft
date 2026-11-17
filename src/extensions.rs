//! Independent local extensions. Never discovers or launches another editor.
mod network;
mod process;
pub use network::script_network_request;
pub use process::script_process_request;

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
pub(crate) fn decode_script_document(json: &str) -> Result<Document, String> {
    let mut edited: Document =
        serde_json::from_str(json).map_err(|e| format!("Invalid script result: {e}"))?;
    edited.validate()?;
    edited.set_color_mode(edited.color_mode);
    Ok(edited)
}

pub fn run_source(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
) -> Result<Document, String> {
    run_source_with_colors(doc, source, frame, layer, command, None)
}

/// Pass the editor color preferences as a snapshot to the isolated Lua worker.
pub fn run_source_with_colors(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
    colors: Option<[[u8; 4]; 2]>,
) -> Result<Document, String> {
    let json = crate::lua::execute(doc, source, frame, layer, command, false, colors)?;
    decode_script_document(&json)
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
    #[serde(default, alias = "keys")]
    pub shortcuts: Vec<Contribution>,
    #[serde(default)]
    pub languages: Vec<Contribution>,
    #[serde(default, rename = "ditheringMatrices")]
    pub dithering_matrices: Vec<Contribution>,
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
pub struct PluginMenu {
    pub title: String,
    pub items: Vec<PluginMenuItem>,
}

#[derive(Clone, Debug)]
pub enum PluginMenuItem {
    Command(PluginCommand),
    Group { title: String, items: Vec<Self> },
    Separator,
}

impl PluginMenuItem {
    fn collect_commands(self, commands: &mut Vec<PluginCommand>) {
        match self {
            Self::Command(command) => commands.push(command),
            Self::Group { items, .. } => {
                for item in items {
                    item.collect_commands(commands);
                }
            }
            Self::Separator => {}
        }
    }
}

#[derive(Deserialize, Default)]
struct ScriptRegistration {
    registered: bool,
    entries: Vec<MenuRegistration>,
}

#[derive(Deserialize)]
struct MenuRegistration {
    group: String,
    #[serde(flatten)]
    entry: MenuEntry,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum MenuEntry {
    Command {
        id: String,
        title: String,
    },
    Group {
        id: String,
        title: String,
        #[serde(default)]
        hidden: bool,
    },
    Separator,
}

fn build_script_menu(
    entries: Vec<MenuRegistration>,
    package: &str,
    script: &Path,
) -> Vec<PluginMenuItem> {
    use std::collections::{HashMap, HashSet};
    let groups: HashSet<_> = entries
        .iter()
        .filter_map(|entry| match &entry.entry {
            MenuEntry::Group { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut children: HashMap<String, Vec<MenuEntry>> = HashMap::new();
    for MenuRegistration { group, entry } in entries {
        let parent = if groups.contains(&group) {
            group
        } else {
            String::new()
        };
        children.entry(parent).or_default().push(entry);
    }

    fn expand(
        children: &mut HashMap<String, Vec<MenuEntry>>,
        group: &str,
        package: &str,
        script: &Path,
        depth: usize,
    ) -> Vec<PluginMenuItem> {
        if depth == 24 {
            return Vec::new();
        }
        let mut menu = Vec::new();
        for entry in children.remove(group).unwrap_or_default() {
            match entry {
                MenuEntry::Group {
                    id,
                    title,
                    hidden: false,
                } => {
                    let items = expand(children, &id, package, script, depth + 1);
                    if !items.is_empty() {
                        menu.push(PluginMenuItem::Group { title, items });
                    }
                }
                MenuEntry::Command { id, title } => {
                    menu.push(PluginMenuItem::Command(PluginCommand {
                        id,
                        title,
                        package: package.to_owned(),
                        script: script.to_path_buf(),
                    }))
                }
                MenuEntry::Separator
                    if menu
                        .last()
                        .is_some_and(|item| !matches!(item, PluginMenuItem::Separator)) =>
                {
                    menu.push(PluginMenuItem::Separator);
                }
                _ => {}
            }
        }
        if matches!(menu.last(), Some(PluginMenuItem::Separator)) {
            menu.pop();
        }
        menu
    }

    expand(&mut children, "", package, script, 0)
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
        || p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
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
                .chain(&c.languages)
                .chain(&c.dithering_matrices)
            {
                let p = safe_path(&contribution.path)?;
                let path = package_dir.join(&p);
                if !(path.is_file() || path.is_dir() && path.join("theme.xml").is_file()) {
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
        let mut commands = Vec::new();
        for menu in self.menus()? {
            for item in menu.items {
                item.collect_commands(&mut commands);
            }
        }
        Ok(commands)
    }

    pub fn menus(&self) -> Result<Vec<PluginMenu>, String> {
        let mut menus = Vec::new();
        for p in self.list()?.into_iter().filter(|p| p.enabled) {
            let mut items = Vec::new();
            for s in &p.manifest.contributes.scripts {
                let script = p.directory.join(safe_path(&s.path)?);
                let source = std::fs::read_to_string(&script).map_err(|e| e.to_string())?;
                // Plain scripts can open dialogs immediately. A discovery-only
                // failure must not hide them from the explicit Run action.
                let registration = match crate::lua::execute(
                    &Document::new(1, 1),
                    &source,
                    0,
                    0,
                    None,
                    true,
                    None,
                ) {
                    Ok(json) => serde_json::from_str::<ScriptRegistration>(&json)
                        .map_err(|e| e.to_string())?,
                    Err(_) => ScriptRegistration::default(),
                };
                if registration.registered {
                    items.extend(build_script_menu(registration.entries, &p.id, &script));
                } else {
                    items.push(PluginMenuItem::Command(PluginCommand {
                        id: String::new(),
                        title: script
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        package: p.id.clone(),
                        script,
                    }));
                }
            }
            let title = if p.manifest.display_name.is_empty() {
                p.id
            } else {
                p.manifest.display_name
            };
            if !items.is_empty() {
                menus.push(PluginMenu { title, items });
            }
        }
        Ok(menus)
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
    visit_zip(b, |relative, contents| {
        let path = dest.join(relative);
        if let Some(bytes) = contents {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, bytes).map_err(|e| e.to_string())
        } else {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())
        }
    })
}

/// Visit validated ZIP entries, with `None` for directory entries.
pub(crate) fn visit_zip(
    b: &[u8],
    mut visit: impl FnMut(PathBuf, Option<Vec<u8>>) -> Result<(), String>,
) -> Result<(), String> {
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
        if name.ends_with('/') {
            visit(relative, None)?;
        } else {
            if b.get(local..local + 4) != Some(b"PK\x03\x04") {
                return Err("Invalid local ZIP header".into());
            }
            let offset = local + 30 + w(b, local + 26)? + w(b, local + 28)?;
            let encoded = b
                .get(offset..offset + compressed)
                .ok_or("Truncated ZIP data")?;
            let bytes = match method {
                0 if encoded.len() == size => encoded.to_vec(),
                0 => return Err("ZIP size mismatch".into()),
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
            visit(relative, Some(bytes))?;
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

/// Interactive hosts receive declarative widgets and validated document snapshots;
/// Lua stays on its worker thread while the editor owns all UI operations.
pub type ScriptHost = Box<dyn FnMut(serde_json::Value) -> Result<serde_json::Value, String> + Send>;

pub const MAX_TILESET_METADATA_BYTES: usize = 4 * 1024 * 1024;

#[derive(serde::Deserialize)]
pub struct ScriptWorkspace {
    pub active: Option<u32>,
    pub closed: Vec<u32>,
    pub sprites: Vec<ScriptSprite>,
    pub colors: [[u8; 4]; 2],
}

#[derive(serde::Deserialize)]
pub struct ScriptSprite {
    pub id: u32,
    pub document: Document,
    #[serde(default, rename = "layerKeys")]
    pub layer_keys: Vec<String>,
    #[serde(default, rename = "celKeys")]
    pub cel_keys: Vec<ScriptCelTarget>,
    #[serde(default, rename = "metadataKeys")]
    pub metadata_keys: ScriptMetadataKeys,
    pub baseline: Option<Document>,
    pub saved: Option<Document>,
    pub filename: PathBuf,
    pub frame: usize,
    pub layer: usize,
    pub selection: ScriptSelection,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ScriptCelTarget {
    pub key: u64,
    pub layer: usize,
    pub frame: usize,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ScriptMetadataKeys {
    pub tags: Vec<String>,
    pub slices: Vec<String>,
    pub tilesets: Vec<String>,
    pub tiles: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ScriptSelection {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<(i32, i32)>>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "Preserve the existing script host inputs"
)]
pub fn run_workspace_with_host(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
    colors: [[u8; 4]; 2],
    mut options: serde_json::Value,
    host: ScriptHost,
) -> Result<ScriptWorkspace, String> {
    options["editorWorkspace"] = serde_json::Value::Bool(true);
    let mut existing = std::collections::HashSet::from([1]);
    if let Some(tabs) = options["editorTabs"].as_array() {
        existing.extend(
            tabs.iter()
                .filter_map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok())),
        );
    }
    let json = crate::lua::execute_with_host(
        doc,
        source,
        frame,
        layer,
        command,
        false,
        Some(colors),
        options,
        Some(host),
    )?;
    let workspace: ScriptWorkspace = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let mut ids = std::collections::HashSet::new();
    let mut pixels = 0usize;
    for sprite in &workspace.sprites {
        if sprite.id == 0
            || !ids.insert(sprite.id)
            || (!existing.contains(&sprite.id) && sprite.baseline.is_none())
        {
            return Err("Invalid sprite identity in extension result".into());
        }
        sprite.document.validate()?;
        pixels = pixels.saturating_add(sprite.document.logical_pixels());
        for snapshot in sprite.baseline.iter().chain(sprite.saved.iter()) {
            snapshot.validate()?;
        }
    }
    let closed: std::collections::HashSet<_> = workspace.closed.iter().copied().collect();
    if pixels > crate::lua::MAX_SCRIPT_PIXELS
        || closed.len() != workspace.closed.len()
        || !closed.is_subset(&existing)
        || workspace
            .active
            .is_some_and(|id| !ids.contains(&id) || closed.contains(&id))
        || (workspace.active.is_none() && !ids.is_subset(&closed))
    {
        return Err("Invalid extension workspace result".into());
    }
    Ok(workspace)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Preserve the established public API"
)]
pub fn run_source_with_host(
    doc: &Document,
    source: &str,
    frame: usize,
    layer: usize,
    command: Option<&str>,
    colors: Option<[[u8; 4]; 2]>,
    options: serde_json::Value,
    host: ScriptHost,
) -> Result<Document, String> {
    let json = crate::lua::execute_with_host(
        doc,
        source,
        frame,
        layer,
        command,
        false,
        colors,
        options,
        Some(host),
    )?;
    decode_script_document(&json)
}

/// Keep editor and file-backed script documents under the same pixel limit.
pub fn script_document_value(document: &Document) -> Result<serde_json::Value, String> {
    if document.logical_pixels() > crate::lua::MAX_SCRIPT_PIXELS {
        return Err("Native scripts are limited to one million pixels per invocation".into());
    }
    serde_json::to_value(document).map_err(|e| e.to_string())
}

/// Filesystem bridge used only by explicitly executed scripts. Relative filenames
/// follow the script folder; it never searches Aseprite's installation or settings.
pub fn script_file_request(
    base: &Path,
    request: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    script_file_request_with_authorization(base, request, |_, _| Ok(()))
}

/// Dispatch a file operation after authorizing each input or output path.
/// Image references in imported documents go through the same gate.
pub fn script_file_request_with_authorization(
    base: &Path,
    request: &serde_json::Value,
    authorize: impl Fn(&str, &Path) -> Result<(), String>,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let op = request["op"].as_str().unwrap_or("");
    let purpose = match op {
        "open" => "Open artwork",
        "readScript" => "Read and run a Lua module",
        "save" => "Write artwork",
        "isFile" | "isDirectory" => "Check whether a path exists",
        "listFiles" => "List directory contents",
        "makeDirectory" => "Create a directory",
        "close" => return Ok(json!({})),
        _ => return Err("Unknown file operation".into()),
    };
    let input = request["path"]
        .as_str()
        .ok_or("Expected a filesystem path")?;
    let path = script_absolute_path(&base.join(input))?;
    authorize(purpose, &path)?;
    match op {
        "open" => {
            require_script_file(&path, false)?;
            let approved = std::cell::RefCell::new(std::collections::HashSet::from([path.clone()]));
            let document = formats::load_with_path_check(&path, &|requested| {
                let requested = script_absolute_path(requested)?;
                if !approved.borrow().contains(&requested) {
                    authorize("Read a resource referenced by the artwork", &requested)?;
                    require_script_file(&requested, false)?;
                    approved.borrow_mut().insert(requested);
                }
                Ok(())
            })?;
            Ok(json!({"document":script_document_value(&document)?,"path":path}))
        }
        "readScript" => {
            require_script_file(&path, false)?;
            const MAX_MODULE: usize = 2 * 1024 * 1024;
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .and_then(|file| file.take((MAX_MODULE + 1) as u64).read_to_end(&mut bytes))
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_MODULE {
                return Err("Lua module exceeds 2 MiB".into());
            }
            Ok(json!({"source":String::from_utf8(bytes).map_err(|e|e.to_string())?}))
        }
        "save" => {
            require_script_file(&path, true)?;
            let d: Document =
                serde_json::from_value(request["document"].clone()).map_err(|e| e.to_string())?;
            d.validate()?;
            match path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "png" => formats::export_png(&path, &d, 0, false)?,
                "gif" => formats::export_gif(&path, &d)?,
                _ => formats::save(&path, &d)?,
            };
            Ok(json!({"path":path}))
        }
        "isFile" => Ok(json!({"value":path.is_file()})),
        "isDirectory" => Ok(json!({"value":path.is_dir()})),
        "listFiles" => Ok(
            json!({"value":std::fs::read_dir(path).map_err(|e|e.to_string())?.filter_map(Result::ok).map(|e|e.file_name().to_string_lossy().to_string()).collect::<Vec<_>>()}),
        ),
        "makeDirectory" => {
            std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
            Ok(json!({"value":true}))
        }
        _ => unreachable!("File operation validated before authorization"),
    }
}

fn require_script_file(path: &Path, allow_missing: bool) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err("Lua artwork and module access requires a regular file".into()),
        Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn script_absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().to_string_lossy().contains('\0') {
        return Err("Invalid filesystem path".into());
    }
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    let mut path = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir if path.parent().is_some() => {
                path.pop();
            }
            Component::ParentDir | Component::CurDir => {}
            #[cfg(windows)]
            Component::Prefix(prefix)
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) =>
            {
                return Err("Lua file access requires a local drive path".into());
            }
            #[cfg(windows)]
            Component::Normal(name) if !ordinary_windows_file_component(name) => {
                return Err("Lua file access requires an ordinary file or directory path".into());
            }
            component => path.push(component.as_os_str()),
        }
    }
    Ok(path)
}

#[cfg(windows)]
fn ordinary_windows_file_component(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    if name.contains(':') || name.ends_with([' ', '.']) {
        return false;
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|number| {
            matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}
