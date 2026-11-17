use super::*;

impl Document {
    /// Move selected frames as a block before a gap in the original timeline.
    /// The gap at the frame count appends the block. Tag ranges stay in place.
    pub fn move_frames(&mut self, frames: &[usize], before: usize) -> Result<bool, String> {
        let count = self.durations.len();
        if before > count {
            return Err("Choose a valid frame insertion point".into());
        }
        let selected = self.sorted_frame_selection(frames)?;
        let mut gaps = (0..count)
            .filter(|frame| selected.binary_search(frame).is_err())
            .collect::<Vec<_>>();
        let insertion = before - selected.partition_point(|&frame| frame < before);
        gaps.splice(insertion..insertion, selected);
        Ok(self.apply_frame_order(&gaps))
    }

    /// Reverse animation frames within the selected slots, keeping tag ranges in place.
    pub fn reverse_frames(&mut self, frames: &[usize]) -> Result<bool, String> {
        let selected = self.sorted_frame_selection(frames)?;
        let mut order = (0..self.durations.len()).collect::<Vec<_>>();
        for (&to, &from) in selected.iter().zip(selected.iter().rev()) {
            order[to] = from;
        }
        Ok(self.apply_frame_order(&order))
    }

    fn sorted_frame_selection(&self, frames: &[usize]) -> Result<Vec<usize>, String> {
        if frames.is_empty() || frames.len() > self.durations.len() {
            return Err("Choose at least one valid frame".into());
        }
        let mut selected = frames.to_vec();
        selected.sort_unstable();
        if selected.last() >= Some(&self.durations.len())
            || selected.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err("Choose distinct frames in the timeline".into());
        }
        Ok(selected)
    }

    fn apply_frame_order(&mut self, order: &[usize]) -> bool {
        let mut positions = vec![0; order.len()];
        for (dest, &source) in order.iter().enumerate() {
            positions[source] = dest;
        }
        let swaps = frame_order_swaps(positions.clone());
        if swaps.is_empty() {
            return false;
        }

        for chunk in &mut self.metadata {
            if chunk.is_frame_independent() {
                continue;
            }
            let Some(&destination) = positions.get(chunk.frame) else {
                continue;
            };
            let source = chunk.frame;
            chunk.frame = chunk
                .cel_user_data_layer(&self.layers)
                .and_then(|layer| {
                    let link = layer.cel_meta[source].artwork_link?;
                    if layer.cel_meta[..source]
                        .iter()
                        .any(|cel| cel.artwork_link == Some(link))
                    {
                        return None;
                    }
                    layer.cel_meta[source..]
                        .iter()
                        .zip(&positions[source..])
                        .filter(|(cel, _)| cel.artwork_link == Some(link))
                        .map(|(_, &slot)| slot)
                        .min()
                })
                .unwrap_or(destination);
        }

        // Use the same permutation on each dense frame array. Move the contents
        // in place so a long animation does not require copying its artwork.
        swap_frames(&mut self.explicit_frame_export_ids().frames, &swaps);
        swap_frames(&mut self.durations, &swaps);
        if !self.frame_names.is_empty() {
            self.frame_names.resize_with(order.len(), String::new);
            swap_frames(&mut self.frame_names, &swaps);
            self.prune_frame_names();
        }
        if !self.frame_palettes.is_empty() {
            let explicit_count = self.frame_palettes.len();
            self.frame_palettes
                .resize_with(order.len(), || self.palette.clone());
            swap_frames(&mut self.frame_palettes, &swaps);
            let last_explicit = order.iter().rposition(|&frame| frame < explicit_count);
            self.frame_palettes
                .truncate(last_explicit.map_or(0, |frame| frame + 1));
        }
        for layer in &mut self.layers {
            swap_frames(&mut layer.cels, &swaps);
            swap_frames(&mut layer.cel_meta, &swaps);
            swap_frames(&mut layer.tile_cels, &swaps);
        }
        for slice in &mut self.slices {
            for key in &mut slice.keys {
                key.frame = positions[key.frame];
            }
            slice.keys.sort_unstable_by_key(|key| key.frame);
        }
        true
    }
}

fn frame_order_swaps(mut destinations: Vec<usize>) -> Vec<(usize, usize)> {
    let mut swaps = Vec::new();
    for slot in 0..destinations.len() {
        while destinations[slot] != slot {
            let other = destinations[slot];
            swaps.push((slot, other));
            destinations.swap(slot, other);
        }
    }
    swaps
}

fn swap_frames<T>(frames: &mut [T], swaps: &[(usize, usize)]) {
    for &(a, b) in swaps {
        frames.swap(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_nonadjacent_frames_keeps_cels_and_export_metadata_together() {
        let mut doc = Document::new(2, 1);
        for frame in 0..4 {
            doc.add_frame(frame, false);
        }
        for frame in 0..5 {
            doc.durations[frame] = 30 + frame as u16 * 10;
            doc.set_pixel(0, frame, 0, [frame as u8, 32, 145, 255]);
        }
        doc.set_frame_name(1, "Windup").unwrap();
        doc.link_cel(0, 3, 1).unwrap();
        doc.frame_palettes = vec![doc.palette.clone(), vec![[23, 45, 67, 255]]];
        doc.slices.push(SpriteSlice {
            name: "Hit area".into(),
            keys: vec![
                SliceKey {
                    frame: 0,
                    bounds: [0, 0, 1, 1],
                    center: None,
                    pivot: None,
                },
                SliceKey {
                    frame: 3,
                    bounds: [1, 0, 1, 1],
                    center: None,
                    pivot: Some([1, 0]),
                },
            ],
        });
        doc.metadata.push(MetadataChunk {
            frame: 1,
            kind: 0x2020,
            data: vec![1, 2, 3],
            owner: "cel:0".into(),
        });
        let before = doc.clone();

        assert!(doc.move_frames(&[3, 1], 5).unwrap());
        for (target, source) in [0, 2, 4, 1, 3].into_iter().enumerate() {
            assert_eq!(doc.frame_export_id(target), source as u32);
            assert_eq!(doc.durations[target], before.durations[source]);
            assert_eq!(doc.palette_for(target), before.palette_for(source));
            assert_eq!(doc.layers[0].cels[target], before.layers[0].cels[source]);
            assert_eq!(
                doc.layers[0].cel_meta[target].editor_id.to_string(),
                before.layers[0].cel_meta[source].editor_id.to_string()
            );
        }
        let cels = &doc.layers[0].cel_meta;
        assert!(cels[3].artwork_link.is_some());
        assert_eq!(cels[3].artwork_link, cels[4].artwork_link);
        assert_eq!(doc.frame_name(3), Some("Windup"));
        assert_eq!(doc.slices[0].keys[1].frame, 4);
        assert_eq!(doc.metadata[0].frame, 3);
        doc.validate().unwrap();
    }

    #[test]
    fn reversing_selected_frames_preserves_unselected_positions_and_tag_boundaries() {
        let mut doc = Document::new(1, 1);
        for frame in 0..5 {
            doc.add_frame(frame, false);
        }
        doc.tags.push(FrameTag {
            editor_id: Default::default(),
            name: "Run".into(),
            from: 1,
            to: 4,
            direction: 2,
            repeat: 3,
            color: [1, 2, 3],
        });
        let before = doc.clone();
        assert!(doc.reverse_frames(&[5, 1, 3]).unwrap());
        assert_eq!(
            (0..6).map(|f| doc.frame_export_id(f)).collect::<Vec<_>>(),
            [0, 5, 2, 3, 4, 1]
        );
        assert_eq!(doc.tags, before.tags);
        assert!(doc.reverse_frames(&[1, 3, 5]).unwrap());
        assert_eq!(doc, before);
        assert!(!doc.move_frames(&[1, 2], 2).unwrap());
        assert!(doc.move_frames(&[1, 1], 0).is_err());
        assert!(doc.move_frames(&[0], 7).is_err());
        assert!(doc.reverse_frames(&[6]).is_err());
        assert_eq!(doc, before);
    }
}
