use crate::{
    model::*,
    productivity::{BrushMode, BrushOptions, paint_brush_with_ink},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Replace,
    SourceOver,
    Multiply,
    Screen,
}

fn default_mode() -> Mode {
    Mode::SourceOver
}
impl Mode {
    fn apply(self, previous: Pixel, mut source: Pixel, opacity: u8) -> Pixel {
        if matches!(self, Self::Replace) {
            source[3] = ((u16::from(source[3]) * u16::from(opacity) + 127) / 255) as u8;
            return source;
        }
        if source[3] == 0 || opacity == 0 {
            return previous;
        }
        match self {
            Self::SourceOver => over(previous, source, opacity),
            Self::Multiply => blend(previous, source, opacity, BlendMode::Multiply),
            Self::Screen => blend(previous, source, opacity, BlendMode::Screen),
            Self::Replace => unreachable!(),
        }
    }
}
fn one() -> i32 {
    1
}

#[derive(Deserialize)]
struct Region {
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

#[derive(Deserialize)]
struct Blit<'a> {
    #[serde(flatten)]
    region: Region,
    #[serde(default = "default_mode")]
    mode: Mode,
    data: &'a str,
    #[serde(default)]
    encoding: Encoding,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Encoding {
    #[default]
    Base64,
    Base64Zlib,
}

#[derive(Deserialize)]
struct Stroke {
    #[serde(default)]
    points: Vec<(i32, i32)>,
    color: Pixel,
    #[serde(default = "one")]
    size: i32,
    #[serde(default = "one")]
    spacing: i32,
    #[serde(default)]
    placements: bool,
    #[serde(default)]
    scatter: i32,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    ramp: Vec<Pixel>,
    stamp: Option<Stamp>,
    alternate: Option<Pixel>,
    #[serde(default)]
    shade: bool,
    #[serde(default)]
    reverse: bool,
    brush_shape: Option<BrushShape>,
    mode: Option<Mode>,
    opacity: Option<u8>,
    end_size: Option<i32>,
    pressure: Option<Vec<u8>>,
    mask: Option<StrokeMask>,
}

#[derive(Deserialize)]
struct StrokeMask {
    #[serde(flatten)]
    region: Region,
    data: String,
}

impl Region {
    fn clipped(&self, doc: &Document) -> (usize, usize, usize, usize) {
        let x = i64::from(self.x);
        let y = i64::from(self.y);
        let bound = |value: i64, max: usize| value.clamp(0, max as i64) as usize;
        (
            bound(x, doc.width),
            bound(y, doc.height),
            bound(x + self.width as i64, doc.width),
            bound(y + self.height as i64, doc.height),
        )
    }

    fn offset(&self, x: usize, y: usize) -> usize {
        (y as i64 - i64::from(self.y)) as usize * self.width
            + (x as i64 - i64::from(self.x)) as usize
    }
}

fn decode_mask(data: &str, count: usize) -> Result<Vec<u8>, String> {
    let bytes = count.div_ceil(8);
    if data.len() != bytes.div_ceil(3) * 4 {
        return Err("Invalid region mask length".into());
    }
    let mask = STANDARD
        .decode(data)
        .map_err(|_| "Invalid region mask base64")?;
    if mask.len() != bytes {
        return Err("Invalid region mask length".into());
    }
    Ok(mask)
}

fn mask_contains(mask: &[u8], bit: usize) -> bool {
    mask[bit / 8] & (1 << (bit % 8)) != 0
}

#[derive(Deserialize)]
struct Stamp {
    width: usize,
    height: usize,
    data: String,
    #[serde(default)]
    encoding: Encoding,
    #[serde(default = "one")]
    scale: i32,
}

#[derive(Deserialize)]
struct RegionEdit {
    #[serde(flatten)]
    region: Region,
    to_x: Option<i32>,
    to_y: Option<i32>,
    from_layer: Option<usize>,
    from_frame: Option<usize>,
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
    #[serde(default)]
    skip_transparent: bool,
    opacity: Option<u8>,
    #[serde(default)]
    colors: Vec<(Pixel, Pixel)>,
    mask: Option<String>,
}

#[derive(Deserialize)]
struct Polygon {
    points: Vec<(i32, i32)>,
    color: Pixel,
    alternate: Option<Pixel>,
    dither_level: Option<u8>,
    #[serde(default)]
    edge_noise: u8,
    #[serde(default)]
    seed: u64,
}

fn charge(budget: &mut usize, count: usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(count)
        .ok_or("Paint batch exceeds its work limit")?;
    Ok(())
}

fn rgba(data: &str, encoding: Encoding, count: usize) -> Result<Vec<Pixel>, String> {
    let bytes = count * 4;
    if data.len() > (bytes + 1024).div_ceil(3) * 4 {
        return Err("Raster payload exceeds its size limit".into());
    }
    let packed = STANDARD
        .decode(data)
        .map_err(|_| "Invalid base64 raster payload")?;
    let raw = match encoding {
        Encoding::Base64 => packed,
        Encoding::Base64Zlib => {
            let mut decoder = flate2::Decompress::new(true);
            let mut raw = vec![0; bytes + 1];
            let status = decoder
                .decompress(&packed, &mut raw, flate2::FlushDecompress::Finish)
                .map_err(|_| "Invalid zlib raster payload")?;
            if status != flate2::Status::StreamEnd
                || decoder.total_in() != packed.len() as u64
                || decoder.total_out() != bytes as u64
            {
                return Err("Invalid zlib raster length or stream terminator".into());
            }
            raw.truncate(bytes);
            raw
        }
    };
    if raw.len() != bytes {
        return Err("Raster byte count must equal width × height × 4".into());
    }
    Ok(raw.as_chunks::<4>().0.to_vec())
}

pub(crate) fn paint(
    doc: &mut Document,
    layer: usize,
    frame: usize,
    op: &str,
    args: &Value,
    budget: &mut usize,
) -> Result<(), String> {
    if doc.color_mode != ColorMode::Rgba
        || !doc.editable(layer)
        || doc.layers[layer].tileset.is_some()
    {
        return Err("Bulk paint requires an unlocked RGBA image layer".into());
    }
    if op == "stroke" {
        let stroke: Stroke = Stroke::deserialize(args).map_err(|e| e.to_string())?;
        return paint_stroke(doc, layer, frame, stroke, budget);
    }
    if op == "polygon" {
        let polygon = Polygon::deserialize(args).map_err(|e| e.to_string())?;
        return paint_polygon(doc, layer, frame, polygon, budget);
    }
    if matches!(op, "copy-region" | "move-region" | "recolor-region") {
        let edit = RegionEdit::deserialize(args).map_err(|e| e.to_string())?;
        return edit_region(doc, layer, frame, op, edit, budget);
    }
    if op == "clear-region" {
        let edit = RegionEdit::deserialize(args).map_err(|e| e.to_string())?;
        let region = edit.region;
        check_size(&region, MAX_DIMENSION)?;
        let (x, y, right, bottom) = region.clipped(doc);
        charge(budget, (right - x) * (bottom - y))?;
        let mask = edit
            .mask
            .as_deref()
            .map(|data| decode_mask(data, region.width * region.height))
            .transpose()?;
        let stride = doc.width;
        let cel = &mut doc.layers[layer].cels[frame];
        for row in y..bottom {
            for col in x..right {
                let pixel = &mut cel[row * stride + col];
                if (!edit.skip_transparent || pixel[3] != 0)
                    && mask
                        .as_ref()
                        .is_none_or(|m| mask_contains(m, region.offset(col, row)))
                {
                    *pixel = CLEAR;
                }
            }
        }
        return Ok(());
    }
    let blit = Blit::deserialize(args).map_err(|e| e.to_string())?;
    let region = blit.region;
    check_size(&region, 1024)?;
    let count = region.width * region.height;
    charge(budget, count)?;
    let pixels = rgba(blit.data, blit.encoding, count)?;
    let (x, y, right, bottom) = region.clipped(doc);
    let stride = doc.width;
    let cel = &mut doc.layers[layer].cels[frame];
    for row in y..bottom {
        for col in x..right {
            let i = row * stride + col;
            let next = blit
                .mode
                .apply(cel[i], pixels[region.offset(col, row)], 255);
            if next != cel[i] {
                cel[i] = next;
            }
        }
    }
    Ok(())
}

fn edit_region(
    doc: &mut Document,
    layer: usize,
    frame: usize,
    op: &str,
    edit: RegionEdit,
    budget: &mut usize,
) -> Result<(), String> {
    let r = edit.region;
    check_size(&r, MAX_DIMENSION)?;
    let x = usize::try_from(r.x).map_err(|_| "Source region must be inside the canvas")?;
    let y = usize::try_from(r.y).map_err(|_| "Source region must be inside the canvas")?;
    if x + r.width > doc.width || y + r.height > doc.height || edit.colors.len() > 256 {
        return Err("Source region or color map exceeds its bounds".into());
    }
    charge(budget, r.width * r.height)?;
    let mask = edit
        .mask
        .as_deref()
        .map(|data| decode_mask(data, r.width * r.height))
        .transpose()?;
    let selected = |col: usize, row: usize| {
        let bit = row * r.width + col;
        mask.as_ref().is_none_or(|m| mask_contains(m, bit))
    };
    let stride = doc.width;
    let colors: std::collections::HashMap<_, _> = edit.colors.into_iter().collect();
    let map = |p: Pixel| {
        let mut p = colors.get(&p).copied().unwrap_or(p);
        if let Some(opacity) = edit.opacity {
            p[3] = ((u16::from(p[3]) * u16::from(opacity) + 127) / 255) as u8;
        }
        p
    };
    if op == "recolor-region" {
        let cel = &mut doc.layers[layer].cels[frame];
        for row in y..y + r.height {
            for col in x..x + r.width {
                let i = row * stride + col;
                if selected(col - x, row - y) && (!edit.skip_transparent || cel[i][3] != 0) {
                    cel[i] = map(cel[i]);
                }
            }
        }
        return Ok(());
    }
    let from_layer = edit.from_layer.unwrap_or(layer);
    let from_frame = edit.from_frame.unwrap_or(frame);
    let source_layer = doc
        .layers
        .get(from_layer)
        .filter(|l| !l.group && l.tileset.is_none())
        .ok_or("Invalid source image layer")?;
    let source = source_layer
        .cels
        .get(from_frame)
        .ok_or("Invalid source frame")?
        .clone();
    let dest_x = i64::from(edit.to_x.ok_or("Missing to_x")?);
    let dest_y = i64::from(edit.to_y.ok_or("Missing to_y")?);
    if op == "move-region" {
        if !doc.editable(from_layer) {
            return Err("Source layer is locked".into());
        }
        let cel = &mut doc.layers[from_layer].cels[from_frame];
        for row in 0..r.height {
            for col in 0..r.width {
                let i = (y + row) * stride + x + col;
                if selected(col, row) && (!edit.skip_transparent || cel[i][3] != 0) {
                    cel[i] = CLEAR;
                }
            }
        }
    }
    let height = doc.height as i64;
    let cel = &mut doc.layers[layer].cels[frame];
    for row in 0..r.height {
        let dy = dest_y + row as i64;
        if !(0..height).contains(&dy) {
            continue;
        }
        let sy = y + if edit.flip_y { r.height - 1 - row } else { row };
        for col in 0..r.width {
            let dx = dest_x + col as i64;
            if !(0..stride as i64).contains(&dx) {
                continue;
            }
            let sx = x + if edit.flip_x { r.width - 1 - col } else { col };
            let p = source[sy * stride + sx];
            if selected(sx - x, sy - y) && (!edit.skip_transparent || p[3] != 0) {
                cel[dy as usize * stride + dx as usize] = map(p);
            }
        }
    }
    Ok(())
}

fn paint_polygon(
    doc: &mut Document,
    layer: usize,
    frame: usize,
    polygon: Polygon,
    budget: &mut usize,
) -> Result<(), String> {
    let points = &polygon.points;
    if polygon.edge_noise > 16 {
        return Err("Polygon edge noise must be from 0 to 16 pixels".into());
    }
    if polygon
        .dither_level
        .is_some_and(|n| n > 16 || polygon.alternate.is_none())
    {
        return Err("Ordered dither requires alternate and a level from 0 to 16".into());
    }
    if !(3..=256).contains(&points.len())
        || points.iter().any(|&(x, y)| {
            x.unsigned_abs() > 2 * MAX_DIMENSION as u32
                || y.unsigned_abs() > 2 * MAX_DIMENSION as u32
        })
    {
        return Err("Polygon needs 3–256 bounded vertices".into());
    }
    let x = points
        .iter()
        .map(|p| p.0)
        .min()
        .unwrap()
        .clamp(0, doc.width as i32);
    let y = points
        .iter()
        .map(|p| p.1)
        .min()
        .unwrap()
        .clamp(0, doc.height as i32);
    let right = points
        .iter()
        .map(|p| p.0)
        .max()
        .unwrap()
        .clamp(x, doc.width as i32);
    let bottom = points
        .iter()
        .map(|p| p.1)
        .max()
        .unwrap()
        .clamp(y, doc.height as i32);
    charge(
        budget,
        ((right - x) as usize * (bottom - y) as usize)
            .saturating_mul(points.len())
            .saturating_mul(if polygon.edge_noise == 0 { 1 } else { 2 }),
    )?;
    let stride = doc.width;
    let cel = &mut doc.layers[layer].cels[frame];
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    for row in y..bottom {
        for col in x..right {
            if crate::workflows::inside_polygon(points, col, row)
                && (polygon.edge_noise == 0 || keep_noisy_pixel(&polygon, col, row))
            {
                let alternate = polygon.dither_level.map_or((col + row) % 2 == 1, |level| {
                    BAYER[row as usize % 4][col as usize % 4] < level
                });
                let color = if alternate {
                    polygon.alternate.unwrap_or(polygon.color)
                } else {
                    polygon.color
                };
                cel[row as usize * stride + col as usize] = color;
            }
        }
    }
    Ok(())
}

fn mix_seed(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn keep_noisy_pixel(polygon: &Polygon, x: i32, y: i32) -> bool {
    let coord = |x: i32, y: i32| (u64::from(x as u32) << 32) | u64::from(y as u32);
    let cluster_size = i32::from(polygon.edge_noise).div_euclid(2) + 1;
    let coarse = mix_seed(polygon.seed ^ coord(x / cluster_size, y / cluster_size));
    let fine = mix_seed(polygon.seed.wrapping_add(1) ^ coord(x, y));
    let depth = f64::from(polygon.edge_noise) * (3 * (coarse & 255) + (fine & 255)) as f64 / 1020.0;
    let center = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    polygon
        .points
        .iter()
        .zip(polygon.points.iter().cycle().skip(1))
        .all(|(&(ax, ay), &(bx, by))| {
            let dx = f64::from(bx - ax);
            let dy = f64::from(by - ay);
            let x = center.0 - f64::from(ax);
            let y = center.1 - f64::from(ay);
            let length = dx * dx + dy * dy;
            let t = if length == 0.0 {
                0.0
            } else {
                ((x * dx + y * dy) / length).clamp(0.0, 1.0)
            };
            (x - t * dx).powi(2) + (y - t * dy).powi(2) >= depth * depth
        })
}

fn check_size(region: &Region, max: usize) -> Result<(), String> {
    if region.width == 0 || region.height == 0 || region.width > max || region.height > max {
        return Err("Invalid raster dimensions".into());
    }
    Ok(())
}

fn paint_stroke(
    doc: &mut Document,
    layer: usize,
    frame: usize,
    stroke: Stroke,
    budget: &mut usize,
) -> Result<(), String> {
    if (stroke.points.is_empty() && stroke.mask.is_none())
        || stroke.points.len() > 16384
        || !(1..=MAX_BRUSH_SIZE).contains(&stroke.size)
        || stroke
            .end_size
            .is_some_and(|n| !(1..=MAX_BRUSH_SIZE).contains(&n) || stroke.stamp.is_some())
        || !(1..=MAX_DIMENSION as i32).contains(&stroke.spacing)
        || !(0..=64).contains(&stroke.scatter)
        || stroke.ramp.len() > 256
        || stroke.points.iter().any(|(x, y)| {
            x.unsigned_abs() > 2 * MAX_DIMENSION as u32
                || y.unsigned_abs() > 2 * MAX_DIMENSION as u32
        })
    {
        return Err("Invalid stroke points, size, spacing, scatter or ramp".into());
    }
    if stroke.pressure.as_ref().is_some_and(|pressure| {
        pressure.len() != stroke.points.len() || stroke.points.is_empty() || stroke.stamp.is_some()
    }) {
        return Err("Pressure requires one 0-255 value per point and a non-bitmap brush".into());
    }
    let stroke_mask = stroke
        .mask
        .as_ref()
        .map(|mask| {
            check_size(&mask.region, MAX_DIMENSION)?;
            decode_mask(&mask.data, mask.region.width * mask.region.height)
        })
        .transpose()?;
    let mask_contains_pixel = |x: usize, y: usize| {
        let (Some(mask), Some(data)) = (&stroke.mask, &stroke_mask) else {
            return true;
        };
        let col = x as i64 - i64::from(mask.region.x);
        let row = y as i64 - i64::from(mask.region.y);
        col >= 0
            && row >= 0
            && col < mask.region.width as i64
            && row < mask.region.height as i64
            && mask_contains(data, mask.region.offset(x, y))
    };
    let mode = match (&stroke.stamp, stroke.shade, stroke.alternate) {
        (Some(_), false, None) => BrushMode::Selection,
        (None, true, None) => BrushMode::Shade,
        (None, false, Some(_)) => BrushMode::Dither,
        (None, false, None) => BrushMode::Normal,
        _ => return Err("Choose one of stamp, shade or alternate color".into()),
    };
    let stamp = stroke
        .stamp
        .map(|stamp| {
            if !(1..=256).contains(&stamp.width)
                || !(1..=256).contains(&stamp.height)
                || !(1..=16).contains(&stamp.scale)
            {
                return Err("Invalid stamp dimensions".to_string());
            }
            let pixels = rgba(&stamp.data, stamp.encoding, stamp.width * stamp.height)?;
            let scale = stamp.scale as usize;
            let (width, height) = (stamp.width * scale, stamp.height * scale);
            if width > 256 || height > 256 {
                return Err("Scaled stamp must fit 256 × 256 pixels".into());
            }
            let scaled = (0..width * height)
                .map(|i| pixels[(i / width / scale) * stamp.width + (i % width / scale)])
                .collect();
            Ok((width, height, scaled))
        })
        .transpose()?;
    let end_size = stroke.end_size.unwrap_or(stroke.size);
    let total_steps = if stroke.placements {
        stroke.points.len().saturating_sub(1)
    } else {
        let distance = stroke
            .points
            .windows(2)
            .map(|p| p[0].0.abs_diff(p[1].0).max(p[0].1.abs_diff(p[1].1)) as usize)
            .sum();
        charge(budget, distance)?;
        distance
    };
    let ink_mode = stroke.mode.unwrap_or(if stamp.is_some() {
        Mode::SourceOver
    } else {
        Mode::Replace
    });
    let opacity = stroke.opacity.unwrap_or(255);
    let options = BrushOptions {
        mode,
        alternate: stroke.alternate.unwrap_or(CLEAR),
        reverse: stroke.reverse,
        stamp,
        mask: None,
    };
    let shape = doc.brush_shape;
    let wrap = doc.wraparound;
    doc.brush_shape = stroke.brush_shape.unwrap_or(shape);
    doc.wraparound = false;
    let mut state = stroke.seed;
    let mut random = || {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        mix_seed(state)
    };
    let mut shade_seen = std::collections::HashSet::new();
    let stride = doc.width;
    let mask_bounds = stroke.mask.as_ref().map(|m| m.region.clipped(doc));
    if let Some((x, y, right, bottom)) = mask_bounds.filter(|_| stroke.points.is_empty()) {
        let spacing = stroke.spacing as usize;
        charge(
            budget,
            (right - x).div_ceil(spacing) * (bottom - y).div_ceil(spacing),
        )?;
    }
    let mut dab = |(x, y), step: usize, pressure: i32| {
        let size = stroke.size + (end_size - stroke.size) * step as i32 / total_steps.max(1) as i32;
        let size = ((size * pressure + 127) / 255).max(1);
        let dab_cost = options
            .stamp
            .as_ref()
            .map_or((size * size) as usize, |(w, h, _)| w * h);
        charge(budget, dab_cost)?;
        let spread = (2 * stroke.scatter + 1) as u64;
        let x = x + (random() % spread) as i32 - stroke.scatter;
        let y = y + (random() % spread) as i32 - stroke.scatter;
        let color = if stroke.ramp.is_empty() {
            stroke.color
        } else {
            stroke.ramp[(random() % stroke.ramp.len() as u64) as usize]
        };
        paint_brush_with_ink(
            doc,
            (layer, frame),
            (x, y),
            color,
            size,
            &options,
            |index, dst, src| {
                if !mask_contains_pixel(index % stride, index / stride) {
                    return dst;
                }
                let next = ink_mode.apply(dst, src, opacity);
                if mode == BrushMode::Shade && next != dst && !shade_seen.insert(index) {
                    dst
                } else {
                    next
                }
            },
        );
        Ok::<_, String>(())
    };
    let pressure_at = |i: usize| stroke.pressure.as_ref().map_or(255, |p| i32::from(p[i]));
    if stroke.points.is_empty() {
        let (x, y, right, bottom) = mask_bounds.expect("mask exists");
        let spacing = stroke.spacing as usize;
        for row in (y..bottom).step_by(spacing) {
            for col in (x..right).step_by(spacing) {
                if mask_contains_pixel(col, row) {
                    dab((col as i32, row as i32), 0, 255)?;
                }
            }
        }
    } else if stroke.placements {
        for (step, &point) in stroke.points.iter().enumerate() {
            dab(point, step, pressure_at(step))?;
        }
    } else {
        let mut step = 0;
        dab(stroke.points[0], 0, pressure_at(0))?;
        for (index, pair) in stroke.points.windows(2).enumerate() {
            let start = step;
            let length = pair[0]
                .0
                .abs_diff(pair[1].0)
                .max(pair[0].1.abs_diff(pair[1].1)) as i32;
            crate::model::line_pixels(pair[0], pair[1], |point| {
                if point != pair[0] {
                    step += 1;
                    if step % stroke.spacing as usize == 0 {
                        let pressure = pressure_at(index)
                            + (pressure_at(index + 1) - pressure_at(index)) * (step - start) as i32
                                / length;
                        dab(point, step, pressure)?;
                    }
                }
                Ok::<_, String>(())
            })?;
        }
    }
    doc.brush_shape = shape;
    doc.wraparound = wrap;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn apply(doc: &Document, command: Value) -> Result<Document, String> {
        crate::automation::apply_command_batch(doc, &json!({"commands":[command]}))
    }

    #[test]
    fn masked_stamps_respect_holes_and_masked_clear_leaves_other_pixels() {
        let mut doc = Document::new(5, 1);
        doc.set_pixel(0, 0, 4, [0, 0, 255, 255]);
        let result = apply(
            &doc,
            json!({"op":"stroke","layer":0,"frame":0,
            "color":[255,0,0,255],"stamp":{"width":3,"height":1,"data":"/wAA//8AAP//AAD/"},
            "mask":{"x":1,"y":0,"width":3,"height":1,"data":"BQ=="}}),
        )
        .unwrap();
        assert_eq!(
            &result.layers[0].cels[0][..],
            &[
                CLEAR,
                [255, 0, 0, 255],
                CLEAR,
                [255, 0, 0, 255],
                [0, 0, 255, 255]
            ]
        );
        let result = apply(
            &result,
            json!({"op":"clear-region","layer":0,"frame":0,
            "x":1,"y":0,"width":4,"height":1,"mask":"AQ=="}),
        )
        .unwrap();
        assert_eq!(
            &result.layers[0].cels[0][..],
            &[CLEAR, CLEAR, CLEAR, [255, 0, 0, 255], [0, 0, 255, 255]]
        );
    }

    #[test]
    fn stroke_pressure_interpolates_between_path_vertices() {
        let doc = Document::new(15, 9);
        let result = apply(
            &doc,
            json!({"op":"stroke","layer":0,"frame":0,
            "points":[[3,4],[11,4]],"pressure":[0,255],"size":5,"brush_shape":"Square",
            "color":[255,0,0,255]}),
        )
        .unwrap();
        let pixels = &result.layers[0].cels[0];
        assert_eq!(pixels[3 * 15 + 3], CLEAR);
        assert_eq!(pixels[3 * 15 + 7], [255, 0, 0, 255]);
        assert_eq!(pixels[2 * 15 + 11], [255, 0, 0, 255]);
    }

    #[test]
    fn polygon_edge_noise_is_seeded_and_preserves_the_interior() {
        let doc = Document::new(20, 20);
        let command = |seed| {
            json!({"op":"polygon","layer":0,"frame":0,
            "points":[[1,1],[19,1],[19,19],[1,19]],"color":[255,0,0,255],
            "edge_noise":3,"seed":seed})
        };
        let first = apply(&doc, command(11)).unwrap();
        assert_eq!(first, apply(&doc, command(11)).unwrap());
        assert!(first.layers[0].cels[0] != apply(&doc, command(12)).unwrap().layers[0].cels[0]);
        let pixels = &first.layers[0].cels[0];
        assert_eq!(pixels[10 * 20 + 10], [255, 0, 0, 255]);
        assert!(pixels[21..39].contains(&CLEAR));
        assert!(pixels[21..39].contains(&[255, 0, 0, 255]));
    }
}
