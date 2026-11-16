use crate::model::*;
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum Transform {
    Move(i32, i32),
    Resize(usize, usize),
    Rotate,
    RotateAround { degrees: f64, pivot: [f64; 2] },
    FlipX,
    FlipY,
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
    let (sin, cos) = degrees.rem_euclid(360.).to_radians().sin_cos();
    let snap = |value: f64| {
        if (value - value.round()).abs() < 1e-9 {
            value.round()
        } else {
            value
        }
    };
    let [sin, cos] = [snap(sin), snap(cos)];
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
    let origin = min.map(|v| snap(v).floor() as i32);
    let end = max.map(|v| snap(v).ceil() as i32);
    Some((
        [origin[0], origin[1], end[0] - origin[0], end[1] - origin[1]],
        [sin, cos],
    ))
}

fn rotated_source(position: [i32; 2], pivot: [f64; 2], [sin, cos]: [f64; 2]) -> (i32, i32) {
    let x = f64::from(position[0]) + 0.5 - pivot[0];
    let y = f64::from(position[1]) + 0.5 - pivot[1];
    (
        (pivot[0] + x * cos + y * sin).floor() as i32,
        (pivot[1] - x * sin + y * cos).floor() as i32,
    )
}

pub fn transform(
    d: &mut Document,
    l: usize,
    f: usize,
    b: [i32; 4],
    mask: Option<&[bool]>,
    t: Transform,
) -> Result<[i32; 4], String> {
    if !d.editable(l) || d.layers[l].tileset.is_some() {
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
    let selected = |sx: i32, sy: i32| {
        d.index(sx, sy)
            .is_some_and(|i| mask.is_none_or(|m| m.get(i) == Some(&true)))
    };
    let mut writes = Vec::new();
    for oy in 0..nh {
        for ox in 0..nw {
            let (sx, sy) = match t {
                Transform::RotateAround { .. } => {
                    let (pivot, basis) = rotation.unwrap();
                    let (sx, sy) =
                        rotated_source([x + dx + ox as i32, y + dy + oy as i32], pivot, basis);
                    if !(x..x + w).contains(&sx) || !(y..y + h).contains(&sy) {
                        continue;
                    }
                    (sx - x, sy - y)
                }
                Transform::Rotate => (oy as i32, h - 1 - ox as i32),
                Transform::FlipX => (w - 1 - ox as i32, oy as i32),
                Transform::FlipY => (ox as i32, h - 1 - oy as i32),
                _ => (ox as i32 * w / nw as i32, oy as i32 * h / nh as i32),
            };
            if selected(x + sx, y + sy)
                && let Some(dst) = d.index(x + dx + ox as i32, y + dy + oy as i32)
            {
                let src = d.index(x + sx, y + sy).unwrap();
                writes.push((dst, source[src], source_indices.as_ref().map(|v| v[src])));
            }
        }
    }
    for sy in y..y + h {
        for sx in x..x + w {
            if let Some(i) = d.index(sx, sy)
                && mask.is_none_or(|m| m.get(i) == Some(&true))
            {
                d.set_pixel(l, f, i, CLEAR);
            }
        }
    }
    for (i, p, index) in writes {
        d.drawing_index = index.map(|n| n as usize);
        d.set_pixel(l, f, i, p);
    }
    d.drawing_index = None;
    Ok([x + dx, y + dy, nw as i32, nh as i32])
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
    let n = n.clamp(2, 256);
    (0..n)
        .map(|i| {
            std::array::from_fn(|c| {
                ((a[c] as f32 * (n - 1 - i) as f32 + b[c] as f32 * i as f32) / (n - 1) as f32)
                    .round() as u8
            })
        })
        .collect()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SheetOptions {
    pub scale: usize,
    pub columns: usize,
    pub padding: usize,
    pub trim: bool,
    pub tag: Option<usize>,
    pub preset: String,
}
impl Default for SheetOptions {
    fn default() -> Self {
        Self {
            scale: 1,
            columns: 8,
            padding: 1,
            trim: false,
            tag: None,
            preset: "Generic".into(),
        }
    }
}
pub fn pack_sheet(d: &Document, o: &SheetOptions) -> Result<(RgbaImage, Value), String> {
    d.validate()?;
    if o.scale == 0 || o.scale > MAX_DIMENSION {
        return Err("Scale must be a positive integer within image limits".into());
    }
    let range = if let Some(i) = o.tag {
        let t = d.tags.get(i).ok_or("Unknown tag")?;
        t.from..=t.to
    } else {
        0..=d.durations.len() - 1
    };
    let (first_frame, last_frame) = (*range.start(), *range.end());
    let count = last_frame - first_frame + 1;
    let cols = o.columns.clamp(1, count);
    let pad = o.padding.min(256);
    if o.trim && count.saturating_mul(d.width * d.height) > MAX_DOCUMENT_PIXELS {
        return Err("Trimming this sprite sheet exceeds the pixel work limit; use a tag".into());
    }
    let mut sprites = Vec::with_capacity(count);
    let (mut x, mut y, mut row, mut width) = (pad, pad, 0, 0);
    for (i, frame) in range.enumerate() {
        if i > 0 && i % cols == 0 {
            x = pad;
            y += row + pad;
            row = 0;
        }
        let b = if o.trim {
            visible_bounds(&d.composite(frame), d.width, d.height)
        } else {
            [0, 0, d.width, d.height]
        };
        sprites.push((frame, (x, y), b));
        width = width.max(x + b[2] + pad);
        x += b[2] + pad;
        row = row.max(b[3]);
        let height = y + row + pad;
        if width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || width.saturating_mul(height) > MAX_PIXELS
        {
            return Err("Sprite sheet exceeds image limits; use more rows or a tag".into());
        }
        scaled_image_dimensions(width, height, o.scale)?;
    }
    let height = y + row + pad;
    let mut im = RgbaImage::new(width as u32, height as u32);
    let mut frames = vec![];
    for &(f, (x, y), b) in &sprites {
        let p = d.composite(f);
        for cy in 0..b[3] {
            for cx in 0..b[2] {
                im.put_pixel(
                    (x + cx) as u32,
                    (y + cy) as u32,
                    Rgba(p[(b[1] + cy) * d.width + b[0] + cx]),
                );
            }
        }
        let id = d.frame_export_id(f);
        frames.push(json!({"filename":format!("frame_{id}"),"frameId":id,"frame":{"x":x,"y":y,"w":b[2],"h":b[3]},"rotated":false,"trimmed":o.trim,"spriteSourceSize":{"x":b[0],"y":b[1],"w":b[2],"h":b[3]},"sourceSize":{"w":d.width,"h":d.height},"duration":d.durations[f],"sourceFrame":f}));
    }
    let tags: Vec<Value> = d
        .tags
        .iter()
        .filter_map(|tag| {
            let from = tag.from.max(first_frame);
            let to = tag.to.min(last_frame);
            (from <= to).then(|| {
                json!({
                    "name": tag.name,
                    "from": from - first_frame,
                    "to": to - first_frame,
                    "repeat": tag.repeat,
                    "direction": match tag.direction {
                        1 => "reverse",
                        2 => "pingpong",
                        3 => "pingpong_reverse",
                        _ => "forward",
                    },
                })
            })
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
    Ok((
        im,
        json!({"frames":frames,"meta":{"app":"PixelCraft","version":"1","image":"sprite.png","format":"RGBA8888","size":{"w":width*scale,"h":height*scale},"scale":scale.to_string(),"padding":pad*scale,"frameTags":tags,"slices":slices,"nextFrameId":d.next_frame_export_id()}}),
    ))
}

fn visible_bounds(pixels: &[Pixel], width: usize, height: usize) -> [usize; 4] {
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, _) in pixels.iter().enumerate().filter(|(_, pixel)| pixel[3] > 0) {
        let (x, y) = (index % width, index / width);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    if left < right && top < bottom {
        [left, top, right - left, bottom - top]
    } else {
        [0, 0, 1, 1]
    }
}
pub fn unpack_sheet(im: &RgbaImage, meta: &Value) -> Result<Document, String> {
    let frames: Vec<&Value> = if let Some(a) = meta["frames"].as_array() {
        a.iter().collect()
    } else if let Some(a) = meta["frames"].as_object() {
        a.values().collect()
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
        || frames.len() > 65535
        || w.saturating_mul(h).saturating_mul(frames.len()) > MAX_PIXELS
    {
        return Err("Sheet exceeds limits".into());
    }
    let mut d = Document::new(w, h);
    for (f, v) in frames.iter().enumerate() {
        if v["rotated"] == true {
            return Err("Rotated atlas entries are unsupported".into());
        }
        if f > 0 {
            d.add_frame(f - 1, false)
        }
        d.durations[f] = v["duration"].as_u64().unwrap_or(100).clamp(1, 65535) as u16;
        let r = &v["frame"];
        let get = |key: &str| {
            r[key]
                .as_u64()
                .ok_or_else(|| format!("Missing frame {key}"))
        };
        let (x, y, rw, rh) = (get("x")?, get("y")?, get("w")?, get("h")?);
        let ox = v["spriteSourceSize"]["x"].as_u64().unwrap_or(0);
        let oy = v["spriteSourceSize"]["y"].as_u64().unwrap_or(0);
        if x.saturating_add(rw) > im.width() as u64
            || y.saturating_add(rh) > im.height() as u64
            || ox.saturating_add(rw) > w as u64
            || oy.saturating_add(rh) > h as u64
        {
            return Err("Frame is outside sheet or canvas".into());
        }
        for cy in 0..rh {
            for cx in 0..rw {
                d.layers[0].cels[f][(oy + cy) as usize * w + (ox + cx) as usize] =
                    im.get_pixel((x + cx) as u32, (y + cy) as u32).0;
            }
        }
    }
    if meta["meta"]["app"] == "PixelCraft"
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
                repeat: match t.get("repeat").and_then(Value::as_u64) {
                    Some(n) => u16::try_from(n).map_err(|_| "Invalid tag repeat")?,
                    None => 0,
                },
                color: [0; 3],
            });
        }
    }
    if let Some(slices) = meta["meta"].get("slices") {
        d.slices = serde_json::from_value(slices.clone()).map_err(|_| "Invalid slice metadata")?;
    }
    d.validate()?;
    Ok(d)
}

/// Transform a selection mask with the same nearest-neighbor mapping as its pixels.
pub fn transform_mask(d: &Document, b: [i32; 4], mask: &[bool], t: Transform) -> Vec<bool> {
    let [x, y, w, h] = b;
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
        let shifts = offset.map_or_else(|| vec![(-1, 0), (1, 0), (0, -1), (0, 1)], |v| vec![v]);
        for (dx, dy) in shifts {
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
