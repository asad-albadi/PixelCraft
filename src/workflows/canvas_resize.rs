use crate::model::{
    CLEAR, Document, MAX_DIMENSION, MAX_DOCUMENT_PIXELS, MAX_PIXELS, Pixels, TilemapGeometry,
    Tileset, nearest,
};
use std::{collections::HashMap, ops::Range, sync::Arc};

/// Change the viewport and move artwork by a pixel offset. Pixels and tiles
/// wholly outside the new canvas are cropped. Tile grids retain their phase.
pub fn resize_canvas(
    doc: &mut Document,
    [width, height]: [usize; 2],
    offset: [i32; 2],
) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("Invalid canvas dimensions".into());
    }
    if [width, height] == [doc.width, doc.height] && offset == [0, 0] {
        return Ok(());
    }

    doc.validate()?;
    let mapping = Mapping {
        width,
        height,
        old_width: doc.width,
        old_height: doc.height,
        offset: offset.map(i64::from),
    };
    let mut result = doc.clone();
    let mut rasters = HashMap::new();
    let mut indices = HashMap::new();
    let mut budget = MAX_DOCUMENT_PIXELS;

    for (layer_index, layer) in result.layers.iter_mut().enumerate() {
        layer.mask = layer
            .mask
            .as_ref()
            .map(|mask| mapping.raster(mask, &mut rasters, &mut budget))
            .transpose()?;
        let tileset = layer.tileset.map(|index| &doc.tilesets[index]);
        let mut links = HashMap::new();
        let mut link_counts = Vec::new();
        for (frame, meta) in layer.cel_meta.iter_mut().enumerate() {
            let bounds = meta
                .bounds
                .unwrap_or([0, 0, doc.width as i32, doc.height as i32]);
            if let Some(link) = meta.artwork_link {
                let group = *links
                    .entry((link, mapping.clip_key(bounds)))
                    .or_insert_with(|| {
                        let group = link_counts.len();
                        link_counts.push(0);
                        group
                    });
                link_counts[group] += 1;
                meta.artwork_link = Some(group);
            }
            let mut pixels = mapping.raster(&layer.cels[frame], &mut rasters, &mut budget)?;
            let remapped_indices = if let Some(source) = &meta.indices {
                let id = Arc::as_ptr(source);
                if let Some(cached) = indices.get(&id) {
                    Some(Arc::clone(cached))
                } else {
                    charge(&mut budget, (width * height).div_ceil(4))?;
                    let mut output = vec![doc.transparent_index as u8; width * height];
                    mapping.copy_rows(source, &mut output);
                    let output = Arc::new(output);
                    indices.insert(id, Arc::clone(&output));
                    Some(output)
                }
            } else {
                None
            };
            meta.indices = remapped_indices;
            let mut palette = None;
            let mut exposed = false;
            for outside in &meta.outside {
                // On-canvas data always wins over a stale duplicate stored outside.
                if doc.index(outside.x, outside.y).is_some() {
                    continue;
                }
                if let Some(index) = mapping.index(i64::from(outside.x), i64::from(outside.y)) {
                    if let Some(slots) = &mut meta.indices {
                        if !exposed {
                            charge(&mut budget, (width * height).div_ceil(4))?;
                        }
                        let slot = if outside.color[3] == 0 && !layer.background {
                            doc.transparent_index
                        } else {
                            nearest(
                                outside.color,
                                palette
                                    .get_or_insert_with(|| doc.editing_palette(layer_index, frame)),
                            )
                        };
                        Arc::make_mut(slots)[index] = slot as u8;
                    }
                    pixels.set(index, outside.color);
                    exposed = true;
                }
            }
            if exposed {
                charge(&mut budget, pixels.allocation_pixels())?;
            }
            layer.cels[frame] = pixels;
            meta.outside.clear();
            meta.bounds = meta.bounds.map(|bounds| mapping.clipped_bounds(bounds));
            if let Some(tileset) = tileset {
                let old = meta.tilemap.clone().unwrap_or(TilemapGeometry {
                    x: 0,
                    y: 0,
                    columns: doc.width.div_ceil(tileset.width),
                    rows: doc.height.div_ceil(tileset.height),
                });
                let (geometry, cells) =
                    mapping.tiles(tileset, old, &layer.tile_cels[frame], &mut budget)?;
                meta.tilemap = Some(geometry);
                layer.tile_cels[frame] = cells;
            }
        }
        for meta in &mut layer.cel_meta {
            if meta
                .artwork_link
                .is_some_and(|group| link_counts[group] < 2)
            {
                meta.artwork_link = None;
            }
        }
    }
    for reference in &mut result.references {
        if !reference.floating {
            reference.x += offset[0] as f32;
            reference.y += offset[1] as f32;
        }
    }
    for slice in &mut result.slices {
        for key in &mut slice.keys {
            key.bounds[0] = key.bounds[0]
                .checked_add(offset[0])
                .ok_or("Slice offset is out of range")?;
            key.bounds[1] = key.bounds[1]
                .checked_add(offset[1])
                .ok_or("Slice offset is out of range")?;
        }
    }
    result.width = width;
    result.height = height;
    result.validate()?;
    *doc = result;
    Ok(())
}

struct Mapping {
    width: usize,
    height: usize,
    old_width: usize,
    old_height: usize,
    offset: [i64; 2],
}

impl Mapping {
    fn index(&self, x: i64, y: i64) -> Option<usize> {
        let [dx, dy] = self.offset;
        let (x, y) = (x + dx, y + dy);
        if (0..self.width as i64).contains(&x) && (0..self.height as i64).contains(&y) {
            Some(y as usize * self.width + x as usize)
        } else {
            None
        }
    }

    fn copy_rows<T: Copy>(&self, source: &[T], target: &mut [T]) {
        copy_grid(
            source,
            target,
            [self.old_width, self.old_height],
            [self.width, self.height],
            self.offset,
        );
    }

    fn raster(
        &self,
        source: &Pixels,
        cache: &mut HashMap<usize, Pixels>,
        budget: &mut usize,
    ) -> Result<Pixels, String> {
        let area = self.width * self.height;
        let Some(id) = source.backing_id() else {
            return Ok(Pixels::empty(area));
        };
        if let Some(raster) = cache.get(&id) {
            return Ok(raster.clone());
        }
        let output = match source {
            Pixels::Dense(pixels) => {
                charge(budget, area)?;
                let mut output = vec![CLEAR; area];
                self.copy_rows(pixels, &mut output);
                output.into()
            }
            _ => {
                let mut output = Pixels::empty(area);
                source.visit_nonclear(|index, color| {
                    if let Some(target) = self.index(
                        (index % self.old_width) as i64,
                        (index / self.old_width) as i64,
                    ) {
                        output.set(target, color);
                    }
                });
                charge(budget, output.allocation_pixels())?;
                output
            }
        };
        cache.insert(id, output.clone());
        Ok(output)
    }

    fn clip_key(&self, bounds: [i32; 4]) -> [i64; 4] {
        let [x, y, w, h] = bounds.map(i64::from);
        let [dx, dy] = self.offset;
        let x = x + dx;
        let y = y + dy;
        [
            (-x).clamp(0, w),
            (-y).clamp(0, h),
            (self.width as i64 - x).clamp(0, w),
            (self.height as i64 - y).clamp(0, h),
        ]
    }

    fn clipped_bounds(&self, bounds: [i32; 4]) -> [i32; 4] {
        let [left, top, right, bottom] = self.clip_key(bounds);
        if left >= right || top >= bottom {
            return [0, 0, 0, 0];
        }
        [
            (i64::from(bounds[0]) + self.offset[0] + left) as i32,
            (i64::from(bounds[1]) + self.offset[1] + top) as i32,
            (right - left) as i32,
            (bottom - top) as i32,
        ]
    }

    fn tiles(
        &self,
        tileset: &Tileset,
        old: TilemapGeometry,
        cells: &[u32],
        budget: &mut usize,
    ) -> Result<(TilemapGeometry, Vec<u32>), String> {
        let shifted = [
            i64::from(old.x) + self.offset[0],
            i64::from(old.y) + self.offset[1],
        ];
        let tile_size = [tileset.width as i64, tileset.height as i64];
        // Put the grid origin at or just before the canvas edge so every
        // canvas position has an editable cell even with sub-tile offsets.
        let origin: [i64; 2] =
            std::array::from_fn(|axis| -(-shifted[axis]).rem_euclid(tile_size[axis]));
        let columns = (self.width + (-origin[0]) as usize).div_ceil(tileset.width);
        let rows = (self.height + (-origin[1]) as usize).div_ceil(tileset.height);
        let count = columns * rows;
        if count > MAX_PIXELS {
            return Err("Resized tile grid exceeds the cell limit".into());
        }
        charge(budget, count)?;
        let delta = std::array::from_fn(|axis| (shifted[axis] - origin[axis]) / tile_size[axis]);
        let mut resized = vec![0; count];
        copy_grid(
            cells,
            &mut resized,
            [old.columns, old.rows],
            [columns, rows],
            delta,
        );
        Ok((
            TilemapGeometry {
                x: origin[0] as i32,
                y: origin[1] as i32,
                columns,
                rows,
            },
            resized,
        ))
    }
}

fn charge(budget: &mut usize, pixels: usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(pixels)
        .ok_or("Canvas resize exceeds the document memory limit")?;
    Ok(())
}

fn copy_grid<T: Copy>(
    source: &[T],
    target: &mut [T],
    [old_width, old_height]: [usize; 2],
    [new_width, new_height]: [usize; 2],
    [dx, dy]: [i64; 2],
) {
    fn source_overlap(old: usize, new: usize, delta: i64) -> Range<usize> {
        let old = old as i64;
        (-delta).clamp(0, old) as usize..(new as i64 - delta).clamp(0, old) as usize
    }

    let cols = source_overlap(old_width, new_width, dx);
    if cols.is_empty() {
        return;
    }
    let dest_col = (cols.start as i64 + dx) as usize;
    for row in source_overlap(old_height, new_height, dy) {
        let old = row * old_width + cols.start;
        let new = (row as i64 + dy) as usize * new_width + dest_col;
        target[new..new + cols.len()].copy_from_slice(&source[old..old + cols.len()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CLEAR, ColorMode, Tileset};

    #[test]
    fn resize_translates_all_frames_and_preserves_exact_indexed_slots() {
        let mut doc = Document::new(3, 2);
        let ink = [211, 21, 55, 255];
        doc.palette = vec![ink, ink, CLEAR];
        doc.transparent_index = 2;
        doc.set_color_mode(ColorMode::Indexed);
        doc.add_frame(0, false);
        doc.durations[1] = 119;
        doc.set_frame_name(1, "Finish").unwrap();
        doc.layers[0].cel_meta[0].indices = Some(vec![1, 2, 2, 2, 0, 2].into());
        doc.layers[0].cel_meta[1].indices = Some(vec![2, 2, 0, 2, 2, 1].into());
        doc.layers[0].cels[0].set(0, ink);
        doc.layers[0].cels[0].set(4, ink);
        doc.layers[0].cels[1].set(2, ink);
        doc.layers[0].cels[1].set(5, ink);
        let frame_id = doc.frame_export_id(1);

        resize_canvas(&mut doc, [4, 3], [1, 1]).unwrap();

        assert_eq!((doc.width, doc.height), (4, 3));
        let indices = &doc.layers[0].cel_meta;
        assert_eq!(
            indices[0].indices.as_deref().unwrap(),
            &[2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 0, 2]
        );
        assert_eq!(
            indices[1].indices.as_deref().unwrap(),
            &[2, 2, 2, 2, 2, 2, 2, 0, 2, 2, 2, 1]
        );
        assert_eq!(doc.frame_name(1), Some("Finish"));
        assert_eq!(doc.frame_export_id(1), frame_id);
        assert_eq!(doc.durations[1], 119);
    }

    #[test]
    fn resize_reveals_off_canvas_pixels_and_translates_slice_and_mask_coordinates() {
        use crate::model::{OutsidePixel, SliceKey, SpriteSlice};

        let mut doc = Document::new(2, 2);
        let ink = [250, 90, 37, 255];
        let layer = &mut doc.layers[0];
        layer.cel_meta[0].bounds = Some([-1, 0, 3, 2]);
        layer.cel_meta[0].outside.push(OutsidePixel {
            x: -1,
            y: 1,
            color: ink,
        });
        let mut mask = Pixels::empty(4);
        mask.set(0, [255; 4]);
        layer.mask = Some(mask);
        doc.slices.push(SpriteSlice {
            editor_id: Default::default(),
            name: "Pivot".into(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: [0, 0, 2, 2],
                center: None,
                pivot: Some([1, 1]),
            }],
        });
        let original = doc.clone();
        assert!(resize_canvas(&mut doc, [MAX_DIMENSION, MAX_DIMENSION], [0, 0]).is_err());
        assert_eq!(doc, original);

        resize_canvas(&mut doc, [4, 3], [1, 1]).unwrap();

        assert_eq!(doc.layers[0].cels[0][8], ink);
        assert_eq!(doc.layers[0].mask.as_ref().unwrap()[5], [255; 4]);
        assert!(doc.layers[0].cel_meta[0].outside.is_empty());
        assert_eq!(doc.layers[0].cel_meta[0].bounds, Some([0, 1, 3, 2]));
        assert_eq!(doc.slices[0].keys[0].bounds, [1, 1, 2, 2]);
        assert_eq!(doc.slices[0].keys[0].pivot, Some([1, 1]));
    }

    #[test]
    fn resize_keeps_tile_grid_phase_and_flips_and_grows_editable_coverage() {
        let mut doc = Document::new(4, 2);
        let red = [255, 0, 0, 255];
        let green = [0, 255, 0, 255];
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Test".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![red, green].into()],
        });
        doc.add_frame(0, false);
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[0][0] = 0x8000_0001;
        doc.layers[1].tile_cels[1][1] = 1;

        resize_canvas(&mut doc, [6, 3], [1, 1]).unwrap();

        assert_eq!(
            &doc.composite(0)[6..12],
            &[CLEAR, green, red, CLEAR, CLEAR, CLEAR]
        );
        assert_eq!(
            &doc.composite(1)[6..12],
            &[CLEAR, CLEAR, CLEAR, red, green, CLEAR]
        );
        let (geometry, _, _) = doc.tile_at_point(1, 0, (5, 2)).unwrap();
        assert_eq!((geometry.x, geometry.columns, geometry.rows), (-1, 4, 3));
        assert!(doc.layers[1].tile_cels[0].contains(&0x8000_0001));

        resize_canvas(&mut doc, [3, 2], [-2, -1]).unwrap();
        assert_eq!(&doc.composite(0)[..3], &[red, CLEAR, CLEAR]);
        assert_eq!(&doc.composite(1)[..3], &[CLEAR, red, green]);
        assert!(doc.tile_at_point(1, 0, (2, 1)).is_some());
    }
}
