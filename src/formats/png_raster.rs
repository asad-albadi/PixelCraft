use crate::{
    automation::mcp_io,
    model::{ColorMode, Document, MAX_DIMENSION, MAX_PIXELS, Pixel},
};
use png::{BitDepth, ColorType};
use std::{collections::HashMap, io::BufReader, path::Path};

pub(super) fn load(path: &Path) -> Result<Document, String> {
    let file = mcp_io::open(path).map_err(|e| e.to_string())?;
    let mut reader = png::Decoder::new(BufReader::new(file))
        .read_info()
        .map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.animation_control.is_some() {
        return crate::interchange::load_animation(path);
    }
    if info.color_type != ColorType::Indexed {
        let grayscale = matches!(
            info.color_type,
            ColorType::Grayscale | ColorType::GrayscaleAlpha
        );
        drop(reader);
        let mut doc = super::raster::load_static(path, "png")?;
        if grayscale {
            doc.color_mode = ColorMode::Grayscale;
        }
        return Ok(doc);
    }

    let (width, height) = (info.width as usize, info.height as usize);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("PNG exceeds canvas limits".into());
    }
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
    let palette: Vec<Pixel> = colors
        .chunks_exact(3)
        .enumerate()
        .map(|(i, rgb)| [rgb[0], rgb[1], rgb[2], alpha.get(i).copied().unwrap_or(255)])
        .collect();
    let size = reader
        .output_buffer_size()
        .ok_or("Indexed PNG exceeds buffer limits")?;
    let mut packed = vec![0; size];
    let frame = reader.next_frame(&mut packed).map_err(|e| e.to_string())?;
    let bits = frame.bit_depth as usize;
    let mask = ((1u16 << bits) - 1) as u8;
    let mut indices = Vec::with_capacity(width * height);
    let mut pixels = Vec::with_capacity(width * height);
    for row in packed[..frame.buffer_size()].chunks_exact(frame.line_size) {
        for x in 0..width {
            let bit = x * bits;
            let slot = (row[bit / 8] >> (8 - bits - bit % 8)) & mask;
            let pixel = palette
                .get(slot as usize)
                .ok_or("Indexed PNG uses an undefined palette slot")?;
            pixels.push(*pixel);
            indices.push(slot);
        }
    }
    let transparent = palette.iter().position(|color| color[3] == 0);
    let mut doc = Document::new(width, height);
    doc.color_mode = ColorMode::Indexed;
    doc.palette = palette;
    doc.transparent_index = transparent.unwrap_or(0);
    doc.layers[0].background = transparent.is_none();
    doc.layers[0].cels[0] = pixels.into();
    doc.layers[0].cel_meta[0].indices = Some(indices.into());
    doc.validate()?;
    Ok(doc)
}

pub(super) fn write(
    path: &Path,
    doc: &Document,
    frames: &[usize],
    image: &image::RgbaImage,
    scale: usize,
) -> Result<bool, String> {
    let mut palette = None;
    let (color, samples) = match doc.color_mode {
        ColorMode::Grayscale if image.pixels().all(|p| p[0] == p[1] && p[1] == p[2]) => {
            let transparent = image.pixels().any(|p| p[3] < 255);
            let color = if transparent {
                ColorType::GrayscaleAlpha
            } else {
                ColorType::Grayscale
            };
            let mut samples = Vec::with_capacity(
                image.width() as usize * image.height() as usize * color.samples(),
            );
            for pixel in image.pixels() {
                samples.push(pixel[0]);
                if transparent {
                    samples.push(pixel[3]);
                }
            }
            (color, samples)
        }
        ColorMode::Indexed => {
            let Some((colors, indices)) = indexed_samples(doc, frames, image) else {
                return Ok(false);
            };
            palette = Some(colors);
            (ColorType::Indexed, indices)
        }
        _ => return Ok(false),
    };
    let (width, height) = (image.width() as usize, image.height() as usize);
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
            .write_image_data(&samples)
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
    Ok(true)
}

fn indexed_samples(
    doc: &Document,
    frames: &[usize],
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
                doc.index((x % doc.width) as i32 - dx, y as i32 - dy)
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
