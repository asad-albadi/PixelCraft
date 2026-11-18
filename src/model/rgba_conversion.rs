use super::{CLEAR, ColorMode, Document, Pixel, Pixels};
use std::collections::HashMap;
use std::sync::Arc;

fn expand_slot(
    fallback: Pixel,
    slot: Option<u8>,
    palette: &[Pixel],
    transparent: usize,
    background: bool,
    opaque_background: bool,
) -> Result<Pixel, String> {
    let Some(slot) = slot.map(usize::from) else {
        return Ok(fallback);
    };
    let mut color = *palette
        .get(slot)
        .ok_or("Indexed image refers to a missing palette slot")?;
    if background && opaque_background {
        color[3] = 255;
    } else if !background && slot == transparent {
        color = CLEAR;
    }
    Ok(color)
}

impl Document {
    /// Resolve indexed artwork into RGB without changing the source document.
    pub fn rgba_conversion_until(&self, cancelled: impl Fn() -> bool) -> Result<Self, String> {
        self.rgba_conversion_with_tile_frame_until(0, cancelled)
    }

    /// Convert shared tiles with a chosen frame palette; raster cels keep their own.
    pub fn rgba_conversion_with_tile_frame_until(
        &self,
        tile_frame: usize,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, String> {
        self.resolve_indexed_conversion_until(ColorMode::Rgba, tile_frame, |pixel| pixel, cancelled)
    }

    pub(super) fn resolve_indexed_conversion_until(
        &self,
        target: ColorMode,
        tile_frame: usize,
        convert: impl Fn(Pixel) -> Pixel,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self, String> {
        self.validate()?;
        if tile_frame >= self.durations.len() {
            return Err("Choose a valid tile conversion frame".into());
        }
        let mut next = self.clone();
        let indexed = self.color_mode == ColorMode::Indexed;
        let mut variants = HashMap::new();
        let frames: Vec<_> = (0..self.durations.len())
            .map(|frame| {
                let next = variants.len();
                *variants.entry(self.palette_for(frame)).or_insert(next)
            })
            .collect();
        // Shared slots avoid hashing a full canvas on every linked frame.
        // Equal independently allocated slots can still reuse the converted buffer.
        let mut shared = HashMap::<_, Pixels>::new();
        let mut values = HashMap::<_, Pixels>::new();
        for (layer_index, layer) in next.layers.iter_mut().enumerate() {
            for (frame, (pixels, meta)) in
                layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate()
            {
                if cancelled() {
                    return Err("Color conversion cancelled".into());
                }
                let palette = self.palette_for(frame);
                if indexed && let Some(slots) = &self.layers[layer_index].cel_meta[frame].indices {
                    let key = (Arc::as_ptr(slots), frames[frame], layer.background);
                    if let Some(converted) = shared.get(&key) {
                        *pixels = converted.clone();
                    } else {
                        let value_key = (slots.as_slice(), frames[frame], layer.background);
                        if let Some(converted) = values.get(&value_key) {
                            *pixels = converted.clone();
                            shared.insert(key, converted.clone());
                        } else {
                            let mut converted = pixels.clone();
                            for index in 0..pixels.len() {
                                if index % 8192 == 0 && cancelled() {
                                    return Err("Color conversion cancelled".into());
                                }
                                let color = expand_slot(
                                    pixels[index],
                                    Some(slots[index]),
                                    palette,
                                    self.transparent_index,
                                    layer.background,
                                    target == ColorMode::Rgba,
                                )?;
                                let color = convert(color);
                                if color != pixels[index] {
                                    converted.set(index, color);
                                }
                            }
                            values.insert(value_key, converted.clone());
                            shared.insert(key, converted.clone());
                            *pixels = converted;
                        }
                    }
                }
                meta.indices = None;
                for (i, hidden) in meta.outside.iter_mut().enumerate() {
                    if i % 8192 == 0 && cancelled() {
                        return Err("Color conversion cancelled".into());
                    }
                    if indexed {
                        hidden.color = expand_slot(
                            hidden.color,
                            hidden.palette_slot,
                            palette,
                            self.transparent_index,
                            layer.background,
                            target == ColorMode::Rgba,
                        )?;
                        hidden.color = convert(hidden.color);
                    }
                    hidden.palette_slot = None;
                }
            }
        }
        let mut tile_shared = HashMap::<_, Pixels>::new();
        for set in &mut next.tilesets {
            for tile in &mut set.tiles {
                if indexed {
                    let slots = tile.indices.clone();
                    let key = (
                        tile.pixels.backing_id(),
                        slots.as_ref().map(Arc::as_ptr),
                        tile.len(),
                    );
                    if let Some(pixels) = tile_shared.get(&key) {
                        tile.pixels = pixels.clone();
                    } else {
                        let mut pixels = tile.pixels.clone();
                        for i in 0..pixels.len() {
                            if i % 8192 == 0 && cancelled() {
                                return Err("Color conversion cancelled".into());
                            }
                            let color = expand_slot(
                                pixels[i],
                                tile.matching_slot(i, &self.palette, self.transparent_index)
                                    .filter(|&slot| {
                                        self.palette_for(tile_frame).get(slot as usize).is_some()
                                    }),
                                self.palette_for(tile_frame),
                                self.transparent_index,
                                false,
                                false,
                            )?;
                            let color = convert(color);
                            if color != pixels[i] {
                                pixels.set(i, color);
                            }
                        }
                        tile_shared.insert(key, pixels.clone());
                        tile.pixels = pixels;
                    }
                }
                tile.indices = None;
            }
        }
        next.color_mode = target;
        next.drawing_index = None;
        next.validate()?;
        Ok(next)
    }
}
