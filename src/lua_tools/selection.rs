use super::{MAX_STAMP_WORK, Point, Stroke, Tool, check_work, coverage};
use crate::productivity::{SelectionOp, combine_selection, matching_on_axes};
use crate::workflows::inside_polygon;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Rectangle,
    Ellipse,
    Lasso,
    Wand,
}

pub(super) fn apply(stroke: &Stroke, kind: Kind) -> Result<Value, String> {
    let doc = &stroke.document;
    let area = doc.width * doc.height;
    let mode = match stroke.selection_mode.unwrap_or(0) {
        0 => SelectionOp::Replace,
        1 => SelectionOp::Add,
        2 => SelectionOp::Subtract,
        3 => SelectionOp::Intersect,
        _ => return Err("Invalid selection mode".into()),
    };
    let hits = match kind {
        Kind::Rectangle | Kind::Ellipse => {
            let tool = match kind {
                Kind::Rectangle => Tool::Rectangle(true),
                _ => Tool::Ellipse(true),
            };
            check_work(tool, &stroke.points, 1, stroke.freehand_algorithm)?;
            coverage(stroke, tool, 1)?
        }
        Kind::Lasso => lasso(stroke)?,
        Kind::Wand => matching_on_axes(
            &doc.layer_pixels(stroke.layer, stroke.frame),
            doc.width,
            doc.height,
            stroke.points[0],
            stroke.tolerance,
            stroke.contiguous,
            doc.wraparound.into(),
        ),
    };
    let mut before = vec![false; area];
    for &(x, y) in stroke.selection.iter().flatten() {
        if let Some(i) = doc.index(x, y) {
            before[i] = true;
        }
    }
    let mut after = before.clone();
    combine_selection(&mut after, &hits, mode);
    if after == before {
        return Ok(json!({}));
    }
    let points: Vec<Point> = after
        .into_iter()
        .enumerate()
        .filter(|(_, hit)| *hit)
        .map(|(i, _)| ((i % doc.width) as i32, (i / doc.width) as i32))
        .collect();
    Ok(json!({"selection":points}))
}

fn lasso(stroke: &Stroke) -> Result<Vec<bool>, String> {
    if stroke.points.len() < 3 {
        return Err("Lasso needs at least three points".into());
    }
    let doc = &stroke.document;
    let bounds = stroke.points.iter().copied().fold(
        [i32::MAX, i32::MAX, i32::MIN, i32::MIN],
        |[left, top, right, bottom], (x, y)| [left.min(x), top.min(y), right.max(x), bottom.max(y)],
    );
    let [left, top, right, bottom] = [
        bounds[0].clamp(0, doc.width as i32) as usize,
        bounds[1].clamp(0, doc.height as i32) as usize,
        bounds[2].clamp(0, doc.width as i32) as usize,
        bounds[3].clamp(0, doc.height as i32) as usize,
    ];
    let area = (right - left) * (bottom - top);
    if (area as u64).saturating_mul(stroke.points.len() as u64) > MAX_STAMP_WORK {
        return Err("Lasso exceeds the selection work limit".into());
    }
    let mut hits = vec![false; doc.width * doc.height];
    for y in top..bottom {
        for x in left..right {
            hits[y * doc.width + x] = inside_polygon(&stroke.points, x as i32, y as i32);
        }
    }
    Ok(hits)
}
