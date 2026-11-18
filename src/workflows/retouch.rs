use crate::model::{
    BrushShape, CLEAR, ColorMode, Document, MAX_DIMENSION, Pixel, Pixels, WrapAxes, line_pixels,
};
use std::{convert::Infallible, sync::Arc};

type Point = (i32, i32);

#[derive(Clone, Copy)]
pub enum Retouch {
    Blur,
    Jumble,
}

pub struct RetouchBrush {
    pub mode: Retouch,
    pub size: i32,
    pub shape: BrushShape,
    pub strength: u8,
    pub wrap: WrapAxes,
    pub mirrors: [Option<i32>; 2],
    pub seed: u64,
}

pub struct RetouchStroke {
    cel: (usize, usize),
    dimensions: (usize, usize),
    brush: RetouchBrush,
    source: Pixels,
    indices: Option<Arc<Vec<u8>>>,
    mask: Option<Vec<bool>>,
}

impl RetouchStroke {
    pub fn new(
        doc: &Document,
        (layer, frame): (usize, usize),
        brush: RetouchBrush,
        mask: Option<Vec<bool>>,
    ) -> Result<Self, String> {
        if !doc.editable(layer)
            || doc.layers[layer].tileset.is_some()
            || frame >= doc.durations.len()
        {
            return Err("Choose an unlocked image layer for the brush".into());
        }
        Ok(Self {
            cel: (layer, frame),
            dimensions: (doc.width, doc.height),
            brush,
            source: doc.layers[layer].cels[frame].clone(),
            indices: doc.layers[layer].cel_meta[frame].indices.clone(),
            mask,
        })
    }

    fn index(&self, (x, y): Point) -> usize {
        y as usize * self.dimensions.0 + x as usize
    }

    fn neighbor(&self, (x, y): Point) -> usize {
        let (w, h) = (self.dimensions.0 as i32, self.dimensions.1 as i32);
        let x = if self.brush.wrap.horizontal {
            x.rem_euclid(w)
        } else {
            x.clamp(0, w - 1)
        };
        let y = if self.brush.wrap.vertical {
            y.rem_euclid(h)
        } else {
            y.clamp(0, h - 1)
        };
        self.index((x, y))
    }

    fn blur(&self, (x, y): Point, original: Pixel) -> Pixel {
        let mut total = [0u16; 4];
        let mut count = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let pixel = self.source[self.neighbor((x + dx, y + dy))];
                if pixel[3] != 0 {
                    for (sum, channel) in total.iter_mut().zip(pixel) {
                        *sum += u16::from(channel);
                    }
                    count += 1;
                }
            }
        }
        let [r, g, b, alpha] = total;
        let Some(red) = r.checked_div(count) else {
            return original;
        };
        [
            red as u8,
            (g / count) as u8,
            (b / count) as u8,
            (alpha / 9) as u8,
        ]
    }

    pub fn paint(&self, doc: &mut Document, start: Point, end: Point) {
        let (layer, frame) = self.cel;
        if self.brush.strength == 0
            || self.dimensions != (doc.width, doc.height)
            || !doc.editable(layer)
            || doc.layers[layer].tileset.is_some()
            || frame >= doc.durations.len()
            || [start, end]
                .into_iter()
                .any(|(x, y)| x.unsigned_abs().max(y.unsigned_abs()) as usize > 2 * MAX_DIMENSION)
        {
            return;
        }
        let drift = (
            ((end.0 - start.0) / 4).clamp(-8, 8),
            ((end.1 - start.1) / 4).clamp(-8, 8),
        );
        let indexed = doc.color_mode == ColorMode::Indexed && !doc.layers[layer].background;
        let [mirror_x, mirror_y] = self.brush.mirrors;
        let _: Result<(), Infallible> = line_pixels(start, end, |(x, y)| {
            for center_x in [Some(x), mirror_x.map(|axis| axis - 1 - x)]
                .into_iter()
                .flatten()
            {
                for center_y in [Some(y), mirror_y.map(|axis| axis - 1 - y)]
                    .into_iter()
                    .flatten()
                {
                    for (dx, dy) in self.brush.shape.offsets(self.brush.size) {
                        let Some(point) = self
                            .brush
                            .wrap
                            .position(self.dimensions, (center_x + dx, center_y + dy))
                        else {
                            continue;
                        };
                        let index = self.index(point);
                        if self
                            .mask
                            .as_ref()
                            .is_some_and(|mask| mask.get(index) != Some(&true))
                        {
                            continue;
                        }
                        let source = self.source[index];
                        let (effect, slot) = match self.brush.mode {
                            Retouch::Blur => (self.blur(point, source), None),
                            Retouch::Jumble => {
                                let random = super::mix_paint_seed(self.brush.seed ^ index as u64);
                                let x = point.0 + (random % 3) as i32 - 1 - drift.0;
                                let y = point.1 + ((random >> 32) % 3) as i32 - 1 - drift.1;
                                let neighbor = self.neighbor((x, y));
                                let slot = self.indices.as_ref().map(|slots| slots[neighbor]);
                                (self.source[neighbor], slot)
                            }
                        };
                        let result = mix(source, effect, self.brush.strength);
                        let (result, slot) = if indexed && result[3] < 128 {
                            (CLEAR, Some(doc.transparent_index as u8))
                        } else {
                            (result, slot.filter(|_| self.brush.strength == 255))
                        };
                        let current = &doc.layers[layer];
                        if result != current.cels[frame][index]
                            || slot.is_some_and(|slot| {
                                current.cel_meta[frame]
                                    .indices
                                    .as_ref()
                                    .is_some_and(|v| v[index] != slot)
                            })
                        {
                            doc.restore_pixel(layer, frame, index, result, slot);
                        }
                    }
                }
            }
            Ok(())
        });
    }
}

fn mix(original: Pixel, effect: Pixel, strength: u8) -> Pixel {
    let source_weight = u32::from(original[3]) * u32::from(255 - strength);
    let effect_weight = u32::from(effect[3]) * u32::from(strength);
    let alpha = source_weight + effect_weight;
    if alpha == 0 {
        return CLEAR;
    }
    let mut pixel = [0; 4];
    for channel in 0..3 {
        let value = u32::from(original[channel]) * source_weight
            + u32::from(effect[channel]) * effect_weight;
        pixel[channel] = ((value + alpha / 2) / alpha) as u8;
    }
    pixel[3] = ((alpha + 127) / 255) as u8;
    pixel
}
