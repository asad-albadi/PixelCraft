use super::{CLEAR, Pixel, Pixels};
use serde::ser::{SerializeSeq, Serializer};
use std::{
    collections::{BTreeMap, HashSet},
    ops::ControlFlow,
    sync::Arc,
};

pub(super) const PAGE_PIXELS: usize = 256;
type Pages = BTreeMap<usize, Arc<[Pixel; PAGE_PIXELS]>>;

/// Dense image with a shared, immutable base and copy-on-write edited pages.
#[derive(Clone, Debug)]
pub struct DenseOverlay {
    base: Arc<Vec<Pixel>>,
    pages: Pages,
}

impl DenseOverlay {
    pub(super) fn new(base: Arc<Vec<Pixel>>) -> Self {
        Self {
            base,
            pages: BTreeMap::new(),
        }
    }

    pub(super) fn len(&self) -> usize {
        self.base.len()
    }

    pub(super) fn needs_flattening(&self, index: usize) -> bool {
        let limit = (self.len() / (4 * PAGE_PIXELS)).min(1024);
        self.pages.len() >= limit && !self.pages.contains_key(&(index / PAGE_PIXELS))
    }

    pub(super) fn pixel(&self, index: usize) -> &Pixel {
        self.pages
            .get(&(index / PAGE_PIXELS))
            .map_or_else(|| &self.base[index], |page| &page[index % PAGE_PIXELS])
    }

    pub(super) fn pixel_mut(&mut self, index: usize) -> &mut Pixel {
        let number = index / PAGE_PIXELS;
        let page = self.pages.entry(number).or_insert_with(|| {
            let mut page = [CLEAR; PAGE_PIXELS];
            let offset = number * PAGE_PIXELS;
            let source = &self.base[offset..(offset + PAGE_PIXELS).min(self.base.len())];
            page[..source.len()].copy_from_slice(source);
            Arc::new(page)
        });
        &mut Arc::make_mut(page)[index % PAGE_PIXELS]
    }

    pub(super) fn slices(&self) -> impl Iterator<Item = &[Pixel]> {
        self.base
            .chunks(PAGE_PIXELS)
            .enumerate()
            .map(|(index, base)| {
                self.pages
                    .get(&index)
                    .map_or(base, |page| &page[..base.len()])
            })
    }

    pub(super) fn to_vec(&self) -> Vec<Pixel> {
        let mut pixels = self.base.as_ref().clone();
        for (&page, edited) in &self.pages {
            let offset = page * PAGE_PIXELS;
            let end = (offset + PAGE_PIXELS).min(pixels.len());
            pixels[offset..end].copy_from_slice(&edited[..end - offset]);
        }
        pixels
    }

    pub(super) fn copy_range(&self, mut start: usize, mut target: &mut [Pixel]) {
        while !target.is_empty() {
            let index = start / PAGE_PIXELS;
            let offset = start % PAGE_PIXELS;
            let count = target.len().min(PAGE_PIXELS - offset);
            let (prefix, rest) = target.split_at_mut(count);
            let source = self.pages.get(&index).map_or_else(
                || &self.base[start..start + count],
                |page| &page[offset..offset + count],
            );
            prefix.copy_from_slice(source);
            start += count;
            target = rest;
        }
    }

    pub(super) fn allocated(&self, seen: &mut HashSet<usize>) -> usize {
        let base = if seen.insert(Arc::as_ptr(&self.base) as usize) {
            self.base.len()
        } else {
            0
        };
        let pages = self
            .pages
            .values()
            .filter(|page| seen.insert(Arc::as_ptr(page) as usize))
            .count();
        base + pages * PAGE_PIXELS + self.pages.len() * 12
    }
}

// Old project readers see a regular dense raster. The overlay is runtime-only.
pub(super) fn serialize<S: Serializer>(
    raster: &DenseOverlay,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(raster.len()))?;
    for pixel in raster.slices().flatten() {
        sequence.serialize_element(pixel)?;
    }
    sequence.end()
}

impl Pixels {
    pub(crate) fn dense_base_pixels(&self) -> Option<&[Pixel]> {
        self.dense_base().map(|(base, _)| base.as_slice())
    }

    pub(crate) fn visit_dense_edits(&self, mut visit: impl FnMut(usize, Pixel)) {
        if let Self::Overlay(overlay) = self {
            for (&index, edited) in &overlay.pages {
                let offset = index * PAGE_PIXELS;
                let original = &overlay.base[offset..(offset + PAGE_PIXELS).min(overlay.len())];
                for (slot, (&old, &new)) in original.iter().zip(edited.iter()).enumerate() {
                    if old != new {
                        visit(offset + slot, new);
                    }
                }
            }
        }
    }

    fn dense_base(&self) -> Option<(&Arc<Vec<Pixel>>, Option<&Pages>)> {
        match self {
            Self::Dense(base) => Some((base, None)),
            Self::Overlay(overlay) => Some((&overlay.base, Some(&overlay.pages))),
            _ => None,
        }
    }

    // Only inspect edited pages when both snapshots refer to the same dense base.
    pub(super) fn visit_shared_dense_changes(
        &self,
        other: &Self,
        mut visit: impl FnMut(usize) -> ControlFlow<()>,
    ) -> Option<ControlFlow<()>> {
        let (base, before) = self.dense_base()?;
        let (other_base, after) = other.dense_base()?;
        if !Arc::ptr_eq(base, other_base) {
            return None;
        }
        let mut compare = |index: usize,
                           before: Option<&Arc<[Pixel; PAGE_PIXELS]>>,
                           after: Option<&Arc<[Pixel; PAGE_PIXELS]>>| {
            if before
                .zip(after)
                .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
            {
                return ControlFlow::Continue(());
            }
            let offset = index * PAGE_PIXELS;
            let original = &base[offset..(offset + PAGE_PIXELS).min(base.len())];
            let before = before.map_or(original, |page| &page[..original.len()]);
            let after = after.map_or(original, |page| &page[..original.len()]);
            for (slot, (before, after)) in before.iter().zip(after).enumerate() {
                if before != after {
                    visit(offset + slot)?;
                }
            }
            ControlFlow::Continue(())
        };
        for (&index, page) in before.into_iter().flatten() {
            if compare(index, Some(page), after.and_then(|pages| pages.get(&index))).is_break() {
                return Some(ControlFlow::Break(()));
            }
        }
        for (&index, page) in after.into_iter().flatten() {
            if before.is_none_or(|pages| !pages.contains_key(&index))
                && compare(index, None, Some(page)).is_break()
            {
                return Some(ControlFlow::Break(()));
            }
        }
        Some(ControlFlow::Continue(()))
    }
}
