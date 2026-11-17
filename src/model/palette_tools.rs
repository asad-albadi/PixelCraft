use super::{ColorMode, Document};
use crate::color::rgb_to_hsv;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteSort {
    #[default]
    Hue,
    Saturation,
    Value,
    Luminance,
    Rgb,
    Alpha,
    Name,
}

impl Document {
    /// Sort selected swatches in place, or the whole palette for an empty selection.
    pub fn sort_palette(
        &mut self,
        selection: &[usize],
        sort: PaletteSort,
    ) -> Result<Vec<usize>, String> {
        self.sort_palette_by_frame(selection, sort, None)
    }

    pub fn sort_palette_by_frame(
        &mut self,
        selection: &[usize],
        sort: PaletteSort,
        frame: Option<usize>,
    ) -> Result<Vec<usize>, String> {
        let palette = frame.map_or(self.palette.as_slice(), |frame| self.palette_for(frame));
        let mut positions = if selection.is_empty() {
            (0..palette.len().min(self.palette.len())).collect()
        } else {
            self.palette_positions(selection)?
        };
        if positions.iter().any(|&slot| slot >= palette.len()) {
            return Err("The current frame palette is missing a selected swatch".into());
        }
        if self.color_mode == ColorMode::Indexed {
            positions.retain(|&position| position != self.transparent_index);
        }
        let mut sources = positions.clone();
        let scaled = |value: f32| (value * 65_535.).round() as u32;
        sources.sort_by(|&a, &b| {
            if sort == PaletteSort::Name {
                return self
                    .palette_names
                    .get(a)
                    .map_or("", String::as_str)
                    .cmp(self.palette_names.get(b).map_or("", String::as_str));
            }
            let key = |index| {
                let pixel = palette[index];
                match sort {
                    PaletteSort::Hue => scaled(rgb_to_hsv(pixel)[0]),
                    PaletteSort::Saturation => scaled(rgb_to_hsv(pixel)[1]),
                    PaletteSort::Value => scaled(rgb_to_hsv(pixel)[2]),
                    PaletteSort::Luminance => {
                        u32::from(pixel[0]) * 2126
                            + u32::from(pixel[1]) * 7152
                            + u32::from(pixel[2]) * 722
                    }
                    PaletteSort::Rgb => u32::from_be_bytes(pixel) >> 8,
                    PaletteSort::Alpha => u32::from(pixel[3]),
                    PaletteSort::Name => unreachable!("Names are sorted as strings"),
                }
            };
            key(a).cmp(&key(b))
        });
        let mut order: Vec<_> = (0..self.palette.len()).collect();
        for (position, source) in positions.into_iter().zip(sources) {
            order[position] = source;
        }
        self.permute_palette(&order)
    }

    pub fn reverse_palette_entries(&mut self, selection: &[usize]) -> Result<Vec<usize>, String> {
        let positions = self.palette_positions(selection)?;
        let mut order: Vec<_> = (0..self.palette.len()).collect();
        for (&position, &source) in positions.iter().zip(positions.iter().rev()) {
            order[position] = source;
        }
        self.permute_palette(&order)
    }

    fn palette_positions(&self, selection: &[usize]) -> Result<Vec<usize>, String> {
        let mut positions = selection.to_vec();
        positions.sort_unstable();
        positions.dedup();
        if positions.iter().any(|&slot| slot >= self.palette.len()) {
            return Err("Invalid palette position".into());
        }
        Ok(positions)
    }

    pub(super) fn permute_palette(&mut self, order: &[usize]) -> Result<Vec<usize>, String> {
        let mut remap = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            remap[old] = new;
        }
        if order.iter().copied().eq(0..order.len()) {
            return Ok(remap);
        }
        if self.frame_palettes.iter().any(|palette| {
            !palette.is_empty()
                && order
                    .iter()
                    .take(palette.len())
                    .any(|&index| index >= palette.len())
        }) {
            return Err("A frame palette is missing swatches needed for this reorder".into());
        }
        let reorder = |palette: &mut Vec<super::Pixel>| {
            let before = palette.clone();
            for (new, &old) in palette.iter_mut().zip(order) {
                *new = before[old];
            }
        };
        reorder(&mut self.palette);
        for palette in &mut self.frame_palettes {
            reorder(palette);
        }
        self.palette_names.resize(order.len(), String::new());
        let names = std::mem::take(&mut self.palette_names);
        self.palette_names = order.iter().map(|&slot| names[slot].clone()).collect();
        let map = |slot| remap.get(slot).copied().unwrap_or(slot);
        self.transparent_index = map(self.transparent_index);
        self.drawing_index = self.drawing_index.map(map);
        let mut shared = HashMap::new();
        for layer in &mut self.layers {
            for cel in &mut layer.cel_meta {
                if let Some(indices) = &mut cel.indices {
                    let mapped = shared.entry(Arc::as_ptr(indices)).or_insert_with(|| {
                        Arc::new(
                            indices
                                .iter()
                                .map(|&slot| map(slot as usize) as u8)
                                .collect(),
                        )
                    });
                    *indices = Arc::clone(mapped);
                }
            }
        }
        Ok(remap)
    }
}
