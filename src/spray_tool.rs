use super::*;
use pixelcraft::workflows::mix_paint_seed;

pub(super) struct SprayStroke {
    generation: u64,
    layer: usize,
    frame: usize,
    state: u64,
    time: f64,
    previous: Option<(i32, i32)>,
    credit: f64,
}

impl SprayStroke {
    fn offset(&mut self, radius: i32) -> (i32, i32) {
        let diameter = (2 * radius + 1) as u64;
        loop {
            self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
            let random = mix_paint_seed(self.state);
            let x = ((random >> 32) % diameter) as i32 - radius;
            let y = ((random & u64::from(u32::MAX)) % diameter) as i32 - radius;
            if x * x + y * y <= radius * radius {
                return (x, y);
            }
        }
    }
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
            state: time.to_bits() ^ self.power.bridge_generation,
            time,
            previous: Some(point),
            credit: f64::from((self.spray_rate / 24).max(1)),
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
        let elapsed = (now - stroke.time).clamp(0., 0.125);
        stroke.time = now;
        if let Some((x, y)) = point {
            let (previous_x, previous_y) = stroke.previous.unwrap_or((x, y));
            if stroke.previous.is_some() {
                stroke.credit += elapsed * f64::from(self.spray_rate);
            }
            let count = stroke.credit.floor() as u16;
            stroke.credit -= f64::from(count);
            let radius = self.spray_radius;
            let axes = self.drawing_axes();
            let (mirror_x, mirror_y) = (self.studio.mirror_x, self.studio.mirror_y);
            let (symmetry_x, symmetry_y) = (self.symmetry_axis(0), self.symmetry_axis(1));
            self.doc.drawing_index = self.drawing_palette_index(self.stroke_color);
            for i in 1..=count {
                let t = f64::from(i) / f64::from(count);
                let (dx, dy) = stroke.offset(radius);
                let x = previous_x + (f64::from(x - previous_x) * t).round() as i32 + dx;
                let y = previous_y + (f64::from(y - previous_y) * t).round() as i32 + dy;
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
                            self.doc
                                .set_pixel(self.layer, self.frame, index, self.stroke_color);
                        }
                    }
                }
            }
            self.doc.drawing_index = None;
            ctx.request_repaint_after(Duration::from_secs_f64(
                ((1. - stroke.credit) / f64::from(self.spray_rate)).max(1. / 60.),
            ));
        } else {
            stroke.credit = 0.;
        }
        stroke.previous = point;
        self.spray_stroke = Some(stroke);
    }
}
