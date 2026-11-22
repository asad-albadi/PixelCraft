//! Platform-independent editor session, selection and history.
use crate::model::*;
mod selection_mask;
pub use selection_mask::SelectionMask;
mod stroke;
pub use stroke::{BrushDynamics, StrokeStabilizer};
#[derive(Clone)]
pub struct HistoryEntry {
    pub label: &'static str,
    pub doc: Document,
    pub selection: Option<(i32, i32, i32, i32)>,
    pub mask: Option<SelectionMask>,
    pub polygon: Vec<(i32, i32)>,
    pub layer: usize,
    pub frame: usize,
    pub selected_tile: u32,
    pub timeline_anchor: Option<(usize, usize)>,
    pub selected_cels: Option<CelSelection>,
    pub timeline_frames: TimelineHeaders,
    pub timeline_layers: TimelineHeaders,
}
#[derive(Clone, Default)]
pub struct TimelineHeaders {
    pub anchor: Option<usize>,
    pub selected: Vec<usize>,
}
impl TimelineHeaders {
    pub fn toggle(&mut self, index: usize) {
        match self.selected.binary_search(&index) {
            Ok(position) => {
                self.selected.remove(position);
            }
            Err(position) => self.selected.insert(position, index),
        }
        self.anchor = Some(index);
    }
}

#[derive(Clone)]
pub struct EditorSession {
    pub doc: Document,
    pub saved: Document,
    pub layer: usize,
    pub frame: usize,
    pub selected_tile: u32,
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub selection: Option<(i32, i32, i32, i32)>,
    pub mask: Option<SelectionMask>,
    pub polygon: Vec<(i32, i32)>,
    pub timeline_anchor: Option<(usize, usize)>,
    pub selected_cels: Option<CelSelection>,
    pub timeline_frames: TimelineHeaders,
    pub timeline_layers: TimelineHeaders,
}

impl EditorSession {
    pub fn new(doc: Document) -> Self {
        Self {
            saved: doc.clone(),
            doc,
            layer: 0,
            frame: 0,
            selected_tile: 0,
            undo: vec![],
            redo: vec![],
            selection: None,
            mask: None,
            polygon: vec![],
            timeline_anchor: None,
            selected_cels: None,
            timeline_frames: TimelineHeaders::default(),
            timeline_layers: TimelineHeaders::default(),
        }
    }
    pub fn remember_action(&mut self, label: &'static str) {
        self.redo.clear();
        self.push_undo(self.doc.clone(), label);
    }
    pub fn push_undo(&mut self, d: Document, label: &'static str) {
        self.undo.push(self.history_entry(d, label));
        while self.undo.len() > 1 && (self.undo.len() > 64 || self.history_bytes() > 192_000_000) {
            self.undo.remove(0);
        }
    }
    pub fn history_bytes(&self) -> usize {
        Self::history_document_bytes(
            self.undo
                .iter()
                .map(|entry| &entry.doc)
                .chain(self.redo.iter().map(|entry| &entry.doc))
                .chain(std::iter::once(&self.doc))
                .chain(std::iter::once(&self.saved)),
        )
    }
    pub fn history_document_bytes<'a>(documents: impl Iterator<Item = &'a Document>) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut bytes = 0usize;
        for d in documents {
            for l in &d.layers {
                for p in l.cels.iter().chain(l.mask.iter()) {
                    bytes = bytes.saturating_add(p.unique_allocation_pixels(&mut seen) * 4);
                }
                if !l.mask_outside.is_empty()
                    && seen.insert(std::sync::Arc::as_ptr(&l.mask_outside) as usize)
                {
                    bytes = bytes
                        .saturating_add(l.mask_outside.len() * std::mem::size_of::<OutsidePixel>());
                }
                for m in &l.cel_meta {
                    if let Some(p) = &m.indices
                        && seen.insert(std::sync::Arc::as_ptr(p) as usize)
                    {
                        bytes = bytes.saturating_add(p.len());
                    }
                    bytes =
                        bytes.saturating_add(m.outside.len() * std::mem::size_of::<OutsidePixel>());
                }
                for cells in &l.tile_cels {
                    if !cells.is_empty() && seen.insert(cells.backing_id()) {
                        bytes = bytes.saturating_add(cells.len() * 4);
                    }
                }
            }
            for t in &d.tilesets {
                for p in &t.tiles {
                    bytes = bytes.saturating_add(p.pixels.unique_allocation_pixels(&mut seen) * 4);
                }
            }
            for r in &d.references {
                bytes = bytes.saturating_add(r.pixels.unique_allocation_pixels(&mut seen) * 4);
            }
            bytes = bytes.saturating_add(d.metadata.iter().map(|m| m.data.len()).sum::<usize>());
        }
        bytes
    }
    pub fn history_entry(&self, doc: Document, label: &'static str) -> HistoryEntry {
        HistoryEntry {
            label,
            doc,
            selection: self.selection,
            mask: self.mask.clone(),
            polygon: self.polygon.clone(),
            layer: self.layer,
            frame: self.frame,
            selected_tile: self.selected_tile,
            timeline_anchor: self.timeline_anchor,
            selected_cels: self.selected_cels.clone(),
            timeline_frames: self.timeline_frames.clone(),
            timeline_layers: self.timeline_layers.clone(),
        }
    }
    pub fn restore_history(&mut self, mut entry: HistoryEntry) -> HistoryEntry {
        entry.doc.retain_frame_export_id_counter(&self.doc);
        let previous = std::mem::replace(&mut self.doc, entry.doc);
        let current = self.history_entry(previous, entry.label);
        self.layer = entry.layer.min(self.doc.layers.len() - 1);
        self.frame = entry.frame.min(self.doc.durations.len() - 1);
        self.selected_tile = entry.selected_tile;
        self.timeline_anchor = entry.timeline_anchor;
        self.selected_cels = entry.selected_cels;
        self.timeline_frames = entry.timeline_frames;
        self.timeline_layers = entry.timeline_layers;
        self.selection = entry.selection;
        self.mask = entry.mask;
        self.polygon = entry.polygon;
        current
    }
    pub fn history_label(&self, position: usize) -> Option<&'static str> {
        if position == 0 {
            Some("Oldest retained state")
        } else if position <= self.undo.len() {
            self.undo.get(position - 1).map(|entry| entry.label)
        } else {
            let index = (self.undo.len() + self.redo.len()).checked_sub(position)?;
            self.redo.get(index).map(|entry| entry.label)
        }
    }
    pub fn jump_history(&mut self, position: usize) -> bool {
        self.jump_history_with(position, |_| {})
    }

    /// Notify a host after each restore, including intermediate jump states.
    pub fn jump_history_with(
        &mut self,
        position: usize,
        mut restored: impl FnMut(&Document),
    ) -> bool {
        if position == self.undo.len() || position > self.undo.len() + self.redo.len() {
            return false;
        }
        while self.undo.len() > position {
            let entry = self.undo.pop().expect("history target is valid");
            let previous = self.restore_history(entry);
            self.redo.push(previous);
            restored(&self.doc);
        }
        while self.undo.len() < position {
            let entry = self.redo.pop().expect("history target is valid");
            let previous = self.restore_history(entry);
            self.undo.push(previous);
            restored(&self.doc);
        }
        true
    }
}
