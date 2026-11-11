//! Shared drawing and interchange operations for the editor and headless tools.
use crate::model::*;
use crate::{formats, interchange, workflows::SheetOptions};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum SelectionOp {
    #[default]
    Replace,
    Add,
    Subtract,
}
pub fn combine_selection(current: &mut Vec<bool>, hits: &[bool], op: SelectionOp) {
    current.resize(hits.len(), false);
    for (a, b) in current.iter_mut().zip(hits) {
        *a = match op {
            SelectionOp::Replace => *b,
            SelectionOp::Add => *a || *b,
            SelectionOp::Subtract => *a && !*b,
        };
    }
}
pub fn matching(
    p: &[Pixel],
    w: usize,
    h: usize,
    seed: (i32, i32),
    tolerance: u8,
    contiguous: bool,
    wrap: bool,
) -> Vec<bool> {
    let mut out = vec![false; p.len()];
    if w == 0
        || h == 0
        || p.len() != w.saturating_mul(h)
        || seed.0 < 0
        || seed.1 < 0
        || seed.0 as usize >= w
        || seed.1 as usize >= h
    {
        return out;
    }
    let first = seed.1 as usize * w + seed.0 as usize;
    let target = p[first];
    let matches = |c: Pixel| {
        (c[3] == 0 && target[3] == 0) || (0..4).all(|i| c[i].abs_diff(target[i]) <= tolerance)
    };
    if !contiguous {
        for (v, c) in out.iter_mut().zip(p) {
            *v = matches(*c);
        }
        return out;
    }
    let mut stack = vec![first];
    out[first] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        for (mut x, mut y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if wrap {
                x = x.rem_euclid(w as i32);
                y = y.rem_euclid(h as i32);
            }
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                continue;
            }
            let n = y as usize * w + x as usize;
            if !out[n] && matches(p[n]) {
                out[n] = true;
                stack.push(n);
            }
        }
    }
    out
}
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BrushMode {
    #[default]
    Normal,
    Shade,
    Dither,
    Selection,
}
#[derive(Clone, Default)]
pub struct BrushOptions {
    pub mode: BrushMode,
    pub alternate: Pixel,
    pub reverse: bool,
    pub stamp: Option<(usize, usize, Vec<Pixel>)>,
    pub mask: Option<Vec<bool>>,
}
pub fn paint_brush(
    d: &mut Document,
    l: usize,
    f: usize,
    p: (i32, i32),
    color: Pixel,
    size: i32,
    o: &BrushOptions,
) {
    if !d.editable(l) || d.layers[l].tileset.is_some() {
        return;
    }
    let mut points = Vec::new();
    if o.mode == BrushMode::Selection {
        if let Some((w, h, pixels)) = &o.stamp {
            for y in 0..*h {
                for x in 0..*w {
                    points.push((
                        p.0 + x as i32 - (*w / 2) as i32,
                        p.1 + y as i32 - (*h / 2) as i32,
                        pixels[y * w + x],
                    ));
                }
            }
        }
    } else {
        let size = size.clamp(1, MAX_BRUSH_SIZE);
        let lo = -(size / 2);
        for y in lo..lo + size {
            for x in lo..lo + size {
                let (a, b) = (
                    2 * x + if size % 2 == 0 { 1 } else { 0 },
                    2 * y + if size % 2 == 0 { 1 } else { 0 },
                );
                let hit = match d.brush_shape {
                    BrushShape::Round => a * a + b * b <= size * size,
                    BrushShape::Square => true,
                    BrushShape::Diamond => a.abs() + b.abs() <= size,
                    BrushShape::Horizontal => y == 0,
                    BrushShape::Vertical => x == 0,
                };
                if hit {
                    points.push((p.0 + x, p.1 + y, color));
                }
            }
        }
    }
    for (mut x, mut y, mut c) in points {
        if d.wraparound {
            x = x.rem_euclid(d.width as i32);
            y = y.rem_euclid(d.height as i32);
        }
        let Some(i) = d.index(x, y) else {
            continue;
        };
        if o.mask.as_ref().is_some_and(|m| m.get(i) != Some(&true)) {
            continue;
        }
        match o.mode {
            BrushMode::Shade => {
                let old = d.layers[l].cels[f][i];
                if old[3] == 0 {
                    continue;
                }
                let palette = if d.color_mode == ColorMode::Indexed {
                    d.editing_palette(l, f)
                } else {
                    d.palette_for(f).to_vec()
                };
                let n = if d.color_mode == ColorMode::Indexed {
                    d.layers[l].cel_meta[f]
                        .indices
                        .as_ref()
                        .and_then(|indices| indices.get(i))
                        .map(|&n| n as usize)
                        .filter(|&n| n < palette.len())
                        .unwrap_or_else(|| nearest(old, &palette))
                } else {
                    nearest(old, &palette)
                };
                let n = if o.reverse {
                    n.saturating_sub(1)
                } else {
                    (n + 1).min(palette.len() - 1)
                };
                c = palette[n];
                if d.color_mode == ColorMode::Indexed {
                    // Keep the destination index even when its color is duplicated.
                    let drawing_index = d.drawing_index.replace(n);
                    d.set_pixel(l, f, i, c);
                    d.drawing_index = drawing_index;
                    continue;
                }
            }
            BrushMode::Dither => {
                if (x + y).rem_euclid(2) == 1 {
                    c = o.alternate;
                }
            }
            _ => {}
        }
        if o.mode != BrushMode::Selection {
            c = d.snapped_color(f, c);
        }
        d.set_pixel(l, f, i, c);
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImage {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Pixels,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: u8,
    pub visible: bool,
}
pub fn read_image(path: &Path) -> Result<RgbaImage, String> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION as u32);
    limits.max_image_height = Some(MAX_DIMENSION as u32);
    limits.max_alloc = Some((MAX_PIXELS * 4) as u64);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| e.to_string())
}
pub fn import_reference(path: &Path) -> Result<ReferenceImage, String> {
    let im = read_image(path)?;
    Ok(ReferenceImage {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        width: im.width() as usize,
        height: im.height() as usize,
        pixels: im.pixels().map(|p| p.0).collect(),
        x: 0.,
        y: 0.,
        scale: 1.,
        opacity: 128,
        visible: true,
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OnionOptions {
    pub previous: usize,
    pub next: usize,
    pub previous_tint: [u8; 3],
    pub next_tint: [u8; 3],
    pub previous_opacity: u8,
    pub next_opacity: u8,
}
impl Default for OnionOptions {
    fn default() -> Self {
        Self {
            previous: 1,
            next: 0,
            previous_tint: [150, 185, 220],
            next_tint: [240, 150, 150],
            previous_opacity: 64,
            next_opacity: 64,
        }
    }
}
pub fn onion_pixels(d: &Document, f: usize, o: &OnionOptions) -> Vec<Pixel> {
    let mut out = vec![CLEAR; d.width * d.height];
    for distance in (1..=8).rev() {
        for (frame, count, tint, opacity) in [
            (
                f.checked_sub(distance),
                o.previous,
                o.previous_tint,
                o.previous_opacity,
            ),
            (
                f.checked_add(distance).filter(|n| *n < d.durations.len()),
                o.next,
                o.next_tint,
                o.next_opacity,
            ),
        ] {
            if distance > count.min(8) {
                continue;
            }
            if let Some(frame) = frame {
                for (p, s) in out.iter_mut().zip(d.composite(frame)) {
                    let alpha = (s[3] as u32 * opacity as u32 / 255 / distance as u32) as u8;
                    *p = over(*p, [tint[0], tint[1], tint[2], alpha], 255);
                }
            }
        }
    }
    for (p, c) in out.iter_mut().zip(d.composite(f)) {
        *p = over(*p, c, 255);
    }
    out
}

pub fn parse_palette(bytes: &[u8], ext: &str) -> Result<Vec<Pixel>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut p = vec![];
    match ext.to_ascii_lowercase().as_str() {
        "json" => {
            let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            p = serde_json::from_value(v.get("colors").unwrap_or(&v).clone())
                .map_err(|e| e.to_string())?;
        }
        "hex" => {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
                    continue;
                }
                let s = line.trim_start_matches('#');
                if ![6, 8].contains(&s.len()) || !s.is_ascii() {
                    return Err("Expected RRGGBB or RRGGBBAA".into());
                }
                let mut c = [0, 0, 0, 255];
                for i in 0..s.len() / 2 {
                    c[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
                        .map_err(|_| "Invalid hex color")?;
                }
                p.push(c);
            }
        }
        "pal" => {
            let mut lines = text.lines();
            if lines.next().map(str::trim) != Some("JASC-PAL")
                || lines.next().map(str::trim) != Some("0100")
            {
                return Err("Expected JASC-PAL 0100".into());
            }
            let n: usize = lines
                .next()
                .ok_or("Missing palette count")?
                .trim()
                .parse()
                .map_err(|_| "Invalid palette count")?;
            for line in lines.filter(|s| !s.trim().is_empty()) {
                p.push(parse_rgb(line)?);
            }
            if p.len() != n {
                return Err("Palette count does not match colors".into());
            }
        }
        "gpl" => {
            if text.lines().next().map(str::trim) != Some("GIMP Palette") {
                return Err("Expected GIMP Palette".into());
            }
            for line in text.lines().skip(1) {
                let line = line.trim();
                if line.is_empty()
                    || line.starts_with('#')
                    || line.starts_with("Name:")
                    || line.starts_with("Columns:")
                {
                    continue;
                }
                p.push(parse_rgb(line)?);
            }
        }
        _ => return Err("Use .hex, .pal, .gpl or .json".into()),
    }
    if p.is_empty() || p.len() > 256 {
        return Err("Palette must contain 1–256 colors".into());
    }
    Ok(p)
}
fn parse_rgb(s: &str) -> Result<Pixel, String> {
    let values: Vec<_> = s.split_whitespace().take(3).collect();
    if values.len() != 3 {
        return Err("Expected three RGB bytes".into());
    }
    let mut c = [0, 0, 0, 255];
    for i in 0..3 {
        c[i] = values[i]
            .parse::<u8>()
            .map_err(|_| "RGB values must be 0–255")?;
    }
    Ok(c)
}
pub fn write_palette(p: &[Pixel], ext: &str) -> Result<Vec<u8>, String> {
    if p.is_empty() || p.len() > 256 {
        return Err("Palette must contain 1–256 colors".into());
    }
    let s = match ext.to_ascii_lowercase().as_str() {
        "json" => serde_json::to_string_pretty(p).map_err(|e| e.to_string())?,
        "hex" => p
            .iter()
            .map(|c| {
                if c[3] == 255 {
                    format!("{:02X}{:02X}{:02X}\n", c[0], c[1], c[2])
                } else {
                    format!("{:02X}{:02X}{:02X}{:02X}\n", c[0], c[1], c[2], c[3])
                }
            })
            .collect(),
        "pal" | "gpl" => {
            let mut s = if ext.eq_ignore_ascii_case("pal") {
                format!("JASC-PAL\n0100\n{}\n", p.len())
            } else {
                "GIMP Palette\nName: PixelCraft\nColumns: 16\n#\n".into()
            };
            for c in p {
                s.push_str(&format!("{} {} {}\n", c[0], c[1], c[2]));
            }
            s
        }
        _ => return Err("Use .hex, .pal, .gpl or .json".into()),
    };
    Ok(s.into_bytes())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GridImport {
    pub width: usize,
    pub height: usize,
    pub offset_x: usize,
    pub offset_y: usize,
    pub spacing_x: usize,
    pub spacing_y: usize,
    pub layers: bool,
}
impl Default for GridImport {
    fn default() -> Self {
        Self {
            width: 16,
            height: 16,
            offset_x: 0,
            offset_y: 0,
            spacing_x: 0,
            spacing_y: 0,
            layers: false,
        }
    }
}
pub fn grid_cells(im: &RgbaImage, g: &GridImport) -> Result<Vec<(u32, u32)>, String> {
    if g.width == 0
        || g.height == 0
        || g.width > MAX_DIMENSION
        || g.height > MAX_DIMENSION
        || g.spacing_x > MAX_DIMENSION
        || g.spacing_y > MAX_DIMENSION
        || g.offset_x > im.width() as usize
        || g.offset_y > im.height() as usize
    {
        return Err("Invalid cell size, offset or spacing".into());
    }
    let mut out = vec![];
    for y in (g.offset_y..im.height() as usize).step_by(g.height + g.spacing_y) {
        if y + g.height > im.height() as usize {
            break;
        }
        for x in (g.offset_x..im.width() as usize).step_by(g.width + g.spacing_x) {
            if x + g.width > im.width() as usize {
                break;
            }
            out.push((x as u32, y as u32));
            if out.len() > 65535 {
                return Err("Too many cells".into());
            }
        }
    }
    if out.is_empty() || out.len().saturating_mul(g.width).saturating_mul(g.height) > MAX_PIXELS {
        return Err("No complete cells, or import exceeds pixel budget".into());
    }
    Ok(out)
}
pub fn import_grid(im: &RgbaImage, g: &GridImport) -> Result<Document, String> {
    let cells = grid_cells(im, g)?;
    let mut d = Document::new(g.width, g.height);
    for (i, (x, y)) in cells.into_iter().enumerate() {
        if i > 0 {
            if g.layers {
                d.add_layer();
            } else {
                d.add_frame(i - 1, false);
            }
        }
        let (l, f) = if g.layers { (i, 0) } else { (0, i) };
        d.layers[l].cels[f] = image::imageops::crop_imm(im, x, y, g.width as u32, g.height as u32)
            .to_image()
            .pixels()
            .map(|p| p.0)
            .collect();
    }
    d.validate()?;
    Ok(d)
}
pub fn tag_frames(t: &FrameTag) -> Vec<usize> {
    let mut frames: Vec<_> = (t.from..=t.to).collect();
    if t.direction == 1 || t.direction == 3 {
        frames.reverse();
    }
    if t.direction >= 2 && frames.len() > 2 {
        frames.extend(
            frames[1..frames.len() - 1]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>(),
        );
    }
    frames
}
pub fn tag_document(d: &Document, index: usize) -> Result<Document, String> {
    d.validate()?;
    let t = d.tags.get(index).ok_or("Unknown tag")?;
    let frames = tag_frames(t);
    let mut out = d.clone();
    out.durations = frames.iter().map(|&f| d.durations[f]).collect();
    out.frame_palettes = frames.iter().map(|&f| d.palette_for(f).to_vec()).collect();
    for (l, s) in out.layers.iter_mut().zip(&d.layers) {
        l.cels = frames.iter().map(|&f| s.cels[f].clone()).collect();
        l.cel_meta = frames.iter().map(|&f| s.cel_meta[f].clone()).collect();
        l.tile_cels = frames.iter().map(|&f| s.tile_cels[f].clone()).collect();
    }
    out.metadata.clear();
    out.tags = vec![FrameTag {
        from: 0,
        to: frames.len() - 1,
        direction: 0,
        ..t.clone()
    }];
    out.validate()?;
    Ok(out)
}
pub fn batch_tags(
    dir: &Path,
    d: &Document,
    gif: bool,
    options: &SheetOptions,
) -> Result<Vec<PathBuf>, String> {
    if d.tags.is_empty() {
        return Err("Add animation tags before batch exporting".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut paths = vec![];
    for (i, t) in d.tags.iter().enumerate() {
        let name: String = t
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = dir.join(format!(
            "{:03}-{}.{}",
            i + 1,
            if name.is_empty() { "tag" } else { &name },
            if gif { "gif" } else { "png" }
        ));
        if gif {
            let doc = tag_document(d, i)?;
            formats::export_gif_with_repeat(&path, &doc, t.repeat)?;
        } else {
            interchange::export_sheet(
                &path,
                d,
                &SheetOptions {
                    tag: Some(i),
                    ..options.clone()
                },
            )?;
        }
        paths.push(path);
    }
    Ok(paths)
}
pub fn shortcut_preset(name: &str) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for (k, a) in [
        ("Ctrl+N", "new"),
        ("Ctrl+O", "open"),
        ("Ctrl+S", "save"),
        ("Ctrl+Shift+S", "saveas"),
        ("Ctrl+W", "closetab"),
        ("Ctrl+Z", "undo"),
        ("Ctrl+Shift+Z", "redo"),
        ("Ctrl+Y", "redo"),
        ("Ctrl+C", "copy"),
        ("Ctrl+X", "cut"),
        ("Ctrl+V", "paste"),
        ("Ctrl+A", "selectall"),
        ("Escape", "deselect"),
        ("Delete", "clear"),
        ("Backspace", "clear"),
        ("Comma", "prev"),
        ("Period", "next"),
        ("B", "pencil"),
        ("E", "eraser"),
        ("G", "fill"),
        ("I", "picker"),
        ("L", "line"),
        ("U", "rectangle"),
        ("O", "ellipse"),
        ("M", "selection"),
        ("W", "wand"),
        ("Q", "lasso"),
        ("V", "move"),
        ("D", "gradient"),
        ("H", "hand"),
        ("Space", "play"),
        ("X", "swap"),
    ] {
        m.insert(k.into(), a.into());
    }
    if name == "Aseprite" {
        m.insert("Q".into(), "rectangle".into());
        m.insert("R".into(), "ellipse".into());
        m.insert("L".into(), "lasso".into());
    }
    if name == "Photoshop" {
        m.insert("L".into(), "lasso".into());
        m.insert("Ctrl+D".into(), "deselect".into());
        m.insert("D".into(), "resetcolors".into());
        m.insert("G".into(), "gradient".into());
        m.insert("Shift+G".into(), "fill".into());
    }
    m
}
