use super::{Document, Pixels, TILE_ID_MASK, Tileset};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct CompositeArea {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePatch {
    pub position: [usize; 2],
    pub size: [usize; 2],
}

struct FrameDamage {
    regions: BTreeMap<(usize, usize), FramePatch>,
    area: usize,
    area_limit: usize,
}

enum TileDamage {
    Pixels(Vec<usize>),
    Entire,
}

fn tile_definition_damage(
    previous: &Tileset,
    current: &Tileset,
) -> Option<HashMap<u32, TileDamage>> {
    const MAX_CHANGED_DEFINITIONS: usize = 256;
    const MAX_POINTS_PER_TILE: usize = 256;
    let mut changed = HashMap::new();
    for index in 0..previous.tiles.len().max(current.tiles.len()) {
        let damage = match (previous.tiles.get(index), current.tiles.get(index)) {
            (Some(before), Some(after)) if before.indices != after.indices => TileDamage::Entire,
            (Some(before), Some(after)) if before.len() == after.len() => {
                let mut points = Vec::new();
                let exact = visit_raster_differences(&before.pixels, &after.pixels, |index| {
                    points.push(index);
                    (points.len() <= MAX_POINTS_PER_TILE).then_some(())
                });
                if exact.is_none() {
                    TileDamage::Entire
                } else if points.is_empty() {
                    continue;
                } else {
                    TileDamage::Pixels(points)
                }
            }
            _ => TileDamage::Entire,
        };
        changed.insert(index as u32 + 1, damage);
        if changed.len() > MAX_CHANGED_DEFINITIONS {
            return None;
        }
    }
    Some(changed)
}

impl FrameDamage {
    fn include(&mut self, left: usize, top: usize, right: usize, bottom: usize) -> Option<()> {
        const SIDE: usize = 64;
        const MAX_PATCHES: usize = 64;
        if right <= left || bottom <= top {
            return Some(());
        }
        for row in top / SIDE..=(bottom - 1) / SIDE {
            for col in left / SIDE..=(right - 1) / SIDE {
                let x = left.max(col * SIDE);
                let y = top.max(row * SIDE);
                let r = right.min((col + 1) * SIDE);
                let b = bottom.min((row + 1) * SIDE);
                let region = self.regions.entry((row, col)).or_insert(FramePatch {
                    position: [x, y],
                    size: [0, 0],
                });
                let [ox, oy] = region.position;
                let [width, height] = region.size;
                let x = x.min(ox);
                let y = y.min(oy);
                let w = r.max(ox + width) - x;
                let h = b.max(oy + height) - y;
                self.area += w * h - width * height;
                *region = FramePatch {
                    position: [x, y],
                    size: [w, h],
                };
                if self.area > self.area_limit || self.regions.len() > MAX_PATCHES {
                    return None;
                }
            }
        }
        Some(())
    }
}

impl Document {
    /// Return small, disjoint update regions, or `None` when a full redraw is needed.
    /// An empty list means no visible area was affected.
    pub fn frame_pixel_damage(&self, previous: &Self, frame: usize) -> Option<Vec<FramePatch>> {
        if !self.same_frame_layout(previous, frame) {
            return None;
        }
        let mut damage = FrameDamage {
            regions: BTreeMap::new(),
            area: 0,
            area_limit: (self.width * self.height / 4).max(1),
        };
        let mut tileset_changes = HashMap::new();
        for (index, (layer, old)) in self.layers.iter().zip(&previous.layers).enumerate() {
            if layer.tileset.is_none()
                && (layer.group
                    || layer.adjustment.is_some()
                    || layer.cels[frame].shares_storage(&old.cels[frame]))
            {
                continue;
            }
            let mut current = Some(index);
            let mut offset = [0i64; 2];
            let mut depth = 0;
            while let Some(parent) = current {
                depth += 1;
                if depth > self.layers.len() {
                    return None;
                }
                let ancestor = self.layers.get(parent)?;
                if !ancestor.visible {
                    break;
                }
                let key = self.sampled_cel_key(parent, frame);
                offset[0] += i64::from(key.x);
                offset[1] += i64::from(key.y);
                current = ancestor.parent;
            }
            if current.is_some() {
                continue;
            }
            let mut include = |x: i64, y: i64, w: usize, h: usize| {
                let x = x + offset[0];
                let y = y + offset[1];
                let x0 = x.clamp(0, self.width as i64) as usize;
                let y0 = y.clamp(0, self.height as i64) as usize;
                let x1 = (x + w as i64).clamp(0, self.width as i64) as usize;
                let y1 = (y + h as i64).clamp(0, self.height as i64) as usize;
                damage.include(x0, y0, x1, y1)
            };
            if let (Some(set_index), Some(prior_index)) = (layer.tileset, old.tileset) {
                let set = self.tilesets.get(set_index)?;
                let prior_set = previous.tilesets.get(prior_index)?;
                if (set.width, set.height) != (prior_set.width, prior_set.height)
                    || layer.tile_cels[frame].len() != old.tile_cels[frame].len()
                {
                    return None;
                }
                let changed = tileset_changes
                    .entry((set_index, prior_index))
                    .or_insert_with(|| tile_definition_damage(prior_set, set))
                    .as_ref()?;
                if changed.is_empty() && old.tile_cels[frame] == layer.tile_cels[frame] {
                    continue;
                }
                let (x, y, columns) = layer.cel_meta[frame]
                    .tilemap
                    .as_ref()
                    .map_or((0, 0, self.width.div_ceil(set.width)), |map| {
                        (map.x, map.y, map.columns)
                    });
                for (slot, (before, after)) in old.tile_cels[frame]
                    .iter()
                    .zip(&layer.tile_cels[frame])
                    .enumerate()
                {
                    let definition = changed.get(&(after & TILE_ID_MASK));
                    if before == after && definition.is_none() {
                        continue;
                    }
                    let left = i64::from(x) + (slot % columns * set.width) as i64;
                    let top = i64::from(y) + (slot / columns * set.height) as i64;
                    if before == after
                        && let Some(TileDamage::Pixels(points)) = definition
                    {
                        for &point in points {
                            if let Some([x, y]) = set.displayed_position(*after, point) {
                                include(left + x as i64, top + y as i64, 1, 1)?;
                            }
                        }
                    } else {
                        include(left, top, set.width, set.height)?;
                    }
                }
            } else if layer.is_ase_reference() && layer.cel_meta[frame].reference_bounds.is_some() {
                if layer.cels[frame] != old.cels[frame] {
                    return None;
                }
            } else {
                visit_raster_differences(&old.cels[frame], &layer.cels[frame], |pixel| {
                    include(
                        (pixel % self.width) as i64,
                        (pixel / self.width) as i64,
                        1,
                        1,
                    )
                })?;
            }
        }
        Some(damage.regions.into_values().collect())
    }
}

fn visit_raster_differences(
    before: &Pixels,
    after: &Pixels,
    mut visit: impl FnMut(usize) -> Option<()>,
) -> Option<()> {
    if before.shares_storage(after) {
        return Some(());
    }
    if let Some(changes) = before.visit_shared_dense_changes(after, |index| {
        if visit(index).is_some() {
            std::ops::ControlFlow::Continue(())
        } else {
            std::ops::ControlFlow::Break(())
        }
    }) {
        return changes.is_continue().then_some(());
    }
    if let (Pixels::Dense(before), Pixels::Dense(after)) = (before, after) {
        const CHUNK: usize = 256;
        for (chunk, (before, after)) in before.chunks(CHUNK).zip(after.chunks(CHUNK)).enumerate() {
            if before != after {
                for (index, (before, after)) in before.iter().zip(after).enumerate() {
                    if before != after {
                        visit(chunk * CHUNK + index)?;
                    }
                }
            }
        }
    } else {
        before.try_visit_nonclear(|index, pixel| {
            if pixel != after[index] {
                visit(index)?;
            }
            Some(())
        })?;
        after.try_visit_nonclear(|index, _| {
            if before[index] == super::CLEAR {
                visit(index)?;
            }
            Some(())
        })?;
    }
    Some(())
}

impl CompositeArea {
    pub fn canvas(width: usize, height: usize) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn coordinates(self, index: usize) -> (i32, i32) {
        (
            (self.x + index % self.width) as i32,
            (self.y + index / self.width) as i32,
        )
    }

    pub fn index(self, x: i32, y: i32) -> Option<usize> {
        let x = (x as usize).checked_sub(self.x)?;
        let y = (y as usize).checked_sub(self.y)?;
        (x < self.width && y < self.height).then(|| y * self.width + x)
    }

    pub fn source(self, offset: [i32; 2], width: usize, height: usize) -> Self {
        let axis = |start: usize, length: usize, shift: i32, limit: usize| {
            let from = (start as i32 - shift).clamp(0, limit as i32) as usize;
            let end = ((start + length) as i32 - shift).clamp(0, limit as i32) as usize;
            (from, end - from)
        };
        let (x, width) = axis(self.x, self.width, offset[0], width);
        let (y, height) = axis(self.y, self.height, offset[1], height);
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Adjustment, BlendMode, CLEAR, CelKeyframe, Document, Layer, OutsidePixel, Pixels,
        TilemapGeometry, Tileset,
    };
    use super::FramePatch;

    #[test]
    fn full_compositor_matches_contiguous_pixels_for_edited_dense_pages() {
        let mut doc = Document::new(65, 64);
        let len = doc.width * doc.height;
        doc.layers[0].cels[0] = vec![[72, 102, 115, 181]; len].into();
        doc.add_layer();
        doc.layers[1].cels[0] = Pixels::Dense(std::sync::Arc::new(
            (0..len)
                .map(|i| [i as u8, 93, 52, if i % 3 == 0 { 0 } else { 144 }])
                .collect(),
        ));
        let original = doc.clone();
        let cel = &mut doc.layers[1].cels[0];
        for i in [255, 256, 1025, len - 1] {
            cel.set(i, [225, 164, 29, 255]);
        }
        assert!(matches!(cel, Pixels::Overlay(_)));
        assert_ne!(cel, &original.layers[1].cels[0]);
        let flattened = Pixels::Dense(std::sync::Arc::new(cel.to_vec()));

        for shifted in [false, true] {
            if shifted {
                let ink = &mut doc.layers[1];
                ink.blend = BlendMode::Multiply;
                ink.mask = Some(
                    (0..len)
                        .map(|i| [0, 0, 0, i as u8])
                        .collect::<Vec<_>>()
                        .into(),
                );
                ink.cel_meta[0].keyframe = Some(CelKeyframe {
                    x: 3,
                    y: -2,
                    opacity: 201,
                    ..CelKeyframe::default()
                });
            }
            let mut contiguous = doc.clone();
            contiguous.layers[1].cels[0] = flattened.clone();
            assert_eq!(doc.composite_view(0), contiguous.composite_view(0));
        }
    }

    #[test]
    fn frame_damage_limits_patch_count_and_ignores_invisible_ink() {
        let before = Document::new(1024, 768);
        let mut edited = before.clone();
        for row in 0..8 {
            for col in 0..8 {
                edited.layers[0].cels[0].set(row * 64 * 1024 + col * 64, [61, 79, 120, 255]);
            }
        }
        let patches = edited.frame_pixel_damage(&before, 0).unwrap();
        assert_eq!(patches.len(), 64);
        assert!(patches.iter().all(|patch| patch.size == [1, 1]));
        edited.layers[0].cels[0].set(8 * 64, [61, 79, 120, 255]);
        assert!(edited.frame_pixel_damage(&before, 0).is_none());

        edited.layers[0].visible = false;
        let mut hidden_baseline = before;
        hidden_baseline.layers[0].visible = false;
        assert_eq!(edited.frame_pixel_damage(&hidden_baseline, 0), Some(vec![]));
    }

    #[test]
    fn pixel_damage_matches_full_render_for_shifted_rasters_and_tile_placements() {
        let mut doc = Document::new(12, 10);
        doc.layers[0].group = true;
        doc.layers[0].cel_meta[0].keyframe = Some(CelKeyframe {
            x: 2,
            y: -1,
            ..CelKeyframe::default()
        });
        doc.add_layer();
        doc.layers[1].parent = Some(0);
        doc.layers[1].cels[0] = Pixels::Dense(std::sync::Arc::new(vec![[63, 128, 191, 220]; 120]));
        doc.layers[1].cel_meta[0].keyframe = Some(CelKeyframe {
            x: -1,
            y: 3,
            ..CelKeyframe::default()
        });
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Tiles".into(),
            width: 2,
            height: 2,
            tiles: vec![
                vec![
                    [48, 79, 109, 210],
                    [151, 31, 87, 0],
                    [203, 116, 25, 255],
                    CLEAR,
                ]
                .into(),
            ],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[2].cel_meta[0].tilemap = Some(TilemapGeometry {
            x: -1,
            y: 0,
            columns: 6,
            rows: 5,
        });
        doc.layers[2].cel_meta[0].keyframe = Some(CelKeyframe {
            x: 1,
            y: 1,
            ..CelKeyframe::default()
        });
        let previous = doc.clone();
        doc.layers[1].cels[0].set(12 + 3, [12, 11, 10, 255]);
        assert_eq!(
            doc.frame_pixel_damage(&previous, 0),
            Some(vec![FramePatch {
                position: [4, 3],
                size: [1, 1]
            }])
        );
        doc.layers[2].tile_cels[0][0] = 0x8000_0001;
        let patches = doc.frame_pixel_damage(&previous, 0).unwrap();
        assert_eq!(patches.len(), 1);
        let FramePatch {
            position: origin,
            size,
        } = patches[0];
        assert_eq!((origin, size), ([0, 1], [5, 3]));
        let patch = doc.composite_view_region(0, origin, size).unwrap();
        let mut updated = previous.composite_view(0);
        for row in 0..size[1] {
            let index = (origin[1] + row) * doc.width + origin[0];
            updated[index..index + size[0]]
                .copy_from_slice(&patch[row * size[0]..(row + 1) * size[0]]);
        }
        assert_eq!(doc.composite_view(0), updated);
        doc.layers[2].opacity = 128;
        assert!(doc.frame_pixel_damage(&previous, 0).is_none());
    }

    #[test]
    fn cropped_compositor_matches_every_pixel_of_the_full_scene() {
        let mut doc = Document::new(11, 9);
        doc.add_frame(0, false);
        let layer = |name: &str| Layer::new(name.into(), 11, 9, 2);
        doc.layers[0].cels[0].set(0, [44, 32, 67, 0]);
        doc.layers[0].cels[0].set(2, [33, 109, 130, 255]);
        doc.layers[0].cels[0].set(88, [71, 19, 90, 186]);

        let mut group = layer("Group");
        group.group = true;
        group.opacity = 149;
        doc.layers.push(group);
        let mut raster = layer("Shifted ink");
        raster.parent = Some(1);
        raster.blend = BlendMode::Multiply;
        raster.cels[0].set(1, [52, 194, 163, 255]);
        raster.cels[0].set(70, [27, 61, 201, 70]);
        raster.cel_meta[0].keyframe = Some(CelKeyframe {
            x: 2,
            y: 1,
            opacity: 211,
            hold: true,
        });
        raster.cel_meta[0].outside.push(OutsidePixel {
            x: -1,
            y: 4,
            color: [247, 155, 81, 245],
            palette_slot: None,
        });
        doc.layers.push(raster);
        doc.link_cel(2, 1, 0).unwrap();

        let mut nested = layer("Nested group");
        nested.group = true;
        nested.parent = Some(1);
        nested.blend = BlendMode::Screen;
        nested.cel_meta[0].keyframe = Some(CelKeyframe {
            x: -1,
            y: 2,
            ..CelKeyframe::default()
        });
        doc.layers.push(nested);
        let mut dense = layer("Masked dense ink");
        dense.parent = Some(3);
        dense.cels = vec![
            Pixels::Dense(std::sync::Arc::new(
                (0..99)
                    .map(|i| if i % 5 == 0 {
                        [i as u8 * 2, 25, 50, 98]
                    } else if i % 3 == 0 {
                        [10, 31, 98, 0]
                    } else {
                        CLEAR
                    })
                    .collect()
            ));
            2
        ];
        dense.mask = Some(
            (0..99)
                .map(|i| [0, 0, 0, (i * 27 % 256) as u8])
                .collect::<Vec<_>>()
                .into(),
        );
        doc.layers.push(dense);

        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Stones".into(),
            width: 2,
            height: 2,
            tiles: vec![
                vec![
                    [48, 79, 109, 210],
                    [151, 31, 87, 0],
                    [203, 116, 25, 255],
                    CLEAR,
                ]
                .into(),
            ],
        });
        doc.add_tile_layer(0).unwrap();
        let tiles = &mut doc.layers[5];
        tiles.cel_meta[0].tilemap = Some(TilemapGeometry {
            x: -2,
            y: -1,
            columns: 6,
            rows: 5,
        });
        tiles.cel_meta[0].keyframe = Some(CelKeyframe {
            x: 3,
            y: 2,
            ..CelKeyframe::default()
        });
        tiles.tile_cels[0][0] = 0x8000_0001;
        tiles.tile_cels[0][1] = 0x2000_0001;
        tiles.tile_cels[0][6] = 0x4000_0001;

        let mut reference = layer("Scaled reference");
        reference.ase_flags = 64;
        reference.cel_meta[0].bounds = Some([-1, 1, 3, 2]);
        reference.cel_meta[0].reference_bounds = Some([32768, 98304, 4 * 65536, 3 * 65536]);
        reference.cel_meta[0].outside.push(OutsidePixel {
            x: -1,
            y: 1,
            color: [229, 63, 27, 128],
            palette_slot: None,
        });
        reference.cels[0].set(11, [157, 56, 167, 177]);
        doc.layers.push(reference);
        let mut adjustment = layer("Levels");
        adjustment.opacity = 158;
        adjustment.adjustment = Some(Adjustment::Levels {
            black: 15,
            white: 200,
        });
        doc.layers.push(adjustment);
        doc.validate().unwrap();

        for isolate in [true, false] {
            doc.compose_groups = isolate;
            for frame in 0..2 {
                let full = doc.composite_view(frame);
                for y in 0..doc.height {
                    for x in 0..doc.width {
                        assert_eq!(
                            doc.composite_view_region(frame, [x, y], [1, 1]).unwrap(),
                            [full[y * doc.width + x]],
                            "pixel {x},{y}, frame {frame}, groups {isolate}"
                        );
                    }
                    let row = &full[y * doc.width..(y + 1) * doc.width];
                    assert_eq!(
                        doc.composite_view_region(frame, [0, y], [doc.width, 1])
                            .unwrap(),
                        row
                    );
                }
            }
        }
        assert!(
            doc.composite_view_region(0, [usize::MAX, 0], [1, 1])
                .is_none()
        );
        assert!(doc.composite_view_region(0, [0, 0], [0, 1]).is_none());
    }
}
