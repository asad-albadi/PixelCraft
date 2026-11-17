use super::{CLEAR, ColorMode, Document, MAX_DOCUMENT_PIXELS, Pixel, Pixels};
use crate::contributions::DitheringMatrix;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SlotSwap {
    from: usize,
    to: usize,
}

impl SlotSwap {
    fn slot(self, index: usize) -> usize {
        if index == self.from {
            self.to
        } else if index == self.to {
            self.from
        } else {
            index
        }
    }
}

fn reserve(palette: &mut Vec<Pixel>, previous: Option<usize>, slot: usize) -> SlotSwap {
    let original_len = palette.len();
    let clear = previous
        .filter(|&index| index < original_len)
        .or_else(|| palette.iter().position(|color| color[3] == 0));
    palette.resize(original_len.max(slot + 1), CLEAR);
    let from = if let Some(clear) = clear {
        clear
    } else if slot >= original_len {
        slot
    } else if palette.len() < 256 {
        palette.push(CLEAR);
        palette.len() - 1
    } else {
        palette[slot] = CLEAR;
        slot
    };
    palette.swap(from, slot);
    SlotSwap { from, to: slot }
}

#[derive(Clone, Copy)]
struct Pair {
    first: usize,
    second: usize,
    coverage: f32,
}

struct Quantizer {
    palette: Vec<Pixel>,
    visible: Vec<(usize, Pixel)>,
    transparent: usize,
    ordered: bool,
    cache_limit: usize,
    pairs: HashMap<Pixel, Pair>,
}

impl Quantizer {
    fn new(mut palette: Vec<Pixel>, transparent: usize, background: bool, ordered: bool) -> Self {
        if !background {
            palette[transparent][3] = 0;
        }
        let visible = palette
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
            .collect();
        Self {
            palette,
            visible,
            transparent,
            ordered,
            cache_limit: 32_768,
            pairs: HashMap::new(),
        }
    }

    fn slot(
        &mut self,
        pixel: Pixel,
        authored: Option<usize>,
        threshold: Option<f32>,
    ) -> Result<usize, String> {
        if let Some(index) = authored
            && self.palette.get(index) == Some(&pixel)
        {
            return Ok(index);
        }
        if pixel[3] == 0 {
            return Ok(self.transparent);
        }
        let pair = if let Some(pair) = self.pairs.get(&pixel) {
            *pair
        } else {
            let pair = self
                .closest_pair(pixel)
                .ok_or("The palette has no visible swatch for opaque artwork")?;
            if self.pairs.len() < self.cache_limit {
                self.pairs.insert(pixel, pair);
            }
            pair
        };
        Ok(if threshold.is_some_and(|value| value < pair.coverage) {
            pair.second
        } else {
            pair.first
        })
    }

    fn closest_pair(&self, pixel: Pixel) -> Option<Pair> {
        let first = self
            .visible
            .iter()
            .min_by_key(|(_, candidate)| {
                pixel
                    .iter()
                    .zip(*candidate)
                    .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                    .sum::<i32>()
            })?
            .0;
        let a = self.palette[first];
        if !self.ordered || a == pixel {
            return Some(Pair {
                first,
                second: first,
                coverage: 0.,
            });
        }
        let mut result = Pair {
            first,
            second: first,
            coverage: 0.,
        };
        let mut best_error = f32::INFINITY;
        for &(slot, color) in &self.visible {
            let direction: [f32; 4] =
                std::array::from_fn(|channel| f32::from(color[channel]) - f32::from(a[channel]));
            let length = direction.iter().map(|d| d * d).sum::<f32>();
            if length == 0. {
                continue;
            }
            let relative: [f32; 4] =
                std::array::from_fn(|channel| f32::from(pixel[channel]) - f32::from(a[channel]));
            let coverage = (relative
                .iter()
                .zip(&direction)
                .map(|(r, d)| r * d)
                .sum::<f32>()
                / length)
                .clamp(0., 1.);
            let error = relative
                .iter()
                .zip(&direction)
                .map(|(r, d)| (r - d * coverage).powi(2))
                .sum::<f32>();
            if error < best_error {
                best_error = error;
                result = Pair {
                    first,
                    second: slot,
                    coverage,
                };
            }
        }
        Some(result)
    }
}

impl Document {
    pub fn indexed_conversion(
        &self,
        palette: Option<Vec<Pixel>>,
        transparent: usize,
        matrix: Option<&DitheringMatrix>,
    ) -> Result<Self, String> {
        self.indexed_conversion_until(palette, transparent, matrix, || false)
    }

    pub fn indexed_conversion_until(
        &self,
        palette: Option<Vec<Pixel>>,
        transparent: usize,
        matrix: Option<&DitheringMatrix>,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, String> {
        if transparent >= 256
            || palette
                .as_ref()
                .is_some_and(|p| p.is_empty() || p.len() > 256)
            || matrix.is_some_and(|m| !m.validate())
        {
            return Err("Invalid palette, transparent slot or dithering matrix".into());
        }
        let mut next = self.clone();
        let previous = (self.color_mode == ColorMode::Indexed && palette.is_none())
            .then_some(self.transparent_index);
        if let Some(palette) = palette {
            next.palette = palette;
            next.frame_palettes.clear();
            next.palette_names.clear();
        }
        let base = reserve(&mut next.palette, previous, transparent);
        next.palette_names.resize(next.palette.len(), String::new());
        next.palette_names.swap(base.from, base.to);
        let swaps: Vec<_> = next
            .frame_palettes
            .iter_mut()
            .map(|colors| {
                if colors.is_empty() {
                    base
                } else {
                    reserve(colors, previous, transparent)
                }
            })
            .collect();
        next.transparent_index = transparent;
        next.color_mode = ColorMode::Indexed;
        next.drawing_index = None;
        let data = matrix.map(|matrix| (matrix.width, matrix.height, matrix.thresholds()));
        let threshold = |x: i32, y: i32| {
            data.as_ref().map(|(w, h, values)| {
                values[y.rem_euclid(*h as i32) as usize * w + x.rem_euclid(*w as i32) as usize]
            })
        };

        let mut palettes = HashMap::<Vec<Pixel>, usize>::new();
        let mut quantizers = Vec::new();
        let mut frames = Vec::with_capacity(self.durations.len());
        for frame in 0..self.durations.len() {
            let colors = next.palette_for(frame);
            let index = if let Some(&index) = palettes.get(colors) {
                index
            } else {
                let index = quantizers.len();
                palettes.insert(colors.to_vec(), index);
                quantizers.push([
                    Quantizer::new(colors.to_vec(), transparent, false, data.is_some()),
                    Quantizer::new(colors.to_vec(), transparent, true, data.is_some()),
                ]);
                index
            };
            frames.push(index);
        }
        let cache_limit = 32_768 / quantizers.len().max(1);
        for variants in &mut quantizers {
            for quantizer in variants {
                quantizer.cache_limit = cache_limit;
            }
        }
        let area = self.width * self.height;
        let mut shared = HashMap::new();
        let mut allocation = 0;
        for (layer, original) in next.layers.iter_mut().zip(&self.layers) {
            for (frame, ((cel, meta), source)) in layer
                .cels
                .iter_mut()
                .zip(&mut layer.cel_meta)
                .zip(&original.cels)
                .enumerate()
            {
                if cancelled() {
                    return Err("Indexed conversion cancelled".into());
                }
                let swap = swaps.get(frame).copied().unwrap_or(base);
                let variant = frames[frame];
                let quantizer = &mut quantizers[variant][usize::from(layer.background)];
                for outside in &mut meta.outside {
                    let slot =
                        quantizer.slot(outside.color, None, threshold(outside.x, outside.y))?;
                    outside.color = quantizer.palette[slot];
                }
                let authored = previous.and(meta.indices.as_ref());
                if source.backing_id().is_none() && authored.is_none() {
                    meta.indices = None;
                    continue;
                }
                let key = (
                    source.backing_id(),
                    authored.map(Arc::as_ptr),
                    swap,
                    variant,
                    layer.background,
                );
                if let Some((pixels, indices)) = shared.get(&key) {
                    *cel = Pixels::clone(pixels);
                    meta.indices = Some(Arc::clone(indices));
                    continue;
                }
                allocation += area;
                if allocation > MAX_DOCUMENT_PIXELS {
                    return Err("Indexed cel storage exceeds the project budget".into());
                }
                let mut indices = vec![transparent as u8; area];
                let mut pixels = Pixels::empty(area);
                for (i, slot) in indices.iter_mut().enumerate() {
                    if i % 8192 == 0 && cancelled() {
                        return Err("Indexed conversion cancelled".into());
                    }
                    let old = authored.map(|values| swap.slot(values[i] as usize));
                    let color = source[i];
                    let mapped = quantizer.slot(
                        color,
                        old,
                        threshold((i % self.width) as i32, (i / self.width) as i32),
                    )?;
                    *slot = mapped as u8;
                    if color != CLEAR {
                        pixels.set(i, quantizer.palette[mapped]);
                    }
                }
                let indices = Arc::new(indices);
                shared.insert(key, (pixels.clone(), Arc::clone(&indices)));
                *cel = pixels;
                meta.indices = Some(indices);
            }
        }
        let mut quantizer =
            Quantizer::new(next.palette.clone(), transparent, false, data.is_some());
        for set in &mut next.tilesets {
            for tile in &mut set.tiles {
                for index in 0..tile.len() {
                    if index % 8192 == 0 && cancelled() {
                        return Err("Indexed conversion cancelled".into());
                    }
                    let color = tile[index];
                    if color[3] == 0 {
                        continue;
                    }
                    let slot = quantizer.slot(
                        color,
                        None,
                        threshold((index % set.width) as i32, (index / set.width) as i32),
                    )?;
                    if quantizer.palette[slot] != color {
                        tile.set(index, quantizer.palette[slot]);
                    }
                }
            }
        }
        next.validate()?;
        Ok(next)
    }
}
