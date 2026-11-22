use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct BrushDynamics {
    pub stabilizer: u8,
    pub pressure_size: bool,
    pub pressure_opacity: bool,
}

#[derive(Clone, Copy)]
pub(super) struct StrokeStabilizer {
    raw: (i32, i32),
    brush: Vec2,
    radius: f32,
}

impl StrokeStabilizer {
    pub(super) fn new(point: (i32, i32), radius: u8) -> Self {
        Self {
            raw: point,
            brush: Vec2::new(point.0 as f32, point.1 as f32),
            radius: f32::from(radius.min(64)),
        }
    }

    fn advance(&mut self, point: Option<(i32, i32)>, release: bool) -> (i32, i32) {
        if let Some(point) = point {
            self.raw = point;
        }
        if release || self.radius == 0. {
            return self.raw;
        }
        let pointer = Vec2::new(self.raw.0 as f32, self.raw.1 as f32);
        let delta = pointer - self.brush;
        let distance = delta.length();
        if distance > self.radius {
            self.brush += delta * ((distance - self.radius) / distance);
        }
        (self.brush.x.round() as i32, self.brush.y.round() as i32)
    }
}

impl PixelCraft {
    pub(super) fn extend_freehand(
        &mut self,
        point: Option<(i32, i32)>,
        release: bool,
        color: Pixel,
    ) {
        let Some(stabilizer) = &mut self.stroke_stabilizer else {
            return;
        };
        let end = stabilizer.advance(point, release);
        let from = self.last.unwrap_or(end);
        self.stroke_line(from, end, color);
        self.last = Some(end);
    }
}
