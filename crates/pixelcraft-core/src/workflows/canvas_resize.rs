use crate::model::{
    CLEAR, CelMeta, ColorMode, Document, Layer, MAX_DIMENSION, MAX_DOCUMENT_PIXELS, MAX_PIXELS,
    OutsidePixel, Pixels, TilemapGeometry, Tileset, nearest,
};
use std::{collections::HashMap, ops::Range, sync::Arc};

/// Change the viewport and move artwork by a pixel offset. Pixels and tiles
/// wholly outside the new canvas are cropped. Tile grids retain their phase.
pub fn resize_canvas(doc: &mut Document, size: [usize; 2], offset: [i32; 2]) -> Result<(), String> {
    resize_canvas_with_trim(doc, size, offset, true)
}

/// Change the canvas, optionally retaining artwork beyond the viewport.
pub fn resize_canvas_with_trim(
    doc: &mut Document,
    [width, height]: [usize; 2],
    offset: [i32; 2],
    trim: bool,
) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.saturating_mul(height) > MAX_PIXELS
    {
        return Err("Invalid canvas dimensions".into());
    }
    let same_geometry = [width, height] == [doc.width, doc.height] && offset == [0, 0];
    if same_geometry && !trim {
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
    let mut references = ReferenceReframe::new(doc, [width, height]);

    for (layer_index, layer) in result.layers.iter_mut().enumerate() {
        let tileset = layer.tileset.map(|index| &doc.tilesets[index]);
        let reference = layer.is_ase_reference();
        if same_geometry
            && layer.mask_outside.is_empty()
            && !layer.cel_meta.iter().any(|meta| {
                if reference && meta.reference_bounds.is_some() {
                    return false;
                }
                !meta.outside.is_empty()
                    || meta
                        .bounds
                        .is_some_and(|bounds| mapping.clipped_bounds(bounds) != bounds)
                    || tileset
                        .zip(meta.tilemap.as_ref())
                        .is_some_and(|(set, grid)| mapping.has_external_tiles(set, grid))
            })
        {
            continue;
        }
        if !references.preserve_mask(layer, &mut budget)? {
            mapping.resize_mask(layer, trim, &mut rasters, &mut budget)?;
        }
        if let Some(tileset) = tileset {
            // Each placement is reframed against the canvas tile grid. Images
            // with different source origins or dimensions can produce distinct grids.
            let default = (
                0,
                0,
                doc.width.div_ceil(tileset.width),
                doc.height.div_ceil(tileset.height),
            );
            layer.repartition_cel_links(|meta| {
                meta.tilemap
                    .as_ref()
                    .map_or(default, |grid| (grid.x, grid.y, grid.columns, grid.rows))
            });
        } else if trim {
            layer.repartition_cel_links(|meta| {
                (!reference || meta.reference_bounds.is_none()).then(|| {
                    let bounds = meta
                        .bounds
                        .unwrap_or([0, 0, doc.width as i32, doc.height as i32]);
                    mapping.clip_key(bounds)
                })
            });
        }
        for (frame, meta) in layer.cel_meta.iter_mut().enumerate() {
            if reference && let Some(bounds) = &mut meta.reference_bounds {
                shift_reference_bounds(bounds, offset)?;
                layer.cels[frame] = references.cel(layer_index, frame, meta, &mut budget)?;
                continue;
            }
            let original = &layer.cels[frame];
            let mut hidden = Vec::new();
            if !trim && tileset.is_none() {
                mapping.retain_clipped(
                    original,
                    meta.indices.as_deref().map(Vec::as_slice),
                    doc.transparent_index as u8,
                    &mut hidden,
                    &mut budget,
                )?;
            }
            let mut pixels = mapping.raster(original, &mut rasters, &mut budget)?;
            if !same_geometry && let Some(source) = &mut meta.indices {
                let id = Arc::as_ptr(source);
                if let Some(cached) = indices.get(&id) {
                    *source = Arc::clone(cached);
                } else {
                    charge(&mut budget, (width * height).div_ceil(4))?;
                    let mut output = vec![doc.transparent_index as u8; width * height];
                    mapping.copy_rows(source, &mut output);
                    *source = Arc::new(output);
                    indices.insert(id, Arc::clone(source));
                }
            }
            let mut reveal = RevealPixels::new(doc, layer_index, frame);
            for mut outside in std::mem::take(&mut meta.outside) {
                // On-canvas data always wins over a stale duplicate stored outside.
                if doc.index(outside.x, outside.y).is_some() {
                    continue;
                }
                if let Some(index) = mapping.index(i64::from(outside.x), i64::from(outside.y)) {
                    reveal.write(&mut pixels, &mut meta.indices, index, &outside, &mut budget)?;
                } else if !trim {
                    [outside.x, outside.y] =
                        mapping.outside_position(outside.x as i64, outside.y as i64)?;
                    charge(&mut budget, OutsidePixel::MEMORY_PIXELS)?;
                    hidden.push(outside);
                }
            }
            if reveal.wrote {
                charge(&mut budget, pixels.allocation_pixels())?;
            }
            layer.cels[frame] = pixels;
            meta.outside = hidden;
            if trim {
                meta.bounds = meta.bounds.map(|bounds| mapping.clipped_bounds(bounds));
            } else if tileset.is_none() && (meta.bounds.is_some() || !meta.outside.is_empty()) {
                let mut bounds = meta
                    .bounds
                    .unwrap_or([0, 0, doc.width as i32, doc.height as i32]);
                [bounds[0], bounds[1]] =
                    mapping.outside_position(bounds[0] as i64, bounds[1] as i64)?;
                meta.bounds = Some(bounds);
            }
            if let Some(tileset) = tileset {
                let old = meta.tilemap.clone().unwrap_or(TilemapGeometry {
                    x: 0,
                    y: 0,
                    columns: doc.width.div_ceil(tileset.width),
                    rows: doc.height.div_ceil(tileset.height),
                });
                if same_geometry && !mapping.has_external_tiles(tileset, &old) {
                    continue;
                }
                let (geometry, cells) =
                    mapping.tiles(tileset, old, &layer.tile_cels[frame], trim, &mut budget)?;
                meta.tilemap = Some(geometry);
                layer.tile_cels[frame] = cells.into();
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

pub(super) const ASE_COORDINATE_UNIT: i64 = 65_536;

pub(super) fn shift_reference_bounds(
    bounds: &mut [i32; 4],
    offset: [i32; 2],
) -> Result<(), String> {
    for axis in 0..2 {
        let shifted = i64::from(bounds[axis]) + i64::from(offset[axis]) * ASE_COORDINATE_UNIT;
        bounds[axis] =
            i32::try_from(shifted).map_err(|_| "ASE reference position is out of range")?;
    }
    Ok(())
}

/// Keep an imported reference's source coordinates and pixels when the
/// canvas dimensions change. Only its displayed rectangle moves or scales.
pub(super) struct ReferenceReframe<'a> {
    source: &'a Document,
    mapping: Mapping,
    rasters: HashMap<usize, Pixels>,
    indices: HashMap<*const Vec<u8>, Arc<Vec<u8>>>,
}

impl<'a> ReferenceReframe<'a> {
    pub(super) fn new(source: &'a Document, [width, height]: [usize; 2]) -> Self {
        Self {
            source,
            mapping: Mapping {
                width,
                height,
                old_width: source.width,
                old_height: source.height,
                offset: [0, 0],
            },
            rasters: HashMap::new(),
            indices: HashMap::new(),
        }
    }

    pub(super) fn preserve_mask(
        &mut self,
        layer: &mut Layer,
        budget: &mut usize,
    ) -> Result<bool, String> {
        if !layer.is_ase_reference()
            || !layer
                .cel_meta
                .iter()
                .any(|meta| meta.reference_bounds.is_some())
        {
            return Ok(false);
        }
        if (self.mapping.width, self.mapping.height)
            != (self.mapping.old_width, self.mapping.old_height)
        {
            self.mapping
                .resize_mask(layer, false, &mut self.rasters, budget)?;
        }
        Ok(true)
    }

    pub(super) fn cel(
        &mut self,
        layer: usize,
        frame: usize,
        meta: &mut CelMeta,
        budget: &mut usize,
    ) -> Result<Pixels, String> {
        let raster = &self.source.layers[layer].cels[frame];
        let m = &self.mapping;
        if (m.width, m.height) == (m.old_width, m.old_height) {
            return Ok(raster.clone());
        }
        let mut pixels = m.raster(raster, &mut self.rasters, budget)?;
        if let Some(source) = &mut meta.indices {
            let id = Arc::as_ptr(source);
            if let Some(cached) = self.indices.get(&id) {
                *source = cached.clone();
            } else {
                let area = m.width * m.height;
                charge(budget, area.div_ceil(4))?;
                let mut output = vec![self.source.transparent_index as u8; area];
                m.copy_rows(source, &mut output);
                *source = Arc::new(output);
                self.indices.insert(id, source.clone());
            }
        }
        let mut outside = Vec::new();
        let mut reveal = RevealPixels::new(self.source, layer, frame);
        for pixel in std::mem::take(&mut meta.outside) {
            if self.source.index(pixel.x, pixel.y).is_some() {
                continue;
            }
            if let Some(index) = m.index(pixel.x.into(), pixel.y.into()) {
                reveal.write(&mut pixels, &mut meta.indices, index, &pixel, budget)?;
            } else {
                charge(budget, OutsidePixel::MEMORY_PIXELS)?;
                outside.push(pixel);
            }
        }
        if reveal.wrote {
            charge(budget, pixels.allocation_pixels())?;
        }
        let previous = &self.source.layers[layer].cel_meta[frame];
        m.retain_clipped(
            raster,
            previous.indices.as_deref().map(Vec::as_slice),
            self.source.transparent_index as u8,
            &mut outside,
            budget,
        )?;
        meta.outside = outside;
        meta.bounds
            .get_or_insert([0, 0, m.old_width as i32, m.old_height as i32]);
        Ok(pixels)
    }
}

struct RevealPixels<'a> {
    source: &'a Document,
    layer: usize,
    frame: usize,
    palette: Option<Vec<crate::model::Pixel>>,
    wrote: bool,
}

impl<'a> RevealPixels<'a> {
    fn new(source: &'a Document, layer: usize, frame: usize) -> Self {
        Self {
            source,
            layer,
            frame,
            palette: None,
            wrote: false,
        }
    }

    fn write(
        &mut self,
        pixels: &mut Pixels,
        indices: &mut Option<Arc<Vec<u8>>>,
        index: usize,
        outside: &OutsidePixel,
        budget: &mut usize,
    ) -> Result<(), String> {
        let mut color = outside.color;
        if indices.is_some()
            || (self.source.color_mode == ColorMode::Indexed && outside.palette_slot.is_some())
        {
            let transparent = self.source.transparent_index;
            let background = self.source.layers[self.layer].background;
            if self.palette.is_none() {
                charge(budget, pixels.len().div_ceil(4))?;
            }
            let palette = self
                .palette
                .get_or_insert_with(|| self.source.editing_palette(self.layer, self.frame));
            let slots = indices.get_or_insert_with(|| {
                let mut slots = vec![transparent as u8; pixels.len()];
                pixels.visit_nonclear(|index, pixel| {
                    slots[index] = nearest(pixel, palette) as u8;
                });
                Arc::new(slots)
            });
            let slot = if let Some(slot) = outside.palette_slot
                && let Some(&pixel) = palette.get(usize::from(slot))
            {
                color = pixel;
                usize::from(slot)
            } else if color[3] == 0 && !background {
                transparent
            } else {
                nearest(color, palette)
            };
            Arc::make_mut(slots)[index] = slot as u8;
        }
        pixels.set(index, color);
        self.wrote = true;
        Ok(())
    }
}

struct Mapping {
    width: usize,
    height: usize,
    old_width: usize,
    old_height: usize,
    offset: [i64; 2],
}

impl Mapping {
    fn resize_mask(
        &self,
        layer: &mut Layer,
        trim: bool,
        rasters: &mut HashMap<usize, Pixels>,
        budget: &mut usize,
    ) -> Result<(), String> {
        if let Some(mask) = &layer.mask {
            let mut resized = self.raster(mask, rasters, budget)?;
            let mut outside = Vec::new();
            if !trim {
                self.retain_clipped(mask, None, 0, &mut outside, budget)?;
            }
            let mut exposed = false;
            for pixel in layer.mask_outside.iter() {
                if let Some(index) = self.index(pixel.x.into(), pixel.y.into()) {
                    resized.set(index, pixel.color);
                    exposed = true;
                } else if !trim {
                    let [x, y] = self.outside_position(pixel.x.into(), pixel.y.into())?;
                    charge(budget, OutsidePixel::MEMORY_PIXELS)?;
                    outside.push(OutsidePixel {
                        x,
                        y,
                        ..pixel.clone()
                    });
                }
            }
            if exposed {
                charge(budget, resized.allocation_pixels())?;
            }
            layer.mask = Some(resized);
            layer.set_mask_outside(outside);
        }
        Ok(())
    }

    fn outside_position(&self, x: i64, y: i64) -> Result<[i32; 2], String> {
        let [dx, dy] = self.offset;
        let position = [x + dx, y + dy];
        if position
            .iter()
            .any(|value| !(-131_072..=131_072).contains(value))
        {
            return Err("Off-canvas artwork exceeds supported coordinates".into());
        }
        Ok(position.map(|value| value as i32))
    }

    fn retain_clipped(
        &self,
        source: &Pixels,
        indices: Option<&[u8]>,
        transparent: u8,
        hidden: &mut Vec<OutsidePixel>,
        budget: &mut usize,
    ) -> Result<(), String> {
        let [dx, dy] = self.offset;
        if dx >= 0
            && dy >= 0
            && self.old_width as i64 + dx <= self.width as i64
            && self.old_height as i64 + dy <= self.height as i64
        {
            return Ok(());
        }
        let mut keep = |index: usize, color, palette_slot| -> Result<(), String> {
            let x = (index % self.old_width) as i64;
            let y = (index / self.old_width) as i64;
            if self.index(x, y).is_none() {
                let [x, y] = self.outside_position(x, y)?;
                charge(budget, OutsidePixel::MEMORY_PIXELS)?;
                hidden.push(OutsidePixel {
                    x,
                    y,
                    color,
                    palette_slot,
                });
            }
            Ok(())
        };
        if let Some(slots) = indices {
            for (index, &slot) in slots.iter().enumerate() {
                let pixel = source[index];
                if slot != transparent || pixel != CLEAR {
                    keep(index, pixel, Some(slot))?;
                }
            }
        } else {
            let mut result = Ok(());
            source.try_visit_nonclear(|index, color| {
                result = keep(index, color, None);
                result.as_ref().ok().copied()
            });
            result?;
        }
        Ok(())
    }

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
            target,
            [self.old_width, self.old_height],
            [self.width, self.height],
            self.offset,
            |old, row| row.copy_from_slice(&source[old..old + row.len()]),
        );
    }

    fn raster(
        &self,
        source: &Pixels,
        cache: &mut HashMap<usize, Pixels>,
        budget: &mut usize,
    ) -> Result<Pixels, String> {
        if (self.width, self.height, self.offset) == (self.old_width, self.old_height, [0, 0]) {
            return Ok(source.clone());
        }
        let area = self.width * self.height;
        let Some(id) = source.backing_id() else {
            return Ok(Pixels::empty(area));
        };
        if let Some(raster) = cache.get(&id) {
            return Ok(raster.clone());
        }
        let output = if source.is_dense() {
            charge(budget, area)?;
            let mut output = vec![CLEAR; area];
            copy_grid(
                &mut output,
                [self.old_width, self.old_height],
                [self.width, self.height],
                self.offset,
                |old, row| source.copy_dense_range(old, row),
            );
            output.into()
        } else {
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

    fn has_external_tiles(&self, tileset: &Tileset, grid: &TilemapGeometry) -> bool {
        let x = i64::from(grid.x);
        let y = i64::from(grid.y);
        let tile_width = tileset.width as i64;
        let tile_height = tileset.height as i64;
        x + tile_width <= 0
            || y + tile_height <= 0
            || x + grid.columns.saturating_sub(1) as i64 * tile_width >= self.width as i64
            || y + grid.rows.saturating_sub(1) as i64 * tile_height >= self.height as i64
    }

    fn tiles(
        &self,
        tileset: &Tileset,
        old: TilemapGeometry,
        cells: &[u32],
        trim: bool,
        budget: &mut usize,
    ) -> Result<(TilemapGeometry, Vec<u32>), String> {
        let shifted = [
            i64::from(old.x) + self.offset[0],
            i64::from(old.y) + self.offset[1],
        ];
        let tile_size = [tileset.width as i64, tileset.height as i64];
        // Put the grid origin at or just before the canvas edge so every
        // canvas position has an editable cell even with sub-tile offsets.
        let mut origin: [i64; 2] =
            std::array::from_fn(|axis| -(-shifted[axis]).rem_euclid(tile_size[axis]));
        let mut end = [self.width as i64, self.height as i64];
        if !trim {
            for (index, _) in cells
                .iter()
                .enumerate()
                .filter(|(_, cell)| **cell & 0x1fff_ffff != 0)
            {
                let cell = [index % old.columns, index / old.columns];
                for axis in 0..2 {
                    let position = shifted[axis] + cell[axis] as i64 * tile_size[axis];
                    origin[axis] = origin[axis].min(position);
                    end[axis] = end[axis].max(position + tile_size[axis]);
                }
            }
        }
        let [columns, rows] = std::array::from_fn(|axis| {
            ((end[axis] - origin[axis]) as usize).div_ceil(tile_size[axis] as usize)
        });
        let count = columns.saturating_mul(rows);
        if count > MAX_PIXELS
            || columns > u16::MAX.into()
            || rows > u16::MAX.into()
            || origin
                .iter()
                .any(|&coordinate| i16::try_from(coordinate).is_err())
        {
            return Err("Resized tile grid exceeds the cell limit".into());
        }
        charge(budget, count)?;
        let delta = std::array::from_fn(|axis| (shifted[axis] - origin[axis]) / tile_size[axis]);
        let mut resized = vec![0; count];
        copy_grid(
            &mut resized,
            [old.columns, old.rows],
            [columns, rows],
            delta,
            |old, row| row.copy_from_slice(&cells[old..old + row.len()]),
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

fn copy_grid<T>(
    target: &mut [T],
    [old_width, old_height]: [usize; 2],
    [new_width, new_height]: [usize; 2],
    [dx, dy]: [i64; 2],
    mut copy_row: impl FnMut(usize, &mut [T]),
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
        copy_row(old, &mut target[new..new + cols.len()]);
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
            palette_slot: None,
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
