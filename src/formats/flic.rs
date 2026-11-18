mod encode;
pub(crate) use encode::{dimensions, export};

use crate::{
    automation::mcp_io,
    model::{
        CelMeta, ColorMode, DEFAULT_DURATION, Document, MAX_DIMENSION, MAX_DOCUMENT_PIXELS,
        MAX_PIXELS, Pixel,
    },
};
use std::{
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};

const FLI: u16 = 0xaf11;
const FLC: u16 = 0xaf12;
const FRAME: u16 = 0xf1fa;
const HEADER_SIZE: usize = 128;

pub(super) fn load(path: &Path) -> Result<Document, String> {
    let mut reader = BufReader::new(mcp_io::open(path).map_err(io_text)?);
    let file_size = reader.get_ref().metadata().map_err(io_text)?.len();
    let mut header = [0; HEADER_SIZE];
    read(&mut reader, &mut header)?;
    let kind = word(&header, 4);
    if !matches!(kind, FLI | FLC) || !matches!(word(&header, 12), 0 | 8) {
        return Err("Unsupported FLIC signature or color depth".into());
    }
    let declared_size = u64::from(dword(&header, 0));
    let end = if declared_size == 0 {
        file_size
    } else {
        declared_size
    };
    if end < HEADER_SIZE as u64 || end > file_size {
        return Err("Invalid FLIC file size".into());
    }
    let count = usize::from(word(&header, 6));
    let (mut width, mut height) = (
        usize::from(word(&header, 8)),
        usize::from(word(&header, 10)),
    );
    if kind == FLI && width == 0 && height == 0 {
        (width, height) = (320, 200);
    }
    let pixels = width.saturating_mul(height);
    if count == 0
        || width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || pixels > MAX_PIXELS
        || pixels.saturating_mul(count) > MAX_DOCUMENT_PIXELS
    {
        return Err("FLIC canvas or frame count exceeds image limits".into());
    }
    let duration = if kind == FLI {
        (u32::from(word(&header, 16)) * 1000 + 35) / 70
    } else {
        dword(&header, 16)
    };
    let duration = if duration == 0 {
        DEFAULT_DURATION
    } else {
        duration.min(u16::MAX as u32) as u16
    };
    let mut doc = Document::new(width, height);
    doc.color_mode = ColorMode::Indexed;
    let layer = &mut doc.layers[0];
    layer.background = true;
    layer.cels.clear();
    layer.cel_meta.clear();
    layer.tile_cels.resize_with(count, Default::default);
    doc.durations.clear();
    let mut indices = Arc::new(vec![0; pixels]);
    let mut palette = [[0, 0, 0, 255]; 256];
    let mut previous_palette = palette;
    let mut offset = HEADER_SIZE as u64;
    while doc.durations.len() < count {
        let chunk = chunk_header(&mut reader, offset, end)?;
        offset = chunk.end;
        if chunk.kind != FRAME {
            reader.seek(SeekFrom::Start(offset)).map_err(io_text)?;
            continue;
        }
        let mut frame_header = [0; 10];
        if chunk.payload_len < frame_header.len() as u64 {
            return Err("Invalid FLIC frame header".into());
        }
        read(&mut reader, &mut frame_header)?;
        let from = offset - chunk.payload_len + 10;
        decode_frame(
            &mut reader,
            from,
            offset,
            word(&frame_header, 0),
            [width, height],
            &mut indices,
            &mut palette,
        )?;

        if doc.durations.is_empty() {
            doc.palette = palette.to_vec();
        }
        if !doc.frame_palettes.is_empty() || doc.palette.as_slice() != palette {
            doc.frame_palettes
                .resize_with(doc.durations.len(), || doc.palette.clone());
            doc.frame_palettes.push(palette.to_vec());
        }
        let previous = layer.cel_meta.last().zip(layer.cels.last());
        let rgba = match previous {
            Some((meta, pixels))
                if palette == previous_palette
                    && meta
                        .indices
                        .as_ref()
                        .is_some_and(|old| Arc::ptr_eq(old, &indices)) =>
            {
                pixels.clone()
            }
            _ => indices
                .iter()
                .map(|&index| palette[index as usize])
                .collect(),
        };
        layer.cels.push(rgba);
        layer.cel_meta.push(CelMeta {
            indices: Some(indices.clone()),
            ..Default::default()
        });
        previous_palette = palette;
        let delay = word(&frame_header, 2);
        doc.durations.push(if kind == FLC && delay != 0 {
            delay
        } else {
            duration
        });
        reader.seek(SeekFrom::Start(offset)).map_err(io_text)?;
    }
    doc.validate()?;
    Ok(doc)
}

struct Chunk {
    kind: u16,
    end: u64,
    payload_len: u64,
}

fn chunk_header(reader: &mut impl Read, start: u64, limit: u64) -> Result<Chunk, String> {
    if limit.saturating_sub(start) < 6 {
        return Err("Truncated FLIC chunk header".into());
    }
    let mut header = [0; 6];
    read(reader, &mut header)?;
    let size = u64::from(dword(&header, 0));
    if size < 6 || size > limit - start {
        return Err("FLIC chunk exceeds its enclosing file or frame".into());
    }
    Ok(Chunk {
        kind: word(&header, 4),
        end: start + size,
        payload_len: size - 6,
    })
}

fn decode_frame(
    reader: &mut (impl Read + Seek),
    mut offset: u64,
    end: u64,
    chunks: u16,
    [width, height]: [usize; 2],
    indices: &mut Arc<Vec<u8>>,
    palette: &mut [Pixel; 256],
) -> Result<(), String> {
    for _ in 0..chunks {
        let chunk = chunk_header(reader, offset, end)?;
        offset = chunk.end;
        let mut payload = (&mut *reader).take(chunk.payload_len);
        match chunk.kind {
            4 | 11 => read_palette(&mut payload, palette, chunk.kind == 11)?,
            7 => word_delta(&mut payload, Arc::make_mut(indices).as_mut_slice(), width)?,
            12 => line_delta(
                &mut payload,
                Arc::make_mut(indices).as_mut_slice(),
                width,
                height,
            )?,
            13 => Arc::make_mut(indices).as_mut_slice().fill(0),
            15 => byte_run(&mut payload, Arc::make_mut(indices).as_mut_slice(), width)?,
            16 => read(&mut payload, Arc::make_mut(indices).as_mut_slice())?,
            _ => {} // Optional thumbnail and metadata chunks.
        }
        reader.seek(SeekFrom::Start(offset)).map_err(io_text)?;
    }
    Ok(())
}

fn read_palette(
    reader: &mut impl Read,
    palette: &mut [Pixel; 256],
    six_bit: bool,
) -> Result<(), String> {
    let mut index = 0;
    for _ in 0..next_word(reader)? {
        index += usize::from(byte(reader)?);
        let count = match byte(reader)? {
            0 => 256,
            count => usize::from(count),
        };
        let entries = palette
            .get_mut(index..index + count)
            .ok_or("FLIC palette packet exceeds 256 entries")?;
        for entry in entries {
            let mut rgb = [0; 3];
            read(reader, &mut rgb)?;
            if six_bit {
                rgb = rgb.map(|value| {
                    let value = value.min(63);
                    (value << 2) | (value >> 4)
                });
            }
            *entry = [rgb[0], rgb[1], rgb[2], 255];
        }
        index += count;
    }
    Ok(())
}

fn byte_run(reader: &mut impl Read, indices: &mut [u8], width: usize) -> Result<(), String> {
    for row in indices.chunks_exact_mut(width) {
        byte(reader)?; // Newer encoders leave the legacy packet count unset.
        let mut x = 0;
        while x < width {
            let length = byte(reader)? as i8;
            if length == 0 {
                return Err("Invalid zero-length FLIC scanline packet".into());
            }
            let target = patch(row, &mut x, usize::from(length.unsigned_abs()))?;
            if length > 0 {
                target.fill(byte(reader)?);
            } else {
                read(reader, target)?;
            }
        }
    }
    Ok(())
}

fn line_delta(
    reader: &mut impl Read,
    indices: &mut [u8],
    width: usize,
    height: usize,
) -> Result<(), String> {
    let first = usize::from(next_word(reader)?);
    let count = usize::from(next_word(reader)?);
    if first + count > height {
        return Err("FLIC delta exceeds the canvas height".into());
    }
    for y in first..first + count {
        let row = &mut indices[y * width..(y + 1) * width];
        let mut x = 0;
        for _ in 0..byte(reader)? {
            x += usize::from(byte(reader)?);
            let length = byte(reader)? as i8;
            let target = patch(row, &mut x, usize::from(length.unsigned_abs()))?;
            if length < 0 {
                target.fill(byte(reader)?);
            } else {
                read(reader, target)?;
            }
        }
    }
    Ok(())
}

fn word_delta(reader: &mut impl Read, indices: &mut [u8], width: usize) -> Result<(), String> {
    let mut remaining = next_word(reader)?;
    let height = indices.len() / width;
    let mut y = 0;
    while remaining > 0 {
        let opcode = next_word(reader)?;
        if opcode & 0xc000 == 0xc000 {
            y += usize::from((opcode as i16).unsigned_abs());
        }
        if y >= height {
            return Err("FLIC word delta exceeds the canvas height".into());
        }
        match opcode & 0xc000 {
            0xc000 => {}
            0x8000 => indices[(y + 1) * width - 1] = opcode as u8,
            0x4000 => return Err("Invalid FLIC word delta opcode".into()),
            _ => {
                let row = &mut indices[y * width..(y + 1) * width];
                let mut x = 0;
                for _ in 0..opcode {
                    x += usize::from(byte(reader)?);
                    let length = byte(reader)? as i8;
                    let target = patch(row, &mut x, usize::from(length.unsigned_abs()) * 2)?;
                    if length < 0 {
                        let mut pair = [0; 2];
                        read(reader, &mut pair)?;
                        for pixels in target.chunks_exact_mut(2) {
                            pixels.copy_from_slice(&pair);
                        }
                    } else {
                        read(reader, target)?;
                    }
                }
                y += 1;
                remaining -= 1;
            }
        }
    }
    Ok(())
}

fn patch<'a>(row: &'a mut [u8], x: &mut usize, len: usize) -> Result<&'a mut [u8], String> {
    let end = *x + len;
    let target = row
        .get_mut(*x..end)
        .ok_or("FLIC scanline packet exceeds the canvas width")?;
    *x = end;
    Ok(target)
}

fn byte(reader: &mut impl Read) -> Result<u8, String> {
    let mut value = [0];
    read(reader, &mut value)?;
    Ok(value[0])
}

fn next_word(reader: &mut impl Read) -> Result<u16, String> {
    let mut value = [0; 2];
    read(reader, &mut value)?;
    Ok(u16::from_le_bytes(value))
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn dword(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read(reader: &mut impl Read, dst: &mut [u8]) -> Result<(), String> {
    reader
        .read_exact(dst)
        .map_err(|err| format!("Truncated FLIC data: {err}"))
}

fn io_text(error: std::io::Error) -> String {
    error.to_string()
}
