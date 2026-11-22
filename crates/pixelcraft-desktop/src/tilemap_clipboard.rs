use super::{
    Clipboard, ClipboardMask, ClipboardTarget, ClipboardTileset, PasteMode, PixelCraft,
    SelectionMask,
};
use crate::api::{extensions, model::*};
use eframe::egui;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

type TileClipboardResult = Result<(Clipboard, Option<Box<Document>>), String>;

pub(super) struct TileClipboardWork {
    target: ClipboardTarget,
    cut: bool,
    pub(super) paste: Option<ClipboardTarget>,
    cancelled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<TileClipboardResult>,
}
impl Drop for TileClipboardWork {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

type TilePasteResult = Result<Option<(Box<Document>, usize)>, String>;

pub(super) struct TilePasteWork {
    target: ClipboardTarget,
    cancelled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<TilePasteResult>,
}
impl Drop for TilePasteWork {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct TilePasteSite {
    mode: PasteMode,
    layer: usize,
    frame: usize,
    bounds: [i32; 4],
    origin: [i32; 2],
    selected: bool,
}

const TILE_ID: u32 = 0x1fff_ffff;
const EMPTY_USER_DATA: &[u8] = &[0; 4];

struct SelectedCells {
    start: [usize; 2],
    size: [usize; 2],
    selected: Vec<bool>,
}

fn grid(doc: &Document, layer: usize, frame: usize, set: &Tileset) -> TilemapGeometry {
    doc.layers[layer].cel_meta[frame]
        .tilemap
        .clone()
        .unwrap_or(TilemapGeometry {
            x: 0,
            y: 0,
            columns: doc.width.div_ceil(set.width),
            rows: doc.height.div_ceil(set.height),
        })
}

fn user_data(doc: &Document, set: usize) -> Vec<Vec<u8>> {
    let owner = format!("tileset:{set}");
    doc.metadata
        .iter()
        .filter(|chunk| chunk.frame == 0 && chunk.kind == 0x2020 && chunk.owner == owner)
        .map(|chunk| chunk.data.clone())
        .collect()
}

fn external_data(doc: &Document) -> Vec<Vec<u8>> {
    doc.metadata
        .iter()
        .filter(|chunk| chunk.frame == 0 && chunk.kind == 0x2008 && chunk.owner.is_empty())
        .map(|chunk| chunk.data.clone())
        .collect()
}

fn write_user_data(doc: &mut Document, set: usize, data: Vec<Vec<u8>>, external: Option<Vec<u8>>) {
    let owner = format!("tileset:{set}");
    doc.metadata
        .retain(|chunk| !(chunk.frame == 0 && chunk.kind == 0x2020 && chunk.owner == owner));
    doc.metadata
        .extend(data.into_iter().map(|data| MetadataChunk {
            frame: 0,
            kind: 0x2020,
            owner: owner.clone(),
            data,
        }));
    if let Some(data) = external {
        if let Some(chunk) = doc
            .metadata
            .iter_mut()
            .find(|chunk| chunk.frame == 0 && chunk.kind == 0x2008 && chunk.owner.is_empty())
        {
            chunk.data = data;
        } else {
            doc.metadata.push(MetadataChunk {
                frame: 0,
                kind: 0x2008,
                owner: String::new(),
                data,
            });
        }
    }
}

fn tile_data(chunks: &[Vec<u8>], tile: usize) -> &[u8] {
    chunks.get(tile + 1).map_or(EMPTY_USER_DATA, Vec::as_slice)
}

fn tile_key_until(
    tile: &TileRaster,
    metadata: &[u8],
    cancelled: impl Fn() -> bool,
) -> Result<u64, String> {
    let check = || {
        if cancelled() {
            Err("Tile paste cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let mut hasher = DefaultHasher::new();
    metadata.len().hash(&mut hasher);
    for chunk in metadata.chunks(8192) {
        check()?;
        hasher.write(chunk);
    }
    tile.indices.is_some().hash(&mut hasher);
    if let Some(indices) = &tile.indices {
        indices.len().hash(&mut hasher);
        for chunk in indices.chunks(8192) {
            check()?;
            hasher.write(chunk);
        }
    }
    tile.len().hash(&mut hasher);
    for (index, pixel) in tile.iter().enumerate() {
        if index.is_multiple_of(4096) {
            check()?;
        }
        pixel.hash(&mut hasher);
    }
    check()?;
    Ok(hasher.finish())
}

fn bytes_match_until(a: &[u8], b: &[u8], cancelled: impl Fn() -> bool) -> Result<bool, String> {
    if a.len() != b.len() {
        return Ok(false);
    }
    if std::ptr::eq(a.as_ptr(), b.as_ptr()) {
        return Ok(true);
    }
    for (a, b) in a.chunks(8192).zip(b.chunks(8192)) {
        if cancelled() {
            return Err("Tile paste cancelled".into());
        }
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn tiles_match_until(
    a: &TileRaster,
    b: &TileRaster,
    cancelled: impl Fn() -> bool,
) -> Result<bool, String> {
    if cancelled() {
        return Err("Tile paste cancelled".into());
    }
    if a.len() != b.len() {
        return Ok(false);
    }
    match (&a.indices, &b.indices) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            if !bytes_match_until(a, b, &cancelled)? {
                return Ok(false);
            }
        }
        _ => return Ok(false),
    }
    if a.pixels.shares_storage(&b.pixels) {
        return Ok(true);
    }
    for (index, (a, b)) in a.iter().zip(b.iter()).enumerate() {
        if index.is_multiple_of(4096) && cancelled() {
            return Err("Tile paste cancelled".into());
        }
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

fn clone_tileset(doc: &mut Document, source: &ClipboardTileset) -> Result<usize, String> {
    let metadata = extensions::remap_tileset_user_data(
        &source.user_data,
        &source.external,
        &external_data(doc),
    )?;
    let index = doc.tilesets.len();
    let mut used: HashSet<_> = (0..index).map(|index| index as u32).collect();
    used.extend(doc.ase_tilesets.iter().map(|info| info.id));
    let id = (0..=u32::MAX)
        .find(|id| !used.contains(id))
        .ok_or("No ASE tileset ID available")?;
    let mut copied = source.set.clone();
    copied.editor_id = EditorId::default();
    for tile in &mut copied.tiles {
        tile.editor_id = EditorId::default();
    }
    doc.tilesets.push(copied);
    doc.ase_tilesets.push(AseTilesetInfo {
        index,
        id,
        flags: (source.flags & !1) | 2,
        base_index: source.base_index,
        external: None,
    });
    write_user_data(doc, index, metadata.data, metadata.external);
    Ok(index)
}

#[derive(Debug)]
struct MergedTiles {
    remap: HashMap<usize, u32>,
    changed: bool,
}

// paint_tiles mutates placement backing only; no-op writes retain their backing.
// Include every linked frame, whose grid geometry can differ from the active cel.
fn placement_storage_changed_until(
    current: &Layer,
    original: &Layer,
    cancelled: impl Fn() -> bool,
) -> Result<bool, String> {
    if cancelled() {
        return Err("Tile clipboard operation cancelled".into());
    }
    if current.tile_cels.len() != original.tile_cels.len() {
        return Ok(true);
    }
    for (index, (current, original)) in current
        .tile_cels
        .iter()
        .zip(&original.tile_cels)
        .enumerate()
    {
        if index.is_multiple_of(4096) && cancelled() {
            return Err("Tile clipboard operation cancelled".into());
        }
        if current.backing_id() != original.backing_id() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
fn merge_tiles(
    doc: &mut Document,
    set: usize,
    source: &ClipboardTileset,
    ids: BTreeSet<usize>,
) -> Result<HashMap<usize, u32>, String> {
    merge_tiles_until(doc, set, source, ids, || false).map(|merged| merged.remap)
}
fn merge_tiles_until(
    doc: &mut Document,
    set: usize,
    source: &ClipboardTileset,
    ids: BTreeSet<usize>,
    cancelled: impl Fn() -> bool,
) -> Result<MergedTiles, String> {
    let check = || {
        if cancelled() {
            Err("Tile paste cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let mut remap = HashMap::new();
    if ids.is_empty() {
        return Ok(MergedTiles {
            remap,
            changed: false,
        });
    }
    let mut source_data = Vec::with_capacity(ids.len());
    for (index, &id) in ids.iter().enumerate() {
        if index.is_multiple_of(4096) {
            check()?;
        }
        if id == 0 || id > source.set.tiles.len() {
            return Err("Invalid clipboard tile ID".into());
        }
        source_data.push(tile_data(&source.user_data, id).to_vec());
    }
    let metadata =
        extensions::remap_tileset_user_data(&source_data, &source.external, &external_data(doc))?;
    check()?;
    let mut destination_data = user_data(doc, set);
    let tiles = &mut doc
        .tilesets
        .get_mut(set)
        .ok_or("Missing destination tileset")?
        .tiles;
    let existing = tiles.len();
    let mut by_content: Option<HashMap<u64, Vec<usize>>> = None;
    let mut metadata_changed = metadata.external.is_some();
    let mut changed = metadata_changed;
    let mut claimed = HashSet::new();
    for (id, original_data) in ids.into_iter().zip(&metadata.data) {
        check()?;
        let original = &source.set.tiles[id - 1];
        let same = |candidate: usize| -> Result<bool, String> {
            if claimed.contains(&candidate) {
                return Ok(false);
            }
            let Some(tile) = tiles.get(candidate - 1) else {
                return Ok(false);
            };
            Ok(bytes_match_until(
                tile_data(&destination_data, candidate),
                original_data,
                &cancelled,
            )? && tiles_match_until(tile, original, &cancelled)?)
        };
        let mut target = if same(id)? { Some(id) } else { None };
        if target.is_none() {
            if by_content.is_none() {
                let mut contents: HashMap<u64, Vec<usize>> = HashMap::new();
                for (index, tile) in tiles[..existing].iter().enumerate() {
                    check()?;
                    let id = index + 1;
                    contents
                        .entry(tile_key_until(
                            tile,
                            tile_data(&destination_data, id),
                            &cancelled,
                        )?)
                        .or_default()
                        .push(id);
                }
                by_content = Some(contents);
            }
            let key = tile_key_until(original, original_data, &cancelled)?;
            if let Some(ids) = by_content.as_ref().and_then(|map| map.get(&key)) {
                for &candidate in ids {
                    check()?;
                    if same(candidate)? {
                        target = Some(candidate);
                        break;
                    }
                }
            }
        }
        let target = if let Some(target) = target {
            target
        } else {
            let mut inserted = original.clone();
            inserted.editor_id = EditorId::default();
            tiles.push(inserted);
            changed = true;
            let target = tiles.len();
            if original_data != EMPTY_USER_DATA {
                destination_data.resize_with(target + 2, || EMPTY_USER_DATA.to_vec());
                destination_data[target + 1].clone_from(original_data);
                metadata_changed = true;
            }
            target
        };
        claimed.insert(target);
        remap.insert(id, target as u32);
    }
    check()?;
    if metadata_changed {
        write_user_data(doc, set, destination_data, metadata.external);
    }
    Ok(MergedTiles { remap, changed })
}

#[cfg(test)]
fn prepare_tile_colors(
    doc: &mut Document,
    frame: usize,
    source: &ClipboardTileset,
) -> Result<ClipboardTileset, String> {
    prepare_selected_tile_colors(doc, frame, source, None)
}

#[cfg(test)]
fn prepare_selected_tile_colors(
    doc: &mut Document,
    frame: usize,
    source: &ClipboardTileset,
    selected: Option<&BTreeSet<usize>>,
) -> Result<ClipboardTileset, String> {
    prepare_selected_tile_colors_until(doc, frame, source, selected, || false)
}

fn prepare_selected_tile_colors_until(
    doc: &mut Document,
    frame: usize,
    source: &ClipboardTileset,
    selected: Option<&BTreeSet<usize>>,
    cancelled: impl Fn() -> bool,
) -> Result<ClipboardTileset, String> {
    if frame >= doc.durations.len() {
        return Err("Choose a valid destination frame".into());
    }
    let area = source
        .set
        .width
        .checked_mul(source.set.height)
        .ok_or("Invalid tile size")?;
    if area == 0 || area.saturating_mul(source.set.tiles.len()) > MAX_PIXELS {
        return Err("Invalid clipboard tile raster".into());
    }
    for (index, tile) in source.set.tiles.iter().enumerate() {
        if index.is_multiple_of(4096) && cancelled() {
            return Err("Tile paste cancelled".into());
        }
        if tile.len() != area
            || tile
                .indices
                .as_ref()
                .is_some_and(|slots| slots.len() != area)
        {
            return Err("Invalid clipboard tile raster".into());
        }
    }
    let mut result = source.clone();
    let indexed = doc.color_mode == ColorMode::Indexed;
    let mut palette = doc.palette_for(frame).to_vec();
    if indexed && palette.is_empty() {
        return Err("Missing destination palette".into());
    }
    if indexed && let Some(mask) = palette.get_mut(doc.transparent_index) {
        mask[3] = 0;
    }
    let mut base = doc.palette.clone();
    let mut shared = HashMap::new();
    let mut colors = HashMap::new();
    for (index, tile) in result.set.tiles.iter_mut().enumerate() {
        if cancelled() {
            return Err("Tile paste cancelled".into());
        }
        if selected.is_some_and(|ids| !ids.contains(&(index + 1))) {
            continue;
        }
        let key = (
            tile.backing_id(),
            tile.indices.as_ref().map(std::sync::Arc::as_ptr),
            tile.len(),
        );
        if let Some((pixels, indices)) = shared.get(&key) {
            tile.pixels = Pixels::clone(pixels);
            tile.indices = Option::clone(indices);
            continue;
        }
        let before = tile.clone();
        let mut slots = Vec::new();
        for i in 0..tile.len() {
            if i % 8192 == 0 && cancelled() {
                return Err("Tile paste cancelled".into());
            }
            let pixel = before[i];
            let color = if indexed {
                let exact = before
                    .indices
                    .as_ref()
                    .and_then(|indices| indices.get(i))
                    .map(|&slot| slot as usize)
                    .filter(|&slot| palette.get(slot) == Some(&pixel));
                let source_slot = before
                    .indices
                    .as_ref()
                    .and_then(|indices| indices.get(i))
                    .copied();
                let authored_mask = source.color_mode == ColorMode::Indexed
                    && source_slot.map(usize::from) == Some(source.transparent_index);
                let slot = if let Some(slot) = exact.filter(|_| pixel[3] != 0 || !authored_mask) {
                    slot
                } else if pixel[3] == 0 {
                    if doc.transparent_index >= palette.len() {
                        return Err("Missing destination transparent slot".into());
                    }
                    doc.transparent_index
                } else if let Some(&slot) = colors.get(&pixel) {
                    slot
                } else {
                    let slot = palette
                        .iter()
                        .enumerate()
                        .filter(|(slot, color)| *slot != doc.transparent_index && color[3] > 0)
                        .min_by_key(|(_, color)| {
                            pixel
                                .iter()
                                .zip(color.iter())
                                .map(|(&a, &b)| (i32::from(a) - i32::from(b)).pow(2) as u32)
                                .sum::<u32>()
                        })
                        .map(|(slot, _)| slot)
                        .ok_or("The destination palette has no visible swatch")?;
                    colors.insert(pixel, slot);
                    slot
                };
                slots.push(slot as u8);
                if slot >= base.len() {
                    base.extend_from_slice(&doc.palette_for(frame)[base.len()..=slot]);
                }
                let mut color = base[slot];
                if slot == doc.transparent_index {
                    color[3] = 0;
                }
                color
            } else {
                convert(pixel, doc.color_mode, &palette)
            };
            if tile[i] != color {
                tile.pixels.set(i, color);
            }
        }
        tile.indices = if indexed {
            let same = if let Some(old) = before.indices.as_deref() {
                bytes_match_until(old, &slots, &cancelled)?
            } else {
                false
            };
            Some(if same {
                std::sync::Arc::clone(before.indices.as_ref().unwrap())
            } else {
                std::sync::Arc::new(slots)
            })
        } else {
            None
        };
        shared.insert(key, (tile.pixels.clone(), tile.indices.clone()));
    }
    if cancelled() {
        return Err("Tile paste cancelled".into());
    }
    doc.palette = base;
    result.color_mode = doc.color_mode;
    result.transparent_index = doc.transparent_index;
    Ok(result)
}

struct TileCopySite {
    layer: usize,
    frame: usize,
    bounds: [i32; 4],
    mask: Option<SelectionMask>,
    polygon: Vec<(i32, i32)>,
    selected: bool,
}

fn selected_tile_cells(
    doc: &Document,
    site: &TileCopySite,
    cancelled: impl Fn() -> bool,
) -> Result<SelectedCells, String> {
    let pixel_selected = |x: i32, y: i32| -> Result<bool, String> {
        if let Some(mask) = &site.mask {
            Ok(doc.index(x, y).is_some_and(|i| mask.get(i) == Some(&true)))
        } else {
            let [left, top, width, height] = site.bounds;
            let inside = x >= left && y >= top && x < left + width && y < top + height;
            if !inside || site.polygon.is_empty() {
                return Ok(inside);
            }
            crate::api::workflows::inside_polygon_until(&site.polygon, x, y, &cancelled)
        }
    };
    let set = &doc.tilesets[doc.layers[site.layer]
        .tileset
        .ok_or("Choose a tile layer")?];
    let g = grid(doc, site.layer, site.frame, set);
    let [x, y, width, height] = site.bounds;
    let left = x.max(0);
    let top = y.max(0);
    let right = (x + width).min(doc.width as i32);
    let bottom = (y + height).min(doc.height as i32);
    if left >= right || top >= bottom {
        return Err("No tiles selected".into());
    }
    let w = set.width as i32;
    let h = set.height as i32;
    let x0 = (left - g.x).div_euclid(w).clamp(0, g.columns as i32) as usize;
    let y0 = (top - g.y).div_euclid(h).clamp(0, g.rows as i32) as usize;
    let x1 = ((right - 1 - g.x).div_euclid(w) + 1).clamp(0, g.columns as i32) as usize;
    let y1 = ((bottom - 1 - g.y).div_euclid(h) + 1).clamp(0, g.rows as i32) as usize;
    if x0 >= x1 || y0 >= y1 {
        return Err("No tiles selected".into());
    }
    let size = [x1 - x0, y1 - y0];
    let mut selected = vec![false; size[0] * size[1]];
    let mut visited = 0usize;
    for row in y0..y1 {
        if cancelled() {
            return Err("Tile clipboard operation cancelled".into());
        }
        for col in x0..x1 {
            let x = g.x + col as i32 * w;
            let y = g.y + row as i32 * h;
            let mut hit = false;
            'pixels: for py in top.max(y)..bottom.min(y + h) {
                for px in left.max(x)..right.min(x + w) {
                    if visited.is_multiple_of(4096) && cancelled() {
                        return Err("Tile clipboard operation cancelled".into());
                    }
                    visited += 1;
                    if pixel_selected(px, py)? {
                        hit = true;
                        break 'pixels;
                    }
                }
            }
            selected[(row - y0) * size[0] + col - x0] = hit;
        }
    }
    selected
        .contains(&true)
        .then_some(SelectedCells {
            start: [x0, y0],
            size,
            selected,
        })
        .ok_or_else(|| "No tiles selected".into())
}

fn build_tile_clipboard(
    doc: &Document,
    layer_index: usize,
    frame: usize,
    region: SelectedCells,
    selected: bool,
    cut: bool,
    cancelled: impl Fn() -> bool,
) -> TileClipboardResult {
    if cancelled() {
        return Err("Tile clipboard operation cancelled".into());
    }
    let layer = &doc.layers[layer_index];
    let index = layer.tileset.ok_or("Choose a tile layer")?;
    let set = &doc.tilesets[index];
    let g = grid(doc, layer_index, frame, set);
    let metadata =
        extensions::remap_tileset_user_data(&user_data(doc, index), &external_data(doc), &[])?;
    let source = &layer.tile_cels[frame];
    let [x, y] = region.start;
    let [w, h] = region.size;
    let cells = region
        .selected
        .iter()
        .enumerate()
        .map(|(i, &selected)| {
            if i % 8192 == 0 && cancelled() {
                return Err("Tile clipboard operation cancelled".to_string());
            }
            Ok(if selected {
                source[(y + i / w) * g.columns + x + i % w]
            } else {
                0
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let info = doc.ase_tilesets.iter().find(|info| info.index == index);
    let mut snapshot = set.clone();
    if doc.color_mode == ColorMode::Indexed {
        let mut shared = HashMap::new();
        for tile in &mut snapshot.tiles {
            if cancelled() {
                return Err("Tile clipboard operation cancelled".into());
            }
            let key = (
                tile.backing_id(),
                tile.indices.as_ref().map(std::sync::Arc::as_ptr),
                tile.len(),
            );
            if let Some(pixels) = shared.get(&key) {
                tile.pixels = Pixels::clone(pixels);
                continue;
            }
            let before = tile.clone();
            for i in 0..tile.len() {
                if i % 8192 == 0 && cancelled() {
                    return Err("Tile clipboard operation cancelled".into());
                }
                let color = before.pixel_for_palette(
                    i,
                    &doc.palette,
                    doc.palette_for(frame),
                    doc.transparent_index,
                );
                if tile[i] != color {
                    tile.pixels.set(i, color);
                }
            }
            shared.insert(key, tile.pixels.clone());
        }
    }
    let clip = Clipboard::new(crate::ClipboardData {
        w,
        h,
        pixels: Vec::new(),
        indexed: None,
        cells: Some(cells),
        selection: selected.then(|| ClipboardMask {
            origin: [x as i32, y as i32],
            selected: region.selected.clone(),
        }),
        palette: Some(doc.palette_for(frame).to_vec()),
        tileset: Some(ClipboardTileset {
            set: snapshot,
            color_mode: doc.color_mode,
            transparent_index: doc.transparent_index,
            flags: info.map_or(6, |info| info.flags & !1),
            base_index: info.map_or(1, |info| info.base_index),
            user_data: metadata.data,
            external: metadata.external.into_iter().collect(),
        }),
    });

    if cancelled() {
        return Err("Tile clipboard operation cancelled".into());
    }
    let result = if cut {
        let mut result = doc.clone();
        let erase = region
            .selected
            .iter()
            .enumerate()
            .take_while(|_| !cancelled())
            .filter_map(|(i, &selected)| selected.then_some((x + i % w, y + i / w, 0)));
        result.paint_tiles(layer_index, frame, erase)?;
        if cancelled() {
            return Err("Tile clipboard operation cancelled".into());
        }
        if placement_storage_changed_until(
            &result.layers[layer_index],
            &doc.layers[layer_index],
            &cancelled,
        )? {
            Some(Box::new(result))
        } else {
            None
        }
    } else {
        None
    };
    Ok((clip, result))
}

fn build_tile_paste(
    source_doc: &Document,
    site: &TilePasteSite,
    clip: &Clipboard,
    cancelled: impl Fn() -> bool,
) -> TilePasteResult {
    clip.validate_until(MAX_PIXELS, &cancelled)?;
    if cancelled() {
        return Err("Tile paste cancelled".into());
    }
    let mode = site.mode;
    if mode == PasteMode::Reference {
        let (width, height, pixels) = clip.native_bitmap_until(&cancelled)?;
        let mut doc = source_doc.clone();
        doc.references
            .push(crate::api::productivity::ReferenceImage {
                behind: false,
                floating: false,
                name: "Pasted reference".into(),
                width,
                height,
                pixels: pixels.into(),
                x: site.origin[0] as f32,
                y: site.origin[1] as f32,
                scale: 1.,
                opacity: 255,
                visible: true,
                options: Default::default(),
            });
        if cancelled() {
            return Err("Reference paste cancelled".into());
        }
        doc.validate_until(&cancelled)?;
        return Ok(Some((Box::new(doc), site.layer)));
    }
    let source = clip.tileset.as_ref().ok_or("Missing clipboard tileset")?;
    let cells = clip.cells.as_ref().ok_or("Missing clipboard tilemap")?;
    let (w, h) = (source.set.width, source.set.height);
    let new_sprite = mode == PasteMode::Sprite;
    let mut doc = if new_sprite {
        let width = clip.w.checked_mul(w).ok_or("Invalid sprite width")?;
        let height = clip.h.checked_mul(h).ok_or("Invalid sprite height")?;
        if width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || width.saturating_mul(height) > MAX_PIXELS
        {
            return Err("Clipboard tilemap exceeds the sprite limits".into());
        }
        let mut doc = Document::new(width, height);
        if let Some(palette) = &clip.palette {
            doc.palette.clone_from(palette);
        }
        doc.transparent_index = source.transparent_index;
        doc.set_color_mode(source.color_mode);
        doc
    } else {
        source_doc.clone()
    };
    let frame = if new_sprite { 0 } else { site.frame };
    let target = (mode == PasteMode::InPlace)
        .then_some(site.layer)
        .and_then(|layer| doc.layers[layer].tileset.map(|set| (layer, set)));
    let prepared = if let Some((layer, set)) = target {
        let destination = &doc.tilesets[set];
        if (destination.width, destination.height) != (w, h) {
            return Err("Clipboard tiles must match the destination grid".into());
        }
        let g = grid(&doc, layer, frame, destination);
        let [x, y, _, _] = if site.selected { site.bounds } else { [0; 4] };
        let left = (x - g.x).div_euclid(w as i32);
        let top = (y - g.y).div_euclid(h as i32);
        let mut ids = BTreeSet::new();
        for (i, &cell) in cells.iter().enumerate() {
            if i.is_multiple_of(4096) && cancelled() {
                return Err("Tile paste cancelled".into());
            }
            let x = left + (i % clip.w) as i32;
            let y = top + (i / clip.w) as i32;
            if cell & TILE_ID != 0
                && clip.selection.as_ref().is_none_or(|mask| mask.selected[i])
                && (0..g.columns as i32).contains(&x)
                && (0..g.rows as i32).contains(&y)
            {
                ids.insert((cell & TILE_ID) as usize);
            }
        }
        if ids.is_empty() {
            return Ok(None);
        }
        prepare_selected_tile_colors_until(&mut doc, frame, source, Some(&ids), &cancelled)?
    } else {
        prepare_selected_tile_colors_until(&mut doc, frame, source, None, &cancelled)?
    };
    let source = &prepared;
    // Palette promotion is a real edit even if no placement word changes.
    let mut changed = doc.palette != source_doc.palette;
    let (layer, set) = if let Some((layer, set)) = target {
        (layer, set)
    } else {
        let set = clone_tileset(&mut doc, source)?;
        doc.add_tile_layer(set)?;
        changed = true;
        if new_sprite {
            doc.layers.remove(0);
        }
        (doc.layers.len() - 1, set)
    };
    let destination = &doc.tilesets[set];
    if (destination.width, destination.height) != (w, h) {
        return Err("Clipboard tiles must match the destination grid".into());
    }
    let g = grid(&doc, layer, frame, destination);
    let [x, y, _, _] = if new_sprite {
        [0; 4]
    } else if site.selected {
        site.bounds
    } else {
        [0; 4]
    };
    let left = (x - g.x).div_euclid(w as i32);
    let top = (y - g.y).div_euclid(h as i32);
    let placements = || {
        cells
            .iter()
            .enumerate()
            .take_while(|_| !cancelled())
            .filter_map(|(i, &cell)| {
                let x = left + (i % clip.w) as i32;
                let y = top + (i / clip.w) as i32;
                (cell & TILE_ID != 0
                    && clip.selection.as_ref().is_none_or(|mask| mask.selected[i])
                    && (0..g.columns as i32).contains(&x)
                    && (0..g.rows as i32).contains(&y))
                .then_some((x as usize, y as usize, cell))
            })
    };
    if cancelled() {
        return Err("Tile paste cancelled".into());
    }
    let remap = if target.is_some() {
        let merged = merge_tiles_until(
            &mut doc,
            set,
            source,
            placements()
                .map(|(_, _, cell)| (cell & TILE_ID) as usize)
                .collect(),
            &cancelled,
        )?;
        changed |= merged.changed;
        merged.remap
    } else {
        HashMap::new()
    };
    let placements = placements().map(|(x, y, cell)| {
        let id = cell & TILE_ID;
        let remapped = remap.get(&(id as usize)).copied().unwrap_or(id);
        (x, y, (cell & !TILE_ID) | remapped)
    });
    doc.paint_tiles(layer, frame, placements)?;
    if cancelled() {
        return Err("Tile paste cancelled".into());
    }
    doc.validate_until(&cancelled)?;

    if !changed && target.is_some() {
        changed = placement_storage_changed_until(
            &doc.layers[layer],
            &source_doc.layers[site.layer],
            &cancelled,
        )?;
    }
    if new_sprite || changed {
        Ok(Some((Box::new(doc), layer)))
    } else {
        Ok(None)
    }
}

impl PixelCraft {
    pub(super) fn uses_tilemap_clipboard(&self) -> bool {
        self.doc.layers[self.layer].tileset.is_some() && self.tile_pixel_mode.is_none()
    }

    pub(super) fn copy_tilemap(&mut self, cut: bool) -> Result<(), String> {
        self.tile_clipboard_work = None;
        self.tile_paste_work = None;
        if cut && !self.doc.editable(self.layer) {
            return Err("Choose an editable tile layer".into());
        }
        let site = TileCopySite {
            layer: self.layer,
            frame: self.frame,
            bounds: self.selection_bounds(),
            mask: self.studio.mask.clone(),
            polygon: self.studio.polygon.clone(),
            selected: self.selection.is_some(),
        };
        let set = &self.doc.tilesets[self.doc.layers[self.layer]
            .tileset
            .ok_or("Choose a tile layer")?];
        let pixels = set
            .width
            .saturating_mul(set.height)
            .saturating_mul(set.tiles.len())
            .saturating_add(self.doc.layers[self.layer].tile_cels[self.frame].len());
        if pixels <= 65_536 {
            let region = selected_tile_cells(&self.doc, &site, || false)?;
            let (clip, result) = build_tile_clipboard(
                &self.doc,
                self.layer,
                self.frame,
                region,
                self.selection.is_some(),
                cut,
                || false,
            )?;
            self.finish_tile_copy(clip, result, cut);
            return Ok(());
        }
        self.clipboard_target = None;
        let target = self.clipboard_destination(PasteMode::InPlace);
        let doc = self.doc.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        let (tx, receiver) = mpsc::channel();
        self.tile_clipboard_work = Some(TileClipboardWork {
            target,
            cut,
            paste: None,
            cancelled,
            receiver,
        });
        std::thread::spawn(move || {
            let result = selected_tile_cells(&doc, &site, || flag.load(Ordering::Relaxed))
                .and_then(|region| {
                    build_tile_clipboard(
                        &doc,
                        site.layer,
                        site.frame,
                        region,
                        site.selected,
                        cut,
                        || flag.load(Ordering::Relaxed),
                    )
                });
            let _ = tx.send(result);
        });
        self.status = if cut {
            "Preparing tile cut..."
        } else {
            "Preparing tile copy..."
        }
        .into();
        Ok(())
    }

    fn finish_tile_copy(&mut self, clip: Clipboard, result: Option<Box<Document>>, cut: bool) {
        if let Some(result) = result {
            self.remember_action("Cut tiles");
            self.doc = *result;
        }
        self.store_clipboard(clip, if cut { "Cut tiles" } else { "Copied tiles" });
    }

    pub(super) fn poll_tile_clipboard(&mut self, ctx: &egui::Context) {
        let Some(work) = &self.tile_clipboard_work else {
            return;
        };
        let result = match work.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("Tile clipboard worker stopped".into()),
        };
        let mut work = self.tile_clipboard_work.take().unwrap();
        if work.cut && work.target != self.clipboard_destination(PasteMode::InPlace) {
            self.status = "Tile cut cancelled because the source changed".into();
            return;
        }
        match result {
            Ok((clip, result)) => {
                let paste = work
                    .paste
                    .take()
                    .filter(|target| *target == self.clipboard_destination(target.mode));
                self.finish_tile_copy(clip, result, work.cut);
                if let Some(target) = paste {
                    self.paste_internal(target.mode);
                }
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn paste_tilemap(
        &mut self,
        mode: PasteMode,
        clip: &Clipboard,
    ) -> Result<(), String> {
        self.tile_paste_work = None;
        let (tile_width, tile_height, definition_pixels) = match &clip.tileset {
            Some(source) => (
                source.set.width,
                source.set.height,
                source
                    .set
                    .width
                    .saturating_mul(source.set.height)
                    .saturating_mul(source.set.tiles.len()),
            ),
            None if mode == PasteMode::Reference => (1, 1, clip.pixels.len()),
            None => return Err("Missing clipboard tileset".into()),
        };
        let destination_pixels = if mode == PasteMode::Sprite {
            clip.w
                .saturating_mul(tile_width)
                .saturating_mul(clip.h)
                .saturating_mul(tile_height)
        } else {
            let definitions =
                self.doc
                    .tilesets
                    .iter()
                    .fold(self.doc.logical_pixels(), |total, set| {
                        total.saturating_add(
                            set.width
                                .saturating_mul(set.height)
                                .saturating_mul(set.tiles.len()),
                        )
                    });
            self.doc
                .layers
                .iter()
                .flat_map(|layer| &layer.tile_cels)
                .fold(definitions, |total, cells| {
                    total.saturating_add(cells.len())
                })
        };
        let expanded_pixels = clip
            .w
            .saturating_mul(tile_width)
            .saturating_mul(clip.h)
            .saturating_mul(tile_height);
        let pixels = definition_pixels
            .saturating_add(expanded_pixels)
            .saturating_add(clip.cells.as_ref().map_or(0, Vec::len))
            .saturating_add(destination_pixels);
        let site = TilePasteSite {
            mode,
            layer: self.layer,
            frame: self.frame,
            bounds: self.selection_bounds(),
            origin: self.selection.map_or([0, 0], |(x, y, _, _)| [x, y]),
            selected: self.selection.is_some(),
        };
        if pixels <= 65_536 {
            if let Some((doc, layer)) = build_tile_paste(&self.doc, &site, clip, || false)? {
                self.finish_tile_paste(mode, doc, layer);
            }
            return Ok(());
        }
        self.clipboard_target = None;
        let target = self.clipboard_destination(mode);
        let doc = self.doc.clone();
        let clip = clip.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        let (tx, receiver) = mpsc::channel();
        self.tile_paste_work = Some(TilePasteWork {
            target,
            cancelled,
            receiver,
        });
        std::thread::spawn(move || {
            let result = build_tile_paste(&doc, &site, &clip, || flag.load(Ordering::Relaxed));
            let _ = tx.send(result);
        });
        self.status = "Preparing tile paste...".into();
        Ok(())
    }

    fn finish_tile_paste(&mut self, mode: PasteMode, doc: Box<Document>, layer: usize) {
        if mode == PasteMode::Sprite {
            self.open_derived_sprite(*doc, "Pasted as a tilemap sprite");
        } else {
            self.remember_action(if mode == PasteMode::Reference {
                "Paste as reference"
            } else {
                "Paste tiles"
            });
            self.doc = *doc;
            self.layer = layer;
            self.status = if mode == PasteMode::Reference {
                "Pasted as a reference image"
            } else {
                "Pasted tiles"
            }
            .into();
        }
    }

    pub(super) fn poll_tile_paste(&mut self, ctx: &egui::Context) {
        let Some(work) = &self.tile_paste_work else {
            return;
        };
        let result = match work.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("Tile paste worker stopped".into()),
        };
        let work = self.tile_paste_work.take().unwrap();
        if work.target != self.clipboard_destination(work.target.mode) {
            self.status = "Tile paste cancelled because the destination changed".into();
            return;
        }
        match result {
            Ok(Some((doc, layer))) => self.finish_tile_paste(work.target.mode, doc, layer),
            Ok(None) => self.status = "No tile changes to paste".into(),
            Err(error) => self.status = error,
        }
    }
}

#[cfg(test)]
mod indexed_slot_tests {
    use super::*;
    use crate::api::formats;
    use std::sync::Arc;

    fn palette_source(pixels: Vec<Pixel>, slots: Vec<u8>) -> ClipboardTileset {
        let width = pixels.len();
        let mut tile: TileRaster = pixels.into();
        tile.indices = Some(Arc::new(slots));
        ClipboardTileset {
            set: Tileset {
                editor_id: Default::default(),
                name: "source".into(),
                width,
                height: 1,
                tiles: vec![tile],
            },
            color_mode: ColorMode::Indexed,
            transparent_index: 0,
            flags: 6,
            base_index: 1,
            user_data: vec![],
            external: vec![],
        }
    }

    #[test]
    fn cancellation_interrupts_selection_scan_inside_one_large_tile() {
        let mut doc = Document::new(512, 512);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Large tile".into(),
            width: 512,
            height: 512,
            tiles: vec![Pixels::empty(512 * 512).into()],
        });
        doc.add_tile_layer(0).unwrap();
        let site = TileCopySite {
            layer: 1,
            frame: 0,
            bounds: [0, 0, 512, 512],
            mask: Some(vec![false; 512 * 512].into()),
            polygon: vec![],
            selected: true,
        };
        let polls = std::cell::Cell::new(0);
        let result = selected_tile_cells(&doc, &site, || {
            polls.set(polls.get() + 1);
            polls.get() >= 3
        });
        assert!(result.is_err());
        assert!(
            result.err().unwrap().contains("cancelled"),
            "Selection scan completed the whole tile before observing cancellation"
        );
    }

    #[test]
    fn empty_tile_cut_does_not_return_a_document_for_ui_comparison_or_publication() {
        let mut doc = Document::new(2, 1);
        let mut source = palette_source(vec![[10, 20, 30, 255]], vec![1]);
        source.set.tiles[0].indices = None;
        doc.tilesets.push(source.set);
        doc.add_tile_layer(0).unwrap();
        let (_, result) = build_tile_clipboard(
            &doc,
            1,
            0,
            SelectedCells {
                start: [0, 0],
                size: [1, 1],
                selected: vec![true],
            },
            true,
            true,
            || false,
        )
        .unwrap();
        assert!(
            result.is_none(),
            "No-op cut returned a full document for UI-thread comparison"
        );
    }

    #[test]
    fn cut_detects_a_changed_linked_peer_with_different_grid_geometry() {
        let mut doc = Document::new(4, 2);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Tiles".into(),
            width: 1,
            height: 1,
            tiles: vec![vec![[10, 20, 30, 255]].into()],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[0] = vec![0, 0, 1, 0, 0, 0, 0, 0].into();
        doc.add_frame(0, true);
        doc.link_cel(1, 1, 0).unwrap();
        doc.layers[1].cel_meta[1].tilemap = Some(TilemapGeometry {
            x: 0,
            y: 0,
            columns: 2,
            rows: 4,
        });
        doc.validate().unwrap();
        let (_, result) = build_tile_clipboard(
            &doc,
            1,
            0,
            SelectedCells {
                start: [0, 1],
                size: [1, 1],
                selected: vec![true],
            },
            true,
            true,
            || false,
        )
        .unwrap();
        let result = result.expect("The linked peer changed even though the active cell was empty");
        assert_eq!(result.layers[1].tile_cels[0][4], 0);
        assert_eq!(result.layers[1].tile_cels[1][2], 0);
        assert_eq!(doc.layers[1].tile_cels[1][2], 1);
    }

    #[test]
    fn no_op_tile_placement_still_commits_frame_palette_promotion() {
        let red = [200, 30, 40, 255];
        let mut doc = Document::new(1, 1);
        doc.palette = vec![CLEAR];
        doc.set_color_mode(ColorMode::Indexed);
        let mut frame_palette = vec![CLEAR; 8];
        frame_palette[7] = red;
        doc.frame_palettes = vec![frame_palette.clone()];
        let source = palette_source(vec![red], vec![7]);
        doc.tilesets.push(source.set.clone());
        doc.add_tile_layer(0).unwrap();
        doc.paint_tile(1, 0, 0, 0, 1).unwrap();
        let clip = Clipboard::new(crate::ClipboardData {
            w: 1,
            h: 1,
            pixels: vec![],
            cells: Some(vec![1]),
            indexed: None,
            selection: None,
            palette: Some(frame_palette),
            tileset: Some(source),
        });
        let site = TilePasteSite {
            mode: PasteMode::InPlace,
            layer: 1,
            frame: 0,
            bounds: [0, 0, 1, 1],
            origin: [0, 0],
            selected: false,
        };
        let (result, _) = build_tile_paste(&doc, &site, &clip, || false)
            .unwrap()
            .expect("Palette promotion is a real edit");
        assert_eq!(result.palette.len(), 8);
        assert_eq!(result.palette[7], red);
        assert_eq!(result.tilesets[0].tiles.len(), 1);
        assert_eq!(result.layers[1].tile_cels[0][0], 1);
        assert_eq!(doc.palette.len(), 1);
    }

    #[test]
    fn native_tile_matching_polls_independent_rasters_and_authored_slot_planes() {
        let a: TileRaster = vec![[10, 20, 30, 255]; 16384].into();
        let b: TileRaster = vec![[10, 20, 30, 255]; 16384].into();
        let polls = std::cell::Cell::new(0);
        assert!(
            tiles_match_until(&a, &b, || {
                polls.set(polls.get() + 1);
                polls.get() >= 3
            })
            .is_err()
        );
        assert!(tiles_match_until(&a, &b, || false).unwrap());
        let mut a = a;
        a.indices = Some(Arc::new(vec![1; 16384]));
        let mut b = a.clone();
        b.indices = Some(Arc::new(vec![1; 16384]));
        polls.set(0);
        assert!(
            tiles_match_until(&a, &b, || {
                polls.set(polls.get() + 1);
                polls.get() >= 3
            })
            .is_err()
        );
        Arc::make_mut(b.indices.as_mut().unwrap())[16383] = 2;
        assert!(!tiles_match_until(&a, &b, || false).unwrap());
        assert_eq!(a.indices.as_ref().unwrap()[16383], 1);
    }

    #[test]
    fn cancelled_native_tile_merge_rejects_without_touching_destination() {
        let mut doc = Document::new(2, 1);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Destination".into(),
            width: 1,
            height: 1,
            tiles: vec![vec![[10, 20, 30, 255]].into()],
        });
        let source = palette_source(vec![[90, 80, 70, 255]], vec![1]);
        let result = merge_tiles_until(&mut doc, 0, &source, BTreeSet::from([1]), || true);
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(doc.tilesets[0].tiles.len(), 1);
        assert_eq!(doc.tilesets[0].tiles[0][0], [10, 20, 30, 255]);
        assert!(doc.metadata.is_empty());
    }

    #[test]
    fn native_tile_hash_observes_cancellation_between_raster_chunks() {
        let tile: TileRaster = vec![[10, 20, 30, 255]; 16384].into();
        let polls = std::cell::Cell::new(0);
        let result = tile_key_until(&tile, &[], || {
            polls.set(polls.get() + 1);
            polls.get() >= 3
        });
        assert!(
            result.is_err(),
            "Hashing ignored cancellation across the raster"
        );
        assert!(result.err().unwrap().contains("cancelled"));
        assert_eq!(tile[16383], [10, 20, 30, 255]);
    }

    #[test]
    fn cancelled_paste_interrupts_clipboard_validation_before_a_late_invalid_cell() {
        let doc = Document::new(1, 1);
        let mut cells = vec![1; 16384];
        cells[16383] = 2;
        let clip = Clipboard::new(crate::ClipboardData {
            w: 16384,
            h: 1,
            pixels: vec![],
            cells: Some(cells),
            indexed: None,
            selection: None,
            palette: Some(vec![CLEAR, [10, 20, 30, 255]]),
            tileset: Some(palette_source(vec![[10, 20, 30, 255]], vec![1])),
        });
        let site = TilePasteSite {
            mode: PasteMode::Reference,
            layer: 0,
            frame: 0,
            bounds: [0, 0, 1, 1],
            origin: [0, 0],
            selected: false,
        };
        let polls = std::cell::Cell::new(0);
        let result = build_tile_paste(&doc, &site, &clip, || {
            polls.set(polls.get() + 1);
            polls.get() >= 4
        });
        assert!(result.is_err());
        assert!(
            result.err().unwrap().contains("cancelled"),
            "Validation did not observe cancellation while walking placement words"
        );
        assert!(polls.get() >= 4);
        assert_eq!(doc.composite(0), vec![CLEAR]);
    }

    #[test]
    fn native_tile_palette_preparation_preserves_exact_duplicate_slots_sharing_and_masks() {
        let red = [200, 20, 10, 255];
        let mut doc = Document::new(2, 1);
        doc.palette = vec![CLEAR, red, red];
        doc.set_color_mode(ColorMode::Indexed);
        let mut source = palette_source(vec![red, red], vec![2, 1]);
        source.set.tiles.push(source.set.tiles[0].clone());
        let converted = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(converted.set.tiles[0].indices, source.set.tiles[0].indices);
        assert!(Arc::ptr_eq(
            converted.set.tiles[0].indices.as_ref().unwrap(),
            source.set.tiles[0].indices.as_ref().unwrap()
        ));
        assert_eq!(
            converted.set.tiles[0].backing_id(),
            converted.set.tiles[1].backing_id()
        );
        assert!(Arc::ptr_eq(
            converted.set.tiles[0].indices.as_ref().unwrap(),
            converted.set.tiles[1].indices.as_ref().unwrap()
        ));
        doc.palette = vec![red, CLEAR, red];
        doc.transparent_index = 1;
        let source = palette_source(vec![CLEAR, red], vec![0, 2]);
        let converted = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(
            converted.set.tiles[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[1, 2]
        );
        assert_eq!(converted.set.tiles[0][0][3], 0);
    }

    #[test]
    fn native_tile_palette_preserves_transparent_nonmask_authored_slots() {
        let mut doc = Document::new(2, 1);
        doc.palette = vec![CLEAR, [255, 0, 0, 255], CLEAR];
        doc.set_color_mode(ColorMode::Indexed);
        let source = palette_source(vec![CLEAR, CLEAR], vec![2, 0]);
        let converted = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(
            converted.set.tiles[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[2, 0]
        );
    }

    #[test]
    fn native_tile_palette_preparation_handles_nearest_colors_and_nonindexed_modes() {
        let red = [255, 0, 0, 255];
        let source = palette_source(vec![red, CLEAR], vec![1, 0]);
        let mut doc = Document::new(2, 1);
        doc.palette = vec![CLEAR, [240, 10, 10, 255], [0, 0, 255, 255]];
        doc.set_color_mode(ColorMode::Indexed);
        let converted = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(converted.set.tiles[0][0], [240, 10, 10, 255]);
        assert_eq!(
            converted.set.tiles[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[1, 0]
        );
        doc.color_mode = ColorMode::Grayscale;
        let gray = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(gray.set.tiles[0][0], [54, 54, 54, 255]);
        assert!(gray.set.tiles[0].indices.is_none());
        doc.color_mode = ColorMode::Rgba;
        let rgb = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(rgb.set.tiles[0].pixels, source.set.tiles[0].pixels);
        assert!(rgb.set.tiles[0].indices.is_none());
    }

    #[test]
    fn native_tile_palette_promotion_is_atomic_on_rejected_source() {
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        let mut doc = Document::new(2, 1);
        doc.palette = vec![CLEAR, red];
        doc.set_color_mode(ColorMode::Indexed);
        doc.frame_palettes = vec![vec![CLEAR, red, blue]];
        let source = palette_source(vec![blue, red], vec![2, 1]);
        let before = doc.clone();
        let converted = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        assert_eq!(doc.palette, vec![CLEAR, red, blue]);
        assert_eq!(
            converted.set.tiles[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[2, 1]
        );
        doc = before;
        doc.transparent_index = 7;
        let before = doc.clone();
        let source = palette_source(vec![blue, CLEAR], vec![2, 0]);
        assert!(prepare_tile_colors(&mut doc, 0, &source).is_err());
        assert_eq!(doc, before);
        let mut invalid = source;
        invalid.set.tiles[0].indices = Some(Arc::new(vec![]));
        assert!(prepare_tile_colors(&mut doc, 0, &invalid).is_err());
        assert_eq!(doc, before);
    }

    #[test]
    #[ignore = "manual Windows large-tileset timing"]
    fn benchmark_large_tile_palette_preparation() {
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        let mut doc = Document::new(64, 64);
        doc.palette = vec![CLEAR, blue];
        doc.set_color_mode(ColorMode::Indexed);
        let mut source = palette_source(vec![red; 64 * 64], vec![1; 64 * 64]);
        source.set.width = 64;
        source.set.height = 64;
        let slots = source.set.tiles[0].indices.clone();
        source.set.tiles = (0..1024)
            .map(|_| {
                let mut tile: TileRaster = vec![red; 64 * 64].into();
                tile.indices = slots.clone();
                tile
            })
            .collect();
        let selected = BTreeSet::from([1]);
        let start = std::time::Instant::now();
        let small = prepare_selected_tile_colors(&mut doc, 0, &source, Some(&selected)).unwrap();
        let selected_elapsed = start.elapsed();
        let start = std::time::Instant::now();
        let full = prepare_tile_colors(&mut doc, 0, &source).unwrap();
        let full_elapsed = start.elapsed();
        assert_eq!(small.set.tiles[0][0], blue);
        assert_eq!(small.set.tiles[1][0], red);
        assert_eq!(full.set.tiles[1023][0], blue);
        assert_eq!(source.set.tiles[0][0], red);
        eprintln!(
            "4,194,304 unique tile pixels: one selected tile {:?}; full tileset {:?}",
            selected_elapsed, full_elapsed
        );
        let ctx = egui::Context::default();
        let mut source_doc = Document::new(64, 64);
        source_doc.palette = vec![CLEAR, red];
        source_doc.set_color_mode(ColorMode::Indexed);
        source_doc.frame_palettes = vec![vec![CLEAR, blue]];
        source_doc.tilesets.push(source.set.clone());
        source_doc.add_tile_layer(0).unwrap();
        source_doc.paint_tile(1, 0, 0, 0, 1).unwrap();
        let start = std::time::Instant::now();
        let (clip, _) = build_tile_clipboard(
            &source_doc,
            1,
            0,
            SelectedCells {
                start: [0, 0],
                size: [1, 1],
                selected: vec![true],
            },
            false,
            false,
            || false,
        )
        .unwrap();
        let source_snapshot = start.elapsed();
        assert_eq!(clip.tileset.as_ref().unwrap().set.tiles[0][0], blue);
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.replace(source_doc, None);
        editor.layer = 1;
        let start = std::time::Instant::now();
        editor.copy_tilemap(false).unwrap();
        let queue_time = start.elapsed();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while editor.tile_clipboard_work.is_some() && std::time::Instant::now() < deadline {
            editor.poll_clipboard(&ctx);
            std::thread::yield_now();
        }
        assert!(editor.tile_clipboard_work.is_none());
        assert_eq!(
            editor
                .clipboard
                .as_ref()
                .unwrap()
                .tileset
                .as_ref()
                .unwrap()
                .set
                .tiles[0][0],
            blue
        );
        eprintln!(
            "source snapshot {:?}; UI queue {:?}; completion including queue {:?}",
            source_snapshot,
            queue_time,
            start.elapsed()
        );
        editor.open_document(Document::new(64, 64), None);
        let start = std::time::Instant::now();
        editor.paste_as(PasteMode::InPlace);
        let paste_queue = start.elapsed();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while editor.tile_paste_work.is_some() && std::time::Instant::now() < deadline {
            editor.poll_clipboard(&ctx);
            std::thread::yield_now();
        }
        assert!(editor.tile_paste_work.is_none());
        assert_eq!(editor.doc.composite(0)[0], blue);
        assert_eq!(editor.undo.len(), 1);
        eprintln!(
            "large paste UI queue {:?}; completion including queue {:?}",
            paste_queue,
            start.elapsed()
        );
        let reference_color = [90, 100, 110, 128];
        editor.store_clipboard(
            Clipboard::raster(2048, 2048, vec![reference_color; 2048 * 2048]),
            "Reference benchmark",
        );
        let start = std::time::Instant::now();
        editor.paste_as(PasteMode::Reference);
        let reference_queue = start.elapsed();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while editor.tile_paste_work.is_some() && std::time::Instant::now() < deadline {
            editor.poll_clipboard(&ctx);
            std::thread::yield_now();
        }
        assert!(editor.tile_paste_work.is_none());
        assert_eq!(editor.doc.references[0].pixels[0], reference_color);
        assert_eq!(
            (
                editor.doc.references[0].width,
                editor.doc.references[0].height
            ),
            (2048, 2048)
        );
        eprintln!(
            "4.2M raster reference UI queue {:?}; completion including queue {:?}",
            reference_queue,
            start.elapsed()
        );
    }

    #[test]
    fn legacy_tile_clipboard_descriptors_keep_rgba_defaults() {
        let old = serde_json::json!({"set":{"name":"legacy","width":1,"height":1,"tiles":[[[1,2,3,255]]]},
            "flags":6,"base_index":1});
        let source: ClipboardTileset = serde_json::from_value(old).unwrap();
        assert_eq!(source.color_mode, ColorMode::Rgba);
        assert_eq!(source.transparent_index, 0);
        let mut indexed = source;
        indexed.color_mode = ColorMode::Indexed;
        indexed.transparent_index = 7;
        let restored: ClipboardTileset =
            serde_json::from_slice(&serde_json::to_vec(&indexed).unwrap()).unwrap();
        assert_eq!(restored.color_mode, ColorMode::Indexed);
        assert_eq!(restored.transparent_index, 7);
    }

    #[test]
    fn clipboard_merge_distinguishes_authored_slots_and_preserves_inserted_metadata() {
        let red = [200, 100, 20, 255];
        let mut doc = Document::new(2, 1);
        doc.palette = vec![CLEAR, red, red];
        doc.set_color_mode(ColorMode::Indexed);
        let mut tile: TileRaster = vec![red, red].into();
        tile.indices = Some(Arc::new(vec![1, 1]));
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "destination".into(),
            width: 2,
            height: 1,
            tiles: vec![tile.clone()],
        });
        tile.indices = Some(Arc::new(vec![2, 1]));
        let source_key = tile.editor_id.runtime_key();
        let source = ClipboardTileset {
            color_mode: ColorMode::Indexed,
            transparent_index: 0,
            set: Tileset {
                editor_id: Default::default(),
                name: "source".into(),
                width: 2,
                height: 1,
                tiles: vec![tile],
            },
            flags: 6,
            base_index: 1,
            user_data: vec![],
            external: vec![],
        };
        let map = merge_tiles(&mut doc, 0, &source, BTreeSet::from([1])).unwrap();
        assert_eq!(
            map[&1], 2,
            "equal colors with different authored slots must not coalesce"
        );
        assert_eq!(
            doc.tilesets[0].tiles[1]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[2, 1]
        );
        assert_ne!(doc.tilesets[0].tiles[1].editor_id.runtime_key(), source_key);
        assert_eq!(
            merge_tiles(&mut doc, 0, &source, BTreeSet::from([1])).unwrap()[&1],
            2
        );
        assert_eq!(doc.tilesets[0].tiles.len(), 2);
        let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
        assert_eq!(
            restored.tilesets[0].tiles[1].indices,
            source.set.tiles[0].indices
        );
    }
}
