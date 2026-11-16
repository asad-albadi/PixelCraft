use crate::model::Pixel;
use std::collections::HashMap;

const ENLARGEMENT: usize = 8;
const MAX_ENLARGED_PIXELS: usize = 4 * 1024 * 1024;

pub(super) fn enlargement_fits(width: usize, height: usize) -> bool {
    width.saturating_mul(height) <= MAX_ENLARGED_PIXELS / ENLARGEMENT.pow(2)
}

type Sample = (Pixel, Option<u8>);

pub(super) struct Enlarged {
    width: usize,
    height: usize,
    ids: Vec<u32>,
    samples: Vec<Sample>,
}

impl Enlarged {
    pub(super) fn new(
        mut width: usize,
        mut height: usize,
        source: impl Fn(usize, usize) -> Option<Sample>,
    ) -> Result<Self, String> {
        if !enlargement_fits(width, height) {
            return Err("Selection is too large for RotSprite. Choose a smaller selection or use Fast rotation.".into());
        }
        // Intern both the color and its indexed slot. ID zero represents a
        // pixel outside the selection, including a clipped part of the source.
        let mut keys = HashMap::new();
        let mut samples = Vec::new();
        let mut ids = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let id = source(x, y).map_or(0, |sample| {
                    *keys.entry(sample).or_insert_with(|| {
                        samples.push(sample);
                        samples.len() as u32
                    })
                });
                ids.push(id);
            }
        }
        for _ in 0..ENLARGEMENT.ilog2() {
            ids = scale2x(&ids, width, height);
            width *= 2;
            height *= 2;
        }
        Ok(Self {
            width,
            height,
            ids,
            samples,
        })
    }

    pub(super) fn sample(&self, x: f64, y: f64) -> Option<Sample> {
        let x = (x * ENLARGEMENT as f64).floor();
        let y = (y * ENLARGEMENT as f64).floor();
        if !(0. ..self.width as f64).contains(&x) || !(0. ..self.height as f64).contains(&y) {
            return None;
        }
        let id = self.ids[y as usize * self.width + x as usize];
        id.checked_sub(1).map(|index| self.samples[index as usize])
    }
}

// Scale2x chooses a matching pair across each corner at a color boundary.
// See https://www.scale2x.it/algorithm for the corner and border rules.
fn scale2x(source: &[u32], width: usize, height: usize) -> Vec<u32> {
    let stride = width * 2;
    let mut output = vec![0; source.len() * 4];
    for y in 0..height {
        let above = y.saturating_sub(1) * width;
        let row = y * width;
        let below = (y + 1).min(height - 1) * width;
        for x in 0..width {
            let center = source[row + x];
            let top = source[above + x];
            let bottom = source[below + x];
            let left = source[row + x.saturating_sub(1)];
            let right = source[row + (x + 1).min(width - 1)];
            let corner = |vertical, horizontal| {
                if top != bottom && left != right && vertical == horizontal {
                    horizontal
                } else {
                    center
                }
            };
            let at = y * 2 * stride + x * 2;
            output[at..at + 2].copy_from_slice(&[corner(top, left), corner(top, right)]);
            output[at + stride..at + stride + 2]
                .copy_from_slice(&[corner(bottom, left), corner(bottom, right)]);
        }
    }
    output
}
