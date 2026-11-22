//! Selection/clipboard host adapter; all raster edits use shared core operations.
use super::*;
use pixelcraft_core::{
    model::{CLEAR, ColorMode, Pixel},
    workflows::{self, Transform},
};
pub(super) struct RasterClipboard {
    width: usize,
    height: usize,
    pixels: Vec<Pixel>,
    palette: Vec<Pixel>,
    indices: Option<Vec<u8>>,
}
fn bounds(s: &MobileSession) -> Result<[i32; 4], String> {
    let (a, b, c, d) = s.editor.selection.unwrap_or((
        0,
        0,
        s.editor.doc.width as i32 - 1,
        s.editor.doc.height as i32 - 1,
    ));
    let (x, y, r, bottom) = (
        a.min(c).max(0),
        b.min(d).max(0),
        a.max(c).min(s.editor.doc.width as i32 - 1),
        b.max(d).min(s.editor.doc.height as i32 - 1),
    );
    if x > r || y > bottom {
        return Err("Selection is outside the canvas".into());
    }
    Ok([x, y, r - x + 1, bottom - y + 1])
}
pub(super) fn apply(s: &mut MobileSession, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 4096 {
        return Err("Selection request is too large".into());
    }
    let p: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid selection request")?;
    if p["revision"].as_u64() != Some(u64::from(s.revision)) {
        return Err("Artwork changed; retry selection action".into());
    }
    let revision = s.next_revision()?;
    let action = p["action"].as_str().ok_or("Missing selection action")?;
    if matches!(action, "select" | "select-all" | "deselect") {
        let rect = if action == "deselect" {
            None
        } else if action == "select-all" {
            Some((
                0,
                0,
                s.editor.doc.width as i32 - 1,
                s.editor.doc.height as i32 - 1,
            ))
        } else {
            let [x, y, r, b]: [i32; 4] = serde_json::from_value(p["bounds"].clone())
                .map_err(|_| "Invalid selection bounds")?;
            if [x, r]
                .iter()
                .any(|v| *v < 0 || *v as usize >= s.editor.doc.width)
                || [y, b]
                    .iter()
                    .any(|v| *v < 0 || *v as usize >= s.editor.doc.height)
            {
                return Err("Selection outside canvas".into());
            }
            Some((x.min(r), y.min(b), x.max(r), y.max(b)))
        };
        s.editor.selection = rect;
        s.editor.mask = None;
        s.revision = revision;
        return Ok(());
    }
    let [x, y, w, h] = bounds(s)?;
    let (layer, frame) = (s.editor.layer, s.editor.frame);
    if s.editor.doc.layers[layer].group || s.editor.doc.layers[layer].tileset.is_some() {
        return Err("Choose an image layer".into());
    }
    if action != "copy" && !s.editor.doc.editable(layer) {
        return Err("Layer is locked".into());
    }
    let copy = || {
        let doc = &s.editor.doc;
        let positions = (y..y + h)
            .flat_map(|row| (x..x + w).map(move |col| row as usize * doc.width + col as usize))
            .collect::<Vec<_>>();
        let indexed = doc.color_mode == ColorMode::Indexed;
        RasterClipboard {
            width: w as usize,
            height: h as usize,
            pixels: positions
                .iter()
                .map(|i| doc.layers[layer].cels[frame][*i])
                .collect(),
            palette: doc.palette_for(frame).to_vec(),
            indices: if indexed {
                doc.layers[layer].cel_meta[frame]
                    .indices
                    .as_ref()
                    .map(|v| positions.iter().map(|i| v[*i]).collect())
            } else {
                None
            },
        }
    };
    if action == "copy" {
        s.clipboard = Some(copy());
        s.revision = revision;
        return Ok(());
    }
    let mut next = s.editor.doc.clone();
    let mut changed_bounds = None;
    let mut clipboard = None;
    match action {
        "cut" | "delete" => {
            if action == "cut" {
                clipboard = Some(copy());
            }
            for row in y..y + h {
                for col in x..x + w {
                    next.set_pixel(
                        layer,
                        frame,
                        row as usize * next.width + col as usize,
                        CLEAR,
                    );
                }
            }
        }
        "paste" => {
            let clip = s.clipboard.as_ref().ok_or("Copy a selection first")?;
            let px = p["x"].as_i64().unwrap_or(i64::from(x));
            let py = p["y"].as_i64().unwrap_or(i64::from(y));
            if px < 0 || py < 0 || px as usize >= next.width || py as usize >= next.height {
                return Err("Paste target is outside canvas".into());
            }
            for row in 0..clip.height {
                for col in 0..clip.width {
                    let (dx, dy) = (px as usize + col, py as usize + row);
                    if dx >= next.width || dy >= next.height {
                        continue;
                    }
                    let source = row * clip.width + col;
                    if next.color_mode == ColorMode::Indexed
                        && next.palette_for(frame) == clip.palette
                        && let Some(indices) = &clip.indices
                    {
                        next.drawing_index = Some(usize::from(indices[source]));
                    }
                    next.set_pixel(layer, frame, dy * next.width + dx, clip.pixels[source]);
                    next.drawing_index = None;
                }
            }
            changed_bounds = Some([px as i32, py as i32, clip.width as i32, clip.height as i32]);
        }
        "crop" => {
            workflows::crop(&mut next, [x, y, w, h])?;
        }
        "flip-x" | "flip-y" | "rotate" | "move" => {
            let transform = match action {
                "flip-x" => Transform::FlipX,
                "flip-y" => Transform::FlipY,
                "rotate" => Transform::Rotate,
                _ => {
                    let dx = p["dx"].as_i64().ok_or("Missing movement")?;
                    let dy = p["dy"].as_i64().ok_or("Missing movement")?;
                    if dx.unsigned_abs() > MAX_SIDE as u64 || dy.unsigned_abs() > MAX_SIDE as u64 {
                        return Err("Movement exceeds mobile limits".into());
                    }
                    Transform::Move(dx as i32, dy as i32)
                }
            };
            changed_bounds = Some(workflows::transform(
                &mut next,
                layer,
                frame,
                [x, y, w, h],
                None,
                transform,
            )?);
        }
        _ => return Err("Unknown selection action".into()),
    }
    s.commit(next, "Selection")?;
    if let Some(copy) = clipboard {
        s.clipboard = Some(copy);
    }
    if action == "crop" {
        s.editor.selection = None;
        s.editor.mask = None;
    }
    if let Some([x, y, w, h]) = changed_bounds {
        s.editor.selection = Some((x, y, x + w - 1, y + h - 1));
        s.editor.mask = None;
    }
    Ok(())
}
pub(super) fn brush_mask(editor: &EditorSession) -> Option<Vec<bool>> {
    if let Some(mask) = &editor.mask {
        return Some(mask.to_vec());
    }
    let (a, b, c, d) = editor.selection?;
    Some(
        (0..editor.doc.height)
            .flat_map(|y| {
                (0..editor.doc.width).map(move |x| {
                    x as i32 >= a.min(c)
                        && x as i32 <= a.max(c)
                        && y as i32 >= b.min(d)
                        && y as i32 <= b.max(d)
                })
            })
            .collect(),
    )
}

pub(super) fn clip_paint(editor: &EditorSession, next: &mut Document) {
    let Some(mask) = brush_mask(editor) else {
        return;
    };
    let (l, f) = (editor.layer, editor.frame);
    let old = &editor.doc;
    let drawing_index = next.drawing_index;
    for (i, selected) in mask.into_iter().enumerate() {
        if selected {
            continue;
        }
        let color = old.layers[l].cels[f][i];
        let index = old.layers[l].cel_meta[f].indices.as_ref().map(|v| v[i]);
        if next.layers[l].cels[f][i] != color
            || next.layers[l].cel_meta[f]
                .indices
                .as_ref()
                .and_then(|v| v.get(i).copied())
                != index
        {
            next.drawing_index = index.map(usize::from);
            next.set_pixel(l, f, i, color);
        }
    }
    next.drawing_index = drawing_index;
}
