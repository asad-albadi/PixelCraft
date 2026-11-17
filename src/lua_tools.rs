mod freehand;
mod selection;
mod tiles;

use crate::lua::MAX_SCRIPT_PIXELS;
use crate::model::{BrushShape, CLEAR, Document, MAX_BRUSH_SIZE, MAX_DIMENSION, Pixel, over};
use crate::productivity::matching_on_axes;
use serde::Deserialize;
use serde_json::{Value, json};

type Point = (i32, i32);
const MAX_STAMP_WORK: u64 = 16_000_000;

#[derive(Clone, Copy)]
enum Tool {
    Pencil,
    Eraser,
    Line,
    Rectangle(bool),
    Ellipse(bool),
    Polygon(bool),
    Curve,
    Fill,
    Picker,
    Selection(selection::Kind),
}

impl Tool {
    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "pencil" => Ok(Self::Pencil),
            "eraser" => Ok(Self::Eraser),
            "line" => Ok(Self::Line),
            "rectangle" => Ok(Self::Rectangle(false)),
            "filled_rectangle" => Ok(Self::Rectangle(true)),
            "ellipse" => Ok(Self::Ellipse(false)),
            "filled_ellipse" => Ok(Self::Ellipse(true)),
            "polygon" => Ok(Self::Polygon(false)),
            "filled_polygon" | "filled_contour" => Ok(Self::Polygon(true)),
            "curve" => Ok(Self::Curve),
            "paint_bucket" | "fill" => Ok(Self::Fill),
            "eyedropper" => Ok(Self::Picker),
            "rectangular_marquee" => Ok(Self::Selection(selection::Kind::Rectangle)),
            "elliptical_marquee" => Ok(Self::Selection(selection::Kind::Ellipse)),
            "lasso" | "polygonal_lasso" => Ok(Self::Selection(selection::Kind::Lasso)),
            "magic_wand" => Ok(Self::Selection(selection::Kind::Wand)),
            _ => Err(format!("Unsupported raster tool: {name}")),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Ink {
    Simple,
    AlphaCompositing,
    CopyColor,
}

#[derive(Deserialize)]
struct Stroke {
    document: Document,
    tool: String,
    layer: usize,
    frame: usize,
    points: Vec<Point>,
    #[serde(default)]
    freehand_algorithm: freehand::Algorithm,
    selection: Option<Vec<Point>>,
    selection_mode: Option<u8>,
    brush_shape: Option<BrushShape>,
    size: i32,
    color: Pixel,
    ink: Ink,
    opacity: u8,
    tolerance: u8,
    contiguous: bool,
    tilemap_mode: Option<u8>,
    tileset_mode: Option<u8>,
    tile: Option<u32>,
}

fn distance(a: Point, b: Point) -> u64 {
    u64::from(a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))) + 1
}

fn check_work(
    tool: Tool,
    points: &[Point],
    size: i32,
    algorithm: freehand::Algorithm,
) -> Result<(), String> {
    if points.is_empty()
        || points.len() > 4096
        || points
            .iter()
            .any(|&(x, y)| x.unsigned_abs().max(y.unsigned_abs()) as usize > 2 * MAX_DIMENSION)
    {
        return Err("Invalid tool points or coordinates exceed the stroke limit".into());
    }
    let a = points[0];
    let b = points[points.len() - 1];
    let width = u64::from(a.0.abs_diff(b.0)) + 1;
    let height = u64::from(a.1.abs_diff(b.1)) + 1;
    let stamps = match tool {
        Tool::Rectangle(true) | Tool::Ellipse(true) => width * height,
        Tool::Rectangle(false) | Tool::Ellipse(false) => 4 * (width + height),
        Tool::Line => distance(a, b),
        Tool::Pencil | Tool::Eraser if algorithm == freehand::Algorithm::Dots => {
            points.len() as u64
        }
        Tool::Pencil | Tool::Eraser => points
            .windows(2)
            .map(|segment| distance(segment[0], segment[1]))
            .sum::<u64>()
            .max(1),
        Tool::Fill | Tool::Picker | Tool::Selection(_) | Tool::Polygon(_) | Tool::Curve => 1,
    };
    let brush = match tool {
        Tool::Rectangle(true) | Tool::Ellipse(true) => 1,
        _ => size as u64,
    };
    if stamps.saturating_mul(brush * brush) > MAX_STAMP_WORK {
        return Err("Tool stroke exceeds the raster work limit".into());
    }
    Ok(())
}

fn coverage(stroke: &Stroke, tool: Tool, size: i32) -> Result<Vec<bool>, String> {
    let doc = &stroke.document;
    let a = stroke.points[0];
    let b = stroke.points[stroke.points.len() - 1];
    if matches!(tool, Tool::Fill) {
        return Ok(matching_on_axes(
            &doc.layer_pixels(stroke.layer, stroke.frame),
            doc.width,
            doc.height,
            a,
            stroke.tolerance,
            stroke.contiguous,
            doc.wraparound.into(),
        ));
    }
    if let Tool::Polygon(filled) = tool {
        return crate::workflows::polygon_coverage(
            (doc.width, doc.height),
            &stroke.points,
            filled,
            (size, stroke.brush_shape.unwrap_or(doc.brush_shape)),
            doc.wraparound.into(),
        );
    }
    if matches!(tool, Tool::Curve) {
        let controls = stroke
            .points
            .as_slice()
            .try_into()
            .map_err(|_| "Curve needs a start, two controls, and an end")?;
        let pixels = crate::workflows::curve::cubic_coverage(
            (doc.width, doc.height),
            controls,
            (size, stroke.brush_shape.unwrap_or(doc.brush_shape)),
            doc.wraparound.into(),
        )?;
        let mut hits = vec![false; doc.width * doc.height];
        pixels.visit_nonclear(|i, _| hits[i] = true);
        return Ok(hits);
    }

    // Use the same native raster routines as the editor, then apply ink once to
    // each covered pixel. Overlapping brush stamps must not accumulate opacity.
    let mut mask = Document::new(doc.width, doc.height);
    mask.wraparound = doc.wraparound;
    mask.brush_shape = stroke.brush_shape.unwrap_or(doc.brush_shape);
    let white = [255; 4];
    match tool {
        Tool::Pencil | Tool::Eraser => {
            freehand::visit(&stroke.points, stroke.freehand_algorithm, Some, |(x, y)| {
                mask.stamp(0, 0, x, y, white, size);
            });
        }
        Tool::Line => mask.line(0, 0, a, b, white, size),
        Tool::Rectangle(filled) => {
            let (left, right) = (a.0.min(b.0), a.0.max(b.0));
            let (top, bottom) = (a.1.min(b.1), a.1.max(b.1));
            if filled {
                for y in top..=bottom {
                    mask.line(0, 0, (left, y), (right, y), white, 1);
                }
            } else {
                let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
                for i in 0..4 {
                    mask.line(0, 0, corners[i], corners[(i + 1) % 4], white, size);
                }
            }
        }
        Tool::Ellipse(filled) => mask.ellipse(0, 0, a, b, white, filled, size),
        Tool::Fill | Tool::Picker | Tool::Selection(_) | Tool::Polygon(_) | Tool::Curve => {
            unreachable!("Handled outside brush coverage")
        }
    }
    Ok(mask.layers[0].cels[0]
        .iter()
        .map(|pixel| *pixel != CLEAR)
        .collect())
}

fn stroke_color(tool: Tool, ink: &Ink, opacity: u8, previous: Pixel, source: Pixel) -> Pixel {
    match (tool, ink) {
        (Tool::Eraser, _) => {
            let mut color = previous;
            color[3] = ((u16::from(color[3]) * u16::from(255 - opacity) + 127) / 255) as u8;
            if color[3] == 0 { CLEAR } else { color }
        }
        (_, Ink::CopyColor) => {
            let mut color = source;
            color[3] = ((u16::from(color[3]) * u16::from(opacity) + 127) / 255) as u8;
            color
        }
        (_, Ink::Simple) if source[3] == 0 => CLEAR,
        _ => over(previous, source, opacity),
    }
}

pub(crate) fn use_tool(value: Value) -> Result<Value, String> {
    let mut stroke: Stroke = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let tool = Tool::parse(&stroke.tool)?;
    let size = stroke.size.clamp(1, MAX_BRUSH_SIZE);
    check_work(tool, &stroke.points, size, stroke.freehand_algorithm)?;
    let doc = &stroke.document;
    if doc.logical_pixels() > MAX_SCRIPT_PIXELS {
        return Err("Tool document exceeds the Lua pixel budget".into());
    }
    doc.validate()?;
    let layer = doc.layers.get(stroke.layer).ok_or("Invalid tool layer")?;
    if stroke.frame >= doc.durations.len() {
        return Err("Invalid tool frame".into());
    }
    if let Tool::Selection(kind) = tool {
        return selection::apply(&stroke, kind);
    }
    if stroke.tilemap_mode.is_some_and(|mode| mode > 1) {
        return Err("Invalid tilemap mode".into());
    }
    if let Some(tileset) = layer.tileset {
        return tiles::use_tilemap_tool(&mut stroke, tool, size, tileset);
    }
    if stroke.tilemap_mode == Some(1) || stroke.tileset_mode.is_some() {
        return Err("Choose a tilemap layer".into());
    }
    if matches!(tool, Tool::Picker) {
        let (x, y) = stroke.points[0];
        let index = doc
            .drawing_position(x, y)
            .ok_or("Eyedropper point is outside the canvas")?;
        return Ok(json!({"color":doc.composite(stroke.frame)[index]}));
    }
    if !doc.editable(stroke.layer) {
        return Err("Choose an editable raster layer".into());
    }
    if stroke.opacity == 0 {
        return Ok(json!({}));
    }
    let selected = stroke.selection.as_ref().map(|points| {
        let mut mask = vec![false; doc.width * doc.height];
        for &(x, y) in points {
            if let Some(i) = doc.index(x, y) {
                mask[i] = true;
            }
        }
        mask
    });
    let hits = coverage(&stroke, tool, size)?;
    let doc = &mut stroke.document;
    doc.drawing_index = None;
    let source = doc.snapped_color(stroke.frame, stroke.color);
    let mut changed = false;
    for (i, hit) in hits.into_iter().enumerate() {
        if !hit || selected.as_ref().is_some_and(|mask| !mask[i]) {
            continue;
        }
        let previous = doc.layers[stroke.layer].cels[stroke.frame][i];
        let color = stroke_color(tool, &stroke.ink, stroke.opacity, previous, source);
        if color != previous {
            doc.set_pixel(stroke.layer, stroke.frame, i, color);
            changed = true;
        }
    }
    Ok(if changed {
        json!({"layer":doc.layers[stroke.layer]})
    } else {
        json!({})
    })
}
