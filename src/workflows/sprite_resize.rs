use super::resize_canvas;
use crate::model::{
    Document, MAX_DIMENSION, MAX_DOCUMENT_PIXELS, MAX_PIXELS, OutsidePixel, Pixels, TilemapGeometry,
};
use std::{collections::HashMap, sync::Arc};

/// Resample each image and tile with nearest-neighbor filtering.
pub fn resample_sprite(doc: &Document, size: [usize; 2]) -> Result<Document, String> {
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
        cache: HashMap::new(),
    };
    let mut indices = HashMap::new();
    for tileset in &mut result.tilesets {
        let old_tile = [tileset.width, tileset.height];
        let new_tile = scale.image_size(old_tile)?;
        if (new_tile[0] * new_tile[1]).saturating_mul(tileset.tiles.len()) > MAX_PIXELS {
            return Err("Resampled tileset exceeds the image limit".into());
        }
        for tile in &mut tileset.tiles {
            *tile = rasters.pixels(
                tile,
                Scale {
                    old: old_tile,
                    new: new_tile,
                },
            )?;
        }
        [tileset.width, tileset.height] = new_tile;
    }
    for layer in &mut result.layers {
        if let Some(mask) = &mut layer.mask {
            *mask = rasters.pixels(mask, scale)?;
        }
        for (frame, (pixels, meta)) in layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate() {
            *pixels = rasters.pixels(pixels, scale)?;
            if let Some(slots) = &mut meta.indices {
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
                let mut outside = Vec::new();
                for pixel in &meta.outside {
                    if doc.index(pixel.x, pixel.y).is_some() {
                        continue;
                    }
                    let [x, y, w, h] = scale.rect([pixel.x, pixel.y, 1, 1])?;
                    if w == 0 || h == 0 {
                        continue;
                    }
                    let last_x = i64::from(x) + i64::from(w) - 1;
                    let last_y = i64::from(y) + i64::from(h) - 1;
                    if x < -131_072 || y < -131_072 || last_x > 131_072 || last_y > 131_072 {
                        return Err(
                            "Resampled off-canvas pixels exceed supported coordinates".into()
                        );
                    }
                    rasters.charge((w as usize).saturating_mul(h as usize).saturating_mul(3))?;
                    for dy in 0..h {
                        for dx in 0..w {
                            outside.push(OutsidePixel {
                                x: x + dx,
                                y: y + dy,
                                color: pixel.color,
                            });
                        }
                    }
                }
                meta.outside = outside;
            }
            if let Some(bounds) = &mut meta.bounds {
                *bounds = scale.rect(*bounds)?;
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
}

struct Resampler {
    remaining: usize,
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

    fn pixels(&mut self, source: &Pixels, scale: Scale) -> Result<Pixels, String> {
        let [w, h] = scale.new;
        let area = w * h;
        let Some(id) = source.backing_id() else {
            return Ok(Pixels::empty(area));
        };
        let key = (id, scale.old, scale.new);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }
        let resampled = if let Pixels::Dense(pixels) = source {
            self.charge(area)?;
            Pixels::from(scale.sample(pixels))
        } else {
            let mut output = Pixels::empty(area);
            source.visit_nonclear(|i, color| {
                let (x, y) = (i % scale.old[0], i / scale.old[0]);
                let (left, right) = (
                    scale.edge(0, x as i64) as usize,
                    scale.edge(0, x as i64 + 1) as usize,
                );
                let (top, bottom) = (
                    scale.edge(1, y as i64) as usize,
                    scale.edge(1, y as i64 + 1) as usize,
                );
                for target_y in top..bottom {
                    for target_x in left..right {
                        output.set(target_y * w + target_x, color);
                    }
                }
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
