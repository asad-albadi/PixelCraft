use crate::model::{Document, MAX_DIMENSION, MAX_PIXELS};
use image::{Rgba, RgbaImage};
use serde::Serialize;

pub enum TilesetAtlasScope<'a> {
    Visible,
    Selected(&'a [usize]),
}

#[derive(Debug, Serialize)]
pub struct TilesetAtlasLayout {
    pub width: usize,
    pub height: usize,
    pub columns: usize,
    pub rows: usize,
    pub cell_size: [usize; 2],
    pub tilesets: Vec<TilesetAtlasEntry>,
}

#[derive(Debug, Serialize)]
pub struct TilesetAtlasEntry {
    pub index: usize,
    pub name: String,
    pub first_cell: usize,
    pub tile_count: usize,
    pub tile_size: [usize; 2],
}

/// Renders authored tile entries in tileset and tile ID order, including
/// unused entries. Multiple layers using one tileset share its atlas cells.
pub fn tileset_atlas(
    doc: &Document,
    scope: TilesetAtlasScope<'_>,
    columns: Option<usize>,
) -> Result<(RgbaImage, TilesetAtlasLayout), String> {
    doc.validate()?;
    let selected = if let TilesetAtlasScope::Selected(layers) = scope {
        if layers.is_empty() || layers.len() > doc.layers.len() {
            return Err("Choose at least one valid layer".into());
        }
        let mut selected = vec![false; doc.layers.len()];
        for &layer in layers {
            *selected
                .get_mut(layer)
                .ok_or("Choose valid layer indices")? = true;
        }
        Some(selected)
    } else {
        None
    };

    let mut included = vec![false; doc.tilesets.len()];
    for (index, layer) in doc.layers.iter().enumerate() {
        if let Some(tileset) = layer.tileset {
            let mut ancestors = std::iter::successors(Some(index), |&i| doc.layers[i].parent);
            included[tileset] |= if let Some(selected) = &selected {
                ancestors.any(|i| selected[i])
            } else {
                ancestors.all(|i| doc.layers[i].visible)
            };
        }
    }

    let mut tilesets = Vec::new();
    let mut total = 0usize;
    let mut cell = [0usize; 2];
    for (index, set) in doc.tilesets.iter().enumerate() {
        if !included[index] || set.tiles.is_empty() {
            continue;
        }
        tilesets.push(TilesetAtlasEntry {
            index,
            name: set.name.clone(),
            first_cell: total,
            tile_count: set.tiles.len(),
            tile_size: [set.width, set.height],
        });
        total = total
            .checked_add(set.tiles.len())
            .ok_or("Too many atlas tiles")?;
        cell = [cell[0].max(set.width), cell[1].max(set.height)];
    }
    if total == 0 {
        return Err("No authored tiles in the chosen layers".into());
    }

    let fits = |columns: usize| {
        if columns == 0 || columns > MAX_DIMENSION / cell[0] {
            return false;
        }
        let width = columns * cell[0];
        let height = total.div_ceil(columns).saturating_mul(cell[1]);
        height <= MAX_DIMENSION && width.saturating_mul(height) <= MAX_PIXELS
    };
    let columns = if let Some(columns) = columns {
        fits(columns).then_some(columns)
    } else {
        (1..=total.min(MAX_DIMENSION / cell[0]))
            .filter(|&columns| fits(columns))
            .min_by_key(|&columns| columns.abs_diff(16))
    }
    .ok_or("Tileset atlas exceeds the image limits for these columns")?;

    let rows = total.div_ceil(columns);
    let width = columns * cell[0];
    let height = rows * cell[1];
    let mut image = RgbaImage::new(width as u32, height as u32);
    for entry in &tilesets {
        let set = &doc.tilesets[entry.index];
        for (tile_index, tile) in set.tiles.iter().enumerate() {
            let slot = entry.first_cell + tile_index;
            let x = (slot % columns) * cell[0];
            let y = (slot / columns) * cell[1];
            tile.visit_nonclear(|offset, pixel| {
                image.put_pixel(
                    (x + offset % set.width) as u32,
                    (y + offset / set.width) as u32,
                    Rgba(pixel),
                );
            });
        }
    }
    Ok((
        image,
        TilesetAtlasLayout {
            width,
            height,
            columns,
            rows,
            cell_size: cell,
            tilesets,
        },
    ))
}
