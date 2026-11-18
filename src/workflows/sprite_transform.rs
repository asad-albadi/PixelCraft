use super::canvas_resize::ASE_COORDINATE_UNIT;
use crate::model::{
    CLEAR, Document, EditorId, MAX_DOCUMENT_PIXELS, MAX_PIXELS, Pixels, TilemapGeometry, Tileset,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy)]
pub enum Orientation {
    TurnRight,
    TurnLeft,
    TurnHalf,
    FlipHorizontal,
    FlipVertical,
}

impl Orientation {
    fn inverse(self) -> Self {
        match self {
            Self::TurnRight => Self::TurnLeft,
            Self::TurnLeft => Self::TurnRight,
            _ => self,
        }
    }

    fn size<T: Copy>(self, size: [T; 2]) -> [T; 2] {
        match self {
            Self::TurnRight | Self::TurnLeft => [size[1], size[0]],
            _ => size,
        }
    }

    fn point(self, [w, h]: [i64; 2], [x, y]: [i64; 2]) -> [i64; 2] {
        match self {
            Self::TurnRight => [h - 1 - y, x],
            Self::TurnLeft => [y, w - 1 - x],
            Self::TurnHalf => [w - 1 - x, h - 1 - y],
            Self::FlipHorizontal => [w - 1 - x, y],
            Self::FlipVertical => [x, h - 1 - y],
        }
    }

    fn rect(self, [w, h]: [i64; 2], [x, y, rw, rh]: [i64; 4]) -> [i64; 4] {
        match self {
            Self::TurnRight => [h - y - rh, x, rh, rw],
            Self::TurnLeft => [y, w - x - rw, rh, rw],
            Self::TurnHalf => [w - x - rw, h - y - rh, rw, rh],
            Self::FlipHorizontal => [w - x - rw, y, rw, rh],
            Self::FlipVertical => [x, h - y - rh, rw, rh],
        }
    }

    fn vector(self, [x, y]: [i32; 2]) -> [i32; 2] {
        match self {
            Self::TurnRight => [-y, x],
            Self::TurnLeft => [y, -x],
            Self::TurnHalf => [-x, -y],
            Self::FlipHorizontal => [-x, y],
            Self::FlipVertical => [x, -y],
        }
    }

    fn rect_i32(self, size: [usize; 2], rect: [i32; 4]) -> Result<[i32; 4], String> {
        let [x, y, w, h] = self.rect(size.map(|n| n as i64), rect.map(i64::from));
        let narrow = |v| i32::try_from(v).map_err(|_| "Sprite transform exceeds coordinate limits");
        Ok([narrow(x)?, narrow(y)?, narrow(w)?, narrow(h)?])
    }

    fn index(self, [w, h]: [usize; 2], i: usize) -> usize {
        let [x, y] = self.point([w as i64, h as i64], [(i % w) as i64, (i / w) as i64]);
        y as usize * self.size([w, h])[0] + x as usize
    }

    fn cells<T: Copy>(self, size: [usize; 2], cells: &[T]) -> Vec<T> {
        let target_size = self.size(size);
        (0..cells.len())
            .map(|index| cells[self.inverse().index(target_size, index)])
            .collect()
    }
}

struct RasterTransform {
    orientation: Orientation,
    cached: HashMap<(usize, [usize; 2]), Pixels>,
    remaining: usize,
}

impl RasterTransform {
    fn charge(&mut self, amount: usize) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or("Sprite transform exceeds the document memory limit")?;
        Ok(())
    }

    fn pixels(&mut self, size: [usize; 2], source: &Pixels) -> Result<Pixels, String> {
        let Some(id) = source.backing_id() else {
            return Ok(Pixels::empty(source.len()));
        };
        if let Some(pixels) = self.cached.get(&(id, size)) {
            return Ok(pixels.clone());
        }
        let result = if let Some(base) = source.dense_base_pixels() {
            self.charge(source.len())?;
            let mut transformed = self.orientation.cells(size, base);
            source.visit_dense_edits(|index, pixel| {
                transformed[self.orientation.index(size, index)] = pixel;
            });
            transformed.into()
        } else {
            let mut resized = Pixels::empty(source.len());
            source.visit_nonclear(|index, color| {
                resized.set(self.orientation.index(size, index), color);
            });
            self.charge(resized.allocation_pixels())?;
            resized
        };
        self.cached.insert((id, size), result.clone());
        Ok(result)
    }
}

/// Reorient all raster artwork, editable tile grids and document coordinates.
pub fn orient_sprite(doc: &Document, orientation: Orientation) -> Result<Document, String> {
    doc.validate()?;
    let size = [doc.width, doc.height];
    let mut output = doc.clone();
    let mut rasters = RasterTransform {
        orientation,
        cached: HashMap::new(),
        remaining: MAX_DOCUMENT_PIXELS,
    };
    let mut indices = HashMap::new();
    let mut tile_variants = HashMap::new();
    let mut tile_indices = HashMap::new();
    let flags = transformed_tile_flags(orientation);
    for tileset in &mut output.tilesets {
        let tile_size = [tileset.width, tileset.height];
        for tile in &mut tileset.tiles {
            tile.pixels = rasters.pixels(tile_size, tile)?;
            if let Some(slots) = &mut tile.indices {
                let key = (Arc::as_ptr(slots), tile_size);
                if let Some(cached) = tile_indices.get(&key) {
                    *slots = Arc::clone(cached);
                } else {
                    rasters.charge(slots.len().div_ceil(4))?;
                    let transformed = Arc::new(orientation.cells(tile_size, slots));
                    tile_indices.insert(key, Arc::clone(&transformed));
                    *slots = transformed;
                }
            }
        }
        [tileset.width, tileset.height] = orientation.size(tile_size);
    }
    for layer in &mut output.layers {
        if matches!(
            orientation,
            Orientation::TurnRight | Orientation::TurnLeft | Orientation::TurnHalf
        ) {
            layer.replace_cel_image_ids();
        }
        if let Some(mask) = &mut layer.mask {
            *mask = rasters.pixels(size, mask)?;
        }
        if !layer.mask_outside.is_empty() {
            rasters.charge(
                layer
                    .mask_outside
                    .len()
                    .saturating_mul(crate::model::OutsidePixel::MEMORY_PIXELS),
            )?;
            let mut outside = layer.mask_outside.as_ref().clone();
            for pixel in &mut outside {
                let [x, y, _, _] = orientation.rect_i32(size, [pixel.x, pixel.y, 1, 1])?;
                pixel.x = x;
                pixel.y = y;
            }
            layer.set_mask_outside(outside);
        }
        for (frame, (pixels, meta)) in layer.cels.iter_mut().zip(&mut layer.cel_meta).enumerate() {
            *pixels = rasters.pixels(size, pixels)?;
            if let Some(slots) = &mut meta.indices {
                let id = Arc::as_ptr(slots);
                if let Some(cached) = indices.get(&id) {
                    *slots = Arc::clone(cached);
                } else {
                    rasters.charge(slots.len().div_ceil(4))?;
                    let oriented = Arc::new(orientation.cells(size, slots));
                    indices.insert(id, Arc::clone(&oriented));
                    *slots = oriented;
                }
            }
            for outside in &mut meta.outside {
                let [x, y, _, _] = orientation.rect_i32(size, [outside.x, outside.y, 1, 1])?;
                outside.x = x;
                outside.y = y;
            }
            if let Some(bounds) = &mut meta.bounds {
                *bounds = orientation.rect_i32(size, *bounds)?;
            }
            if let Some(bounds) = &mut meta.reference_bounds {
                let fixed_size = size.map(|dimension| dimension * ASE_COORDINATE_UNIT as usize);
                *bounds = orientation.rect_i32(fixed_size, *bounds)?;
            }
            if let Some(keyframe) = &mut meta.keyframe {
                [keyframe.x, keyframe.y] = orientation.vector([keyframe.x, keyframe.y]);
            }
            if let Some(tileset_index) = layer.tileset {
                let source = &doc.tilesets[tileset_index];
                let geometry = meta.tilemap.clone().unwrap_or(TilemapGeometry {
                    x: 0,
                    y: 0,
                    columns: doc.width.div_ceil(source.width),
                    rows: doc.height.div_ceil(source.height),
                });
                let tiles = &mut layer.tile_cels[frame];
                rasters.charge(tiles.len())?;
                let mut oriented = orientation.cells([geometry.columns, geometry.rows], tiles);
                for tile in &mut oriented {
                    let orientation_bits = (*tile >> 29) as usize;
                    let index = *tile & 0x1fff_ffff;
                    if index == 0 {
                        *tile = 0;
                    } else if source.width != source.height && orientation_bits & 1 != 0 {
                        // A diagonal flag on a rectangular tile samples only its
                        // overlapping square. Materialize that exact appearance.
                        let key = (tileset_index, *tile);
                        if let Some(variant) = tile_variants.get(&key) {
                            *tile = *variant;
                        } else {
                            let target = &mut output.tilesets[tileset_index];
                            let variant = rectangular_tile_variant(
                                source,
                                target,
                                index,
                                orientation_bits,
                                &mut rasters,
                                doc.transparent_index as u8,
                            )?;
                            tile_variants.insert(key, variant);
                            *tile = variant;
                        }
                    } else {
                        *tile = index | ((flags[orientation_bits] as u32) << 29);
                    }
                }
                *tiles = oriented.into();
                let [x, y, _, _] = orientation.rect_i32(
                    size,
                    [
                        geometry.x,
                        geometry.y,
                        (geometry.columns * source.width) as i32,
                        (geometry.rows * source.height) as i32,
                    ],
                )?;
                let [columns, rows] = orientation.size([geometry.columns, geometry.rows]);
                meta.tilemap = Some(TilemapGeometry {
                    x,
                    y,
                    columns,
                    rows,
                });
            }
        }
    }
    for slice in &mut output.slices {
        for key in &mut slice.keys {
            let slice_size = [key.bounds[2] as usize, key.bounds[3] as usize];
            key.bounds = orientation.rect_i32(size, key.bounds)?;
            if let Some(center) = &mut key.center {
                *center = orientation.rect_i32(slice_size, *center)?;
            }
            if let Some(pivot) = &mut key.pivot {
                let [x, y, _, _] = orientation.rect_i32(slice_size, [pivot[0], pivot[1], 1, 1])?;
                *pivot = [x, y];
            }
        }
    }
    for reference in &mut output.references {
        if reference.floating {
            continue;
        }
        let [w, h] = size.map(|n| n as f32);
        let [x, y] = [reference.x, reference.y];
        let [rw, rh] = [reference.width, reference.height].map(|n| n as f32 * reference.scale);
        [reference.x, reference.y] = match orientation {
            Orientation::TurnRight => [h - y - rh, x],
            Orientation::TurnLeft => [y, w - x - rw],
            Orientation::TurnHalf => [w - x - rw, h - y - rh],
            Orientation::FlipHorizontal => [w - x - rw, y],
            Orientation::FlipVertical => [x, h - y - rh],
        };
        let reference_size = [reference.width, reference.height];
        reference.pixels = rasters.pixels(reference_size, &reference.pixels)?;
        [reference.width, reference.height] = orientation.size(reference_size);
    }
    [output.width, output.height] = orientation.size(size);
    output.validate()?;
    Ok(output)
}

fn tile_source([w, h]: [i64; 2], [mut x, mut y]: [i64; 2], bits: usize) -> Option<[i64; 2]> {
    if bits & 1 != 0 {
        std::mem::swap(&mut x, &mut y);
    }
    if x >= w || y >= h {
        return None;
    }
    if bits & 4 != 0 {
        x = w - 1 - x;
    }
    if bits & 2 != 0 {
        y = h - 1 - y;
    }
    Some([x, y])
}

fn transformed_tile_flags(orientation: Orientation) -> [usize; 8] {
    let points = [[0, 0], [1, 0], [0, 1], [1, 1]];
    let size = [2, 2];
    std::array::from_fn(|bits| {
        (0..8)
            .find(|&candidate| {
                points.into_iter().all(|point| {
                    let old_point = orientation.inverse().point(size, point);
                    let new_source = tile_source(size, old_point, bits)
                        .map(|point| orientation.point(size, point));
                    tile_source(size, point, candidate) == new_source
                })
            })
            .expect("orthogonal tile transform has a matching flag")
    })
}

fn rectangular_tile_variant(
    source: &Tileset,
    target: &mut Tileset,
    index: u32,
    bits: usize,
    rasters: &mut RasterTransform,
    transparent: u8,
) -> Result<u32, String> {
    let area = target.width * target.height;
    if (target.tiles.len() + 1).saturating_mul(area) > MAX_PIXELS {
        return Err("Sprite transform would exceed the tileset limit".into());
    }
    rasters.charge(area)?;
    let source_pixels = &source.tiles[index as usize - 1];
    let size = [source.width, source.height].map(|n| n as i64);
    let new_size = [target.width, target.height].map(|n| n as i64);
    let mut pixels = vec![CLEAR; area];
    let mut slots = source_pixels
        .indices
        .as_ref()
        .map(|_| vec![transparent; area]);
    if slots.is_some() {
        rasters.charge(area.div_ceil(4))?;
    }
    for (i, pixel) in pixels.iter_mut().enumerate() {
        let point = [(i % target.width) as i64, (i / target.width) as i64];
        let old = rasters.orientation.inverse().point(new_size, point);
        if let Some([x, y]) = tile_source(size, old, bits) {
            let source_index = y as usize * source.width + x as usize;
            *pixel = source_pixels[source_index];
            if let (Some(slots), Some(source)) = (&mut slots, &source_pixels.indices) {
                slots[i] = source[source_index];
            }
        }
    }
    let mut tile: crate::model::TileRaster = pixels.into();
    tile.indices = slots.map(Arc::new);
    target.tiles.push(tile);
    Ok(target.tiles.len() as u32)
}

pub fn duplicate_sprite(doc: &Document) -> Document {
    let mut duplicate = doc.clone();
    duplicate.editor_id = EditorId::default();
    for layer in &mut duplicate.layers {
        layer.editor_id = EditorId::default();
        for cel in &mut layer.cel_meta {
            cel.editor_id = EditorId::default();
        }
    }
    duplicate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CLEAR, ColorMode, SliceKey, SpriteSlice, Tileset};

    #[test]
    fn whole_sprite_rotation_preserves_indices_and_repositions_slice_keys() {
        let mut doc = Document::new(3, 2);
        let ink = [200, 32, 40, 255];
        doc.palette = vec![CLEAR, ink, ink];
        doc.set_color_mode(ColorMode::Indexed);
        doc.add_frame(0, false);
        doc.add_layer();
        doc.set_frame_name(1, "Land").unwrap();
        doc.layers[0].cel_meta[0].indices = Some(vec![0, 2, 0, 0, 0, 0].into());
        doc.layers[0].cels[0].set(1, ink);
        doc.layers[1].cel_meta[1].indices = Some(vec![0, 0, 0, 1, 0, 0].into());
        doc.layers[1].cels[1].set(3, ink);
        doc.slices.push(SpriteSlice {
            editor_id: Default::default(),
            name: "Region".into(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: [0, 0, 2, 1],
                center: None,
                pivot: Some([1, 0]),
            }],
        });

        let rotated = orient_sprite(&doc, Orientation::TurnRight).unwrap();
        assert_eq!([rotated.width, rotated.height], [2, 3]);
        assert_eq!(
            rotated.layers[0].cel_meta[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[0, 0, 0, 2, 0, 0]
        );
        assert_eq!(
            rotated.layers[1].cel_meta[1]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[1, 0, 0, 0, 0, 0]
        );
        assert_eq!(rotated.frame_name(1), Some("Land"));
        assert_eq!(rotated.slices[0].keys[0].bounds, [1, 0, 1, 2]);
        assert_eq!(rotated.slices[0].keys[0].pivot, Some([0, 1]));
    }

    #[test]
    fn whole_sprite_flip_and_rotation_preserve_tilemap_appearance() {
        let mut doc = Document::new(6, 3);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Tile".into(),
            width: 2,
            height: 3,
            tiles: vec![
                vec![
                    [20, 1, 2, 255],
                    [40, 3, 4, 255],
                    [60, 5, 6, 255],
                    [80, 7, 8, 255],
                    [100, 9, 10, 255],
                    [120, 11, 12, 255],
                ]
                .into(),
            ],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[0].copy_from_slice(&[1, 0x8000_0001, 0xe000_0001]);
        let pixels = doc.composite(0);

        let flipped = orient_sprite(&doc, Orientation::FlipHorizontal).unwrap();
        let expected: Vec<_> = pixels
            .chunks(6)
            .flat_map(|row| row.iter().rev().copied())
            .collect();
        assert_eq!(flipped.composite(0), expected);
        assert_eq!(flipped.layers[1].tileset, Some(0));

        let rotated = orient_sprite(&doc, Orientation::TurnRight).unwrap();
        let expected: Vec<_> = (0..6)
            .flat_map(|x| (0..3).rev().map(move |y| y * 6 + x))
            .map(|index| pixels[index])
            .collect();
        assert_eq!(rotated.composite(0), expected);
        assert_eq!(rotated.tilesets[0].width, 3);
        assert_eq!(rotated.tilesets[0].height, 2);
    }

    #[test]
    fn duplicating_an_animated_sprite_gives_every_edit_target_a_fresh_identity() {
        let mut doc = Document::new(2, 2);
        let ink = [200, 32, 40, 255];
        doc.set_pixel(0, 0, 0, ink);
        doc.add_frame(0, true);
        doc.add_layer();

        let mut duplicate = duplicate_sprite(&doc);
        assert_eq!(duplicate, doc);
        assert_ne!(duplicate.editor_id.to_string(), doc.editor_id.to_string());
        for (source, target) in doc.layers.iter().zip(&duplicate.layers) {
            assert_ne!(source.editor_id.to_string(), target.editor_id.to_string());
            for (source, target) in source.cel_meta.iter().zip(&target.cel_meta) {
                assert_ne!(source.editor_id.to_string(), target.editor_id.to_string());
            }
        }
        duplicate.set_pixel(0, 0, 0, CLEAR);
        assert_eq!(doc.layers[0].cels[0][0], ink);
    }
}
