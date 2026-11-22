use crate::{
    model::{ColorMode, Document, MAX_DIMENSION, MAX_PIXELS, Pixel},
    storage::mcp_io,
};
use image::RgbaImage;
use std::{
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

const HEADER_LEN: usize = 128;
const VGA_PALETTE_LEN: usize = 769;

pub(super) fn load(path: &Path) -> Result<Document, String> {
    let mut reader = BufReader::new(mcp_io::open(path).map_err(io_error)?);
    let mut header = [0; HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|_| "Truncated PCX header")?;
    if header[0] != 10 || !matches!(header[2], 0 | 1) {
        return Err("Invalid PCX signature or encoding".into());
    }
    let word = |offset| u16::from_le_bytes([header[offset], header[offset + 1]]) as usize;
    let dimension = |min, max| {
        word(max)
            .checked_sub(word(min))
            .map(|span| span + 1)
            .ok_or("Invalid PCX canvas bounds")
    };
    let (width, height) = (dimension(4, 8)?, dimension(6, 10)?);
    let bits = usize::from(header[3]);
    let planes = usize::from(header[65]);
    let direct_rgb = (bits, planes) == (8, 3);
    let vga = (bits, planes) == (8, 1);
    let indexed = matches!(bits, 1 | 2 | 4) && planes > 0 && bits * planes <= 4 || vga;
    if !indexed && !direct_rgb {
        return Err("Unsupported PCX bit depth or plane count".into());
    }
    let stride = word(66);
    let line_len = stride * planes;
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
        || stride < (width * bits).div_ceil(8)
        || line_len.saturating_mul(height) > (MAX_PIXELS + MAX_DIMENSION) * 3
    {
        return Err("PCX canvas or scanline exceeds image limits".into());
    }

    let mut end = reader.seek(SeekFrom::End(0)).map_err(io_error)?;
    let palette = if vga {
        let mut trailer = [0; VGA_PALETTE_LEN];
        if end >= (HEADER_LEN + VGA_PALETTE_LEN) as u64 {
            reader
                .seek(SeekFrom::End(-(VGA_PALETTE_LEN as i64)))
                .map_err(io_error)?;
            reader.read_exact(&mut trailer).map_err(io_error)?;
        }
        if trailer[0] == 12 {
            end -= VGA_PALETTE_LEN as u64;
            palette_entries(&trailer[1..])
        } else if word(68) == 2 {
            (0..=255).map(|value| [value, value, value, 255]).collect()
        } else {
            return Err("Missing PCX VGA palette".into());
        }
    } else if indexed {
        let count = 1 << (bits * planes);
        let mut palette = palette_entries(&header[16..16 + count * 3]);
        if count == 2 && palette.iter().all(|color| color[..3] == [0, 0, 0]) {
            palette[1] = [255; 4];
        }
        palette
    } else {
        Vec::new()
    };

    reader
        .seek(SeekFrom::Start(HEADER_LEN as u64))
        .map_err(io_error)?;
    let mut pixels = reader.take(end - HEADER_LEN as u64);
    let mut row = vec![0; line_len];
    let mut decoded = Vec::with_capacity(width * height);
    let mut indices = if indexed {
        Vec::with_capacity(width * height)
    } else {
        Vec::new()
    };
    let mask = ((1u16 << bits) - 1) as u8;
    for _ in 0..height {
        read_scanline(&mut pixels, &mut row, header[2] == 1)?;
        for x in 0..width {
            if direct_rgb {
                decoded.push([row[x], row[stride + x], row[2 * stride + x], 255]);
            } else {
                let shift = 8 - bits - (x * bits) % 8;
                let index = (0..planes).fold(0, |index, plane| {
                    let value = row[plane * stride + x * bits / 8] >> shift & mask;
                    index | value << (plane * bits)
                });
                decoded.push(palette[usize::from(index)]);
                indices.push(index);
            }
        }
    }
    let mut doc = Document::new(width, height);
    doc.layers[0].cels[0] = decoded.into();
    if indexed {
        doc.color_mode = ColorMode::Indexed;
        doc.palette = palette;
        // PCX palette slots are opaque, including slot zero.
        doc.layers[0].background = true;
        doc.layers[0].cel_meta[0].indices = Some(indices.into());
    }
    doc.validate()?;
    Ok(doc)
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

fn palette_entries(bytes: &[u8]) -> Vec<Pixel> {
    bytes
        .chunks_exact(3)
        .map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}

fn read_scanline(reader: &mut impl Read, row: &mut [u8], rle: bool) -> Result<(), String> {
    let truncated = "Truncated PCX scanline";
    if !rle {
        return reader.read_exact(row).map_err(|_| truncated.into());
    }
    let mut packet = [0];
    let mut position = 0;
    while position < row.len() {
        reader.read_exact(&mut packet).map_err(|_| truncated)?;
        let (length, value) = if packet[0] >= 0xc0 {
            let length = usize::from(packet[0] & 0x3f);
            if length == 0 || length > row.len() - position {
                return Err("Invalid PCX run length in scanline".into());
            }
            reader.read_exact(&mut packet).map_err(|_| truncated)?;
            (length, packet[0])
        } else {
            (1, packet[0])
        };
        row[position..position + length].fill(value);
        position += length;
    }
    Ok(())
}

pub(super) fn write_indexed(
    path: &Path,
    doc: &Document,
    frame: usize,
    scale: usize,
) -> Result<bool, String> {
    let [layer] = doc.layers.as_slice() else {
        return Ok(false);
    };
    if doc.color_mode != ColorMode::Indexed
        || !layer.visible
        || layer.group
        || layer.tileset.is_some()
    {
        return Ok(false);
    }
    let Some(indices) = &layer.cel_meta[frame].indices else {
        return Ok(false);
    };
    let palette = doc.palette_for(frame);
    if palette.len() > 256
        || doc
            .composite(frame)
            .iter()
            .zip(indices.iter())
            .any(|(pixel, &index)| {
                pixel[3] != 255 || palette.get(usize::from(index)) != Some(pixel)
            })
    {
        return Ok(false);
    }
    write(
        path,
        doc.width * scale,
        doc.height * scale,
        Some(palette),
        |x, y, _| indices[y / scale * doc.width + x / scale],
    )?;
    Ok(true)
}

pub(super) fn write_rgb(path: &Path, image: &RgbaImage) -> Result<(), String> {
    write(
        path,
        image.width() as usize,
        image.height() as usize,
        None,
        |x, y, plane| {
            let color = image.get_pixel(x as u32, y as u32).0;
            super::raster::on_white(color[plane], color[3])
        },
    )
}

fn write(
    path: &Path,
    width: usize,
    height: usize,
    palette: Option<&[Pixel]>,
    sample: impl Fn(usize, usize, usize) -> u8,
) -> Result<(), String> {
    let mut header = [0; HEADER_LEN];
    header[..4].copy_from_slice(&[10, 5, 1, 8]);
    let planes = if palette.is_some() { 1 } else { 3 };
    header[65] = planes;
    let stride = width.next_multiple_of(2);
    for (offset, value) in [(8, width - 1), (10, height - 1), (66, stride), (68, 1)] {
        header[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes());
    }
    let mut writer = BufWriter::new(mcp_io::create(path).map_err(io_error)?);
    writer.write_all(&header).map_err(io_error)?;
    let mut row = vec![0; stride];
    let mut encoded = Vec::with_capacity(stride * 2);
    for y in 0..height {
        for plane in 0..usize::from(planes) {
            for (x, value) in row[..width].iter_mut().enumerate() {
                *value = sample(x, y, plane);
            }
            encoded.clear();
            encode_scanline(&row, &mut encoded);
            writer.write_all(&encoded).map_err(io_error)?;
        }
    }
    if let Some(palette) = palette {
        let mut trailer = [0; VGA_PALETTE_LEN];
        trailer[0] = 12;
        for (entry, color) in trailer[1..].chunks_exact_mut(3).zip(palette) {
            entry.copy_from_slice(&color[..3]);
        }
        writer.write_all(&trailer).map_err(io_error)?;
    }
    writer.flush().map_err(io_error)
}

fn encode_scanline(mut data: &[u8], output: &mut Vec<u8>) {
    while let Some(&byte) = data.first() {
        let length = data
            .iter()
            .take(63)
            .take_while(|&&value| value == byte)
            .count();
        if length > 1 || byte >= 0xc0 {
            output.push(0xc0 | length as u8);
        }
        output.push(byte);
        data = &data[length..];
    }
}
