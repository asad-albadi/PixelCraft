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
    Ok(raw
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect())
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
    let (region, patch) = if op == "blit" {
        let blit = Blit::deserialize(args).map_err(|e| e.to_string())?;
        check_size(&blit.region, 1024)?;
        let count = blit.region.width * blit.region.height;
        charge(budget, count)?;
        let pixels = rgba(blit.data, blit.encoding, count)?;
        (blit.region, Some((blit.mode, pixels)))
    } else {
        let region = Region::deserialize(args).map_err(|e| e.to_string())?;
        check_size(&region, MAX_DIMENSION)?;
        (region, None)
    };
    let left = i64::from(region.x);
    let top = i64::from(region.y);
    let x = (left.max(0) as usize).min(doc.width);
    let y = (top.max(0) as usize).min(doc.height);
    let right = (left + region.width as i64).clamp(x as i64, doc.width as i64) as usize;
    let bottom = (top + region.height as i64).clamp(y as i64, doc.height as i64) as usize;
    if patch.is_none() {
        charge(budget, (right - x) * (bottom - y))?;
    }
    let stride = doc.width;
    let cel = &mut doc.layers[layer].cels[frame];
    for row in y..bottom {
        for col in x..right {
            let i = row * stride + col;
            let next = if let Some((mode, pixels)) = &patch {
                let source = pixels
                    [(row as i64 - top) as usize * region.width + (col as i64 - left) as usize];
                mode.apply(cel[i], source, 255)
            } else {
                CLEAR
            };
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
        .map(|data| {
            let bytes = (r.width * r.height).div_ceil(8);
            if data.len() != bytes.div_ceil(3) * 4 {
                return Err("Invalid region mask length");
            }
            let mask = STANDARD
                .decode(data)
                .map_err(|_| "Invalid region mask base64")?;
            if mask.len() != bytes {
                return Err("Invalid region mask length");
            }
            Ok(mask)
        })
        .transpose()?;
    let selected = |col: usize, row: usize| {
        let bit = row * r.width + col;
        mask.as_ref()
            .is_none_or(|m| m[bit / 8] & (1 << (bit % 8)) != 0)
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
        ((right - x) as usize * (bottom - y) as usize).saturating_mul(points.len()),
    )?;
    let stride = doc.width;
    let cel = &mut doc.layers[layer].cels[frame];
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    for row in y..bottom {
        for col in x..right {
            if crate::workflows::inside_polygon(points, col, row) {
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
    if stroke.points.is_empty()
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
        stroke.points.len() - 1
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
        let n = (state ^ (state >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        let n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        n ^ (n >> 31)
    };
    let mut shade_seen = std::collections::HashSet::new();
    let mut dab = |(x, y), step: usize| {
        let size = stroke.size + (end_size - stroke.size) * step as i32 / total_steps.max(1) as i32;
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
            layer,
            frame,
            (x, y),
            color,
            size,
            &options,
            |index, dst, src| {
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
    if stroke.placements {
        for (step, point) in stroke.points.into_iter().enumerate() {
            dab(point, step)?;
        }
    } else {
        let mut step = 0;
        dab(stroke.points[0], 0)?;
        for pair in stroke.points.windows(2) {
            crate::model::line_pixels(pair[0], pair[1], |point| {
                if point != pair[0] {
                    step += 1;
                    if step % stroke.spacing as usize == 0 {
                        dab(point, step)?;
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
