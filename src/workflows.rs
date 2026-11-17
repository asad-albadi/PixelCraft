use crate::model::*;
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{borrow::Cow, ops::RangeInclusive};

mod artwork_palette;
mod canvas_resize;
pub mod curve;
pub use artwork_palette::{generate_artwork_palette, generate_artwork_palette_until};
pub mod filters;
pub mod retouch;
mod rotsprite;
pub mod selection;
mod sheet_layout;
mod sheet_split;
pub(crate) use sheet_layout::{
    uniform_sheet_area, uniform_sheet_dimensions, validate_sheet_dimensions,
};
pub(crate) use sheet_split::{SheetCrop, pack_split_sheet, split_sheet_work};
mod sprite_resize;
mod sprite_transform;

pub use canvas_resize::resize_canvas;
pub use sprite_resize::{resample_sprite, trim_sprite};
pub use sprite_transform::{Orientation, duplicate_sprite, orient_sprite};

/// SplitMix64's finalizer, used to reproduce seeded painting effects.
pub fn mix_paint_seed(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Transform {
    Move(i32, i32),
    Resize(usize, usize),
    SkewX(i32),
    SkewY(i32),
    Rotate,
    RotateAround { degrees: f64, pivot: [f64; 2] },
    FlipX,
    FlipY,
}

fn skew_bounds([x, y, w, h]: [i32; 4], t: Transform) -> [i32; 4] {
    match t {
        Transform::SkewX(delta) => [x + delta.min(0), y, w + delta.abs(), h],
        Transform::SkewY(delta) => [x, y + delta.min(0), w, h + delta.abs()],
        _ => unreachable!(),
    }
}

fn skew_source([x, y, w, h]: [i32; 4], position: [i32; 2], t: Transform) -> (i32, i32) {
    let [dx, dy] = position;
    // Offset of the inverse shear at the center of this destination pixel.
    let shift = |offset: i32, extent: i32, delta: i32| {
        let extent = i64::from(extent);
        let shift = i64::from(2 * offset + 1) * i64::from(delta);
        ((extent - shift).div_euclid(2 * extent)) as i32
    };
    match t {
        Transform::SkewX(delta) => (dx + shift(dy - y, h, delta), dy),
        Transform::SkewY(delta) => (dx, dy + shift(dx - x, w, delta)),
        _ => unreachable!(),
    }
}
pub fn inside_polygon(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    if points.len() < 3 {
        return false;
    }
    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
    let mut hit = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[j]);
        if (a.1 as f64 > py) != (b.1 as f64 > py)
            && px < (b.0 - a.0) as f64 * (py - a.1 as f64) / (b.1 - a.1) as f64 + a.0 as f64
        {
            hit = !hit
        }
        j = i;
    }
    hit
}

/// Rasterize the closed outline and optional even-odd interior.
pub fn polygon_coverage(
    dimensions: (usize, usize),
    vertices: &[(i32, i32)],
    filled: bool,
    brush: (i32, BrushShape),
    axes: WrapAxes,
) -> Result<Vec<bool>, String> {
    let (width, height) = dimensions;
    if !(3..=4096).contains(&vertices.len())
        || vertices
            .iter()
            .any(|&(x, y)| x.unsigned_abs().max(y.unsigned_abs()) as usize > 2 * MAX_DIMENSION)
    {
        return Err("Polygon needs 3 to 4096 points within the drawing limits".into());
    }
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("Invalid polygon canvas dimensions".into());
    }
    let edges = vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1));
    let size = if filled {
        1
    } else {
        brush.0.clamp(1, MAX_BRUSH_SIZE)
    };
    let stamps: u64 = edges
        .clone()
        .map(|(a, b)| u64::from(a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))) + 1)
        .sum();
    if stamps * u64::from(size as u32).pow(2) > MAX_PIXELS as u64 {
        return Err("Polygon outline exceeds the drawing work limit".into());
    }
    let mut outline = Document::new(width, height);
    outline.brush_shape = brush.1;
    for (from, to) in edges {
        outline.line_on_axes((0, 0), from, to, [255; 4], size, axes);
    }
    let mut coverage = if filled {
        selection::polygon_mask(width, height, vertices)?
    } else {
        vec![false; width * height]
    };
    outline.layers[0].cels[0].visit_nonclear(|index, _| coverage[index] = true);
    Ok(coverage)
}
fn snap_near_integer(value: f64) -> f64 {
    if (value - value.round()).abs() < 1e-9 {
        value.round()
    } else {
        value
    }
}

fn rotation_basis(degrees: f64) -> [f64; 2] {
    let (sin, cos) = degrees.rem_euclid(360.).to_radians().sin_cos();
    [snap_near_integer(sin), snap_near_integer(cos)]
}

fn needs_enlargement(basis: [f64; 2]) -> bool {
    basis.into_iter().all(|v| v != 0.)
}

/// Check RotSprite's temporary enlargement budget. Right-angle turns use exact sampling.
pub fn rotsprite_within_budget(width: usize, height: usize, degrees: f64) -> bool {
    !needs_enlargement(rotation_basis(degrees)) || rotsprite::enlargement_fits(width, height)
}

fn rotation_bounds(
    bounds: [i32; 4],
    degrees: f64,
    pivot: [f64; 2],
) -> Option<([i32; 4], [f64; 2])> {
    if !degrees.is_finite()
        || pivot
            .iter()
            .any(|v| !v.is_finite() || v.abs() > (MAX_DIMENSION * 2) as f64)
    {
        return None;
    }
    let [x, y, w, h] = bounds;
    let [sin, cos] = rotation_basis(degrees);
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for [x, y] in [[x, y], [x + w, y], [x, y + h], [x + w, y + h]] {
        let (x, y) = (f64::from(x) - pivot[0], f64::from(y) - pivot[1]);
        let result = [pivot[0] + x * cos - y * sin, pivot[1] + x * sin + y * cos];
        for axis in 0..2 {
            min[axis] = min[axis].min(result[axis]);
            max[axis] = max[axis].max(result[axis]);
        }
    }
    let origin = min.map(|v| snap_near_integer(v).floor() as i32);
    let end = max.map(|v| snap_near_integer(v).ceil() as i32);
    Some((
        [origin[0], origin[1], end[0] - origin[0], end[1] - origin[1]],
        [sin, cos],
    ))
}

fn inverse_rotation(position: [i32; 2], pivot: [f64; 2], [sin, cos]: [f64; 2]) -> [f64; 2] {
    let x = f64::from(position[0]) + 0.5 - pivot[0];
    let y = f64::from(position[1]) + 0.5 - pivot[1];
    [pivot[0] + x * cos + y * sin, pivot[1] - x * sin + y * cos]
}

fn rotated_source(position: [i32; 2], pivot: [f64; 2], basis: [f64; 2]) -> (i32, i32) {
    let [x, y] = inverse_rotation(position, pivot, basis);
    (x.floor() as i32, y.floor() as i32)
}

pub fn transform(
    d: &mut Document,
    l: usize,
    f: usize,
    b: [i32; 4],
    mask: Option<&[bool]>,
    t: Transform,
) -> Result<[i32; 4], String> {
    transform_selection(d, l, f, b, mask, t, false).map(|(bounds, _)| bounds)
}

/// Rotate using RotSprite and return the exact destination selection sampled by the rotation.
pub fn transform_rotsprite(
    d: &mut Document,
    l: usize,
    f: usize,
    b: [i32; 4],
    mask: Option<&[bool]>,
    degrees: f64,
    pivot: [f64; 2],
) -> Result<([i32; 4], Vec<bool>), String> {
    transform_selection(
        d,
        l,
        f,
        b,
        mask,
        Transform::RotateAround { degrees, pivot },
        true,
    )
}

fn transform_selection(
    d: &mut Document,
    l: usize,
    f: usize,
    b: [i32; 4],
    mask: Option<&[bool]>,
    t: Transform,
    use_rotsprite: bool,
) -> Result<([i32; 4], Vec<bool>), String> {
    if !d.editable(l) || d.layers[l].tileset.is_some() || f >= d.durations.len() {
        return Err("Select an unlocked image layer".into());
    }
    let [x, y, w, h] = b;
    if w < 1
        || h < 1
        || w as usize > MAX_DIMENSION
        || h as usize > MAX_DIMENSION
        || x.unsigned_abs() > MAX_DIMENSION as u32
        || y.unsigned_abs() > MAX_DIMENSION as u32
    {
        return Err("Empty selection".into());
    }
    let mut rotation = None;
    let (nw, nh, dx, dy) = match t {
        Transform::RotateAround { degrees, pivot } => {
            let ([nx, ny, nw, nh], basis) =
                rotation_bounds(b, degrees, pivot).ok_or("Invalid rotation angle or pivot")?;
            rotation = Some((pivot, basis));
            (nw as usize, nh as usize, nx - x, ny - y)
        }
        Transform::Move(dx, dy) => (w as usize, h as usize, dx, dy),
        Transform::Resize(w, h) => (w, h, 0, 0),
        Transform::SkewX(delta) | Transform::SkewY(delta) => {
            if delta.unsigned_abs() > MAX_DIMENSION as u32 {
                return Err("Selection exceeds limits".into());
            }
            let [nx, ny, nw, nh] = skew_bounds(b, t);
            (nw as usize, nh as usize, nx - x, ny - y)
        }
        Transform::Rotate => (h as usize, w as usize, 0, 0),
        _ => (w as usize, h as usize, 0, 0),
    };
    if nw == 0
        || nh == 0
        || nw > MAX_DIMENSION
        || nh > MAX_DIMENSION
        || nw.saturating_mul(nh) > MAX_PIXELS
    {
        return Err("Selection exceeds limits".into());
    }
    let source = d.layers[l].cels[f].clone();
    let source_indices = d.layers[l].cel_meta[f].indices.clone();
    let (width, height) = (d.width as i32, d.height as i32);
    let index = |sx: i32, sy: i32| {
        (sx >= 0 && sy >= 0 && sx < width && sy < height)
            .then(|| sy as usize * width as usize + sx as usize)
    };
    let selected_index =
        |sx, sy| index(sx, sy).filter(|&i| mask.is_none_or(|m| m.get(i) == Some(&true)));
    let sample_source = |sx, sy| {
        let src = selected_index(sx, sy)?;
        Some((source[src], source_indices.as_ref().map(|v| v[src])))
    };
    let enlarged = if use_rotsprite && rotation.is_some_and(|(_, basis)| needs_enlargement(basis)) {
        Some(rotsprite::Enlarged::new(
            w as usize,
            h as usize,
            |sx, sy| sample_source(x + sx as i32, y + sy as i32),
        )?)
    } else {
        None
    };
    for sy in y.max(0)..(y + h).min(height) {
        for sx in x.max(0)..(x + w).min(width) {
            if let Some(i) = selected_index(sx, sy) {
                d.set_pixel(l, f, i, CLEAR);
            }
        }
    }
    let mut rotated_selection = if use_rotsprite {
        vec![false; d.width * d.height]
    } else {
        Vec::new()
    };
    let mut write = |dst, pixel, slot: Option<u8>| {
        if use_rotsprite {
            rotated_selection[dst] = true;
        }
        d.drawing_index = slot.map(usize::from);
        d.set_pixel(l, f, dst, pixel);
    };
    let left = x.saturating_add(dx);
    let top = y.saturating_add(dy);
    for dy in top.max(0)..top.saturating_add(nh as i32).min(height) {
        for dx in left.max(0)..left.saturating_add(nw as i32).min(width) {
            let (ox, oy) = (dx - left, dy - top);
            let position = [dx, dy];
            let dst = dy as usize * width as usize + dx as usize;
            if let Some(enlarged) = &enlarged {
                let (pivot, basis) = rotation.unwrap();
                let [sx, sy] = inverse_rotation(position, pivot, basis);
                if let Some((pixel, index)) = enlarged.sample(sx - f64::from(x), sy - f64::from(y))
                {
                    write(dst, pixel, index);
                }
                continue;
            }
            let (sx, sy) = match t {
                Transform::RotateAround { .. } => {
                    let (pivot, basis) = rotation.unwrap();
                    let (sx, sy) = rotated_source(position, pivot, basis);
                    if !(x..x + w).contains(&sx) || !(y..y + h).contains(&sy) {
                        continue;
                    }
                    (sx - x, sy - y)
                }
                Transform::SkewX(_) | Transform::SkewY(_) => {
                    let (sx, sy) = skew_source(b, position, t);
                    if !(x..x + w).contains(&sx) || !(y..y + h).contains(&sy) {
                        continue;
                    }
                    (sx - x, sy - y)
                }
                Transform::Rotate => (oy, h - 1 - ox),
                Transform::FlipX => (w - 1 - ox, oy),
                Transform::FlipY => (ox, h - 1 - oy),
                _ => (ox * w / nw as i32, oy * h / nh as i32),
            };
            if let Some((pixel, index)) = sample_source(x + sx, y + sy) {
                write(dst, pixel, index);
            }
        }
    }
    d.drawing_index = None;
    Ok(([left, top, nw as i32, nh as i32], rotated_selection))
}
pub fn crop(d: &mut Document, b: [i32; 4]) -> Result<(), String> {
    let [x, y, w, h] = b;
    if w < 1
        || h < 1
        || x < 0
        || y < 0
        || x.checked_add(w).is_none_or(|right| right > d.width as i32)
        || y.checked_add(h)
            .is_none_or(|bottom| bottom > d.height as i32)
    {
        return Err("Crop must be inside canvas".into());
    }
    if d.layers.iter().any(|l| l.tileset.is_some()) {
        return Err("Rasterize tile layers before cropping".into());
    }
    let mut cropped = d.clone();
    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    let mut rasters = std::collections::HashMap::new();
    let mut palette_indices = std::collections::HashMap::new();
    let mut crop_raster = |source: &Pixels| {
        let Some(id) = source.backing_id() else {
            return Pixels::empty(w * h);
        };
        rasters
            .entry(id)
            .or_insert_with(|| {
                if let Pixels::Dense(pixels) = source {
                    let mut cropped = Vec::with_capacity(w * h);
                    for row in y..y + h {
                        let start = row * d.width + x;
                        cropped.extend_from_slice(&pixels[start..start + w]);
                    }
                    if cropped.iter().all(|pixel| *pixel == CLEAR) {
                        Pixels::empty(w * h)
                    } else {
                        cropped.into()
                    }
                } else {
                    let mut cropped = Pixels::empty(w * h);
                    source.visit_nonclear(|index, color| {
                        let (col, row) = (index % d.width, index / d.width);
                        if (x..x + w).contains(&col) && (y..y + h).contains(&row) {
                            cropped.set((row - y) * w + col - x, color);
                        }
                    });
                    cropped
                }
            })
            .clone()
    };
    for l in &mut cropped.layers {
        // Cropping each placement clips a different part of the shared image
        // when its origin differs. Keep links only among matching origins.
        let mut link_groups = std::collections::HashMap::new();
        let mut link_counts = Vec::new();
        for meta in &mut l.cel_meta {
            if let Some(link) = meta.artwork_link {
                let origin = meta.bounds.map_or([0, 0], |b| [b[0], b[1]]);
                let group = *link_groups.entry((link, origin)).or_insert_with(|| {
                    let group = link_counts.len();
                    link_counts.push(0);
                    group
                });
                link_counts[group] += 1;
                meta.artwork_link = Some(group);
            }
        }
        for meta in &mut l.cel_meta {
            if meta
                .artwork_link
                .is_some_and(|group| link_counts[group] < 2)
            {
                meta.artwork_link = None;
            }
        }

        l.mask = l.mask.as_ref().map(&mut crop_raster);
        for (pixels, meta) in l.cels.iter_mut().zip(&mut l.cel_meta) {
            *pixels = crop_raster(pixels);
            if let Some(indices) = &mut meta.indices {
                *indices = palette_indices
                    .entry(std::sync::Arc::as_ptr(indices))
                    .or_insert_with(|| {
                        let mut cropped = Vec::with_capacity(w * h);
                        for row in y..y + h {
                            let start = row * d.width + x;
                            cropped.extend_from_slice(&indices[start..start + w]);
                        }
                        std::sync::Arc::new(cropped)
                    })
                    .clone();
            }
            meta.outside.clear();
            meta.bounds = None;
        }
    }
    cropped.width = w;
    cropped.height = h;
    cropped.validate()?;
    *d = cropped;
    Ok(())
}
pub fn ramp(a: Pixel, b: Pixel, n: usize) -> Vec<Pixel> {
    ramp_interpolated(a, b, n, RampInterpolation::Rgb)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RampInterpolation {
    #[default]
    Rgb,
    Hue,
}

pub fn ramp_interpolated(a: Pixel, b: Pixel, n: usize, space: RampInterpolation) -> Vec<Pixel> {
    let n = n.clamp(2, 256);
    if space == RampInterpolation::Hue {
        let [mut start, saturation, value] = crate::color::rgb_to_hsv(a);
        let [mut end, end_saturation, end_value] = crate::color::rgb_to_hsv(b);
        if saturation == 0. {
            start = end;
        }
        if end_saturation == 0. {
            end = start;
        }
        let arc = (end - start + 0.5).rem_euclid(1.) - 0.5;
        return (0..n)
            .map(|i| {
                if i == 0 {
                    return a;
                }
                if i == n - 1 {
                    return b;
                }
                let t = i as f32 / (n - 1) as f32;
                let lerp = |u: f32, v: f32| u + (v - u) * t;
                crate::color::hsv_to_rgb(
                    [
                        start + arc * t,
                        lerp(saturation, end_saturation),
                        lerp(value, end_value),
                    ],
                    lerp(f32::from(a[3]), f32::from(b[3])).round() as u8,
                )
            })
            .collect();
    }
    (0..n)
        .map(|i| {
            std::array::from_fn(|c| {
                ((a[c] as f32 * (n - 1 - i) as f32 + b[c] as f32 * i as f32) / (n - 1) as f32)
                    .round() as u8
            })
        })
        .collect()
}
/// Export explicit layers with their ancestors. Groups include visible descendants.
pub(crate) fn filter_layers_for_export<'a>(
    d: &'a Document,
    layers: Option<&[usize]>,
) -> Result<Cow<'a, Document>, String> {
    let Some(indices) = layers else {
        return Ok(Cow::Borrowed(d));
    };
    if indices.is_empty()
        || indices.len() > d.layers.len()
        || indices.iter().any(|&i| i >= d.layers.len())
    {
        return Err("Choose at least one valid layer".into());
    }
    let selected: std::collections::HashSet<_> = indices.iter().copied().collect();
    let mut filtered = d.clone();
    for layer in &mut filtered.layers {
        layer.visible = false;
    }
    for index in 0..d.layers.len() {
        let mut ancestor = Some(index);
        while let Some(i) = ancestor {
            if selected.contains(&i) {
                let mut parent = Some(index);
                while let Some(j) = parent {
                    filtered.layers[j].visible = true;
                    parent = d.layers[j].parent;
                }
                break;
            }
            if !d.layers[i].visible {
                break;
            }
            ancestor = d.layers[i].parent;
        }
    }
    Ok(Cow::Owned(filtered))
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetLayout {
    #[default]
    #[serde(alias = "Rows")]
    Rows,
    #[serde(alias = "Columns")]
    Columns,
    #[serde(alias = "Horizontal")]
    Horizontal,
    #[serde(alias = "Vertical")]
    Vertical,
    #[serde(alias = "Packed", alias = "MaxRects", alias = "max_rects")]
    Packed,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetOptions {
    pub scale: usize,
    pub columns: usize,
    pub rows: usize,
    pub layout: SheetLayout,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub padding: usize,
    pub border_padding: Option<usize>,
    pub shape_padding: Option<usize>,
    pub inner_padding: usize,
    pub trim: bool,
    pub trim_by_grid: bool,
    pub tag: Option<usize>,
    pub frame_range: Option<[usize; 2]>,
    pub layers: Option<Vec<usize>>,
    pub split_layers: bool,
    pub split_grid: bool,
    pub filename_template: String,
    pub tag_template: String,
    pub preset: String,
}

impl SheetOptions {
    pub const PRESETS: [&str; 4] = ["Generic", "Godot", "Unity", "Unity + Godot"];
}

impl Default for SheetOptions {
    fn default() -> Self {
        Self {
            scale: 1,
            columns: 8,
            rows: 8,
            layout: SheetLayout::Rows,
            width: None,
            height: None,
            padding: 1,
            border_padding: None,
            shape_padding: None,
            inner_padding: 0,
            trim: false,
            trim_by_grid: false,
            tag: None,
            frame_range: None,
            layers: None,
            split_layers: false,
            split_grid: false,
            filename_template: String::new(),
            tag_template: String::new(),
            preset: "Generic".into(),
        }
    }
}
impl SheetOptions {
    pub(crate) fn trims(&self) -> bool {
        self.trim || self.trim_by_grid
    }

    pub(crate) fn frames_for(&self, doc: &Document) -> Result<RangeInclusive<usize>, String> {
        let last = doc
            .durations
            .len()
            .checked_sub(1)
            .ok_or("Sprite has no frames")?;
        let [from, to] = self.frame_range.unwrap_or([0, last]);
        if from > to || to > last {
            return Err("Invalid frame range".into());
        }
        if let Some(index) = self.tag {
            let tag = doc.tags.get(index).ok_or("Unknown tag")?;
            let (from, to) = (from.max(tag.from), to.min(tag.to));
            if from > to {
                return Err("Frame range does not overlap the selected tag".into());
            }
            Ok(from..=to)
        } else {
            Ok(from..=to)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SheetSpacing {
    border: usize,
    gap: usize,
    inner: usize,
    extrude: usize,
}

impl SheetSpacing {
    pub(crate) fn new(options: &SheetOptions, extrude: usize) -> Result<Self, String> {
        let legacy = options.padding.min(256);
        let spacing = Self {
            border: options.border_padding.unwrap_or(legacy),
            gap: options.shape_padding.unwrap_or(legacy),
            inner: options.inner_padding,
            extrude,
        };
        if [spacing.border, spacing.gap, spacing.inner, extrude]
            .into_iter()
            .any(|value| value > 256)
        {
            return Err("Sheet padding and extrusion must be 0–256 pixels".into());
        }
        Ok(spacing)
    }

    fn inset(self) -> usize {
        self.inner + self.extrude
    }

    pub(crate) fn extent(self, sample_size: usize, samples: usize) -> usize {
        2 * self.border
            + samples * (sample_size + 2 * self.inset())
            + samples.saturating_sub(1) * self.gap
    }
}

pub fn pack_sheet(d: &Document, o: &SheetOptions) -> Result<(RgbaImage, Value), String> {
    pack_sheet_with_extrusion(d, o, 0)
}

pub(crate) fn sheet_frame_tag(tag: &FrameTag, from: usize, to: usize) -> Value {
    json!({
        "name": tag.name,
        "from": from,
        "to": to,
        "repeat": tag.repeat,
        "direction": match tag.direction {
            1 => "reverse",
            2 => "pingpong",
            3 => "pingpong_reverse",
            _ => "forward",
        },
    })
}

pub(crate) fn pack_sheet_with_extrusion(
    d: &Document,
    o: &SheetOptions,
    extrude: usize,
) -> Result<(RgbaImage, Value), String> {
    d.validate()?;
    if o.scale == 0 || o.scale > MAX_DIMENSION {
        return Err("Scale must be a positive integer within image limits".into());
    }
    let spacing = SheetSpacing::new(o, extrude)?;
    let range = o.frames_for(d)?;
    let sprite = filter_layers_for_export(d, o.layers.as_deref())?;
    let d = sprite.as_ref();
    if o.split_layers || o.split_grid {
        return pack_split_sheet(d, o, extrude, range, None);
    }
    let (first_frame, last_frame) = (*range.start(), *range.end());
    let count = last_frame - first_frame + 1;
    let trim = o.trims();
    if trim && count.saturating_mul(d.width * d.height) > MAX_DOCUMENT_PIXELS {
        return Err("Trimming this sprite sheet exceeds the pixel work limit; use a tag".into());
    }
    let bounds: Vec<_> = range
        .clone()
        .map(|frame| {
            if trim {
                visible_bounds(&d.composite(frame), d.width, d.height).map_or(
                    [0, 0, 1, 1],
                    |bounds| {
                        if o.trim_by_grid {
                            expand_bounds_to_grid(bounds, [d.width, d.height], d.drawing_grid())
                        } else {
                            bounds
                        }
                    },
                )
            } else {
                [0, 0, d.width, d.height]
            }
        })
        .collect();
    let sizes: Vec<_> = bounds.iter().map(|b| [b[2], b[3]]).collect();
    let layout = sheet_layout::place_samples(&sizes, o, spacing)?;
    let [width, height] = layout.dimensions;
    let mut im = RgbaImage::new(width as u32, height as u32);
    let mut frames = vec![];
    for ((f, b), [x, y]) in range.zip(bounds).zip(layout.positions) {
        let p = d.composite(f);
        for cy in 0..b[3] + 2 * extrude {
            let source_y = b[1] + cy.saturating_sub(extrude).min(b[3] - 1);
            for cx in 0..b[2] + 2 * extrude {
                let source_x = b[0] + cx.saturating_sub(extrude).min(b[2] - 1);
                im.put_pixel(
                    (x + cx - extrude) as u32,
                    (y + cy - extrude) as u32,
                    Rgba(p[source_y * d.width + source_x]),
                );
            }
        }
        let id = d.frame_export_id(f);
        let mut entry = json!({"filename":format!("frame_{id}"),"frameId":id,"frame":{"x":x,"y":y,"w":b[2],"h":b[3]},"rotated":false,"trimmed":trim,"spriteSourceSize":{"x":b[0],"y":b[1],"w":b[2],"h":b[3]},"sourceSize":{"w":d.width,"h":d.height},"duration":d.durations[f],"sourceFrame":f});
        if let Some(name) = d.frame_name(f) {
            entry["frameName"] = json!(name);
        }
        frames.push(entry);
    }
    let tags: Vec<Value> = d
        .tags
        .iter()
        .filter_map(|tag| {
            let from = tag.from.max(first_frame);
            let to = tag.to.min(last_frame);
            (from <= to).then(|| sheet_frame_tag(tag, from - first_frame, to - first_frame))
        })
        .collect();
    let scale = o.scale;
    let im = if scale == 1 {
        im
    } else {
        scale_image(&im, scale)?
    };
    for frame in &mut frames {
        for key in ["frame", "spriteSourceSize", "sourceSize"] {
            for value in frame[key].as_object_mut().unwrap().values_mut() {
                if let Some(n) = value.as_u64() {
                    *value = json!(n * scale as u64);
                }
            }
        }
    }
    let slices: Vec<_> = d
        .slices
        .iter()
        .filter_map(|slice| {
            // The final key at or before the range start remains active in the export.
            let start = slice
                .keys
                .partition_point(|key| key.frame <= first_frame)
                .saturating_sub(1);
            let scale_value = |value: i32| value.saturating_mul(scale as i32);
            let keys: Vec<_> = slice.keys[start..]
                .iter()
                .take_while(|key| key.frame <= last_frame)
                .map(|key| SliceKey {
                    frame: key.frame.saturating_sub(first_frame),
                    bounds: key.bounds.map(scale_value),
                    center: key.center.map(|rect| rect.map(scale_value)),
                    pivot: key.pivot.map(|point| point.map(scale_value)),
                })
                .collect();
            (!keys.is_empty()).then(|| SpriteSlice {
                name: slice.name.clone(),
                keys,
            })
        })
        .collect();
    let mut metadata = json!({"frames":frames,"meta":{"app":"PixelCraft","version":"1","image":"sprite.png","format":"RGBA8888","size":{"w":width*scale,"h":height*scale},"scale":scale.to_string(),"padding":spacing.gap*scale,"frameTags":tags,"slices":slices,"nextFrameId":d.next_frame_export_id()}});
    if o.border_padding.is_some() || o.shape_padding.is_some() || spacing.inner != 0 {
        metadata["meta"]["borderPadding"] = json!(spacing.border * scale);
        metadata["meta"]["shapePadding"] = json!(spacing.gap * scale);
        metadata["meta"]["innerPadding"] = json!(spacing.inner * scale);
    }
    if o.trim_by_grid {
        metadata["meta"]["trimByGrid"] = json!(true);
    }
    if o.layout != SheetLayout::Rows {
        metadata["meta"]["layout"] = json!(o.layout);
    }
    if extrude > 0 {
        metadata["meta"]["extrude"] = json!(extrude * scale);
    }
    Ok((im, metadata))
}

fn visible_bounds(pixels: &[Pixel], width: usize, height: usize) -> Option<[usize; 4]> {
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, _) in pixels.iter().enumerate().filter(|(_, pixel)| pixel[3] > 0) {
        let (x, y) = (index % width, index / width);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    (left < right && top < bottom).then(|| [left, top, right - left, bottom - top])
}

fn expand_bounds_to_grid(
    [x, y, width, height]: [usize; 4],
    [canvas_width, canvas_height]: [usize; 2],
    grid: DrawingGrid,
) -> [usize; 4] {
    let axis = |start, size, limit, origin: i32, step: u32| {
        let (origin, step) = (i64::from(origin), i64::from(step));
        let edge = |value: usize, round_up| {
            let offset = value as i64 - origin;
            let round = if round_up { step - 1 } else { 0 };
            origin + (offset + round).div_euclid(step) * step
        };
        let first = edge(start, false).max(0) as usize;
        let last = edge(start + size, true).min(limit as i64) as usize;
        (first, last - first)
    };
    let (x, width) = axis(x, width, canvas_width, grid.origin[0], grid.size[0]);
    let (y, height) = axis(y, height, canvas_height, grid.origin[1], grid.size[1]);
    [x, y, width, height]
}

pub fn unpack_sheet(im: &RgbaImage, meta: &Value) -> Result<Document, String> {
    unpack_sheet_samples(
        [im.width() as usize, im.height() as usize],
        meta,
        |x, y| im.get_pixel(x as u32, y as u32).0,
        None,
    )
}

pub(crate) fn unpack_png_sheet(sheet: &Document, meta: &Value) -> Result<Document, String> {
    unpack_sheet_samples(
        [sheet.width, sheet.height],
        meta,
        |x, y| sheet.layers[0].cels[0][y * sheet.width + x],
        Some(sheet),
    )
}

fn unpack_sheet_samples(
    [sheet_width, sheet_height]: [usize; 2],
    meta: &Value,
    pixel_at: impl Fn(usize, usize) -> Pixel,
    png: Option<&Document>,
) -> Result<Document, String> {
    let entries = &meta["frames"];
    let count = match entries {
        Value::Array(frames) => frames.len(),
        Value::Object(frames) => frames.len(),
        _ => return Err("Missing frames".into()),
    };
    if count > u16::MAX as usize {
        return Err("Sheet exceeds limits".into());
    }
    let frames: Vec<&Value> = if let Some(a) = meta["frames"].as_array() {
        a.iter().collect()
    } else if let Some(a) = meta["frames"].as_object() {
        // Foreign hashes use serialized order for tag indices. PixelCraft
        // atlases also provide explicit indices for older key-sorted files.
        let mut frames: Vec<_> = a.values().collect();
        if meta["meta"]["app"] == "PixelCraft" && meta["meta"]["sources"].is_array() {
            frames.sort_by_key(|frame| (frame["source"].as_u64(), frame["sourceFrame"].as_u64()));
        }
        frames
    } else {
        return Err("Missing frames".into());
    };
    let first = frames.first().ok_or("Empty sheet")?;
    let w = first["sourceSize"]["w"]
        .as_u64()
        .ok_or("Missing canvas width")? as usize;
    let h = first["sourceSize"]["h"]
        .as_u64()
        .ok_or("Missing canvas height")? as usize;
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h).saturating_mul(frames.len()) > MAX_PIXELS
    {
        return Err("Sheet exceeds limits".into());
    }
    let mut d = Document::new(w, h);
    let mut indexed = None;
    if let Some(png) = png {
        d.metadata.clone_from(&png.metadata);
        d.color_profile.clone_from(&png.color_profile);
        if png.color_mode == ColorMode::Grayscale {
            d.color_mode = ColorMode::Grayscale;
        } else if let Some(slots) = png.layers[0].cel_meta[0].indices.as_ref() {
            let padding = frames.iter().any(|frame| {
                frame["frame"]["w"].as_u64() != Some(w as u64)
                    || frame["frame"]["h"].as_u64() != Some(h as u64)
            });
            let mut palette = png.palette.clone();
            let mut clear = palette.iter().position(|color| color[3] == 0);
            if padding && clear.is_none() && palette.len() < 256 {
                clear = Some(palette.len());
                palette.push(CLEAR);
            }
            if !padding || clear.is_some() {
                let fill = clear.unwrap_or(0);
                indexed = Some((slots, fill as u8));
                d.color_mode = ColorMode::Indexed;
                d.palette = palette;
                d.transparent_index = fill;
                d.layers[0].background = clear.is_none();
            }
        }
    }
    for (f, v) in frames.iter().enumerate() {
        if v["rotated"] == true {
            return Err("Rotated atlas entries are unsupported".into());
        }
        if f > 0 {
            d.add_frame(f - 1, false)
        }
        d.durations[f] = v["duration"].as_u64().unwrap_or(100).clamp(1, 65535) as u16;
        if let Some(name) = v["frameName"].as_str() {
            d.set_frame_name(f, name)?;
        }
        let r = &v["frame"];
        let get = |key: &str| {
            r[key]
                .as_u64()
                .ok_or_else(|| format!("Missing frame {key}"))
        };
        let (x, y, rw, rh) = (get("x")?, get("y")?, get("w")?, get("h")?);
        let ox = v["spriteSourceSize"]["x"].as_u64().unwrap_or(0);
        let oy = v["spriteSourceSize"]["y"].as_u64().unwrap_or(0);
        if x.saturating_add(rw) > sheet_width as u64
            || y.saturating_add(rh) > sheet_height as u64
            || ox.saturating_add(rw) > w as u64
            || oy.saturating_add(rh) > h as u64
        {
            return Err("Frame is outside sheet or canvas".into());
        }
        for cy in 0..rh {
            for cx in 0..rw {
                d.layers[0].cels[f][(oy + cy) as usize * w + (ox + cx) as usize] =
                    pixel_at((x + cx) as usize, (y + cy) as usize);
            }
        }
        if let Some((slots, fill)) = indexed {
            let mut indices = vec![fill; w * h];
            for row in 0..rh as usize {
                let from = (y as usize + row) * sheet_width + x as usize;
                let to = (oy as usize + row) * w + ox as usize;
                let len = rw as usize;
                indices[to..to + len].copy_from_slice(&slots[from..from + len]);
            }
            d.layers[0].cel_meta[f].indices = Some(indices.into());
        }
    }
    // Atlas frame IDs belong to their source sprite. When importing multiple
    // sources as one timeline, keep the newly assigned document IDs.
    let multiple_sources = meta["meta"]["sources"]
        .as_array()
        .is_some_and(|sources| sources.len() > 1);
    if meta["meta"]["app"] == "PixelCraft"
        && !multiple_sources
        && frames.iter().any(|frame| frame.get("frameId").is_some())
    {
        let ids: Vec<_> = frames
            .iter()
            .map(|frame| {
                frame["frameId"]
                    .as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                    .ok_or("Invalid frame export ID")
            })
            .collect::<Result<_, _>>()?;
        let next = meta["meta"]
            .get("nextFrameId")
            .map(|value| value.as_u64().ok_or("Invalid next frame export ID"))
            .transpose()?
            .unwrap_or_else(|| u64::from(*ids.iter().max().unwrap()) + 1);
        d.set_sheet_frame_export_ids(ids, next);
    }
    if let Some(tags) = meta["meta"]["frameTags"].as_array() {
        for t in tags {
            d.tags.push(FrameTag {
                name: t["name"].as_str().unwrap_or("Animation").into(),
                from: t["from"].as_u64().unwrap_or(0) as usize,
                to: t["to"].as_u64().unwrap_or(0) as usize,
                direction: match t["direction"].as_str() {
                    Some("reverse") => 1,
                    Some("pingpong") => 2,
                    Some("pingpong_reverse") => 3,
                    _ => 0,
                },
                repeat: match t.get("repeat") {
                    None | Some(Value::Null) => 0,
                    Some(Value::String(repeat)) => {
                        repeat.parse().map_err(|_| "Invalid tag repeat")?
                    }
                    Some(repeat) => repeat
                        .as_u64()
                        .and_then(|repeat| u16::try_from(repeat).ok())
                        .ok_or("Invalid tag repeat")?,
                },
                color: [0; 3],
            });
        }
    }
    if let Some(slices) = meta["meta"].get("slices") {
        let mut slices = slices.clone();
        for slice in slices.as_array_mut().ok_or("Invalid slice metadata")? {
            let keys = slice
                .get_mut("keys")
                .and_then(Value::as_array_mut)
                .ok_or("Invalid slice metadata")?;
            for key in keys {
                for (field, coordinates) in [
                    ("bounds", ["x", "y", "w", "h"].as_slice()),
                    ("center", ["x", "y", "w", "h"].as_slice()),
                    ("pivot", ["x", "y"].as_slice()),
                ] {
                    if let Some(value @ Value::Object(_)) = key.get_mut(field) {
                        *value = Value::Array(
                            coordinates
                                .iter()
                                .map(|coordinate| value[*coordinate].take())
                                .collect(),
                        );
                    }
                }
            }
        }
        d.slices = serde_json::from_value(slices).map_err(|_| "Invalid slice metadata")?;
    }
    d.validate()?;
    Ok(d)
}

/// Transform a selection mask with the same nearest-neighbor mapping as its pixels.
pub fn transform_mask(d: &Document, b: [i32; 4], mask: &[bool], t: Transform) -> Vec<bool> {
    let [x, y, w, h] = b;
    if let Transform::SkewX(delta) | Transform::SkewY(delta) = t {
        let mut out = vec![false; d.width * d.height];
        if w <= 0 || h <= 0 || delta.unsigned_abs() > MAX_DIMENSION as u32 {
            return out;
        }
        let [nx, ny, nw, nh] = skew_bounds(b, t);
        for dy in ny.max(0)..(ny + nh).min(d.height as i32) {
            for dx in nx.max(0)..(nx + nw).min(d.width as i32) {
                let (sx, sy) = skew_source(b, [dx, dy], t);
                if (x..x + w).contains(&sx)
                    && (y..y + h).contains(&sy)
                    && let Some(source) = d.index(sx, sy)
                {
                    out[dy as usize * d.width + dx as usize] = mask.get(source) == Some(&true);
                }
            }
        }
        return out;
    }
    if let Transform::RotateAround { degrees, pivot } = t {
        let mut out = vec![false; d.width * d.height];
        if let Some(([nx, ny, nw, nh], basis)) = rotation_bounds(b, degrees, pivot) {
            for dy in ny.max(0)..(ny + nh).min(d.height as i32) {
                for dx in nx.max(0)..(nx + nw).min(d.width as i32) {
                    let (sx, sy) = rotated_source([dx, dy], pivot, basis);
                    if (x..x + w).contains(&sx)
                        && (y..y + h).contains(&sy)
                        && let Some(source) = d.index(sx, sy)
                    {
                        out[dy as usize * d.width + dx as usize] = mask.get(source) == Some(&true);
                    }
                }
            }
        }
        return out;
    }
    let (nw, nh, dx, dy) = match t {
        Transform::Move(dx, dy) => (w, h, dx, dy),
        Transform::Rotate => (h, w, 0, 0),
        Transform::Resize(w, h) => (w as i32, h as i32, 0, 0),
        _ => (w, h, 0, 0),
    };
    let mut out = vec![false; d.width * d.height];
    if nw <= 0 || nh <= 0 {
        return out;
    }
    for oy in 0..nh {
        for ox in 0..nw {
            let (sx, sy) = match t {
                Transform::Rotate => (oy, h - 1 - ox),
                Transform::FlipX => (w - 1 - ox, oy),
                Transform::FlipY => (ox, h - 1 - oy),
                _ => (ox * w / nw, oy * h / nh),
            };
            if let (Some(src), Some(dst)) =
                (d.index(x + sx, y + sy), d.index(x + dx + ox, y + dy + oy))
            {
                out[dst] = mask.get(src) == Some(&true);
            }
        }
    }
    out
}

/// Integer nearest-neighbor scaling, bounded before allocation.
pub fn scale_image(im: &RgbaImage, scale: usize) -> Result<RgbaImage, String> {
    let (w, h) = scaled_image_dimensions(im.width() as usize, im.height() as usize, scale)?;
    if scale == 1 {
        return Ok(im.clone());
    }
    Ok(image::imageops::resize(
        im,
        w as u32,
        h as u32,
        image::imageops::FilterType::Nearest,
    ))
}

fn scaled_image_dimensions(
    width: usize,
    height: usize,
    scale: usize,
) -> Result<(usize, usize), String> {
    let w = width.checked_mul(scale).ok_or("Export too large")?;
    let h = height.checked_mul(scale).ok_or("Export too large")?;
    if scale == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION || w.saturating_mul(h) > MAX_PIXELS {
        return Err("Scaled export exceeds image limits".into());
    }
    Ok((w, h))
}
/// Render a one-pixel outline or offset shadow from a stable source. Both the
/// effect source and its destination are clipped to the active selection.
pub fn spatial_effect(
    d: &mut Document,
    l: usize,
    f: usize,
    mask: &[bool],
    color: Pixel,
    offset: Option<(i32, i32)>,
) -> Result<(), String> {
    if let Some(shift) = offset {
        spatial_neighbors(d, l, f, mask, color, &[shift])
    } else {
        spatial_outline(d, l, f, mask, color, false)
    }
}

pub fn spatial_outline(
    d: &mut Document,
    l: usize,
    f: usize,
    mask: &[bool],
    color: Pixel,
    corners: bool,
) -> Result<(), String> {
    let shifts = [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
    ];
    spatial_neighbors(
        d,
        l,
        f,
        mask,
        color,
        if corners { &shifts } else { &shifts[..4] },
    )
}

fn spatial_neighbors(
    d: &mut Document,
    l: usize,
    f: usize,
    mask: &[bool],
    color: Pixel,
    shifts: &[(i32, i32)],
) -> Result<(), String> {
    if l >= d.layers.len()
        || f >= d.durations.len()
        || !d.editable(l)
        || d.layers[l].tileset.is_some()
        || mask.len() != d.width * d.height
    {
        return Err("Choose an unlocked image layer and valid selection".into());
    }
    let source = d.layers[l].cels[f].clone();
    let mut effect = vec![CLEAR; source.len()];
    for (i, p) in source.iter().enumerate() {
        if !mask[i] || p[3] == 0 {
            continue;
        }
        let (x, y) = ((i % d.width) as i32, (i / d.width) as i32);
        for &(dx, dy) in shifts {
            if let Some(j) = d.index(x + dx, y + dy)
                && mask[j]
            {
                let mut c = color;
                c[3] = ((color[3] as u16 * p[3] as u16 + 127) / 255) as u8;
                if c[3] > effect[j][3] {
                    effect[j] = c;
                }
            }
        }
    }
    for (i, c) in effect.into_iter().enumerate() {
        if c[3] > 0 {
            d.set_pixel(l, f, i, over(c, source[i], 255));
        }
    }
    Ok(())
}
