use super::canvas_resize::ReferenceReframe;
use super::{resize_canvas, rotsprite};
use crate::model::{
    CLEAR, ColorMode, Document, MAX_DIMENSION, MAX_DOCUMENT_PIXELS, MAX_PIXELS, OutsidePixel,
    Pixel, Pixels, TilemapGeometry, nearest,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpriteResampling {
    #[default]
    Nearest,
    Bilinear,
    RotSprite,
}

/// Resample each image and tile with nearest-neighbor filtering.
pub fn resample_sprite(doc: &Document, size: [usize; 2]) -> Result<Document, String> {
    resample_sprite_with(doc, size, SpriteResampling::Nearest)
}

pub fn resample_sprite_with(
    doc: &Document,
    size: [usize; 2],
    method: SpriteResampling,
) -> Result<Document, String> {
    let [width, height] = size;
    if !valid_size(size) {
        return Err("Invalid sprite dimensions".into());
    }
    doc.validate()?;
    let old = [doc.width, doc.height];
    let mut result = doc.clone();
    if old == size {
        return Ok(result);
    }
    let scale = Scale { old, new: size };
    let mut rasters = Resampler {
        remaining: MAX_DOCUMENT_PIXELS,
        method,
        cache: HashMap::new(),
    };
    let mut indices = HashMap::new();
    let mut tile_indices = HashMap::new();
    let mut references = ReferenceReframe::new(doc, size);
    let tile_palette =
        (method == SpriteResampling::Bilinear && doc.color_mode == ColorMode::Indexed).then(|| {
            let mut palette = doc.palette.clone();
            if let Some(mask) = palette.get_mut(doc.transparent_index) {
                mask[3] = 0;
            }
            palette
        });
    for tileset in &mut result.tilesets {
        let old_tile = [tileset.width, tileset.height];
        let new_tile = scale.image_size(old_tile)?;
        if (new_tile[0] * new_tile[1]).saturating_mul(tileset.tiles.len()) > MAX_PIXELS {
            return Err("Resampled tileset exceeds the image limit".into());
        }
        for tile in &mut tileset.tiles {
            let tile_scale = Scale {
                old: old_tile,
                new: new_tile,
            };
            if let Some(palette) = &tile_palette {
                (tile.pixels, tile.indices) = rasters.indexed(
                    &tile.pixels,
                    tile.indices.as_deref().map(Vec::as_slice),
                    palette,
                    Some(doc.transparent_index),
                    tile_scale,
                )?;
            } else if method == SpriteResampling::RotSprite && tile.indices.is_some() {
                (tile.pixels, tile.indices) = rasters.rotsprite(
                    &tile.pixels,
                    tile.indices.as_deref().map(Vec::as_slice),
                    doc.transparent_index as u8,
                    tile_scale,
                )?;
            } else {
                tile.pixels = rasters.pixels(tile, tile_scale)?;
                if let Some(slots) = &mut tile.indices {
                    let key = (Arc::as_ptr(slots), old_tile, new_tile);
                    if let Some(cached) = tile_indices.get(&key) {
                        *slots = Arc::clone(cached);
                    } else {
                        rasters.charge((new_tile[0] * new_tile[1]).div_ceil(4))?;
                        let sampled = Arc::new(tile_scale.sample(slots));
                        tile_indices.insert(key, Arc::clone(&sampled));
                        *slots = sampled;
                    }
                }
            }
        }
        [tileset.width, tileset.height] = new_tile;
    }
    for (layer_index, layer) in result.layers.iter_mut().enumerate() {
        let reference = layer.is_ase_reference();
        if !reference {
            layer.replace_cel_image_ids();
            if method != SpriteResampling::Nearest {
                let mut palettes: HashMap<&[Pixel], usize> = HashMap::new();
                let groups: HashMap<_, _> = layer
                    .cel_meta
                    .iter()
                    .enumerate()
                    .filter(|(_, meta)| meta.artwork_link.is_some())
                    .map(|(frame, meta)| {
                        let next = palettes.len();
                        let palette_id = if doc.color_mode == ColorMode::Indexed {
                            *palettes.entry(doc.palette_for(frame)).or_insert(next)
                        } else {
                            0
                        };
                        (meta.editor_id.runtime_key(), (meta.bounds, palette_id))
                    })
                    .collect();
                layer.repartition_cel_links(|meta| {
                    groups.get(&meta.editor_id.runtime_key()).copied()
                });
            }
        }
        let full_bounds = [0, 0, doc.width as i32, doc.height as i32];
        if !references.preserve_mask(layer, &mut rasters.remaining)? {
            if let Some(mask) = &mut layer.mask
                && method != SpriteResampling::Nearest
                && !layer.mask_outside.is_empty()
            {
                let filtered = rasters.filter_window(
                    mask,
                    None,
                    &layer.mask_outside,
                    full_bounds,
                    scale,
                    None,
                )?;
                *mask = filtered.pixels;
                layer.set_mask_outside(filtered.outside);
            } else {
                if let Some(mask) = &mut layer.mask {
                    *mask = rasters.pixels(mask, scale)?;
                }
                if !layer.mask_outside.is_empty() {
                    layer.set_mask_outside(rasters.outside_pixels(&layer.mask_outside, scale)?);
                }
            }
        }
        for (frame, (pixels, meta)) in layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate() {
            if reference && let Some(bounds) = &mut meta.reference_bounds {
                *bounds = scale.reference_bounds(*bounds)?;
                if let Some(keyframe) = &mut meta.keyframe {
                    [keyframe.x, keyframe.y] = scale.position([keyframe.x, keyframe.y])?;
                }
                *pixels = references.cel(layer_index, frame, meta, &mut rasters.remaining)?;
                continue;
            }
            if method != SpriteResampling::Nearest
                && layer.tileset.is_none()
                && !meta.outside.is_empty()
            {
                let palette = (doc.color_mode == ColorMode::Indexed)
                    .then(|| doc.editing_palette(layer_index, frame));
                let transparent = (!layer.background).then_some(doc.transparent_index);
                let filtered = rasters.filter_window(
                    pixels,
                    meta.indices.as_deref().map(Vec::as_slice),
                    &meta.outside,
                    meta.bounds.unwrap_or(full_bounds),
                    scale,
                    palette.as_deref().map(|palette| (palette, transparent)),
                )?;
                *pixels = filtered.pixels;
                meta.indices = filtered.indices;
                meta.outside = filtered.outside;
                meta.bounds = Some(filtered.bounds);
                if let Some(keyframe) = &mut meta.keyframe {
                    [keyframe.x, keyframe.y] = scale.position([keyframe.x, keyframe.y])?;
                }
                continue;
            }
            if method == SpriteResampling::Bilinear
                && doc.color_mode == ColorMode::Indexed
                && layer.tileset.is_none()
            {
                let palette = doc.editing_palette(layer_index, frame);
                let transparent = (!layer.background).then_some(doc.transparent_index);
                (*pixels, meta.indices) = rasters.indexed(
                    pixels,
                    meta.indices.as_deref().map(Vec::as_slice),
                    &palette,
                    transparent,
                    scale,
                )?;
            } else if method == SpriteResampling::RotSprite && meta.indices.is_some() {
                (*pixels, meta.indices) = rasters.rotsprite(
                    pixels,
                    meta.indices.as_deref().map(Vec::as_slice),
                    doc.transparent_index as u8,
                    scale,
                )?;
            } else {
                *pixels = rasters.pixels(pixels, scale)?;
            }
            if method == SpriteResampling::Nearest
                && let Some(slots) = &mut meta.indices
            {
                let id = Arc::as_ptr(slots);
                if let Some(cached) = indices.get(&id) {
                    *slots = Arc::clone(cached);
                } else {
                    rasters.charge((width * height).div_ceil(4))?;
                    let resampled = Arc::new(scale.sample(slots));
                    indices.insert(id, Arc::clone(&resampled));
                    *slots = resampled;
                }
            }
            if !meta.outside.is_empty() {
                meta.outside = rasters.outside_pixels(&meta.outside, scale)?;
            }
            if let Some(bounds) = &mut meta.bounds {
                *bounds = scale.rect(*bounds)?;
                if method != SpriteResampling::Nearest && layer.tileset.is_none() {
                    let [x, y, w, h] = *bounds;
                    let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
                    pixels.visit_nonclear(|index, color| {
                        if color[3] > 0 {
                            let (x, y) = ((index % width) as i32, (index / width) as i32);
                            left = left.min(x);
                            top = top.min(y);
                            right = right.max(x + 1);
                            bottom = bottom.max(y + 1);
                        }
                    });
                    *bounds = [left, top, right - left, bottom - top];
                }
            }
            if let Some(keyframe) = &mut meta.keyframe {
                [keyframe.x, keyframe.y] = scale.position([keyframe.x, keyframe.y])?;
            }
            if let Some(tileset) = layer.tileset {
                let set = &doc.tilesets[tileset];
                let mut geometry = meta.tilemap.clone().unwrap_or(TilemapGeometry {
                    x: 0,
                    y: 0,
                    columns: doc.width.div_ceil(set.width),
                    rows: doc.height.div_ceil(set.height),
                });
                [geometry.x, geometry.y] = scale.position([geometry.x, geometry.y])?;
                rasters.charge(layer.tile_cels[frame].len())?;
                meta.tilemap = Some(geometry);
            }
        }
    }
    for slice in &mut result.slices {
        for key in &mut slice.keys {
            key.bounds = scale.rect(key.bounds)?;
            if let Some(center) = &mut key.center {
                *center = scale.rect(*center)?;
            }
            if let Some(pivot) = &mut key.pivot {
                *pivot = scale.position(*pivot)?;
            }
        }
    }
    for reference in &mut result.references {
        if reference.floating {
            continue;
        }
        reference.x *= width as f32 / doc.width as f32;
        reference.y *= height as f32 / doc.height as f32;
        let old = [reference.width, reference.height];
        let new = scale.image_size(old)?;
        reference.pixels = rasters.pixels(&reference.pixels, Scale { old, new })?;
        [reference.width, reference.height] = new;
    }
    [result.width, result.height] = size;
    result.validate()?;
    Ok(result)
}

/// Trim fully transparent outer rows and columns shared by all rendered frames.
pub fn trim_sprite(doc: &Document) -> Result<Document, String> {
    doc.validate()?;
    let [mut left, mut top, mut right, mut bottom] = [doc.width, doc.height, 0, 0];
    for frame in 0..doc.durations.len() {
        let pixels = doc.composite(frame);
        for (i, pixel) in pixels.iter().enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let (x, y) = (i % doc.width, i / doc.width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
        if [left, top, right, bottom] == [0, 0, doc.width, doc.height] {
            break;
        }
    }
    let mut trimmed = doc.clone();
    if left < right && top < bottom {
        resize_canvas(
            &mut trimmed,
            [right - left, bottom - top],
            [-(left as i32), -(top as i32)],
        )?;
    }
    Ok(trimmed)
}

fn valid_size([w, h]: [usize; 2]) -> bool {
    w > 0 && h > 0 && w <= MAX_DIMENSION && h <= MAX_DIMENSION && w.saturating_mul(h) <= MAX_PIXELS
}

#[derive(Clone, Copy)]
struct Scale {
    old: [usize; 2],
    new: [usize; 2],
}

impl Scale {
    fn reference_bounds(self, mut bounds: [i32; 4]) -> Result<[i32; 4], String> {
        for (index, value) in bounds.iter_mut().enumerate() {
            let axis = index % 2;
            let denominator = self.old[axis] as i64;
            let numerator = i64::from(*value) * self.new[axis] as i64;
            let scaled = (numerator + numerator.signum() * (denominator / 2)) / denominator;
            let scaled = if index >= 2 && *value > 0 {
                scaled.max(1)
            } else {
                scaled
            };
            *value = i32::try_from(scaled).map_err(|_| "ASE reference bounds are out of range")?;
        }
        Ok(bounds)
    }

    fn image_size(self, size: [usize; 2]) -> Result<[usize; 2], String> {
        let scaled =
            std::array::from_fn(|axis| (size[axis] * self.new[axis] / self.old[axis]).max(1));
        if !valid_size(scaled) {
            return Err("Resampled image exceeds canvas limits".into());
        }
        Ok(scaled)
    }

    // First target pixel whose nearest source coordinate is at or past this edge.
    fn edge(self, axis: usize, value: i64) -> i64 {
        let divisor = self.old[axis] as i64;
        let numerator = value * self.new[axis] as i64;
        -(-numerator).div_euclid(divisor)
    }

    fn rect(self, [x, y, w, h]: [i32; 4]) -> Result<[i32; 4], String> {
        let [x, y, w, h] = [x, y, w, h].map(i64::from);
        let left = self.edge(0, x);
        let top = self.edge(1, y);
        let right = self.edge(0, x + w);
        let bottom = self.edge(1, y + h);
        let narrow =
            |n| i32::try_from(n).map_err(|_| "Sprite resampling exceeds coordinate limits");
        Ok([
            narrow(left)?,
            narrow(top)?,
            narrow(right - left)?,
            narrow(bottom - top)?,
        ])
    }

    fn position(self, [x, y]: [i32; 2]) -> Result<[i32; 2], String> {
        let [x, y, _, _] = self.rect([x, y, 0, 0])?;
        Ok([x, y])
    }

    fn sample<T: Copy>(self, source: &[T]) -> Vec<T> {
        let [width, height] = self.new;
        let [old_w, old_h] = self.old;
        let mut result = Vec::with_capacity(width * height);
        for y in 0..height {
            let row = (y * old_h / height) * old_w;
            result.extend((0..width).map(|x| source[row + x * old_w / width]));
        }
        result
    }

    fn visit_samples(self, source: usize, mut visit: impl FnMut(usize)) {
        let x = (source % self.old[0]) as i64;
        let y = (source / self.old[0]) as i64;
        let left = self.edge(0, x) as usize;
        let right = self.edge(0, x + 1) as usize;
        for y in self.edge(1, y) as usize..self.edge(1, y + 1) as usize {
            let row = y * self.new[0];
            for x in left..right {
                visit(row + x);
            }
        }
    }
}

struct BilinearSample {
    indices: [usize; 4],
    weights: [u64; 4],
}

impl BilinearSample {
    fn new(scale: Scale, x: usize, y: usize) -> Self {
        let axis = |position: usize, axis: usize| {
            let denominator = scale.new[axis].saturating_sub(1).max(1);
            let numerator = position * (scale.old[axis] - 1);
            let first = numerator / denominator;
            let fraction = (numerator % denominator) as u64;
            (
                first,
                (first + 1).min(scale.old[axis] - 1),
                denominator as u64 - fraction,
                fraction,
            )
        };
        let (left, right, west, east) = axis(x, 0);
        let (top, bottom, north, south) = axis(y, 1);
        let stride = scale.old[0];
        Self {
            indices: [
                top * stride + left,
                top * stride + right,
                bottom * stride + left,
                bottom * stride + right,
            ],
            weights: [north * west, north * east, south * west, south * east],
        }
    }

    fn color(&self, source: &Pixels) -> Pixel {
        let mut channels = [0; 3];
        let mut alpha = 0;
        let total: u64 = self.weights.iter().sum();
        if let Some(i) = self.weights.iter().position(|&weight| weight == total) {
            return source[self.indices[i]];
        }
        for (index, weight) in self.indices.into_iter().zip(self.weights) {
            if weight == 0 {
                continue;
            }
            let pixel = source[index];
            let covered = weight * u64::from(pixel[3]);
            alpha += covered;
            for channel in 0..3 {
                channels[channel] += covered * u64::from(pixel[channel]);
            }
        }
        if alpha == 0 {
            return CLEAR;
        }
        let [red, green, blue] = channels.map(|value| ((value + alpha / 2) / alpha) as u8);
        [red, green, blue, ((alpha + total / 2) / total) as u8]
    }
}

type IndexedResample = (Pixels, Option<Arc<Vec<u8>>>);

struct FilteredWindow {
    pixels: Pixels,
    indices: Option<Arc<Vec<u8>>>,
    outside: Vec<OutsidePixel>,
    bounds: [i32; 4],
}

struct Resampler {
    remaining: usize,
    method: SpriteResampling,
    cache: HashMap<(usize, [usize; 2], [usize; 2]), Pixels>,
}

impl Resampler {
    fn charge(&mut self, pixels: usize) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(pixels)
            .ok_or("Sprite resampling exceeds the document memory limit")?;
        Ok(())
    }

    fn outside_pixels(
        &mut self,
        source: &[OutsidePixel],
        scale: Scale,
    ) -> Result<Vec<OutsidePixel>, String> {
        let mut outside = Vec::new();
        for pixel in source {
            if (0..scale.old[0] as i32).contains(&pixel.x)
                && (0..scale.old[1] as i32).contains(&pixel.y)
            {
                continue;
            }
            let [x, y, w, h] = scale.rect([pixel.x, pixel.y, 1, 1])?;
            if w == 0 || h == 0 {
                continue;
            }
            let last_x = i64::from(x) + i64::from(w) - 1;
            let last_y = i64::from(y) + i64::from(h) - 1;
            if x < -131_072 || y < -131_072 || last_x > 131_072 || last_y > 131_072 {
                return Err("Resampled off-canvas pixels exceed supported coordinates".into());
            }
            self.charge(
                (w as usize)
                    .saturating_mul(h as usize)
                    .saturating_mul(OutsidePixel::MEMORY_PIXELS),
            )?;
            for dy in 0..h {
                for dx in 0..w {
                    outside.push(OutsidePixel {
                        x: x + dx,
                        y: y + dy,
                        ..pixel.clone()
                    });
                }
            }
        }
        Ok(outside)
    }

    fn filter_window(
        &mut self,
        pixels: &Pixels,
        slots: Option<&[u8]>,
        hidden: &[OutsidePixel],
        [x, y, w, h]: [i32; 4],
        scale: Scale,
        indexed: Option<(&[Pixel], Option<usize>)>,
    ) -> Result<FilteredWindow, String> {
        let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
        let mut include = |x, y| {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        };
        let source_width = scale.old[0];
        pixels.visit_nonclear(|index, _| {
            include((index % source_width) as i32, (index / source_width) as i32)
        });
        for pixel in hidden {
            include(pixel.x, pixel.y);
        }

        let width = (right - left) as usize;
        let height = (bottom - top) as usize;
        let [x, y, w, h] = scale.rect([left, top, width as i32, height as i32])?;
        let area = width.saturating_mul(height);
        let target_area = (w as usize).saturating_mul(h as usize);
        if area > MAX_PIXELS || target_area > MAX_PIXELS || w > 65535 || h > 65535 {
            return Err("Resampled off-canvas cel exceeds the image size limit".into());
        }
        let mut output = FilteredWindow {
            pixels: Pixels::empty(scale.new[0] * scale.new[1]),
            indices: None,
            outside: Vec::new(),
            bounds: [x, y, w, h],
        };
        if target_area == 0 {
            return Ok(output);
        }

        let mask = indexed.and_then(|(_, mask)| mask).unwrap_or(0) as u8;
        let mut source = Pixels::empty(area);
        if indexed.is_some() {
            self.charge(area.div_ceil(4))?;
        }
        let mut source_slots = indexed.map(|_| vec![mask; area]);
        let mut put = |index, color: Pixel, slot: Option<u8>| {
            source.set(index, color);
            if let (Some(slots), Some((palette, transparent))) = (&mut source_slots, indexed) {
                slots[index] = slot.unwrap_or_else(|| {
                    if color[3] == 0
                        && let Some(transparent) = transparent
                    {
                        return transparent as u8;
                    }
                    nearest(color, palette) as u8
                });
            }
        };
        let inside = |x: i32, y: i32| {
            x >= 0 && y >= 0 && (x as usize) < scale.old[0] && (y as usize) < scale.old[1]
        };
        for py in top.max(0)..bottom.min(scale.old[1] as i32) {
            for px in left.max(0)..right.min(scale.old[0] as i32) {
                let src = py as usize * source_width + px as usize;
                let dst = (py - top) as usize * width + (px - left) as usize;
                put(dst, pixels[src], slots.map(|slots| slots[src]));
            }
        }
        for pixel in hidden {
            if !inside(pixel.x, pixel.y) {
                let dst = (pixel.y - top) as usize * width + (pixel.x - left) as usize;
                put(dst, pixel.color, pixel.palette_slot);
            }
        }
        self.charge(source.allocation_pixels())?;
        let window = Scale {
            old: [width, height],
            new: [w as usize, h as usize],
        };
        let (filtered, indices) = match (self.method, indexed) {
            (SpriteResampling::RotSprite, _) => {
                self.rotsprite(&source, source_slots.as_deref(), mask, window)?
            }
            (SpriteResampling::Bilinear, Some((palette, transparent))) => self.indexed(
                &source,
                source_slots.as_deref(),
                palette,
                transparent,
                window,
            )?,
            _ => (self.bilinear(&source, window)?, None),
        };

        let canvas_area = scale.new[0] * scale.new[1];
        if indices.is_some() {
            self.charge(canvas_area.div_ceil(4))?;
        }
        let mut canvas_slots = indices.as_ref().map(|_| vec![mask; canvas_area]);
        for index in 0..target_area {
            let px = i64::from(x) + (index % w as usize) as i64;
            let py = i64::from(y) + (index / w as usize) as i64;
            let color = filtered[index];
            let slot = indices.as_ref().map(|indices| indices[index]);
            if (0..scale.new[0] as i64).contains(&px) && (0..scale.new[1] as i64).contains(&py) {
                let dst = py as usize * scale.new[0] + px as usize;
                output.pixels.set(dst, color);
                if let (Some(slots), Some(slot)) = (&mut canvas_slots, slot) {
                    slots[dst] = slot;
                }
            } else if color != CLEAR || slot.is_some_and(|slot| slot != mask) {
                if !(-131_072..=131_072).contains(&px) || !(-131_072..=131_072).contains(&py) {
                    return Err("Resampled off-canvas pixels exceed supported coordinates".into());
                }
                self.charge(OutsidePixel::MEMORY_PIXELS)?;
                output.outside.push(OutsidePixel {
                    x: px as i32,
                    y: py as i32,
                    color,
                    palette_slot: slot,
                });
            }
        }
        self.charge(output.pixels.allocation_pixels())?;
        output.indices = canvas_slots.map(Arc::new);
        Ok(output)
    }

    fn bilinear(&mut self, source: &Pixels, scale: Scale) -> Result<Pixels, String> {
        let [width, height] = scale.new;
        self.charge(width * height)?;
        let mut output = Vec::with_capacity(width * height);
        for y in 0..height {
            output.extend((0..width).map(|x| BilinearSample::new(scale, x, y).color(source)));
        }
        Ok(output.into())
    }

    fn indexed(
        &mut self,
        source: &Pixels,
        slots: Option<&[u8]>,
        palette: &[Pixel],
        transparent: Option<usize>,
        scale: Scale,
    ) -> Result<IndexedResample, String> {
        let [width, height] = scale.new;
        let area = width * height;
        if source.backing_id().is_none() && slots.is_none() {
            return Ok((Pixels::empty(area), None));
        }
        if transparent.is_some_and(|slot| slot >= palette.len()) {
            return Err("Indexed resampling needs a transparent slot in the frame palette".into());
        }
        self.charge(area + area.div_ceil(4))?;
        let mut colors = Vec::with_capacity(area);
        let mut indices = Vec::with_capacity(area);
        let mut matches = HashMap::new();
        for y in 0..height {
            for x in 0..width {
                let sample = BilinearSample::new(scale, x, y);
                let color = sample.color(source);
                let retained = slots.and_then(|slots| {
                    sample
                        .indices
                        .into_iter()
                        .zip(sample.weights)
                        .filter(|(_, weight)| *weight != 0)
                        .max_by_key(|(_, weight)| *weight)
                        .map(|(i, _)| usize::from(slots[i]))
                        .filter(|&slot| palette.get(slot) == Some(&color))
                });
                let slot = retained
                    .or_else(|| (color[3] == 0).then_some(transparent).flatten())
                    .unwrap_or_else(|| {
                        if let Some(&slot) = matches.get(&color) {
                            return slot;
                        }
                        let slot = nearest(color, palette);
                        if matches.len() < 4096 {
                            matches.insert(color, slot);
                        }
                        slot
                    });
                colors.push(palette[slot]);
                indices.push(slot as u8);
            }
        }
        Ok((colors.into(), Some(Arc::new(indices))))
    }

    fn rotsprite(
        &mut self,
        source: &Pixels,
        source_slots: Option<&[u8]>,
        transparent: u8,
        scale: Scale,
    ) -> Result<IndexedResample, String> {
        let area = scale.new[0] * scale.new[1];
        self.charge(area + source_slots.map_or(0, |_| area.div_ceil(4)))?;
        let mut colors = vec![CLEAR; area];
        let mut slots = source_slots.map(|_| vec![transparent; area]);
        rotsprite::resample(
            scale.old,
            scale.new,
            |x, y| {
                let index = y * scale.old[0] + x;
                Some((source[index], source_slots.map(|indices| indices[index])))
            },
            |index, (color, slot)| {
                colors[index] = color;
                if let (Some(slots), Some(slot)) = (&mut slots, slot) {
                    slots[index] = slot;
                }
            },
        )?;
        Ok((colors.into(), slots.map(Arc::new)))
    }

    fn pixels(&mut self, source: &Pixels, scale: Scale) -> Result<Pixels, String> {
        if self.method == SpriteResampling::RotSprite && scale.old == scale.new {
            return Ok(source.clone());
        }
        let [w, h] = scale.new;
        let area = w * h;
        let Some(id) = source.backing_id() else {
            return Ok(Pixels::empty(area));
        };
        let key = (id, scale.old, scale.new);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }
        let resampled = if self.method == SpriteResampling::RotSprite {
            self.rotsprite(source, None, 0, scale)?.0
        } else if self.method == SpriteResampling::Bilinear {
            self.bilinear(source, scale)?
        } else if let Some(base) = source.dense_base_pixels() {
            self.charge(area)?;
            let mut output = scale.sample(base);
            source.visit_dense_edits(|index, pixel| {
                scale.visit_samples(index, |target| output[target] = pixel);
            });
            output.into()
        } else {
            let mut output = Pixels::empty(area);
            source.visit_nonclear(|i, color| {
                scale.visit_samples(i, |target| output.set(target, color));
            });
            self.charge(output.allocation_pixels())?;
            output
        };
        self.cache.insert(key, resampled.clone());
        Ok(resampled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CLEAR, Tileset};

    #[test]
    fn resample_scales_every_cel_and_keeps_tiles_editable() {
        let mut doc = Document::new(2, 2);
        let red = [231, 10, 12, 255];
        let green = [12, 201, 15, 255];
        doc.add_frame(0, false);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Test".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![red, green].into()],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[1][0] = 0x8000_0001;
        doc.set_pixel(0, 0, 2, green);

        let resized = resample_sprite(&doc, [4, 4]).unwrap();
        assert_eq!((resized.width, resized.height), (4, 4));
        assert_eq!(
            (resized.tilesets[0].width, resized.tilesets[0].height),
            (4, 2)
        );
        assert_eq!(resized.layers[1].tile_cels[1][0], 0x8000_0001);
        assert_eq!(
            &resized.composite(1)[..8],
            &[green, green, red, red, green, green, red, red]
        );
        assert_eq!(&resized.composite(0)[8..12], &[green, green, CLEAR, CLEAR]);
        assert!(resized.tile_at_point(1, 1, (3, 3)).is_some());
    }

    #[test]
    fn trim_uses_visible_bounds_across_the_animation() {
        let mut doc = Document::new(5, 5);
        let ink = [231, 10, 12, 255];
        doc.add_frame(0, false);
        doc.add_layer();
        doc.layers[1].visible = false;
        doc.set_pixel(1, 0, 0, ink);
        doc.set_pixel(0, 0, 2 * 5 + 1, ink);
        doc.set_pixel(0, 1, 3 * 5 + 3, ink);

        let trimmed = trim_sprite(&doc).unwrap();
        assert_eq!([trimmed.width, trimmed.height], [3, 2]);
        assert_eq!(trimmed.composite(0)[0], ink);
        assert_eq!(trimmed.composite(1)[5], ink);
        assert_eq!(trimmed.durations, doc.durations);
        assert_eq!(trimmed.layers.len(), 2);
    }
}
