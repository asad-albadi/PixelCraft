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
    let depth = input.u16()?;
    let mode = input.u16()?;
    if depth != 8 || !matches!(mode, 1 | 3) {
        return Err("Photoshop import currently supports 8-bit RGB and grayscale documents".into());
    }
    let colors = if mode == 3 { 3 } else { 1 };
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
    for _ in 0..2 {
        let length = input.length(false)?;
        input.skip(length)?;
    }
    let section = input.length(large)?;
    let mut doc = Document::new(width, height);
    if colors == 1 {
        doc.color_mode = ColorMode::Grayscale;
    }
    if section > 0 {
        let decoded = input.section(section, |input| {
            let layer_info = input.length(large)?;
            if layer_info == 0 {
                return Ok(false);
            }
            input.section(layer_info, |input| {
                read_layers(input, &mut doc, colors, large)
            })
        })?;
        if decoded {
            doc.validate()?;
            return Ok(doc);
        }
    }
    let compression = input.u16()?;
    let included = channels.min(colors + 1);
    let planes = decode_planes(
        &mut input,
        compression,
        [width, height],
        channels,
        included,
        large,
    )?;
    doc.layers[0].cels[0] = (0..width * height)
        .map(|i| color_at(&planes, colors, i))
        .collect();
    doc.validate()?;
    Ok(doc)
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
        let (mask, name, group) = input.section(length, |input| {
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
            while input.end.saturating_sub(input.position) >= 12 {
                let signature = input.array::<4>()?;
                if signature == [0; 4] {
                    break;
                }
                if !matches!(&signature, b"8BIM" | b"8B64") {
                    return Err("Invalid Photoshop tagged block signature".into());
                }
                let key = input.array::<4>()?;
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
                let length = input.length(wide)?;
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
                        _ => {}
                    }
                    Ok(())
                })?;
                if length % 2 == 1 && input.position < input.end {
                    input.skip(1)?;
                }
            }
            Ok((mask, name, group))
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
        })
    }
}

fn read_layers(
    input: &mut Input,
    doc: &mut Document,
    colors: usize,
    large: bool,
) -> Result<bool, String> {
    let count = input.i16()?.unsigned_abs() as usize;
    if count == 0 {
        return Ok(false);
    }
    let records = (0..count)
        .map(|_| Record::read(input, large))
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
        return Ok(false);
    }
    if area.saturating_mul(retained + masks) > MAX_DOCUMENT_PIXELS
        || channel_work > MAX_DOCUMENT_PIXELS * 4
    {
        return Err("Photoshop layers exceed the pixel work limit".into());
    }
    let mut parents = vec![None; count];
    let mut groups = Vec::new();
    for (i, record) in records.iter().enumerate().rev() {
        if record.group == 3 {
            groups.pop().ok_or("Unbalanced Photoshop layer groups")?;
        } else {
            parents[i] = groups.last().copied();
            if matches!(record.group, 1 | 2) {
                groups.push(index_map[i].unwrap());
            }
        }
    }
    if !groups.is_empty() {
        return Err("Unbalanced Photoshop layer groups".into());
    }

    doc.layers.clear();
    for (record, parent) in records.into_iter().zip(parents) {
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
                if record.group == 3 || length == 0 {
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
                let plane = decode_planes(input, compression, size, 1, 1, large)?
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
        if !layer.group && planes[..colors].iter().all(|plane| !plane.is_empty()) {
            let bounds = record.bounds;
            for y in 0..bounds.height {
                for x in 0..bounds.width {
                    let pixel = color_at(&planes, colors, y * bounds.width + x);
                    if pixel == CLEAR {
                        continue;
                    }
                    let px = bounds.x.saturating_add(x as i32);
                    let py = bounds.y.saturating_add(y as i32);
                    if let Some(index) = doc.index(px, py) {
                        layer.cels[0].set(index, pixel);
                    } else {
                        layer.cel_meta[0].outside.push(OutsidePixel {
                            x: px,
                            y: py,
                            color: pixel,
                        });
                    }
                }
            }
        }
        doc.layers.push(layer);
    }
    Ok(true)
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

fn decode_planes(
    input: &mut Input,
    compression: u16,
    [width, height]: [usize; 2],
    count: usize,
    included: usize,
    large: bool,
) -> Result<Vec<Vec<u8>>, String> {
    let area = width * height;
    let mut planes = Vec::with_capacity(included);
    match compression {
        0 => {
            input.end_for((area as u64) * (count as u64))?;
            for _ in 0..included {
                let mut data = vec![0; area];
                input.read_exact(&mut data).map_err(io_text)?;
                planes.push(data);
            }
            input.skip(((count - included) * area) as u64)?;
        }
        1 => {
            let sizes = (0..count * height)
                .map(|_| {
                    if large {
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
                    continue;
                }
                let mut data = vec![0; area];
                for (row, &size) in data.chunks_exact_mut(width).zip(rows) {
                    input.section(u64::from(size), |input| unpack_row(input, row))?;
                }
                planes.push(data);
            }
        }
        2 | 3 => {
            let mut zip = ZlibDecoder::new(input);
            for _ in 0..included {
                let mut data = vec![0; area];
                zip.read_exact(&mut data).map_err(io_text)?;
                if compression == 3 {
                    for row in data.chunks_exact_mut(width) {
                        for i in 1..row.len() {
                            row[i] = row[i].wrapping_add(row[i - 1]);
                        }
                    }
                }
                planes.push(data);
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
