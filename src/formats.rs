use crate::model::*;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::{
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};
const PROJECT_MAGIC_V1: &[u8] = b"PIXEL-WORKSHOP\0\x01";
const PROJECT_MAGIC: &[u8] = b"PIXEL-WORKSHOP\0\x02";
// Four 3-digit channels, commas and brackets can use 18 JSON bytes per pixel.
const MAX_PROJECT_BYTES: u64 = MAX_DOCUMENT_PIXELS as u64 * 18 + 128 * 1024 * 1024;
fn u16at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2)
            .ok_or("Truncated sprite file")?
            .try_into()
            .unwrap(),
    ))
}
fn u32at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4)
            .ok_or("Truncated sprite file")?
            .try_into()
            .unwrap(),
    ))
}
fn word(b: &mut Vec<u8>, v: u16) {
    b.extend(v.to_le_bytes())
}
fn dword(b: &mut Vec<u8>, v: u32) {
    b.extend(v.to_le_bytes())
}
fn string(b: &mut Vec<u8>, v: &str) -> Result<(), String> {
    if v.len() > 65535 {
        return Err("ASE name too long".into());
    }
    word(b, v.len() as u16);
    b.extend(v.as_bytes());
    Ok(())
}
fn read_string(b: &[u8], p: &mut usize) -> Result<String, String> {
    let len = u16at(b, *p)? as usize;
    *p += 2;
    let s = String::from_utf8(b.get(*p..*p + len).ok_or("Truncated string")?.to_vec())
        .map_err(|e| e.to_string())?;
    *p += len;
    Ok(s)
}
fn chunk(kind: u16, data: Vec<u8>) -> Vec<u8> {
    let mut out = vec![];
    dword(&mut out, (data.len() + 6) as u32);
    word(&mut out, kind);
    out.extend(data);
    out
}
fn compress(b: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = ZlibEncoder::new(vec![], Compression::default());
    z.write_all(b).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())
}
fn decompress(b: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    if expected > MAX_PIXELS * 4 {
        return Err("Decompressed image exceeds memory budget".into());
    }
    let mut v = vec![];
    ZlibDecoder::new(b)
        .take(expected as u64 + 1)
        .read_to_end(&mut v)
        .map_err(|e| e.to_string())?;
    if v.len() != expected {
        return Err("Image cel size does not match its pixels".into());
    }
    Ok(v)
}
fn pixels_bytes(p: &[Pixel], mode: ColorMode, palette: &[Pixel]) -> Vec<u8> {
    let mut b = vec![];
    for &p in p {
        match mode {
            ColorMode::Rgba => b.extend(p),
            ColorMode::Grayscale => b.extend([p[0], p[3]]),
            ColorMode::Indexed => b.push(nearest(p, palette) as u8),
        }
    }
    b
}
fn read_pixels(
    b: &[u8],
    depth: usize,
    palette: &[Pixel],
    transparent: usize,
    background: bool,
) -> Result<Vec<Pixel>, String> {
    b.chunks_exact(depth / 8)
        .map(|p| {
            Ok(match depth {
                32 => [p[0], p[1], p[2], p[3]],
                16 => [p[0], p[0], p[0], p[1]],
                8 => {
                    let mut c = *palette.get(p[0] as usize).ok_or("Invalid palette index")?;
                    if p[0] as usize == transparent && !background {
                        c[3] = 0;
                    }
                    c
                }
                _ => return Err("Invalid color depth".into()),
            })
        })
        .collect()
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectV2 {
    document: Document,
    #[serde(default)]
    shared_rasters: Vec<SharedRaster>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SharedRaster {
    source: [usize; 2],
    copies: Vec<[usize; 2]>,
}

impl ProjectV2 {
    fn new(document: &Document) -> Self {
        let mut document = document.clone();
        let mut sources = std::collections::HashMap::new();
        let mut shared_rasters = Vec::<SharedRaster>::new();
        for (layer, entry) in document.layers.iter_mut().enumerate() {
            for (frame, raster) in entry.cels.iter_mut().enumerate() {
                let Some(id) = raster.backing_id() else {
                    continue;
                };
                use std::collections::hash_map::Entry;
                match sources.entry(id) {
                    Entry::Vacant(slot) => {
                        slot.insert(shared_rasters.len());
                        shared_rasters.push(SharedRaster {
                            source: [layer, frame],
                            copies: Vec::new(),
                        });
                    }
                    Entry::Occupied(slot) => {
                        shared_rasters[*slot.get()].copies.push([layer, frame]);
                        *raster = Pixels::empty(raster.len());
                    }
                }
            }
        }
        shared_rasters.retain(|group| !group.copies.is_empty());
        Self {
            document,
            shared_rasters,
        }
    }

    fn restore(mut self) -> Result<Document, String> {
        let invalid = || "Invalid project raster reference".to_string();
        let targets: std::collections::HashSet<_> = self
            .shared_rasters
            .iter()
            .flat_map(|group| group.copies.iter().copied())
            .collect();
        let mut used = std::collections::HashSet::new();
        for group in self.shared_rasters {
            if targets.contains(&group.source) {
                return Err(invalid());
            }
            let [layer, frame] = group.source;
            let raster = self
                .document
                .layers
                .get(layer)
                .and_then(|l| l.cels.get(frame))
                .cloned()
                .ok_or_else(invalid)?;
            for [layer, frame] in group.copies {
                if !used.insert([layer, frame]) {
                    return Err(invalid());
                }
                let target = self
                    .document
                    .layers
                    .get_mut(layer)
                    .and_then(|l| l.cels.get_mut(frame))
                    .ok_or_else(invalid)?;
                if !matches!(target, Pixels::Empty { .. }) || target.len() != raster.len() {
                    return Err(invalid());
                }
                *target = raster.clone();
            }
        }
        Ok(self.document)
    }
}

/// Workspace recovery embeds the same shared-raster representation used
/// by native files while retaining support for earlier dense workspace data.
pub mod recovery_document {
    use super::{Document, ProjectV2};
    use serde::{Deserialize, Serialize};

    pub fn serialize<S: serde::Serializer>(
        doc: &Document,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        ProjectV2::new(doc).serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Document, D::Error> {
        use serde::de::{Error, MapAccess, Visitor};

        struct Recovery;
        impl<'de> Visitor<'de> for Recovery {
            type Value = Document;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a PixelCraft recovery document")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Document, M::Error> {
                let key = map
                    .next_key::<String>()?
                    .ok_or_else(|| Error::missing_field("document"))?;
                let mut doc = if matches!(key.as_str(), "document" | "shared_rasters") {
                    // Decode raster arrays directly into their typed storage. A JSON Value for
                    // each channel of a dense canvas can take much more memory than the image.
                    let (mut document, mut shared) = (None, None);
                    let mut next = Some(key);
                    while let Some(field) = next {
                        match field.as_str() {
                            "document" if document.is_none() => document = Some(map.next_value()?),
                            "shared_rasters" if shared.is_none() => {
                                shared = Some(map.next_value()?)
                            }
                            "document" => return Err(Error::duplicate_field("document")),
                            "shared_rasters" => {
                                return Err(Error::duplicate_field("shared_rasters"));
                            }
                            _ => {
                                map.next_value::<serde::de::IgnoredAny>()?;
                            }
                        }
                        next = map.next_key()?;
                    }
                    ProjectV2 {
                        document: document.ok_or_else(|| Error::missing_field("document"))?,
                        shared_rasters: shared.unwrap_or_default(),
                    }
                    .restore()
                    .map_err(Error::custom)?
                } else {
                    let decoder = serde::de::value::MapAccessDeserializer::new(RecoveryLegacy {
                        first_value: Some(key),
                        map,
                    });
                    Document::deserialize(decoder)?
                };
                doc.share_linked_rasters();
                Ok(doc)
            }
        }
        // Forward the field we peeked at into the legacy document's normal streaming decoder.
        struct RecoveryLegacy<M> {
            first_value: Option<String>,
            map: M,
        }
        impl<'de, M: MapAccess<'de>> MapAccess<'de> for RecoveryLegacy<M> {
            type Error = M::Error;

            fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(
                &mut self,
                seed: K,
            ) -> Result<Option<K::Value>, Self::Error> {
                match self.first_value.take() {
                    Some(key) => seed
                        .deserialize(serde::de::value::StringDeserializer::new(key))
                        .map(Some),
                    None => self.map.next_key_seed(seed),
                }
            }

            fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(
                &mut self,
                seed: V,
            ) -> Result<V::Value, Self::Error> {
                self.map.next_value_seed(seed)
            }
        }
        decoder.deserialize_map(Recovery)
    }
}

pub fn encode_project(d: &Document) -> Result<Vec<u8>, String> {
    d.validate()?;
    let mut out = PROJECT_MAGIC.to_vec();
    let mut z = ZlibEncoder::new(vec![], Compression::fast());
    let mut buffered = BufWriter::with_capacity(64 * 1024, &mut z);
    serde_json::to_writer(&mut buffered, &ProjectV2::new(d)).map_err(|e| e.to_string())?;
    buffered.into_inner().map_err(|e| e.to_string())?;
    out.extend(z.finish().map_err(|e| e.to_string())?);
    Ok(out)
}
pub fn decode_project(b: &[u8]) -> Result<Document, String> {
    let (b, legacy) = if let Some(data) = b.strip_prefix(PROJECT_MAGIC) {
        (data, false)
    } else if let Some(data) = b.strip_prefix(PROJECT_MAGIC_V1) {
        (data, true)
    } else {
        return Err("Invalid PixelCraft project version".into());
    };
    // Parse common projects from a byte slice. Keep the streaming path for
    // large projects so decoding does not also retain the whole JSON payload.
    const SLICE_LIMIT: usize = 96 * 1024 * 1024;
    let mut decoder = ZlibDecoder::new(b);
    let mut prefix = Vec::new();
    (&mut decoder)
        .take(SLICE_LIMIT as u64 + 1)
        .read_to_end(&mut prefix)
        .map_err(|e| e.to_string())?;
    let mut d: Document = match (legacy, prefix.len() <= SLICE_LIMIT) {
        (true, true) => serde_json::from_slice(&prefix).map_err(|e| e.to_string())?,
        (false, true) => serde_json::from_slice::<ProjectV2>(&prefix)
            .map_err(|e| e.to_string())?
            .restore()?,
        (legacy, false) => {
            let remaining = MAX_PROJECT_BYTES - prefix.len() as u64;
            let reader = BufReader::with_capacity(
                64 * 1024,
                std::io::Cursor::new(prefix).chain(decoder.take(remaining)),
            );
            if legacy {
                serde_json::from_reader(reader).map_err(|e| e.to_string())?
            } else {
                serde_json::from_reader::<_, ProjectV2>(reader)
                    .map_err(|e| e.to_string())?
                    .restore()?
            }
        }
    };
    d.share_linked_rasters();
    d.validate()?;
    Ok(d)
}
fn append_metadata(chunks: &mut Vec<Vec<u8>>, d: &Document, f: usize, owner: &str) {
    for m in d
        .metadata
        .iter()
        .filter(|m| m.frame == f && m.owner == owner)
    {
        chunks.push(chunk(m.kind, m.data.clone()));
    }
}
fn encode_slice(slice: &SpriteSlice) -> Result<Vec<u8>, String> {
    let center = slice.keys.iter().any(|key| key.center.is_some());
    let pivot = slice.keys.iter().any(|key| key.pivot.is_some());
    let mut data = Vec::new();
    dword(&mut data, slice.keys.len() as u32);
    dword(&mut data, u32::from(center) | (u32::from(pivot) << 1));
    dword(&mut data, 0);
    string(&mut data, &slice.name)?;
    for key in &slice.keys {
        dword(&mut data, key.frame as u32);
        for value in key.bounds {
            dword(&mut data, value as u32);
        }
        if center {
            for value in key.center.unwrap_or([0; 4]) {
                dword(&mut data, value as u32);
            }
        }
        if pivot {
            for value in key.pivot.unwrap_or([0; 2]) {
                dword(&mut data, value as u32);
            }
        }
    }
    Ok(data)
}

fn decode_slice(data: &[u8]) -> Result<SpriteSlice, String> {
    let count = u32at(data, 0)? as usize;
    let flags = u32at(data, 4)?;
    if flags & !3 != 0 || count > u16::MAX as usize {
        return Err("Unsupported slice flags or key count".into());
    }
    let mut cursor = 12;
    let name = read_string(data, &mut cursor)?;
    let mut next = || {
        let value = u32at(data, cursor);
        cursor += 4;
        value
    };
    let mut keys = Vec::new();
    for _ in 0..count {
        let frame = next()? as usize;
        let bounds = [
            next()? as i32,
            next()? as i32,
            next()? as i32,
            next()? as i32,
        ];
        let center = if flags & 1 != 0 {
            Some([
                next()? as i32,
                next()? as i32,
                next()? as i32,
                next()? as i32,
            ])
        } else {
            None
        };
        let pivot = if flags & 2 != 0 {
            Some([next()? as i32, next()? as i32])
        } else {
            None
        };
        keys.push(SliceKey {
            frame,
            bounds,
            center,
            pivot,
        });
    }
    keys.sort_by_key(|key| key.frame);
    Ok(SpriteSlice { name, keys })
}

pub fn encode_ase(d: &Document) -> Result<Vec<u8>, String> {
    d.validate()?;
    if d.needs_baked_ase() {
        let mut baked = Document::new(d.width, d.height);
        baked.layers[0].name = "Baked artwork".into();
        baked.durations = d.durations.clone();
        baked.tags = d.tags.clone();
        baked.slices = d.slices.clone();
        baked.color_profile = d.color_profile.clone();
        baked.layers[0].cel_meta = vec![CelMeta::default(); d.durations.len()];
        baked.layers[0].tile_cels = vec![vec![]; d.durations.len()];
        if d.width
            .saturating_mul(d.height)
            .saturating_mul(d.durations.len())
            > MAX_DOCUMENT_PIXELS
        {
            return Err("Baked ASE copy exceeds the project pixel budget".into());
        }
        baked.layers[0].cels = (0..d.durations.len())
            .map(|frame| d.composite(frame).into())
            .collect();
        return encode_ase(&baked);
    }
    let mut out = d
        .metadata
        .iter()
        .find(|m| m.owner == "ase:header" && m.data.len() == 128)
        .map(|m| m.data.clone())
        .unwrap_or_else(|| vec![0; 128]);
    out[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    out[6..8].copy_from_slice(&(d.durations.len() as u16).to_le_bytes());
    out[8..10].copy_from_slice(&(d.width as u16).to_le_bytes());
    out[10..12].copy_from_slice(&(d.height as u16).to_le_bytes());
    let depth = match d.color_mode {
        ColorMode::Rgba => 32u16,
        ColorMode::Grayscale => 16,
        ColorMode::Indexed => 8,
    };
    out[12..14].copy_from_slice(&depth.to_le_bytes());
    let header_flags = 1u32
        | if d.compose_groups { 2 } else { 0 }
        | if d.layers.iter().any(|l| !l.uuid.is_empty()) {
            4
        } else {
            0
        };
    out[14..18].copy_from_slice(&header_flags.to_le_bytes());
    out[18..20].copy_from_slice(&100u16.to_le_bytes());
    out[28] = d.transparent_index as u8;
    out[32..34].copy_from_slice(&(d.palette.len() as u16).to_le_bytes());
    if out[34] == 0 {
        out[34] = 1;
    }
    if out[35] == 0 {
        out[35] = 1;
    }
    fn visit(d: &Document, p: Option<usize>, depth: u16, out: &mut Vec<(usize, u16)>) {
        for (i, l) in d.layers.iter().enumerate() {
            if l.parent == p {
                out.push((i, depth));
                if l.group {
                    visit(d, Some(i), depth + 1, out)
                }
            }
        }
    }
    let mut order = vec![];
    visit(d, None, 0, &mut order);
    let mut ids = vec![0; d.layers.len()];
    for (new, (old, _)) in order.iter().enumerate() {
        ids[*old] = new;
    }
    let mut link_sources = std::collections::HashMap::new();
    for f in 0..d.durations.len() {
        let mut chunks = vec![];
        let palette = d.palette_for(f);
        if f == 0 {
            if !d.color_profile.is_empty() {
                chunks.push(chunk(0x2007, d.color_profile.clone()));
            }
            let mut c = vec![];
            dword(&mut c, palette.len() as u32);
            dword(&mut c, 0);
            dword(&mut c, (palette.len() - 1) as u32);
            c.extend([0; 8]);
            for (i, p) in palette.iter().enumerate() {
                let name = d.palette_names.get(i).filter(|s| !s.is_empty());
                word(&mut c, u16::from(name.is_some()));
                c.extend(p);
                if let Some(name) = name {
                    string(&mut c, name)?;
                }
            }
            chunks.push(chunk(0x2019, c));
            append_metadata(&mut chunks, d, f, "palette");
            for (i, t) in d.tilesets.iter().enumerate() {
                let mut c = vec![];
                let info = d.ase_tilesets.iter().find(|v| v.index == i);
                dword(&mut c, info.map_or(i as u32, |v| v.id));
                // Resolved external tiles are embedded too, making saves portable.
                dword(&mut c, info.map_or(6, |v| v.flags | 6));
                dword(&mut c, (t.tiles.len() + 1) as u32);
                word(&mut c, t.width as u16);
                word(&mut c, t.height as u16);
                word(&mut c, info.map_or(1, |v| v.base_index) as u16);
                c.extend([0; 14]);
                string(&mut c, &t.name)?;
                if let Some(link) = info.and_then(|v| v.external) {
                    dword(&mut c, link[0]);
                    dword(&mut c, link[1]);
                }
                let mut p = vec![CLEAR; t.width * t.height];
                for tile in &t.tiles {
                    p.extend(tile.iter().copied());
                }
                let z = compress(&pixels_bytes(&p, d.color_mode, palette))?;
                dword(&mut c, z.len() as u32);
                c.extend(z);
                chunks.push(chunk(0x2023, c));
                append_metadata(&mut chunks, d, f, &format!("tileset:{i}"));
            }
            for &(li, level) in &order {
                let l = &d.layers[li];
                let mut c = vec![];
                word(
                    &mut c,
                    u16::from(l.visible)
                        | if l.locked { 0 } else { 2 }
                        | l.ase_flags
                        | if l.background { 8 } else { 0 },
                );
                word(
                    &mut c,
                    if l.group {
                        1
                    } else if l.tileset.is_some() {
                        2
                    } else {
                        0
                    },
                );
                word(&mut c, level);
                word(&mut c, 0);
                word(&mut c, 0);
                word(&mut c, l.blend as u16);
                c.push(l.opacity);
                c.extend([0; 3]);
                string(&mut c, &l.name)?;
                if let Some(t) = l.tileset {
                    dword(&mut c, t as u32)
                }
                if header_flags & 4 != 0 {
                    if l.uuid.len() == 16 {
                        c.extend(&l.uuid);
                    } else {
                        c.extend([0; 16]);
                    }
                }
                chunks.push(chunk(0x2004, c));
                append_metadata(&mut chunks, d, f, &format!("layer:{li}"));
            }
            if !d.tags.is_empty() {
                let mut c = vec![];
                word(&mut c, d.tags.len() as u16);
                c.extend([0; 8]);
                for t in &d.tags {
                    word(&mut c, t.from as u16);
                    word(&mut c, t.to as u16);
                    c.push(t.direction);
                    word(&mut c, t.repeat);
                    c.extend([0; 6]);
                    c.extend(t.color);
                    c.push(0);
                    string(&mut c, &t.name)?;
                }
                chunks.push(chunk(0x2018, c));
                append_metadata(&mut chunks, d, f, "tags");
            }
        }
        if f > 0 && palette != d.palette_for(f - 1) {
            let mut c = vec![];
            dword(&mut c, palette.len() as u32);
            dword(&mut c, 0);
            dword(&mut c, (palette.len() - 1) as u32);
            c.extend([0; 8]);
            for (i, p) in palette.iter().enumerate() {
                let name = d.palette_names.get(i).filter(|s| !s.is_empty());
                word(&mut c, u16::from(name.is_some()));
                c.extend(p);
                if let Some(name) = name {
                    string(&mut c, name)?;
                }
            }
            chunks.push(chunk(0x2019, c));
            append_metadata(&mut chunks, d, f, "palette");
        }
        if f == 0 {
            for (index, slice) in d.slices.iter().enumerate() {
                chunks.push(chunk(0x2022, encode_slice(slice)?));
                append_metadata(&mut chunks, d, f, &format!("slice:{index}"));
            }
        }
        for &(li, _) in &order {
            let l = &d.layers[li];
            if l.group {
                continue;
            }
            let m = &l.cel_meta[f];
            if l.tileset.is_none()
                && m.artwork_link.is_none()
                && m.bounds.is_none()
                && m.indices.is_none()
                && m.outside.is_empty()
                && l.cels[f].allocation_pixels() == 0
                && m.opacity == 255
                && m.z_index == 0
                && !d
                    .metadata
                    .iter()
                    .any(|chunk| chunk.frame == f && chunk.owner == format!("cel:{li}"))
            {
                continue;
            }
            let (mut x, mut y, mut w, mut h) = (0i32, 0i32, d.width, d.height);
            if l.tileset.is_none() {
                if let Some(bounds) = m.bounds {
                    x = bounds[0];
                    y = bounds[1];
                    w = bounds[2] as usize;
                    h = bounds[3] as usize;
                }
                if m.bounds.is_some() || !m.outside.is_empty() {
                    let mut maxx = x + w as i32;
                    let mut maxy = y + h as i32;
                    l.cels[f].visit_nonclear(|i, _| {
                        let (cx, cy) = ((i % d.width) as i32, (i / d.width) as i32);
                        x = x.min(cx);
                        y = y.min(cy);
                        maxx = maxx.max(cx + 1);
                        maxy = maxy.max(cy + 1);
                    });
                    for p in &m.outside {
                        x = x.min(p.x);
                        y = y.min(p.y);
                    }
                    for p in &m.outside {
                        maxx = maxx.max(p.x + 1);
                        maxy = maxy.max(p.y + 1);
                    }
                    w = (maxx - x) as usize;
                    h = (maxy - y) as usize;
                }
                if x < i16::MIN as i32
                    || y < i16::MIN as i32
                    || w > 65535
                    || h > 65535
                    || w.saturating_mul(h) > MAX_PIXELS
                {
                    return Err("Off-canvas cel exceeds ASE limits".into());
                }
            }
            if let Some(g) = &m.tilemap {
                x = g.x;
                y = g.y;
            }
            let mut c = vec![];
            word(&mut c, ids[li] as u16);
            word(&mut c, x as i16 as u16);
            word(&mut c, y as i16 as u16);
            c.push(m.opacity);
            let linked_from = m.artwork_link.and_then(|link| {
                let source = link_sources.entry((li, link)).or_insert(f);
                (*source < f).then_some(*source)
            });
            let kind = if linked_from.is_some() {
                1
            } else if l.tileset.is_some() {
                3
            } else {
                2
            };
            word(&mut c, kind);
            word(&mut c, m.z_index as u16);
            c.extend([0; 5]);
            if let Some(source) = linked_from {
                word(&mut c, source as u16);
            } else if let Some(ti) = l.tileset {
                let t = &d.tilesets[ti];
                word(
                    &mut c,
                    m.tilemap
                        .as_ref()
                        .map_or(d.width.div_ceil(t.width), |g| g.columns)
                        as u16,
                );
                word(
                    &mut c,
                    m.tilemap
                        .as_ref()
                        .map_or(d.height.div_ceil(t.height), |g| g.rows) as u16,
                );
                word(&mut c, 32);
                for mask in [0x1fffffff, 0x80000000, 0x40000000, 0x20000000] {
                    dword(&mut c, mask)
                }
                c.extend([0; 10]);
                let b: Vec<u8> = l.tile_cels[f]
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect();
                c.extend(compress(&b)?);
            } else {
                word(&mut c, w as u16);
                word(&mut c, h as u16);
                if d.color_mode == ColorMode::Rgba
                    && (x, y, w, h) == (0, 0, d.width, d.height)
                    && m.outside.is_empty()
                    && let Pixels::Dense(pixels) = &l.cels[f]
                {
                    c.extend(compress(pixels.as_slice().as_flattened())?);
                    chunks.push(chunk(0x2005, c));
                    append_metadata(&mut chunks, d, f, &format!("cel:{li}"));
                    continue;
                }

                let mut p = vec![CLEAR; w * h];
                l.cels[f].visit_nonclear(|index, pixel| {
                    let cx = (index % d.width) as i32;
                    let cy = (index / d.width) as i32;
                    p[(cy - y) as usize * w + (cx - x) as usize] = pixel;
                });
                for o in &m.outside {
                    p[(o.y - y) as usize * w + (o.x - x) as usize] = o.color;
                }
                let encoded = if d.color_mode == ColorMode::Indexed {
                    let effective_palette = d.editing_palette(li, f);
                    let palette = effective_palette.as_slice();
                    // Reuse index matches for repeated colors without growing with the image.
                    let mut indices_by_color = std::collections::HashMap::new();
                    let mut indices: Vec<u8> = p
                        .iter()
                        .map(|&pixel| {
                            if let Some(&index) = indices_by_color.get(&pixel) {
                                return index;
                            }
                            let index = nearest(pixel, palette) as u8;
                            if indices_by_color.len() < 4096 {
                                indices_by_color.insert(pixel, index);
                            }
                            index
                        })
                        .collect();
                    if let Some(stored) = &m.indices {
                        for cy in 0..d.height {
                            for cx in 0..d.width {
                                let i = cy * d.width + cx;
                                let id = stored[i];
                                if cx as i32 >= x
                                    && cy as i32 >= y
                                    && (cx as i32) < x + w as i32
                                    && (cy as i32) < y + h as i32
                                    && palette.get(id as usize) == Some(&l.cels[f][i])
                                {
                                    indices
                                        [(cy as i32 - y) as usize * w + (cx as i32 - x) as usize] =
                                        id;
                                }
                            }
                        }
                    }
                    indices
                } else {
                    pixels_bytes(&p, d.color_mode, &[])
                };
                c.extend(compress(&encoded)?);
            }
            chunks.push(chunk(0x2005, c));
            append_metadata(&mut chunks, d, f, &format!("cel:{li}"));
        }
        append_metadata(&mut chunks, d, f, "");
        let size = 16 + chunks.iter().map(Vec::len).sum::<usize>();
        dword(&mut out, size as u32);
        word(&mut out, 0xF1FA);
        word(&mut out, chunks.len().min(65535) as u16);
        word(&mut out, d.durations[f]);
        word(&mut out, 0);
        dword(&mut out, chunks.len() as u32);
        for c in chunks {
            out.extend(c)
        }
    }
    let len = out.len() as u32;
    out[0..4].copy_from_slice(&len.to_le_bytes());
    Ok(out)
}
pub fn decode_ase(b: &[u8]) -> Result<Document, String> {
    decode_ase_with_resolver(b, &mut |file, _| {
        Err(format!("External tileset requires a file resolver: {file}"))
    })
}
/// Resolve external references relative to the containing project, never an editor installation.
pub fn decode_ase_with_resolver(
    b: &[u8],
    resolver: &mut dyn FnMut(&str, u32) -> Result<Tileset, String>,
) -> Result<Document, String> {
    if b.len() < 128 || u16at(b, 4)? != 0xA5E0 {
        return Err("This is not an ASE sprite file".into());
    }
    let depth = u16at(b, 12)? as usize;
    if ![8, 16, 32].contains(&depth) {
        return Err("Unsupported sprite color depth".into());
    }
    let (w, h, frames) = (
        u16at(b, 8)? as usize,
        u16at(b, 10)? as usize,
        u16at(b, 6)? as usize,
    );
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w * h > MAX_PIXELS
        || frames == 0
    {
        return Err("Sprite exceeds canvas or memory limits".into());
    }
    let mut d = Document::new(w, h);
    d.compose_groups = u32at(b, 14)? & 2 != 0;
    d.layers.clear();
    d.durations = vec![100; frames];
    d.color_mode = match depth {
        8 => ColorMode::Indexed,
        16 => ColorMode::Grayscale,
        _ => ColorMode::Rgba,
    };
    let transparent = b[28] as usize;
    d.transparent_index = transparent;
    let mut canonical = [0u8; 128];
    canonical[34] = 1;
    canonical[35] = 1;
    if b[20..28].iter().any(|v| *v != 0)
        || b[29..32].iter().any(|v| *v != 0)
        || b[34..128] != canonical[34..128]
    {
        d.metadata.push(MetadataChunk {
            frame: 0,
            kind: 0,
            data: b[..128].to_vec(),
            owner: "ase:header".into(),
        });
    }
    type RawCel = (
        usize,
        usize,
        std::sync::Arc<Vec<Pixel>>,
        Option<std::sync::Arc<Vec<u8>>>,
    );
    let mut raw: Vec<Vec<Option<RawCel>>> = vec![];
    let mut decoded_pixels = 0usize;
    let mut projected_pixels = 0usize;
    let mut decoded_tile_bytes = 0usize;
    let mut parents: Vec<usize> = vec![];
    let mut flags = vec![];
    let mut p = 128;
    let mut deferred_tiles = vec![];
    let mut palettes = vec![];
    let mut external_files = std::collections::HashMap::new();
    let mut legacy_tilesets = vec![];
    for f in 0..frames {
        let size = u32at(b, p)? as usize;
        let end = p
            .checked_add(size)
            .filter(|e| *e <= b.len() && size >= 16)
            .ok_or("Invalid frame size")?;
        if u16at(b, p + 4)? != 0xF1FA {
            return Err("Invalid frame header".into());
        }
        let count = u32at(b, p + 12)?;
        let count = if count == 0 {
            u16at(b, p + 6)? as u32
        } else {
            count
        };
        d.durations[f] = u16at(b, p + 8)?.max(1);
        let mut c = p + 16;
        let mut chunks = vec![];
        for _ in 0..count {
            let size = u32at(b, c)? as usize;
            let ce = c
                .checked_add(size)
                .filter(|e| *e <= end && size >= 6)
                .ok_or("Invalid chunk size")?;
            chunks.push((u16at(b, c + 4)?, &b[c + 6..ce]));
            c = ce;
        }
        if c != end {
            return Err("Frame chunk count does not match its size".into());
        }
        for &(kind, data) in &chunks {
            if kind == 0x2008 {
                let mut q = 12;
                for _ in 0..u32at(data, 0)? {
                    let id = u32at(data, q)?;
                    q += 12;
                    external_files.insert(id, read_string(data, &mut q)?);
                }
            }
        }
        // Palettes can follow cels. Resolve them before decoding image indices.
        let has_new = chunks.iter().any(|(k, _)| *k == 0x2019);
        for &(kind, data) in &chunks {
            match kind {
                0x2019 => {
                    let size = u32at(data, 0)? as usize;
                    let first = u32at(data, 4)? as usize;
                    let last = u32at(data, 8)? as usize;
                    if size == 0 || size > 256 || first > last || last >= size {
                        return Err("Invalid palette range".into());
                    }
                    d.palette.resize(size, CLEAR);
                    let mut q = 20;
                    for i in first..=last {
                        let flags = u16at(data, q)?;
                        q += 2;
                        let a = data.get(q..q + 4).ok_or("Truncated palette")?;
                        d.palette[i] = [a[0], a[1], a[2], a[3]];
                        q += 4;
                        if flags & 1 != 0 {
                            let name = read_string(data, &mut q)?;
                            d.palette_names.resize(size, String::new());
                            d.palette_names[i] = name;
                        }
                    }
                }
                0x0004 | 0x0011 if !has_new => {
                    d.palette.resize(256, CLEAR);
                    let mut q = 2;
                    let mut index = 0;
                    for _ in 0..u16at(data, 0)? {
                        index += *data.get(q).ok_or("Truncated palette")? as usize;
                        let n = *data.get(q + 1).ok_or("Truncated palette")? as usize;
                        let n = if n == 0 { 256 } else { n };
                        q += 2;
                        for _ in 0..n {
                            let px = data.get(q..q + 3).ok_or("Truncated palette")?;
                            if index >= 256 {
                                return Err("Palette overflow".into());
                            }
                            let scale = if kind == 0x0011 { 255. / 63. } else { 1. };
                            d.palette[index] = [
                                (px[0] as f32 * scale) as u8,
                                (px[1] as f32 * scale) as u8,
                                (px[2] as f32 * scale) as u8,
                                255,
                            ];
                            q += 3;
                            index += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        let mut owner = String::new();
        for (kind, data) in chunks {
            match kind {
                0x2019 | 0x0004 | 0x0011 => owner = "palette".into(),
                0x2004 => {
                    if f != 0 {
                        return Err("Animated layer definitions are not supported".into());
                    }
                    let flag = u16at(data, 0)?;
                    let ty = u16at(data, 2)?;
                    let level = u16at(data, 4)? as usize;
                    if ty > 2 || level > parents.len() {
                        return Err("Invalid layer hierarchy".into());
                    }
                    let mut q = 16;
                    let name = read_string(data, &mut q)?;
                    if frames * (d.layers.len() + 1) > MAX_CEL_SLOTS {
                        return Err("Too many layers for this sprite size".into());
                    }
                    let mut l = Layer::new(name, w, h, frames);
                    l.ase_flags = flag & !3;
                    l.background = flag & 8 != 0;
                    l.visible = flag & 1 != 0;
                    l.locked = flag & 2 == 0;
                    l.group = ty == 1;
                    l.parent = if level == 0 {
                        None
                    } else {
                        Some(parents[level - 1])
                    };
                    l.blend = BlendMode::from_id(u16at(data, 10)?)?;
                    l.opacity = if u32at(b, 14)? & 1 != 0 {
                        *data.get(12).ok_or("Truncated layer")?
                    } else {
                        255
                    };
                    if ty == 2 {
                        l.tileset = Some(u32at(data, q)? as usize);
                        q += 4;
                    }
                    if u32at(b, 14)? & 4 != 0 {
                        l.uuid = data.get(q..q + 16).ok_or("Truncated layer UUID")?.to_vec();
                    }
                    if l.group {
                        parents.truncate(level);
                        parents.push(d.layers.len());
                    } else {
                        parents.truncate(level);
                    }
                    owner = format!("layer:{}", d.layers.len());
                    flags.push(flag);
                    d.layers.push(l);
                    raw.push(vec![None; frames]);
                }
                0x2005 => {
                    let li = u16at(data, 0)? as usize;
                    if li >= d.layers.len() {
                        return Err("Cel refers to a missing layer".into());
                    }
                    let x = u16at(data, 2)? as i16 as i32;
                    let y = u16at(data, 4)? as i16 as i32;
                    let opacity = *data.get(6).ok_or("Truncated cel")?;
                    let ty = u16at(data, 7)?;
                    let z_index = u16at(data, 9)? as i16;
                    d.layers[li].cel_meta[f].opacity = opacity;
                    d.layers[li].cel_meta[f].z_index = z_index;
                    owner = format!("cel:{li}");
                    if ty == 3 {
                        deferred_tiles.push((li, f, x, y, std::sync::Arc::new(data.to_vec())));
                        continue;
                    }
                    if ty == 1 && d.layers[li].tileset.is_some() {
                        let link = u16at(data, 16)? as usize;
                        if link >= f {
                            return Err("Invalid linked tilemap".into());
                        }
                        let source = deferred_tiles
                            .iter()
                            .find(|(l, frame, _, _, _)| *l == li && *frame == link)
                            .ok_or("Linked tilemap is missing")?
                            .4
                            .clone();
                        d.set_cel_link(li, f, link);
                        deferred_tiles.push((li, f, x, y, source));
                        continue;
                    }
                    let (cw, ch, pixels, indices) = match ty {
                        0 | 2 => {
                            let cw = u16at(data, 16)? as usize;
                            let ch = u16at(data, 18)? as usize;
                            if cw * ch > MAX_PIXELS {
                                return Err("Cel too large".into());
                            }
                            decoded_pixels = decoded_pixels.saturating_add(cw * ch);
                            if decoded_pixels > MAX_DOCUMENT_PIXELS {
                                return Err(
                                    "Sprite image data exceeds the project pixel budget".into()
                                );
                            }
                            let expected = cw * ch * (depth / 8);
                            let encoded = data.get(20..).ok_or("Truncated image cel")?;
                            let bytes = if ty == 0 {
                                encoded.to_vec()
                            } else {
                                decompress(encoded, expected)?
                            };
                            if bytes.len() != expected {
                                return Err("Image cel size does not match its pixels".into());
                            }
                            (
                                cw,
                                ch,
                                std::sync::Arc::new(read_pixels(
                                    &bytes,
                                    depth,
                                    &d.palette,
                                    transparent,
                                    flags[li] & 8 != 0,
                                )?),
                                if depth == 8 {
                                    Some(std::sync::Arc::new(bytes))
                                } else {
                                    None
                                },
                            )
                        }
                        1 => {
                            let link = u16at(data, 16)? as usize;
                            if link >= f {
                                return Err("Invalid linked cel".into());
                            }
                            let (cw, ch, pixels, indices) =
                                raw[li][link].clone().ok_or("Linked cel is missing")?;
                            d.set_cel_link(li, f, link);
                            let origin =
                                |bounds: Option<[i32; 4]>| bounds.map_or((0, 0), |b| (b[0], b[1]));
                            if depth != 8
                                && origin(d.layers[li].cel_meta[link].bounds) == (x, y)
                                && d.layers[li].cel_meta[link].outside.is_empty()
                            {
                                let layer = &mut d.layers[li];
                                layer.cels[f] = layer.cels[link].clone();
                                layer.cel_meta[f].bounds = layer.cel_meta[link].bounds;
                                raw[li][f] = Some((cw, ch, pixels, indices));
                                continue;
                            }
                            (cw, ch, pixels, indices)
                        }
                        _ => return Err("Unknown cel encoding".into()),
                    };
                    if x != 0 || y != 0 || cw != w || ch != h {
                        d.layers[li].cel_meta[f].bounds = Some([x, y, cw as i32, ch as i32]);
                    }
                    // Dense, canvas-aligned data can keep its decoded backing.
                    // Sparse cels still go through the placement path below.
                    if depth != 8
                        && (x, y, cw, ch) == (0, 0, w, h)
                        && pixels
                            .iter()
                            .filter(|&&pixel| pixel != CLEAR)
                            .take(w * h / 2 + 1)
                            .count()
                            > w * h / 2
                    {
                        projected_pixels = projected_pixels.saturating_add(w * h);
                        if projected_pixels > MAX_DOCUMENT_PIXELS {
                            return Err("Placed cel data exceeds the project pixel budget".into());
                        }
                        d.layers[li].cels[f] = Pixels::Dense(pixels.clone());
                        raw[li][f] = Some((cw, ch, pixels, indices));
                        continue;
                    }
                    let indexed_palette = (depth == 8).then(|| d.editing_palette(li, f));
                    if indices.is_some() {
                        decoded_pixels = decoded_pixels.saturating_add((w * h).div_ceil(4));
                        if decoded_pixels > MAX_DOCUMENT_PIXELS {
                            return Err("Indexed cel data exceeds the project pixel budget".into());
                        }
                        d.layers[li].cel_meta[f].indices =
                            Some(std::sync::Arc::new(vec![transparent as u8; w * h]));
                    }
                    for cy in 0..ch {
                        for cx in 0..cw {
                            let i = cy * cw + cx;
                            let color = if let (Some(indices), Some(palette)) =
                                (&indices, &indexed_palette)
                            {
                                *palette
                                    .get(indices[i] as usize)
                                    .ok_or("Invalid palette index")?
                            } else {
                                pixels[i]
                            };
                            let (px, py) = (x + cx as i32, y + cy as i32);
                            if let Some(i) = d.index(px, py) {
                                d.layers[li].cels[f].set(i, color);
                                if let (Some(target), Some(source)) =
                                    (&mut d.layers[li].cel_meta[f].indices, &indices)
                                {
                                    std::sync::Arc::make_mut(target)[i] = source[cy * cw + cx];
                                }
                            } else if color != CLEAR {
                                d.layers[li].cel_meta[f].outside.push(OutsidePixel {
                                    x: px,
                                    y: py,
                                    color,
                                });
                            }
                        }
                    }
                    projected_pixels = projected_pixels.saturating_add(
                        d.layers[li].cels[f].allocation_pixels()
                            + d.layers[li].cel_meta[f].outside.len().saturating_mul(3),
                    );
                    if projected_pixels > MAX_DOCUMENT_PIXELS {
                        return Err("Sprite cel placements exceed the project pixel budget".into());
                    }
                    raw[li][f] = Some((cw, ch, pixels, indices));
                }
                0x2007 => {
                    if data.len() < 16 {
                        return Err("Truncated profile".into());
                    }
                    if u16at(data, 0)? == 2 {
                        let n = u32at(data, 16)? as usize;
                        if data.get(20..20 + n).is_none() {
                            return Err("Truncated ICC profile".into());
                        }
                    }
                    d.color_profile = data.to_vec();
                }
                0x2018 => {
                    let n = u16at(data, 0)? as usize;
                    let mut q = 10;
                    for _ in 0..n {
                        let from = u16at(data, q)? as usize;
                        let to = u16at(data, q + 2)? as usize;
                        let dir = *data.get(q + 4).ok_or("Truncated tag")?;
                        let repeat = u16at(data, q + 5)?;
                        let color = data.get(q + 13..q + 16).ok_or("Truncated tag")?;
                        let color = [color[0], color[1], color[2]];
                        q += 17;
                        let name = read_string(data, &mut q)?;
                        d.tags.push(FrameTag {
                            name,
                            from,
                            to,
                            direction: dir,
                            repeat,
                            color,
                        });
                    }
                    owner = "tags".into();
                }
                0x2022 => match decode_slice(data) {
                    Ok(slice) => {
                        owner = format!("slice:{}", d.slices.len());
                        d.slices.push(slice);
                    }
                    Err(_) => d.metadata.push(MetadataChunk {
                        frame: f,
                        kind,
                        data: data.to_vec(),
                        owner: String::new(),
                    }),
                },
                0x2023 => {
                    let id = u32at(data, 0)? as usize;
                    let fl = u32at(data, 4)?;
                    let n = u32at(data, 8)? as usize;
                    let tw = u16at(data, 12)? as usize;
                    let th = u16at(data, 14)? as usize;
                    if tw == 0
                        || th == 0
                        || n == 0
                        || tw.saturating_mul(th).saturating_mul(n) > MAX_PIXELS
                    {
                        return Err("Invalid tileset dimensions".into());
                    }
                    let mut q = 32;
                    let name = read_string(data, &mut q)?;
                    let external = if fl & 1 != 0 {
                        let link = [u32at(data, q)?, u32at(data, q + 4)?];
                        q += 8;
                        Some(link)
                    } else {
                        None
                    };
                    let tiles = if fl & 2 != 0 {
                        let len = u32at(data, q)? as usize;
                        q += 4;
                        let bytes = decompress(
                            data.get(q..q + len).ok_or("Truncated tileset")?,
                            tw * th * n * (depth / 8),
                        )?;
                        let pixels = read_pixels(&bytes, depth, &d.palette, transparent, false)?;
                        let mut tiles: Vec<Pixels> =
                            pixels.chunks(tw * th).map(|p| p.to_vec().into()).collect();
                        if fl & 4 != 0 {
                            tiles.remove(0);
                        }
                        tiles
                    } else {
                        let link =
                            external.ok_or("Tileset has neither pixels nor external reference")?;
                        let file = external_files
                            .get(&link[0])
                            .ok_or("Missing external file entry")?;
                        let resolved = resolver(file, link[1])?;
                        if resolved.width != tw || resolved.height != th {
                            return Err("External tileset dimensions differ".into());
                        }
                        resolved.tiles
                    };
                    let index = d.tilesets.len();
                    legacy_tilesets.push(fl & 4 == 0);
                    if id != index || fl != 6 || u16at(data, 16)? != 1 {
                        d.ase_tilesets.push(AseTilesetInfo {
                            index,
                            id: id as u32,
                            flags: fl | 6,
                            base_index: u16at(data, 16)? as i16,
                            external,
                        });
                    }
                    d.tilesets.push(Tileset {
                        name,
                        width: tw,
                        height: th,
                        tiles,
                    });
                    owner = format!("tileset:{index}");
                }
                0x2008 => {
                    d.metadata.push(MetadataChunk {
                        frame: f,
                        kind,
                        data: data.to_vec(),
                        owner: String::new(),
                    });
                }
                _ => {
                    d.metadata.push(MetadataChunk {
                        frame: f,
                        kind,
                        data: data.to_vec(),
                        owner: owner.clone(),
                    });
                }
            }
        }
        palettes.push(d.palette.clone());
        p = end;
    }
    for (li, f, x, y, data) in deferred_tiles {
        let ti = d.layers[li]
            .tileset
            .ok_or("Tilemap cel in an image layer")?;
        let t = d.tilesets.get(ti).ok_or("Missing tileset")?;
        let cw = u16at(&data, 16)? as usize;
        let ch = u16at(&data, 18)? as usize;
        let bits = u16at(&data, 20)? as usize;
        if ![8, 16, 32].contains(&bits) || cw == 0 || ch == 0 || cw * ch > MAX_PIXELS {
            return Err("Unsupported tile size".into());
        }
        decoded_tile_bytes = decoded_tile_bytes.saturating_add(cw * ch * 4);
        if decoded_tile_bytes / 4 > MAX_DOCUMENT_PIXELS {
            return Err("Tilemaps exceed the project pixel budget".into());
        }
        let mask = u32at(&data, 22)?;
        let mx = u32at(&data, 26)?;
        let my = u32at(&data, 30)?;
        let md = u32at(&data, 34)?;
        let b = decompress(
            data.get(48..).ok_or("Truncated tilemap")?,
            cw * ch * (bits / 8),
        )?;
        let cols = w.div_ceil(t.width);
        let rows = h.div_ceil(t.height);
        let canonical = x == 0 && y == 0 && cw == cols && ch == rows;
        let mut tiles = vec![0; cw * ch];
        for (i, bytes) in b.chunks_exact(bits / 8).enumerate() {
            let mut buf = [0; 4];
            buf[..bytes.len()].copy_from_slice(bytes);
            let v = u32::from_le_bytes(buf);
            let empty = if legacy_tilesets[ti] {
                v == ((1u64 << bits) - 1) as u32
            } else {
                v & mask == 0
            };
            if empty {
                continue;
            }
            let tile_id = (v & mask) >> mask.trailing_zeros().min(31);
            let tile_id = tile_id
                .checked_add(u32::from(legacy_tilesets[ti]))
                .filter(|&id| id <= 0x1fff_ffff && id as usize <= t.tiles.len())
                .ok_or("Invalid tile reference")?;
            tiles[i] = tile_id
                | if v & mx != 0 { 0x80000000 } else { 0 }
                | if v & my != 0 { 0x40000000 } else { 0 }
                | if v & md != 0 { 0x20000000 } else { 0 };
        }
        if !canonical {
            d.layers[li].cel_meta[f].tilemap = Some(TilemapGeometry {
                x,
                y,
                columns: cw,
                rows: ch,
            });
        }
        d.layers[li].tile_cels[f] = tiles;
    }
    for l in &mut d.layers {
        if let Some(ti) = l.tileset {
            let t = d.tilesets.get(ti).ok_or("Missing tileset")?;
            for c in &mut l.tile_cels {
                if c.is_empty() {
                    let count = w.div_ceil(t.width) * h.div_ceil(t.height);
                    decoded_tile_bytes = decoded_tile_bytes.saturating_add(count * 4);
                    if decoded_tile_bytes / 4 > MAX_DOCUMENT_PIXELS {
                        return Err("Tilemaps exceed the project pixel budget".into());
                    }
                    *c = vec![0; count];
                }
            }
        }
    }
    if palettes.iter().any(|p| p != &palettes[0]) {
        d.frame_palettes = palettes.clone();
    }
    d.palette = palettes[0].clone();
    d.share_linked_rasters();
    d.validate()?;
    Ok(d)
}
fn load_ase_path(path: &Path, stack: &mut Vec<std::path::PathBuf>) -> Result<Document, String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if stack.len() >= 16 || stack.contains(&path) {
        return Err("External tileset reference cycle or depth limit".into());
    }
    stack.push(path.clone());
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let result = decode_ase_with_resolver(&bytes, &mut |file, id| {
        let external = load_ase_path(&path.parent().unwrap().join(file.replace('\\', "/")), stack)?;
        let index = external
            .ase_tilesets
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.index)
            .or_else(|| {
                ((id as usize) < external.tilesets.len()
                    && !external.ase_tilesets.iter().any(|v| v.index == id as usize))
                .then_some(id as usize)
            })
            .ok_or("External tileset ID not found")?;
        external
            .tilesets
            .get(index)
            .cloned()
            .ok_or("External tileset ID not found".into())
    });
    stack.pop();
    result
}
pub fn load(path: &Path) -> Result<Document, String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "pixel" {
        return decode_project(&std::fs::read(path).map_err(|e| e.to_string())?);
    }
    if ext == "ase" || ext == "aseprite" {
        return load_ase_path(path, &mut Vec::new());
    }
    if matches!(ext.as_str(), "gif" | "webp" | "apng") {
        return crate::interchange::load_animation(path);
    }
    if ext == "ora" {
        return crate::interchange::load_ora(path);
    }
    if matches!(ext.as_str(), "tmj" | "tsj") {
        return crate::interchange::load_tiled(path);
    }
    if ext == "json" {
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let name = meta["meta"]["image"]
            .as_str()
            .ok_or("Sheet JSON must name its image")?;
        let im = image::open(path.parent().unwrap_or(Path::new(".")).join(name))
            .map_err(|e| e.to_string())?
            .to_rgba8();
        return crate::workflows::unpack_sheet(&im, &meta);
    }
    if ext == "png" {
        let dec = image::codecs::png::PngDecoder::new(std::io::BufReader::new(
            std::fs::File::open(path).map_err(|e| e.to_string())?,
        ))
        .map_err(|e| e.to_string())?;
        if dec.is_apng().map_err(|e| e.to_string())? {
            return crate::interchange::load_animation(path);
        }
    }
    let im = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = im.dimensions();
    if w == 0
        || h == 0
        || w as usize > MAX_DIMENSION
        || h as usize > MAX_DIMENSION
        || w as usize * h as usize > MAX_PIXELS
    {
        return Err("Image exceeds canvas or memory limits".into());
    }
    let mut d = Document::new(w as usize, h as usize);
    d.layers[0].cels[0] = im.pixels().map(|p| p.0).collect();
    if ext == "png" {
        read_png_metadata(path, &mut d)?;
    }
    Ok(d)
}
pub fn save(path: &Path, d: &Document) -> Result<(), String> {
    if path.extension().and_then(|s| s.to_str()) != Some("pixel") && !d.references.is_empty() {
        return Err("Reference images require .pixel. Save as .pixel or explicitly discard references before saving ASE.".into());
    }
    if path.extension().and_then(|s| s.to_str()) != Some("pixel") && d.needs_baked_ase() {
        return Err(
            "Save an editable .pixel project and use Export baked ASE copy for interchange.".into(),
        );
    }
    let bytes = if path.extension().and_then(|s| s.to_str()) == Some("pixel") {
        encode_project(d)?
    } else {
        encode_ase(d)?
    };
    write_project_bytes(path, bytes)
}

pub fn export_baked_ase(path: &Path, doc: &Document) -> Result<(), String> {
    write_project_bytes(path, encode_ase(doc)?)
}

fn write_project_bytes(path: &Path, bytes: Vec<u8>) -> Result<(), String> {
    let temp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}
pub fn export_png(path: &Path, d: &Document, f: usize, sheet: bool) -> Result<(), String> {
    export_raster_scaled(path, d, f, sheet, 1)
}
pub fn export_raster_scaled(
    path: &Path,
    d: &Document,
    f: usize,
    sheet: bool,
    scale: usize,
) -> Result<(), String> {
    d.validate()?;
    if f >= d.durations.len() {
        return Err("Invalid frame".into());
    }
    let count = if sheet { d.durations.len() } else { 1 };
    if d.width.saturating_mul(count) > MAX_DIMENSION
        || d.width.saturating_mul(count).saturating_mul(d.height) > MAX_PIXELS
    {
        return Err("Export exceeds image limits".into());
    }
    let mut im = image::RgbaImage::new((d.width * count) as u32, d.height as u32);
    for n in 0..count {
        for (i, p) in d
            .composite(if sheet { n } else { f })
            .into_iter()
            .enumerate()
        {
            im.put_pixel(
                (n * d.width + i % d.width) as u32,
                (i / d.width) as u32,
                image::Rgba(p),
            );
        }
    }
    let im = crate::workflows::scale_image(&im, scale)?;
    im.save(path).map_err(|e| e.to_string())?;
    if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        write_png_metadata(path, d)?;
    }
    Ok(())
}
pub fn export_gif(path: &Path, d: &Document) -> Result<(), String> {
    export_gif_scaled(path, d, 1)
}
pub fn export_gif_scaled(path: &Path, d: &Document, scale: usize) -> Result<(), String> {
    export_gif_progress(path, d, scale, |_, _| Ok(()))
}
pub fn export_gif_with_repeat(path: &Path, d: &Document, repeat: u16) -> Result<(), String> {
    export_gif_frames(path, d, repeat, 0..d.durations.len())
}
/// Encode playback order directly without constructing an expanded editable document.
pub fn export_gif_frames(
    path: &Path,
    d: &Document,
    repeat: u16,
    frames: impl IntoIterator<Item = usize>,
) -> Result<(), String> {
    encode_gif(path, d, repeat, frames, 1, |_, _| Ok(()))
}

pub(crate) fn export_gif_progress(
    path: &Path,
    d: &Document,
    scale: usize,
    progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    encode_gif(path, d, 0, 0..d.durations.len(), scale, progress)
}

fn encode_gif(
    path: &Path,
    d: &Document,
    repeat: u16,
    frames: impl IntoIterator<Item = usize>,
    scale: usize,
    mut progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    d.validate()?;
    animation_dimensions(d, scale)?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = image::codecs::gif::GifEncoder::new(file);
    enc.set_repeat(if repeat == 0 {
        image::codecs::gif::Repeat::Infinite
    } else {
        image::codecs::gif::Repeat::Finite(repeat)
    })
    .map_err(|e| e.to_string())?;
    for (done, f) in frames.into_iter().enumerate() {
        progress(done, d.durations.len())?;
        if f >= d.durations.len() {
            return Err("Unknown animation frame".into());
        }
        let bytes = d.composite(f).into_iter().flatten().collect();
        let im = image::RgbaImage::from_raw(d.width as u32, d.height as u32, bytes)
            .ok_or("Invalid canvas")?;
        let im = crate::workflows::scale_image(&im, scale)?;
        enc.encode_frame(image::Frame::from_parts(
            im,
            0,
            0,
            image::Delay::from_numer_denom_ms(d.durations[f] as u32, 1),
        ))
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn animation_dimensions(d: &Document, scale: usize) -> Result<(usize, usize), String> {
    let w = d.width.saturating_mul(scale);
    let h = d.height.saturating_mul(scale);
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION || w * h > MAX_PIXELS {
        return Err("Scaled animation exceeds image limits".into());
    }
    Ok((w, h))
}

/// Stream one composited frame at a time and distribute rounding across the timeline.
pub fn export_mp4(path: &Path, d: &Document, scale: usize) -> Result<(), String> {
    export_mp4_progress(path, d, scale, |_, _| Ok(()))
}

pub(crate) fn export_mp4_progress(
    path: &Path,
    d: &Document,
    scale: usize,
    mut progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    use std::process::{Command, Stdio};
    d.validate()?;
    let (width, height) = animation_dimensions(d, scale)?;
    let size = format!("{width}x{height}");
    let mut command = Command::new("ffmpeg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            &size,
            "-framerate",
            "60",
            "-i",
            "pipe:0",
            "-an",
            "-vf",
            "pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("MP4 export requires FFmpeg on PATH with libx264: {e}"))?;
    let errors = child.stderr.take().unwrap();
    let drain = std::thread::spawn(move || {
        let mut errors = BufReader::new(errors);
        let mut message = Vec::new();
        let _ = (&mut errors).take(8192).read_to_end(&mut message);
        let _ = std::io::copy(&mut errors, &mut std::io::sink());
        String::from_utf8_lossy(&message).trim().to_owned()
    });
    let mut input = child.stdin.take().unwrap();
    let written = (|| -> Result<(), String> {
        let mut elapsed = 0u64;
        let mut emitted = 0u64;
        for (f, &duration) in d.durations.iter().enumerate() {
            progress(f, d.durations.len())?;
            elapsed += u64::from(duration);
            let end = (elapsed * 60).div_ceil(1000);
            if end == emitted {
                continue;
            }
            let rgba = image::RgbaImage::from_raw(
                d.width as u32,
                d.height as u32,
                crate::color::display_pixels(d, f)?
                    .into_iter()
                    .flatten()
                    .collect(),
            )
            .ok_or("Invalid canvas")?;
            let rgba = crate::workflows::scale_image(&rgba, scale)?;
            let rgb: Vec<_> = rgba
                .pixels()
                .flat_map(|p| {
                    let alpha = u16::from(p[3]);
                    [p[0], p[1], p[2]]
                        .map(|channel| ((u16::from(channel) * alpha + 127) / 255) as u8)
                })
                .collect();
            for _ in emitted..end {
                progress(f, d.durations.len())?;
                input.write_all(&rgb).map_err(|e| e.to_string())?;
            }
            emitted = end;
        }
        Ok(())
    })();
    drop(input);
    if written.is_err() {
        let _ = child.kill();
    }
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Ok(status);
        }
        if let Err(error) = progress(d.durations.len(), d.durations.len()) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let stderr = drain.join().unwrap_or_default();
    written?;
    let status = status?;
    if !status.success() {
        return Err(format!("FFmpeg could not encode MP4 ({status}): {stderr}"));
    }
    Ok(())
}
type PngRecord = ([u8; 4], Vec<u8>);
fn png_records(bytes: &[u8]) -> Result<Vec<PngRecord>, String> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG signature".into());
    }
    let mut p = 8;
    let mut out = vec![];
    while p + 12 <= bytes.len() {
        let n = u32::from_be_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
        let end = p
            .checked_add(n)
            .and_then(|n| n.checked_add(12))
            .filter(|&n| n <= bytes.len())
            .ok_or("Invalid PNG chunk")?;
        out.push((
            bytes[p + 4..p + 8].try_into().unwrap(),
            bytes[p..end].to_vec(),
        ));
        p = end;
    }
    Ok(out)
}
fn read_png_metadata(path: &Path, d: &mut Document) -> Result<(), String> {
    for (kind, data) in png_records(&std::fs::read(path).map_err(|e| e.to_string())?)? {
        if matches!(
            &kind,
            b"gAMA"
                | b"cHRM"
                | b"sRGB"
                | b"iCCP"
                | b"tEXt"
                | b"zTXt"
                | b"iTXt"
                | b"eXIf"
                | b"pHYs"
                | b"tIME"
        ) {
            if &kind == b"iCCP" {
                let raw = &data[8..data.len() - 4];
                if let Some(n) = raw.iter().position(|v| *v == 0)
                    && raw.get(n + 1) == Some(&0)
                {
                    let mut icc = vec![];
                    ZlibDecoder::new(&raw[n + 2..])
                        .take(16 * 1024 * 1024 + 1)
                        .read_to_end(&mut icc)
                        .map_err(|e| e.to_string())?;
                    if icc.len() > 16 * 1024 * 1024 {
                        return Err("ICC profile exceeds budget".into());
                    }
                    let mut profile = vec![0; 16];
                    profile[0] = 2;
                    dword(&mut profile, icc.len() as u32);
                    profile.extend(icc);
                    d.color_profile = profile;
                }
            }
            d.metadata.push(MetadataChunk {
                frame: 0,
                kind: 0,
                data,
                owner: "png".into(),
            });
        }
    }
    Ok(())
}
fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
    let mut crc = 0xffffffffu32;
    for b in &out[4..] {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32.wrapping_mul(crc & 1));
        }
    }
    out.extend((!crc).to_be_bytes());
    out
}
fn write_png_metadata(path: &Path, d: &Document) -> Result<(), String> {
    let stored: Vec<_> = d.metadata.iter().filter(|m| m.owner == "png").collect();
    if stored.is_empty() && d.color_profile.is_empty() {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let records = png_records(&bytes)?;
    let mut out = bytes[..8].to_vec();
    for (kind, data) in records {
        if matches!(
            &kind,
            b"gAMA" | b"cHRM" | b"sRGB" | b"iCCP" | b"tEXt" | b"zTXt" | b"iTXt"
        ) {
            continue;
        }
        out.extend(data);
        if &kind == b"IHDR" {
            for m in &stored {
                out.extend(&m.data);
            }
            if !stored.iter().any(|m| m.data.get(4..8) == Some(b"iCCP")) {
                if d.color_profile.len() >= 20 && u16at(&d.color_profile, 0)? == 2 {
                    let n = u32at(&d.color_profile, 16)? as usize;
                    let icc = d
                        .color_profile
                        .get(20..20 + n)
                        .ok_or("Invalid ICC profile")?;
                    let mut payload = b"ICC\0\0".to_vec();
                    payload.extend(compress(icc)?);
                    out.extend(png_chunk(b"iCCP", &payload));
                } else if d.color_profile.len() >= 16 && u16at(&d.color_profile, 0)? == 1 {
                    out.extend(png_chunk(b"sRGB", &[0]));
                }
            }
        }
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}
