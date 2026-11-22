use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl Document {
    /// A conversion/render snapshot of one frame, with its animation sampled.
    /// Only placed tile definitions are retained, with placement IDs remapped.
    /// Export-only metadata and editor reference images are intentionally omitted.
    pub fn conversion_frame_snapshot(&self, frame: usize) -> Result<Self, String> {
        let duration = *self
            .durations
            .get(frame)
            .ok_or("Choose a valid preview frame")?;
        let mut layers = Vec::with_capacity(self.layers.len());
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.cels.len() != self.durations.len()
                || layer.cel_meta.len() != self.durations.len()
                || layer.tile_cels.len() != self.durations.len()
                || layer.cel_meta.iter().any(|meta| {
                    meta.keyframe.is_some_and(|key| {
                        key.x.unsigned_abs() > MAX_DIMENSION as u32
                            || key.y.unsigned_abs() > MAX_DIMENSION as u32
                    })
                })
            {
                return Err("Invalid animation frame metadata".into());
            }
            let mut meta = layer.cel_meta[frame].clone();
            meta.artwork_link = None;
            meta.keyframe = Some(self.sampled_cel_key(index, frame));
            layers.push(Layer {
                editor_id: layer.editor_id,
                name: layer.name.clone(),
                visible: layer.visible,
                locked: layer.locked,
                opacity: layer.opacity,
                cels: vec![layer.cels[frame].clone()],
                group: layer.group,
                parent: layer.parent,
                blend: layer.blend,
                cel_meta: vec![meta],
                tileset: layer.tileset,
                tile_cels: vec![layer.tile_cels[frame].clone()],
                background: layer.background,
                ase_flags: layer.ase_flags,
                uuid: layer.uuid.clone(),
                link_duplicates: layer.link_duplicates,
                exclude_from_onion: layer.exclude_from_onion,
                mask: layer.mask.clone(),
                mask_outside: layer.mask_outside.clone(),
                adjustment: layer.adjustment.clone(),
            });
        }
        let (tilesets, ase_tilesets) = self.referenced_preview_tiles(&mut layers)?;
        let snapshot = Self {
            editor_id: EditorId::default(),
            compose_groups: self.compose_groups,
            width: self.width,
            height: self.height,
            layers,
            durations: vec![duration],
            frame_names: self
                .frame_name(frame)
                .map(|name| vec![name.to_owned()])
                .unwrap_or_default(),
            frame_export_ids: Default::default(),
            color_mode: self.color_mode,
            palette: self.palette.clone(),
            palette_names: self.palette_names.clone(),
            frame_palettes: vec![self.palette_for(frame).to_vec()],
            transparent_index: self.transparent_index,
            drawing_index: self.drawing_index,
            tags: Vec::new(),
            slices: Vec::new(),
            tilesets,
            ase_tilesets,
            metadata: Vec::new(),
            color_profile: self.color_profile.clone(),
            brush_shape: self.brush_shape,
            dithering: self.dithering.clone(),
            wraparound: self.wraparound,
            palette_snap: self.palette_snap,
            drawing_grid: self.drawing_grid,
            pixel_aspect: self.pixel_aspect,
            references: Vec::new(),
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn referenced_preview_tiles(
        &self,
        layers: &mut [Layer],
    ) -> Result<(Vec<Tileset>, Vec<AseTilesetInfo>), String> {
        let mut used = BTreeMap::<usize, BTreeSet<u32>>::new();
        for layer in layers.iter() {
            let Some(index) = layer.tileset else {
                continue;
            };
            let set = self.tilesets.get(index).ok_or("Missing preview tileset")?;
            let ids = used.entry(index).or_default();
            for placed in &layer.tile_cels[0] {
                let id = placed & TILE_ID_MASK;
                if id as usize > set.tiles.len() {
                    return Err("Invalid preview tile reference".into());
                }
                if id != 0 {
                    ids.insert(id);
                }
            }
        }
        let mut sets = Vec::with_capacity(used.len());
        let mut mapping = BTreeMap::new();
        for (original, ids) in used {
            let source = &self.tilesets[original];
            let remap: BTreeMap<_, _> = ids
                .iter()
                .enumerate()
                .map(|(i, &id)| (id, (i + 1) as u32))
                .collect();
            mapping.insert(original, (sets.len(), remap));
            sets.push(Tileset {
                editor_id: source.editor_id,
                name: source.name.clone(),
                width: source.width,
                height: source.height,
                tiles: ids
                    .into_iter()
                    .map(|id| source.tiles[id as usize - 1].clone())
                    .collect(),
            });
        }
        for layer in layers {
            let Some(original) = layer.tileset else {
                continue;
            };
            let (index, ids) = &mapping[&original];
            layer.tileset = Some(*index);
            for placed in &mut layer.tile_cels[0] {
                let id = *placed & TILE_ID_MASK;
                if id != 0 {
                    *placed = (*placed & !TILE_ID_MASK) | ids[&id];
                }
            }
        }
        let annotations = self
            .ase_tilesets
            .iter()
            .filter_map(|info| {
                let (index, _) = mapping.get(&info.index)?;
                let mut retained = info.clone();
                retained.index = *index;
                Some(retained)
            })
            .collect();
        Ok((sets, annotations))
    }
}
