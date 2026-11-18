use super::{FLC, FLI, FRAME, HEADER_SIZE, io_text};
use crate::{
    automation::mcp_io,
    formats::{animation_dimensions, raster::on_white},
    model::{ColorMode, Document, MAX_DOCUMENT_PIXELS, Pixel},
};
use std::{
    collections::HashMap,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

struct Timing {
    speed: u16,
    copies: Vec<u16>,
    frames: u16,
}

impl Timing {
    fn new(doc: &Document, classic: bool) -> Result<Self, String> {
        let (speed, copies) = if classic {
            let ticks: Vec<u16> = doc
                .durations
                .iter()
                .map(|&ms| ((u32::from(ms) * 70 + 500) / 1000).max(1) as u16)
                .collect();
            let speed = ticks.iter().copied().reduce(gcd).unwrap_or(1);
            (
                speed,
                ticks
                    .into_iter()
                    .map(|ticks| ticks / speed)
                    .collect::<Vec<_>>(),
            )
        } else {
            (doc.durations[0], vec![1; doc.durations.len()])
        };
        let frames = u16::try_from(copies.iter().map(|&count| u32::from(count)).sum::<u32>())
            .map_err(|_| "FLI timing requires more than 65,535 frames; use FLC")?;
        Ok(Self {
            speed,
            copies,
            frames,
        })
    }
}

fn gcd(mut lhs: u16, mut rhs: u16) -> u16 {
    while rhs != 0 {
        (lhs, rhs) = (rhs, lhs % rhs);
    }
    lhs
}

fn plan(doc: &Document, scale: usize, classic: bool) -> Result<([usize; 2], Timing), String> {
    doc.validate()?;
    let (width, height) = animation_dimensions(doc, scale)?;
    if classic && (width, height) != (320, 200) {
        return Err("FLI requires a 320 × 200 output; use FLC for other sizes".into());
    }
    let timing = Timing::new(doc, classic)?;
    if (width * height).saturating_mul(usize::from(timing.frames)) > MAX_DOCUMENT_PIXELS {
        return Err("FLIC export exceeds the combined frame pixel limit".into());
    }
    Ok(([width, height], timing))
}

pub(crate) fn dimensions(
    doc: &Document,
    scale: usize,
    classic: bool,
) -> Result<(usize, usize), String> {
    let ([width, height], _) = plan(doc, scale, classic)?;
    Ok((width, height))
}

pub(crate) fn export(
    path: &Path,
    doc: &Document,
    scale: usize,
    classic: bool,
    mut progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    let ([width, height], timing) = plan(doc, scale, classic)?;
    let mut writer = BufWriter::new(mcp_io::create(path).map_err(io_text)?);
    writer.write_all(&[0; HEADER_SIZE]).map_err(io_text)?;
    let mut first = None;
    let mut previous = None;
    let mut second_offset = None;
    for (frame, &copies) in timing.copies.iter().enumerate() {
        progress(frame, timing.copies.len())?;
        let image = indexed_frame(doc, frame, classic, scale);
        let delay = if classic || doc.durations[frame] == timing.speed {
            0
        } else {
            doc.durations[frame]
        };
        if frame == 1 {
            second_offset = Some(file_position(&mut writer)?);
        }
        write_frame(
            &mut writer,
            &image,
            previous.as_ref(),
            width,
            delay,
            classic,
        )?;
        for _ in 1..copies {
            write_frame(&mut writer, &image, Some(&image), width, 0, classic)?;
        }
        if first.is_none() {
            first = Some(image.clone());
        }
        previous = Some(image);
    }
    if second_offset.is_none() {
        second_offset = Some(file_position(&mut writer)?);
    }
    // The ring frame returns to the first image and is outside the authored count.
    write_frame(
        &mut writer,
        &first.unwrap(),
        previous.as_ref(),
        width,
        0,
        classic,
    )?;
    progress(timing.copies.len(), timing.copies.len())?;
    let size = file_position(&mut writer)?;
    let mut header = [0; HEADER_SIZE];
    header[..4].copy_from_slice(&size.to_le_bytes());
    for (offset, value) in [
        (4, if classic { FLI } else { FLC }),
        (6, timing.frames),
        (8, width as u16),
        (10, height as u16),
        (12, 8),
        (14, 3),
        (16, timing.speed),
    ] {
        header[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    if !classic {
        header[38..42].copy_from_slice(&[1, 0, 1, 0]);
        header[80..84].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
        header[84..88].copy_from_slice(&second_offset.unwrap().to_le_bytes());
    }
    writer.seek(SeekFrom::Start(0)).map_err(io_text)?;
    writer.write_all(&header).map_err(io_text)?;
    writer.flush().map_err(io_text)
}

fn file_position(writer: &mut impl Seek) -> Result<u32, String> {
    u32::try_from(writer.stream_position().map_err(io_text)?)
        .map_err(|_| "FLIC output exceeds the 32-bit file size limit".into())
}

#[derive(Clone)]
struct IndexedFrame {
    palette: [[u8; 3]; 256],
    pixels: Vec<u8>,
}

fn indexed_frame(doc: &Document, frame: usize, classic: bool, scale: usize) -> IndexedFrame {
    let pixel_color = |p: Pixel| {
        let rgb = [
            on_white(p[0], p[3]),
            on_white(p[1], p[3]),
            on_white(p[2], p[3]),
        ];
        if classic {
            rgb.map(|channel| ((u16::from(channel) * 63 + 127) / 255) as u8)
        } else {
            rgb
        }
    };
    let pixels = doc.composite(frame);
    let mut indexed = if let [layer] = doc.layers.as_slice()
        && doc.color_mode == ColorMode::Indexed
        && let Some(indices) = &layer.cel_meta[frame].indices
    {
        let source = doc.palette_for(frame);
        let mut palette = [[0; 3]; 256];
        for (target, &color) in palette.iter_mut().zip(source) {
            *target = pixel_color(color);
        }
        if pixels
            .iter()
            .zip(indices.as_slice())
            .all(|(&pixel, &index)| {
                usize::from(index) < source.len()
                    && palette[usize::from(index)] == pixel_color(pixel)
            })
        {
            IndexedFrame {
                palette,
                pixels: indices.as_ref().clone(),
            }
        } else {
            quantize(&pixels, pixel_color, classic)
        }
    } else {
        quantize(&pixels, pixel_color, classic)
    };

    if scale > 1 {
        let mut scaled = Vec::with_capacity(indexed.pixels.len() * scale * scale);
        let stride = doc.width * scale;
        for row in indexed.pixels.chunks_exact(doc.width) {
            let start = scaled.len();
            for &index in row {
                scaled.extend(std::iter::repeat_n(index, scale));
            }
            for _ in 1..scale {
                scaled.extend_from_within(start..start + stride);
            }
        }
        indexed.pixels = scaled;
    }
    indexed
}

fn quantize(
    pixels: &[Pixel],
    pixel_color: impl Fn(Pixel) -> [u8; 3],
    classic: bool,
) -> IndexedFrame {
    let mut result = IndexedFrame {
        palette: [[0; 3]; 256],
        pixels: Vec::with_capacity(pixels.len()),
    };
    let mut index = HashMap::new();
    for &pixel in pixels {
        let color = pixel_color(pixel);
        let next = index.len();
        let slot = match index.entry(color) {
            std::collections::hash_map::Entry::Occupied(slot) => *slot.get(),
            std::collections::hash_map::Entry::Vacant(slot) => {
                if next == 256 {
                    return quantize_large(pixels, pixel_color, classic);
                }
                result.palette[next] = color;
                *slot.insert(next as u8)
            }
        };
        result.pixels.push(slot);
    }
    result
}

fn quantize_large(
    pixels: &[Pixel],
    pixel_color: impl Fn(Pixel) -> [u8; 3],
    classic: bool,
) -> IndexedFrame {
    let expanded = |pixel| {
        let rgb = pixel_color(pixel);
        let [r, g, b] = if classic {
            rgb.map(|v| (v << 2) | (v >> 4))
        } else {
            rgb
        };
        [r, g, b, 255]
    };
    // Train from a bounded, evenly spaced sample across the frame.
    let stride = pixels.len().div_ceil(32768);
    let training: Vec<u8> = pixels
        .iter()
        .step_by(stride)
        .flat_map(|&pixel| expanded(pixel))
        .collect();
    let quantizer = color_quant::NeuQuant::new(1, 256, &training);
    let mut palette = [[0; 3]; 256];
    for (entry, rgb) in palette
        .iter_mut()
        .zip(quantizer.color_map_rgb().chunks_exact(3))
    {
        let channels: [u8; 3] = rgb.try_into().unwrap();
        *entry = if classic {
            channels.map(|v| ((u16::from(v) * 63 + 127) / 255) as u8)
        } else {
            channels
        };
    }
    IndexedFrame {
        palette,
        pixels: pixels
            .iter()
            .map(|&pixel| quantizer.index_of(&expanded(pixel)) as u8)
            .collect(),
    }
}

fn write_frame(
    writer: &mut impl Write,
    image: &IndexedFrame,
    previous: Option<&IndexedFrame>,
    width: usize,
    delay: u16,
    classic: bool,
) -> Result<(), String> {
    let mut frame = vec![0; 16];
    let mut count = 0u16;
    if previous.is_none_or(|prev| prev.palette != image.palette) {
        append_chunk(&mut frame, if classic { 11 } else { 4 }, |payload| {
            payload.extend_from_slice(&[1, 0, 0, 0]);
            payload.extend(image.palette.into_iter().flatten());
        });
        count += 1;
    }
    if previous.is_none_or(|prev| prev.pixels != image.pixels) {
        append_chunk(&mut frame, 15, |payload| {
            for row in image.pixels.chunks_exact(width) {
                encode_row(row, payload);
            }
        });
        count += 1;
    }
    let size = u32::try_from(frame.len()).map_err(|_| "FLIC frame exceeds the file size limit")?;
    frame[..4].copy_from_slice(&size.to_le_bytes());
    frame[4..6].copy_from_slice(&FRAME.to_le_bytes());
    frame[6..8].copy_from_slice(&count.to_le_bytes());
    frame[8..10].copy_from_slice(&delay.to_le_bytes());
    writer.write_all(&frame).map_err(io_text)
}

fn append_chunk(frame: &mut Vec<u8>, kind: u16, encode: impl FnOnce(&mut Vec<u8>)) {
    let offset = frame.len();
    frame.extend_from_slice(&[0; 4]);
    frame.extend_from_slice(&kind.to_le_bytes());
    encode(frame);
    if !frame.len().is_multiple_of(2) {
        frame.push(0);
    }
    let size = (frame.len() - offset) as u32;
    frame[offset..offset + 4].copy_from_slice(&size.to_le_bytes());
}

fn encode_row(mut row: &[u8], bytes: &mut Vec<u8>) {
    let start = bytes.len();
    bytes.push(0);
    let mut packets = 0usize;
    while let Some(&index) = row.first() {
        let repeated = row
            .iter()
            .take(127)
            .take_while(|&&other| other == index)
            .count();
        let len = if repeated >= 3 {
            bytes.extend_from_slice(&[repeated as u8, index]);
            repeated
        } else {
            let mut literals = 1;
            while literals < row.len().min(128)
                && !row
                    .get(literals..literals + 3)
                    .is_some_and(|three| three[0] == three[1] && three[0] == three[2])
            {
                literals += 1;
            }
            bytes.push((-(literals as i16)) as u8);
            bytes.extend_from_slice(&row[..literals]);
            literals
        };
        packets += 1;
        row = &row[len..];
    }
    // The original 320-wide FLI rows always fit. FLC decoders count decoded pixels.
    bytes[start] = u8::try_from(packets).unwrap_or(0);
}
