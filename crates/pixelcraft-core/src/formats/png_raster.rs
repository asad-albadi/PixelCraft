use crate::{
    model::{CLEAR, ColorMode, Document, MAX_DIMENSION, MAX_DOCUMENT_PIXELS, MAX_PIXELS, Pixel},
    storage::mcp_io,
};
use image::Pixel as _;
use png::{BitDepth, BlendOp, ColorType, DisposeOp};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Seek},
    path::Path,
};

pub(super) fn load(path: &Path, require_animation: bool) -> Result<Document, String> {
    let file = mcp_io::open(path).map_err(|e| e.to_string())?;
    let mut reader = png::Decoder::new(BufReader::new(file))
        .read_info()
        .map_err(|e| e.to_string())?;
    let info = reader.info();
    let (width, height) = (info.width as usize, info.height as usize);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("PNG exceeds canvas limits".into());
    }
    let animation = info.animation_control;
    if require_animation && animation.is_none() {
        return Err("Not an animated PNG".into());
    }
    if info.color_type != ColorType::Indexed {
        let grayscale = matches!(
            info.color_type,
            ColorType::Grayscale | ColorType::GrayscaleAlpha
        );
        drop(reader);
        let mut doc = if animation.is_some() {
            crate::interchange::load_animation(path)?
        } else {
            super::raster::load_static(path, "png")?
        };
        if grayscale {
            doc.color_mode = ColorMode::Grayscale;
        }
        return Ok(doc);
    }

    let palette = rgba_palette(info)?;
    if let Some(animation) = animation {
        return match load_indexed_animation(
            reader,
            palette,
            [width, height],
            animation.num_frames as usize,
        )? {
            Some(doc) => Ok(doc),
            None => crate::interchange::load_animation(path),
        };
    }
    let size = reader
        .output_buffer_size()
        .ok_or("Indexed PNG exceeds buffer limits")?;
    let mut packed = vec![0; size];
    let frame = reader.next_frame(&mut packed).map_err(|e| e.to_string())?;
    let indices = packed_indices(&frame, &packed).collect::<Vec<_>>();
    let mut doc = Document::new(width, height);
    set_indexed_cel(&mut doc, 0, &palette, indices)?;
    set_palette(&mut doc, palette);
    doc.validate()?;
    Ok(doc)
}

fn rgba_palette(info: &png::Info<'_>) -> Result<Vec<Pixel>, String> {
    let colors = info
        .palette
        .as_deref()
        .ok_or("Indexed PNG has no palette")?;
    let alpha = info.trns.as_deref().unwrap_or_default();
    if colors.is_empty()
        || colors.len() % 3 != 0
        || colors.len() > 768
        || alpha.len() > colors.len() / 3
    {
        return Err("Invalid indexed PNG palette".into());
    }
    Ok(colors
        .chunks_exact(3)
        .enumerate()
        .map(|(i, rgb)| [rgb[0], rgb[1], rgb[2], alpha.get(i).copied().unwrap_or(255)])
        .collect())
}

fn packed_indices<'a>(
    frame: &'a png::OutputInfo,
    packed: &'a [u8],
) -> impl Iterator<Item = u8> + 'a {
    let bits = frame.bit_depth as usize;
    let mask = ((1u16 << bits) - 1) as u8;
    packed[..frame.buffer_size()]
        .chunks_exact(frame.line_size)
        .flat_map(move |row| {
            (0..frame.width as usize).map(move |x| {
                let bit = x * bits;
                (row[bit / 8] >> (8 - bits - bit % 8)) & mask
            })
        })
}

fn set_indexed_cel(
    doc: &mut Document,
    frame: usize,
    palette: &[Pixel],
    indices: Vec<u8>,
) -> Result<(), String> {
    let pixels = indices
        .iter()
        .map(|&slot| {
            palette
                .get(slot as usize)
                .copied()
                .ok_or("Indexed PNG uses an undefined palette slot")
        })
        .collect::<Result<Vec<_>, _>>()?;
    doc.layers[0].cels[frame] = pixels.into();
    doc.layers[0].cel_meta[frame].indices = Some(indices.into());
    Ok(())
}

fn set_palette(doc: &mut Document, palette: Vec<Pixel>) {
    let transparent = palette.iter().position(|color| color[3] == 0);
    doc.color_mode = ColorMode::Indexed;
    doc.palette = palette;
    doc.transparent_index = transparent.unwrap_or(0);
    doc.layers[0].background = transparent.is_none();
}

/// Return None if compositing needs more slots than the indexed model can hold.
fn load_indexed_animation(
    mut reader: png::Reader<impl BufRead + Seek>,
    mut palette: Vec<Pixel>,
    [width, height]: [usize; 2],
    frame_count: usize,
) -> Result<Option<Document>, String> {
    const EMPTY: u16 = 256;
    let area = width * height;
    if frame_count == 0
        || frame_count > 65_535
        || frame_count.saturating_mul(area) > MAX_DOCUMENT_PIXELS
    {
        return Err("Animated PNG exceeds frame or pixel limits".into());
    }
    if frame_count.saturating_mul(area + area.div_ceil(4)) > MAX_DOCUMENT_PIXELS {
        return Ok(None);
    }
    let mut packed = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("Animated PNG exceeds buffer limits")?
    ];
    if reader.info().frame_control.is_none() {
        // A separate default image is a poster, not a timeline frame.
        reader.next_frame(&mut packed).map_err(|e| e.to_string())?;
    }
    let source_colors = palette.len();
    let mut clear = palette
        .iter()
        .position(|color| color[3] == 0)
        .map(|i| i as u8);
    let mut mapped = HashMap::new();
    for (slot, &pixel) in palette.iter().enumerate() {
        mapped.entry(pixel).or_insert(slot as u8);
    }
    let mut canvas = vec![EMPTY; area];
    let mut doc = Document::new(width, height);
    doc.layers[0].name = "Animation".into();
    for n in 0..frame_count {
        let frame = reader.next_frame(&mut packed).map_err(|e| e.to_string())?;
        let control = reader
            .info()
            .frame_control
            .ok_or("Animated PNG frame has no control chunk")?;
        let [left, top, w, h] = [
            control.x_offset,
            control.y_offset,
            frame.width,
            frame.height,
        ]
        .map(|v| v as usize);
        if w == 0 || h == 0 || left > width || top > height || w > width - left || h > height - top
        {
            return Err("Animated PNG frame exceeds its canvas".into());
        }
        let row = |y| (top + y) * width + left..(top + y) * width + left + w;
        let previous = (control.dispose_op == DisposeOp::Previous).then(|| {
            (0..h)
                .flat_map(|y| canvas[row(y)].iter().copied())
                .collect::<Vec<_>>()
        });
        for (index, slot) in packed_indices(&frame, &packed).enumerate() {
            if slot as usize >= source_colors {
                return Err("Indexed PNG uses an undefined palette slot".into());
            }
            let target = &mut canvas[(top + index / w) * width + left + index % w];
            if control.blend_op == BlendOp::Source {
                *target = u16::from(slot);
                continue;
            }
            let source = palette[slot as usize];
            if source[3] == 0 {
                continue;
            }
            let dest = palette.get(*target as usize).copied().unwrap_or(CLEAR);
            if source[3] == 255 || dest[3] == 0 {
                *target = u16::from(slot);
                continue;
            }
            let mut blended = image::Rgba(dest);
            blended.blend(&image::Rgba(source));
            if blended.0 != dest {
                let Some(index) = palette_slot(&mut palette, &mut mapped, blended.0) else {
                    return Ok(None);
                };
                *target = u16::from(index);
            }
        }
        if clear.is_none() && canvas.contains(&EMPTY) {
            clear = palette_slot(&mut palette, &mut mapped, CLEAR);
            if clear.is_none() {
                return Ok(None);
            }
        }
        let indices = canvas
            .iter()
            .map(|&slot| {
                if slot == EMPTY {
                    clear.unwrap()
                } else {
                    slot as u8
                }
            })
            .collect();
        if n > 0 {
            doc.add_frame(n - 1, false);
        }
        let den = if control.delay_den == 0 {
            100
        } else {
            control.delay_den
        };
        doc.durations[n] =
            (u32::from(control.delay_num) * 1000 / u32::from(den)).clamp(1, 65_535) as u16;
        set_indexed_cel(&mut doc, n, &palette, indices)?;
        match control.dispose_op {
            DisposeOp::None => {}
            DisposeOp::Background => {
                for y in 0..h {
                    canvas[row(y)].fill(EMPTY);
                }
            }
            DisposeOp::Previous => {
                let previous = previous.unwrap();
                for (y, saved) in previous.chunks_exact(w).enumerate() {
                    canvas[row(y)].copy_from_slice(saved);
                }
            }
        }
    }
    reader.finish().map_err(|e| e.to_string())?;
    set_palette(&mut doc, palette);
    doc.validate()?;
    Ok(Some(doc))
}

fn palette_slot(
    palette: &mut Vec<Pixel>,
    mapped: &mut HashMap<Pixel, u8>,
    pixel: Pixel,
) -> Option<u8> {
    if let Some(&slot) = mapped.get(&pixel) {
        return Some(slot);
    }
    let slot = u8::try_from(palette.len()).ok()?;
    palette.push(pixel);
    mapped.insert(pixel, slot);
    Some(slot)
}

pub(super) fn write(
    path: &Path,
    doc: &Document,
    frames: &[usize],
    origin: [usize; 2],
    image: &image::RgbaImage,
    scale: usize,
) -> Result<bool, String> {
    let (color, samples, palette) = match doc.color_mode {
        ColorMode::Grayscale => {
            let Some((color, samples)) = grayscale_samples(image) else {
                return Ok(false);
            };
            (color, samples, None)
        }
        ColorMode::Indexed => {
            let Some((colors, indices)) = indexed_samples(doc, frames, origin, image) else {
                return Ok(false);
            };
            (ColorType::Indexed, indices, Some(colors))
        }
        _ => return Ok(false),
    };
    write_samples(
        path,
        [image.width(), image.height()],
        color,
        &samples,
        palette.as_deref(),
        scale,
    )?;
    Ok(true)
}

pub(super) fn write_sheet(
    path: &Path,
    doc: &Document,
    options: &crate::workflows::SheetOptions,
    metadata: &Value,
    image: &image::RgbaImage,
    extrude: usize,
) -> Result<Option<ColorType>, String> {
    let (color, samples, palette) = match doc.color_mode {
        ColorMode::Grayscale => {
            let Some((color, samples)) = grayscale_samples(image) else {
                return Ok(None);
            };
            (color, samples, None)
        }
        ColorMode::Indexed => {
            let filtered =
                crate::workflows::filter_layers_for_export(doc, options.layers.as_deref())?;
            let Some((palette, indices)) =
                packed_sheet_indices(&filtered, metadata, image, options.scale, extrude)
            else {
                return Ok(None);
            };
            (ColorType::Indexed, indices, Some(palette))
        }
        ColorMode::Rgba => return Ok(None),
    };
    write_samples(
        path,
        [image.width(), image.height()],
        color,
        &samples,
        palette.as_deref(),
        1,
    )?;
    Ok(Some(color))
}

fn grayscale_samples(image: &image::RgbaImage) -> Option<(ColorType, Vec<u8>)> {
    if image.pixels().any(|p| p[0] != p[1] || p[1] != p[2]) {
        return None;
    }
    let transparent = image.pixels().any(|p| p[3] < 255);
    let color = if transparent {
        ColorType::GrayscaleAlpha
    } else {
        ColorType::Grayscale
    };
    let mut samples =
        Vec::with_capacity(image.width() as usize * image.height() as usize * color.samples());
    for pixel in image.pixels() {
        samples.push(pixel[0]);
        if transparent {
            samples.push(pixel[3]);
        }
    }
    Some((color, samples))
}

fn write_samples(
    path: &Path,
    [width, height]: [u32; 2],
    color: ColorType,
    samples: &[u8],
    palette: Option<&[Pixel]>,
    scale: usize,
) -> Result<(), String> {
    let (width, height) = (width as usize, height as usize);
    let file = mcp_io::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(file, (width * scale) as u32, (height * scale) as u32);
    enc.set_color(color);
    enc.set_depth(BitDepth::Eight);
    if let Some(palette) = palette {
        let rgb: Vec<_> = palette
            .iter()
            .flat_map(|color| color[..3].iter().copied())
            .collect();
        enc.set_palette(rgb);
        if let Some(last) = palette.iter().rposition(|color| color[3] < 255) {
            let alpha: Vec<_> = palette[..=last].iter().map(|color| color[3]).collect();
            enc.set_trns(alpha);
        }
    }
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    if scale == 1 {
        writer
            .write_image_data(samples)
            .map_err(|e| e.to_string())?;
    } else {
        use std::io::Write;
        let stride = width * color.samples();
        let mut row = Vec::with_capacity(stride * scale);
        let mut stream = writer.stream_writer().map_err(|e| e.to_string())?;
        for source in samples.chunks_exact(stride) {
            row.clear();
            for pixel in source.chunks_exact(color.samples()) {
                for _ in 0..scale {
                    row.extend_from_slice(pixel);
                }
            }
            for _ in 0..scale {
                stream.write_all(&row).map_err(|e| e.to_string())?;
            }
        }
        stream.finish().map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn packed_sheet_indices(
    doc: &Document,
    metadata: &Value,
    image: &image::RgbaImage,
    scale: usize,
    extrude: usize,
) -> Option<(Vec<Pixel>, Vec<u8>)> {
    let frames = metadata["frames"].as_array()?;
    let first = json_index(&frames.first()?["sourceFrame"])?;
    let default_layer = sole_raster_layer(doc);
    let initial_layer = json_index(&frames[0]["sourceLayer"]).or(default_layer);
    let mut palette = initial_layer.map_or_else(
        || doc.palette_for(first).to_vec(),
        |layer| doc.editing_palette(layer, first),
    );
    let mut mapped = HashMap::new();
    for (slot, &pixel) in palette.iter().enumerate() {
        mapped.entry(pixel).or_insert(slot as u8);
    }
    let clear = palette
        .iter()
        .position(|p| p[3] == 0)
        .map(|slot| slot as u8);
    let mut indices = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for pixel in image.pixels() {
        let slot = if let Some(slot) = clear.filter(|_| pixel.0 == CLEAR) {
            slot
        } else {
            palette_slot(&mut palette, &mut mapped, pixel.0)?
        };
        indices.push(slot);
    }
    let slice = metadata["meta"]["sourceSlice"]
        .as_str()
        .and_then(|name| doc.slices.iter().find(|s| s.name == name));
    let sheet_width = image.width() as usize;
    let border = extrude * scale;
    // Use the packing rectangles to recover source slots lost in the RGBA composite.
    for entry in frames {
        let frame = json_index(&entry["sourceFrame"])?;
        let Some(layer) = json_index(&entry["sourceLayer"]).or(default_layer) else {
            continue;
        };
        let Some(source) = doc.layers[layer].cel_meta[frame].indices.as_deref() else {
            continue;
        };
        let rect = &entry["frame"];
        let (left, top) = (json_index(&rect["x"])?, json_index(&rect["y"])?);
        let (width, height) = (json_index(&rect["w"])?, json_index(&rect["h"])?);
        let crop = &entry["spriteSourceSize"];
        let grid = &entry["gridCell"];
        let key = doc.sampled_cel_key(layer, frame);
        let slice = slice.and_then(|slice| slice.key_at(frame));
        let offset = |axis, grid_axis: &str| {
            ((json_index(&crop[grid_axis])? / scale) as i32)
                .checked_add(json_index(&grid[grid_axis]).unwrap_or(0) as i32)?
                .checked_add(slice.map_or(0, |key| key.bounds[axis]))
        };
        let dx = offset(0, "x")? - key.x;
        let dy = offset(1, "y")? - key.y;
        let right = left + width + border;
        let bottom = top + height + border;
        if width == 0 || height == 0 || right > sheet_width || bottom > image.height() as usize {
            return None;
        }
        for y in top.checked_sub(border)?..bottom {
            let sy = dy + (y.saturating_sub(top).min(height - 1) / scale) as i32;
            for x in left.checked_sub(border)?..right {
                let sx = dx + (x.saturating_sub(left).min(width - 1) / scale) as i32;
                if let Some(index) = doc.index(sx, sy) {
                    let candidate = source[index];
                    let pixel = image.get_pixel(x as u32, y as u32).0;
                    if palette.get(candidate as usize) == Some(&pixel) {
                        indices[y * sheet_width + x] = candidate;
                    }
                }
            }
        }
    }
    Some((palette, indices))
}

fn json_index(value: &Value) -> Option<usize> {
    usize::try_from(value.as_u64()?).ok()
}

fn sole_raster_layer(doc: &Document) -> Option<usize> {
    let mut visible = doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| layer.visible && !layer.group && !layer.is_ase_reference());
    let (index, layer) = visible.next()?;
    (layer.tileset.is_none() && layer.adjustment.is_none() && visible.next().is_none())
        .then_some(index)
}

fn indexed_samples(
    doc: &Document,
    frames: &[usize],
    [left, top]: [usize; 2],
    image: &image::RgbaImage,
) -> Option<(Vec<Pixel>, Vec<u8>)> {
    let mut visible = doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| layer.visible && !layer.is_ase_reference());
    let layer = visible
        .next()
        .filter(|(_, l)| !l.group && l.parent.is_none() && l.tileset.is_none())
        .filter(|_| visible.next().is_none());
    let palette_for = |frame| {
        layer.map_or_else(
            || doc.palette_for(frame).to_vec(),
            |(index, _)| doc.editing_palette(index, frame),
        )
    };
    let palette = palette_for(frames[0]);
    if frames
        .iter()
        .skip(1)
        .any(|&frame| palette_for(frame) != palette)
    {
        return None;
    }
    let mut by_color = HashMap::new();
    for (index, &pixel) in palette.iter().enumerate() {
        by_color.entry(pixel).or_insert(index as u8);
    }
    let clear = palette
        .iter()
        .position(|color| color[3] == 0)
        .map(|index| index as u8);
    let cached: Vec<_> = frames
        .iter()
        .map(|&frame| {
            let (index, layer) = layer?;
            let indices = layer.cel_meta[frame].indices.as_deref()?;
            let key = doc.sampled_cel_key(index, frame);
            Some((indices, key.x, key.y))
        })
        .collect();
    let mut indices = Vec::with_capacity(image.width() as usize * image.height() as usize);
    for (x, y, color) in image.enumerate_pixels() {
        let x = x as usize;
        let y = y as usize;
        let pixel = color.0;
        let index = cached[x / doc.width]
            .and_then(|(indices, dx, dy)| {
                doc.index((left + x % doc.width) as i32 - dx, (top + y) as i32 - dy)
                    .map(|i| indices[i])
            })
            .filter(|&slot| {
                palette
                    .get(slot as usize)
                    .is_some_and(|&p| p == pixel || p[3] == 0 && pixel[3] == 0)
            })
            .or_else(|| by_color.get(&pixel).copied())
            .or_else(|| clear.filter(|_| pixel[3] == 0))?;
        indices.push(index);
    }
    Some((palette, indices))
}
