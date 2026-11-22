//! In-memory codec adapters for the bounded tablet ABI. Desktop codecs share the
//! same readers/writers. PNG ancillary metadata is omitted on this path; GIF is
//! composited RGBA with GIF palette/10ms timing limitations. ASE tilemaps and
//! external tilesets are intentionally rejected before decoding. ASE does not
//! encode native-only frame names; sheet JSON retains them. Off-canvas cels are
//! rejected before decoding to bound linked source expansion.
use super::*;
use image::{AnimationDecoder, ImageEncoder};
use std::io::Cursor;
const SIDE: usize = 256;
const FRAMES: usize = 64;
const RASTER: usize = 16 * 1024 * 1024;
const BUFFER: usize = 8 * 1024 * 1024;

/// Decode 1=PNG/APNG, 2=GIF, 3=ASE without opening paths.
pub fn decode_mobile_format(format: u32, bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() > BUFFER {
        return Err("Format input exceeds 8 MiB".into());
    }
    let doc = match format {
        1 => png_raster::decode_reader(Cursor::new(bytes), false, Some((SIDE, FRAMES, RASTER)))?,
        2 => {
            preflight_gif(bytes)?;
            let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            crate::interchange::from_frames_bounded(
                decoder.into_frames(),
                Some((SIDE, FRAMES, RASTER)),
            )?
        }
        3 => {
            preflight_ase(bytes)?;
            decode_ase(bytes)?
        }
        _ => return Err("Unknown import format".into()),
    };
    doc.validate()?;
    if doc.width > SIDE
        || doc.height > SIDE
        || doc.durations.len() > FRAMES
        || doc.layers.len() > 32
        || doc.stored_pixels() > RASTER / 4
    {
        return Err("Format exceeds bounded document limits".into());
    }
    Ok(doc)
}

fn dimensions(w: usize, h: usize, count: usize) -> Result<(), String> {
    if w == 0
        || h == 0
        || w > SIDE
        || h > SIDE
        || count == 0
        || count > FRAMES
        || w * h * count * 4 > RASTER
    {
        return Err("Format exceeds bounded canvas, frame or raster limits".into());
    }
    Ok(())
}
fn preflight_gif(b: &[u8]) -> Result<(), String> {
    if !b.starts_with(b"GIF87a") && !b.starts_with(b"GIF89a") {
        return Err("Invalid GIF signature".into());
    }
    dimensions(u16at(b, 6)? as usize, u16at(b, 8)? as usize, 1)?;
    let packed = *b.get(10).ok_or("Truncated GIF")?;
    let mut p = 13
        + if packed & 128 != 0 {
            3 * (1 << ((packed & 7) + 1))
        } else {
            0
        };
    let mut count = 0;
    loop {
        let kind = *b.get(p).ok_or("Truncated GIF")?;
        p += 1;
        match kind {
            0x3b => {
                return if count > 0 {
                    Ok(())
                } else {
                    Err("GIF has no frames".into())
                };
            }
            0x21 => {
                p += 1;
            }
            0x2c => {
                let w = u16at(b, p + 4)? as usize;
                let h = u16at(b, p + 6)? as usize;
                count += 1;
                dimensions(w, h, count)?;
                dimensions(u16at(b, 6)? as usize, u16at(b, 8)? as usize, count)?;
                let packed = *b.get(p + 8).ok_or("Truncated GIF")?;
                p += 9 + if packed & 128 != 0 {
                    3 * (1 << ((packed & 7) + 1))
                } else {
                    0
                };
                p += 1; // LZW code size
            }
            _ => return Err("Invalid GIF block".into()),
        }
        loop {
            let n = *b.get(p).ok_or("Truncated GIF block")? as usize;
            p += 1;
            if n == 0 {
                break;
            }
            p = p
                .checked_add(n)
                .filter(|&p| p <= b.len())
                .ok_or("Truncated GIF data")?;
        }
    }
}
fn preflight_ase(b: &[u8]) -> Result<(), String> {
    if b.len() < 128 || u16at(b, 4)? != 0xa5e0 {
        return Err("Invalid ASE header".into());
    }
    let w = u16at(b, 8)? as usize;
    let h = u16at(b, 10)? as usize;
    let frames = u16at(b, 6)? as usize;
    dimensions(w, h, frames)?;
    let mut layers = 0usize;
    let mut raster = 0usize;
    let mut p: usize = 128;
    let mut cel_sizes = std::collections::HashMap::new();
    for frame in 0..frames {
        let end = p
            .checked_add(u32at(b, p)? as usize)
            .filter(|&n| n <= b.len() && n >= p + 16)
            .ok_or("Invalid ASE frame")?;
        let mut c = p + 16;
        while c < end {
            let ce = c
                .checked_add(u32at(b, c)? as usize)
                .filter(|&n| n <= end && n >= c + 6)
                .ok_or("Invalid ASE chunk")?;
            let data = &b[c + 6..ce];
            match u16at(b, c + 4)? {
                0x2004 => {
                    layers += 1;
                    if layers > 32 {
                        return Err("ASE exceeds 32 layers".into());
                    }
                }
                0x2023 => return Err("Tablet ASE import does not support tilesets".into()),
                0x2005 => {
                    let ty = u16at(data, 7)?;
                    if ty == 3 {
                        return Err("Tablet ASE import does not support tilemaps".into());
                    }
                    let layer = u16at(data, 0)?;
                    if layer as usize >= layers {
                        return Err("ASE cel refers to a missing layer".into());
                    }
                    let size = match ty {
                        0 | 2 => (u16at(data, 16)? as usize, u16at(data, 18)? as usize),
                        1 => *cel_sizes
                            .get(&(layer, u16at(data, 16)? as usize))
                            .ok_or("Missing ASE linked source")?,
                        _ => return Err("Unknown ASE cel encoding".into()),
                    };
                    let x = u16at(data, 2)? as i16;
                    let y = u16at(data, 4)? as i16;
                    if x < 0
                        || y < 0
                        || (x as usize).saturating_add(size.0) > w
                        || (y as usize).saturating_add(size.1) > h
                    {
                        return Err("Tablet ASE import does not support off-canvas cels".into());
                    }
                    cel_sizes.insert((layer, frame), size);
                    raster = raster.saturating_add(w * h * 8);
                    if raster > RASTER {
                        return Err("ASE exceeds bounded raster budget".into());
                    }
                    if matches!(ty, 0 | 2) {
                        let cw = u16at(data, 16)? as usize;
                        let ch = u16at(data, 18)? as usize;
                        if cw > SIDE || ch > SIDE {
                            return Err("ASE cel exceeds 256 pixels".into());
                        }
                        // Reserve both decoded source and placed canvas, including
                        // indices/sparse bookkeeping/outside pixel storage.
                        raster = raster.saturating_add(cw * ch * 32);
                        if raster > RASTER {
                            return Err("ASE exceeds bounded raster budget".into());
                        }
                    }
                }
                _ => {}
            }
            c = ce;
        }
        p = end;
    }
    Ok(())
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > BUFFER {
            return Err(std::io::Error::other("Export exceeds 8 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Encode 1=active PNG, 2=animated GIF, 3=ASE, 4=sheet PNG, 5=sheet JSON.
/// Sheet files always pair as PixelCraft-sheet.png/PixelCraft-sheet.json, eight columns, no padding.
pub fn encode_mobile_format(format: u32, doc: &Document, frame: usize) -> Result<Vec<u8>, String> {
    doc.validate()?;
    dimensions(doc.width, doc.height, doc.durations.len())?;
    let mut out = BoundedOutput(Vec::new());
    match format {
        1 => {
            if frame >= doc.durations.len() {
                return Err("Unknown export frame".into());
            }
            let image = image::RgbaImage::from_raw(
                doc.width as u32,
                doc.height as u32,
                doc.composite(frame).into_iter().flatten().collect(),
            )
            .ok_or("Invalid canvas")?;
            if !png_raster::write_to(|| Ok(&mut out), doc, &[frame], [0, 0], &image, 1)? {
                rgba_png(&mut out, &image)?;
            }
        }
        2 => encode_gif_to(
            || Ok(&mut out),
            doc,
            0,
            0..doc.durations.len(),
            1,
            |_, _| Ok(()),
        )?,
        3 => {
            // ASE chunk assembly is shared with desktop. Bound its allocation by
            // preflighting a conservative raster/metadata output upper bound.
            let estimate = doc
                .stored_pixels()
                .saturating_mul(8)
                .saturating_add(doc.metadata.iter().map(|m| m.data.len()).sum::<usize>())
                .saturating_add(doc.color_profile.len())
                .saturating_add(doc.layers.len() * doc.durations.len() * 2048);
            if estimate > BUFFER {
                return Err("ASE exceeds bounded export budget".into());
            }
            out.write_all(&encode_ase(doc)?)
                .map_err(|e| e.to_string())?;
        }
        4 | 5 => {
            let options = crate::workflows::SheetOptions {
                padding: 0,
                ..Default::default()
            };
            let (image, mut meta) = crate::workflows::pack_sheet(doc, &options)?;
            meta["meta"]["image"] = serde_json::json!("PixelCraft-sheet.png");
            let mut png = BoundedOutput(Vec::new());
            let color =
                png_raster::write_sheet_to(|| Ok(&mut png), doc, &options, &meta, &image, 0)?;
            meta["meta"]["format"] = serde_json::json!(match color {
                Some(png::ColorType::Indexed) => "INDEXED8",
                Some(png::ColorType::Grayscale) => "GRAY8",
                Some(png::ColorType::GrayscaleAlpha) => "GRAYALPHA88",
                _ => "RGBA8888",
            });
            if color.is_none() {
                rgba_png(&mut png, &image)?;
            }
            if format == 4 {
                return Ok(png.0);
            }
            serde_json::to_writer(&mut out, &meta).map_err(|e| e.to_string())?;
        }
        _ => return Err("Unknown export format".into()),
    }
    Ok(out.0)
}
fn rgba_png(out: impl Write, image: &image::RgbaImage) -> Result<(), String> {
    image::codecs::png::PngEncoder::new(out)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_png_preserves_duplicate_palette_slots_and_sheet_metadata() {
        let mut doc = Document::new(2, 1);
        doc.color_mode = ColorMode::Indexed;
        doc.palette = vec![[200, 10, 20, 255], [200, 10, 20, 255]];
        doc.layers[0].background = true;
        doc.layers[0].cels[0] = vec![doc.palette[0]; 2].into();
        doc.layers[0].cel_meta[0].indices = Some(vec![0, 1].into());
        doc.set_frame_name(0, "idle").unwrap();
        let bytes = encode_mobile_format(1, &doc, 0).unwrap();
        let decoded = decode_mobile_format(1, &bytes).unwrap();
        assert_eq!(decoded.color_mode, ColorMode::Indexed);
        assert_eq!(
            decoded.layers[0].cel_meta[0].indices,
            doc.layers[0].cel_meta[0].indices
        );
        let meta: serde_json::Value =
            serde_json::from_slice(&encode_mobile_format(5, &doc, 0).unwrap()).unwrap();
        assert_eq!(meta["meta"]["image"], "PixelCraft-sheet.png");
        assert_eq!(meta["meta"]["format"], "INDEXED8");
        assert_eq!(meta["frames"][0]["frameName"], "idle");
    }
    #[test]
    fn apng_retains_timeline_pixels_and_durations() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.set_frame_delay(3, 100).unwrap();
            writer
                .write_image_data(&[255, 0, 0, 255, 255, 0, 0, 255])
                .unwrap();
            writer.set_frame_delay(7, 100).unwrap();
            writer
                .write_image_data(&[0, 255, 0, 255, 0, 255, 0, 255])
                .unwrap();
            writer.finish().unwrap();
        }
        let mut doc = decode_mobile_format(1, &bytes).unwrap();
        doc.set_frame_name(0, "idle").unwrap();
        doc.set_frame_name(1, "attack").unwrap();
        doc.tags.push(FrameTag {
            editor_id: Default::default(),
            name: "loop".into(),
            from: 0,
            to: 1,
            direction: 2,
            repeat: 3,
            color: [10, 20, 30],
        });
        assert_eq!(doc.durations, vec![30, 70]);
        assert_eq!(doc.composite(0), vec![[255, 0, 0, 255]; 2]);
        assert_eq!(doc.composite(1), vec![[0, 255, 0, 255]; 2]);
        let active = decode_mobile_format(1, &encode_mobile_format(1, &doc, 1).unwrap()).unwrap();
        assert_eq!(active.durations.len(), 1);
        assert_eq!(active.composite(0), doc.composite(1));
        let ase = decode_mobile_format(3, &encode_mobile_format(3, &doc, 0).unwrap()).unwrap();
        assert_eq!(ase.durations, doc.durations);
        assert_eq!(ase.tags, doc.tags);
        let meta: serde_json::Value =
            serde_json::from_slice(&encode_mobile_format(5, &doc, 0).unwrap()).unwrap();
        assert_eq!(meta["frames"][1]["duration"], 70);
        assert_eq!(meta["frames"][1]["frameName"], "attack");
        assert_eq!(meta["meta"]["frameTags"][0]["name"], "loop");
        assert_eq!(ase.composite(1), doc.composite(1));
        let gif = decode_mobile_format(2, &encode_mobile_format(2, &doc, 0).unwrap()).unwrap();
        assert_eq!(gif.durations, doc.durations);
        assert_eq!(gif.composite(1), doc.composite(1));
    }
    #[test]
    fn indexed_apng_preserves_indices_across_frames() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![255, 0, 0, 0, 255, 0]);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.set_frame_delay(3, 100).unwrap();
            writer.write_image_data(&[0, 1]).unwrap();
            writer.set_frame_delay(7, 100).unwrap();
            writer.write_image_data(&[1, 0]).unwrap();
            writer.finish().unwrap();
        }
        let doc = decode_mobile_format(1, &bytes).unwrap();
        assert_eq!(doc.color_mode, ColorMode::Indexed);
        assert_eq!(doc.durations, vec![30, 70]);
        assert_eq!(
            doc.layers[0].cel_meta[0].indices.as_deref().unwrap(),
            &[0, 1]
        );
        assert_eq!(
            doc.layers[0].cel_meta[1].indices.as_deref().unwrap(),
            &[1, 0]
        );
    }

    #[test]
    fn ase_preflight_rejects_linked_offcanvas_sources_before_decode() {
        let mut doc = Document::new(256, 256);
        doc.layers[0].cels[0] = vec![[4, 5, 6, 255]; 256 * 256].into();
        for frame in 0..63 {
            doc.add_frame(frame, true);
        }
        let mut bytes = encode_ase(&doc).unwrap();
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
        assert!(preflight_ase(&bytes).unwrap_err().contains("off-canvas"));
    }

    #[test]
    fn rejects_oversized_headers_frames_layers_and_buffers() {
        let mut ase = encode_ase(&Document::new(1, 1)).unwrap();
        ase[8..10].copy_from_slice(&257u16.to_le_bytes());
        assert!(decode_mobile_format(3, &ase).is_err());
        ase[8..10].copy_from_slice(&1u16.to_le_bytes());
        ase[6..8].copy_from_slice(&65u16.to_le_bytes());
        assert!(decode_mobile_format(3, &ase).is_err());
        let mut layers = Document::new(1, 1);
        for _ in 0..32 {
            layers.add_layer();
        }
        assert!(decode_mobile_format(3, &encode_ase(&layers).unwrap()).is_err());
        let mut rasters = Document::new(256, 256);
        for _ in 0..7 {
            rasters.add_frame(0, false);
        }
        for cel in &mut rasters.layers[0].cels {
            *cel = vec![[4, 5, 6, 255]; 256 * 256].into();
        }
        assert!(
            decode_mobile_format(3, &encode_ase(&rasters).unwrap())
                .unwrap_err()
                .contains("raster budget")
        );
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 257, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.write_header().unwrap().finish().unwrap();
        }
        assert!(decode_mobile_format(1, &png).is_err());
        let mut gif = b"GIF89a".to_vec();
        gif.extend(257u16.to_le_bytes());
        gif.extend(1u16.to_le_bytes());
        gif.extend([0, 0, 0, 0x3b]);
        assert!(decode_mobile_format(2, &gif).is_err());
        assert!(decode_mobile_format(1, &vec![0; BUFFER + 1]).is_err());
        for format in 1..=3 {
            for len in 0..128 {
                assert!(decode_mobile_format(format, &vec![0; len]).is_err());
            }
        }
    }
}
