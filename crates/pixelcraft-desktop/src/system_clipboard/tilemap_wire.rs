//! Bounded native clipboard payloads and interoperable image previews.
use super::Clipboard;
use crate::api::model::{MAX_DOCUMENT_PIXELS, Pixel};
use crate::{ClipboardIndices, ClipboardMask, ClipboardTileset};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io::Write;

const MAGIC: &[u8; 8] = b"PCTILE\0\x01";
const HEADER: usize = MAGIC.len() + size_of::<u32>();
const MAX_BYTES: usize = 32 * 1024 * 1024;
const INDEXED_MAGIC: &[u8; 8] = b"PCINDEX\x01";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Flavor {
    Tilemap,
    Indexed,
}

impl Flavor {
    pub(super) const ALL: [Self; 2] = [Self::Tilemap, Self::Indexed];

    pub(super) fn of(clip: &Clipboard) -> Option<Self> {
        if clip.cells.is_some() {
            Some(Self::Tilemap)
        } else if clip.indexed.is_some() {
            Some(Self::Indexed)
        } else {
            None
        }
    }

    pub(super) const fn limit(self) -> usize {
        match self {
            Self::Tilemap => MAX_BYTES,
            Self::Indexed => 64 * 1024 * 1024,
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) const fn mime(self) -> &'static str {
        match self {
            Self::Tilemap => "application/vnd.pixelcraft.tilemap",
            Self::Indexed => "application/vnd.pixelcraft.indexed-raster",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct IndexedHeader<'a> {
    width: usize,
    height: usize,
    transparent: u8,
    palette: Cow<'a, [Pixel]>,
    origin: Option<[i32; 2]>,
    tileset: Option<Cow<'a, ClipboardTileset>>,
}

struct BoundedBytes(Vec<u8>);

impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other(
                "Tilemap clipboard data exceeds limits",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode(clip: &Clipboard) -> Result<Vec<u8>, String> {
    clip.validate(MAX_DOCUMENT_PIXELS)?;
    if let Some(indexed) = &clip.indexed {
        return encode_indexed(clip, indexed);
    }
    if clip.cells.is_none() {
        return Err("Clipboard image has no packed metadata".into());
    }
    let mut bytes = BoundedBytes(vec![0; HEADER]);
    serde_json::to_writer(&mut bytes, clip).map_err(|error| error.to_string())?;
    let mut bytes = bytes.0;
    let length = (bytes.len() - HEADER) as u32;
    bytes[..MAGIC.len()].copy_from_slice(MAGIC);
    bytes[MAGIC.len()..HEADER].copy_from_slice(&length.to_le_bytes());
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8], flavor: Flavor) -> Option<Clipboard> {
    if flavor == Flavor::Indexed {
        return decode_indexed(bytes);
    }
    if bytes.len() > MAX_BYTES || !bytes.starts_with(MAGIC) {
        return None;
    }
    let count = u32::from_le_bytes(bytes.get(MAGIC.len()..HEADER)?.try_into().ok()?) as usize;
    let payload = bytes.get(HEADER..HEADER.checked_add(count)?)?;
    let clip: Clipboard = serde_json::from_slice(payload).ok()?;
    clip.cells.as_ref()?;
    clip.validate(MAX_DOCUMENT_PIXELS).ok()?;
    validate_tileset(&clip)?;
    Some(clip)
}

fn validate_tileset(clip: &Clipboard) -> Option<()> {
    if let Some(tileset) = &clip.tileset {
        crate::api::extensions::remap_tileset_user_data(&tileset.user_data, &tileset.external, &[])
            .ok()?;
    }
    Some(())
}

fn encode_indexed(clip: &Clipboard, indexed: &ClipboardIndices) -> Result<Vec<u8>, String> {
    let header = IndexedHeader {
        width: clip.w,
        height: clip.h,
        transparent: indexed.transparent,
        palette: Cow::Borrowed(
            clip.palette
                .as_deref()
                .ok_or("Missing indexed clipboard palette")?,
        ),
        origin: clip.selection.as_ref().map(|mask| mask.origin),
        tileset: clip.tileset.as_ref().map(Cow::Borrowed),
    };
    let mut metadata = BoundedBytes(Vec::new());
    serde_json::to_writer(&mut metadata, &header).map_err(|error| error.to_string())?;
    let metadata = metadata.0;
    let mask_bytes = clip
        .selection
        .as_ref()
        .map_or(0, |mask| mask.selected.len().div_ceil(8));
    let size = INDEXED_MAGIC.len()
        + 2 * size_of::<u32>()
        + metadata.len()
        + clip.pixels.len() * 5
        + mask_bytes;
    if size > Flavor::Indexed.limit() {
        return Err("Indexed clipboard data exceeds limits".into());
    }
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(INDEXED_MAGIC);
    bytes.extend_from_slice(&(size as u32).to_le_bytes());
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(clip.pixels.as_flattened());
    bytes.extend_from_slice(&indexed.slots);
    if let Some(mask) = &clip.selection {
        for group in mask.selected.chunks(8) {
            let mut packed = 0;
            for (bit, &selected) in group.iter().enumerate() {
                packed |= u8::from(selected) << bit;
            }
            bytes.push(packed);
        }
    }
    Ok(bytes)
}

fn decode_indexed(bytes: &[u8]) -> Option<Clipboard> {
    if bytes.len() > Flavor::Indexed.limit() || !bytes.starts_with(INDEXED_MAGIC) {
        return None;
    }
    let (size, bytes) = bytes[INDEXED_MAGIC.len()..].split_at_checked(size_of::<u32>())?;
    let size = u32::from_le_bytes(size.try_into().ok()?) as usize;
    // Clipboard APIs may pad an allocation. Only parse the declared payload.
    let bytes = bytes.get(..size.checked_sub(INDEXED_MAGIC.len() + size_of::<u32>())?)?;
    let (length, bytes) = bytes.split_at_checked(size_of::<u32>())?;
    let length = u32::from_le_bytes(length.try_into().ok()?) as usize;
    if length > MAX_BYTES {
        return None;
    }
    let (meta, bytes) = bytes.split_at_checked(length)?;
    let header: IndexedHeader<'_> = serde_json::from_slice(meta).ok()?;
    let count = header.width.checked_mul(header.height)?;
    if count > MAX_DOCUMENT_PIXELS {
        return None;
    }
    let (pixels, bytes) = bytes.split_at_checked(count.checked_mul(4)?)?;
    let (slots, mask) = bytes.split_at_checked(count)?;
    let mask_bytes = header.origin.map_or(0, |_| count.div_ceil(8));
    if mask.len() != mask_bytes {
        return None;
    }
    let clip = Clipboard::new(crate::ClipboardData {
        w: header.width,
        h: header.height,
        pixels: pixels.as_chunks::<4>().0.to_vec(),
        indexed: Some(ClipboardIndices {
            slots: slots.to_vec(),
            transparent: header.transparent,
        }),
        cells: None,
        selection: header.origin.map(|origin| ClipboardMask {
            origin,
            selected: (0..count)
                .map(|i| mask[i / 8] & (1 << (i % 8)) != 0)
                .collect(),
        }),
        palette: Some(header.palette.into_owned()),
        tileset: header.tileset.map(Cow::into_owned),
    });
    clip.validate(MAX_DOCUMENT_PIXELS).ok()?;
    validate_tileset(&clip)?;
    Some(clip)
}

pub(super) fn preview_png(
    width: usize,
    height: usize,
    pixels: &[Pixel],
) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            pixels.as_flattened(),
            width as u32,
            height as u32,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(png)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::api::model::Tileset;
    use crate::{ClipboardMask, ClipboardTileset};
    pub(in crate::system_clipboard) fn sample() -> Clipboard {
        Clipboard::new(crate::ClipboardData {
            w: 2,
            h: 2,
            pixels: Vec::new(),
            indexed: None,
            cells: Some(vec![0x8000_0001, 0, 0, 0x4000_0001]),
            selection: Some(ClipboardMask {
                origin: [1, -2],
                selected: vec![true, false, false, true],
            }),
            palette: Some(vec![[0; 4], [21, 41, 61, 127]]),
            tileset: Some(ClipboardTileset {
                color_mode: crate::api::model::ColorMode::Rgba,
                transparent_index: 0,
                set: Tileset {
                    editor_id: Default::default(),
                    name: "Cross-process terrain".into(),
                    width: 2,
                    height: 1,
                    tiles: vec![vec![[21, 41, 61, 127], [70, 80, 90, 255]].into()],
                },
                flags: 6,
                base_index: 19,
                user_data: vec![vec![0; 4], vec![0; 4], vec![1, 0, 0, 0, 1, 0, b'A']],
                external: vec![],
            }),
        })
    }

    pub(in crate::system_clipboard) fn indexed_sample() -> Clipboard {
        let color = [190, 90, 30, 127];
        let (w, h) = (257, 65);
        let mut clip = Clipboard::raster(w, h, vec![color; w * h]);
        clip.palette = Some(vec![[0; 4], color, color]);
        clip.indexed = Some(ClipboardIndices {
            slots: (0..w * h).map(|index| 1 + (index % 2) as u8).collect(),
            transparent: 0,
        });
        clip.selection = Some(ClipboardMask {
            origin: [-7, 13],
            selected: (0..w * h).map(|index| index % 7 != 0).collect(),
        });
        clip
    }

    #[test]
    fn packed_indexed_wire_roundtrips_indices_and_rejects_malformed_payloads() {
        let clip = indexed_sample();
        let bytes = encode(&clip).unwrap();
        assert!(bytes.len() < clip.w * clip.h * 6);
        let decoded = decode(&bytes, Flavor::Indexed).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(&clip).unwrap()
        );
        assert!(decode(&bytes, Flavor::Tilemap).is_none());
        assert!(decode(&bytes[..bytes.len() - 1], Flavor::Indexed).is_none());
        let mut invalid = bytes;
        let first_slot = invalid.len() - (clip.w * clip.h).div_ceil(8) - clip.w * clip.h;
        invalid[first_slot] = 3;
        assert!(decode(&invalid, Flavor::Indexed).is_none());
    }

    #[test]
    fn packed_tilemap_wire_checks_version_length_shape_and_ids() {
        let clip = sample();
        let encoded = encode(&clip).unwrap();
        let decoded = decode(&encoded, Flavor::Tilemap).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(&clip).unwrap()
        );
        assert!(decode(&encoded[..encoded.len() - 1], Flavor::Tilemap).is_none());
        let mut padded = encoded.clone();
        padded.extend_from_slice(&[0; 4]);
        assert!(decode(&padded, Flavor::Tilemap).is_some());
        padded[MAGIC.len() - 1] = 2;
        assert!(decode(&padded, Flavor::Tilemap).is_none());
        let invalid = std::str::from_utf8(&encoded[HEADER..])
            .unwrap()
            .replace("2147483649", "2147483650");
        let mut invalid_wire = encoded[..HEADER].to_vec();
        invalid_wire.extend_from_slice(invalid.as_bytes());
        assert!(decode(&invalid_wire, Flavor::Tilemap).is_none());
    }
}
