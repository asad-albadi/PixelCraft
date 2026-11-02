//! Readers for original contribution formats. No original assets are bundled.
use crate::model::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
pub fn theme(bytes: &[u8]) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_slice(bytes) {
        return Ok(value);
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let xml = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    if xml.root_element().tag_name().name() != "theme" {
        return Err("Expected theme XML".into());
    }
    let mut colors = BTreeMap::new();
    for c in xml.descendants().filter(|n| n.has_tag_name("color")) {
        if let (Some(id), Some(value)) = (c.attribute("id"), c.attribute("value")) {
            let hex = value.trim_start_matches('#');
            if hex.len() < 6 {
                continue;
            }
            if !hex.is_ascii() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("Invalid theme color: {value}"));
            }
            let rgb = (0..3)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            colors.insert(id, rgb);
        }
    }
    let face = colors
        .get("face")
        .or(colors.get("editor_face"))
        .cloned()
        .unwrap_or(vec![32, 36, 44]);
    let text = colors.get("text").cloned().unwrap_or(vec![230, 230, 230]);
    let accent = colors
        .get("selected")
        .or(colors.get("button_selected_face"))
        .cloned()
        .unwrap_or(vec![86, 201, 184]);
    Ok(
        json!({"dark":face.iter().map(|n|*n as u32).sum::<u32>()<384,"panel":face,"text":text,"accent":accent,"colors":colors}),
    )
}
pub fn shortcut_action(command: &str) -> Option<&'static str> {
    Some(match command {
        "NewFile" => "new",
        "OpenFile" => "open",
        "SaveFile" => "save",
        "SaveFileAs" => "saveas",
        "CloseFile" => "closetab",
        "Undo" => "undo",
        "Redo" => "redo",
        "Copy" => "copy",
        "Cut" => "cut",
        "Paste" => "paste",
        "Clear" => "clear",
        "SelectAll" => "selectall",
        "DeselectMask" => "deselect",
        "NewLayer" => "layer",
        "RemoveLayer" => "deletelayer",
        "NewFrame" => "frame",
        "NewEmptyFrame" => "blank",
        "RemoveFrame" => "deleteframe",
        "PlayAnimation" => "play",
        "GotoNextFrame" => "next",
        "GotoPreviousFrame" => "prev",
        "ShowOnionSkin" => "onion",
        "ShowGrid" => "grid",
        "SpriteProperties" => "properties",
        "pencil" => "pencil",
        "eraser" => "eraser",
        "paint_bucket" => "fill",
        "eyedropper" => "picker",
        "line" => "line",
        "rectangle" => "rect",
        "ellipse" => "ellipse",
        "rectangular_marquee" => "select",
        "hand" => "hand",
        _ => return None,
    })
}
#[derive(Debug, Default)]
pub struct Shortcuts {
    pub bindings: BTreeMap<String, String>,
    pub unsupported: Vec<String>,
}
pub fn shortcuts(bytes: &[u8]) -> Result<Shortcuts, String> {
    if let Ok(bindings) = serde_json::from_slice(bytes) {
        return Ok(Shortcuts {
            bindings,
            unsupported: vec![],
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let xml = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let mut result = Shortcuts::default();
    for key in xml.descendants().filter(|n| n.has_tag_name("key")) {
        let Some(binding) = key.attribute("shortcut") else {
            continue;
        };
        if binding.is_empty() {
            continue;
        }
        let command = key
            .attribute("command")
            .or(key.attribute("tool"))
            .unwrap_or("");
        // Parameters can materially change commands: retain a clear unsupported report.
        if key.children().any(|n| n.has_tag_name("param")) {
            result
                .unsupported
                .push(format!("{command} ({binding}, parameters)"));
            continue;
        }
        if let Some(action) = shortcut_action(command) {
            result.bindings.insert(
                binding
                    .replace("Space", "Space")
                    .replace("Esc", "Escape")
                    .replace("Del", "Delete"),
                action.into(),
            );
        } else {
            result.unsupported.push(format!("{command} ({binding})"));
        }
    }
    if result.bindings.is_empty() {
        return Err(format!(
            "No supported shortcuts; {} entries need unavailable commands",
            result.unsupported.len()
        ));
    }
    Ok(result)
}
pub fn language(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())?
        .trim_start_matches('\u{feff}');
    let mut section = "";
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            out.insert(
                format!("{section}.{}", key.trim()),
                value
                    .trim()
                    .replace("\\n", "\n")
                    .replace("&&", "\u{0}")
                    .replace('&', "")
                    .replace('\u{0}', "&"),
            );
        }
    }
    if out.is_empty() {
        return Err("Language contains no INI entries".into());
    }
    Ok(out)
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DitheringMatrix {
    pub width: usize,
    pub height: usize,
    pub values: Vec<u8>,
}
impl DitheringMatrix {
    pub fn load(path: &Path) -> Result<Self, String> {
        let im = image::open(path).map_err(|e| e.to_string())?.to_luma8();
        let (w, h) = im.dimensions();
        if w == 0 || h == 0 || w > 256 || h > 256 {
            return Err("Dithering matrix must be 1–256 pixels per side".into());
        }
        Ok(Self {
            width: w as usize,
            height: h as usize,
            values: im.into_raw(),
        })
    }
    pub fn validate(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 256
            && self.height <= 256
            && self.values.len() == self.width * self.height
    }
    pub fn index(&self, p: Pixel, palette: &[Pixel], x: usize, y: usize) -> usize {
        let first = nearest(p, palette);
        if p[3] == 0 || palette.len() < 2 {
            return first;
        }
        let a = palette[first];
        let mut second = first;
        let mut error = f64::INFINITY;
        let mut mix = 0.;
        for (i, b) in palette.iter().enumerate() {
            if i == first || b[3] == 0 {
                continue;
            }
            let den = (0..3)
                .map(|c| (b[c] as f64 - a[c] as f64).powi(2))
                .sum::<f64>();
            if den == 0. {
                continue;
            }
            let t = ((0..3)
                .map(|c| (p[c] as f64 - a[c] as f64) * (b[c] as f64 - a[c] as f64))
                .sum::<f64>()
                / den)
                .clamp(0., 1.);
            let e = (0..3)
                .map(|c| (p[c] as f64 - (a[c] as f64 + t * (b[c] as f64 - a[c] as f64))).powi(2))
                .sum::<f64>();
            if e < error {
                error = e;
                second = i;
                mix = t;
            }
        }
        // Matrix images encode ordered ranks, which need normalization for 2x2/4x4 BMPs.
        let max = self.values.iter().copied().max().unwrap_or(255) as f64 + 1.;
        let threshold =
            (self.values[(y % self.height) * self.width + x % self.width] as f64 + 0.5) / max;
        if mix > threshold { second } else { first }
    }
}
