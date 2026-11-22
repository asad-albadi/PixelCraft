//! Bounded native clipboard validation shared by workers and wire consumers.
use super::{Clipboard, MAX_DIMENSION, extensions};

pub(super) fn validate(
    clip: &Clipboard,
    limit: usize,
    cancelled: impl Fn() -> bool,
) -> Result<(), String> {
    let check = || {
        if cancelled() {
            Err("Clipboard validation cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let area = clip.w.saturating_mul(clip.h);
    let payload = clip.cells.as_ref().map_or(clip.pixels.len(), Vec::len);
    if clip.w == 0
        || clip.h == 0
        || clip.w > MAX_DIMENSION
        || clip.h > MAX_DIMENSION
        || area != payload
        || area > limit
        || clip.cells.is_some() && (!clip.pixels.is_empty() || clip.tileset.is_none())
    {
        return Err("Invalid clipboard image or pixel budget exceeded".into());
    }
    if clip.selection.as_ref().is_some_and(|mask| {
        mask.selected.len() != area
            || mask.origin[0].checked_add(clip.w as i32 - 1).is_none()
            || mask.origin[1].checked_add(clip.h as i32 - 1).is_none()
    }) || clip
        .palette
        .as_ref()
        .is_some_and(|palette| palette.is_empty() || palette.len() > 256)
    {
        return Err("Invalid clipboard selection or palette".into());
    }
    if let Some(indexed) = &clip.indexed {
        let palette = clip
            .palette
            .as_ref()
            .ok_or("Indexed clipboard image has no palette")?;
        if clip.cells.is_some()
            || indexed.slots.len() != area
            || usize::from(indexed.transparent) >= palette.len()
        {
            return Err("Invalid indexed clipboard image".into());
        }
        for (index, &slot) in indexed.slots.iter().enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if usize::from(slot) >= palette.len() {
                return Err("Invalid indexed clipboard image".into());
            }
        }
    }
    if let Some(tileset) = &clip.tileset {
        let set = &tileset.set;
        let area = set.width.saturating_mul(set.height);
        if set.width == 0
            || set.height == 0
            || set.width > MAX_DIMENSION
            || set.height > MAX_DIMENSION
            || set.name.len() > u16::MAX as usize
            || tileset.flags & 1 != 0
            || tileset.user_data.len() > set.tiles.len().saturating_add(2)
            || area.saturating_mul(set.tiles.len()).saturating_add(payload) > limit
        {
            return Err("Invalid clipboard tileset or pixel budget exceeded".into());
        }
        let mut bytes = 0usize;
        for (index, chunk) in tileset.user_data.iter().enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if chunk.len() < 4 {
                return Err("Invalid clipboard tileset or pixel budget exceeded".into());
            }
            bytes = bytes.saturating_add(chunk.len());
        }
        for (index, chunk) in tileset.external.iter().enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if chunk.len() < 12 {
                return Err("Invalid clipboard tileset or pixel budget exceeded".into());
            }
            bytes = bytes.saturating_add(chunk.len());
        }
        if bytes > extensions::MAX_TILESET_METADATA_BYTES {
            return Err("Invalid clipboard tileset or pixel budget exceeded".into());
        }
        for (index, pixels) in set.tiles.iter().enumerate() {
            if index.is_multiple_of(4096) {
                check()?;
            }
            if pixels.len() != area {
                return Err("Invalid clipboard tileset or pixel budget exceeded".into());
            }
        }
        if let Some(cells) = &clip.cells {
            for (index, cell) in cells.iter().enumerate() {
                if index.is_multiple_of(4096) {
                    check()?;
                }
                if (cell & 0x1fff_ffff) as usize > set.tiles.len() {
                    return Err("Invalid clipboard tileset or pixel budget exceeded".into());
                }
            }
        }
    }
    check()?;
    Ok(())
}
