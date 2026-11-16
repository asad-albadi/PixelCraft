use super::*;
use std::{
    collections::{BTreeMap, HashMap},
    hash::{DefaultHasher, Hash, Hasher},
};

#[derive(Clone, Copy)]
pub enum LayerConversion {
    Raster,
    Background(Pixel),
    Tilemap { width: usize, height: usize },
}

impl Document {
    pub fn convert_layer(
        &mut self,
        layer: usize,
        conversion: LayerConversion,
    ) -> Result<usize, String> {
        self.validate()?;
        if !self.editable(layer) || self.layers[layer].adjustment.is_some() {
            return Err("Choose an unlocked artwork layer to convert".into());
        }

        let mut result = self.clone();
        let destination = match conversion {
            LayerConversion::Raster => {
                if self.layers[layer].tileset.is_some() {
                    result.rasterize_tiles(self, layer)?;
                } else if self.layers[layer].background {
                    result.layers[layer].background = false;
                    result.remap_converted_indices(layer)?;
                }
                layer
            }
            LayerConversion::Background(fill) => {
                if self.layers[layer].tileset.is_some() {
                    return Err("Convert this tilemap to a raster layer first".into());
                }
                if self.layers[layer].background {
                    return Ok(layer);
                }
                if self.layers.iter().any(|other| other.background) {
                    return Err("The sprite already has a background layer".into());
                }
                result.make_background(self, layer, fill)?;
                0
            }
            LayerConversion::Tilemap { width, height } => {
                if self.layers[layer].tileset.is_some() || self.layers[layer].background {
                    return Err("Choose a regular raster layer to convert to tiles".into());
                }
                result.make_tilemap(layer, width, height)?;
                layer
            }
        };
        result.validate()?;
        *self = result;
        Ok(destination)
    }

    fn make_background(
        &mut self,
        original: &Document,
        layer: usize,
        mut fill: Pixel,
    ) -> Result<(), String> {
        fill[3] = 255;
        if self.color_mode == ColorMode::Grayscale {
            fill = convert(fill, ColorMode::Grayscale, &[]);
        }
        let area = self.width * self.height;
        let mut budget = MAX_DOCUMENT_PIXELS;
        let mut scene = Document::new(self.width, self.height);
        scene.durations.clone_from(&self.durations);
        let mut isolated = original.layers[layer].clone();
        isolated.parent = None;
        isolated.visible = true;
        scene.layers = vec![isolated];

        let target = &mut self.layers[layer];
        target.background = true;
        target.ase_flags &= !64;
        target.visible = true;
        target.parent = None;
        target.opacity = 255;
        target.blend = BlendMode::Normal;
        target.mask = None;
        for frame in 0..self.durations.len() {
            reserve(&mut budget, area)?;
            let mut pixels = vec![fill; area];
            scene.composite_into(frame, None, 0, None, true, &mut pixels);
            target.cels[frame] = pixels.into();
            let meta = &mut target.cel_meta[frame];
            meta.indices = None;
            meta.opacity = 255;
            meta.z_index = 0;
            meta.keyframe = None;
            meta.artwork_link = None;
            meta.bounds = None;
            meta.reference_bounds = None;
        }
        self.remap_converted_indices(layer)?;
        self.layers[..=layer].rotate_right(1);
        let remap = |index| {
            if index == layer {
                0
            } else if index < layer {
                index + 1
            } else {
                index
            }
        };
        for entry in &mut self.layers {
            entry.parent = entry.parent.map(remap);
        }
        for chunk in &mut self.metadata {
            for prefix in ["layer:", "cel:"] {
                if let Some(index) = chunk
                    .owner
                    .strip_prefix(prefix)
                    .and_then(|index| index.parse::<usize>().ok())
                {
                    chunk.owner = format!("{prefix}{}", remap(index));
                    break;
                }
            }
        }
        Ok(())
    }

    fn make_tilemap(&mut self, layer: usize, width: usize, height: usize) -> Result<(), String> {
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || width.saturating_mul(height) > MAX_PIXELS
        {
            return Err("Invalid tile dimensions".into());
        }
        let source = &mut self.layers[layer];
        let mut tileset = Tileset {
            name: format!("{} tiles", source.name),
            width,
            height,
            tiles: Vec::new(),
        };
        let mut hashing: HashMap<u64, Vec<usize>> = HashMap::new();
        let mut budget = MAX_DOCUMENT_PIXELS;
        let area = width * height;
        // Bound the staging map as well as the saved tileset. Every occupied
        // grid slot can allocate a sparse backing and a map node.
        let staging_units = area.div_ceil(PIXEL_BLOCK) * (PIXEL_BLOCK + 8) + 24;
        for (frame, (raster, cel)) in source.cels.iter_mut().zip(&mut source.cel_meta).enumerate() {
            let grid = grid_for_raster(self.width, self.height, width, height, &cel.outside)?;
            reserve(&mut budget, grid.columns * grid.rows)?;
            let mut cells = vec![0; grid.columns * grid.rows];
            let mut images = BTreeMap::new();
            let mut over_limit = false;
            let mut place = |x: i32, y: i32, pixel: Pixel| {
                if pixel == CLEAR || over_limit {
                    return;
                }
                let x = (x - grid.x) as usize;
                let y = (y - grid.y) as usize;
                let cell = (y / height) * grid.columns + x / width;
                let at_limit = images.len().saturating_mul(staging_units) >= MAX_DOCUMENT_PIXELS;
                if at_limit && !images.contains_key(&cell) {
                    over_limit = true;
                    return;
                }
                let tile = images.entry(cell).or_insert_with(|| Pixels::empty(area));
                tile.set((y % height) * width + (x % width), pixel);
            };
            raster.visit_nonclear(|index, pixel| {
                place(
                    (index % self.width) as i32,
                    (index / self.width) as i32,
                    pixel,
                );
            });
            for pixel in &cel.outside {
                if pixel.x < 0
                    || pixel.y < 0
                    || pixel.x as usize >= self.width
                    || pixel.y as usize >= self.height
                {
                    place(pixel.x, pixel.y, pixel.color);
                }
            }
            if over_limit {
                return Err("Conversion exceeds the tile work limit".into());
            }
            for (cell, tile) in images {
                let mut hash = DefaultHasher::new();
                tile.visit_nonclear(|index, color| {
                    index.hash(&mut hash);
                    color.hash(&mut hash);
                });
                let matches = hashing.entry(hash.finish()).or_default();
                let index = if let Some(&index) =
                    matches.iter().find(|&&index| tileset.tiles[index] == tile)
                {
                    index
                } else {
                    if (tileset.tiles.len() + 1).saturating_mul(area) > MAX_PIXELS {
                        return Err("Converted tileset exceeds the image limit".into());
                    }
                    reserve(&mut budget, tile.allocation_pixels().saturating_add(24))?;
                    let index = tileset.tiles.len();
                    tileset.tiles.push(tile);
                    matches.push(index);
                    index
                };
                cells[cell] = (index + 1) as u32;
            }
            *raster = Pixels::empty(self.width * self.height);
            cel.tilemap = Some(grid);
            cel.indices = None;
            cel.bounds = None;
            cel.outside.clear();
            source.tile_cels[frame] = cells;
        }
        // Different raster placements can cut different pieces of a shared
        // image into tiles. Link only the grids that are still equivalent.
        let mut first = HashMap::new();
        for frame in 0..source.cel_meta.len() {
            if let Some(link) = source.cel_meta[frame].artwork_link {
                let representative = *first.entry(link).or_insert(frame);
                if source.cel_meta[frame].tilemap != source.cel_meta[representative].tilemap
                    || source.tile_cels[frame] != source.tile_cels[representative]
                {
                    source.cel_meta[frame].artwork_link = None;
                }
            }
        }
        source.prune_cel_links();
        source.tileset = Some(self.tilesets.len());
        self.tilesets.push(tileset);
        Ok(())
    }

    fn rasterize_tiles(&mut self, original: &Document, layer: usize) -> Result<(), String> {
        let source = &original.layers[layer];
        let tileset = &original.tilesets[source.tileset.unwrap()];
        let target = &mut self.layers[layer];
        let (width, height) = (self.width, self.height);
        let mut work = MAX_DOCUMENT_PIXELS;
        let mut budget = MAX_DOCUMENT_PIXELS;
        for (frame, (raster, cel)) in target.cels.iter_mut().zip(&mut target.cel_meta).enumerate() {
            let nonempty = source.tile_cels[frame]
                .iter()
                .filter(|&&tile| tile & 0x1fff_ffff != 0)
                .count();
            reserve(
                &mut work,
                nonempty.saturating_mul(tileset.width * tileset.height),
            )?;
            let mut converted = Pixels::empty(width * height);
            let mut outside = Vec::new();
            let mut bound = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
            let mut exceeded = false;
            original.visit_tile_pixels(layer, frame, |x, y, color| {
                if color == CLEAR || exceeded {
                    return;
                }
                if let Some(index) = original.index(x, y) {
                    converted.set(index, color);
                } else if reserve(&mut budget, 3).is_ok() {
                    outside.push(OutsidePixel { x, y, color });
                } else {
                    exceeded = true;
                }
                bound[0] = bound[0].min(x);
                bound[1] = bound[1].min(y);
                bound[2] = bound[2].max(x);
                bound[3] = bound[3].max(y);
            });
            if exceeded {
                return Err("Layer conversion exceeds the document memory limit".into());
            }
            reserve(&mut budget, converted.allocation_pixels())?;
            *raster = converted;
            cel.bounds = (!outside.is_empty()).then_some([
                bound[0],
                bound[1],
                bound[2].saturating_sub(bound[0]).saturating_add(1),
                bound[3].saturating_sub(bound[1]).saturating_add(1),
            ]);
            cel.outside = outside;
            cel.tilemap = None;
            cel.indices = None;
            target.tile_cels[frame].clear();
        }
        target.tileset = None;
        self.remap_converted_indices(layer)
    }

    fn remap_converted_indices(&mut self, layer: usize) -> Result<(), String> {
        if self.color_mode != ColorMode::Indexed {
            return Ok(());
        }
        let area = self.width * self.height;
        let background = self.layers[layer].background;
        let mut budget = MAX_DOCUMENT_PIXELS;
        for frame in 0..self.durations.len() {
            let current_palette = self.palette_for(frame).to_vec();
            let mut palette = current_palette.clone();
            let transparent = self.transparent_index;
            if transparent >= palette.len() {
                return Err("Indexed conversion requires a valid transparent slot".into());
            }
            let target = &mut self.layers[layer];
            let pixels = &target.cels[frame];
            let cel = &mut target.cel_meta[frame];
            let previous = cel.indices.as_deref();
            if !background
                && pixels.backing_id().is_none()
                && previous.is_none()
                && cel.outside.is_empty()
            {
                continue;
            }
            reserve(&mut budget, area.div_ceil(4))?;
            let mut result = vec![transparent as u8; area];
            let mut by_color = HashMap::new();
            let mut lookup = |color: Pixel, previous: Option<u8>| {
                if let Some(slot) = previous {
                    let index = usize::from(slot);
                    if (background || index != transparent) && palette.get(index) == Some(&color) {
                        return Ok(slot);
                    }
                }
                if !background && color == CLEAR {
                    return Ok(transparent as u8);
                }
                if let Some(slot) = by_color.get(&color) {
                    return Ok(*slot);
                }
                let index = if let Some(index) =
                    palette.iter().enumerate().position(|(index, entry)| {
                        (background || index != transparent) && *entry == color
                    }) {
                    index
                } else if palette.len() < 256 {
                    let index = palette.len();
                    palette.push(color);
                    index
                } else {
                    return Err("Converted colors exceed the indexed palette. Convert the sprite to RGB and retry.".to_string());
                };
                by_color.insert(color, index as u8);
                Ok(index as u8)
            };
            let mut failure = None;
            pixels.visit_nonclear(|index, pixel| {
                if failure.is_some() {
                    return;
                }
                match lookup(pixel, previous.and_then(|slots| slots.get(index).copied())) {
                    Ok(slot) => result[index] = slot,
                    Err(error) => failure = Some(error),
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            // Outside cels are stored as RGBA and looked up in the palette on ASE export.
            for pixel in &cel.outside {
                lookup(pixel.color, None)?;
            }
            cel.indices = Some(Arc::new(result));
            if palette != current_palette {
                self.frame_palettes
                    .resize_with(self.frame_palettes.len().max(frame + 1), || {
                        self.palette.clone()
                    });
                self.frame_palettes[frame] = palette;
            }
        }
        Ok(())
    }
}

fn grid_for_raster(
    canvas_width: usize,
    canvas_height: usize,
    width: usize,
    height: usize,
    outside: &[OutsidePixel],
) -> Result<TilemapGeometry, String> {
    let [mut left, mut top, mut right, mut bottom] =
        [0, 0, canvas_width as i32, canvas_height as i32];
    for pixel in outside {
        left = left.min(pixel.x);
        top = top.min(pixel.y);
        right = right.max(pixel.x + 1);
        bottom = bottom.max(pixel.y + 1);
    }
    let x = left.div_euclid(width as i32) * width as i32;
    let y = top.div_euclid(height as i32) * height as i32;
    let columns = ((right - x) as usize).div_ceil(width);
    let rows = ((bottom - y) as usize).div_ceil(height);
    if i16::try_from(x).is_err()
        || i16::try_from(y).is_err()
        || columns > 65535
        || rows > 65535
        || columns.saturating_mul(rows) > MAX_PIXELS
    {
        return Err("The converted tile grid exceeds supported bounds".into());
    }
    Ok(TilemapGeometry {
        x,
        y,
        columns,
        rows,
    })
}

fn reserve(budget: &mut usize, units: usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(units)
        .ok_or("Layer conversion exceeds the document memory or work limit")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_tiles_roundtrip_all_frames_and_share_tiles_across_the_animation() {
        let mut doc = Document::new(3, 2);
        let rust = [194, 59, 33, 255];
        let gold = [235, 199, 51, 210];
        doc.add_frame(0, false);
        for frame in 0..2 {
            doc.layers[0].cels[frame].set(0, rust);
            doc.layers[0].cel_meta[frame].opacity = 200;
        }
        doc.layers[0].cels[1].set(5, gold);
        let frames = [doc.composite(0), doc.composite(1)];
        let identity = doc.layers[0].editor_id.to_string();

        assert_eq!(
            doc.convert_layer(
                0,
                LayerConversion::Tilemap {
                    width: 2,
                    height: 2
                }
            )
            .unwrap(),
            0
        );
        let tile_set = doc.layers[0].tileset.unwrap();
        assert_eq!(doc.tilesets[tile_set].tiles.len(), 2);
        assert_eq!(doc.layers[0].tile_cels[0], [1, 0]);
        assert_eq!(doc.layers[0].tile_cels[1], [1, 2]);
        assert_eq!(doc.composite(0), frames[0]);
        assert_eq!(doc.composite(1), frames[1]);
        assert_eq!(doc.layers[0].editor_id.to_string(), identity);

        doc.convert_layer(0, LayerConversion::Raster).unwrap();
        assert_eq!(doc.layers[0].tileset, None);
        assert_eq!(doc.composite(0), frames[0]);
        assert_eq!(doc.composite(1), frames[1]);
    }

    #[test]
    fn tile_conversion_preserves_a_keyframed_cel_that_moves_into_view() {
        let mut doc = Document::new(2, 2);
        let ink = [104, 99, 250, 255];
        let cel = &mut doc.layers[0].cel_meta[0];
        cel.outside.push(OutsidePixel {
            x: -1,
            y: 0,
            color: ink,
        });
        cel.bounds = Some([-1, 0, 1, 1]);
        cel.keyframe = Some(CelKeyframe {
            x: 1,
            ..CelKeyframe::default()
        });
        let original = doc.composite(0);
        assert_eq!(original[0], ink);

        doc.convert_layer(
            0,
            LayerConversion::Tilemap {
                width: 2,
                height: 2,
            },
        )
        .unwrap();
        assert_eq!(doc.composite(0), original);
        doc.convert_layer(0, LayerConversion::Raster).unwrap();
        assert_eq!(doc.composite(0), original);
        assert_eq!(doc.layers[0].cel_meta[0].outside[0].x, -1);
    }

    #[test]
    fn background_conversion_fills_every_frame_and_moves_the_layer_below_other_art() {
        let mut doc = Document::new(2, 2);
        let foreground = [9, 61, 211, 255];
        let ink = [180, 30, 7, 128];
        let paper = [37, 49, 59, 123];
        let opaque_paper = [37, 49, 59, 255];
        doc.add_frame(0, false);
        doc.add_layer();
        doc.layers[0].cels[0].set(0, foreground);
        doc.layers[1].cels[1].set(2, ink);
        doc.layers[1].name = "Paper".into();
        doc.metadata.push(MetadataChunk {
            frame: 0,
            kind: 0x2999,
            data: vec![1],
            owner: "layer:1".into(),
        });

        assert_eq!(
            doc.convert_layer(1, LayerConversion::Background(paper))
                .unwrap(),
            0
        );
        assert!(doc.layers[0].background);
        assert_eq!(doc.layers[0].name, "Paper");
        assert_eq!(doc.layers[0].cels[0][0], opaque_paper);
        assert_eq!(doc.composite(0)[0], foreground);
        assert_eq!(doc.composite(1)[2], over(opaque_paper, ink, 255));
        assert_eq!(doc.metadata[0].owner, "layer:0");

        let frames = [doc.composite(0), doc.composite(1)];
        doc.convert_layer(0, LayerConversion::Raster).unwrap();
        assert!(!doc.layers[0].background);
        assert_eq!(doc.composite(0), frames[0]);
        assert_eq!(doc.composite(1), frames[1]);
    }

    #[test]
    fn converting_indexed_background_keeps_opaque_colors_in_the_transparent_slot() {
        let mut doc = Document::new(2, 1);
        let brick = [190, 9, 17, 255];
        doc.palette = vec![brick, [90, 71, 60, 255]];
        doc.layers[0].background = true;
        doc.layers[0].cels[0].set(0, brick);
        doc.layers[0].cels[0].set(1, brick);
        doc.set_color_mode(ColorMode::Indexed);
        let original = doc.composite(0);
        assert_eq!(
            doc.layers[0].cel_meta[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            &[0, 0]
        );

        doc.convert_layer(0, LayerConversion::Raster).unwrap();

        assert_eq!(doc.composite(0), original);
        let slot = doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[0] as usize;
        assert_ne!(slot, doc.transparent_index);
        assert_eq!(doc.palette_for(0)[slot], brick);
        let ase = crate::formats::encode_ase(&doc).unwrap();
        let reopened = crate::formats::decode_ase(&ase).unwrap();
        assert_eq!(reopened.composite(0), original);
    }
}
