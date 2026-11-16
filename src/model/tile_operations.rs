use super::{CLEAR, Document, MAX_PIXELS, Pixel, TILE_ID_MASK, Tileset};

#[derive(Clone, Copy, Debug)]
pub enum TileDefinitionEdit {
    Duplicate,
    Delete,
    MoveTo(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TilePixelEdit {
    Shared,
    ReuseVariant,
    CreateVariant,
}

const TILE_FLIP_X: u32 = 0x8000_0000;
const TILE_FLIP_Y: u32 = 0x4000_0000;
const TILE_TRANSPOSE: u32 = 0x2000_0000;

#[derive(Clone, Copy, Debug)]
pub enum TileTransform {
    FlipHorizontal,
    FlipVertical,
    RotateClockwise,
}

impl TileTransform {
    pub fn apply(self, tile: u32, set: &Tileset) -> Result<u32, String> {
        if !(1..=set.tiles.len()).contains(&((tile & TILE_ID_MASK) as usize)) {
            return Err("Choose a tile to transform".into());
        }
        let transposed = tile & TILE_TRANSPOSE != 0;
        if set.width != set.height && (transposed || matches!(self, Self::RotateClockwise)) {
            return Err("Quarter turns and diagonal flips require square tiles".into());
        }
        let (horizontal, vertical) = if transposed {
            (TILE_FLIP_Y, TILE_FLIP_X)
        } else {
            (TILE_FLIP_X, TILE_FLIP_Y)
        };
        Ok(tile
            ^ match self {
                Self::FlipHorizontal => horizontal,
                Self::FlipVertical => vertical,
                Self::RotateClockwise => TILE_TRANSPOSE | vertical,
            })
    }
}

impl Tileset {
    pub(super) fn source_offset(
        &self,
        placement: u32,
        mut x: usize,
        mut y: usize,
    ) -> Option<usize> {
        if placement & TILE_TRANSPOSE != 0 {
            std::mem::swap(&mut x, &mut y);
        }
        if x >= self.width || y >= self.height {
            return None;
        }
        if placement & TILE_FLIP_X != 0 {
            x = self.width - 1 - x;
        }
        if placement & TILE_FLIP_Y != 0 {
            y = self.height - 1 - y;
        }
        Some(y * self.width + x)
    }

    /// Sample a tile in displayed coordinates using the placement's orientation.
    pub fn placement_pixel(&self, placement: u32, x: usize, y: usize) -> Pixel {
        let id = (placement & TILE_ID_MASK) as usize;
        id.checked_sub(1)
            .and_then(|index| self.tiles.get(index))
            .zip(self.source_offset(placement, x, y))
            .map_or(CLEAR, |(tile, offset)| tile[offset])
    }
}

impl Document {
    /// Use one-based tile IDs. A deleted ID becomes an empty placement; edits
    /// that shift IDs preserve the orientation of all surviving placements.
    pub fn edit_tile_definition(
        &mut self,
        tileset: usize,
        id: u32,
        edit: TileDefinitionEdit,
    ) -> Result<u32, String> {
        let set = self.tilesets.get_mut(tileset).ok_or("Choose a tileset")?;
        let count = set.tiles.len() as u32;
        if !(1..=count).contains(&id) {
            return Err("Choose a tile from the tileset".into());
        }
        let from = (id - 1) as usize;
        match edit {
            TileDefinitionEdit::Duplicate => {
                if (set.tiles.len() + 1)
                    .saturating_mul(set.width)
                    .saturating_mul(set.height)
                    > MAX_PIXELS
                {
                    return Err("Duplicating this tile exceeds the tileset size limit".into());
                }
                set.tiles.push(set.tiles[from].clone());
                Ok(count + 1)
            }
            TileDefinitionEdit::Delete => {
                set.tiles.remove(from);
                self.remap_tiles(tileset, |current| match current.cmp(&id) {
                    std::cmp::Ordering::Less => current,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => current - 1,
                });
                Ok(id.min(count - 1))
            }
            TileDefinitionEdit::MoveTo(to) => {
                if !(1..=count).contains(&to) {
                    return Err("Choose a valid tile destination".into());
                }
                if to != id {
                    let tile = set.tiles.remove(from);
                    set.tiles.insert((to - 1) as usize, tile);
                    self.remap_tiles(tileset, |current| {
                        if current == id {
                            to
                        } else if id < current && current <= to {
                            current - 1
                        } else if to <= current && current < id {
                            current + 1
                        } else {
                            current
                        }
                    });
                }
                Ok(to)
            }
        }
    }

    fn remap_tiles(&mut self, tileset: usize, remap: impl Fn(u32) -> u32) {
        for layer in self
            .layers
            .iter_mut()
            .filter(|layer| layer.tileset == Some(tileset))
        {
            for placement in layer.tile_cels.iter_mut().flatten() {
                let old = *placement & TILE_ID_MASK;
                let new = remap(old);
                if new != old {
                    *placement = if new == 0 {
                        0
                    } else {
                        (*placement & !TILE_ID_MASK) | new
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pixels;

    fn tilemap() -> Document {
        let mut doc = Document::new(3, 1);
        let tiles = [[100, 50, 30, 255], [10, 80, 90, 255], [44, 33, 120, 255]]
            .map(|color| Pixels::from(vec![color]));
        for name in ["Shared", "Other"] {
            doc.tilesets.push(Tileset {
                name: name.into(),
                width: 1,
                height: 1,
                tiles: tiles.to_vec(),
            });
        }
        doc.add_frame(0, false);
        doc.add_tile_layer(0).unwrap();
        doc.add_tile_layer(0).unwrap();
        doc.add_tile_layer(1).unwrap();
        doc.layers[1].tile_cels[0] = vec![0x8000_0001, 2, 3];
        doc.layers[1].tile_cels[1] = vec![0x4000_0003, 1, 0];
        doc.layers[2].tile_cels[1] = vec![2, 3, 1];
        doc.layers[3].tile_cels[1] = vec![1, 2, 3];
        doc
    }

    #[test]
    fn new_variant_keeps_its_own_id_when_existing_artwork_matches() {
        let mut doc = Document::new(4, 1);
        let a = [10, 20, 30, 255];
        let b = [40, 50, 60, 255];
        doc.tilesets.push(Tileset {
            name: "Variants".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![a, a].into(), vec![a, b].into()],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[0] = vec![0x8000_0001, 1];
        doc.paint_tile_pixel(1, 0, (0, 0), (0, 0), a, TilePixelEdit::CreateVariant)
            .unwrap();
        assert_eq!(doc.tilesets[0].tiles.len(), 2);
        doc.paint_tile_pixel(1, 0, (0, 0), (0, 0), b, TilePixelEdit::CreateVariant)
            .unwrap();
        assert_eq!(doc.layers[1].tile_cels[0], [0x8000_0003, 1]);
        assert_eq!(doc.tilesets[0].tiles[1], doc.tilesets[0].tiles[2]);
        doc.paint_tile_pixel(1, 0, (1, 0), (1, 0), b, TilePixelEdit::ReuseVariant)
            .unwrap();
        assert_eq!(doc.layers[1].tile_cels[0], [0x8000_0003, 2]);
    }

    #[test]
    fn automation_accepts_explicit_variant_mode_and_legacy_boolean() {
        let doc = tilemap();
        let mut edit = serde_json::json!({
            "op": "edit-tile-pixel", "layer": 1, "frame": 0,
            "x": 0, "y": 0, "pixel_x": 0, "pixel_y": 0, "color": [10, 80, 90, 255],
            "variant": true
        });
        let apply = |edit| {
            crate::automation::apply_command_batch(&doc, &serde_json::json!({"commands": [edit]}))
        };
        let legacy = apply(edit.clone()).unwrap();
        assert_eq!(legacy.tilesets[0].tiles.len(), 3);
        assert_eq!(legacy.layers[1].tile_cels[0][0], 0x8000_0002);
        edit["variant_mode"] = serde_json::json!("create-variant");
        let fresh = apply(edit.clone()).unwrap();
        assert_eq!(fresh.tilesets[0].tiles.len(), 4);
        assert_eq!(fresh.layers[1].tile_cels[0][0], 0x8000_0004);
        edit["variant_mode"] = serde_json::json!("invalid");
        assert!(apply(edit).is_err());
    }

    #[test]
    fn brush_transform_composes_with_existing_orientation() {
        let mut doc = Document::new(2, 2);
        let colors = [
            [1, 0, 0, 255],
            [2, 0, 0, 255],
            [3, 0, 0, 255],
            [4, 0, 0, 255],
        ];
        doc.tilesets.push(Tileset {
            name: "Asymmetric".into(),
            width: 2,
            height: 2,
            tiles: vec![colors.to_vec().into()],
        });
        doc.add_tile_layer(0).unwrap();
        let (a, b, c, d) = (colors[0], colors[1], colors[2], colors[3]);
        let mut tile = 1;
        for (command, expected) in [
            (TileTransform::RotateClockwise, [c, a, d, b]),
            (TileTransform::FlipHorizontal, [a, c, b, d]),
            (TileTransform::FlipVertical, [b, d, a, c]),
            (TileTransform::RotateClockwise, [a, b, c, d]),
        ] {
            tile = command.apply(tile, &doc.tilesets[0]).unwrap();
            doc.paint_tile(1, 0, 0, 0, tile).unwrap();
            assert_eq!(doc.composite(0), expected);
        }
        let rect = Tileset {
            name: "Rectangular".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![a, b].into()],
        };
        assert!(TileTransform::RotateClockwise.apply(1, &rect).is_err());
        assert!(TileTransform::FlipHorizontal.apply(1, &rect).is_ok());
        assert!(TileTransform::FlipHorizontal.apply(6, &rect).is_err());
    }

    #[test]
    fn reorder_preserves_artwork_and_transforms_across_the_shared_tileset() {
        let mut doc = tilemap();
        doc.layers[2].locked = true;
        let before = doc.clone();
        assert_eq!(
            doc.edit_tile_definition(0, 3, TileDefinitionEdit::MoveTo(1))
                .unwrap(),
            1
        );
        assert_eq!(doc.layers[1].tile_cels[0], [0x8000_0002, 3, 1]);
        assert_eq!(doc.layers[1].tile_cels[1], [0x4000_0001, 2, 0]);
        assert_eq!(doc.layers[2].tile_cels[1], [3, 1, 2]);
        assert_eq!(doc.layers[3], before.layers[3]);
        assert_eq!(doc.composite(0), before.composite(0));
        assert_eq!(doc.composite(1), before.composite(1));
        let data = crate::formats::encode_ase(&doc).unwrap();
        let reopened = crate::formats::decode_ase(&data).unwrap();
        assert_eq!(reopened.layers[1].tile_cels, doc.layers[1].tile_cels);
    }

    #[test]
    fn duplicate_is_independent_and_delete_clears_only_matching_placements() {
        let mut doc = tilemap();
        let before = doc.clone();
        let id = doc
            .edit_tile_definition(0, 2, TileDefinitionEdit::Duplicate)
            .unwrap();
        assert_eq!(id, 4);
        assert_eq!(doc.tilesets[0].tiles[1], doc.tilesets[0].tiles[3]);
        doc.tilesets[0].tiles[3].set(0, [33, 55, 44, 255]);
        assert_ne!(doc.tilesets[0].tiles[1], doc.tilesets[0].tiles[3]);
        assert_eq!(doc.layers, before.layers);
        assert_eq!(
            doc.edit_tile_definition(0, 1, TileDefinitionEdit::Delete)
                .unwrap(),
            1
        );
        assert_eq!(doc.layers[1].tile_cels[0], [0, 1, 2]);
        assert_eq!(doc.layers[1].tile_cels[1], [0x4000_0002, 0, 0]);
        assert_eq!(doc.layers[3], before.layers[3]);
        let before_invalid = doc.clone();
        assert!(
            doc.edit_tile_definition(0, 0, TileDefinitionEdit::Delete)
                .is_err()
        );
        assert!(
            doc.edit_tile_definition(0, 2, TileDefinitionEdit::MoveTo(4))
                .is_err()
        );
        assert_eq!(doc, before_invalid);
        doc.validate().unwrap();
    }
}
