use super::{Quantizer, SlotSwap};
use crate::model::{CLEAR, OutsidePixel, Pixel, Pixels};
use std::sync::Arc;

struct Diffusion {
    left: i32,
    phase: i32,
    row: Option<i32>,
    current: Vec<[i32; 4]>,
    next: Vec<[i32; 4]>,
}

fn scan_key(x: i32, y: i32, phase: i32) -> (i32, i32) {
    (
        y,
        if (y - phase).rem_euclid(2) == 0 {
            x
        } else {
            -x
        },
    )
}

impl Diffusion {
    fn new(left: i32, right: i32, phase: i32) -> Result<Self, String> {
        if left < -131_072 || right > 131_072 || left > right {
            return Err("Indexed diffusion exceeds supported coordinates".into());
        }
        let width = (right - left) as usize + 3;
        Ok(Self {
            left,
            phase,
            row: None,
            current: vec![[0; 4]; width],
            next: vec![[0; 4]; width],
        })
    }

    fn map(
        &mut self,
        q: &mut Quantizer,
        color: Pixel,
        authored: Option<usize>,
        x: i32,
        y: i32,
    ) -> Result<usize, String> {
        if self.row != Some(y) {
            if self.row.is_some_and(|row| row + 1 == y) {
                std::mem::swap(&mut self.current, &mut self.next);
            } else {
                self.current.fill([0; 4]);
            }
            self.next.fill([0; 4]);
            self.row = Some(y);
        }
        let index = (x - self.left) as usize + 1;
        if color[3] == 0 {
            self.current[index] = [0; 4];
            return q.slot(color, authored, None);
        }
        let corrected = std::array::from_fn(|channel| {
            (i32::from(color[channel]) + self.current[index][channel]).clamp(0, 255) as u8
        });
        let mut slot = q.slot(corrected, authored, None).or_else(|error| {
            if !q.background && q.visible.is_empty() {
                Ok(q.transparent)
            } else {
                Err(error)
            }
        })?;
        // Preserve the old alpha fallback only for the compatibility metric.
        // Explicit criteria already choose a slot with their own alpha distance.
        if !q.background && q.fit == super::IndexedFit::Default {
            let error = corrected
                .iter()
                .zip(q.palette[slot])
                .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum::<i32>();
            if i32::from(corrected[3]).pow(2) < error {
                slot = q.transparent;
            }
        }
        let mut chosen = q.palette[slot];
        if !q.background && slot == q.transparent {
            chosen = [corrected[0], corrected[1], corrected[2], 0];
        }
        let reverse = (y - self.phase).rem_euclid(2) != 0;
        let forward = if reverse { index - 1 } else { index + 1 };
        let behind = if reverse { index + 1 } else { index - 1 };
        for channel in 0..4 {
            let error = (i32::from(corrected[channel]) - i32::from(chosen[channel]))
                * q.diffusion_factor
                / 100;
            self.current[forward][channel] += error * 7 / 16;
            self.next[behind][channel] += error * 3 / 16;
            self.next[index][channel] += error * 5 / 16;
            self.next[forward][channel] += error / 16;
        }
        Ok(slot)
    }
}

pub(super) struct Converted {
    pub pixels: Pixels,
    pub indices: Arc<Vec<u8>>,
    pub outside: Vec<OutsidePixel>,
}

pub(super) struct Raster<'a> {
    pub pixels: &'a Pixels,
    pub size: [usize; 2],
    pub outside: &'a [OutsidePixel],
    pub phase: i32,
    pub authored: Option<&'a [u8]>,
    pub swap: SlotSwap,
    pub keep_outside_slots: bool,
}

pub(super) fn convert(
    raster: Raster<'_>,
    quantizer: &mut Quantizer,
    cancelled: &impl Fn() -> bool,
) -> Result<Converted, String> {
    let Raster {
        pixels: source,
        size: [width, height],
        outside,
        phase,
        authored,
        swap,
        keep_outside_slots,
    } = raster;
    let mut left = 0;
    let mut right = width as i32 - 1;
    for sample in outside {
        left = left.min(sample.x);
        right = right.max(sample.x);
        if !(-131_072..=131_072).contains(&sample.y) {
            return Err("Indexed diffusion exceeds supported coordinates".into());
        }
    }
    let mut diffuser = Diffusion::new(left, right, phase)?;
    let mut hidden: Vec<_> = outside.iter().enumerate().collect();
    hidden.sort_unstable_by_key(|(_, sample)| scan_key(sample.x, sample.y, phase));
    let mut hidden = hidden.into_iter().peekable();
    let mut indices = vec![quantizer.transparent as u8; width * height];
    let mut pixels = Pixels::empty(indices.len());
    let mut output = outside.to_vec();
    let mut visible = 0;
    let mut processed = 0;
    while visible < indices.len() || hidden.peek().is_some() {
        if processed % 8192 == 0 && cancelled() {
            return Err("Indexed conversion cancelled".into());
        }
        processed += 1;
        let y = (visible / width) as i32;
        let column = visible % width;
        let x = if (y - phase).rem_euclid(2) == 0 {
            column
        } else {
            width - 1 - column
        } as i32;
        if hidden.peek().is_some_and(|(_, sample)| {
            visible == indices.len() || scan_key(sample.x, sample.y, phase) < scan_key(x, y, phase)
        }) {
            let (index, sample) = hidden.next().unwrap();
            let old = keep_outside_slots
                .then_some(sample.palette_slot)
                .flatten()
                .map(|slot| swap.slot(slot as usize));
            let slot = diffuser.map(quantizer, sample.color, old, sample.x, sample.y)?;
            output[index].color = quantizer.palette[slot];
            output[index].palette_slot = Some(slot as u8);
        } else {
            let index = y as usize * width + x as usize;
            let old = authored.map(|values| swap.slot(values[index] as usize));
            let color = source[index];
            let slot = diffuser.map(quantizer, color, old, x, y)?;
            indices[index] = slot as u8;
            if color != CLEAR {
                pixels.set(index, quantizer.palette[slot]);
            }
            visible += 1;
        }
    }
    Ok(Converted {
        pixels,
        indices: Arc::new(indices),
        outside: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffusion_alternates_midtones_and_absorbs_transparent_barriers() {
        let mut q = Quantizer::new(
            vec![CLEAR, [0, 0, 0, 255], [255; 4]],
            0,
            false,
            false,
            super::super::IndexedFit::Default,
        );
        let mut d = Diffusion::new(0, 2, 0).unwrap();
        assert_eq!(d.map(&mut q, [128, 128, 128, 255], None, 0, 0).unwrap(), 2);
        assert_eq!(d.map(&mut q, CLEAR, None, 1, 0).unwrap(), 0);
        assert_eq!(d.map(&mut q, [128, 128, 128, 255], None, 2, 0).unwrap(), 2);
        // Several absent rows absorb all previous errors.
        assert_eq!(d.map(&mut q, [128, 128, 128, 255], None, 0, 5).unwrap(), 2);
    }
}
