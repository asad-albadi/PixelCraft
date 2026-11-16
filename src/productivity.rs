//! Shared drawing and interchange operations for the editor and headless tools.
use crate::automation::mcp_io;
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
    matching_on_axes(p, w, h, seed, tolerance, contiguous, wrap.into())
}

pub fn matching_on_axes(
    p: &[Pixel],
    w: usize,
    h: usize,
    seed: (i32, i32),
    tolerance: u8,
    contiguous: bool,
    axes: WrapAxes,
) -> Vec<bool> {
    matching_by_on_axes(p, w, h, seed, contiguous, axes, |c, target| {
        (c[3] == 0 && target[3] == 0) || (0..4).all(|i| c[i].abs_diff(target[i]) <= tolerance)
    })
}

pub fn matching_by<T: Copy>(
    p: &[T],
    w: usize,
    h: usize,
    seed: (i32, i32),
    contiguous: bool,
    wrap: bool,
    matches: impl Fn(T, T) -> bool,
) -> Vec<bool> {
    matching_by_on_axes(p, w, h, seed, contiguous, wrap.into(), matches)
}

pub fn matching_by_on_axes<T: Copy>(
    p: &[T],
    w: usize,
    h: usize,
    seed: (i32, i32),
    contiguous: bool,
    axes: WrapAxes,
    matches: impl Fn(T, T) -> bool,
) -> Vec<bool> {
    let mut out = vec![false; p.len()];
    let (x, y) = seed;
    if w == 0
        || h == 0
        || p.len() != w.saturating_mul(h)
        || x < 0
        || y < 0
        || x as usize >= w
        || y as usize >= h
    {
        return out;
    }
    let first = y as usize * w + x as usize;
    let target = p[first];
    if !contiguous {
        for (v, c) in out.iter_mut().zip(p) {
            *v = matches(*c, target);
        }
        return out;
    }
    let mut stack = vec![first];
    out[first] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        for point in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            let Some((x, y)) = axes.position((w, h), point) else {
                continue;
            };
            let n = y as usize * w + x as usize;
            if !out[n] && matches(p[n], target) {
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
    paint_brush_with_ink(d, (l, f), p, color, size, o, |_, _, source| source);
}

pub fn paint_brush_on_axes(
    d: &mut Document,
    (l, f): (usize, usize),
    p: (i32, i32),
    color: Pixel,
    size: i32,
    o: &BrushOptions,
    axes: WrapAxes,
) {
    paint_brush_with_ink_on_axes(d, (l, f, axes), p, color, size, o, |_, _, source| source);
}

pub(crate) fn paint_brush_with_ink(
    d: &mut Document,
    (l, f): (usize, usize),
    p: (i32, i32),
    color: Pixel,
    size: i32,
    o: &BrushOptions,
    ink: impl FnMut(usize, Pixel, Pixel) -> Pixel,
) {
    paint_brush_with_ink_on_axes(d, (l, f, d.wraparound.into()), p, color, size, o, ink);
}

fn paint_brush_with_ink_on_axes(
    d: &mut Document,
    (l, f, axes): (usize, usize, WrapAxes),
    p: (i32, i32),
    color: Pixel,
    size: i32,
    o: &BrushOptions,
    mut ink: impl FnMut(usize, Pixel, Pixel) -> Pixel,
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
    let mut shaded = std::collections::HashSet::new();
    for (x, y, mut c) in points {
        let Some(i) = d.position_on_axes(x, y, axes) else {
            continue;
        };
        if o.mask.as_ref().is_some_and(|m| m.get(i) != Some(&true)) {
            continue;
        }
        let (x, y) = ((i % d.width) as i32, (i / d.width) as i32);
        match o.mode {
            BrushMode::Shade => {
                if !shaded.insert(i) {
                    continue;
                }
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
            BrushMode::Dither if (x + y).rem_euclid(2) == 1 => {
                c = o.alternate;
            }
            _ => {}
        }
        if o.mode != BrushMode::Selection {
            c = d.snapped_color(f, c);
        }
        let previous = d.layers[l].cels[f][i];
        let next = ink(i, previous, c);
        if next != previous || d.color_mode == ColorMode::Indexed {
            d.set_pixel(l, f, i, next);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceImage {
    #[serde(default)]
    pub behind: bool,
    #[serde(default)]
    pub floating: bool,
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
    let mut reader = mcp_io::image_reader(path)
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
        behind: false,
        floating: false,
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
    let mut onion = std::borrow::Cow::Borrowed(d);
    if d.layers.iter().any(|layer| layer.exclude_from_onion) {
        for layer in &mut onion.to_mut().layers {
            if layer.exclude_from_onion {
                layer.visible = false;
            }
        }
    }
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
                for (p, s) in out.iter_mut().zip(onion.composite(frame)) {
                    let alpha = (s[3] as u32 * opacity as u32 / 255 / distance as u32) as u8;
                    *p = over(*p, [tint[0], tint[1], tint[2], alpha], 255);
                }
            }
        }
    }
    for (p, c) in out.iter_mut().zip(d.composite_view(f)) {
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
impl GridImport {
    /// Suggest square cells when one complete row or column fits the image.
    pub fn suggest_strip(width: usize, height: usize) -> Option<(usize, usize)> {
        let short = width.min(height);
        let long = width.max(height);
        (short != 0 && long > short && long.is_multiple_of(short)).then_some((short, short))
    }

    pub fn use_cell_size(&mut self, (width, height): (usize, usize)) {
        self.width = width;
        self.height = height;
        self.offset_x = 0;
        self.offset_y = 0;
        self.spacing_x = 0;
        self.spacing_y = 0;
    }
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridImportOptions {
    #[serde(flatten)]
    pub grid: GridImport,
    pub skip_transparent: bool,
    pub duration: u16,
    pub order: GridOrder,
    pub include_partial: bool,
    pub columns: Option<usize>,
    pub rows: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridOrder {
    #[default]
    RowMajor,
    ColumnMajor,
}

impl Default for GridImportOptions {
    fn default() -> Self {
        Self {
            grid: GridImport::default(),
            skip_transparent: false,
            duration: DEFAULT_DURATION,
            order: GridOrder::RowMajor,
            include_partial: false,
            columns: None,
            rows: None,
        }
    }
}

#[derive(Debug)]
pub struct GridPreview {
    /// The cells that will become frames or layers, in the selected order.
    pub cells: Vec<(u32, u32)>,
    pub skipped: usize,
    pub cell_size: [usize; 2],
}

pub fn grid_cells(im: &RgbaImage, g: &GridImport) -> Result<Vec<(u32, u32)>, String> {
    Ok(preview_grid(im, g, false)?.cells)
}

pub fn preview_grid(
    im: &RgbaImage,
    g: &GridImport,
    skip_transparent: bool,
) -> Result<GridPreview, String> {
    preview_grid_with_options(
        im,
        &GridImportOptions {
            grid: g.clone(),
            skip_transparent,
            ..GridImportOptions::default()
        },
    )
}

pub fn preview_grid_with_options(
    im: &RgbaImage,
    options: &GridImportOptions,
) -> Result<GridPreview, String> {
    let g = &options.grid;
    let (width, columns) = grid_axis(
        im.width() as usize,
        g.offset_x,
        g.spacing_x,
        g.width,
        options.columns,
        options.include_partial,
    )?;
    let (height, rows) = grid_axis(
        im.height() as usize,
        g.offset_y,
        g.spacing_y,
        g.height,
        options.rows,
        options.include_partial,
    )?;
    let mut out = GridPreview {
        cells: vec![],
        skipped: 0,
        cell_size: [width, height],
    };
    let row_width = im.width() as usize;
    for index in 0..columns.saturating_mul(rows) {
        let (column, row) = match options.order {
            GridOrder::RowMajor => (index % columns, index / columns),
            GridOrder::ColumnMajor => (index / rows, index % rows),
        };
        let x = g.offset_x + column * (width + g.spacing_x);
        let y = g.offset_y + row * (height + g.spacing_y);
        let right = (x + width).min(row_width);
        let bottom = (y + height).min(im.height() as usize);
        if options.skip_transparent
            && (y..bottom).all(|row| {
                let start = (row * row_width + x) * 4;
                let end = (row * row_width + right) * 4;
                im.as_raw()[start..end]
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .all(|&alpha| alpha == 0)
            })
        {
            out.skipped += 1;
            continue;
        }
        out.cells.push((x as u32, y as u32));
        if out.cells.len() > 65535 {
            return Err("Too many cells".into());
        }
    }
    if (out.cells.is_empty() && out.skipped == 0)
        || out.cells.len().saturating_mul(width).saturating_mul(height) > MAX_PIXELS
    {
        return Err("No complete cells, or import exceeds pixel budget".into());
    }
    Ok(out)
}

fn grid_axis(
    image: usize,
    offset: usize,
    spacing: usize,
    size: usize,
    count: Option<usize>,
    partial: bool,
) -> Result<(usize, usize), String> {
    if spacing > MAX_DIMENSION || offset > image {
        return Err("Invalid cell offset or spacing".into());
    }
    let span = image - offset;
    if let Some(count) = count {
        if !(1..=MAX_DIMENSION).contains(&count) {
            return Err("Invalid grid row or column count".into());
        }
        let available = span
            .checked_sub(spacing * (count - 1))
            .ok_or("Grid spacing exceeds the image")?;
        let size = if partial {
            available.div_ceil(count)
        } else {
            available / count
        };
        if !(1..=MAX_DIMENSION).contains(&size) || (size + spacing) * (count - 1) >= span {
            return Err("The requested grid count does not fit uniform cells".into());
        }
        return Ok((size, count));
    }
    if !(1..=MAX_DIMENSION).contains(&size) {
        return Err("Invalid cell size".into());
    }
    let pitch = size + spacing;
    let count = if partial {
        span.div_ceil(pitch)
    } else {
        (span + spacing) / pitch
    };
    Ok((size, count))
}

pub fn import_grid(im: &RgbaImage, g: &GridImport) -> Result<Document, String> {
    import_grid_with_options(
        im,
        &GridImportOptions {
            grid: g.clone(),
            ..GridImportOptions::default()
        },
    )
}

pub fn import_grid_with_options(
    im: &RgbaImage,
    options: &GridImportOptions,
) -> Result<Document, String> {
    import_grid_samples(im, options, None)
}

pub struct GridSource {
    pub image: RgbaImage,
    mode: ColorMode,
    palette: Vec<Pixel>,
    slots: Option<std::sync::Arc<Vec<u8>>>,
    profile: Vec<u8>,
    metadata: Vec<MetadataChunk>,
}

impl GridSource {
    pub fn read(path: &Path) -> Result<Self, String> {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            let mut doc = formats::load(path)?;
            if doc.durations.len() == 1 {
                let layer = &mut doc.layers[0];
                let pixels = layer.cels[0].iter().flatten().copied().collect();
                let image = RgbaImage::from_raw(doc.width as u32, doc.height as u32, pixels)
                    .ok_or("Invalid grid image")?;
                return Ok(Self {
                    image,
                    mode: doc.color_mode,
                    palette: doc.palette,
                    slots: layer.cel_meta[0].indices.take(),
                    profile: doc.color_profile,
                    metadata: doc.metadata,
                });
            }
        }
        Ok(Self {
            image: read_image(path)?,
            mode: ColorMode::Rgba,
            palette: Vec::new(),
            slots: None,
            profile: Vec::new(),
            metadata: Vec::new(),
        })
    }

    pub fn import(&self, options: &GridImportOptions) -> Result<Document, String> {
        import_grid_samples(&self.image, options, Some(self))
    }
}

fn import_grid_samples(
    im: &RgbaImage,
    options: &GridImportOptions,
    source: Option<&GridSource>,
) -> Result<Document, String> {
    let g = &options.grid;
    if options.duration == 0 {
        return Err("Duration must be 1–65535 ms".into());
    }
    let GridPreview {
        cells,
        cell_size: [width, height],
        ..
    } = preview_grid_with_options(im, options)?;
    if cells.is_empty() {
        return Err("Every selected cell is transparent".into());
    }
    let mut d = Document::new(width, height);
    let mut indexed = None;
    if let Some(source) = source {
        d.color_profile.clone_from(&source.profile);
        d.metadata.clone_from(&source.metadata);
        if source.mode == ColorMode::Grayscale {
            d.color_mode = ColorMode::Grayscale;
        } else if let Some(slots) = &source.slots {
            let has_padding = cells.iter().any(|&(x, y)| {
                x as usize + width > im.width() as usize
                    || y as usize + height > im.height() as usize
            });
            let mut palette = source.palette.clone();
            let mut clear = palette.iter().position(|p| p[3] == 0);
            if has_padding && clear.is_none() && palette.len() < 256 {
                clear = Some(palette.len());
                palette.push(CLEAR);
            }
            if !has_padding || clear.is_some() {
                d.color_mode = ColorMode::Indexed;
                d.palette = palette;
                d.transparent_index = clear.unwrap_or(0);
                indexed = Some((slots, clear));
            }
        }
    }
    d.durations[0] = options.duration;
    for (i, (x, y)) in cells.into_iter().enumerate() {
        if i > 0 {
            if g.layers {
                d.add_layer();
            } else {
                d.add_frame(i - 1, false);
            }
        }
        let (l, f) = if g.layers { (i, 0) } else { (0, i) };
        let copied_width = width.min((im.width() - x) as usize);
        let copied_height = height.min((im.height() - y) as usize);
        let mut pixels = vec![CLEAR; width * height];
        for row in 0..copied_height {
            for column in 0..copied_width {
                pixels[row * width + column] = im.get_pixel(x + column as u32, y + row as u32).0;
            }
        }
        d.layers[l].cels[f] = pixels.into();
        if let Some((slots, clear)) = indexed {
            let mut indices = vec![clear.unwrap_or(0) as u8; width * height];
            for row in 0..copied_height {
                let from = (y as usize + row) * im.width() as usize + x as usize;
                let to = row * width;
                indices[to..to + copied_width].copy_from_slice(&slots[from..from + copied_width]);
            }
            d.layers[l].background = clear.is_none();
            d.layers[l].cel_meta[f].indices = Some(indices.into());
        }
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
    // A derived clip can contain the same source frame more than once.
    out.frame_export_ids = Default::default();
    if d.has_frame_names() {
        out.frame_names = frames
            .iter()
            .map(|&f| d.frame_name(f).unwrap_or_default().to_owned())
            .collect();
        out.prune_frame_names();
    }
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
pub(crate) fn tag_export_paths(dir: &Path, d: &Document, gif: bool) -> Vec<PathBuf> {
    d.tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let name: String = tag
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
            dir.join(format!(
                "{:03}-{}.{}",
                i + 1,
                if name.is_empty() { "tag" } else { &name },
                if gif { "gif" } else { "png" }
            ))
        })
        .collect()
}

pub fn batch_tags(
    dir: &Path,
    d: &Document,
    gif: bool,
    options: &SheetOptions,
) -> Result<Vec<PathBuf>, String> {
    batch_tags_with_previous(dir, dir, d, gif, options)
}

pub(crate) fn batch_tags_with_previous(
    dir: &Path,
    previous_dir: &Path,
    d: &Document,
    gif: bool,
    options: &SheetOptions,
) -> Result<Vec<PathBuf>, String> {
    if d.tags.is_empty() {
        return Err("Add animation tags before batch exporting".into());
    }
    mcp_io::create_dir_all(dir).map_err(|e| e.to_string())?;
    let paths = tag_export_paths(dir, d, gif);
    for (i, (tag, path)) in d.tags.iter().zip(&paths).enumerate() {
        if gif {
            formats::export_gif_frames(path, d, tag.repeat, tag_frames(tag))?;
        } else {
            let previous = previous_dir.join(path.file_name().unwrap());
            interchange::export_sheet_with_previous(
                path,
                &previous,
                d,
                &SheetOptions {
                    tag: Some(i),
                    frame_range: None,
                    ..options.clone()
                },
            )?;
        }
    }
    Ok(paths)
}
/// Merge older partial maps with defaults; null explicitly disables a binding.
pub fn load_shortcuts(bytes: &[u8]) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut bindings = shortcut_preset("PixelCraft");
    if let Ok(saved) =
        serde_json::from_slice::<std::collections::BTreeMap<String, Option<String>>>(bytes)
    {
        for (key, action) in saved {
            if let Some(action) = action {
                bindings.insert(key, action);
            } else {
                bindings.remove(&key);
            }
        }
    } else {
        bindings.extend(crate::contributions::shortcuts(bytes)?.bindings);
    }
    Ok(bindings)
}

/// Record absent defaults as removals so loading cannot silently restore them.
pub fn save_shortcuts(
    bindings: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<u8>, String> {
    let mut saved: std::collections::BTreeMap<String, Option<&String>> = bindings
        .iter()
        .map(|(key, action)| (key.clone(), Some(action)))
        .collect();
    for key in shortcut_preset("PixelCraft").into_keys() {
        saved.entry(key).or_insert(None);
    }
    serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())
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

/// Rasterize a reference's placement to a new image layer using nearest sampling.
pub fn reference_to_layer(
    d: &mut Document,
    r: &ReferenceImage,
    frame: usize,
) -> Result<(), String> {
    if frame >= d.durations.len() || !d.can_grow(d.width * d.height * d.durations.len()) {
        return Err("Layer exceeds document limits".into());
    }
    let n = d.layers.len();
    d.add_layer();
    d.layers[n].name = r.name.clone();
    for y in 0..d.height {
        for x in 0..d.width {
            let sx = ((x as f32 - r.x) / r.scale).floor();
            let sy = ((y as f32 - r.y) / r.scale).floor();
            if sx >= 0. && sy >= 0. && sx < r.width as f32 && sy < r.height as f32 {
                let mut p = r.pixels[sy as usize * r.width + sx as usize];
                p[3] = ((p[3] as u16 * r.opacity as u16 + 127) / 255) as u8;
                d.set_pixel(n, frame, y * d.width + x, p);
            }
        }
    }
    d.validate()
}

/// Expand folders deterministically. Symlinks are not followed to avoid cycles.
pub const ATLAS_EXTENSIONS: &[&str] = &[
    "pixel", "ase", "aseprite", "png", "gif", "bmp", "jpg", "jpeg", "jfif", "webp", "json", "ora",
    "apng", "tmj", "tsj", "ico", "pcx", "qoi", "tga", "fli", "flc", "psd", "psb",
];
pub fn expand_atlas_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    expand_atlas_inputs_with_path_check(inputs, |_| Ok(()))
}

pub(crate) fn expand_atlas_inputs_with_path_check(
    inputs: &[PathBuf],
    check_path: impl Fn(&Path) -> Result<(), String>,
) -> Result<Vec<PathBuf>, String> {
    fn visit(
        p: &Path,
        out: &mut Vec<PathBuf>,
        check_path: &impl Fn(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let meta = mcp_io::metadata(p).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Ok(());
        }
        if meta.is_dir() {
            check_path(p)?;
            let mut children = mcp_io::children(p).map_err(|e| e.to_string())?;
            children.sort();
            for child in children {
                visit(&child, out, check_path)?;
            }
        } else if meta.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| ATLAS_EXTENSIONS.contains(&s.to_ascii_lowercase().as_str()))
        {
            if out.len() >= 4096 {
                return Err("Atlas input limit is 4096 files".into());
            }
            out.push(p.to_path_buf());
        }
        Ok(())
    }
    let mut out = vec![];
    for p in inputs {
        visit(p, &mut out, &check_path)?;
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err("No supported sprites found".into());
    }
    Ok(out)
}

/// Sample the rendered reference stack, excluding checkerboard and onion skin.
pub fn sample_canvas(d: &Document, frame: usize, x: i32, y: i32, references: bool) -> Pixel {
    let artwork = d.index(x, y).map_or(CLEAR, |i| {
        if references {
            d.composite_view(frame)[i]
        } else {
            d.composite(frame)[i]
        }
    });
    if !references {
        return artwork;
    }
    let mut color = CLEAR;
    for behind in [true, false] {
        if !behind {
            color = over(color, artwork, 255);
        }
        for r in &d.references {
            if !r.visible || r.floating || r.behind != behind {
                continue;
            }
            let rx = ((x as f32 - r.x) / r.scale).floor();
            let ry = ((y as f32 - r.y) / r.scale).floor();
            if rx >= 0. && ry >= 0. && rx < r.width as f32 && ry < r.height as f32 {
                color = over(
                    color,
                    r.pixels[ry as usize * r.width + rx as usize],
                    r.opacity,
                );
            }
        }
    }
    color
}
