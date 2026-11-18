use super::{ColorMode, Document, Pixel, Pixels};
use crate::workflows::{RampInterpolation, ramp_interpolated};
use std::{collections::HashMap, sync::Arc};

struct PaletteRamp {
    colors: Vec<Pixel>,
    replacements: Vec<Option<Pixel>>,
    by_color: HashMap<Pixel, Pixel>,
}

impl PaletteRamp {
    fn new(
        old: &[Pixel],
        low: usize,
        high: usize,
        steps: usize,
        remap: &[usize],
        reserved: Option<usize>,
        interpolation: RampInterpolation,
    ) -> Self {
        let mut shades = ramp_interpolated(
            old[low],
            old[high],
            steps - usize::from(reserved.is_some()),
            interpolation,
        );
        if let Some(slot) = reserved {
            shades.insert(remap[slot] - low, old[slot]);
        }
        let colors: Vec<_> = old[..low]
            .iter()
            .copied()
            .chain(shades)
            .chain(old[high + 1..].iter().copied())
            .collect();
        let replacements = old
            .iter()
            .zip(remap)
            .map(|(&before, &slot)| (before != colors[slot]).then_some(colors[slot]))
            .collect::<Vec<_>>();
        // Tiles and legacy off-canvas pixels may store RGBA alone. Prefer any surviving
        // swatch of the same color when the original index is unavailable.
        let mut by_color = HashMap::with_capacity(old.len());
        for (&old, &new) in old.iter().zip(&replacements) {
            if let Some(new) = new {
                by_color.entry(old).or_insert(new);
            } else {
                by_color.insert(old, old);
            }
        }
        Self {
            colors,
            replacements,
            by_color,
        }
    }

    fn color(&self, pixel: Pixel) -> Pixel {
        if pixel[3] == 0 {
            return pixel;
        }
        self.by_color.get(&pixel).copied().unwrap_or(pixel)
    }
}

impl Document {
    /// Replace an inclusive span and return the old-to-new slot map.
    /// Selected old slots map proportionally from one endpoint to the other.
    pub fn replace_palette_span(
        &mut self,
        first: usize,
        last: usize,
        steps: usize,
    ) -> Result<Vec<usize>, String> {
        self.replace_palette_span_interpolated(first, last, steps, RampInterpolation::Rgb)
    }

    pub fn replace_palette_span_interpolated(
        &mut self,
        first: usize,
        last: usize,
        steps: usize,
        interpolation: RampInterpolation,
    ) -> Result<Vec<usize>, String> {
        let (low, high) = (first.min(last), first.max(last));
        if low == high || !(2..=256).contains(&steps) {
            return Err("Choose two distinct swatches and 2–256 ramp steps".into());
        }
        let fits = |len| high < len && len - (high - low + 1) + steps <= 256;
        if !fits(self.palette.len())
            || self
                .frame_palettes
                .iter()
                .any(|colors| !colors.is_empty() && !fits(colors.len()))
        {
            return Err("The ramp span must fit every palette within 256 colors".into());
        }
        let indexed = self.color_mode == ColorMode::Indexed;
        let transparent = self.transparent_index;
        let inside = indexed && low < transparent && transparent < high;
        if inside && steps < 3 {
            return Err(
                "Use at least 3 steps to keep both endpoints and the transparent swatch".into(),
            );
        }
        let map_len = self
            .frame_palettes
            .iter()
            .map(Vec::len)
            .chain([self.palette.len()])
            .max()
            .unwrap();
        let span = high - low;
        let proportional = |offset, width, last| (offset * last + width / 2) / width;
        let reserved =
            inside.then(|| proportional(transparent - low, span, steps - 1).clamp(1, steps - 2));
        let remap: Vec<_> = (0..map_len)
            .map(|slot| {
                if slot < low {
                    slot
                } else if slot > high {
                    slot - (span + 1) + steps
                } else if let Some(reserved) = reserved {
                    if slot == transparent {
                        low + reserved
                    } else {
                        let offset = slot - low - usize::from(slot > transparent);
                        let mapped = proportional(offset, span - 1, steps - 2);
                        low + mapped + usize::from(mapped >= reserved)
                    }
                } else {
                    let mapped = low + proportional(slot - low, span, steps - 1);
                    if indexed && transparent == low && slot != low {
                        mapped.max(low + 1)
                    } else if indexed && transparent == high && slot != high {
                        mapped.min(low + steps - 2)
                    } else {
                        mapped
                    }
                }
            })
            .collect();
        let reserved = reserved.map(|_| transparent);
        let mut ramps = vec![PaletteRamp::new(
            &self.palette,
            low,
            high,
            steps,
            &remap,
            reserved,
            interpolation,
        )];
        let mut variants = HashMap::from([(self.palette.as_slice(), 0)]);
        let frame_variant: Vec<_> = self
            .frame_palettes
            .iter()
            .map(|palette| {
                if palette.is_empty() {
                    0
                } else {
                    *variants.entry(palette.as_slice()).or_insert_with(|| {
                        let variant = ramps.len();
                        ramps.push(PaletteRamp::new(
                            palette,
                            low,
                            high,
                            steps,
                            &remap,
                            reserved,
                            interpolation,
                        ));
                        variant
                    })
                }
            })
            .collect();

        let mut next = self.clone();
        next.palette.clone_from(&ramps[0].colors);
        for (palette, &variant) in next.frame_palettes.iter_mut().zip(&frame_variant) {
            if !palette.is_empty() {
                palette.clone_from(&ramps[variant].colors);
            }
        }
        next.palette_names.resize(self.palette.len(), String::new());
        let mut names = vec![String::new(); steps];
        names[0] = next.palette_names[low].clone();
        names[steps - 1] = next.palette_names[high].clone();
        if let Some(slot) = reserved {
            names[remap[slot] - low] = next.palette_names[slot].clone();
        }
        next.palette_names.splice(low..=high, names);
        next.transparent_index = remap.get(transparent).copied().unwrap_or(transparent);
        next.drawing_index = self.drawing_index.and_then(|slot| remap.get(slot).copied());
        if indexed {
            next.remap_ramp_artwork(&ramps, &frame_variant, &remap, &self.palette, transparent);
        }
        next.validate()?;
        *self = next;
        Ok(remap)
    }

    fn remap_ramp_artwork(
        &mut self,
        ramps: &[PaletteRamp],
        frame_variant: &[usize],
        remap: &[usize],
        old_palette: &[Pixel],
        old_transparent: usize,
    ) {
        let mut shared = HashMap::new();
        for layer in &mut self.layers {
            for (frame, (pixels, meta)) in
                layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate()
            {
                let variant = frame_variant.get(frame).copied().unwrap_or(0);
                let ramp = &ramps[variant];
                for pixel in &mut meta.outside {
                    if let Some(old) = pixel.palette_slot.map(usize::from)
                        && ramp.replacements.get(old).is_some()
                    {
                        let slot = remap[old];
                        pixel.palette_slot = Some(slot as u8);
                        pixel.color = ramp.colors[slot];
                        if !layer.background && slot == self.transparent_index {
                            pixel.color[3] = 0;
                        }
                    } else {
                        pixel.palette_slot = None;
                        pixel.color = ramp.color(pixel.color);
                    }
                }
                let key = (
                    pixels.backing_id(),
                    meta.indices.as_ref().map(Arc::as_ptr),
                    variant,
                    layer.background,
                );
                if let Some((updated, indices)) = shared.get(&key) {
                    *pixels = Pixels::clone(updated);
                    meta.indices = Option::clone(indices);
                    continue;
                }
                if let Some(indices) = &mut meta.indices {
                    let before = Arc::clone(indices);
                    for (index, &old) in before.iter().enumerate() {
                        let old = old as usize;
                        let Some(replacement) = ramp.replacements.get(old) else {
                            continue;
                        };
                        if let Some(color) = replacement
                            && pixels[index] != *color
                        {
                            pixels.set(index, *color);
                        }
                        let new = remap[old] as u8;
                        if old as u8 != new {
                            Arc::make_mut(indices)[index] = new;
                        }
                    }
                } else {
                    let before = pixels.clone();
                    before.visit_nonclear(|index, old| {
                        let color = ramp.color(old);
                        if old != color {
                            pixels.set(index, color);
                        }
                    });
                }
                if key.0.is_some() || key.1.is_some() {
                    shared.insert(key, (pixels.clone(), meta.indices.clone()));
                }
            }
        }
        let mut shared_tiles = HashMap::new();
        for set in &mut self.tilesets {
            for tile in &mut set.tiles {
                let key = (
                    tile.backing_id(),
                    tile.indices.as_ref().map(Arc::as_ptr),
                    tile.len(),
                );
                if let Some((pixels, indices)) = shared_tiles.get(&key) {
                    tile.pixels = Pixels::clone(pixels);
                    tile.indices = Option::clone(indices);
                    continue;
                }
                let before = tile.clone();
                if let Some(indices) = &before.indices {
                    let mut mapped = Vec::with_capacity(indices.len());
                    for (index, &old) in indices.iter().enumerate() {
                        let slot = remap.get(old as usize).copied().unwrap_or(old as usize);
                        mapped.push(slot as u8);
                        let color = if before
                            .matching_slot(index, old_palette, old_transparent)
                            .is_some()
                        {
                            let mut color = ramps[0].colors[slot];
                            if slot == self.transparent_index {
                                color[3] = 0;
                            }
                            color
                        } else {
                            // Raw raster edits can invalidate authored metadata. Never
                            // restore an obsolete color from its former palette slot.
                            ramps[0].color(before[index])
                        };
                        if color != before[index] {
                            tile.pixels.set(index, color);
                        }
                    }
                    tile.indices = Some(if mapped == **indices {
                        Arc::clone(indices)
                    } else {
                        Arc::new(mapped)
                    });
                } else {
                    before.visit_nonclear(|index, old| {
                        let color = ramps[0].color(old);
                        if old != color {
                            tile.pixels.set(index, color);
                        }
                    });
                }
                shared_tiles.insert(key, (tile.pixels.clone(), tile.indices.clone()));
            }
        }
    }
}
