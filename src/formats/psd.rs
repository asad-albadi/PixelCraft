use crate::{automation::mcp_io, model::*};
use flate2::read::ZlibDecoder;
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

pub(super) fn load(path: &Path) -> Result<Document, String> {
    let file = mcp_io::open(path).map_err(io_text)?;
    let end = file.metadata().map_err(io_text)?.len();
    let mut input = Input {
        file: BufReader::new(file),
        position: 0,
        end,
    };
    if input.array::<4>()? != *b"8BPS" {
        return Err("Invalid Photoshop file signature".into());
    }
    let version = input.u16()?;
    if !matches!(version, 1 | 2) {
        return Err("Unsupported Photoshop file version".into());
    }
    let large = version == 2;
    input.skip(6)?;
    let channels = usize::from(input.u16()?);
    let height = input.u32()? as usize;
    let width = input.u32()? as usize;
    let depth = SampleDepth::parse(input.u16()?)?;
    let mode = input.u16()?;
    if !matches!(mode, 1..=3) {
        return Err("Photoshop import supports RGB, grayscale, and indexed documents".into());
    }
    let indexed = mode == 2;
    if indexed && depth != SampleDepth::Byte {
        return Err("Indexed Photoshop palettes require 8-bit channels".into());
    }
    let colors = if mode == 3 { 3 } else { 1 };
    let encoding = ChannelEncoding {
        large,
        depth,
        color_planes: colors,
    };
    if channels < colors
        || channels > 56
        || width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("Photoshop document exceeds canvas or channel limits".into());
    }
    let mut doc = Document::new(width, height);
    let color_data = input.length(false)?;
    let transparent = if indexed {
        if color_data != 768 {
            return Err("Invalid Photoshop indexed color table size".into());
        }
        let table = input.array::<768>()?;
        let resources = input.length(false)?;
        let (count, transparent) = input.section(resources, indexed_resources)?;
        doc.color_mode = ColorMode::Indexed;
        doc.palette = (0..count)
            .map(|i| [table[i], table[256 + i], table[512 + i], 255])
            .collect();
        doc.transparent_index = transparent.map_or(0, usize::from);
        transparent
    } else {
        input.skip(color_data)?;
        let resources = input.length(false)?;
        input.skip(resources)?;
        None
    };
    let section = input.length(large)?;
    if mode == 1 {
        doc.color_mode = ColorMode::Grayscale;
    }
    let layers = if section > 0 {
        input.section(section, |input| {
            read_layer_section(input, &mut doc, transparent, encoding)
        })?
    } else {
        ImportedLayers::None
    };
    match layers {
        ImportedLayers::Editable => {}
        ImportedLayers::None => {
            let palette = indexed.then_some(doc.palette.as_slice());
            decode_merged(
                &mut input,
                [width, height],
                channels,
                encoding,
                palette,
                transparent,
                &mut doc.layers[0],
            )?;
        }
        ImportedLayers::NeedsPreview if input.end - input.position >= 2 => {
            let mut preview = Layer::new("Photoshop merged preview".into(), width, height, 1);
            let palette = indexed.then_some(doc.palette.as_slice());
            decode_merged(
                &mut input,
                [width, height],
                channels,
                encoding,
                palette,
                transparent,
                &mut preview,
            )?;
            let group_index = doc.layers.len();
            for layer in &mut doc.layers {
                if layer.parent.is_none() {
                    layer.parent = Some(group_index);
                }
            }
            let mut group = Layer::new("Editable Photoshop layers".into(), width, height, 1);
            group.group = true;
            group.visible = false;
            doc.layers.push(group);
            doc.layers.push(preview);
        }
        ImportedLayers::NeedsPreview => {}
    }
    doc.validate()?;
    Ok(doc)
}

fn indexed_resources(input: &mut Input) -> Result<(usize, Option<u8>), String> {
    let mut count = 256;
    let mut transparent = None;
    while input.position < input.end {
        if input.array::<4>()? != *b"8BIM" {
            return Err("Invalid Photoshop image resource signature".into());
        }
        let id = input.u16()?;
        let name = u64::from(input.byte()?) + 1;
        input.skip(name.next_multiple_of(2) - 1)?;
        let length = input.length(false)?;
        input.section(length, |input| {
            match id {
                1046 => {
                    count = usize::from(input.u16()?);
                    if count == 0 || count > 256 {
                        return Err("Invalid Photoshop indexed color count".into());
                    }
                }
                1047 => transparent = u8::try_from(input.u16()?).ok(),
                _ => {}
            }
            Ok(())
        })?;
        input.skip(length % 2)?;
    }
    Ok((count, transparent.filter(|&slot| usize::from(slot) < count)))
}

fn decode_merged(
    input: &mut Input,
    [width, height]: [usize; 2],
    channels: usize,
    encoding: ChannelEncoding,
    palette: Option<&[Pixel]>,
    transparent: Option<u8>,
    layer: &mut Layer,
) -> Result<(), String> {
    let colors = encoding.color_planes;
    let compression = input.u16()?;
    let mut planes = decode_planes(
        input,
        compression,
        [width, height],
        channels,
        if palette.is_some() {
            1
        } else {
            channels.min(colors + 1)
        },
        encoding,
    )?;
    if let Some(palette) = palette {
        let indices = planes.pop().unwrap();
        layer.cels[0] = indices
            .iter()
            .map(|&slot| indexed_pixel(palette, transparent, slot, 255))
            .collect::<Result<Vec<_>, _>>()?
            .into();
        layer.cel_meta[0].indices = Some(indices.into());
        layer.background = transparent.is_none();
    } else {
        layer.cels[0] = (0..width * height)
            .map(|i| color_at(&planes, colors, i))
            .collect();
    }
    Ok(())
}

fn indexed_pixel(
    palette: &[Pixel],
    transparent: Option<u8>,
    slot: u8,
    alpha: u8,
) -> Result<Pixel, String> {
    let mut pixel = *palette
        .get(usize::from(slot))
        .ok_or("Photoshop pixel uses an undefined palette slot")?;
    pixel[3] = if Some(slot) == transparent { 0 } else { alpha };
    Ok(pixel)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportedLayers {
    None,
    Editable,
    NeedsPreview,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Bounds {
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

impl Bounds {
    fn read(input: &mut Input) -> Result<Self, String> {
        let (y, x, bottom, right) = (input.i32()?, input.i32()?, input.i32()?, input.i32()?);
        let width = i64::from(right) - i64::from(x);
        let height = i64::from(bottom) - i64::from(y);
        if width < 0
            || height < 0
            || width > MAX_DIMENSION as i64
            || height > MAX_DIMENSION as i64
            || width * height > MAX_PIXELS as i64
        {
            return Err("Photoshop layer or mask bounds exceed image limits".into());
        }
        Ok(Self {
            x,
            y,
            width: width as usize,
            height: height as usize,
        })
    }
    fn size(self) -> [usize; 2] {
        [self.width, self.height]
    }
}

struct Mask {
    bounds: Bounds,
    default: u8,
    flags: u8,
    density: u8,
}
impl Mask {
    fn read(input: &mut Input) -> Result<Self, String> {
        let bounds = Bounds::read(input)?;
        let default = input.byte()?;
        let flags = input.byte()?;
        let density = if flags & 16 != 0 && input.byte()? & 1 != 0 {
            input.byte()?
        } else {
            255
        };
        Ok(Self {
            bounds,
            default,
            flags,
            density,
        })
    }
    fn value(&self, x: usize, y: usize, pixels: &[u8]) -> u8 {
        let x = x as i64 - i64::from(self.bounds.x);
        let y = y as i64 - i64::from(self.bounds.y);
        let value = if (0..self.bounds.width as i64).contains(&x)
            && (0..self.bounds.height as i64).contains(&y)
        {
            pixels[y as usize * self.bounds.width + x as usize]
        } else {
            self.default
        };
        let value = if self.flags & 4 != 0 {
            255 - value
        } else {
            value
        };
        255 - ((u16::from(255 - value) * u16::from(self.density) + 127) / 255) as u8
    }
}

struct Record {
    bounds: Bounds,
    channels: Vec<(i16, u64)>,
    name: String,
    group: u32,
    visible: bool,
    opacity: u8,
    blend: BlendMode,
    mask: Option<Mask>,
    has_fill: bool,
}

impl Record {
    fn read(input: &mut Input, large: bool) -> Result<Self, String> {
        let bounds = Bounds::read(input)?;
        let count = input.u16()?;
        if count > 56 {
            return Err("Photoshop layer has too many channels".into());
        }
        let channels = (0..count)
            .map(|_| Ok((input.i16()?, input.length(large)?)))
            .collect::<Result<Vec<_>, String>>()?;
        if input.array::<4>()? != *b"8BIM" {
            return Err("Invalid Photoshop layer signature".into());
        }
        let blend = blend_mode(input.array()?);
        let [opacity, _, flags, _] = input.array::<4>()?;
        let length = input.length(false)?;
        let (mask, name, group, has_fill) = input.section(length, |input| {
            let mask_length = input.length(false)?;
            let mask = if mask_length > 0 {
                Some(input.section(mask_length, Mask::read)?)
            } else {
                None
            };
            let ranges = input.length(false)?;
            input.skip(ranges)?;
            let length = usize::from(input.byte()?);
            let mut name = vec![0; length];
            input.read_exact(&mut name).map_err(io_text)?;
            input.skip(((length + 1).next_multiple_of(4) - length - 1) as u64)?;
            let mut name = String::from_utf8_lossy(&name).into_owned();
            let mut group = 0;
            let mut has_fill = false;
            while let Some((key, length)) = input.tagged_header(large)? {
                input.section(length, |input| {
                    match &key {
                        b"luni" => {
                            let len = input.u32()? as usize;
                            if len > 65535 || (len as u64) * 2 > input.end - input.position {
                                return Err("Invalid Photoshop Unicode layer name".into());
                            }
                            let chars = (0..len)
                                .map(|_| input.u16())
                                .collect::<Result<Vec<_>, _>>()?;
                            name = String::from_utf16_lossy(&chars)
                                .trim_end_matches('\0')
                                .to_owned();
                        }
                        b"lsct" | b"lsdk" => group = input.u32()?,
                        b"SoCo" | b"GdFl" | b"PtFl" => has_fill = true,
                        _ => {}
                    }
                    Ok(())
                })?;
                if length % 2 == 1 && input.position < input.end {
                    input.skip(1)?;
                }
            }
            Ok((mask, name, group, has_fill))
        })?;
        Ok(Self {
            bounds,
            channels,
            name,
            group,
            visible: flags & 2 == 0,
            opacity,
            blend,
            mask,
            has_fill,
        })
    }
}

fn read_layer_section(
    input: &mut Input,
    doc: &mut Document,
    transparent: Option<u8>,
    encoding: ChannelEncoding,
) -> Result<ImportedLayers, String> {
    let length = input.length(encoding.large)?;
    let start = input.position;
    if encoding.depth == SampleDepth::Byte && length != 0 {
        let decoded = input.section(length, |input| {
            read_layers(input, doc, transparent, encoding)
        })?;
        if decoded != ImportedLayers::None {
            return Ok(decoded);
        }
    } else {
        input.skip(length)?;
    }
    if input.end - input.position >= 4 {
        let mask_len = input.length(false)?;
        input.skip(mask_len)?;
        let wanted = match encoding.depth {
            SampleDepth::Byte => b"Layr",
            SampleDepth::Word => b"Lr16",
            SampleDepth::Float => b"Lr32",
        };
        while let Some((key, length)) = input.tagged_header(encoding.large)? {
            if &key == wanted {
                let decoded = input.section(length, |input| {
                    read_layers(input, doc, transparent, encoding)
                })?;
                if decoded != ImportedLayers::None {
                    return Ok(decoded);
                }
            } else {
                input.skip(length)?;
            }
            let padding = (4 - length % 4) % 4;
            input.skip(padding.min(input.end - input.position))?;
        }
    }
    if encoding.depth != SampleDepth::Byte && length != 0 {
        input.seek_to(start)?;
        return input.section(length, |input| {
            read_layers(input, doc, transparent, encoding)
        });
    }
    Ok(ImportedLayers::None)
}

fn read_layers(
    input: &mut Input,
    doc: &mut Document,
    transparent: Option<u8>,
    encoding: ChannelEncoding,
) -> Result<ImportedLayers, String> {
    let colors = encoding.color_planes;
    let indexed = doc.color_mode == ColorMode::Indexed;
    let count = input.i16()?.unsigned_abs() as usize;
    if count == 0 {
        return Ok(ImportedLayers::None);
    }
    let records = (0..count)
        .map(|_| Record::read(input, encoding.large))
        .collect::<Result<Vec<_>, _>>()?;
    let mut index_map = vec![None; count];
    let mut retained = 0;
    for (i, record) in records.iter().enumerate() {
        if record.group != 3 {
            index_map[i] = Some(retained);
            retained += 1;
        }
    }
    let area = doc.width * doc.height;
    let masks = records
        .iter()
        .filter(|record| {
            record.mask.as_ref().is_some_and(|mask| mask.flags & 2 == 0)
                && record.channels.iter().any(|&(id, _)| id == -2)
        })
        .count();
    let channel_work = records.iter().fold(0usize, |total, record| {
        total.saturating_add(
            record
                .bounds
                .width
                .saturating_mul(record.bounds.height)
                .saturating_mul(record.channels.len().min(colors + 1)),
        )
    });
    if retained == 0 {
        return Ok(ImportedLayers::None);
    }
    let indexed_storage = if indexed {
        area.div_ceil(4).saturating_mul(retained)
    } else {
        0
    };
    if area
        .saturating_mul(retained + masks)
        .saturating_add(indexed_storage)
        > MAX_DOCUMENT_PIXELS
        || channel_work > MAX_DOCUMENT_PIXELS * 4
    {
        return Err("Photoshop layers exceed the pixel work limit".into());
    }
    let mut parents = vec![None; count];
    let mut groups = Vec::new();
    let mut visible = vec![false; count];
    for (i, record) in records.iter().enumerate().rev() {
        if record.group == 3 {
            groups.pop().ok_or("Unbalanced Photoshop layer groups")?;
        } else {
            parents[i] = groups.last().map(|&(index, _)| index);
            visible[i] = record.visible && groups.last().is_none_or(|&(_, visible)| visible);
            if matches!(record.group, 1 | 2) {
                groups.push((index_map[i].unwrap(), visible[i]));
            }
        }
    }
    if !groups.is_empty() {
        return Err("Unbalanced Photoshop layer groups".into());
    }

    doc.layers.clear();
    let mut needs_preview = false;
    for ((record, parent), visible) in records.into_iter().zip(parents).zip(visible) {
        let mut layer = Layer::new(record.name, doc.width, doc.height, 1);
        layer.parent = parent;
        layer.group = matches!(record.group, 1 | 2);
        layer.visible = record.visible;
        layer.opacity = record.opacity;
        layer.blend = record.blend;
        let mut planes = vec![Vec::new(); colors + 1];
        let mut mask_pixels = None;
        for &(id, length) in &record.channels {
            input.section(length, |input| {
                if record.group == 3 || length <= 2 {
                    return Ok(());
                }
                let (size, target) = match id {
                    -2 => match record.mask.as_ref() {
                        Some(mask) if mask.flags & 2 == 0 => (mask.bounds.size(), None),
                        _ => return Ok(()),
                    },
                    -1 if !layer.group => (record.bounds.size(), Some(colors)),
                    id if id >= 0 && (id as usize) < colors && !layer.group => {
                        (record.bounds.size(), Some(id as usize))
                    }
                    _ => return Ok(()),
                };
                if size.contains(&0) {
                    return Ok(());
                }
                let compression = input.u16()?;
                let encoding = ChannelEncoding {
                    color_planes: usize::from(target.is_some_and(|index| index < colors)),
                    ..encoding
                };
                let plane = decode_planes(input, compression, size, 1, 1, encoding)?
                    .pop()
                    .unwrap();
                if let Some(index) = target {
                    planes[index] = plane;
                } else {
                    mask_pixels = Some(plane);
                }
                Ok(())
            })?;
        }
        if record.group == 3 {
            continue;
        }
        if let (Some(mask), Some(pixels)) = (record.mask, mask_pixels)
            && !(mask.flags & 8 != 0 && mask.bounds == record.bounds && pixels == planes[colors])
        {
            layer.mask = Some(
                (0..area)
                    .map(|i| [0, 0, 0, mask.value(i % doc.width, i / doc.width, &pixels)])
                    .collect(),
            );
        }
        let raster = planes[..colors].iter().all(|plane| !plane.is_empty());
        if !layer.group
            && visible
            && !raster
            && (record.has_fill || !record.bounds.size().contains(&0))
        {
            needs_preview = true;
        }
        if !layer.group && raster {
            let bounds = record.bounds;
            let mut indices = indexed.then(|| vec![transparent.unwrap_or(0); area]);
            layer.background = indexed
                && transparent.is_none()
                && planes[colors].is_empty()
                && bounds.x == 0
                && bounds.y == 0
                && bounds.size() == [doc.width, doc.height];
            for y in 0..bounds.height {
                for x in 0..bounds.width {
                    let source = y * bounds.width + x;
                    let pixel = if indexed {
                        let alpha = planes[colors].get(source).copied().unwrap_or(255);
                        indexed_pixel(&doc.palette, transparent, planes[0][source], alpha)?
                    } else {
                        color_at(&planes, colors, source)
                    };
                    let px = bounds.x.saturating_add(x as i32);
                    let py = bounds.y.saturating_add(y as i32);
                    if let Some(index) = doc.index(px, py) {
                        if pixel != CLEAR {
                            layer.cels[0].set(index, pixel);
                        }
                        if let Some(indices) = &mut indices {
                            indices[index] = planes[0][source];
                        }
                    } else if pixel[3] != 0 {
                        layer.cel_meta[0].outside.push(OutsidePixel {
                            x: px,
                            y: py,
                            color: pixel,
                        });
                    }
                }
            }
            layer.cel_meta[0].indices = indices.map(Into::into);
        }
        doc.layers.push(layer);
    }
    Ok(if needs_preview {
        ImportedLayers::NeedsPreview
    } else {
        ImportedLayers::Editable
    })
}

fn color_at(planes: &[Vec<u8>], colors: usize, index: usize) -> Pixel {
    let alpha = planes
        .get(colors)
        .and_then(|plane| plane.get(index))
        .copied()
        .unwrap_or(255);
    let red = planes[0][index];
    if colors == 3 {
        [red, planes[1][index], planes[2][index], alpha]
    } else {
        [red, red, red, alpha]
    }
}

fn blend_mode(key: [u8; 4]) -> BlendMode {
    use BlendMode::*;
    match &key {
        b"mul " => Multiply,
        b"scrn" => Screen,
        b"over" => Overlay,
        b"dark" => Darken,
        b"lite" => Lighten,
        b"div " => ColorDodge,
        b"idiv" => ColorBurn,
        b"hLit" => HardLight,
        b"sLit" => SoftLight,
        b"diff" => Difference,
        b"smud" => Exclusion,
        b"hue " => Hue,
        b"sat " => Saturation,
        b"colr" => Color,
        b"lum " => Luminosity,
        b"lddg" => Addition,
        b"fsub" => Subtract,
        b"fdiv" => Divide,
        _ => Normal,
    }
}

#[derive(Clone, Copy)]
struct ChannelEncoding {
    large: bool,
    depth: SampleDepth,
    color_planes: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SampleDepth {
    Byte,
    Word,
    Float,
}

impl SampleDepth {
    fn parse(value: u16) -> Result<Self, String> {
        match value {
            8 => Ok(Self::Byte),
            16 => Ok(Self::Word),
            32 => Ok(Self::Float),
            _ => Err("Unsupported Photoshop channel depth".into()),
        }
    }

    fn bytes(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Word => 2,
            Self::Float => 4,
        }
    }

    fn append_row(
        self,
        row: &mut [u8],
        width: usize,
        predict: bool,
        color: bool,
        out: &mut Vec<u8>,
    ) {
        let sum_bytes = |row: &mut [u8]| {
            for i in 1..row.len() {
                row[i] = row[i].wrapping_add(row[i - 1]);
            }
        };
        match self {
            Self::Byte => {
                if predict {
                    sum_bytes(row);
                }
                out.extend_from_slice(row);
            }
            Self::Word => {
                let mut last = 0u16;
                for sample in row.chunks_exact(2) {
                    let mut value = u16::from_be_bytes(sample.try_into().unwrap());
                    if predict {
                        value = last.wrapping_add(value);
                    }
                    last = value;
                    out.push(((u32::from(value) * 255 + 32767) / 65535) as u8);
                }
            }
            Self::Float => {
                let to_byte = |bytes| {
                    let mut value = f32::from_be_bytes(bytes).clamp(0., 1.);
                    if color {
                        value = if value <= 0.003_130_8 {
                            12.92 * value
                        } else {
                            1.055 * value.powf(1. / 2.4) - 0.055
                        };
                    }
                    (value * 255.0).round() as u8
                };
                if predict {
                    sum_bytes(row);
                    for x in 0..width {
                        out.push(to_byte([
                            row[x],
                            row[x + width],
                            row[x + 2 * width],
                            row[x + 3 * width],
                        ]));
                    }
                } else {
                    out.extend(
                        row.chunks_exact(4)
                            .map(|sample| to_byte(sample.try_into().unwrap())),
                    );
                }
            }
        }
    }
}

fn decode_planes(
    input: &mut Input,
    compression: u16,
    [width, height]: [usize; 2],
    count: usize,
    included: usize,
    encoding: ChannelEncoding,
) -> Result<Vec<Vec<u8>>, String> {
    let area = width * height;
    let mut row = vec![0; width * encoding.depth.bytes()];
    let mut read_plane =
        |plane_index, read_row: &mut dyn FnMut(&mut [u8]) -> Result<(), String>| {
            let mut plane = Vec::with_capacity(area);
            for _ in 0..height {
                read_row(&mut row)?;
                encoding.depth.append_row(
                    &mut row,
                    width,
                    compression == 3,
                    plane_index < encoding.color_planes,
                    &mut plane,
                );
            }
            Ok::<_, String>(plane)
        };
    let mut planes = Vec::with_capacity(included);
    match compression {
        0 => {
            let bytes = (area as u64) * (encoding.depth.bytes() as u64);
            input.end_for(bytes * count as u64)?;
            for index in 0..included {
                planes.push(read_plane(index, &mut |row| {
                    input.read_exact(row).map_err(io_text)
                })?);
            }
            input.skip(bytes * (count - included) as u64)?;
        }
        1 => {
            let sizes = (0..count * height)
                .map(|_| {
                    if encoding.large {
                        input.u32()
                    } else {
                        input.u16().map(u32::from)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            input.end_for(sizes.iter().map(|&size| u64::from(size)).sum())?;
            for (index, rows) in sizes.chunks_exact(height).enumerate() {
                if index >= included {
                    input.skip(rows.iter().map(|&size| u64::from(size)).sum())?;
                } else {
                    let mut sizes = rows.iter().copied();
                    planes.push(read_plane(index, &mut |row| {
                        input.section(u64::from(sizes.next().unwrap()), |input| {
                            unpack_row(input, row)
                        })
                    })?);
                }
            }
        }
        2 | 3 => {
            let mut zip = ZlibDecoder::new(input);
            for index in 0..included {
                planes.push(read_plane(index, &mut |row| {
                    zip.read_exact(row).map_err(io_text)
                })?);
            }
        }
        _ => return Err("Unsupported Photoshop channel compression".into()),
    }
    Ok(planes)
}

fn unpack_row(input: &mut Input, row: &mut [u8]) -> Result<(), String> {
    let mut offset = 0;
    while offset < row.len() {
        let code = input.byte()? as i8;
        if code == -128 {
            continue;
        }
        let count = usize::from(code.unsigned_abs()) + 1;
        let target = row
            .get_mut(offset..offset + count)
            .ok_or("Photoshop PackBits packet exceeds its row")?;
        if code < 0 {
            target.fill(input.byte()?);
        } else {
            input.read_exact(target).map_err(io_text)?;
        }
        offset += count;
    }
    Ok(())
}

struct Input {
    file: BufReader<File>,
    position: u64,
    end: u64,
}

impl Input {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes).map_err(io_text)?;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_be_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    fn length(&mut self, large: bool) -> Result<u64, String> {
        if large {
            Ok(u64::from_be_bytes(self.array()?))
        } else {
            self.u32().map(u64::from)
        }
    }
    fn tagged_header(&mut self, large: bool) -> Result<Option<([u8; 4], u64)>, String> {
        if self.end - self.position < 12 {
            return Ok(None);
        }
        let signature = self.array::<4>()?;
        if signature == [0; 4] {
            return Ok(None);
        }
        if !matches!(&signature, b"8BIM" | b"8B64") {
            return Err("Invalid Photoshop tagged block signature".into());
        }
        let key = self.array::<4>()?;
        let wide = large
            && matches!(
                &key,
                b"LMsk"
                    | b"Lr16"
                    | b"Lr32"
                    | b"Layr"
                    | b"Mt16"
                    | b"Mt32"
                    | b"Mtrn"
                    | b"Alph"
                    | b"FMsk"
                    | b"lnk2"
                    | b"FEid"
                    | b"FXid"
                    | b"PxSD"
            );
        Ok(Some((key, self.length(wide)?)))
    }
    fn seek_to(&mut self, position: u64) -> Result<(), String> {
        if position > self.end {
            return Err("Photoshop seek exceeds the current section".into());
        }
        self.file.seek(SeekFrom::Start(position)).map_err(io_text)?;
        self.position = position;
        Ok(())
    }
    fn end_for(&self, length: u64) -> Result<u64, String> {
        self.position
            .checked_add(length)
            .filter(|&end| end <= self.end)
            .ok_or_else(|| "Photoshop section exceeds its enclosing data".into())
    }
    fn skip(&mut self, length: u64) -> Result<(), String> {
        self.position = self.end_for(length)?;
        if let Ok(relative) = i64::try_from(length) {
            self.file.seek_relative(relative).map_err(io_text)?;
        } else {
            self.file
                .seek(SeekFrom::Start(self.position))
                .map_err(io_text)?;
        }
        Ok(())
    }
    fn section<T>(
        &mut self,
        length: u64,
        decode: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let end = self.end_for(length)?;
        let outer = std::mem::replace(&mut self.end, end);
        let value = decode(self)?;
        self.skip(end - self.position)?;
        self.end = outer;
        Ok(value)
    }
}

impl Read for Input {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = buffer
            .len()
            .min((self.end - self.position).try_into().unwrap_or(usize::MAX));
        let count = self.file.read(&mut buffer[..limit])?;
        self.position += count as u64;
        Ok(count)
    }
}

fn io_text(error: io::Error) -> String {
    error.to_string()
}
