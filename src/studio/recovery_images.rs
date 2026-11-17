use super::{StoredTab, Workspace, validate_recovery_tab};
use pixelcraft::model::{
    CelMeta, Document, Layer, MAX_DOCUMENT_PIXELS, MAX_FRAME_NAME_CHARS, Pixels,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::{
    collections::HashSet,
    io::{BufReader, Read},
    path::PathBuf,
};

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Layers,
    Frames,
}

struct Image {
    name: String,
    duration: u16,
    pixels: Pixels,
    meta: CelMeta,
}

pub(super) fn salvage(source: &Document, layout: Layout) -> Result<Document, String> {
    source.validate()?;
    let mut images = Vec::new();
    let mut seen_rasters = HashSet::new();
    let mut remaining_raster_pixels = MAX_DOCUMENT_PIXELS;
    let mut remaining_tile_pixels = MAX_DOCUMENT_PIXELS;
    for (frame, &duration) in source.durations.iter().enumerate() {
        for (layer_index, layer) in source.layers.iter().enumerate() {
            if layer.group || layer.adjustment.is_some() {
                continue;
            }
            let pixels = &layer.cels[frame];
            let cel = &layer.cel_meta[frame];
            let pixels = if let Some(tileset) = layer.tileset {
                let tiles = &source.tilesets[tileset];
                let nonempty = layer.tile_cels[frame]
                    .iter()
                    .filter(|tile| (**tile & 0x1fff_ffff) != 0)
                    .count();
                if nonempty == 0 {
                    continue;
                }
                remaining_tile_pixels = remaining_tile_pixels
                    .checked_sub(nonempty.saturating_mul(tiles.width * tiles.height))
                    .ok_or("Recovered tile images exceed the rendering limit")?;
                remaining_raster_pixels = remaining_raster_pixels
                    .checked_sub(source.width * source.height)
                    .ok_or("Recovered images exceed the document memory limit")?;
                Pixels::from(source.layer_pixels(layer_index, frame))
            } else {
                if pixels.backing_id().is_none() && cel.bounds.is_none() && cel.outside.is_empty() {
                    continue;
                }
                if pixels
                    .backing_id()
                    .is_some_and(|id| seen_rasters.insert(id))
                {
                    remaining_raster_pixels = remaining_raster_pixels
                        .checked_sub(pixels.allocation_pixels())
                        .ok_or("Recovered images exceed the document memory limit")?;
                }
                pixels.clone()
            };
            remaining_raster_pixels = remaining_raster_pixels
                .checked_sub(cel.outside.len().saturating_mul(3))
                .ok_or("Recovered images exceed the document memory limit")?;
            if images.len() == usize::from(u16::MAX) {
                return Err("Too many images for one recovered document".into());
            }
            images.push(Image {
                name: format!("Frame {} · {}", frame + 1, layer.name),
                duration,
                pixels,
                meta: CelMeta {
                    bounds: cel.bounds,
                    outside: cel.outside.clone(),
                    ..CelMeta::default()
                },
            });
        }
    }
    if images.is_empty() {
        return Err("No recoverable images".into());
    }

    let (w, h) = (source.width, source.height);
    let mut result = Document::new(w, h);
    result.color_profile.clone_from(&source.color_profile);
    match layout {
        Layout::Layers => {
            result.layers = images
                .into_iter()
                .map(|image| {
                    let mut layer = Layer::new(image.name, w, h, 1);
                    layer.cels[0] = image.pixels;
                    layer.cel_meta[0] = image.meta;
                    layer
                })
                .collect();
        }
        Layout::Frames => {
            let mut layer = Layer::new("Recovered images".into(), w, h, images.len());
            result.durations = images.iter().map(|image| image.duration).collect();
            for (frame, image) in images.into_iter().enumerate() {
                let name: String = image
                    .name
                    .chars()
                    .filter(|character| !character.is_control())
                    .take(MAX_FRAME_NAME_CHARS)
                    .collect();
                result.set_frame_name(frame, &name)?;
                layer.cels[frame] = image.pixels;
                layer.cel_meta[frame] = image.meta;
            }
            result.layers = vec![layer];
        }
    }
    result.validate()?;
    Ok(result)
}

/// Try intact tab records first. For damaged records, decode only the artwork
/// and label it for image salvage. Buffer one JSON tab at a time without
/// expanding its per-pixel arrays into a generic JSON tree.
pub(super) fn read_partial_workspace(file: std::fs::File) -> Result<Workspace, String> {
    #[derive(Deserialize)]
    struct PartialWorkspace {
        version: u32,
        #[serde(deserialize_with = "read_partial_tabs")]
        tabs: Vec<StoredTab>,
        #[serde(default)]
        colors: Option<Box<RawValue>>,
        #[serde(default)]
        color: Option<Box<RawValue>>,
    }

    let decoder = flate2::read::ZlibDecoder::new(BufReader::new(file));
    const MAX_SALVAGE_JSON: u64 = 256 * 1024 * 1024;
    let partial: PartialWorkspace =
        serde_json::from_reader(BufReader::new(decoder.take(MAX_SALVAGE_JSON)))
            .map_err(|error| error.to_string())?;
    if partial.version != 1 || partial.tabs.is_empty() {
        return Err("No readable documents in this workspace backup".into());
    }
    let colors: Vec<(String, pixelcraft::model::Pixel)> = partial
        .colors
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
        .filter(|colors: &Vec<_>| !colors.is_empty())
        .unwrap_or_else(|| vec![("White".into(), [255; 4])]);
    let color = partial
        .color
        .and_then(|raw| serde_json::from_str::<usize>(raw.get()).ok())
        .filter(|index| *index < colors.len())
        .unwrap_or(0);
    Ok(Workspace {
        version: 1,
        tabs: partial.tabs,
        active: 0,
        colors,
        color,
    })
}

fn read_partial_tabs<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<StoredTab>, D::Error> {
    struct ReadTabs;
    impl<'de> serde::de::Visitor<'de> for ReadTabs {
        type Value = Vec<StoredTab>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("workspace recovery tabs")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut tabs = Vec::new();
            while let Some(raw) = sequence.next_element::<Box<RawValue>>()? {
                if let Ok(tab) = serde_json::from_str::<StoredTab>(raw.get())
                    && validate_recovery_tab(&tab).is_ok()
                {
                    tabs.push(tab);
                } else if let Some(tab) = salvage_tab_raster(raw.get()) {
                    tabs.push(tab);
                }
            }
            Ok(tabs)
        }
    }
    decoder.deserialize_seq(ReadTabs)
}

fn salvage_tab_raster(raw: &str) -> Option<StoredTab> {
    #[derive(Deserialize)]
    struct Artwork {
        #[serde(with = "pixelcraft::formats::recovery_document")]
        doc: Document,
        #[serde(default)]
        path: Option<Box<RawValue>>,
    }

    let artwork: Artwork = serde_json::from_str(raw).ok()?;
    artwork.doc.validate().ok()?;
    let path = artwork
        .path
        .and_then(|raw| serde_json::from_str::<PathBuf>(raw.get()).ok());
    Some(StoredTab {
        saved: Document::new(artwork.doc.width, artwork.doc.height),
        doc: artwork.doc,
        path,
        raw_recovery: true,
        live_save_root: None,
        live_save_root_identity: None,
        layer: 0,
        frame: 0,
        zoom: 1.,
        pan: [0., 0.],
        selection: None,
        polygon: Vec::new(),
        mask: None,
        symmetry_axes: [None; 2],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixelcraft::model::{CLEAR, OutsidePixel};

    #[test]
    fn recovers_hidden_raw_cels_as_independent_layers_or_frames() {
        let mut source = Document::new(2, 2);
        source.add_frame(0, false);
        source.add_layer();
        source.layers[1].visible = false;
        source.layers[1].opacity = 0;
        source.durations = vec![120, 85];
        let red = [240, 30, 12, 255];
        let green = [21, 220, 40, 255];
        source.set_pixel(1, 0, 0, red);
        source.set_pixel(0, 1, 3, green);
        source.layers[1].cel_meta[0].bounds = Some([-1, 0, 3, 2]);
        source.layers[1].cel_meta[0].outside.push(OutsidePixel {
            x: -1,
            y: 0,
            color: red,
        });

        let layers = salvage(&source, Layout::Layers).unwrap();
        assert_eq!(layers.layers.len(), 2);
        assert_eq!(layers.layers[0].cels[0][0], red);
        assert!(layers.layers[0].visible);
        assert_eq!(layers.layers[0].opacity, 255);
        assert_eq!(layers.layers[0].cel_meta[0].outside[0].x, -1);
        assert_eq!(layers.layers[1].cels[0][3], green);

        let frames = salvage(&source, Layout::Frames).unwrap();
        assert_eq!(frames.layers.len(), 1);
        assert_eq!(frames.durations, [120, 85]);
        assert_eq!(frames.layers[0].cels[0][0], red);
        assert_eq!(frames.layers[0].cels[0][3], CLEAR);
        assert_eq!(frames.layers[0].cels[1][3], green);
        assert!(frames.frame_name(0).is_some());
        assert!(!source.layers[1].visible);
    }
}
