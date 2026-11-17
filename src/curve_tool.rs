use super::*;
use pixelcraft::workflows::curve::{cubic_coverage, cubic_polyline};

pub(super) struct CurveDraft {
    pane: CanvasPane,
    layer: usize,
    frame: usize,
    generation: u64,
    color: Pixel,
    points: Vec<(i32, i32)>,
}

impl PixelCraft {
    pub(super) fn cancel_curve(&mut self) {
        if self.curve_draft.take().is_some() {
            self.status = "Curve cancelled".into();
        }
    }

    pub(super) fn curve_click(&mut self, pane: CanvasPane, point: (i32, i32)) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            self.status = "Choose an unlocked image layer for the curve".into();
            return;
        }
        let draft = self.curve_draft.get_or_insert_with(|| CurveDraft {
            pane,
            layer: self.layer,
            frame: self.frame,
            generation: self.power.bridge_generation,
            color: self.stroke_color,
            points: Vec::with_capacity(4),
        });
        if (draft.pane, draft.layer, draft.frame, draft.generation)
            != (pane, self.layer, self.frame, self.power.bridge_generation)
        {
            self.cancel_curve();
            return;
        }
        draft.points.push(point);
        match draft.points.as_slice() {
            [_] => self.status = "Click the curve endpoint. Esc cancels.".into(),
            [_, _] => self.status = "Place the first control point.".into(),
            [_, _, _] => self.status = "Place the second control point to draw the curve.".into(),
            &[start, end, first, second] => {
                let color = draft.color;
                self.curve_draft = None;
                match cubic_coverage(
                    (self.doc.width, self.doc.height),
                    [start, first, second, end],
                    (self.brush, self.doc.brush_shape),
                    self.drawing_axes(),
                ) {
                    Ok(pixels) => {
                        self.apply_shape_coverage(color, "Cubic Bézier", |paint| {
                            pixels.visit_nonclear(|index, _| paint(index));
                        });
                        self.status = "Drew cubic Bézier curve".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            _ => unreachable!("A curve draft has up to four points"),
        }
    }

    pub(super) fn draw_curve_draft(
        &self,
        painter: &egui::Painter,
        board: Rect,
        pane: CanvasPane,
        hover: Option<(i32, i32)>,
    ) {
        let Some(draft) = self.curve_draft.as_ref().filter(|draft| {
            (draft.pane, draft.layer, draft.frame, draft.generation)
                == (pane, self.layer, self.frame, self.power.bridge_generation)
        }) else {
            return;
        };
        let step = self.canvas_pixel_size();
        let position =
            |(x, y): (i32, i32)| board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * step;
        let start = draft.points[0];
        let end = draft.points.get(1).copied().or(hover).unwrap_or(start);
        let first = draft
            .points
            .get(2)
            .copied()
            .or(hover.filter(|_| draft.points.len() == 2))
            .unwrap_or(start);
        let second = hover.filter(|_| draft.points.len() == 3).unwrap_or(end);
        if let Ok(path) = cubic_polyline([start, first, second, end])
            && path.len() > 1
        {
            let path: Vec<_> = path.into_iter().map(position).collect();
            painter.add(egui::Shape::line(
                path.clone(),
                Stroke::new(3., self.colors().ink),
            ));
            painter.add(egui::Shape::line(
                path,
                Stroke::new(1., self.colors().accent),
            ));
        }
        let guide = Stroke::new(1., self.colors().ink);
        for (anchor, control) in [(start, first), (end, second)] {
            if anchor != control {
                painter.line_segment([position(anchor), position(control)], guide);
                painter.circle_stroke(position(control), 3., guide);
            }
        }
        for &point in draft.points.iter().take(2) {
            painter.circle_filled(position(point), 3., self.colors().accent);
        }
    }
}
