use super::{CelRange, Document, EditorId};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CelSelection {
    Rectangle(CelRange),
    Individual(Arc<BTreeSet<(usize, usize)>>),
}

impl CelSelection {
    pub fn contains(&self, layer: usize, frame: usize) -> bool {
        match self {
            Self::Rectangle(range) => range.contains(layer, frame),
            Self::Individual(cells) => cells.contains(&(layer, frame)),
        }
    }

    pub fn cells(&self) -> Box<dyn Iterator<Item = (usize, usize)> + '_> {
        match self {
            Self::Rectangle(range) => Box::new((range.first_layer..=range.last_layer).flat_map(
                move |layer| {
                    (range.first_frame..=range.last_frame).map(move |frame| (layer, frame))
                },
            )),
            Self::Individual(cells) => Box::new(cells.iter().copied()),
        }
    }

    pub fn bounds(&self) -> Option<CelRange> {
        match self {
            Self::Rectangle(range) => Some(*range),
            Self::Individual(cells) => {
                let first_layer = cells.first()?.0;
                let last_layer = cells.last()?.0;
                let first_frame = cells.iter().map(|&(_, frame)| frame).min()?;
                let last_frame = cells.iter().map(|&(_, frame)| frame).max()?;
                Some(CelRange {
                    first_layer,
                    last_layer,
                    first_frame,
                    last_frame,
                })
            }
        }
    }

    fn layers(&self) -> Box<dyn Iterator<Item = usize> + '_> {
        match self {
            Self::Rectangle(range) => Box::new(range.first_layer..=range.last_layer),
            Self::Individual(cells) => {
                let mut last_layer = None;
                Box::new(cells.iter().filter_map(move |&(layer, _)| {
                    (last_layer.replace(layer) != Some(layer)).then_some(layer)
                }))
            }
        }
    }
}

fn cel_chunk_position(owner: &str, frame: usize) -> Option<(usize, usize)> {
    Some((owner.strip_prefix("cel:")?.parse().ok()?, frame))
}

impl Document {
    pub fn clear_cel_range(&mut self, range: CelRange) -> Result<(), String> {
        self.clear_selected_cels(&CelSelection::Rectangle(range))
    }

    pub fn clear_selected_cels(&mut self, selection: &CelSelection) -> Result<(), String> {
        self.validate_cel_selection(selection)?;
        for (layer, frame) in selection.cells() {
            self.clear_cel(layer, frame);
        }
        for layer in selection.layers() {
            self.layers[layer].prune_cel_links();
        }
        self.metadata.retain(|chunk| {
            cel_chunk_position(&chunk.owner, chunk.frame)
                .is_none_or(|(layer, frame)| !selection.contains(layer, frame))
        });
        Ok(())
    }

    fn validate_cel_selection(&self, selection: &CelSelection) -> Result<CelRange, String> {
        selection
            .bounds()
            .filter(|range| {
                range.fits(self) && selection.layers().all(|layer| self.editable(layer))
            })
            .ok_or_else(|| "Choose cels in unlocked image or tile layers".into())
    }

    pub fn cel_selection_destination(
        &self,
        selection: &CelSelection,
        to_layer: usize,
        to_frame: usize,
    ) -> Result<CelRange, String> {
        let source = self.validate_cel_selection(selection)?;
        let target = CelRange {
            first_layer: to_layer,
            last_layer: to_layer.saturating_add(source.last_layer - source.first_layer),
            first_frame: to_frame,
            last_frame: to_frame.saturating_add(source.last_frame - source.first_frame),
        };
        if !target.fits(self) {
            return Err("Destination is outside the timeline".into());
        }
        for layer in selection.layers() {
            let dest = to_layer + layer - source.first_layer;
            if !self.editable(dest) {
                return Err("Destination contains a locked or group layer".into());
            }
            if self.layers[layer].tileset != self.layers[dest].tileset {
                return Err(
                    "Source and destination layers must use the same image or tile format".into(),
                );
            }
        }
        Ok(target)
    }

    pub fn move_cel_range(
        &mut self,
        range: CelRange,
        to_layer: usize,
        to_frame: usize,
        duplicate: bool,
    ) -> Result<(), String> {
        self.move_selected_cels(
            &CelSelection::Rectangle(range),
            to_layer,
            to_frame,
            duplicate,
        )
    }

    pub fn move_selected_cels(
        &mut self,
        selection: &CelSelection,
        to_layer: usize,
        to_frame: usize,
        duplicate: bool,
    ) -> Result<(), String> {
        if let Some(next) = self.with_moved_cels(selection, to_layer, to_frame, duplicate)? {
            *self = next;
        }
        Ok(())
    }

    pub fn with_moved_cels(
        &self,
        selection: &CelSelection,
        to_layer: usize,
        to_frame: usize,
        duplicate: bool,
    ) -> Result<Option<Self>, String> {
        let target = self.cel_selection_destination(selection, to_layer, to_frame)?;
        let source = selection.bounds().ok_or("Choose at least one cel")?;
        if source == target {
            return Ok(None);
        }
        let translate = |layer, frame| {
            (
                to_layer + layer - source.first_layer,
                to_frame + frame - source.first_frame,
            )
        };
        let moving_into_selection = |layer, frame| {
            target.contains(layer, frame)
                && selection.contains(
                    source.first_layer + layer - to_layer,
                    source.first_frame + frame - to_frame,
                )
        };
        let link_duplicate =
            |source, dest: usize| duplicate && source == dest && self.layers[dest].link_duplicates;
        if duplicate
            && selection.cells().any(|(layer, frame)| {
                let (dest, target_frame) = translate(layer, frame);
                link_duplicate(layer, dest) && selection.contains(dest, target_frame)
            })
        {
            return Err("Choose a non-overlapping destination when duplicating linked cels".into());
        }

        let mut next = self.clone();
        if !duplicate {
            for (layer, frame) in selection.cells() {
                next.clear_cel(layer, frame);
                next.layers[layer].cel_meta[frame].editor_id = EditorId::default();
            }
        }
        next.metadata.retain(|chunk| {
            cel_chunk_position(&chunk.owner, chunk.frame).is_none_or(|(layer, frame)| {
                !(moving_into_selection(layer, frame)
                    || !duplicate && selection.contains(layer, frame))
            })
        });
        for (layer, frame) in selection.cells() {
            let (dest, target_frame) = translate(layer, frame);
            let source_layer = &self.layers[layer];
            let dest_layer = &mut next.layers[dest];
            dest_layer.cels[target_frame] = source_layer.cels[frame].clone();
            dest_layer.tile_cels[target_frame] = source_layer.tile_cels[frame].clone();
            let mut meta = source_layer.cel_meta[frame].clone();
            if duplicate {
                meta.editor_id = dest_layer.cel_meta[target_frame].editor_id;
                meta.image_id = EditorId::default();
            }
            if layer != dest || duplicate && !dest_layer.link_duplicates {
                meta.artwork_link = None;
            }
            dest_layer.cel_meta[target_frame] = meta;
        }
        if duplicate {
            for (layer, frame) in selection.cells() {
                let (dest, target_frame) = translate(layer, frame);
                if link_duplicate(layer, dest) {
                    next.set_cel_link(dest, target_frame, frame);
                }
            }
        }
        for chunk in &self.metadata {
            if let Some((layer, frame)) = cel_chunk_position(&chunk.owner, chunk.frame)
                && selection.contains(layer, frame)
            {
                let (dest, target_frame) = translate(layer, frame);
                let mut chunk = chunk.clone();
                chunk.owner = format!("cel:{dest}");
                chunk.frame = target_frame;
                next.metadata.push(chunk);
            }
        }
        for layer in &mut next.layers {
            layer.prune_cel_links();
        }
        next.validate()?;
        Ok(Some(next))
    }

    pub fn set_range_duration(&mut self, range: CelRange, duration: u16) -> Result<(), String> {
        self.set_selected_cel_duration(&CelSelection::Rectangle(range), duration)
    }

    pub fn set_selected_cel_duration(
        &mut self,
        selection: &CelSelection,
        duration: u16,
    ) -> Result<(), String> {
        let Some(range) = selection
            .bounds()
            .filter(|range| duration != 0 && range.fits(self))
        else {
            return Err("Choose a valid timeline range and positive duration".into());
        };
        match selection {
            CelSelection::Rectangle(_) => {
                self.durations[range.first_frame..=range.last_frame].fill(duration);
            }
            CelSelection::Individual(cells) => {
                for &(_, frame) in cells.iter() {
                    self.durations[frame] = duration;
                }
            }
        }
        Ok(())
    }

    pub fn set_range_linked(&mut self, range: CelRange, linked: bool) -> Result<(), String> {
        self.set_selected_cels_linked(&CelSelection::Rectangle(range), linked)
    }

    pub fn set_selected_cels_linked(
        &mut self,
        selection: &CelSelection,
        linked: bool,
    ) -> Result<(), String> {
        self.validate_cel_selection(selection)?;
        let mut source_layer = None;
        let mut source_frame = 0;
        for (layer, frame) in selection.cells() {
            if !linked {
                self.unlink_cel(layer, frame)?;
            } else if source_layer == Some(layer) {
                self.link_cel(layer, frame, source_frame)?;
            } else {
                source_layer = Some(layer);
                source_frame = frame;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MetadataChunk;

    #[test]
    fn individual_operations_skip_unselected_rows_frames_and_metadata() {
        let mut doc = Document::new(1, 1);
        for frame in 0..5 {
            doc.add_frame(frame, false);
        }
        doc.add_layer();
        doc.add_layer();
        doc.layers[1].locked = true;
        for (layer, frame) in [(0, 0), (0, 2), (1, 1), (2, 1)] {
            doc.set_pixel(layer, frame, 0, [25, 100, 115, 255]);
            doc.metadata.push(MetadataChunk {
                frame,
                kind: 0x2020,
                data: vec![],
                owner: format!("cel:{layer}"),
            });
        }
        let selected = CelSelection::Individual(Arc::new([(0, 0), (0, 2), (2, 1)].into()));
        let before = doc.clone();
        assert!(doc.move_selected_cels(&selected, 1, 0, false).is_err());
        assert_eq!(doc, before);
        doc.set_selected_cel_duration(&selected, 40).unwrap();
        assert_eq!(doc.durations, [40, 40, 40, 100, 100, 100]);
        doc.set_selected_cels_linked(&selected, true).unwrap();
        assert_eq!(
            doc.layers[0].cel_meta[0].artwork_link,
            doc.layers[0].cel_meta[2].artwork_link
        );
        assert!(doc.layers[0].cel_meta[0].artwork_link.is_some());
        assert!(doc.layers[0].cel_meta[1].artwork_link.is_none());
        doc.move_selected_cels(&selected, 0, 3, false).unwrap();
        assert_eq!(
            doc.layers[0].cel_meta[3].artwork_link,
            doc.layers[0].cel_meta[5].artwork_link
        );
        assert!(doc.layers[0].cel_meta[3].artwork_link.is_some());
        assert_eq!(doc.metadata[0].owner, "cel:1");
        assert_eq!(doc.metadata[0].frame, 1);
        assert_eq!(
            doc.metadata[1..]
                .iter()
                .map(|chunk| chunk.frame)
                .collect::<Vec<_>>(),
            [3, 5, 4]
        );
        let moved = CelSelection::Individual(Arc::new([(0, 3), (0, 5), (2, 4)].into()));
        doc.clear_selected_cels(&moved).unwrap();
        assert_eq!(doc.metadata.len(), 1);
        assert_eq!(doc.metadata[0].owner, "cel:1");
    }
}
