use crate::automation::mcp_io;
use crate::{model::*, workflows::*};
use image::AnimationDecoder;
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{BufRead, BufReader, Cursor, ErrorKind, Read},
    path::{Path, PathBuf},
};

mod sheet_templates;
mod tileset_atlas;
pub use tileset_atlas::{TilesetAtlasEntry, TilesetAtlasLayout, TilesetAtlasScope, tileset_atlas};
pub(crate) use tileset_atlas::{plan_tileset_atlas, render_tileset_atlas};

pub fn from_frames(
    frames: impl Iterator<Item = Result<image::Frame, image::ImageError>>,
) -> Result<Document, String> {
    let mut doc: Option<Document> = None;
    for item in frames {
        let frame = item.map_err(|e| e.to_string())?;
        let (num, den) = frame.delay().numer_denom_ms();
        let im = frame.into_buffer();
        let (w, h) = (im.width() as usize, im.height() as usize);
        if w == 0
            || h == 0
            || w > MAX_DIMENSION
            || h > MAX_DIMENSION
            || w.saturating_mul(h) > MAX_PIXELS
        {
            return Err("Animation exceeds limits".into());
        }
        if doc.is_none() {
            doc = Some(Document::new(w, h));
        }
        let d = doc.as_mut().unwrap();
        if d.width != w || d.height != h {
            return Err("Animation frames must use the same canvas".into());
        }
        let f = if d.layers[0].name == "__first__" {
            let f = d.durations.len();
            if f >= 65535 || !d.can_grow(w * h) {
                return Err("Animation exceeds limits".into());
            }
            d.add_frame(f - 1, false);
            f
        } else {
            d.layers[0].name = "__first__".into();
            0
        };
        d.durations[f] = (num / den.max(1)).clamp(1, 65535) as u16;
        d.layers[0].cels[f] = im.pixels().map(|p| p.0).collect();
    }
    let mut d = doc.ok_or("Animation has no frames")?;
    d.layers[0].name = "Animation".into();
    d.validate()?;
    Ok(d)
}
pub fn load_animation(path: &Path) -> Result<Document, String> {
    let file = || {
        mcp_io::open(path)
            .map(BufReader::new)
            .map_err(|e| e.to_string())
    };
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "gif" => from_frames(
            image::codecs::gif::GifDecoder::new(file()?)
                .map_err(|e| e.to_string())?
                .into_frames(),
        ),
        "png" | "apng" => {
            let dec = image::codecs::png::PngDecoder::new(file()?).map_err(|e| e.to_string())?;
            if dec.is_apng().map_err(|e| e.to_string())? {
                from_frames(dec.apng().map_err(|e| e.to_string())?.into_frames())
            } else {
                Err("Not an animated PNG".into())
            }
        }
        "webp" => load_webp(path),
        _ => Err("Unknown animation format".into()),
    }
}
pub fn load_sequence(paths: &[PathBuf]) -> Result<Document, String> {
    let mut paths = paths.to_vec();
    paths.sort_by_key(|a| natural_key(a));
    let frames = paths.iter().map(|p| {
        mcp_io::open_image(p).map(|im| {
            image::Frame::from_parts(
                im.to_rgba8(),
                0,
                0,
                image::Delay::from_numer_denom_ms(100, 1),
            )
        })
    });
    from_frames(frames)
}
fn natural_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::new();
    let mut digits = String::new();
    for c in s.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c)
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{:0>20}", digits));
                digits.clear();
            }
            out.push(c);
        }
    }
    out
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SheetEngine {
    Godot,
    Unity,
}

impl SheetEngine {
    fn sidecar(self, path: &Path) -> PathBuf {
        match self {
            Self::Godot => path.with_extension("tres"),
            Self::Unity => {
                let mut name = path.as_os_str().to_owned();
                name.push(".meta");
                PathBuf::from(name)
            }
        }
    }
}

fn sheet_engines(preset: &str) -> &'static [SheetEngine] {
    match preset {
        "Godot" => &[SheetEngine::Godot],
        "Unity" => &[SheetEngine::Unity],
        "Unity + Godot" => &[SheetEngine::Godot, SheetEngine::Unity],
        _ => &[],
    }
}

pub(crate) fn sheet_output_paths(path: &Path, options: &SheetOptions) -> Vec<PathBuf> {
    let mut paths = vec![path.to_path_buf(), path.with_extension("json")];
    paths.extend(
        sheet_engines(&options.preset)
            .iter()
            .map(|engine| engine.sidecar(path)),
    );
    paths
}

pub fn export_sheet(path: &Path, d: &Document, o: &SheetOptions) -> Result<(), String> {
    export_sheet_with_previous(path, path, d, o)
}

pub(crate) fn export_sheet_with_previous(
    path: &Path,
    previous_path: &Path,
    d: &Document,
    o: &SheetOptions,
) -> Result<(), String> {
    export_sheet_extruded(path, previous_path, d, o, 0)
}

pub(crate) fn export_sheet_extruded(
    path: &Path,
    previous_path: &Path,
    d: &Document,
    o: &SheetOptions,
    extrude: usize,
) -> Result<(), String> {
    write_packed_sheet(
        path,
        previous_path,
        o,
        d,
        extrude,
        pack_sheet_with_extrusion(d, o, extrude)?,
    )
}

/// Pack the frame range cropped to one authored slice, or the whole sprite.
pub fn pack_sheet_for_slice(
    doc: &Document,
    options: &SheetOptions,
    slice: Option<&str>,
) -> Result<(image::RgbaImage, Value), String> {
    if let Some(name) = slice {
        doc.validate()?;
        let plans = plan_slice_sheets(doc, options, 0, Some(name))?;
        let filtered = filter_layers_for_export(doc, options.layers.as_deref())?;
        plans[0].pack(&filtered, options, 0)
    } else {
        pack_sheet(doc, options)
    }
}

/// Preview sample counts and known output dimensions without compositing.
pub fn sheet_export_summary(
    doc: &Document,
    options: &SheetOptions,
    slice: Option<&str>,
) -> Result<(usize, Option<[usize; 2]>), String> {
    doc.validate()?;
    validate_sheet_dimensions(options)?;
    let spacing = SheetSpacing::new(options, 0)?;
    let filtered = filter_layers_for_export(doc, options.layers.as_deref())?;
    let frames = options.frames_for(&filtered)?;
    let dimensions = options
        .width
        .zip(options.height)
        .map(|(w, h)| [w * options.scale, h * options.scale]);
    if let Some(slice) = slice {
        let plans = plan_slice_sheets(&filtered, options, 0, Some(slice))?;
        Ok((plans[0].samples, dimensions))
    } else if options.split_layers || options.split_grid {
        let count = split_sheet_work(&filtered, options, 0, &frames, None)?.samples;
        Ok((count, dimensions))
    } else {
        let count = frames.count();
        let dimensions = if options.trims() {
            dimensions
        } else {
            Some(uniform_sheet_dimensions(
                [doc.width, doc.height],
                count,
                options,
                spacing,
            )?)
        };
        Ok((count, dimensions))
    }
}

pub fn export_sheet_for_slice(
    path: &Path,
    doc: &Document,
    options: &SheetOptions,
    slice: Option<&str>,
) -> Result<(), String> {
    export_sheet_for_slice_with_previous(path, path, doc, options, slice)
}

pub(crate) fn export_sheet_for_slice_with_previous(
    path: &Path,
    previous: &Path,
    doc: &Document,
    options: &SheetOptions,
    slice: Option<&str>,
) -> Result<(), String> {
    write_packed_sheet(
        path,
        previous,
        options,
        doc,
        0,
        pack_sheet_for_slice(doc, options, slice)?,
    )
}

pub(crate) struct SliceSheet<'a> {
    slice: &'a SpriteSlice,
    first: usize,
    last: usize,
    width: usize,
    height: usize,
    pub file_name: String,
    samples: usize,
}

pub(crate) fn plan_slice_sheets<'a>(
    d: &'a Document,
    o: &SheetOptions,
    extrude: usize,
    slice_name: Option<&str>,
) -> Result<Vec<SliceSheet<'a>>, String> {
    if o.scale == 0 || o.scale > MAX_DIMENSION || extrude > 256 {
        return Err("Invalid split slice sheet scale or extrusion".into());
    }
    let target = if let Some(name) = slice_name {
        let mut matches = d
            .slices
            .iter()
            .enumerate()
            .filter(|(_, slice)| slice.name == name);
        let (index, _) = matches
            .next()
            .ok_or_else(|| format!("Unknown slice: {name:?}"))?;
        if matches.next().is_some() {
            return Err(format!("Ambiguous slice name: {name:?}"));
        }
        Some(index)
    } else {
        None
    };
    let frames = o.frames_for(d)?;
    let (first, last) = (*frames.start(), *frames.end());
    let mut plans = Vec::new();
    let mut source_budget = MAX_DOCUMENT_PIXELS;
    let mut crop_budget = MAX_DOCUMENT_PIXELS;
    let mut atlas_budget = MAX_DOCUMENT_PIXELS;
    let mut record_budget = MAX_CEL_SLOTS;
    let mut label_budget = 32 * 1024 * 1024usize;
    let mut godot_budget = MAX_CEL_SLOTS;
    let exports_godot = sheet_engines(&o.preset).contains(&SheetEngine::Godot);
    let spacing = SheetSpacing::new(o, extrude)?;
    for (index, slice) in d.slices.iter().enumerate() {
        if target.is_some_and(|target| index != target) {
            continue;
        }
        let Some(key) = slice.keys.first().filter(|key| key.frame <= last) else {
            continue;
        };
        let first = first.max(key.frame);
        if plans.len() == 1024 {
            return Err("Split slice export supports at most 1024 sheets".into());
        }
        let count = last - first + 1;
        let start_key = slice.keys.partition_point(|key| key.frame <= first) - 1;
        let end_key = slice.keys.partition_point(|key| key.frame <= last);
        let keys = &slice.keys[start_key..end_key];
        let width = keys.iter().map(|key| key.bounds[2] as usize).max().unwrap();
        let height = keys.iter().map(|key| key.bounds[3] as usize).max().unwrap();
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || width * height > MAX_PIXELS
        {
            return Err(format!("Slice {:?} exceeds the export limits", slice.name));
        }
        let split = if o.split_layers || o.split_grid {
            Some(split_sheet_work(
                d,
                o,
                extrude,
                &(first..=last),
                Some([width, height]),
            )?)
        } else {
            None
        };
        let samples = split.as_ref().map_or(count, |work| work.samples);
        source_budget = source_budget
            .checked_sub(
                split
                    .as_ref()
                    .map_or(count.saturating_mul(d.width * d.height), |work| {
                        work.source_pixels
                    }),
            )
            .ok_or("Split slice export exceeds the source pixel work limit; use a tag")?;
        crop_budget = crop_budget
            .checked_sub(
                split
                    .as_ref()
                    .map_or(count.saturating_mul(width * height), |work| {
                        work.cropped_pixels
                    }),
            )
            .ok_or("Split slice export exceeds the cropped pixel work limit; use a tag")?;

        let atlas_pixels = if let Some(split) = &split {
            split.atlas_pixels
        } else {
            uniform_sheet_area([width, height], count, o, spacing)
                .map_err(|error| format!("Slice {:?}: {error}", slice.name))?
        };
        let groups = split.as_ref().map_or(1, |work| work.groups);
        // A trimmed sheet may be smaller, but can never exceed the packer's output limit.
        atlas_budget = atlas_budget
            .checked_sub(atlas_pixels.min(MAX_PIXELS))
            .ok_or("Split slice export exceeds the combined atlas pixel limit; use a tag")?;
        record_budget = record_budget
            .checked_sub(
                (samples + keys.len()).saturating_add(d.tags.len().max(1).saturating_mul(groups)),
            )
            .ok_or("Split slice export exceeds the metadata record limit")?;
        let frame_label_bytes = (first..=last)
            .filter_map(|frame| d.frame_name(frame))
            .map(str::len)
            .sum::<usize>();
        label_budget = label_budget
            .checked_sub(
                frame_label_bytes
                    .saturating_mul(groups)
                    .saturating_add(slice.name.len())
                    .saturating_add(split.as_ref().map_or(0, |work| work.layer_label_bytes)),
            )
            .ok_or("Split slice export exceeds the metadata label limit")?;
        let mut has_tag = false;
        for tag in &d.tags {
            let (from, to) = (first.max(tag.from), last.min(tag.to));
            if from <= to {
                has_tag = true;
                label_budget = label_budget
                    .checked_sub(tag.name.len().saturating_mul(groups))
                    .ok_or("Split slice export exceeds the metadata label limit")?;
                if exports_godot {
                    let span = to - from + 1;
                    let turn = if tag.direction >= 2 {
                        span.saturating_sub(2)
                    } else {
                        0
                    };
                    godot_budget = godot_budget
                        .checked_sub(
                            (span + turn)
                                .saturating_mul(usize::from(tag.repeat.max(1)))
                                .saturating_mul(groups),
                        )
                        .ok_or(
                            "Split slice export exceeds the combined Godot animation key limit",
                        )?;
                }
            }
        }
        if exports_godot && !has_tag {
            godot_budget = godot_budget
                .checked_sub(samples)
                .ok_or("Split slice export exceeds the combined Godot animation key limit")?;
        }
        let label: String = slice
            .name
            .chars()
            .take(48)
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let label = if label.is_empty() { "slice" } else { &label };
        plans.push(SliceSheet {
            slice,
            first,
            last,
            width,
            height,
            file_name: format!("{:03}-{label}.png", index + 1),
            samples,
        });
    }
    if plans.is_empty() {
        return Err("No authored slices in the selected frame range".into());
    }
    Ok(plans)
}

#[cfg(test)]
mod slice_sheet_limits {
    use super::*;

    fn sliced(count: usize, size: i32) -> Document {
        let mut doc = Document::new(1, 1);
        doc.slices = (0..count)
            .map(|index| SpriteSlice {
                editor_id: Default::default(),
                name: format!("Slice {index}"),
                keys: vec![SliceKey {
                    frame: 0,
                    bounds: [0, 0, size, size],
                    center: None,
                    pivot: None,
                }],
            })
            .collect();
        doc
    }

    #[test]
    fn split_sheets_preflight_crop_and_scaled_atlas_work_across_slices() {
        let options = SheetOptions::default();
        let doc = sliced(3, 7_998);
        assert!(
            matches!(plan_slice_sheets(&doc, &options, 0, None), Err(error) if error.contains("cropped pixel work"))
        );

        let options = SheetOptions {
            scale: 8,
            ..options
        };
        let doc = sliced(3, 996);
        assert!(
            matches!(plan_slice_sheets(&doc, &options, 1, None), Err(error) if error.contains("combined atlas pixel"))
        );
        assert!(plan_slice_sheets(&sliced(2, 996), &options, 1, None).is_ok());
    }

    #[test]
    fn split_sheets_cap_output_file_count_and_combined_godot_timing() {
        let doc = sliced(1_025, 1);
        assert!(
            matches!(plan_slice_sheets(&doc, &SheetOptions::default(), 0, None), Err(error) if error.contains("1024 sheets"))
        );

        let mut doc = sliced(3, 1);
        for frame in 0..7 {
            doc.add_frame(frame, false);
        }
        doc.tags.push(FrameTag {
            editor_id: Default::default(),
            name: "Idle".into(),
            from: 0,
            to: 7,
            direction: 0,
            repeat: 62_500,
            color: [0; 3],
        });
        let options = SheetOptions {
            preset: "Godot".into(),
            ..Default::default()
        };
        assert!(
            matches!(plan_slice_sheets(&doc, &options, 0, None), Err(error) if error.contains("combined Godot animation key"))
        );
    }
}

impl SliceSheet<'_> {
    pub(crate) fn export(
        &self,
        path: &Path,
        previous_path: &Path,
        d: &Document,
        o: &SheetOptions,
        extrude: usize,
    ) -> Result<(), String> {
        write_packed_sheet(
            path,
            previous_path,
            o,
            d,
            extrude,
            self.pack(d, o, extrude)?,
        )
    }

    fn pack(
        &self,
        d: &Document,
        o: &SheetOptions,
        extrude: usize,
    ) -> Result<(image::RgbaImage, Value), String> {
        let mut keys = self.slice.keys.iter().peekable();
        let mut active = keys.next().unwrap();
        let active_keys: Vec<_> = (self.first..=self.last)
            .map(|f| {
                while keys.peek().is_some_and(|next| next.frame <= f) {
                    active = keys.next().unwrap();
                }
                active
            })
            .collect();
        if o.split_layers || o.split_grid {
            return pack_split_sheet(
                d,
                o,
                extrude,
                self.first..=self.last,
                Some(SheetCrop {
                    size: [self.width, self.height],
                    name: &self.slice.name,
                    keys: &active_keys,
                }),
            );
        }
        let (width, height) = (self.width, self.height);

        let mut cropped = Document::new(width, height);
        cropped.durations = d.durations[self.first..=self.last].to_vec();
        cropped.drawing_grid = d.drawing_grid;
        let frame_count = cropped.durations.len();
        cropped.layers[0]
            .cel_meta
            .resize_with(frame_count, CelMeta::default);
        cropped.layers[0]
            .tile_cels
            .resize_with(frame_count, Default::default);
        cropped.set_sheet_frame_export_ids(
            (self.first..=self.last)
                .map(|f| d.frame_export_id(f))
                .collect(),
            d.next_frame_export_id(),
        );
        cropped.tags = d
            .tags
            .iter()
            .filter_map(|tag| {
                let from = tag.from.max(self.first);
                let to = tag.to.min(self.last);
                (from <= to).then(|| FrameTag {
                    from: from - self.first,
                    to: to - self.first,
                    ..tag.clone()
                })
            })
            .collect();

        let mut local_keys = Vec::new();
        let mut cels = Vec::with_capacity(frame_count);
        for (n, &key) in active_keys.iter().enumerate() {
            let source_frame = self.first + n;
            if let Some(name) = d.frame_name(source_frame) {
                cropped.set_frame_name(n, name)?;
            }
            if n == 0 || active_keys[n - 1].frame != key.frame {
                let mut local = key.clone();
                local.frame = n;
                local.bounds[..2].fill(0);
                local_keys.push(local);
            }
            let [x, y, w, h] = key.bounds.map(i64::from);
            let (left, right) = (x.max(0), (x + w).min(d.width as i64));
            let (top, bottom) = (y.max(0), (y + h).min(d.height as i64));
            if left >= right || top >= bottom {
                cels.push(Pixels::empty(width * height));
                continue;
            }
            let mut pixels = vec![CLEAR; width * height];
            let source = d.composite(source_frame);
            let len = (right - left) as usize;
            for row in top..bottom {
                let from = row as usize * d.width + left as usize;
                let to = (row - y) as usize * width + (left - x) as usize;
                pixels[to..to + len].copy_from_slice(&source[from..from + len]);
            }
            cels.push(pixels.into());
        }
        cropped.layers[0].cels = cels;
        cropped.slices.push(SpriteSlice {
            editor_id: Default::default(),
            name: self.slice.name.clone(),
            keys: local_keys,
        });
        let mut options = o.clone();
        options.tag = None;
        options.frame_range = None;
        options.layers = None;
        let (image, mut metadata) = pack_sheet_with_extrusion(&cropped, &options, extrude)?;
        for (n, frame) in metadata["frames"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            frame["sourceFrame"] = json!(self.first + n);
        }
        metadata["meta"]["sourceSlice"] = json!(self.slice.name);
        Ok((image, metadata))
    }
}

pub(crate) fn preflight_sheet_export(
    path: &Path,
    doc: &Document,
    options: &SheetOptions,
    extrude: usize,
    slice: Option<&SliceSheet<'_>>,
) -> Result<(), String> {
    let templates =
        !options.filename_template.trim().is_empty() || !options.tag_template.trim().is_empty();
    let custom_layout =
        options.width.is_some() || options.height.is_some() || options.layout != SheetLayout::Rows;
    if !templates {
        if !custom_layout {
            return Ok(());
        }
        if !options.trims() {
            if slice.is_none() && !options.split_layers && !options.split_grid {
                let spacing = SheetSpacing::new(options, extrude)?;
                uniform_sheet_area(
                    [doc.width, doc.height],
                    options.frames_for(doc)?.count(),
                    options,
                    spacing,
                )?;
            }
            // Split and slice planners checked their uniform sample dimensions.
            return Ok(());
        }
    }
    let (_, mut metadata) = if let Some(slice) = slice {
        slice.pack(doc, options, extrude)?
    } else {
        pack_sheet_with_extrusion(doc, options, extrude)?
    };
    sheet_templates::apply(doc, options, path, &mut metadata)
}

fn write_packed_sheet(
    path: &Path,
    previous_path: &Path,
    o: &SheetOptions,
    doc: &Document,
    extrude: usize,
    (im, mut meta): (image::RgbaImage, Value),
) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy();
    sheet_templates::apply(doc, o, previous_path, &mut meta)?;
    meta["meta"]["image"] = json!(name);
    let engines = sheet_engines(&o.preset);
    if engines.contains(&SheetEngine::Unity) {
        let mut names = std::collections::HashSet::new();
        for tag in meta["meta"]["frameTags"].as_array().unwrap() {
            let name = tag["name"].as_str().unwrap();
            if name.trim().is_empty() || !names.insert(name) {
                return Err(
                    "Give each exported Unity animation tag a unique, non-empty name".into(),
                );
            }
        }
    }
    let json_path = path.with_extension("json");
    companion_exists(&json_path)?;
    let mut sidecars = Vec::with_capacity(engines.len());
    for &engine in engines {
        let contents = match engine {
            SheetEngine::Godot => {
                godot(&meta, &name, previous_godot_uid(previous_path)?.as_deref())?
            }
            SheetEngine::Unity => unity(
                &meta,
                &previous_path.to_string_lossy(),
                im.height(),
                &previous_unity_meta(previous_path)?,
            )?,
        };
        sidecars.push((engine.sidecar(path), contents));
    }
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        crate::formats::write_packed_png(path, doc, o, &mut meta, &im, extrude)?;
    } else {
        mcp_io::write_image(path, &im).map_err(|e| e.to_string())?;
    }
    write_json(&json_path, &meta)?;
    for (companion, contents) in sidecars {
        mcp_io::write(&companion, contents).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn previous_godot_uid(previous_path: &Path) -> Result<Option<String>, String> {
    let resource = SheetEngine::Godot.sidecar(previous_path);
    let mut header = String::new();
    if companion_exists(&resource)? {
        const MAX_HEADER_BYTES: usize = 4096;
        let source = mcp_io::open(&resource).map_err(|e| e.to_string())?;
        BufReader::new(source.take(MAX_HEADER_BYTES as u64 + 1))
            .read_line(&mut header)
            .map_err(|e| e.to_string())?;
        if header.len() > MAX_HEADER_BYTES {
            return Err("Previous Godot resource header exceeds 4 KiB".into());
        }
    }
    Ok(header
        .lines()
        .next()
        .and_then(|header| header.strip_prefix("[gd_resource "))
        .and_then(|header| header.split_once(" uid=\"uid://"))
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(uid, _)| uid)
        .filter(|uid| {
            !uid.is_empty()
                && uid.len() <= 16
                && uid
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        .map(str::to_owned))
}

fn previous_unity_meta(previous_path: &Path) -> Result<String, String> {
    const MAX_UNITY_META_BYTES: usize = 64 * 1024 * 1024;
    let meta_path = SheetEngine::Unity.sidecar(previous_path);
    let mut previous = String::new();
    if companion_exists(&meta_path)? {
        let source = mcp_io::open(&meta_path).map_err(|e| e.to_string())?;
        let too_large = || "Previous Unity metadata exceeds 64 MiB".to_string();
        if source.metadata().map_err(|e| e.to_string())?.len() > MAX_UNITY_META_BYTES as u64 {
            return Err(too_large());
        }
        source
            .take(MAX_UNITY_META_BYTES as u64 + 1)
            .read_to_string(&mut previous)
            .map_err(|e| e.to_string())?;
        if previous.len() > MAX_UNITY_META_BYTES {
            return Err(too_large());
        }
    }
    Ok(previous)
}

fn companion_exists(path: &Path) -> Result<bool, String> {
    match mcp_io::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(format!(
            "Sprite-sheet companion is not a regular file: {}",
            path.display()
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Could not inspect {}: {error}", path.display())),
    }
}
pub fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    mcp_io::write(
        path,
        serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn godot(meta: &Value, name: &str, uid: Option<&str>) -> Result<String, String> {
    use std::fmt::Write;

    let frames = meta["frames"].as_array().unwrap();
    let default =
        vec![json!({"name":"default","from":0,"to":frames.len()-1,"direction":"forward"})];
    let tags = meta["meta"]["frameTags"]
        .as_array()
        .filter(|v| !v.is_empty())
        .unwrap_or(&default);
    let mut remaining = 1_000_000u64;
    for tag in tags {
        let span = tag["to"].as_u64().unwrap() - tag["from"].as_u64().unwrap() + 1;
        let turn = if tag["direction"]
            .as_str()
            .is_some_and(|d| d.starts_with("pingpong"))
        {
            span.saturating_sub(2)
        } else {
            0
        };
        let entries = (span + turn) * tag["repeat"].as_u64().unwrap_or(0).max(1);
        remaining = remaining
            .checked_sub(entries)
            .ok_or("Godot animations exceed one million entries in total")?;
    }
    let quoted = serde_json::to_string(&format!("res://{name}")).unwrap();
    let uid = uid.map_or_else(String::new, |uid| format!(" uid=\"uid://{uid}\""));
    let mut s = format!(
        "[gd_resource type=\"SpriteFrames\" load_steps={} format=3{uid}]\n\n[ext_resource type=\"Texture2D\" path={quoted} id=\"1\"]\n",
        frames.len() + 2
    );
    for f in frames {
        let id = &f["frameId"];
        let r = &f["frame"];
        s += &format!(
            "\n[sub_resource type=\"AtlasTexture\" id=\"Atlas_{id}\"]\natlas = ExtResource(\"1\")\nregion = Rect2({}, {}, {}, {})\nmargin = Rect2({}, {}, {}, {})\n",
            r["x"],
            r["y"],
            r["w"],
            r["h"],
            f["spriteSourceSize"]["x"],
            f["spriteSourceSize"]["y"],
            f["sourceSize"]["w"].as_i64().unwrap() - r["w"].as_i64().unwrap(),
            f["sourceSize"]["h"].as_i64().unwrap() - r["h"].as_i64().unwrap()
        );
    }
    s.push_str("\n[resource]\nanimations = [");
    let mut names = std::collections::HashSet::new();
    let mut next_suffix = std::collections::HashMap::new();
    for (index, t) in tags.iter().enumerate() {
        let mut order: Vec<usize> =
            (t["from"].as_u64().unwrap() as usize..=t["to"].as_u64().unwrap() as usize).collect();
        let direction = t["direction"].as_str().unwrap_or("forward");
        if direction == "reverse" || direction == "pingpong_reverse" {
            order.reverse()
        }
        if direction.starts_with("pingpong") && order.len() > 2 {
            let back: Vec<_> = order[1..order.len() - 1].iter().rev().copied().collect();
            order.extend(back)
        }
        let repeat = t["repeat"].as_u64().unwrap_or(0);
        let base = t["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Animation");
        let mut name = base.to_string();
        if !names.insert(name.clone()) {
            let suffix = next_suffix.entry(base).or_insert(2);
            loop {
                name = format!("{base}_{suffix}");
                *suffix += 1;
                if names.insert(name.clone()) {
                    break;
                }
            }
        }
        if index > 0 {
            s.push_str(",\n");
        }
        s.push_str("{\"frames\": [");
        // SpriteFrames has a boolean loop flag: finite repeats are expanded.
        for (entry, &frame) in order
            .iter()
            .cycle()
            .take(order.len() * repeat.max(1) as usize)
            .enumerate()
        {
            if entry > 0 {
                s.push_str(", ");
            }
            write!(
                s,
                "{{\"duration\": {}, \"texture\": SubResource(\"Atlas_{}\")}}",
                frames[frame]["duration"].as_f64().unwrap() / 1000.,
                frames[frame]["frameId"]
            )
            .unwrap();
        }
        write!(
            s,
            "], \"loop\": {}, \"name\": &{}, \"speed\": 1.0}}",
            repeat == 0,
            serde_json::to_string(&name).unwrap()
        )
        .unwrap();
    }
    s.push_str("]\n");
    Ok(s)
}
#[derive(Default)]
struct UnitySpriteIdentity<'a> {
    file_id: Option<i64>,
    sprite_id: Option<&'a str>,
}

fn unity_guid(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_unity_name(value: &str) -> Cow<'_, str> {
    let value = value.trim();
    if value.starts_with('"')
        && let Ok(decoded) = serde_json::from_str::<String>(value)
    {
        return Cow::Owned(decoded);
    }
    if let Some(quoted) = value
        .strip_prefix(char::from(39))
        .and_then(|v| v.strip_suffix(char::from(39)))
    {
        return Cow::Owned(quoted.replace("''", "'"));
    }
    Cow::Borrowed(value)
}

fn encode_unity_name(name: &str) -> Cow<'_, str> {
    if name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "y" | "yes" | "n" | "no" | "true" | "false" | "on" | "off" | "null"
        )
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(serde_json::to_string(name).unwrap())
    }
}

fn unity_sprite_ids(
    previous: &str,
) -> Result<std::collections::HashMap<Cow<'_, str>, UnitySpriteIdentity<'_>>, String> {
    let mut sprites = std::collections::HashMap::<_, UnitySpriteIdentity<'_>>::new();
    let mut current = None;
    let mut in_sprites = false;
    let mut in_table = false;
    for line in previous.lines() {
        if line.starts_with("    ") && !line.starts_with("     ") {
            match line.trim() {
                "sprites:" => in_sprites = true,
                entry if in_sprites && entry.starts_with("- serializedVersion:") => current = None,
                "nameFileIdTable:" => {
                    in_sprites = false;
                    in_table = true;
                }
                _ => {
                    in_sprites = false;
                    in_table = false;
                }
            }
        }
        if in_sprites {
            if let Some(name) = line.strip_prefix("      name: ") {
                current = Some(decode_unity_name(name));
            } else if let Some(name) = &current {
                if let Some(id) = line.strip_prefix("      internalID: ") {
                    sprites.entry(name.clone()).or_default().file_id =
                        id.trim().parse().ok().filter(|id| *id != 0);
                }
                if let Some(id) = line.strip_prefix("      spriteID: ").map(str::trim)
                    && unity_guid(id)
                {
                    sprites.entry(name.clone()).or_default().sprite_id = Some(id);
                }
            }
        } else if in_table
            && let Some((name, id)) = line.strip_prefix("      ").and_then(|v| v.rsplit_once(':'))
            && let Ok(id) = id.trim().parse::<i64>()
            && id != 0
        {
            sprites.entry(decode_unity_name(name)).or_default().file_id = Some(id);
        }
        if sprites.len() > u16::MAX as usize {
            return Err("Previous Unity metadata exceeds the sprite count limit".into());
        }
    }
    Ok(sprites)
}

fn unity(meta: &Value, name: &str, height: u32, previous: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let generated_guid = || {
        let digest = Sha256::digest(name.as_bytes());
        format!(
            "{:032x}",
            u128::from_be_bytes(digest[..16].try_into().unwrap())
        )
    };
    let guid = previous
        .lines()
        .find_map(|line| line.strip_prefix("guid: ").map(str::trim))
        .filter(|value| unity_guid(value))
        .map_or_else(generated_guid, str::to_owned);
    let previous = unity_sprite_ids(previous)?;
    let frames = meta["frames"].as_array().unwrap();
    let mut reserved_ids: std::collections::HashSet<_> =
        previous.values().filter_map(|id| id.file_id).collect();
    let mut assigned = std::collections::HashSet::new();
    let mut spare = -1;
    let ids: Vec<_> = frames
        .iter()
        .map(|frame| {
            let name = frame["filename"].as_str().unwrap();
            let old = previous.get(name);
            let default = 21300000 + 2 * frame["frameId"].as_i64().unwrap();
            let id = old
                .and_then(|old| old.file_id)
                .filter(|id| assigned.insert(*id))
                .or_else(|| {
                    (reserved_ids.insert(default) && assigned.insert(default)).then_some(default)
                })
                .unwrap_or_else(|| {
                    while reserved_ids.contains(&spare) {
                        spare -= 1;
                    }
                    reserved_ids.insert(spare);
                    assigned.insert(spare);
                    spare
                });
            let sprite_id = old.and_then(|old| old.sprite_id).map_or_else(
                || {
                    let digest = Sha256::digest(format!("{guid}:{name}").as_bytes());
                    format!(
                        "{:032x}",
                        u128::from_be_bytes(digest[..16].try_into().unwrap())
                    )
                },
                str::to_owned,
            );
            (name, id, sprite_id)
        })
        .collect();

    let mut s =
        format!("fileFormatVersion: 2\nguid: {guid}\nTextureImporter:\n  internalIDToNameTable:\n");
    for (name, id, _) in &ids {
        let name = encode_unity_name(name);
        s += &format!("  - first:\n      213: {id}\n    second: {name}\n");
    }
    s += "  serializedVersion: 12\n  mipmaps:\n    enableMipMap: 0\n  isReadable: 0\n  filterMode: 0\n  textureSettings:\n    serializedVersion: 2\n    filterMode: 0\n    aniso: 1\n    mipBias: 0\n    wrapU: 1\n    wrapV: 1\n  maxTextureSize: 16384\n  textureType: 8\n  spriteMode: 2\n  spritePixelsToUnits: 100\n  alphaIsTransparency: 1\n  textureCompression: 0\n  spriteSheet:\n    serializedVersion: 2\n    sprites:\n";
    for (frame, (name, id, sprite_id)) in frames.iter().zip(&ids) {
        let r = &frame["frame"];
        let name = encode_unity_name(name);
        s += &format!(
            "    - serializedVersion: 2\n      name: {name}\n      rect:\n        serializedVersion: 2\n        x: {}\n        y: {}\n        width: {}\n        height: {}\n      alignment: 0\n      pivot: {{x: 0.5, y: 0.5}}\n      border: {{x: 0, y: 0, z: 0, w: 0}}\n      spriteID: {sprite_id}\n      internalID: {id}\n",
            r["x"],
            height as u64 - r["y"].as_u64().unwrap() - r["h"].as_u64().unwrap(),
            r["w"],
            r["h"],
        );
    }
    s += "    nameFileIdTable:\n";
    for (name, id, _) in ids {
        let name = encode_unity_name(name);
        s += &format!("      {name}: {id}\n");
    }
    Ok(s)
}
pub fn load_ora(path: &Path) -> Result<Document, String> {
    let mut entries = HashMap::new();
    crate::extensions::visit_zip(
        &mcp_io::read(path).map_err(|e| e.to_string())?,
        |path, contents| {
            if let Some(contents) = contents {
                entries.insert(path, contents);
            }
            Ok(())
        },
    )?;
    let xml = entries
        .get(Path::new("stack.xml"))
        .ok_or("Missing OpenRaster stack.xml")?;
    let xml = std::str::from_utf8(xml).map_err(|e| e.to_string())?;
    let tree = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let image = tree.root_element();
    let w = image
        .attribute("w")
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let h = image
        .attribute("h")
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let count = image
        .descendants()
        .filter(|n| n.has_tag_name("layer") || n.has_tag_name("stack"))
        .count();
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h).saturating_mul(count) > MAX_PIXELS
    {
        return Err("OpenRaster exceeds limits".into());
    }
    let mut d = Document::new(w, h);
    d.layers.clear();
    fn visit(
        node: roxmltree::Node,
        parent: Option<usize>,
        entries: &HashMap<PathBuf, Vec<u8>>,
        d: &mut Document,
    ) -> Result<(), String> {
        for n in node.children().filter(|n| n.is_element()).rev() {
            if !n.has_tag_name("layer") && !n.has_tag_name("stack") {
                continue;
            }
            let mut l = Layer::new(
                n.attribute("name").unwrap_or("Layer").into(),
                d.width,
                d.height,
                1,
            );
            l.parent = parent;
            l.group = n.has_tag_name("stack");
            l.visible = n.attribute("visibility") != Some("hidden");
            l.opacity = (n
                .attribute("opacity")
                .unwrap_or("1")
                .parse::<f32>()
                .map_err(|e| e.to_string())?
                .clamp(0., 1.)
                * 255.)
                .round() as u8;
            l.blend = match n.attribute("composite-op").unwrap_or("svg:src-over") {
                "svg:multiply" => BlendMode::Multiply,
                "svg:screen" => BlendMode::Screen,
                "svg:overlay" => BlendMode::Overlay,
                "svg:darken" => BlendMode::Darken,
                "svg:lighten" => BlendMode::Lighten,
                "svg:src-over" => BlendMode::Normal,
                v => return Err(format!("Unsupported OpenRaster blend: {v}")),
            };
            let index = d.layers.len();
            if !l.group {
                let src = n.attribute("src").ok_or("Missing OpenRaster source")?;
                if Path::new(src)
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    return Err("Unsafe OpenRaster source".into());
                }
                let bytes = entries
                    .get(Path::new(src))
                    .ok_or_else(|| format!("Missing OpenRaster layer in archive: {src}"))?;
                let im = image::ImageReader::new(Cursor::new(bytes))
                    .with_guessed_format()
                    .map_err(|e| e.to_string())?
                    .decode()
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                let ox = n
                    .attribute("x")
                    .unwrap_or("0")
                    .parse::<i32>()
                    .map_err(|e| e.to_string())?;
                let oy = n
                    .attribute("y")
                    .unwrap_or("0")
                    .parse::<i32>()
                    .map_err(|e| e.to_string())?;
                for (x, y, p) in im.enumerate_pixels() {
                    let (x, y) = (x as i32 + ox, y as i32 + oy);
                    if let Some(i) = d.index(x, y) {
                        l.cels[0][i] = p.0
                    } else {
                        l.cel_meta[0].outside.push(OutsidePixel {
                            x,
                            y,
                            color: p.0,
                            palette_slot: None,
                        });
                    }
                }
            }
            d.layers.push(l);
            if n.has_tag_name("stack") {
                visit(n, Some(index), entries, d)?;
            }
        }
        Ok(())
    }
    let stack = image
        .children()
        .find(|n| n.has_tag_name("stack"))
        .ok_or("Missing OpenRaster stack")?;
    visit(stack, None, &entries, &mut d)?;
    d.validate()?;
    Ok(d)
}

pub fn load_webp(path: &Path) -> Result<Document, String> {
    // libwebp's animation decoder returns complete composited canvases, including disposal.
    #[repr(C)]
    struct Data {
        bytes: *const u8,
        size: usize,
    }
    #[repr(C)]
    struct Info {
        width: u32,
        height: u32,
        loop_count: u32,
        bgcolor: u32,
        frame_count: u32,
        pad: [u32; 4],
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let load_runtime = || {
        ["libwebpdemux.so.2", "libwebpdemux.so", "libwebpdemux.dylib"]
            .into_iter()
            // SAFETY: these are fixed runtime library names. The decoder keeps the
            // library loaded until after all its symbols and decoder state are dropped.
            .find_map(|name| unsafe { libloading::Library::new(name).ok() })
            .ok_or("WebP import requires the libwebpdemux runtime on this system")
    };
    #[cfg(target_os = "linux")]
    let lib = crate::automation::mcp_linux::with_codec_runtime(load_runtime)??;
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    let lib = load_runtime()?;
    use std::ffi::c_void;
    type New = unsafe extern "C" fn(*const Data, *const c_void, i32) -> *mut c_void;
    type ReadInfo = unsafe extern "C" fn(*const c_void, *mut Info) -> i32;
    type Next = unsafe extern "C" fn(*mut c_void, *mut *mut u8, *mut i32) -> i32;
    type Delete = unsafe extern "C" fn(*mut c_void);

    #[cfg(any(windows, target_os = "macos"))]
    unsafe extern "C" {
        fn WebPAnimDecoderNewInternal(
            data: *const Data,
            options: *const c_void,
            version: i32,
        ) -> *mut c_void;
        fn WebPAnimDecoderGetInfo(decoder: *const c_void, info: *mut Info) -> i32;
        fn WebPAnimDecoderGetNext(
            decoder: *mut c_void,
            frame: *mut *mut u8,
            timestamp: *mut i32,
        ) -> i32;
        fn WebPAnimDecoderDelete(decoder: *mut c_void);
    }

    unsafe {
        #[cfg(any(windows, target_os = "macos"))]
        let (new, info, next, delete) = (
            WebPAnimDecoderNewInternal as New,
            WebPAnimDecoderGetInfo as ReadInfo,
            WebPAnimDecoderGetNext as Next,
            WebPAnimDecoderDelete as Delete,
        );
        #[cfg(not(any(windows, target_os = "macos")))]
        let (new, info, next, delete) = (
            *lib.get::<New>(b"WebPAnimDecoderNewInternal")
                .map_err(|e| e.to_string())?,
            *lib.get::<ReadInfo>(b"WebPAnimDecoderGetInfo")
                .map_err(|e| e.to_string())?,
            *lib.get::<Next>(b"WebPAnimDecoderGetNext")
                .map_err(|e| e.to_string())?,
            *lib.get::<Delete>(b"WebPAnimDecoderDelete")
                .map_err(|e| e.to_string())?,
        );
        let bytes = mcp_io::read(path).map_err(|e| e.to_string())?;
        let dec = new(
            &Data {
                bytes: bytes.as_ptr(),
                size: bytes.len(),
            },
            std::ptr::null(),
            0x0107,
        );
        if dec.is_null() {
            return Err("Invalid WebP".into());
        }
        let result = (|| {
            let mut inf = Info {
                width: 0,
                height: 0,
                loop_count: 0,
                bgcolor: 0,
                frame_count: 0,
                pad: [0; 4],
            };
            if info(dec, &mut inf) == 0
                || inf.width == 0
                || inf.height == 0
                || inf.width as usize > MAX_DIMENSION
                || inf.height as usize > MAX_DIMENSION
                || (inf.width as usize)
                    .saturating_mul(inf.height as usize)
                    .saturating_mul(inf.frame_count as usize)
                    > MAX_PIXELS
                || inf.frame_count > 65535
            {
                return Err("WebP exceeds limits".into());
            }
            let mut d = Document::new(inf.width as usize, inf.height as usize);
            let mut last = 0;
            for f in 0..inf.frame_count as usize {
                let mut ptr = std::ptr::null_mut();
                let mut stamp = 0;
                if next(dec, &mut ptr, &mut stamp) == 0 {
                    return Err("Invalid WebP frame".into());
                }
                if f > 0 {
                    d.add_frame(f - 1, false)
                }
                let pixels = std::slice::from_raw_parts(ptr, d.width * d.height * 4);
                d.layers[0].cels[f] = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| [p[0], p[1], p[2], p[3]])
                    .collect();
                d.durations[f] = (stamp - last).clamp(1, 65535) as u16;
                last = stamp;
            }
            d.validate()?;
            Ok(d)
        })();
        delete(dec);
        result
    }
}

pub fn export_tiled(path: &Path, d: &Document, f: usize) -> Result<(), String> {
    if d.tilesets.is_empty() {
        return Err("Import a tileset first".into());
    }
    let first = &d.tilesets[0];
    if d.tilesets
        .iter()
        .any(|t| t.width != first.width || t.height != first.height)
    {
        return Err("Tiled map export requires matching tile sizes".into());
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut sets = vec![];
    let mut gid = 1u32;
    let mut offsets = vec![];
    for (i, t) in d.tilesets.iter().enumerate() {
        let columns = 16.min(t.tiles.len()).max(1);
        let rows = t.tiles.len().div_ceil(columns);
        if t.width
            .saturating_mul(columns)
            .saturating_mul(t.height)
            .saturating_mul(rows)
            > MAX_PIXELS
        {
            return Err("Tileset exceeds image limits".into());
        }
        let mut im = image::RgbaImage::new((columns * t.width) as u32, (rows * t.height) as u32);
        for (n, tile) in t.tiles.iter().enumerate() {
            for y in 0..t.height {
                for x in 0..t.width {
                    im.put_pixel(
                        ((n % columns) * t.width + x) as u32,
                        ((n / columns) * t.height + y) as u32,
                        image::Rgba(tile[y * t.width + x]),
                    );
                }
            }
        }
        let png = format!("{stem}-tiles-{i}.png");
        let tsj = format!("{stem}-tiles-{i}.tsj");
        mcp_io::write_image(path.with_file_name(&png), &im).map_err(|e| e.to_string())?;
        write_json(
            &path.with_file_name(&tsj),
            &json!({"type":"tileset","version":"1.10","name":t.name,"tilewidth":t.width,"tileheight":t.height,"tilecount":t.tiles.len(),"columns":columns,"image":png,"imagewidth":im.width(),"imageheight":im.height(),"margin":0,"spacing":0}),
        )?;
        sets.push(json!({"firstgid":gid,"source":tsj}));
        offsets.push(gid);
        gid += t.tiles.len() as u32;
    }
    let mut layers = vec![];
    for (i, l) in d.layers.iter().enumerate() {
        if let Some(ti) = l.tileset {
            let t = &d.tilesets[ti];
            let g = l.cel_meta[f].tilemap.as_ref();
            let columns = g.map_or(d.width.div_ceil(t.width), |g| g.columns);
            let rows = g.map_or(d.height.div_ceil(t.height), |g| g.rows);
            let data: Vec<u32> = l.tile_cels[f]
                .iter()
                .map(|id| {
                    let n = id & 0x1fffffff;
                    if n == 0 {
                        0
                    } else {
                        (id & 0xe0000000) | (n - 1 + offsets[ti])
                    }
                })
                .collect();
            layers.push(json!({"id":i+1,"type":"tilelayer","name":l.name,"width":columns,"height":rows,"x":0,"y":0,"offsetx":g.map_or(0,|g|g.x),"offsety":g.map_or(0,|g|g.y),"visible":l.visible,"opacity":l.opacity as f32/255.,"data":data}));
        }
    }
    if layers.is_empty() {
        return Err("No tile layers to export".into());
    }
    write_json(
        path,
        &json!({"type":"map","version":"1.10","tiledversion":"1.10","orientation":"orthogonal","renderorder":"right-down","infinite":false,"width":d.width.div_ceil(first.width),"height":d.height.div_ceil(first.height),"tilewidth":first.width,"tileheight":first.height,"layers":layers,"tilesets":sets,"nextlayerid":d.layers.len()+1,"nextobjectid":1}),
    )
}
pub fn load_tiled(path: &Path) -> Result<Document, String> {
    load_tiled_with_path_check(path, &|_| Ok(()))
}

pub(crate) fn load_tiled_with_path_check(
    path: &Path,
    check_path: &impl Fn(&Path) -> Result<(), String>,
) -> Result<Document, String> {
    let v: Value = serde_json::from_slice(&mcp_io::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let num = |v: &Value, k: &str| {
        v[k].as_u64()
            .and_then(|x| usize::try_from(x).ok())
            .ok_or_else(|| format!("Missing Tiled {k}"))
    };
    let base = path.parent().unwrap_or(Path::new("."));
    fn tileset(
        v: &Value,
        base: &Path,
        check_path: &impl Fn(&Path) -> Result<(), String>,
        remaining_pixels: &mut usize,
    ) -> Result<Tileset, String> {
        let mut v = v.clone();
        let mut base = base.to_path_buf();
        if let Some(source) = v["source"].as_str() {
            let p = base.join(source);
            check_path(&p)?;
            v = serde_json::from_slice(&mcp_io::read(&p).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            base = p.parent().unwrap().into();
        }
        let n = |k: &str| {
            v[k].as_u64()
                .and_then(|x| usize::try_from(x).ok())
                .ok_or_else(|| format!("Missing tileset {k}"))
        };
        let (w, h, count) = (n("tilewidth")?, n("tileheight")?, n("tilecount")?);
        if w == 0
            || h == 0
            || w > MAX_DIMENSION
            || h > MAX_DIMENSION
            || count == 0
            || w.saturating_mul(h).saturating_mul(count) > MAX_PIXELS
        {
            return Err("Tileset exceeds limits".into());
        }
        // Small tiles also allocate a Pixels entry and a separate backing vector.
        const TILE_OVERHEAD: usize = (std::mem::size_of::<Pixels>()
            + std::mem::size_of::<Vec<Pixel>>())
        .div_ceil(std::mem::size_of::<Pixel>());
        let storage = count.saturating_mul(w * h + TILE_OVERHEAD);
        *remaining_pixels = remaining_pixels
            .checked_sub(storage)
            .ok_or("Tiled document exceeds the storage budget")?;
        let columns = n("columns")?;
        if columns == 0 {
            return Err("Image collection tilesets are unsupported".into());
        }
        let image_path = base.join(v["image"].as_str().ok_or("Missing tileset image")?);
        check_path(&image_path)?;
        let im = mcp_io::open_image(image_path)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        let margin = usize::try_from(v["margin"].as_u64().unwrap_or(0))
            .map_err(|_| "Invalid tileset margin")?;
        let spacing = usize::try_from(v["spacing"].as_u64().unwrap_or(0))
            .map_err(|_| "Invalid tileset spacing")?;
        let x_step = w.checked_add(spacing).ok_or("Invalid tileset spacing")?;
        let y_step = h.checked_add(spacing).ok_or("Invalid tileset spacing")?;
        let max_x = (im.width() as usize)
            .checked_sub(w)
            .ok_or("Tile outside image")?;
        let max_y = (im.height() as usize)
            .checked_sub(h)
            .ok_or("Tile outside image")?;
        let coordinate = |index: usize, step: usize, edge| {
            index
                .checked_mul(step)
                .and_then(|offset| margin.checked_add(offset))
                .filter(|&start| start <= edge)
                .ok_or("Tile outside image")
        };
        let mut tiles = vec![];
        for i in 0..count {
            let x = coordinate(i % columns, x_step, max_x)?;
            let y = coordinate(i / columns, y_step, max_y)?;
            tiles.push(
                (0..w * h)
                    .map(|p| im.get_pixel((x + p % w) as u32, (y + p / w) as u32).0)
                    .collect::<Vec<_>>()
                    .into(),
            );
        }
        Ok(Tileset {
            editor_id: Default::default(),
            name: v["name"].as_str().unwrap_or("Tileset").into(),
            width: w,
            height: h,
            tiles,
        })
    }
    let mut remaining_pixels = MAX_DOCUMENT_PIXELS;
    if v["type"] == "tileset" {
        let t = tileset(&v, base, check_path, &mut remaining_pixels)?;
        let mut d = Document::new(t.width, t.height);
        d.tilesets.push(t);
        d.add_tile_layer(0)?;
        d.remove_layer(0);
        d.validate()?;
        return Ok(d);
    }
    if v["orientation"] != "orthogonal" || v["infinite"] == true {
        return Err("Tiled import supports finite orthogonal JSON maps".into());
    }
    let (tw, th) = (num(&v, "tilewidth")?, num(&v, "tileheight")?);
    let (w, h) = (
        num(&v, "width")?.saturating_mul(tw),
        num(&v, "height")?.saturating_mul(th),
    );
    if w == 0
        || h == 0
        || w > MAX_DIMENSION
        || h > MAX_DIMENSION
        || w.saturating_mul(h) > MAX_PIXELS
    {
        return Err("Tiled map exceeds limits".into());
    }
    let mut d = Document::new(w, h);
    let sets = v["tilesets"].as_array().ok_or("Missing tilesets")?;
    let mut gids = vec![];
    const MAX_GID: u32 = 0x1fff_ffff;
    for s in sets {
        let t = tileset(s, base, check_path, &mut remaining_pixels)?;
        if t.width != tw || t.height != th {
            return Err("Mixed tile sizes are unsupported".into());
        }
        let first = s["firstgid"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|&id| id > 0)
            .ok_or("Invalid first global tile ID")?;
        let end = first
            .checked_add(t.tiles.len() as u32)
            .filter(|&end| end <= MAX_GID + 1)
            .ok_or("Invalid first global tile ID")?;
        gids.push(first..end);
        d.tilesets.push(t);
    }
    let mut by_gid: Vec<_> = gids.iter().enumerate().collect();
    by_gid.sort_unstable_by_key(|(_, range)| range.start);
    if by_gid
        .windows(2)
        .any(|pair| pair[0].1.end > pair[1].1.start)
    {
        return Err("Overlapping global tile ID ranges".into());
    }
    for l in v["layers"].as_array().ok_or("Missing Tiled layers")? {
        if l["type"] != "tilelayer" {
            return Err("Tiled import supports tile layers only".into());
        }
        let data = l["data"]
            .as_array()
            .ok_or("Use uncompressed JSON tile arrays")?;
        let (cols, rows) = (num(l, "width")?, num(l, "height")?);
        if cols == 0
            || rows == 0
            || cols.saturating_mul(rows) != data.len()
            || cols.saturating_mul(rows) > MAX_PIXELS
        {
            return Err("Invalid tile layer dimensions".into());
        }
        let offset = |cell: &str, offset: &str, tile_size| {
            l[cell]
                .as_i64()
                .unwrap_or(0)
                .checked_mul(tile_size)
                .and_then(|cell| cell.checked_add(l[offset].as_i64().unwrap_or(0)))
                .and_then(|position| i32::try_from(position).ok())
                .ok_or("Invalid tile layer offset")
        };
        let x = offset("x", "offsetx", tw as i64)?;
        let y = offset("y", "offsety", th as i64)?;
        let mut used = std::collections::BTreeSet::new();
        if !gids.is_empty() {
            // Preserve the empty layer backed by the first tileset.
            used.insert(0);
        }
        for tile in data {
            let id = tile
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or("Invalid tile ID")?;
            let gid = id & MAX_GID;
            if gid == 0 {
                continue;
            }
            let index = by_gid.partition_point(|(_, range)| range.start <= gid);
            let (ti, _) = index
                .checked_sub(1)
                .and_then(|index| by_gid.get(index))
                .filter(|(_, range)| range.contains(&gid))
                .ok_or("Unknown tile ID")?;
            used.insert(*ti);
        }
        remaining_pixels = remaining_pixels
            .checked_sub(used.len().saturating_mul(data.len()))
            .ok_or("Tiled document exceeds the storage budget")?;
        for ti in used {
            if !d.can_add_layer() {
                return Err("Layer limit reached".into());
            }
            let range = &gids[ti];
            let entries = data
                .iter()
                .map(|tile| {
                    let id = tile.as_u64().unwrap() as u32;
                    let gid = id & MAX_GID;
                    if range.contains(&gid) {
                        (id & !MAX_GID) | (gid - range.start + 1)
                    } else {
                        0
                    }
                })
                .collect();
            d.add_layer();
            let layer = d.layers.last_mut().unwrap();
            layer.tileset = Some(ti);
            layer.name = l["name"].as_str().unwrap_or("Tile layer").into();
            layer.visible = l["visible"].as_bool().unwrap_or(true);
            layer.opacity =
                (l["opacity"].as_f64().unwrap_or(1.).clamp(0., 1.) * 255.).round() as u8;
            layer.tile_cels[0] = entries;
            layer.cel_meta[0].tilemap = Some(TilemapGeometry {
                x,
                y,
                columns: cols,
                rows,
            });
        }
    }
    if d.layers.len() > 1 {
        d.remove_layer(0)
    }
    d.validate()?;
    Ok(d)
}
