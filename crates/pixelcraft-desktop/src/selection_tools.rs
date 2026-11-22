use super::*;
use crate::api::{
    productivity::{SelectionOp, combine_selection},
    workflows::selection::{
        MaskEffect, change_mask, color_range, polygon_mask, read_mask_png, write_mask_png,
    },
};

pub(super) struct RememberedMask {
    dimensions: (usize, usize),
    bounds: SelectionBounds,
    mask: Option<SelectionMask>,
    polygon: Vec<(i32, i32)>,
}

pub(super) fn marquee_operation(modifiers: egui::Modifiers) -> SelectionOp {
    match (modifiers.shift, modifiers.command || modifiers.ctrl) {
        (true, true) => SelectionOp::Intersect,
        (true, false) => SelectionOp::Add,
        (false, true) => SelectionOp::Subtract,
        (false, false) => SelectionOp::Replace,
    }
}

#[derive(Clone, Copy)]
enum SelectionDialog {
    Modify,
    ColorRange,
}

pub(super) struct SelectionTools {
    dialog: Option<SelectionDialog>,
    pub(super) previous: Option<RememberedMask>,
    radius: usize,
    color: Pixel,
    tolerance: u8,
    merged: bool,
    operation: SelectionOp,
    polygon: Option<PolygonDraft>,
}

struct PolygonDraft {
    points: Vec<(i32, i32)>,
    pane: CanvasPane,
    action: PolygonAction,
}

#[derive(Clone, Copy)]
enum PolygonAction {
    Select(SelectionOp),
    Paint {
        layer: usize,
        frame: usize,
        generation: u64,
        color: Pixel,
    },
}

impl Default for SelectionTools {
    fn default() -> Self {
        Self {
            dialog: None,
            previous: None,
            radius: 1,
            color: CLEAR,
            tolerance: 0,
            merged: true,
            operation: SelectionOp::Replace,
            polygon: None,
        }
    }
}

impl SelectionTools {
    pub(super) fn open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn cancel_polygon(&mut self) {
        self.polygon = None;
    }
}

impl PixelCraft {
    pub(super) fn polygon_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if self.selection_tools.polygon.is_none() {
            return false;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.selection_tools.cancel_polygon();
            self.status = "Polygon cancelled".into();
            return true;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter)) {
            if self.tool != Tool::Contour {
                self.finish_polygon();
            }
            return true;
        }
        false
    }

    pub(super) fn polygon_vertex(
        &mut self,
        point: (i32, i32),
        pane: CanvasPane,
        operation: SelectionOp,
    ) {
        if self
            .selection_tools
            .polygon
            .as_ref()
            .is_none_or(|draft| draft.pane != pane)
        {
            if self.tool == Tool::PolygonLasso {
                self.selection_tools.polygon = Some(PolygonDraft {
                    points: vec![point],
                    pane,
                    action: PolygonAction::Select(operation),
                });
            } else if !self.begin_painted_polygon(point, pane) {
                return;
            }
            self.status =
                "Click to add vertices. Enter or click the first point to finish. Esc cancels."
                    .into();
            return;
        }
        let draft = self.selection_tools.polygon.as_mut().unwrap();
        if draft.points.len() >= 3 && draft.points.first() == Some(&point) {
            self.finish_polygon();
        } else if draft.points.last() != Some(&point) && draft.points.len() < 4096 {
            draft.points.push(point);
        }
    }

    pub(super) fn begin_painted_polygon(&mut self, point: (i32, i32), pane: CanvasPane) -> bool {
        if !self.session.doc.editable(self.session.layer)
            || self.session.doc.layers[self.session.layer]
                .tileset
                .is_some()
        {
            self.status = "Choose an unlocked image layer to draw this shape".into();
            return false;
        }
        self.selection_tools.polygon = Some(PolygonDraft {
            points: vec![point],
            pane,
            action: PolygonAction::Paint {
                layer: self.session.layer,
                frame: self.session.frame,
                generation: self.power.bridge_generation,
                color: self.stroke_color,
            },
        });
        true
    }

    pub(super) fn contour_point(&mut self, point: (i32, i32)) {
        let Some(draft) = &mut self.selection_tools.polygon else {
            return;
        };
        let last = *draft.points.last().unwrap();
        if point == last {
            return;
        }
        if let Some(previous) = draft.points.iter().rev().nth(1).copied() {
            let a = (last.0 - previous.0, last.1 - previous.1);
            let b = (point.0 - last.0, point.1 - last.1);
            if a.0 * b.1 == a.1 * b.0 && a.0 * b.0 + a.1 * b.1 > 0 {
                *draft.points.last_mut().unwrap() = point;
                return;
            }
        }
        if draft.points.len() < 4096 {
            draft.points.push(point);
        } else {
            self.selection_tools.cancel_polygon();
            self.status = "Contour exceeds 4096 distinct path points".into();
        }
    }

    pub(super) fn finish_polygon(&mut self) {
        if self
            .selection_tools
            .polygon
            .as_ref()
            .is_none_or(|draft| draft.points.len() < 3)
        {
            return;
        }
        let draft = self.selection_tools.polygon.take().unwrap();
        match draft.action {
            PolygonAction::Select(operation) => {
                match polygon_mask(
                    self.session.doc.width,
                    self.session.doc.height,
                    &draft.points,
                ) {
                    Ok(hits) => {
                        let mut mask = self.canvas_selection_mask();
                        combine_selection(&mut mask, &hits, operation);
                        self.commit_selection_mask(mask, "Polygonal lasso");
                        self.status = "Polygon selection".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            PolygonAction::Paint {
                layer,
                frame,
                generation,
                color,
            } => {
                if (layer, frame, generation)
                    != (
                        self.session.layer,
                        self.session.frame,
                        self.power.bridge_generation,
                    )
                    || !self.session.doc.editable(layer)
                    || self.session.doc.layers[layer].tileset.is_some()
                {
                    self.status = "Shape cancelled after the target cel changed".into();
                    return;
                }
                let contour = self.tool == Tool::Contour;
                let hits = match crate::api::workflows::polygon_coverage(
                    (self.session.doc.width, self.session.doc.height),
                    &draft.points,
                    contour || self.filled,
                    (self.brush, self.session.doc.brush_shape),
                    self.drawing_axes(),
                ) {
                    Ok(hits) => hits,
                    Err(error) => {
                        self.status = error;
                        return;
                    }
                };
                self.apply_shape_coverage(
                    color,
                    if contour { "Filled contour" } else { "Polygon" },
                    |paint| {
                        for (index, hit) in hits.into_iter().enumerate() {
                            if hit {
                                paint(index);
                            }
                        }
                    },
                );
                self.status = if contour {
                    "Drew filled contour"
                } else {
                    "Drew polygon"
                }
                .into();
            }
        }
    }

    pub(super) fn apply_shape_coverage(
        &mut self,
        color: Pixel,
        label: &'static str,
        visit: impl FnOnce(&mut dyn FnMut(usize)),
    ) {
        let mut result = self.session.doc.clone();
        result.drawing_index = self.drawing_palette_index(color);
        let (layer, frame, width) = (self.session.layer, self.session.frame, result.width);
        let ink = self.tool_ink();
        visit(&mut |index| {
            if self.selected((index % width) as i32, (index / width) as i32) {
                let original = self.session.doc.layers[layer].cels[frame][index];
                ink.draw(&mut result, (layer, frame), index, original, color);
            }
        });
        result.drawing_index = None;
        if result != self.session.doc {
            self.remember_action(label);
            self.session.doc = result;
        }
    }

    pub(super) fn draw_polygon_draft(
        &self,
        painter: &egui::Painter,
        board: Rect,
        pane: CanvasPane,
        pointer: Option<Pos2>,
    ) {
        let Some(draft) = self
            .selection_tools
            .polygon
            .as_ref()
            .filter(|draft| draft.pane == pane)
        else {
            return;
        };
        let step = self.canvas_pixel_size();
        let points: Vec<_> = draft
            .points
            .iter()
            .map(|&(x, y)| board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * step)
            .chain(pointer)
            .collect();
        let white = Stroke::new(1., Color32::WHITE);
        if points.len() > 1 {
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(3., self.colors().ink),
            ));
            painter.add(egui::Shape::line(points.clone(), white));
        }
        painter.circle_stroke(points[0], 4., white);
    }
    pub(super) fn canvas_selection_mask(&self) -> Vec<bool> {
        if self.session.selection.is_none() {
            vec![false; self.session.doc.width * self.session.doc.height]
        } else if let Some(mask) = &self.session.mask {
            mask.to_vec()
        } else {
            (0..self.session.doc.height as i32)
                .flat_map(|y| (0..self.session.doc.width as i32).map(move |x| self.selected(x, y)))
                .collect()
        }
    }

    pub(super) fn commit_selection_mask(&mut self, mask: Vec<bool>, label: &'static str) {
        let changed = self.session.selection.is_none()
            || mask.iter().enumerate().any(|(index, &selected)| {
                selected
                    != self.selected(
                        (index % self.session.doc.width) as i32,
                        (index / self.session.doc.width) as i32,
                    )
            });
        if changed {
            self.remember_action(label);
            self.session.polygon.clear();
            self.set_selection_mask(mask);
        }
    }

    pub(super) fn deselect_and_remember(&mut self) {
        if let Some(bounds) = self.session.selection.take() {
            self.selection_tools.previous = Some(RememberedMask {
                dimensions: (self.session.doc.width, self.session.doc.height),
                bounds,
                mask: self.session.mask.take(),
                polygon: std::mem::take(&mut self.session.polygon),
            });
        } else {
            self.session.mask = None;
            self.session.polygon.clear();
        }
    }

    pub(super) fn reselect_previous(&mut self) {
        let Some(previous) = &self.selection_tools.previous else {
            self.status = "No previous selection".into();
            return;
        };
        if previous.dimensions != (self.session.doc.width, self.session.doc.height) {
            self.status = "The previous selection belongs to a different canvas size".into();
            return;
        }
        let (bounds, mask, polygon) = (
            previous.bounds,
            previous.mask.clone(),
            previous.polygon.clone(),
        );
        if self.session.selection != Some(bounds)
            || self.session.mask != mask
            || self.session.polygon != polygon
        {
            self.remember_action("Reselect");
            self.session.selection = Some(bounds);
            self.session.mask = mask;
            self.session.polygon = polygon;
        }
    }

    pub(super) fn invert_canvas_selection(&mut self) {
        let mut mask = self.canvas_selection_mask();
        for selected in &mut mask {
            *selected = !*selected;
        }
        self.commit_selection_mask(mask, "Invert selection");
    }

    pub(super) fn apply_ellipse_marquee(
        &mut self,
        (left, top, right, bottom): SelectionBounds,
        op: SelectionOp,
    ) {
        let mut shape = Document::new(self.session.doc.width, self.session.doc.height);
        shape.ellipse(0, 0, (left, top), (right, bottom), [255; 4], true, 1);
        let mut hits = vec![false; self.session.doc.width * self.session.doc.height];
        shape.layers[0].cels[0].visit_nonclear(|index, _| hits[index] = true);
        let mut mask = self.canvas_selection_mask();
        combine_selection(&mut mask, &hits, op);
        self.commit_selection_mask(mask, "Elliptical marquee");
    }

    fn modify_canvas_selection(&mut self, effect: MaskEffect) {
        if self.session.selection.is_none() {
            self.status = "Choose a selection first".into();
            return;
        }
        match change_mask(
            &self.canvas_selection_mask(),
            self.session.doc.width,
            self.session.doc.height,
            self.selection_tools.radius,
            effect,
        ) {
            Ok(mask) => self.commit_selection_mask(
                mask,
                match effect {
                    MaskEffect::Grow => "Grow selection",
                    MaskEffect::Shrink => "Shrink selection",
                    MaskEffect::InsideBorder | MaskEffect::OutsideBorder => "Select border",
                },
            ),
            Err(error) => self.status = error,
        }
    }

    fn apply_color_range(&mut self) {
        let pixels = if self.selection_tools.merged {
            self.session.doc.composite(self.session.frame)
        } else {
            self.session
                .doc
                .layer_pixels(self.session.layer, self.session.frame)
        };
        let hits = color_range(
            &pixels,
            self.selection_tools.color,
            self.selection_tools.tolerance,
        );
        let mut mask = self.canvas_selection_mask();
        combine_selection(&mut mask, &hits, self.selection_tools.operation);
        self.commit_selection_mask(mask, "Select color range");
    }

    pub(super) fn open_selection_tools(&mut self, range: bool) {
        if range {
            self.selection_tools.color = self.fg;
        }
        self.selection_tools.dialog = Some(if range {
            SelectionDialog::ColorRange
        } else {
            SelectionDialog::Modify
        });
    }

    pub(super) fn selection_tools_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.selection_tools.dialog else {
            return;
        };
        let mut modification = None;
        let mut range = false;
        let mut close_button = false;
        let close = fluent::modal_dialog(
            ctx,
            match dialog {
                SelectionDialog::ColorRange => "Color Range",
                SelectionDialog::Modify => "Modify selection",
            },
            |ui| {
                match dialog {
                    SelectionDialog::ColorRange => {
                        ui.horizontal(|ui| {
                            ui.label("Match color");
                            let mut color = self.selection_tools.color;
                            if ui
                                .color_edit_button_srgba_unmultiplied(&mut color)
                                .changed()
                            {
                                self.selection_tools.color = color;
                            }
                        });
                        ui.add(
                            egui::Slider::new(&mut self.selection_tools.tolerance, 0..=255)
                                .text("Tolerance"),
                        );
                        ui.pc_checkbox(&mut self.selection_tools.merged, "Sample visible layers");
                        ui.horizontal_wrapped(|ui| {
                            for (mode, label) in [
                                (SelectionOp::Replace, "Replace"),
                                (SelectionOp::Add, "Add"),
                                (SelectionOp::Subtract, "Subtract"),
                                (SelectionOp::Intersect, "Intersect"),
                            ] {
                                ui.pc_selectable_value(
                                    &mut self.selection_tools.operation,
                                    mode,
                                    label,
                                );
                            }
                        });
                        range = ui.pc_button("Select color range").clicked();
                    }
                    SelectionDialog::Modify => {
                        ui.add(
                            egui::DragValue::new(&mut self.selection_tools.radius)
                                .range(1..=256)
                                .prefix("Radius ")
                                .suffix(" px"),
                        );
                        ui.add_enabled_ui(self.session.selection.is_some(), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (effect, label) in [
                                    (MaskEffect::Grow, "Grow"),
                                    (MaskEffect::Shrink, "Shrink"),
                                    (MaskEffect::InsideBorder, "Inside border"),
                                    (MaskEffect::OutsideBorder, "Outside border"),
                                ] {
                                    if ui.pc_button(label).clicked() {
                                        modification = Some(effect);
                                    }
                                }
                            });
                        });
                    }
                }
                close_button = ui.pc_button("Cancel").clicked();
                close_button || range || modification.is_some()
            },
        );
        if let Some(effect) = modification {
            self.modify_canvas_selection(effect);
        }
        if range {
            self.apply_color_range();
        }
        if close || close_button || range || modification.is_some() {
            self.selection_tools.dialog = None;
        }
    }

    pub(super) fn import_selection_mask(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG mask", &["png"])
            .pick_file()
        {
            match read_mask_png(&path, self.session.doc.width, self.session.doc.height) {
                Ok(mask) => self.commit_selection_mask(mask, "Import selection mask"),
                Err(error) => self.status = error,
            }
        }
    }

    pub(super) fn export_selection_mask(&mut self) {
        if self.session.selection.is_none() {
            self.status = "Choose a selection first".into();
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG mask", &["png"])
            .set_file_name("selection.png")
            .save_file()
        {
            let path = if path.extension().is_none() {
                path.with_extension("png")
            } else {
                path
            };
            self.status = match write_mask_png(
                &path,
                &self.canvas_selection_mask(),
                self.session.doc.width,
                self.session.doc.height,
            ) {
                Ok(()) => "Exported selection mask".into(),
                Err(error) => error,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reusable_selections_keep_holes_color_tolerance_and_per_tab_reselect() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(3, 3);
        let red = [100, 20, 30, 255];
        doc.layers[0].cels[0].set(0, red);
        doc.layers[0].cels[0].set(4, [104, 20, 30, 255]);
        doc.layers[0].cels[0].set(8, [180, 20, 30, 255]);
        app.replace(doc.clone(), None);
        app.fg = red;
        app.action("color-range");
        app.selection_tools.tolerance = 4;
        app.apply_color_range();
        app.selection_tools.dialog = None;
        let isolated = [true, false, false, false, true, false, false, false, false];
        assert_eq!(app.canvas_selection_mask(), isolated);
        assert_eq!(app.undo.len(), 1);
        app.action("deselect");
        assert!(app.selection.is_none());
        let original_tab = app.active_tab;
        app.open_document(Document::new(3, 3), None);
        app.action("selectall");
        app.action("deselect");
        app.switch_tab(original_tab);
        app.action("reselect");
        assert_eq!(app.canvas_selection_mask(), isolated);
        app.action("invert-selection");
        assert_eq!(app.canvas_selection_mask(), isolated.map(|pixel| !pixel));
        app.undo();
        assert_eq!(app.canvas_selection_mask(), isolated);

        app.selection_tools.operation = SelectionOp::Intersect;
        app.selection_tools.color = [180, 20, 30, 255];
        app.apply_color_range();
        assert!(app.selection.is_some());
        assert_eq!(app.canvas_selection_mask(), [false; 9]);
        assert_eq!(app.doc, doc);
    }
}
