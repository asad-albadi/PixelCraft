use super::*;
pub use pixelcraft_core::editor::{BrushDynamics, StrokeStabilizer};
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
