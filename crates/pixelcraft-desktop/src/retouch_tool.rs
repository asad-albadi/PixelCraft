use super::*;
use crate::api::workflows::retouch::{Retouch, RetouchBrush, RetouchStroke};

pub(super) struct RetouchGesture {
    stroke: RetouchStroke,
    generation: u64,
    cel: (usize, usize),
}

impl PixelCraft {
    pub(super) fn begin_retouch(
        &mut self,
        pane: CanvasPane,
        button: egui::PointerButton,
        point: (i32, i32),
    ) {
        let (mode, label) = if self.tool == Tool::BlurBrush {
            (Retouch::Blur, "Blur brush")
        } else {
            (Retouch::Jumble, "Jumble brush")
        };
        let cel = (self.layer, self.frame);
        let strength = ((u16::from(self.retouch_strength) * 255 + 50) / 100) as u8;
        if strength == 0 {
            return;
        }
        let brush = RetouchBrush {
            mode,
            size: self.brush,
            shape: self.doc.brush_shape,
            strength,
            wrap: self.drawing_axes(),
            mirrors: [
                self.studio.mirror_x.then(|| self.symmetry_axis(0)),
                self.studio.mirror_y.then(|| self.symmetry_axis(1)),
            ],
            seed: self.power.bridge_generation,
        };
        let mask = self.selection.map(|_| self.canvas_selection_mask());
        match RetouchStroke::new(&self.doc, cel, brush, mask) {
            Ok(stroke) => {
                self.remember_action(label);
                self.gesture_canvas = Some(pane);
                self.gesture_button = Some(button);
                self.start = Some(point);
                self.last = Some(point);
                self.retouch_gesture = Some(RetouchGesture {
                    stroke,
                    cel,
                    generation: self.power.bridge_generation,
                });
                let gesture = self.retouch_gesture.as_ref().unwrap();
                gesture.stroke.paint(&mut self.doc, point, point);
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn extend_retouch(&mut self, point: (i32, i32)) {
        let Some(gesture) = &self.retouch_gesture else {
            return;
        };
        if gesture.cel != (self.layer, self.frame)
            || gesture.generation != self.power.bridge_generation
        {
            self.cancel_gesture();
            return;
        }
        if let Some(last) = self.last.filter(|&last| last != point) {
            gesture.stroke.paint(&mut self.doc, last, point);
        }
        self.last = Some(point);
    }
}
