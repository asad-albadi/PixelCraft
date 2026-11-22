use super::*;
use crate::api::workflows::spray::SprayPattern;

pub(super) struct SprayStroke {
    generation: u64,
    layer: usize,
    frame: usize,
    pattern: SprayPattern,
    time: f64,
    previous: Option<(i32, i32)>,
}

impl PixelCraft {
    pub(super) fn begin_spray(
        &mut self,
        point: (i32, i32),
        pane: CanvasPane,
        button: egui::PointerButton,
        ctx: &egui::Context,
    ) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            self.status = "Choose an unlocked image layer to spray".into();
            return;
        }
        self.remember_action("Spray stroke");
        self.gesture_canvas = Some(pane);
        self.gesture_button = Some(button);
        let time = ctx.input(|i| i.time);
        self.spray_stroke = Some(SprayStroke {
            layer: self.layer,
            frame: self.frame,
            generation: self.power.bridge_generation,
            pattern: SprayPattern::new(
                time.to_bits() ^ self.power.bridge_generation,
                self.spray_rate,
            ),
            time,
            previous: Some(point),
        });
        self.spray_tick(Some(point), ctx);
    }

    pub(super) fn spray_tick(&mut self, point: Option<(i32, i32)>, ctx: &egui::Context) {
        let Some(mut stroke) = self.spray_stroke.take() else {
            return;
        };
        if (stroke.layer, stroke.frame, stroke.generation)
            != (self.layer, self.frame, self.power.bridge_generation)
        {
            self.cancel_gesture();
            return;
        }
        let now = ctx.input(|i| i.time);
        let elapsed = if stroke.previous.is_some() {
            now - stroke.time
        } else {
            0.
        };
        stroke.time = now;
        if let Some(point) = point {
            let previous = stroke.previous.unwrap_or(point);
            let radius = self.spray_radius;
            let axes = self.drawing_axes();
            let (mirror_x, mirror_y) = (self.studio.mirror_x, self.studio.mirror_y);
            let (symmetry_x, symmetry_y) = (self.symmetry_axis(0), self.symmetry_axis(1));
            self.doc.drawing_index = self.drawing_palette_index(self.stroke_color);
            stroke.pattern.emit(
                previous,
                point,
                elapsed,
                radius,
                self.spray_rate,
                |(x, y)| {
                    for px in [x, symmetry_x - 1 - x]
                        .into_iter()
                        .take(1 + usize::from(mirror_x))
                    {
                        for py in [y, symmetry_y - 1 - y]
                            .into_iter()
                            .take(1 + usize::from(mirror_y))
                        {
                            if let Some((px, py)) =
                                axes.position((self.doc.width, self.doc.height), (px, py))
                                && self.selected(px, py)
                            {
                                let index = py as usize * self.doc.width + px as usize;
                                self.doc.set_pixel(
                                    self.layer,
                                    self.frame,
                                    index,
                                    self.stroke_color,
                                );
                            }
                        }
                    }
                },
            );
            self.doc.drawing_index = None;
            ctx.request_repaint_after(Duration::from_secs_f64(
                stroke
                    .pattern
                    .seconds_to_next_dot(self.spray_rate)
                    .max(1. / 60.),
            ));
        } else {
            stroke.pattern.clear_credit();
        }
        stroke.previous = point;
        self.spray_stroke = Some(stroke);
    }
}
