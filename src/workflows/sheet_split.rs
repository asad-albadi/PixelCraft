use super::{SheetOptions, SheetSpacing, pack_sheet_with_extrusion, scaled_image_dimensions};
use crate::model::*;
use image::RgbaImage;
use serde_json::{Value, json};
use std::ops::RangeInclusive;

pub(crate) struct SheetCrop<'a> {
    pub size: [usize; 2],
    pub name: &'a str,
    pub keys: &'a [&'a SliceKey],
}

pub(crate) struct SplitSheetWork {
    pub samples: usize,
    pub source_pixels: usize,
    pub cropped_pixels: usize,
    pub atlas_pixels: usize,
    pub groups: usize,
    pub layer_label_bytes: usize,
}

pub(crate) fn split_sheet_work(
    source: &Document,
    options: &SheetOptions,
    extrude: usize,
    frames: &RangeInclusive<usize>,
    size: Option<[usize; 2]>,
) -> Result<SplitSheetWork, String> {
    let layout = SplitLayout::new(source, options, extrude, frames, size)?;
    Ok(SplitSheetWork {
        samples: layout.samples,
        source_pixels: layout.layers.len() * frames.clone().count() * source.width * source.height,
        cropped_pixels: layout.samples * layout.width * layout.height,
        atlas_pixels: layout.atlas_pixels,
        groups: layout.layers.len() * layout.cells.len(),
        layer_label_bytes: layout.layer_label_bytes,
    })
}

pub(crate) fn pack_split_sheet(
    source: &Document,
    options: &SheetOptions,
    extrude: usize,
    frames: RangeInclusive<usize>,
    crop: Option<SheetCrop<'_>>,
) -> Result<(RgbaImage, Value), String> {
    let layout = SplitLayout::new(
        source,
        options,
        extrude,
        &frames,
        crop.as_ref().map(|crop| crop.size),
    )?;
    let tags = layout.tags(source, &frames);
    let mut projected = Document::new(layout.width, layout.height);
    projected.drawing_grid = if options.split_grid {
        Some(DrawingGrid {
            origin: [0, 0],
            size: [layout.width as u32, layout.height as u32],
        })
    } else {
        source.drawing_grid
    };
    projected.durations = frames
        .clone()
        .cycle()
        .take(layout.samples)
        .map(|frame| source.durations[frame])
        .collect();
    projected.layers[0]
        .cel_meta
        .resize_with(layout.samples, CelMeta::default);
    projected.layers[0]
        .tile_cels
        .resize_with(layout.samples, Vec::new);
    projected.layers[0].cels = vec![Pixels::empty(layout.width * layout.height); layout.samples];
    let first = *frames.start();
    let span = frames.clone().count();
    for (layer_offset, &layer) in layout.layers.iter().enumerate() {
        for frame in frames.clone() {
            let pixels = layer.map_or_else(
                || source.composite(frame),
                |layer| source.composite_layer_for_export(frame, layer),
            );
            let area = crop
                .as_ref()
                .map_or([0, 0, source.width as i32, source.height as i32], |crop| {
                    crop.keys[frame - first].bounds
                });
            for (cell_offset, &[x, y]) in layout.cells.iter().enumerate() {
                let group = layer_offset * layout.cells.len() + cell_offset;
                let output = group * span + (frame - first);
                let cell = [x, y, layout.width, layout.height];
                projected.layers[0].cels[output] = sample_pixels(source, &pixels, area, cell);
                if let Some(name) = source.frame_name(frame) {
                    projected.set_frame_name(output, name)?;
                }
            }
        }
    }
    projected.tags = tags;
    let mut settings = options.clone();
    settings.tag = None;
    settings.frame_range = None;
    settings.layers = None;
    settings.split_layers = false;
    settings.split_grid = false;
    let (image, mut meta) = pack_sheet_with_extrusion(&projected, &settings, extrude)?;
    let entries = meta["frames"].as_array_mut().unwrap();
    let mut entries = entries.iter_mut();
    for &layer in &layout.layers {
        for &[x, y] in &layout.cells {
            let label = group_label(layer, options.split_grid.then_some([x, y]));
            for frame in frames.clone() {
                let item = entries.next().unwrap();
                let id = source.frame_export_id(frame);
                item["filename"] = json!(format!("{label}_frame_{id}"));
                item["sourceFrameId"] = json!(id);
                item["sourceFrame"] = json!(frame);
                if let Some(layer) = layer {
                    item["sourceLayer"] = json!(layer);
                    item["sourceLayerName"] = json!(source.layers[layer].name);
                }
                if options.split_grid {
                    item["gridCell"] = json!({"x":x,"y":y,"w":layout.width,"h":layout.height});
                }
            }
        }
    }
    if let Some(crop) = crop {
        meta["meta"]["sourceSlice"] = json!(crop.name);
    }
    Ok((image, meta))
}

struct SplitLayout {
    layers: Vec<Option<usize>>,
    cells: Vec<[usize; 2]>,
    grid: bool,
    width: usize,
    height: usize,
    samples: usize,
    atlas_pixels: usize,
    layer_label_bytes: usize,
}

impl SplitLayout {
    fn new(
        source: &Document,
        options: &SheetOptions,
        extrude: usize,
        frames: &RangeInclusive<usize>,
        size: Option<[usize; 2]>,
    ) -> Result<Self, String> {
        if options.scale == 0 || options.scale > MAX_DIMENSION || extrude > 256 {
            return Err("Invalid split sheet scale or extrusion".into());
        }
        let visible = |index| {
            let mut parent = Some(index);
            while let Some(index) = parent {
                let layer: &Layer = &source.layers[index];
                if !layer.visible {
                    return false;
                }
                parent = layer.parent;
            }
            true
        };
        let layers = if options.split_layers {
            source
                .layers
                .iter()
                .enumerate()
                .filter(|(i, layer)| !layer.group && layer.adjustment.is_none() && visible(*i))
                .map(|(i, _)| Some(i))
                .collect::<Vec<_>>()
        } else {
            vec![None]
        };
        if layers.is_empty() {
            return Err("No visible raster or tilemap layers to split".into());
        }
        let [canvas_width, canvas_height] = size.unwrap_or([source.width, source.height]);
        let (mut width, mut height) = (canvas_width, canvas_height);
        let (mut start_x, mut start_y) = (0, 0);
        if options.split_grid {
            let grid = source.drawing_grid();
            [width, height] = grid.size.map(|value| value as usize);
            start_x = grid.origin[0].rem_euclid(width as i32) as usize;
            start_y = grid.origin[1].rem_euclid(height as i32) as usize;
        }
        let columns = canvas_width.saturating_sub(start_x) / width;
        let rows = canvas_height.saturating_sub(start_y) / height;
        if columns == 0 || rows == 0 {
            return Err("No complete grid cells fit in the sheet source".into());
        }
        let span = frames.clone().count();
        let composites = layers.len().saturating_mul(span);
        let cell_count = columns.saturating_mul(rows);
        let samples = composites.saturating_mul(cell_count);
        if samples > u16::MAX as usize
            || samples.saturating_mul(width * height) > MAX_PIXELS
            || composites.saturating_mul(source.width * source.height) > MAX_DOCUMENT_PIXELS
        {
            return Err("Split sprite sheet exceeds the frame or pixel work limit".into());
        }
        let groups = layers.len() * cell_count;
        let active = active_tags(source, frames);
        let count = active.clone().count().max(1);
        let tag_bytes = active.map(|tag| tag.name.len()).sum::<usize>();
        let frame_label_bytes = frames
            .clone()
            .filter_map(|frame| source.frame_name(frame))
            .map(str::len)
            .sum::<usize>();
        let layer_label_bytes = layers
            .iter()
            .flatten()
            .map(|&layer| source.layers[layer].name.len())
            .sum::<usize>()
            .saturating_mul(span * cell_count);
        if groups.saturating_mul(count) > u16::MAX as usize
            || groups
                .saturating_mul(tag_bytes.saturating_add(frame_label_bytes))
                .saturating_add(layer_label_bytes)
                > 32 * 1024 * 1024
        {
            return Err("Split sprite sheet exceeds the animation metadata limit".into());
        }
        let packed_columns = options.columns.clamp(1, samples);
        let spacing = SheetSpacing::new(options, extrude)?;
        let (packed_width, packed_height) = (
            spacing.extent(width, packed_columns),
            spacing.extent(height, samples.div_ceil(packed_columns)),
        );
        if !options.trims() {
            scaled_image_dimensions(packed_width, packed_height, options.scale)?;
        }
        let atlas_pixels = packed_width
            .saturating_mul(packed_height)
            .saturating_mul(options.scale.saturating_mul(options.scale));
        let cells = (0..rows)
            .flat_map(|row| {
                (0..columns).map(move |column| [start_x + column * width, start_y + row * height])
            })
            .collect();
        Ok(Self {
            layers,
            cells,
            grid: options.split_grid,
            width,
            height,
            samples,
            atlas_pixels,
            layer_label_bytes,
        })
    }

    fn tags(&self, source: &Document, frames: &RangeInclusive<usize>) -> Vec<FrameTag> {
        let first = *frames.start();
        let last = *frames.end();
        let span = last - first + 1;
        let active = active_tags(source, frames).collect::<Vec<_>>();
        let count = self.layers.len() * self.cells.len() * active.len().max(1);
        let mut tags = Vec::with_capacity(count);
        for (layer_index, &layer) in self.layers.iter().enumerate() {
            for (cell_index, &cell) in self.cells.iter().enumerate() {
                let base = (layer_index * self.cells.len() + cell_index) * span;
                let label = group_label(layer, self.grid.then_some(cell));
                if active.is_empty() {
                    tags.push(FrameTag {
                        name: label,
                        from: base,
                        to: base + span - 1,
                        direction: 0,
                        repeat: 0,
                        color: [0; 3],
                    });
                } else {
                    for &tag in &active {
                        tags.push(FrameTag {
                            name: format!("{label}__{}", tag.name),
                            from: base + tag.from.max(first) - first,
                            to: base + tag.to.min(last) - first,
                            ..tag.clone()
                        });
                    }
                }
            }
        }
        tags
    }
}

fn active_tags<'a>(
    source: &'a Document,
    frames: &RangeInclusive<usize>,
) -> impl Iterator<Item = &'a FrameTag> + Clone {
    let (first, last) = (*frames.start(), *frames.end());
    source
        .tags
        .iter()
        .filter(move |tag| tag.from <= last && tag.to >= first)
}

fn group_label(layer: Option<usize>, cell: Option<[usize; 2]>) -> String {
    match (layer, cell) {
        (Some(layer), Some([x, y])) => format!("layer_{layer}_cell_{x}_{y}"),
        (Some(layer), None) => format!("layer_{layer}"),
        (None, Some([x, y])) => format!("cell_{x}_{y}"),
        (None, None) => unreachable!("A split sheet has at least one split axis"),
    }
}

fn sample_pixels(
    source: &Document,
    pixels: &[Pixel],
    [crop_x, crop_y, crop_w, crop_h]: [i32; 4],
    [x, y, width, height]: [usize; 4],
) -> Pixels {
    let (x, y, width, height) = (x as i64, y as i64, width as i64, height as i64);
    let left = x.max(-i64::from(crop_x));
    let top = y.max(-i64::from(crop_y));
    let right = (x + width)
        .min(i64::from(crop_w))
        .min(source.width as i64 - i64::from(crop_x));
    let bottom = (y + height)
        .min(i64::from(crop_h))
        .min(source.height as i64 - i64::from(crop_y));
    if left >= right || top >= bottom {
        return Pixels::empty((width * height) as usize);
    }
    let mut sample = vec![CLEAR; (width * height) as usize];
    let length = (right - left) as usize;
    for row in top..bottom {
        let from =
            (row + i64::from(crop_y)) as usize * source.width + (left + i64::from(crop_x)) as usize;
        let to = ((row - y) * width + left - x) as usize;
        sample[to..to + length].copy_from_slice(&pixels[from..from + length]);
    }
    sample.into()
}
