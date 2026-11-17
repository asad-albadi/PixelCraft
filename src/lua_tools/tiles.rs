use super::{Point, Stroke, Tool, coverage, stroke_color};
use crate::lua::MAX_SCRIPT_PIXELS;
use crate::model::{CLEAR, Document, TILE_ID_MASK, TilePixelEdit, line_pixels};
use crate::productivity::matching_by_on_axes;
use serde_json::{Value, json};
use std::{collections::HashSet, convert::Infallible};

pub(super) fn use_tilemap_tool(
    stroke: &mut Stroke,
    tool: Tool,
    size: i32,
    tileset: usize,
) -> Result<Value, String> {
    match stroke.tilemap_mode.unwrap_or(1) {
        0 => paint_pixels(stroke, tool, size, tileset),
        1 => paint_placements(stroke, tool),
        _ => Err("Invalid tilemap mode".into()),
    }
}

fn map_point(doc: &Document, point: Point) -> Point {
    doc.position_on_axes(point.0, point.1, doc.wraparound.into())
        .map_or(point, |index| {
            ((index % doc.width) as i32, (index / doc.width) as i32)
        })
}

fn check_editable(stroke: &Stroke) -> Result<(), String> {
    if !stroke.document.editable(stroke.layer) {
        return Err("Choose an editable tilemap layer".into());
    }
    Ok(())
}

fn paint_placements(stroke: &mut Stroke, tool: Tool) -> Result<Value, String> {
    let doc = &stroke.document;
    let layer = stroke.layer;
    let frame = stroke.frame;
    let layer_data = &doc.layers[layer];
    let cells = &layer_data.tile_cels[frame];
    if cells.len() > MAX_SCRIPT_PIXELS {
        return Err("Tilemap cel exceeds the Lua cell budget".into());
    }
    let a = stroke.points[0];
    let cell_at = |point| doc.tile_at_point(layer, frame, map_point(doc, point));
    if matches!(tool, Tool::Picker) {
        let (geometry, x, y) = cell_at(a).ok_or("Eyedropper point is outside the tilemap")?;
        return Ok(json!({"tile":cells[y * geometry.columns + x]}));
    }
    check_editable(stroke)?;
    let mut hits = vec![false; cells.len()];
    match tool {
        Tool::Fill => {
            let Some((geometry, x, y)) = cell_at(a) else {
                return Ok(json!({}));
            };
            hits = matching_by_on_axes(
                cells,
                geometry.columns,
                geometry.rows,
                (x as i32, y as i32),
                stroke.contiguous,
                doc.wraparound.into(),
                |a, b| a == b,
            );
        }
        Tool::Pencil | Tool::Line | Tool::Eraser => {
            let mut mark = |point| -> Result<(), Infallible> {
                if let Some((geometry, x, y)) = cell_at(point) {
                    hits[y * geometry.columns + x] = true;
                }
                Ok(())
            };
            _ = mark(a);
            if matches!(tool, Tool::Line) {
                _ = line_pixels(a, stroke.points[stroke.points.len() - 1], mark);
            } else {
                for segment in stroke.points.windows(2) {
                    _ = line_pixels(segment[0], segment[1], &mut mark);
                }
            }
        }
        _ => return Err("This tool does not place tiles".into()),
    }
    if let Some(selection) = stroke.selection.as_ref() {
        let mut selected = vec![false; cells.len()];
        for &point in selection {
            if let Some((geometry, x, y)) = doc.tile_at_point(layer, frame, point) {
                selected[y * geometry.columns + x] = true;
            }
        }
        for (hit, selected) in hits.iter_mut().zip(selected) {
            *hit &= selected;
        }
    }
    let mut tile = if matches!(tool, Tool::Eraser) {
        0
    } else {
        stroke.tile.unwrap_or(1)
    };
    if tile & TILE_ID_MASK == 0 {
        tile = 0;
    }
    if !hits
        .iter()
        .zip(cells)
        .any(|(hit, existing)| *hit && *existing != tile)
    {
        return Ok(json!({}));
    }
    stroke
        .document
        .paint_matching_tiles(layer, frame, tile, &hits)?;
    Ok(json!({"layer":stroke.document.layers[layer]}))
}

fn paint_pixels(
    stroke: &mut Stroke,
    tool: Tool,
    size: i32,
    tileset: usize,
) -> Result<Value, String> {
    let doc = &stroke.document;
    let layer = stroke.layer;
    let frame = stroke.frame;
    if matches!(tool, Tool::Picker) {
        let point = stroke.points[0];
        let index = doc
            .drawing_position(point.0, point.1)
            .ok_or("Eyedropper point is outside the canvas")?;
        return Ok(json!({"color":doc.composite(frame)[index]}));
    }
    check_editable(stroke)?;
    if matches!(tool, Tool::Fill) {
        return Err("Switch to tile placement mode to fill tile cells".into());
    }
    if stroke.opacity == 0 {
        return Ok(json!({}));
    }
    let mode = match stroke.tileset_mode.unwrap_or(0) {
        0 => TilePixelEdit::Shared,
        1 => TilePixelEdit::ReuseVariant,
        2 => TilePixelEdit::CreateVariant,
        _ => return Err("Invalid tileset mode".into()),
    };
    let selected: Option<HashSet<_>> = stroke
        .selection
        .as_ref()
        .map(|points| points.iter().copied().collect());
    let hits = coverage(stroke, tool, size);
    let width = doc.width;
    let cells = &doc.layers[layer].tile_cels[frame];
    if cells.len() > MAX_SCRIPT_PIXELS {
        return Err("Tilemap cel exceeds the Lua cell budget".into());
    }
    let old_cells = cells.clone();
    let old_tiles = doc.tilesets[tileset].tiles.clone();
    let (tw, th) = (doc.tilesets[tileset].width, doc.tilesets[tileset].height);
    let max_tiles = MAX_SCRIPT_PIXELS / (tw * th);
    if old_tiles.len() > max_tiles {
        return Err("Tileset exceeds the Lua pixel budget".into());
    }
    let mut applied = HashSet::new();
    let doc = &mut stroke.document;
    let source = doc.snapped_color(frame, stroke.color);
    for (index, hit) in hits.into_iter().enumerate() {
        if !hit {
            continue;
        }
        let p = ((index % width) as i32, (index / width) as i32);
        if selected
            .as_ref()
            .is_some_and(|selected| !selected.contains(&p))
        {
            continue;
        }
        let Some((geometry, x, y)) = doc.tile_at_point(layer, frame, p) else {
            continue;
        };
        let pixel = (
            (i64::from(p.0) - i64::from(geometry.x)) as usize % tw,
            (i64::from(p.1) - i64::from(geometry.y)) as usize % th,
        );
        let placed = doc.layers[layer].tile_cels[frame][y * geometry.columns + x];
        let id = (placed & TILE_ID_MASK) as usize;
        let set = &doc.tilesets[tileset];
        let Some(offset) = set.source_offset(placed, pixel.0, pixel.1) else {
            continue;
        };
        let effective_mode = if mode == TilePixelEdit::CreateVariant && id > old_tiles.len() {
            TilePixelEdit::Shared
        } else {
            mode
        };
        if effective_mode == TilePixelEdit::Shared && id != 0 && !applied.insert((id, offset)) {
            continue;
        }
        let previous = id.checked_sub(1).map_or(CLEAR, |i| set.tiles[i][offset]);
        let color = stroke_color(tool, &stroke.ink, stroke.opacity, previous, source);
        if color != previous {
            doc.paint_tile_pixel(layer, frame, (x, y), pixel, color, effective_mode)?;
            if doc.tilesets[tileset].tiles.len() > max_tiles {
                return Err("Tile variants exceed the Lua pixel budget".into());
            }
        }
    }
    let updates: Vec<_> = doc.tilesets[tileset]
        .tiles
        .iter()
        .enumerate()
        .filter(|(index, pixels)| old_tiles.get(*index) != Some(*pixels))
        .map(|(index, pixels)| json!({"index":index+1,"pixels":pixels}))
        .collect();
    Ok(
        if updates.is_empty() && doc.layers[layer].tile_cels[frame] == old_cells {
            json!({})
        } else {
            json!({"layer":doc.layers[layer],"tileset":tileset+1,"tile_updates":updates})
        },
    )
}
