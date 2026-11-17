use super::*;

impl Document {
    pub fn flatten_layers(&mut self, visible_only: bool) -> Result<usize, String> {
        self.validate()?;
        let included: Vec<_> = (0..self.layers.len())
            .map(|layer| !visible_only || self.layer_effectively_visible(layer))
            .collect();
        if !included.iter().any(|included| *included) {
            return Err("There are no visible layers to flatten".into());
        }

        let mut scene = self.clone();
        if !visible_only {
            for layer in &mut scene.layers {
                layer.visible = true;
            }
        }
        scene.references.clear();
        scene.slices.clear();
        scene.metadata.clear();
        let [left, top, width, height] = merge_region(&scene)?;
        let original_bounds = [0, 0, self.width as i32, self.height as i32];
        let bounds = [left, top, width, height];
        let expanded = if bounds != original_bounds {
            let mut expanded = scene.clone();
            crate::workflows::resize_canvas(
                &mut expanded,
                [width as usize, height as usize],
                [-left, -top],
            )?;
            Some(expanded)
        } else {
            None
        };
        let mut flattened = Layer::new(
            "Flattened".into(),
            self.width,
            self.height,
            self.durations.len(),
        );
        let mut budget = MAX_DOCUMENT_PIXELS;
        for frame in 0..self.durations.len() {
            let palette = self.palette_for(frame);
            let mut rendered = expanded.as_ref().unwrap_or(&scene).composite(frame);
            if expanded.is_some() {
                // Preserve the original canvas clipping of nested groups, even
                // when expanding the temporary scene to recover off-canvas cels.
                let original = scene.composite(frame);
                for (row, pixels) in original.chunks_exact(self.width).enumerate() {
                    let first = (row + (-top) as usize) * width as usize + (-left) as usize;
                    rendered[first..first + self.width].copy_from_slice(pixels);
                }
            }
            let mut palette = palette.to_vec();
            if let Some(color) = palette.get_mut(self.transparent_index) {
                color[3] = 0;
            }
            let (pixels, meta) =
                self.bake_merged_frame(rendered, bounds, &palette, false, &mut budget)?;
            flattened.cels[frame] = pixels;
            flattened.cel_meta[frame] = meta;
        }

        let mut result = self.clone();
        let mut remapped = vec![None; self.layers.len()];
        result.layers.clear();
        for (index, layer) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(index, _)| !included[*index])
        {
            remapped[index] = Some(result.layers.len());
            result.layers.push(layer.clone());
        }
        for (index, source) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(index, _)| !included[*index])
        {
            let mut ancestor = source.parent;
            while let Some(parent) = ancestor {
                if !included[parent] {
                    break;
                }
                ancestor = self.layers[parent].parent;
            }
            result.layers[remapped[index].unwrap()].parent =
                ancestor.and_then(|parent| remapped[parent]);
        }
        result.metadata.retain_mut(|meta| {
            let Some((prefix, old)) = layer_owner(&meta.owner) else {
                return true;
            };
            let Some(Some(index)) = remapped.get(old) else {
                return false;
            };
            meta.owner = format!("{prefix}{index}");
            true
        });
        let layer = result.layers.len();
        result.layers.push(flattened);
        result.validate()?;
        *self = result;
        Ok(layer)
    }

    fn layer_effectively_visible(&self, layer: usize) -> bool {
        let mut current = Some(layer);
        while let Some(index) = current {
            let layer = &self.layers[index];
            if !layer.visible {
                return false;
            }
            current = layer.parent;
        }
        true
    }

    pub fn layer_from_selection(
        &mut self,
        layer: usize,
        frame: usize,
        mask: &[bool],
        cut: bool,
    ) -> Result<usize, String> {
        let source = self.layers.get(layer).ok_or("Invalid layer")?;
        let area = self.width * self.height;
        if source.group
            || source.adjustment.is_some()
            || source.tileset.is_some()
            || (cut && !self.editable(layer))
            || frame >= self.durations.len()
            || mask.len() != area
        {
            return Err(
                "Choose a raster cel and a valid selection. Cutting requires an unlocked layer."
                    .into(),
            );
        }
        if !self.can_add_layer() {
            return Err("Adding a layer would exceed the document limits".into());
        }

        let source_pixels = &source.cels[frame];
        let source_indices = source.cel_meta[frame].indices.as_deref();
        let mut result = self.clone();
        let mut target = Layer::new(
            if cut {
                "Layer via cut"
            } else {
                "Layer via copy"
            }
            .into(),
            self.width,
            self.height,
            self.durations.len(),
        );
        target.parent = source.parent;
        target.opacity = source.opacity;
        target.blend = source.blend;
        target.visible = source.visible;
        target.mask = source.mask.clone();
        target.cel_meta[frame].opacity = source.cel_meta[frame].opacity;
        target.cel_meta[frame].z_index = source.cel_meta[frame].z_index;
        target.cel_meta[frame].keyframe = Some(self.sampled_cel_key(layer, frame));
        let mut target_indices = source_indices.map(|_| vec![self.transparent_index as u8; area]);
        let mut copied = false;
        for (index, selected) in mask.iter().copied().enumerate() {
            if !selected {
                continue;
            }
            let pixel = source_pixels[index];
            let slot = source_indices.map(|indices| indices[index]);
            if pixel == CLEAR && slot.is_none_or(|slot| usize::from(slot) == self.transparent_index)
            {
                continue;
            }
            target.cels[frame].set(index, pixel);
            if let (Some(indices), Some(slot)) = (&mut target_indices, slot) {
                indices[index] = slot;
            }
            if cut {
                result.restore_pixel(
                    layer,
                    frame,
                    index,
                    CLEAR,
                    Some(self.transparent_index as u8),
                );
            }
            copied = true;
        }
        if !copied {
            return Err("The selection contains no artwork".into());
        }
        target.cel_meta[frame].indices = target_indices.map(Arc::new);
        let index = layer + 1;
        for existing in &mut result.layers {
            if let Some(parent) = existing.parent.as_mut().filter(|parent| **parent >= index) {
                *parent += 1;
            }
        }
        if let Some(parent) = target.parent.as_mut().filter(|parent| **parent >= index) {
            *parent += 1;
        }
        for meta in &mut result.metadata {
            if let Some((prefix, old)) = layer_owner(&meta.owner)
                && (index..self.layers.len()).contains(&old)
            {
                meta.owner = format!("{prefix}{}", old + 1);
            }
        }
        result.layers.insert(index, target);
        result.validate()?;
        *self = result;
        Ok(index)
    }
    /// Insert a complete copy of the selected layer or group immediately above its root.
    pub fn duplicate_layer(&mut self, root: usize) -> Result<usize, String> {
        if root >= self.layers.len() {
            return Err("Invalid layer".into());
        }
        self.validate()?;
        let members: Vec<_> = (0..self.layers.len())
            .filter(|&layer| self.is_descendant_or_self(layer, root))
            .collect();
        let count = self.layers.len() + members.len();
        let extra_pixels = members.iter().fold(0usize, |total, &index| {
            let layer = &self.layers[index];
            total
                .saturating_add(layer.tile_cels.iter().map(Vec::len).sum())
                .saturating_add(layer.cel_meta.iter().map(|cel| cel.outside.len() * 3).sum())
        });
        if count > usize::from(u16::MAX)
            || count.saturating_mul(self.durations.len()) > MAX_CEL_SLOTS
            || !self.can_grow(extra_pixels)
        {
            return Err("Duplicating this layer would exceed the document limits".into());
        }

        let insert = root + 1;
        let original_index = |index| index + if index >= insert { members.len() } else { 0 };
        let mut copy_index = vec![None; self.layers.len()];
        for (offset, &index) in members.iter().enumerate() {
            copy_index[index] = Some(insert + offset);
        }
        let mut result = self.clone();
        for layer in &mut result.layers {
            layer.parent = layer.parent.map(original_index);
        }
        let copies = members.iter().map(|&index| {
            let mut layer = self.layers[index].clone();
            layer.editor_id = EditorId::default();
            layer.uuid.clear();
            layer.parent = layer.parent.map(|parent| {
                if index == root {
                    original_index(parent)
                } else {
                    copy_index[parent].expect("copied layer has its group in the subtree")
                }
            });
            if index == root {
                layer.name.push_str(" copy");
            }
            for cel in &mut layer.cel_meta {
                cel.editor_id = EditorId::default();
            }
            layer
        });
        result.layers.splice(insert..insert, copies);
        for (original, destination) in self.metadata.iter().zip(&mut result.metadata) {
            if let Some((prefix, layer)) = layer_owner(&original.owner)
                && layer < self.layers.len()
            {
                destination.owner = format!("{prefix}{}", original_index(layer));
            }
        }
        for original in &self.metadata {
            if let Some((prefix, layer)) = layer_owner(&original.owner)
                && let Some(index) = copy_index.get(layer).copied().flatten()
            {
                let mut chunk = original.clone();
                chunk.owner = format!("{prefix}{index}");
                result.metadata.push(chunk);
            }
        }
        result.validate()?;
        *self = result;
        Ok(copy_index[root].expect("root was copied"))
    }

    fn is_descendant_or_self(&self, mut layer: usize, root: usize) -> bool {
        loop {
            if layer == root {
                return true;
            }
            let Some(parent) = self.layers[layer].parent else {
                return false;
            };
            layer = parent;
        }
    }

    /// Bake the selected leaf and its lower sibling using the normal compositor.
    /// The destination retains its position and visibility.
    pub fn merge_down(&mut self, upper: usize) -> Result<usize, String> {
        self.validate()?;
        let above = self.layers.get(upper).ok_or("Invalid layer")?;
        let lower = (0..upper)
            .rev()
            .find(|&index| self.layers[index].parent == above.parent)
            .ok_or("There is no layer below in this group")?;
        let below = &self.layers[lower];
        if !self.editable(upper) || !self.editable(lower) {
            return Err(
                "Merge down requires two unlocked image or tile layers in the same group".into(),
            );
        }
        if above.adjustment.is_some() || below.adjustment.is_some() {
            return Err("Merge down requires artwork layers. Adjustment layers affect the layers below them.".into());
        }

        let mut pair = Document::new(self.width, self.height);
        pair.layers = vec![below.clone(), above.clone()];
        pair.tilesets.clone_from(&self.tilesets);
        pair.durations.clone_from(&self.durations);
        for layer in &mut pair.layers {
            layer.parent = None;
            layer.visible = true;
        }
        // Keep the pair's original z-order even when other entries in the
        // layer vector sit between these siblings.
        for frame in 0..self.durations.len() {
            let bottom_z = i32::from(below.cel_meta[frame].z_index);
            let top_z = i32::from(above.cel_meta[frame].z_index);
            let top_first = (upper as i32 + top_z, top_z) < (lower as i32 + bottom_z, bottom_z);
            pair.layers[0].cel_meta[frame].z_index = 0;
            pair.layers[1].cel_meta[frame].z_index = if top_first { -2 } else { 0 };
        }

        let [left, top, width, height] = merge_region(&pair)?;
        crate::workflows::resize_canvas(
            &mut pair,
            [width as usize, height as usize],
            [-left, -top],
        )?;
        let mut result = self.clone();
        let merged = &mut result.layers[lower];
        merged.opacity = 255;
        merged.blend = BlendMode::Normal;
        merged.mask = None;
        merged.tileset = None;
        let mut budget = MAX_DOCUMENT_PIXELS;
        for frame in 0..self.durations.len() {
            let palette = self.editing_palette(lower, frame);
            let (raster, mut meta) = self.bake_merged_frame(
                pair.composite(frame),
                [left, top, width, height],
                &palette,
                merged.background,
                &mut budget,
            )?;
            meta.editor_id = merged.cel_meta[frame].editor_id;
            merged.cels[frame] = raster;
            merged.cel_meta[frame] = meta;
            merged.tile_cels[frame].clear();
        }
        result.remove_layer(upper);
        result.validate()?;
        if !result.can_grow(0) {
            return Err("Merging these layers would exceed the document memory limit".into());
        }
        *self = result;
        Ok(lower)
    }

    fn bake_merged_frame(
        &self,
        rendered: Vec<Pixel>,
        [left, top, width, _height]: [i32; 4],
        palette: &[Pixel],
        background: bool,
        budget: &mut usize,
    ) -> Result<(Pixels, CelMeta), String> {
        let area = self.width * self.height;
        let mut raster = Pixels::empty(area);
        let mut meta = CelMeta::default();
        let indexed = self.color_mode == ColorMode::Indexed;
        let transparent = self.transparent_index.min(palette.len() - 1);
        let mut indices = None;
        let mut slots_by_color = std::collections::HashMap::new();
        let mut bounds = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for (offset, mut color) in rendered.into_iter().enumerate() {
            if color == CLEAR {
                continue;
            }
            let x = left + (offset % width as usize) as i32;
            let y = top + (offset / width as usize) as i32;
            let slot = if indexed {
                let slot = if let Some(&slot) = slots_by_color.get(&color) {
                    slot
                } else {
                    let slot = merged_index(color, palette, transparent, background);
                    if slots_by_color.len() < 4096 {
                        slots_by_color.insert(color, slot);
                    }
                    slot
                };
                color = palette[slot];
                slot
            } else {
                0
            };
            if let Some(index) = self.index(x, y) {
                if indexed {
                    if indices.is_none() {
                        consume_merge_budget(budget, area.div_ceil(4))?;
                    }
                    indices.get_or_insert_with(|| vec![transparent as u8; area])[index] =
                        slot as u8;
                }
                raster.set(index, color);
            } else {
                consume_merge_budget(budget, 3)?;
                meta.outside.push(OutsidePixel { x, y, color });
            }
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x);
            bounds[3] = bounds[3].max(y);
        }
        consume_merge_budget(budget, raster.allocation_pixels())?;
        if !meta.outside.is_empty() {
            let [x, y, right, bottom] = bounds;
            meta.bounds = Some([x, y, right - x + 1, bottom - y + 1]);
        }
        meta.indices = indices.map(Arc::new);
        Ok((raster, meta))
    }
}

pub(super) fn layer_owner(owner: &str) -> Option<(&'static str, usize)> {
    ["layer:", "cel:"].into_iter().find_map(|prefix| {
        let index = owner.strip_prefix(prefix)?.parse().ok()?;
        Some((prefix, index))
    })
}

fn merged_index(color: Pixel, palette: &[Pixel], transparent: usize, background: bool) -> usize {
    if color[3] == 0 && !background {
        return transparent;
    }
    let index = nearest(color, palette);
    if background || color[3] == 0 || palette[index][3] > 0 {
        return index;
    }
    palette
        .iter()
        .enumerate()
        .filter(|(_, value)| value[3] > 0)
        .min_by_key(|(_, value)| {
            value
                .iter()
                .zip(color)
                .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .map_or(index, |(index, _)| index)
}

fn consume_merge_budget(budget: &mut usize, units: usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(units)
        .ok_or("Merging these layers would exceed the document memory limit")?;
    Ok(())
}

fn merge_region(pair: &Document) -> Result<[i32; 4], String> {
    let mut left = 0;
    let mut top = 0;
    let mut right = pair.width as i32;
    let mut bottom = pair.height as i32;
    let mut include = |x: i32, y: i32, key: CelKeyframe| {
        left = left.min(x).min(x + key.x);
        top = top.min(y).min(y + key.y);
        right = right.max(x + 1).max(x + key.x + 1);
        bottom = bottom.max(y + 1).max(y + key.y + 1);
    };
    for (index, layer) in pair.layers.iter().enumerate() {
        if !pair.layer_effectively_visible(index) {
            continue;
        }
        for (frame, meta) in layer.cel_meta.iter().enumerate() {
            let mut key = pair.sampled_cel_key(index, frame);
            let mut ancestor = layer.parent;
            while let Some(group) = ancestor {
                let offset = pair.sampled_cel_key(group, frame);
                key.x += offset.x;
                key.y += offset.y;
                ancestor = pair.layers[group].parent;
            }
            for pixel in &meta.outside {
                include(pixel.x, pixel.y, key);
            }
            if let Some(tileset) = layer.tileset {
                let tileset = &pair.tilesets[tileset];
                let width = tileset.width;
                let height = tileset.height;
                let (x, y, columns) = meta
                    .tilemap
                    .as_ref()
                    .map_or((0, 0, pair.width.div_ceil(width)), |g| {
                        (g.x, g.y, g.columns)
                    });
                for (i, &tile) in layer.tile_cels[frame].iter().enumerate() {
                    if tile & 0x1fff_ffff != 0 {
                        let left = x + (i % columns * width) as i32;
                        let top = y + (i / columns * height) as i32;
                        include(left, top, key);
                        include(left + width as i32 - 1, top + height as i32 - 1, key);
                    }
                }
            } else if key.x != 0 || key.y != 0 {
                layer.cels[frame].visit_nonclear(|source, _| {
                    include(
                        (source % pair.width) as i32,
                        (source / pair.width) as i32,
                        key,
                    );
                });
            }
        }
    }
    let width = right - left;
    let height = bottom - top;
    if width as usize > MAX_DIMENSION
        || height as usize > MAX_DIMENSION
        || (width as usize).saturating_mul(height as usize) > MAX_PIXELS
    {
        return Err("The merged artwork exceeds the supported canvas bounds".into());
    }
    Ok([left, top, width, height])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_visible_bakes_every_frame_and_retains_hidden_subtrees() {
        let mut doc = Document::new(2, 2);
        let visible = doc.add_group("Visible").unwrap();
        doc.layers[0].parent = Some(visible);
        let hidden = doc.add_group("Hidden").unwrap();
        doc.layers[hidden].visible = false;
        doc.add_layer();
        doc.layers[3].parent = Some(hidden);
        doc.add_frame(0, false);
        let ink = [201, 44, 12, 255];
        doc.layers[0].cels[0].set(0, ink);
        doc.layers[0].cels[1].set(3, ink);
        doc.layers[3].cels[1].set(0, [99, 88, 77, 255]);
        let frames = [doc.composite(0), doc.composite(1)];

        let flat = doc.flatten_layers(true).unwrap();

        assert_eq!(doc.layers.len(), 3);
        assert_eq!(doc.layers[flat].name, "Flattened");
        assert!(!doc.layers[0].visible);
        assert_eq!(doc.layers[1].parent, Some(0));
        assert_eq!(doc.layers[1].cels[1][0], [99, 88, 77, 255]);
        assert_eq!(doc.composite(0), frames[0]);
        assert_eq!(doc.composite(1), frames[1]);

        doc.flatten_layers(false).unwrap();
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.composite(1)[0], [99, 88, 77, 255]);
    }

    #[test]
    fn cut_to_layer_uses_the_current_frame_mask_and_exact_palette_slots() {
        let mut doc = Document::new(2, 2);
        let color = [201, 44, 12, 255];
        doc.palette = vec![CLEAR, color, color];
        doc.set_color_mode(ColorMode::Indexed);
        doc.add_frame(0, false);
        doc.layers[0].cels[0].set(0, color);
        doc.layers[0].cels[1].set(1, color);
        doc.layers[0].cels[1].set(2, color);
        doc.layers[0].cel_meta[1].indices = Some(Arc::new(vec![0, 2, 1, 0]));

        let copied = doc
            .layer_from_selection(0, 1, &[false, true, false, false], false)
            .unwrap();
        assert_eq!(doc.layers[0].cels[1][1], color);
        assert_eq!(
            doc.layers[copied].cel_meta[1].indices.as_ref().unwrap()[1],
            2
        );
        let cut = doc
            .layer_from_selection(0, 1, &[false, true, false, false], true)
            .unwrap();

        assert_eq!(doc.layers[0].cels[0][0], color);
        assert_eq!(doc.layers[0].cels[1][1], CLEAR);
        assert_eq!(doc.layers[0].cels[1][2], color);
        assert_eq!(doc.layers[cut].cels[0][0], CLEAR);
        assert_eq!(doc.layers[cut].cels[1][1], color);
        assert_eq!(doc.layers[cut].cel_meta[1].indices.as_ref().unwrap()[1], 2);
    }

    #[test]
    fn duplicates_nested_groups_across_frames_with_fresh_targets_and_mapped_owners() {
        let mut sprite = Document::new(2, 2);
        sprite.add_frame(0, false);
        let group = sprite.add_group("Props").unwrap();
        let child_group = sprite.add_group("Lantern").unwrap();
        sprite.layers[child_group].parent = Some(group);
        sprite.layers[0].parent = Some(child_group);
        sprite.layers[0].name = "Flame".into();
        sprite.layers[0].uuid = vec![7; 16];
        sprite.add_layer();
        sprite.layers[3].name = "Unrelated".into();
        let ink = [230, 90, 3, 255];
        sprite.layers[0].cels[1].set(2, ink);
        sprite.metadata.push(MetadataChunk {
            frame: 1,
            kind: 0x2999,
            data: vec![1, 2, 3],
            owner: "cel:0".into(),
        });
        let original = sprite.clone();

        let cloned_root = sprite.duplicate_layer(group).unwrap();

        assert_eq!(sprite.layers.len(), 7);
        assert_eq!(sprite.layers[cloned_root].name, "Props copy");
        let copied_group = sprite
            .layers
            .iter()
            .position(|l| l.name == "Lantern" && l.parent == Some(cloned_root))
            .unwrap();
        let copied_child = sprite
            .layers
            .iter()
            .position(|l| l.name == "Flame" && l.parent == Some(copied_group))
            .unwrap();
        assert_eq!(sprite.layers[copied_child].cels[1][2], ink);
        assert!(sprite.layers[copied_child].uuid.is_empty());
        assert_ne!(
            sprite.layers[copied_child].editor_id.to_string(),
            original.layers[0].editor_id.to_string()
        );
        assert_ne!(
            sprite.layers[copied_child].cel_meta[1]
                .editor_id
                .to_string(),
            original.layers[0].cel_meta[1].editor_id.to_string()
        );
        assert!(
            sprite
                .metadata
                .iter()
                .any(|m| m.owner == format!("cel:{copied_child}") && m.data == [1, 2, 3])
        );
        sprite.layers[copied_child].cels[1].set(2, CLEAR);
        assert_eq!(sprite.layers[0].cels[1][2], ink);
        assert!(
            sprite
                .layers
                .iter()
                .find(|l| l.name == "Unrelated")
                .unwrap()
                .parent
                .is_none()
        );
    }

    #[test]
    fn merge_down_preserves_off_canvas_artwork_and_rasterizes_overlaid_tiles() {
        let mut doc = Document::new(2, 2);
        let ink = [201, 21, 31, 255];
        let tile_color = [13, 175, 80, 255];
        doc.layers[0].cel_meta[0].bounds = Some([-1, 0, 3, 2]);
        doc.layers[0].cel_meta[0].outside.push(OutsidePixel {
            x: -1,
            y: 1,
            color: ink,
        });
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Detail".into(),
            width: 1,
            height: 1,
            tiles: vec![vec![tile_color].into()],
        });
        doc.add_tile_layer(0).unwrap();
        doc.layers[1].tile_cels[0][0] = 0x8000_0001;
        let expected = doc.composite(0);

        assert_eq!(doc.merge_down(1).unwrap(), 0);
        assert_eq!(doc.composite(0), expected);
        assert_eq!(doc.layers[0].tileset, None);
        assert_eq!(
            doc.layers[0].cel_meta[0].outside,
            [OutsidePixel {
                x: -1,
                y: 1,
                color: ink
            }]
        );
        let encoded = crate::formats::encode_project(&doc).unwrap();
        let reopened = crate::formats::decode_project(&encoded).unwrap();
        assert_eq!(
            reopened.layers[0].cel_meta[0].outside,
            doc.layers[0].cel_meta[0].outside
        );
    }

    #[test]
    fn merges_sibling_rasters_across_frames_using_the_real_compositor() {
        let mut sprite = Document::new(3, 2);
        let group = sprite.add_group("Set").unwrap();
        sprite.layers[0].parent = Some(group);
        sprite.add_layer();
        let upper = 2;
        sprite.layers[upper].parent = Some(group);
        sprite.layers[upper].blend = BlendMode::Multiply;
        sprite.layers[upper].opacity = 180;
        sprite.layers[0].opacity = 242;
        sprite.add_frame(0, false);
        sprite.layers[0].cels[0].set(0, [97, 150, 200, 255]);
        sprite.layers[upper].cels[0].set(0, [180, 81, 32, 220]);
        sprite.layers[upper].cel_meta[0].opacity = 140;
        sprite.layers[upper].cels[1].set(4, [120, 240, 42, 255]);
        sprite.layers[upper].cel_meta[1].keyframe = Some(CelKeyframe {
            x: 1,
            ..CelKeyframe::default()
        });
        let frames = [sprite.composite(0), sprite.composite(1)];

        assert_eq!(sprite.merge_down(upper).unwrap(), 0);

        assert_eq!(sprite.layers.len(), 2);
        assert_eq!(sprite.layers[0].parent, Some(1));
        assert_eq!(sprite.layers[0].blend, BlendMode::Normal);
        assert_eq!(sprite.layers[0].opacity, 255);
        assert_eq!(sprite.composite(0), frames[0]);
        assert_eq!(sprite.composite(1), frames[1]);
    }
}
