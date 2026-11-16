//! Headless commands and a newline-delimited JSON-RPC MCP stdio server.
use crate::{
    atlas::*,
    formats, interchange,
    model::*,
    productivity::*,
    workflows::{self, SheetOptions},
};
mod batch;
mod headless_lua;
pub(crate) mod mcp_access;
mod mcp_files;
pub(crate) mod mcp_io;
#[cfg(target_os = "linux")]
pub(crate) mod mcp_linux;
#[cfg(windows)]
mod mcp_windows;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    hash::{Hash, Hasher},
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc,
};
pub fn document_revision(d: &Document) -> String {
    struct HashWriter(std::collections::hash_map::DefaultHasher);
    impl Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.write(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut metadata = d.clone();
    let empty = Pixels::from(Vec::new());
    let mut cached = std::collections::HashMap::new();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut add_pixels = |slot: &mut Pixels| {
        let digest = cached
            .entry((slot.backing_id(), slot.len()))
            .or_insert_with(|| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                slot.len().hash(&mut h);
                slot.visit_nonclear(|index, pixel| {
                    index.hash(&mut h);
                    pixel.hash(&mut h);
                });
                h.finish()
            });
        digest.hash(&mut hash);
        *slot = empty.clone();
    };
    for layer in &mut metadata.layers {
        for cel in &mut layer.cels {
            add_pixels(cel);
        }
    }
    for reference in &mut metadata.references {
        add_pixels(&mut reference.pixels);
    }
    for tileset in &mut metadata.tilesets {
        for tile in &mut tileset.tiles {
            add_pixels(tile);
        }
    }
    let mut writer = std::io::BufWriter::with_capacity(64 * 1024, HashWriter(hash));
    serde_json::to_writer(&mut writer, &metadata).expect("Hash writer accepts all bytes");
    let writer = writer.into_inner().ok().expect("Hash writer cannot fail");
    format!("{:016x}", writer.0.finish())
}
pub fn inspect(d: &Document) -> Value {
    let mut result = inspect_revision(d, &document_revision(d));
    if let Some(info) = result.as_object_mut() {
        info.remove("document_id");
        info.remove("capabilities");
        if let Some(layers) = info["layers"].as_array_mut() {
            for layer in layers {
                if let Some(layer) = layer.as_object_mut() {
                    layer.remove("id");
                }
            }
        }
    }
    result
}
pub fn inspect_revision(d: &Document, revision: &str) -> Value {
    let layers: Vec<_> = d.layers.iter().enumerate().map(|(i, layer)| {
        json!({"index":i,"id":layer.editor_id.to_string(),"name":layer.name,"locked":layer.locked,
            "group":layer.group,"visible":layer.visible,"parent":layer.parent})
    }).collect();
    let mut info = json!({"revision":revision,"document_id":d.editor_id.to_string(),
        "capabilities":{"stable_targets":true,"command_preview":1},
        "width":d.width,"height":d.height,"frames":d.durations,"layers":layers,
        "tags":d.tags,"slices":d.slices,"palette":d.palette,"references":d.references.iter()
            .map(|r|json!({"name":r.name,"x":r.x,"y":r.y,"scale":r.scale,"opacity":r.opacity}))
            .collect::<Vec<_>>()});
    if d.has_frame_names() {
        info["frame_names"] = json!(
            (0..d.durations.len())
                .map(|f| d.frame_name(f))
                .collect::<Vec<_>>()
        );
    }
    info
}
pub fn inspect_cel_page(doc: &Document, args: &Value) -> Result<Value, String> {
    let layer = resolve_layer(doc, args)?;
    let offset = args
        .get("frame")
        .map(|_| number(args, "frame"))
        .transpose()?
        .unwrap_or(0);
    let count = args
        .get("count")
        .map(|_| number(args, "count"))
        .transpose()?
        .unwrap_or(100);
    if offset >= doc.durations.len() || !(1..=512).contains(&count) {
        return Err("Choose a valid start frame and 1–512 cel targets".into());
    }
    let entry = &doc.layers[layer];
    Ok(
        json!({"document_id":doc.editor_id.to_string(),"layer_id":entry.editor_id.to_string(),
        "cels":entry.cel_meta.iter().enumerate().skip(offset).take(count).map(|(frame, cel)| {
            json!({"frame":frame,"id":cel.editor_id.to_string(),"artwork_link":cel.artwork_link})
        }).collect::<Vec<_>>() }),
    )
}

pub fn verify_document_target(d: &Document, args: &Value) -> Result<(), String> {
    if let Some(id) = args.get("document_id")
        && id.as_str() != Some(&d.editor_id.to_string())
    {
        return Err("The requested document is no longer active. Inspect it again.".into());
    }
    Ok(())
}

fn resolve_layer(doc: &Document, args: &Value) -> Result<usize, String> {
    if let Some(id) = args.get("layer_id") {
        return doc
            .layers
            .iter()
            .position(|l| id.as_str() == Some(&l.editor_id.to_string()))
            .ok_or_else(|| "Target layer no longer exists. Inspect the sprite again.".into());
    }
    number(args, "layer").and_then(|layer| {
        (layer < doc.layers.len())
            .then_some(layer)
            .ok_or_else(|| "Invalid layer".into())
    })
}

fn resolve_cel(doc: &Document, args: &Value) -> Result<(usize, usize), String> {
    let layer = resolve_layer(doc, args)?;
    if let Some(id) = args.get("cel_id") {
        let frame = doc.layers[layer]
            .cel_meta
            .iter()
            .position(|meta| id.as_str() == Some(&meta.editor_id.to_string()))
            .ok_or("Target cel no longer exists. Inspect the sprite again.")?;
        return Ok((layer, frame));
    }
    let frame = number(args, "frame")?;
    if frame >= doc.durations.len() {
        return Err("Invalid frame".into());
    }
    Ok((layer, frame))
}

pub fn preview_commands(
    doc: &Document,
    frame: usize,
    args: &Value,
    revision: &str,
    deadline: Option<std::time::Instant>,
) -> Result<Value, String> {
    if args["revision"] != revision {
        return Err("Stale revision; inspect the sprite again".into());
    }
    let prospective = apply_command_batch_until(doc, args, deadline)?;
    let prospective_frame = frame.min(prospective.durations.len() - 1);
    let mut result = render_preview(&prospective, prospective_frame, args, revision, 4_194_304)?;
    result["changed_cels"] = changed_cel_bounds(doc, &prospective);
    result["visual_change"] = json!(!prospective.same_frame_pixels(doc, prospective_frame));
    result["preview"] = json!(true);
    Ok(result)
}

pub fn apply_edits(d: &Document, v: &Value) -> Result<Document, String> {
    if v["revision"].as_str() != Some(&document_revision(d)) {
        return Err("Stale revision; inspect the document again".into());
    }
    apply_pixel_batch(d, v)
}
pub fn apply_pixel_batch(d: &Document, v: &Value) -> Result<Document, String> {
    verify_document_target(d, v)?;
    let edits = v["edits"].as_array().ok_or("Expected edits array")?;
    if edits.len() > 1_000_000 {
        return Err("Too many edits".into());
    }
    let mut out = d.clone();
    for e in edits {
        let (l, f) = resolve_cel(&out, e)?;
        let x = e["x"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("Invalid x")?;
        let y = e["y"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or("Invalid y")?;
        if l >= d.layers.len()
            || f >= d.durations.len()
            || !d.editable(l)
            || d.layers[l].tileset.is_some()
        {
            return Err("Choose an unlocked image layer and valid frame".into());
        }
        let i = d.index(x, y).ok_or("Pixel outside canvas")?;
        let color: Pixel = serde_json::from_value(e["color"].clone())
            .map_err(|_| "Color must contain four RGBA bytes")?;
        out.set_pixel(l, f, i, color);
    }
    out.validate()?;
    Ok(out)
}
/// Apply a complete assistant transaction to a private copy; failures never mutate the source.
pub fn apply_commands(d: &Document, v: &Value) -> Result<Document, String> {
    if v["revision"].as_str() != Some(&document_revision(d)) {
        return Err("Stale revision; inspect the document again".into());
    }
    apply_command_batch(d, v)
}

pub fn apply_command_batch(d: &Document, v: &Value) -> Result<Document, String> {
    apply_command_batch_until(d, v, None)
}

pub fn apply_command_batch_until(
    d: &Document,
    v: &Value,
    deadline: Option<std::time::Instant>,
) -> Result<Document, String> {
    verify_document_target(d, v)?;
    let commands = v["commands"].as_array().ok_or("Expected commands array")?;
    if commands.len() > 1024 {
        return Err("Too many commands".into());
    }
    let mut out = d.clone();
    let mut paint_budget = 4_000_000usize;
    for c in commands {
        if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            return Err("Live request expired; no changes applied".into());
        }
        let layer = || resolve_layer(&out, c);
        let frame = || {
            if c.get("cel_id").is_some() {
                return resolve_cel(&out, c).map(|(_, frame)| frame);
            }
            let frame = number(c, "frame")?;
            if frame >= out.durations.len() {
                return Err("Invalid frame".into());
            }
            Ok(frame)
        };
        match c["op"].as_str().ok_or("Missing command op")? {
            "add-layer" => {
                let n = out.layers.len();
                out.add_layer();
                if n == out.layers.len() {
                    return Err("Layer limit reached".into());
                }
                if let Some(name) = c["name"].as_str() {
                    out.layers[n].name = name.into();
                }
            }
            "remove-layer" => {
                let l = layer()?;
                if out.layers.len() == 1 {
                    return Err("Cannot remove last layer".into());
                }
                if !out.editable(l) {
                    return Err("Layer is locked".into());
                }
                out.remove_layer(l);
            }
            "set-layer" => {
                let l = layer()?;
                let target = &mut out.layers[l];
                if let Some(n) = c["name"].as_str() {
                    target.name = n.into();
                }
                if let Some(b) = c["visible"].as_bool() {
                    target.visible = b;
                }
                if let Some(b) = c["locked"].as_bool() {
                    target.locked = b;
                }
                if let Some(exclude) = c["exclude_from_onion"].as_bool() {
                    target.exclude_from_onion = exclude;
                }
                if let Some(link_duplicates) = c["link_duplicates"].as_bool() {
                    target.link_duplicates = link_duplicates;
                }
                if let Some(a) = c.get("opacity") {
                    target.opacity =
                        serde_json::from_value(a.clone()).map_err(|_| "Invalid opacity")?;
                }
            }
            "edit-tile-pixel" => {
                let color =
                    serde_json::from_value(c["color"].clone()).map_err(|_| "Invalid RGBA color")?;
                let mode = if let Some(value) = c.get("variant_mode") {
                    serde_json::from_value(value.clone())
                        .map_err(|_| "Invalid tile variant mode")?
                } else if c["variant"].as_bool().unwrap_or(false) {
                    crate::model::TilePixelEdit::ReuseVariant
                } else {
                    crate::model::TilePixelEdit::Shared
                };
                out.paint_tile_pixel(
                    layer()?,
                    frame()?,
                    (number(c, "x")?, number(c, "y")?),
                    (number(c, "pixel_x")?, number(c, "pixel_y")?),
                    color,
                    mode,
                )?;
            }
            "set-slice" => {
                let slice: SpriteSlice =
                    serde_json::from_value(c["slice"].clone()).map_err(|_| "Invalid slice")?;
                if let Some(existing) = out
                    .slices
                    .iter_mut()
                    .find(|existing| existing.name == slice.name)
                {
                    *existing = slice;
                } else {
                    out.slices.push(slice);
                }
            }
            "set-cel-keyframe" => {
                let keyframe = serde_json::from_value(c["keyframe"].clone())
                    .map_err(|_| "Invalid keyframe")?;
                out.set_cel_keyframe(layer()?, frame()?, keyframe)?;
            }
            "set-adjustment" => {
                let adjustment = serde_json::from_value(c["adjustment"].clone())
                    .map_err(|_| "Invalid adjustment parameters")?;
                out.set_adjustment(layer()?, adjustment)?;
            }
            "set-layer-mask" => {
                let layer = layer()?;
                let mask = if let Some(region) = c.get("region") {
                    let (x, y) = (number(region, "x")?, number(region, "y")?);
                    let (w, h) = (number(region, "width")?, number(region, "height")?);
                    if w == 0
                        || h == 0
                        || x.saturating_add(w) > out.width
                        || y.saturating_add(h) > out.height
                    {
                        return Err("Layer mask must be within the canvas".into());
                    }
                    paint_budget = paint_budget
                        .checked_sub(w * h)
                        .ok_or("Paint batch exceeds its work limit")?;
                    let mut mask = Pixels::empty(out.width * out.height);
                    for row in y..y + h {
                        for col in x..x + w {
                            mask.set(row * out.width + col, [0, 0, 0, 255]);
                        }
                    }
                    Some(mask)
                } else {
                    None
                };
                out.set_layer_mask(layer, mask)?;
            }
            "move-layer" => {
                let l = layer()?;
                let to = number(c, "to")?;
                if to >= out.layers.len() {
                    return Err("Invalid destination layer".into());
                }
                if l < to {
                    for i in l..to {
                        out.swap_layers(i, i + 1);
                    }
                } else {
                    for i in (to..l).rev() {
                        out.swap_layers(i, i + 1);
                    }
                }
            }
            "add-frame" => {
                let f = frame()?;
                let n = out.durations.len();
                if n >= 65535 {
                    return Err("Frame limit reached".into());
                }
                out.add_frame(f, c["duplicate"].as_bool().unwrap_or(false));
                if out.durations.len() == n {
                    return Err("Frame limit reached".into());
                }
            }
            "remove-frame" => {
                let f = frame()?;
                if out.durations.len() == 1 {
                    return Err("Cannot remove last frame".into());
                }
                out.remove_frame(f);
            }
            op @ ("move-cel-range"
            | "duplicate-cel-range"
            | "clear-cel-range"
            | "set-range-duration"
            | "link-cel-range"
            | "unlink-cel-range") => {
                let range = serde_json::from_value::<CelRange>(c["range"].clone())
                    .map_err(|_| "Invalid cel range")?;
                match op {
                    "clear-cel-range" => out.clear_cel_range(range)?,
                    "set-range-duration" => out.set_range_duration(
                        range,
                        u16::try_from(number(c, "duration")?).map_err(|_| "Invalid duration")?,
                    )?,
                    "link-cel-range" | "unlink-cel-range" => {
                        out.set_range_linked(range, op == "link-cel-range")?
                    }
                    _ => out.move_cel_range(
                        range,
                        number(c, "to_layer")?,
                        number(c, "to_frame")?,
                        op == "duplicate-cel-range",
                    )?,
                }
            }
            "link-cel" => out.link_cel(layer()?, frame()?, number(c, "source_frame")?)?,
            "unlink-cel" => out.unlink_cel(layer()?, frame()?)?,
            "set-frame" => {
                let f = frame()?;
                if c.get("duration").is_none() && c.get("name").is_none() {
                    return Err("Supply a frame name or duration".into());
                }
                if c.get("duration").is_some() {
                    let duration = number(c, "duration")?;
                    if !(1..=65535).contains(&duration) {
                        return Err("Duration must be 1–65535 ms".into());
                    }
                    out.durations[f] = duration as u16;
                }
                if let Some(name) = c.get("name") {
                    out.set_frame_name(f, name.as_str().ok_or("Expected a frame name")?)?;
                }
            }
            "set-tag" | "add-tag" => {
                let tag: FrameTag =
                    serde_json::from_value(c["tag"].clone()).map_err(|e| e.to_string())?;
                if c["op"] == "add-tag" {
                    out.tags.push(tag);
                } else {
                    let i = number(c, "index")?;
                    *out.tags.get_mut(i).ok_or("Invalid tag")? = tag;
                }
            }
            "remove-tag" => {
                let i = number(c, "index")?;
                if i >= out.tags.len() {
                    return Err("Invalid tag".into());
                }
                out.tags.remove(i);
            }
            op @ ("blit" | "stroke" | "clear-region" | "copy-region" | "move-region"
            | "recolor-region" | "polygon") => {
                let (layer, frame) = (layer()?, frame()?);
                crate::automation_paint::paint(&mut out, layer, frame, op, c, &mut paint_budget)?;
            }
            "shape" => {
                let l = layer()?;
                let f = frame()?;
                if !out.editable(l) || out.layers[l].tileset.is_some() {
                    return Err("Choose an unlocked image layer".into());
                }
                let coord = |k: &str| -> Result<i32, String> {
                    let n = number(c, k)?;
                    let max = if k.starts_with('x') {
                        out.width
                    } else {
                        out.height
                    };
                    if n >= max {
                        return Err("Shape outside canvas".into());
                    }
                    Ok(n as i32)
                };
                let a = (coord("x")?, coord("y")?);
                let b = (coord("x2")?, coord("y2")?);
                if deadline.is_some() {
                    let width = a.0.abs_diff(b.0) as usize + 1;
                    let height = a.1.abs_diff(b.1) as usize + 1;
                    let cost = if c["kind"] == "line" {
                        width.max(height)
                    } else {
                        width * height
                    };
                    paint_budget = paint_budget
                        .checked_sub(cost)
                        .ok_or("Paint batch exceeds its work limit")?;
                }
                let color: Pixel =
                    serde_json::from_value(c["color"].clone()).map_err(|_| "Invalid RGBA color")?;
                let filled = c["filled"].as_bool().unwrap_or(false);
                match c["kind"].as_str().unwrap_or("rectangle") {
                    "line" => out.line(l, f, a, b, color, 1),
                    "ellipse" => out.ellipse(l, f, a, b, color, filled, 1),
                    "rectangle" => {
                        for y in a.1.min(b.1)..=a.1.max(b.1) {
                            for x in a.0.min(b.0)..=a.0.max(b.0) {
                                if filled || x == a.0 || x == b.0 || y == a.1 || y == b.1 {
                                    let i = out.index(x, y).unwrap();
                                    out.set_pixel(l, f, i, color);
                                }
                            }
                        }
                    }
                    _ => return Err("Unknown shape".into()),
                }
            }
            _ => return Err("Unknown document command".into()),
        }
        out.validate()?;
    }
    Ok(out)
}
fn number(v: &Value, key: &str) -> Result<usize, String> {
    v[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("Expected nonnegative integer: {key}"))
}
fn path(v: &Value, key: &str) -> Result<PathBuf, String> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("Missing path: {key}"))
}
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct SheetExportOptions {
    #[serde(flatten)]
    sheet: SheetOptions,
    split_slices: bool,
    slice: Option<String>,
    extrude: usize,
}

fn options<T: serde::de::DeserializeOwned + Default>(v: &Value) -> Result<T, String> {
    if v.is_null() {
        Ok(T::default())
    } else {
        serde_json::from_value(v.clone()).map_err(|e| e.to_string())
    }
}
pub fn operation(op: &str, v: &Value) -> Result<Value, String> {
    file_operation(op, v, None)
}

fn write_operation_file<T>(
    files: Option<&mcp_files::Files<'_>>,
    out: &Path,
    encode: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    if let Some(files) = files {
        files.write_file(out, encode)
    } else {
        encode(out)
    }
}

fn file_operation(
    op: &str,
    v: &Value,
    files: Option<&mcp_files::Files<'_>>,
) -> Result<Value, String> {
    file_operation_mode(op, v, files, false)
}

fn file_operation_mode(
    op: &str,
    v: &Value,
    files: Option<&mcp_files::Files<'_>>,
    preview: bool,
) -> Result<Value, String> {
    let plan = |out: &Path, paths: Vec<PathBuf>, directory| {
        batch::preview_paths(files, out, paths, directory)
    };
    let resolve = |p| mcp_files::Files::resolve(files.and_then(|files| files.root), p);
    let argument_path = |key| path(v, key).map(resolve);
    let check_input = |p: &Path| files.map_or(Ok(()), |f| f.check_read(p));
    let input_path = |key| {
        let p = argument_path(key)?;
        check_input(&p)?;
        Ok::<_, String>(p)
    };
    let input_paths = |allow_directories| {
        v["inputs"]
            .as_array()
            .ok_or("Expected inputs array")?
            .iter()
            .map(|p| {
                let p = p.as_str().ok_or("Input must be a path")?;
                let p = resolve(p.into());
                if allow_directories {
                    if let Some(files) = files {
                        files.check_atlas_input(&p)?;
                    }
                } else {
                    check_input(&p)?;
                }
                Ok::<_, String>(p)
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let save = |out: &Path, d: &Document| {
        write_operation_file(files, out, |target| formats::save(target, d))
    };
    match op {
        "batch" => batch::execute(v, files),
        "create" => {
            let (w, h) = (number(v, "width")?, number(v, "height")?);
            if w == 0
                || h == 0
                || w > MAX_DIMENSION
                || h > MAX_DIMENSION
                || w.saturating_mul(h) > MAX_PIXELS
            {
                return Err("Invalid canvas dimensions".into());
            }
            let d = Document::new(w, h);
            let output = argument_path("output")?;
            if files.is_none() && output.exists() {
                return Err("Output already exists".into());
            }
            save(&output, &d)?;
            Ok(inspect(&d))
        }
        "atlas" => {
            let mut docs = vec![];
            let paths = input_paths(true)?;
            for p in expand_atlas_inputs_with_path_check(&paths, |p| {
                files.map_or(Ok(()), |files| files.check_atlas_input(p))
            })? {
                docs.push((
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    formats::load_with_path_check(&p, &check_input)?,
                ));
            }
            let out = argument_path("output")?;
            let options = options::<AtlasOptions>(&v["options"])?;
            if preview {
                let mut destinations = vec![out.clone()];
                if options.metadata != AtlasMetadata::None {
                    destinations.push(out.with_extension("json"));
                }
                return plan(&out, destinations, false);
            }
            if let Some(files) = files {
                let mut destinations = vec![out.clone()];
                if options.metadata != AtlasMetadata::None {
                    destinations.push(out.with_extension("json"));
                }
                files.write_files(&out, &destinations, false, |target| {
                    export_atlas(target, &docs, &options)
                })?;
            } else {
                export_atlas(&out, &docs, &options)?;
            }
            Ok(json!({"output":out}))
        }
        "import-sequence" => {
            let paths = input_paths(false)?;
            let d = interchange::load_sequence(&paths)?;
            save(&argument_path("output")?, &d)?;
            Ok(inspect(&d))
        }
        "import-grid" => {
            let im = read_image(&input_path("input")?)?;
            let d = import_grid_with_options(&im, &options::<GridImportOptions>(&v["options"])?)?;
            let out = argument_path("output")?;
            save(&out, &d)?;
            Ok(inspect(&d))
        }
        "palette-convert" => {
            let input = input_path("input")?;
            let output = argument_path("output")?;
            let p = parse_palette(
                &mcp_io::read(&input).map_err(|e| e.to_string())?,
                extension(&input),
            )?;
            write_operation_file(files, &output, |target| {
                mcp_io::write(target, write_palette(&p, extension(target))?)
                    .map_err(|e| e.to_string())
            })?;
            Ok(json!({"colors":p.len(),"output":output}))
        }
        _ => {
            let input = input_path("input")?;
            let mut d = formats::load_with_path_check(&input, &check_input)?;
            match op {
                "save" | "save-as" => {
                    let revision = document_revision(&d);
                    if v["revision"] != revision {
                        return Err("Stale revision; inspect the document again".into());
                    }
                    let output = if op == "save" {
                        input
                    } else {
                        argument_path("output")?
                    };
                    if let Some(files) = files {
                        let metadata = save_metadata(&d, &output, &revision)?;
                        files.write_file(&output, |target| formats::save(target, &d))?;
                        Ok(metadata)
                    } else {
                        prepare_live_save(&d, output, &revision, op == "save")?.write()
                    }
                }
                "inspect" => Ok(inspect(&d)),
                "render" | "export" | "import" => {
                    if op == "render" && v.get("output").is_none() {
                        if preview {
                            return Err("Batch render needs an output PNG path".into());
                        }
                        let f = if v.get("frame").is_some() {
                            number(v, "frame")?
                        } else {
                            0
                        };
                        return render_preview(&d, f, v, &document_revision(&d), MAX_PIXELS);
                    }
                    let out = argument_path("output")?;
                    if op == "render" && (v.get("region").is_some() || v.get("layers").is_some()) {
                        if !extension(&out).eq_ignore_ascii_case("png")
                            || v.get("format").is_some_and(|format| format != "png")
                        {
                            return Err(
                                "Region or layer-filtered render output requires a PNG path".into(),
                            );
                        }
                        let frame = if v.get("frame").is_some() {
                            number(v, "frame")?
                        } else {
                            0
                        };
                        if preview {
                            let sprite = filter_export_layers(&d, v)?;
                            preview_geometry(&sprite, frame, v, MAX_PIXELS)?;
                            return plan(&out, vec![out.clone()], false);
                        }
                        let (image, mut metadata) =
                            preview_image(&d, frame, v, &document_revision(&d), MAX_PIXELS)?;
                        write_operation_file(files, &out, |target| {
                            mcp_io::write_image_as(target, &image, image::ImageFormat::Png)
                                .map_err(|e| e.to_string())
                        })?;
                        metadata["output"] = json!(out);
                        return Ok(metadata);
                    }
                    let kind = v["format"].as_str().unwrap_or_else(|| extension(&out));
                    match kind {
                        "tileset-atlas" if op == "export" => {
                            check_tileset_atlas_path(&out)?;
                            if preview {
                                tileset_atlas_layout(&d, v)?;
                                return plan(&out, vec![out.clone()], false);
                            }
                            let mut metadata = write_operation_file(files, &out, |target| {
                                write_tileset_atlas(target, &d, v, &document_revision(&d))
                            })?;
                            metadata["output"] = json!(out);
                            return Ok(metadata);
                        }
                        "png-sequence" => {
                            if preview {
                                if v.get("frame").is_some() {
                                    return Err(
                                        "PNG sequence export uses all timeline frames".into()
                                    );
                                }
                                let sprite = filter_export_layers(&d, v)?;
                                preview_geometry(&sprite, 0, v, MAX_PIXELS)?;
                                let policy = files.ok_or("Missing batch path policy")?;
                                if !policy.overwrite && mcp_io::metadata(&out).is_ok() {
                                    return Err(format!(
                                        "Output already exists: {}. Set overwrite to true to replace it.",
                                        out.display()
                                    ));
                                }
                                let paths =
                                    check_sequence_outputs(&out, &sequence_names(&d), policy)?;
                                return plan(&out, paths, true);
                            }
                            let replace = files.is_some_and(|files| files.overwrite);
                            if let Some(files) = files {
                                files.check_write(&out)?;
                                for name in sequence_names(&d) {
                                    files.check_write(&out.join(name))?;
                                }
                            }
                            return prepare_png_sequence_progress(
                                &d,
                                out,
                                v,
                                &document_revision(&d),
                                replace,
                                files.and_then(|files| files.root),
                                |_, _| Ok(()),
                            )?
                            .write();
                        }
                        "gif" | "mp4" => {
                            let filtered = filter_export_layers(&d, v)?;
                            let scale = export_scale(v)?;
                            if preview {
                                formats::animation_dimensions(&filtered, scale)?;
                                return plan(&out, vec![out.clone()], false);
                            }
                            if let Some(files) = files {
                                files.write_file(&out, |target| {
                                    export_animation(kind, target, &filtered, scale)
                                })?;
                            } else {
                                write_animation_export(kind, &out, &filtered, scale)?;
                            }
                        }
                        "sheet" => {
                            let mut options = options::<SheetExportOptions>(&v["options"])?;
                            workflows::SheetSpacing::new(&options.sheet, options.extrude)?;
                            workflows::validate_sheet_dimensions(&options.sheet)?;
                            let layers = export_layer_indices(v)?.or(options.sheet.layers.take());
                            let sprite =
                                workflows::filter_layers_for_export(&d, layers.as_deref())?;
                            let frames = options.sheet.frames_for(&sprite)?;
                            let plans = if options.split_slices || options.slice.is_some() {
                                interchange::plan_slice_sheets(
                                    &sprite,
                                    &options.sheet,
                                    options.extrude,
                                    options.slice.as_deref(),
                                )?
                            } else {
                                Vec::new()
                            };
                            if options.split_slices {
                                let paths: Vec<_> =
                                    plans.iter().map(|plan| out.join(&plan.file_name)).collect();
                                let destinations: Vec<_> = paths
                                    .iter()
                                    .flat_map(|path| {
                                        interchange::sheet_output_paths(path, &options.sheet)
                                    })
                                    .collect();
                                if preview {
                                    for (slice, path) in plans.iter().zip(&paths) {
                                        interchange::preflight_sheet_export(
                                            path,
                                            &sprite,
                                            &options.sheet,
                                            options.extrude,
                                            Some(slice),
                                        )?;
                                    }
                                    return plan(&out, destinations, true);
                                }
                                let cli_files = mcp_files::Files {
                                    root: None,
                                    overwrite: true,
                                };
                                files.unwrap_or(&cli_files).write_files(
                                    &out,
                                    &destinations,
                                    true,
                                    |target| {
                                        mcp_io::create_dir_all(target)
                                            .map_err(|e| e.to_string())?;
                                        for (plan, previous) in plans.iter().zip(&paths) {
                                            plan.export(
                                                &target.join(&plan.file_name),
                                                previous,
                                                &sprite,
                                                &options.sheet,
                                                options.extrude,
                                            )?;
                                        }
                                        Ok(())
                                    },
                                )?;
                                return Ok(json!({"outputs":paths}));
                            }
                            if preview {
                                if plans.is_empty()
                                    && (options.sheet.split_grid || options.sheet.split_layers)
                                {
                                    workflows::split_sheet_work(
                                        &sprite,
                                        &options.sheet,
                                        options.extrude,
                                        &frames,
                                        None,
                                    )?;
                                }
                                interchange::preflight_sheet_export(
                                    &out,
                                    &sprite,
                                    &options.sheet,
                                    options.extrude,
                                    plans.first(),
                                )?;
                                return plan(
                                    &out,
                                    interchange::sheet_output_paths(&out, &options.sheet),
                                    false,
                                );
                            }
                            let export = |target: &Path| {
                                if let Some(slice) = plans.first() {
                                    slice.export(
                                        target,
                                        &out,
                                        &sprite,
                                        &options.sheet,
                                        options.extrude,
                                    )
                                } else {
                                    interchange::export_sheet_extruded(
                                        target,
                                        &out,
                                        &sprite,
                                        &options.sheet,
                                        options.extrude,
                                    )
                                }
                            };
                            if let Some(files) = files {
                                files.write_files(
                                    &out,
                                    &interchange::sheet_output_paths(&out, &options.sheet),
                                    false,
                                    export,
                                )?;
                            } else {
                                export(&out)?;
                            }
                        }
                        "ase" | "aseprite"
                            if op == "export" && (d.needs_baked_ase() || d.has_frame_names()) =>
                        {
                            if preview {
                                return plan(&out, vec![out.clone()], false);
                            }
                            write_operation_file(files, &out, |target| {
                                formats::export_baked_ase(target, &d)
                            })?;
                        }
                        "pixel" | "ase" | "aseprite" => {
                            if preview {
                                return plan(&out, vec![out.clone()], false);
                            }
                            save(&out, &d)?;
                        }
                        kind if formats::raster_format(kind).is_some() => {
                            let format = formats::raster_format(kind).unwrap();
                            if formats::raster_format(extension(&out)) != Some(format) {
                                return Err("Export format must match the output extension".into());
                            }
                            let frame = v
                                .get("frame")
                                .map(|_| number(v, "frame"))
                                .transpose()?
                                .unwrap_or(0);
                            let custom = v.get("layers").is_some() || v.get("region").is_some();
                            if preview || custom {
                                let sprite = filter_export_layers(&d, v)?;
                                let geometry = preview_geometry(&sprite, frame, v, MAX_PIXELS)?;
                                formats::validate_raster_size(
                                    format,
                                    geometry.width,
                                    geometry.height,
                                    geometry.scale,
                                )?;
                                if preview {
                                    return plan(&out, vec![out.clone()], false);
                                }
                                let (image, mut metadata) = preview_composite(
                                    &sprite,
                                    frame,
                                    v,
                                    &document_revision(&d),
                                    MAX_PIXELS,
                                )?;
                                write_operation_file(files, &out, |target| {
                                    formats::write_raster_image(target, &image, format)
                                })?;
                                metadata["output"] = json!(out);
                                return Ok(metadata);
                            }
                            write_operation_file(files, &out, |target| {
                                formats::export_raster_scaled(
                                    target,
                                    &d,
                                    frame,
                                    false,
                                    export_scale(v)?,
                                )
                            })?;
                        }
                        _ => return Err("Unknown export format".into()),
                    }
                    Ok(json!({"output":out}))
                }
                "batch-tags" => {
                    let out = argument_path("output")?;
                    let gif = v["format"] == "gif";
                    let options = options::<SheetOptions>(&v["options"])?;
                    let paths = if let Some(files) = files {
                        let paths = tag_export_paths(&out, &d, gif);
                        let destinations = if gif {
                            paths.clone()
                        } else {
                            paths
                                .iter()
                                .flat_map(|path| interchange::sheet_output_paths(path, &options))
                                .collect()
                        };
                        if preview {
                            if !gif {
                                for (tag, path) in paths.iter().enumerate() {
                                    let options = SheetOptions {
                                        tag: Some(tag),
                                        frame_range: None,
                                        ..options.clone()
                                    };
                                    interchange::preflight_sheet_export(
                                        path, &d, &options, 0, None,
                                    )?;
                                }
                            }
                            return plan(&out, destinations, true);
                        }
                        files.write_files(&out, &destinations, true, |target| {
                            batch_tags_with_previous(target, &out, &d, gif, &options)
                        })?;
                        paths
                    } else {
                        batch_tags(&out, &d, gif, &options)?
                    };
                    Ok(json!({"outputs":paths}))
                }
                "palette-export" => {
                    let out = argument_path("output")?;
                    if preview {
                        write_palette(&d.palette, extension(&out))?;
                        return plan(&out, vec![out.clone()], false);
                    }
                    write_operation_file(files, &out, |target| {
                        mcp_io::write(target, write_palette(&d.palette, extension(target))?)
                            .map_err(|e| e.to_string())
                    })?;
                    Ok(json!({"output":out}))
                }
                "palette-import" => {
                    let p = input_path("palette")?;
                    let palette = parse_palette(
                        &mcp_io::read(&p).map_err(|e| e.to_string())?,
                        extension(&p),
                    )?;
                    d.import_palette(palette)?;
                    save(&argument_path("output")?, &d)?;
                    Ok(inspect(&d))
                }
                "edit" | "commands" => {
                    let d = if op == "commands" {
                        apply_commands(&d, v)?
                    } else {
                        apply_edits(&d, v)?
                    };
                    save(&argument_path("output")?, &d)?;
                    Ok(inspect(&d))
                }
                _ => Err(format!("Unknown operation: {op}")),
            }
        }
    }
}
fn extension(p: &Path) -> &str {
    p.extension().and_then(|s| s.to_str()).unwrap_or("")
}
const OPERATIONS: &[&str] = &[
    "batch",
    "save",
    "save-as",
    "create",
    "commands",
    "inspect",
    "render",
    "edit",
    "export",
    "import",
    "import-sequence",
    "import-grid",
    "batch-tags",
    "atlas",
    "palette-import",
    "palette-export",
    "palette-convert",
];
pub const CLI_HELP: &str = "PixelCraft headless tools\n  pixelcraft --cli OPERATION '{JSON arguments}'\n  pixelcraft --cli OPERATION --input FILE --output FILE [--options '{JSON}']\n  pixelcraft --mcp [--live BRIDGE_CONFIG] [--project-root DIRECTORY]\nOperations: batch, create, commands, inspect, render, edit, save, save-as, export, import, import-sequence, import-grid, batch-tags, atlas, palette-import, palette-export, palette-convert, lua, lua-console\nPaths: input, output; atlas and import-sequence use inputs (JSON array), palette-import uses palette.\nExport: format=png|png-sequence|tileset-atlas|bmp|ico|pcx|qoi|tga|jpeg|webp|gif|mp4|sheet|pixel|aseprite; scale is a positive integer; frame is zero-based. MP4 requires FFmpeg with libx264 on PATH.\nBatch takes steps: [{operation, arguments}], dry_run (optional), and overwrite (optional). Export steps are preflighted before any writes.\nCLI Lua takes script, optional input/output, params (JSON object), frame/layer, and overwrite. Without input it starts a 16x16 sprite (override with width and height).\nCLI lua-console takes the same canvas options and reads commands on stdin. Use :save to write output or :quit to discard.\nTileset atlas export takes a PNG output and optional layers (zero-based array) and columns; it defaults to visible tilemap layers.\nSave takes input and revision; save-as also takes output.\nGrid options: width,height,offset_x,offset_y,spacing_x,spacing_y,layers,skip_transparent,duration (ms, default 100).\nSheet options: scale,layout (rows/columns/horizontal/vertical/packed),width,height,columns,rows,padding,trim,tag,preset (Generic/Godot/Unity),layers (zero-based indices),frame_range (inclusive zero-based pair, intersected with tag).\nExport format=sheet also accepts extrude (0–256 pixels before scaling) and slice (exact authored name), split_slices (true uses output as a directory), split_layers, split_grid, filename_template, tag_template, border_padding, shape_padding, inner_padding, and trim_by_grid.\nAtlas options: layout (Rows/MaxRects),width,padding,trim,deduplicate,power_of_two,metadata (array/hash/none).\nEdits: revision from inspect, edits:[{layer,frame,x,y,color:[r,g,b,a]}].\nCreate: width,height,output (must not exist). Commands: revision,commands:[{op,...}]. Ops: add-layer(name), set-layer(layer,name,visible,locked,opacity), move-layer(layer,to), remove-layer(layer), add-frame(frame,duplicate), set-frame(frame,name and/or duration ms), remove-frame(frame), add-tag(tag), set-tag(index,tag), remove-tag(index), shape(layer,frame,kind=line|rectangle|ellipse,x,y,x2,y2,color RGBA,filled). Tags: name,from,to,direction (0 forward/1 reverse/2 pingpong/3 reverse pingpong),repeat (0 infinite),color RGB. All indices start at 0. File commands use input/output; live commands require the live-inspect revision and are one Undo step. Live-create takes width,height and a live revision; Undo restores the previous artwork.\nMCP exposes these operations and live-inspect, live-render, live-edit, live-commands, live-create, live-undo, live-redo, live-save, live-save-as and live-export. Live access must first be enabled in the editor's Workflow tools.\n";
pub fn run_cli(args: &[String]) -> Result<Value, String> {
    let op = args.first().ok_or(CLI_HELP)?;
    let v = cli_arguments(&args[1..])?;
    if op == "lua" {
        headless_lua::run(&v)
    } else {
        operation(op, &v)
    }
}
/// Runs the terminal console with process stdin, stdout, and stderr.
pub fn run_cli_console(args: &[String]) -> Result<(), String> {
    headless_lua::console(&cli_arguments(args)?)
}

fn cli_arguments(args: &[String]) -> Result<Value, String> {
    let mut v = json!({});
    if args.first().is_some_and(|s| s.starts_with('{')) {
        if args.len() != 1 {
            return Err("Unexpected extra arguments".into());
        }
        v = serde_json::from_str(&args[0]).map_err(|e| e.to_string())?;
    } else {
        let mut i = 0;
        while i < args.len() {
            let key = args[i]
                .strip_prefix("--")
                .ok_or("Expected --argument value")?;
            let key = if key == "dry-run" { "dry_run" } else { key };
            let value = args.get(i + 1).ok_or("Missing argument value")?;
            v[key] = if [
                "inputs",
                "options",
                "edits",
                "commands",
                "frame",
                "width",
                "height",
                "scale",
                "layers",
                "columns",
                "steps",
                "dry_run",
                "overwrite",
                "params",
                "layer",
            ]
            .contains(&key)
            {
                serde_json::from_str(value).map_err(|e| e.to_string())?
            } else {
                json!(value)
            };
            i += 2;
        }
    }
    Ok(v)
}

pub fn serve_mcp(live: Option<&Path>) -> Result<(), String> {
    serve_mcp_with_root(live, None)
}

pub fn serve_mcp_with_root(live: Option<&Path>, root: Option<&Path>) -> Result<(), String> {
    let root = root.map(mcp_files::project_root).transpose()?;
    let root = root.as_deref();
    let project = root.map(mcp_access::Project::new).transpose()?;
    let project = project.as_ref();
    let stdin = std::io::stdin();
    std::thread::scope(|scope| {
        let stdout = std::sync::Arc::new(std::sync::Mutex::new(std::io::stdout()));
        let send = move |response: &Value| -> Result<(), String> {
            let mut stdout = stdout.lock().map_err(|e| e.to_string())?;
            writeln!(stdout, "{response}").map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())
        };
        let mut exports = Vec::<std::thread::ScopedJoinHandle<'_, ()>>::new();
        for line in stdin.lock().lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(error) => {
                    send(
                        &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":error.to_string()}}),
                    )?;
                    continue;
                }
            };
            if live.is_some()
                && request["method"] == "tools/call"
                && request["params"]["name"] == "live-export"
            {
                exports.retain(|job| !job.is_finished());
                if exports.len() >= 8 {
                    send(
                        &json!({"jsonrpc":"2.0","id":request["id"],"result":{"isError":true,"content":[{"type":"text","text":"Too many concurrent exports"}]}}),
                    )?;
                    continue;
                }
                let send = send.clone();
                exports.push(scope.spawn(move || {
                    if let Some(response) = rpc_with_root(&request, live, root, project) {
                        let _ = send(&response);
                    }
                }));
            } else if let Some(response) = rpc_with_root(&request, live, root, project) {
                send(&response)?;
            }
        }
        Ok(())
    })
}

pub fn rpc(r: &Value, live: Option<&Path>) -> Option<Value> {
    rpc_with_root(r, live, None, None)
}

fn rpc_with_root(
    r: &Value,
    live: Option<&Path>,
    root: Option<&Path>,
    project: Option<&std::sync::Arc<mcp_access::Project>>,
) -> Option<Value> {
    r.get("id")?;
    let method = r["method"].as_str().unwrap_or("");
    let result: Result<Value, String> = match method {
        "initialize" => Ok(
            json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"PixelCraft","version":env!("CARGO_PKG_VERSION")}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let mut names = OPERATIONS.to_vec();
            if live.is_some() {
                names.extend([
                    "live-inspect",
                    "live-cel-targets",
                    "live-render",
                    "live-export",
                    "live-export-progress",
                    "live-cancel-export",
                    "live-save",
                    "live-save-as",
                    "live-edit",
                    "live-commands",
                    "live-preview-commands",
                    "live-create",
                    "live-undo",
                    "live-redo",
                ]);
            }
            Ok(json!({"tools":names.iter().map(|name| {
                    let details = match *name {
                        "live-commands" | "commands" => "Atomic edits with the current revision. Set frame name and/or timing with set-frame (frame,name,duration ms). Raster ops: blit (layer,frame,x,y,width,height,data,encoding,mode); stroke (layer,frame,points [[x,y],...],color RGBA,size,end_size,spacing,placements,scatter,seed,ramp,stamp,alternate,shade,opacity,mode,pressure,mask); polygon (layer,frame,points,color,alternate,dither_level,edge_noise,seed); clear-region, copy-region, move-region, recolor-region. Raster data is straight RGBA8, row-major, encoding base64 or base64-zlib. See the PixelCraft live painting documentation for full arguments.",
                        "live-preview-commands" => "Preview an atomic command batch at the supplied revision without changing the document or undo history.",
                        "batch" => "Run ordered file export steps, or set dry_run to list output paths without writing files.",
                        "live-render" => "Return a PNG of the active sprite or a crop/layer subset. Optional revision, frame, layers, region, and integer scale.",
                        "live-export" => "Export a revision-matched sprite as PNG, BMP, ICO, PCX, QOI, TGA, JPEG, WebP, GIF, MP4, a PNG sequence directory, or an authored tileset atlas PNG. Use format png-sequence or tileset-atlas for those exports. Layers apply to all formats. Region applies to raster images and PNG sequences. Frame applies to single-frame raster exports.",
                        "live-cel-targets" => "List up to 512 cel IDs for a layer, starting at the requested frame.",
                        "live-export-progress" => "Report the completed and total frames of a running live export by job_id.",
                        "live-cancel-export" => "Request cancellation of a running live export by job_id.",
                        "live-save" => "Save the revision-matched sprite to its current project path. Use live-save-as for untitled sprites.",
                        "live-save-as" => "Save the revision-matched sprite to a .pixel or .aseprite path and make it the current project path.",
                        _ => CLI_HELP,
                    };
                    let mut tool = json!({"name":name,"description":format!("PixelCraft {name}. {details}"),
                    "inputSchema":{"type":"object","properties":{"input":{"type":"string"},"output":{"type":"string"},"revision":{"type":"string"},"document_id":{"type":"string"},"edits":{"type":"array","items":{"type":"object"}},"options":{"type":"object"},"inputs":{"type":"array","items":{"type":"string"}},"format":{"type":"string"},"frame":{"type":"integer"},"palette":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"commands":{"type":"array","items":{"type":"object"}},"scale":{"type":"integer","minimum":1}},"additionalProperties":false}});
                if (OPERATIONS.contains(name) && *name != "inspect")
                    || matches!(*name, "live-save-as" | "live-export")
                {
                    tool["inputSchema"]["properties"]["overwrite"] = json!({
                        "type": "boolean",
                        "default": false,
                        "description": "Allow replacement of existing output files, including generated companions."
                    });
                }
                if matches!(*name, "render" | "live-render" | "export" | "live-export" | "live-preview-commands" | "live-commands" | "live-edit") {
                    let props = &mut tool["inputSchema"]["properties"];
                    props["layers"] = json!({"type":"array","items":{"type":"integer","minimum":0}});
                    props["region"] = json!({"type":"object","required":["x","y","width","height"],
                        "properties":{"x":{"type":"integer","minimum":0},"y":{"type":"integer","minimum":0},"width":{"type":"integer","minimum":1},"height":{"type":"integer","minimum":1}},
                        "additionalProperties":false});
                }
                if *name == "batch" {
                    let props = &mut tool["inputSchema"]["properties"];
                    props["steps"] = json!({"type":"array","minItems":1,"maxItems":64,"items":{
                        "type":"object","required":["operation","arguments"],"additionalProperties":false,
                        "properties":{
                            "operation":{"enum":["export","render","atlas","batch-tags","palette-export"]},
                            "arguments":{"type":"object"}
                        }
                    }});
                    props["dry_run"] = json!({"type":"boolean","default":false});
                    props["overwrite"] = json!({"type":"boolean","default":false});
                    tool["inputSchema"]["required"] = json!(["steps"]);
                }
                if matches!(*name, "export" | "live-export") {
                    tool["inputSchema"]["properties"]["columns"] = json!({
                        "type":"integer", "minimum":1, "maximum":MAX_DIMENSION,
                        "description":"Columns for tileset-atlas PNG exports."
                    });
                }
                if matches!(*name, "live-commands" | "live-edit") {
                    tool["inputSchema"]["properties"]["return_preview"] = json!({"type":"boolean"});
                }
                if *name == "import-grid" {
                    tool["inputSchema"]["properties"]["options"] = json!({
                        "type":"object","properties":{
                            "width":{"type":"integer","minimum":1,"maximum":MAX_DIMENSION,"default":16},
                            "height":{"type":"integer","minimum":1,"maximum":MAX_DIMENSION,"default":16},
                            "offset_x":{"type":"integer","minimum":0},"offset_y":{"type":"integer","minimum":0},
                            "spacing_x":{"type":"integer","minimum":0,"maximum":MAX_DIMENSION},
                            "spacing_y":{"type":"integer","minimum":0,"maximum":MAX_DIMENSION},
                            "layers":{"type":"boolean","default":false},
                            "skip_transparent":{"type":"boolean","default":false},
                            "duration":{"type":"integer","minimum":1,"maximum":65535,"default":DEFAULT_DURATION}
                        }
                    });
                    tool["inputSchema"]["required"] = json!(["input","output"]);
                }
                if *name == "live-cel-targets" {
                    tool["inputSchema"]["properties"]["layer"] = json!({"type":"integer","minimum":0});
                    tool["inputSchema"]["properties"]["layer_id"] = json!({"type":"string"});
                    tool["inputSchema"]["properties"]["count"] = json!({"type":"integer","minimum":1,"maximum":512});
                }
                if matches!(*name, "live-export" | "live-export-progress" | "live-cancel-export") {
                    tool["inputSchema"]["properties"]["job_id"] = json!({"type":"string","minLength":1,"maxLength":80});
                }
                if matches!(*name, "live-export-progress" | "live-cancel-export") {
                    tool["inputSchema"]["required"] = json!(["job_id"]);
                } else if *name == "save" {
                    tool["inputSchema"]["required"] = json!(["input","revision"]);
                } else if *name == "save-as" {
                    tool["inputSchema"]["required"] = json!(["input","output","revision"]);
                } else if matches!(*name, "live-export" | "live-save-as") {
                    tool["inputSchema"]["required"] = json!(["revision","output"]);
                } else if *name == "live-save" {
                    tool["inputSchema"]["required"] = json!(["revision"]);
                }
                tool
            }).collect::<Vec<_>>()}))
        }
        "tools/call" => {
            let name = r["params"]["name"].as_str().unwrap_or("");
            let args = &r["params"]["arguments"];
            let value = (|| {
                let overwrite = match args.get("overwrite") {
                    Some(value) => value.as_bool().ok_or("overwrite must be a boolean")?,
                    None => false,
                };
                let files = mcp_files::Files { root, overwrite };
                if name.starts_with("live-") {
                    let bridge = live.ok_or("Live editor access is disabled")?;
                    let mut args = args.clone();
                    if matches!(name, "live-export" | "live-save-as") {
                        let out = mcp_files::Files::resolve(root, path(&args, "output")?);
                        let preflight = || {
                            if name == "live-export" && args["format"] == "png-sequence" {
                                files.check_write(&out)
                            } else {
                                files.check_file(&out)
                            }
                        };
                        if let Some(project) = project {
                            project.run(preflight)?;
                        } else {
                            preflight()?;
                        }
                        args["output"] = json!(out);
                    }
                    live_call(
                        bridge,
                        name,
                        &args,
                        root,
                        project.and_then(|p| p.identity()),
                    )
                } else {
                    let operation = || file_operation(name, args, Some(&files));
                    if let Some(project) = project {
                        project.run(operation)
                    } else {
                        operation()
                    }
                }
            })();
            Ok(match value {
                Ok(mut v) if v["mimeType"] == "image/png" => {
                    let data = v.as_object_mut().and_then(|o| o.remove("data"));
                    json!({"content":[{"type":"image","mimeType":"image/png","data":data},{"type":"text","text":v.to_string()}]})
                }
                Ok(v) => json!({"content":[{"type":"text","text":v.to_string()}]}),
                Err(e) => json!({"isError":true,"content":[{"type":"text","text":e}]}),
            })
        }
        _ => Err("Method not found".into()),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":r["id"],"result":result}),
        Err(e) => json!({"jsonrpc":"2.0","id":r["id"],"error":{"code":-32601,"message":e}}),
    })
}

#[derive(Clone, Default)]
pub struct LiveExportProgress {
    phase: std::sync::Arc<std::sync::atomic::AtomicU8>,
    done: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    total: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl LiveExportProgress {
    pub fn cancel(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.phase.compare_exchange(0, 1, Relaxed, Relaxed).is_ok() || self.is_cancelled()
    }

    pub fn is_cancelled(&self) -> bool {
        self.phase.load(std::sync::atomic::Ordering::Relaxed) == 1
    }

    pub fn begin_commit(&self) -> Result<(), String> {
        use std::sync::atomic::Ordering::Relaxed;
        self.phase
            .compare_exchange(0, 2, Relaxed, Relaxed)
            .map(|_| ())
            .map_err(|_| "Export cancelled; no output published".into())
    }

    pub fn update(&self, done: usize, total: usize) -> Result<(), String> {
        self.total
            .store(total, std::sync::atomic::Ordering::Relaxed);
        self.done.store(done, std::sync::atomic::Ordering::Relaxed);
        if self.is_cancelled() {
            return Err("Export cancelled".into());
        }
        Ok(())
    }

    pub fn counts(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.done.load(Relaxed), self.total.load(Relaxed))
    }
}

type ActiveExports = std::sync::Mutex<std::collections::HashMap<String, LiveExportProgress>>;

struct ActiveExport<'a> {
    jobs: &'a ActiveExports,
    id: Option<String>,
}

impl Drop for ActiveExport<'_> {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            self.jobs.lock().unwrap().remove(id);
        }
    }
}

fn export_job_id(args: &Value) -> Result<&str, String> {
    args["job_id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 80)
        .ok_or_else(|| "Expected an export job_id of 1–80 characters".into())
}

pub struct LiveRequest {
    pub deadline: std::time::Instant,
    pub operation: String,
    pub arguments: Value,
    pub project_root: Option<PathBuf>,
    pub project_identity: Option<(u64, u64)>,
    pub response: mpsc::Sender<Result<Value, String>>,
    pub export_progress: LiveExportProgress,
}
pub struct LiveBridge {
    pub requests: mpsc::Receiver<LiveRequest>,
    pub config: PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl LiveBridge {
    pub fn start(config: PathBuf) -> Result<Self, String> {
        Self::start_with_wake(config, || {})
    }
    pub fn start_with_wake(
        config: PathBuf,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        let mut secret = [0u8; 32];
        #[cfg(unix)]
        {
            std::fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut secret))
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        {
            use std::hash::BuildHasher;
            for chunk in secret.chunks_mut(8) {
                chunk.copy_from_slice(
                    &std::collections::hash_map::RandomState::new()
                        .hash_one(std::time::SystemTime::now())
                        .to_ne_bytes(),
                );
            }
        }
        let token: String = secret.iter().map(|v| format!("{v:02x}")).collect();
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&config).map_err(|e| e.to_string())?;
        write!(
            file,
            "{}",
            json!({"address":addr.to_string(),"token":token,"restricted_filesystem":2})
        )
        .map_err(|e| e.to_string())?;
        let (tx, requests) = mpsc::channel();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            let jobs = std::sync::Arc::new(ActiveExports::default());
            let wake = std::sync::Arc::new(std::sync::Mutex::new(wake));
            let mut connections = Vec::<std::thread::JoinHandle<()>>::new();
            while !stopping.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        connections.retain(|connection| !connection.is_finished());
                        if connections.len() >= 16 {
                            continue;
                        }
                        let (stopping, jobs, wake, tx, token) = (
                            stopping.clone(),
                            jobs.clone(),
                            wake.clone(),
                            tx.clone(),
                            token.clone(),
                        );
                        connections.push(std::thread::spawn(move || {
                        let started = std::time::Instant::now();
                        let mut deadline = started + std::time::Duration::from_secs(10);
                        // Short individual waits allow revocation to interrupt an active read.
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(40)));
                        let result = (|| {
                            let line = read_live_line(
                                &mut stream,
                                started + std::time::Duration::from_secs(2),
                                &stopping,
                            )?;
                            let v: Value =
                                serde_json::from_str(&line).map_err(|e| e.to_string())?;
                            if v["token"] != token {
                                return Err("Invalid bridge token".into());
                            }
                            let operation = v["operation"].as_str().unwrap_or("");
                            let arguments = &v["arguments"];
                            if matches!(operation, "live-export-progress" | "live-cancel-export") {
                                let id = export_job_id(arguments)?;
                                let job = jobs.lock().unwrap().get(id).cloned().ok_or("Export is not running")?;
                                if operation == "live-cancel-export" {
                                    let accepted = job.cancel();
                                    return Ok(json!({"job_id":id,"cancelled":accepted}));
                                }
                                let (done, total) = job.counts();
                                return Ok(json!({"job_id":id,"completed_frames":done,"total_frames":total,"cancelled":job.is_cancelled()}));
                            }
                            if long_file_operation(operation) {
                                deadline = started + std::time::Duration::from_secs(300);
                            }
                            check_live_request(deadline, &stopping)?;
                            let (res_tx, res_rx) = mpsc::channel();
                            let export_progress = LiveExportProgress::default();
                            let job_id = if operation == "live-export" && arguments.get("job_id").is_some() {
                                let id = export_job_id(arguments)?.to_owned();
                                let mut jobs = jobs.lock().unwrap();
                                if jobs.contains_key(&id) {
                                    return Err("An export with this job_id is already running".into());
                                }
                                jobs.insert(id.clone(), export_progress.clone());
                                Some(id)
                            } else {
                                None
                            };
                            let _active_export = ActiveExport { jobs: &jobs, id: job_id };
                            let project_root = v["project_root"].as_str().map(PathBuf::from);
                            let project_identity: Option<(u64, u64)> =
                                serde_json::from_value(v["project_identity"].clone())
                                    .map_err(|_| "Invalid project root identity")?;
                            if project_root.is_some() && project_identity.is_none()
                                && matches!(operation, "live-save-as" | "live-export")
                            {
                                return Err("Restricted live file operations require the same PixelCraft version for the editor and MCP server".into());
                            }
                            tx.send(LiveRequest {
                                deadline,
                                operation: operation.into(),
                                arguments: arguments.clone(),
                                project_root,
                                project_identity,
                                response: res_tx,
                                export_progress: export_progress.clone(),
                            })
                            .map_err(|e| e.to_string())?;
                            wake.lock().unwrap()();
                            let result = wait_live_response_with_peer(&res_rx, deadline, &stopping, Some(&stream));
                            if result.is_err() {
                                export_progress.cancel();
                            }
                            result
                        })();
                        if !stopping.load(std::sync::atomic::Ordering::Relaxed) {
                            let remaining =
                                deadline.saturating_duration_since(std::time::Instant::now());
                            if !remaining.is_zero() {
                                let _ = stream.set_write_timeout(Some(
                                    remaining.min(std::time::Duration::from_millis(40)),
                                ));
                                let response = match result {
                                    Ok(v) => json!({"result":v}),
                                    Err(e) => json!({"error":e}),
                                };
                                if let Ok(mut bytes) = serde_json::to_vec(&response) {
                                    if bytes.len() >= MAX_LIVE_RESPONSE_BYTES {
                                        bytes = format!(
                                            "{{\"error\":\"{LIVE_RESPONSE_LIMIT_ERROR}\"}}"
                                        ).into_bytes();
                                    }
                                    bytes.push(b'\n');
                                    let _ =
                                        write_live_bytes(&mut stream, &bytes, deadline, &stopping);
                                }
                            }
                        }
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
            for connection in connections {
                let _ = connection.join();
            }
        });
        Ok(Self {
            requests,
            config,
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for LiveBridge {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.config);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn check_live_request(
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    if stop.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Live access disabled".into());
    }
    if std::time::Instant::now() >= deadline {
        return Err("Live request deadline exceeded".into());
    }
    Ok(())
}
fn read_live_line(
    reader: &mut impl Read,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<String, String> {
    let mut line = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        check_live_request(deadline, stop)?;
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                check_live_request(deadline, stop)?;
                let end = chunk[..n]
                    .iter()
                    .position(|&c| c == b'\n')
                    .map_or(n, |i| i + 1);
                if line.len() + end > 4_000_000 {
                    return Err("Request too large".into());
                }
                line.extend_from_slice(&chunk[..end]);
                if chunk[end - 1] == b'\n' {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    String::from_utf8(line).map_err(|e| e.to_string())
}
fn write_live_bytes(
    writer: &mut impl Write,
    mut bytes: &[u8],
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    while !bytes.is_empty() {
        check_live_request(deadline, stop)?;
        match writer.write(bytes) {
            Ok(0) => return Err("Bridge connection closed".into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    check_live_request(deadline, stop)
}
#[cfg(test)]
fn wait_live_response(
    receiver: &mpsc::Receiver<Result<Value, String>>,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Value, String> {
    wait_live_response_with_peer(receiver, deadline, stop, None)
}

fn wait_live_response_with_peer(
    receiver: &mpsc::Receiver<Result<Value, String>>,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
    peer: Option<&TcpStream>,
) -> Result<Value, String> {
    loop {
        check_live_request(deadline, stop)?;
        if let Some(peer) = peer {
            let mut next = [0];
            match peer.peek(&mut next) {
                Ok(0) => return Err("Live client disconnected".into()),
                Ok(_) => return Err("Expected one request per bridge connection".into()),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(e.to_string()),
            }
        }

        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(remaining.min(std::time::Duration::from_millis(20))) {
            Ok(result) => {
                check_live_request(deadline, stop)?;
                return result;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}
const MAX_LIVE_RESPONSE_BYTES: usize = 8_000_000;
const LIVE_RESPONSE_LIMIT_ERROR: &str =
    "Result exceeds the live response limit; request a smaller crop or scale";

pub fn check_live_response_size(value: &Value) -> Result<(), String> {
    struct ResponseBudget(usize);
    impl Write for ResponseBudget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other(LIVE_RESPONSE_LIMIT_ERROR))?;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let overhead = b"{\"result\":}\n".len();
    serde_json::to_writer(ResponseBudget(MAX_LIVE_RESPONSE_BYTES - overhead), value)
        .map_err(|error| error.to_string())
}

fn live_call(
    config: &Path,
    op: &str,
    args: &Value,
    root: Option<&Path>,
    identity: Option<(u64, u64)>,
) -> Result<Value, String> {
    let v: Value = serde_json::from_slice(&std::fs::read(config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if root.is_some() && long_file_operation(op) && v["restricted_filesystem"] != 2 {
        return Err("This editor does not support the restricted MCP filesystem protocol. Reopen the live bridge with the same PixelCraft version.".into());
    }
    let addr: std::net::SocketAddr = v["address"]
        .as_str()
        .ok_or("Missing address")?
        .parse()
        .map_err(|_| "Invalid bridge address")?;
    if !addr.ip().is_loopback() {
        return Err("Live bridge must use loopback".into());
    }
    let mut stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(
            if long_file_operation(op) { 302 } else { 12 },
        )))
        .map_err(|e| e.to_string())?;
    writeln!(
        stream,
        "{}",
        json!({"token":v["token"],"operation":op,"arguments":args,"project_root":root,"project_identity":identity})
    )
    .map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::BufReader::new(stream)
        .take(MAX_LIVE_RESPONSE_BYTES as u64)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let result: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if let Some(e) = result["error"].as_str() {
        Err(e.into())
    } else {
        Ok(result["result"].clone())
    }
}

pub fn render_document(d: &Document, f: usize) -> Result<Value, String> {
    render_preview(d, f, &Value::Null, &document_revision(d), MAX_PIXELS)
}

pub fn render_preview(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    limit: usize,
) -> Result<Value, String> {
    let (image, mut metadata) = preview_image(d, f, args, revision, limit)?;
    let mut png = std::io::Cursor::new(vec![]);
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    if limit != MAX_PIXELS && png.get_ref().len() > 5_900_000 {
        return Err("PNG exceeds the live response limit; request a smaller crop or scale".into());
    }
    metadata["mimeType"] = json!("image/png");
    metadata["data"] = json!(STANDARD.encode(png.get_ref()));
    Ok(metadata)
}

fn long_file_operation(op: &str) -> bool {
    matches!(op, "live-save" | "live-save-as" | "live-export")
}

fn export_scale(args: &Value) -> Result<usize, String> {
    if args.get("scale").is_some() {
        number(args, "scale")
    } else {
        Ok(1)
    }
}

fn export_animation(kind: &str, output: &Path, doc: &Document, scale: usize) -> Result<(), String> {
    match kind {
        "gif" => formats::export_gif_scaled(output, doc, scale),
        "mp4" => formats::export_mp4(output, doc, scale),
        _ => Err("Unknown animation format".into()),
    }
}

/// Save an editor sheet with its metadata and engine sidecars as one staged export.
pub fn write_editor_sheet_export(
    output: &Path,
    doc: &Document,
    options: &SheetOptions,
    slice: Option<&str>,
) -> Result<(), String> {
    let files = mcp_files::Files {
        root: None,
        overwrite: true,
    };
    let destinations = interchange::sheet_output_paths(output, options);
    files.write_files(output, &destinations, false, |target| {
        interchange::export_sheet_for_slice_with_previous(target, output, doc, options, slice)
    })
}

pub fn write_animation_export(
    kind: &str,
    output: &Path,
    doc: &Document,
    scale: usize,
) -> Result<(), String> {
    prepare_output(output.to_path_buf(), true, |temporary| {
        export_animation(kind, temporary, doc, scale)?;
        Ok(json!({}))
    })?
    .write()
    .map(|_| ())
}

pub struct PreparedExport {
    pub output: PathBuf,
    staging: PathBuf,
    staging_identity: Option<(u64, u64)>,
    temporary_directory_identity: Option<(u64, u64)>,
    staged_files: Option<std::sync::Arc<mcp_io::StagedFiles>>,
    preserve_staging: bool,
    temporary: PathBuf,
    metadata: Value,
    replace: bool,
    sequence: Vec<String>,
    project_root: Option<PathBuf>,
    access: Option<std::sync::Arc<mcp_access::Project>>,
}

impl Drop for PreparedExport {
    fn drop(&mut self) {
        if self.preserve_staging {
            return;
        }
        let root = self.project_root.as_deref();
        let staging = &self.staging;
        let staging_identity = self.staging_identity;
        let temporary_directory = self
            .temporary
            .file_name()
            .zip(self.temporary_directory_identity);
        let cleanup = || {
            mcp_files::Files {
                root,
                overwrite: true,
            }
            .check_write(staging)?;
            mcp_io::remove_staging(
                staging,
                staging_identity,
                temporary_directory,
                self.staged_files.as_deref(),
            )
            .map_err(|error| error.to_string())
        };
        if let Some(access) = &self.access {
            let _ = access.run(cleanup);
        } else if root.is_none() {
            let _ = cleanup();
        }
    }
}

fn prepare_output(
    output: PathBuf,
    replace: bool,
    encode: impl FnOnce(&Path) -> Result<Value, String>,
) -> Result<PreparedExport, String> {
    let root = mcp_access::active_root();
    prepare_output_under(output, replace, root.as_deref(), false, encode)
}

fn prepare_output_under(
    output: PathBuf,
    replace: bool,
    root: Option<&Path>,
    directory: bool,
    encode: impl FnOnce(&Path) -> Result<Value, String>,
) -> Result<PreparedExport, String> {
    let files = mcp_files::Files {
        root,
        overwrite: replace,
    };
    let access = root
        .map(|root| {
            mcp_access::Project::active(root)
                .ok_or("Restricted MCP filesystem access was not established")
        })
        .transpose()?;
    files.check_write(&output)?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if !replace && mcp_io::exists(&output).map_err(|e| e.to_string())? {
        return Err("Output already exists. Choose a new path.".into());
    }
    let parent = output.parent().ok_or("Invalid output path")?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let resolved_parent;
    let parent = if root.is_some() {
        resolved_parent = mcp_io::directory_path(parent).map_err(|e| e.to_string())?;
        files.check_write(&resolved_parent)?;
        &resolved_parent
    } else {
        parent
    };
    let filename = output.file_name().ok_or("Invalid output filename")?;
    let (staging, staging_identity) = loop {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = parent.join(format!(".pixelcraft-output-{}-{n}", std::process::id()));
        match mcp_io::create_staging_dir(&dir) {
            Ok(identity) => break (dir, identity),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let staged_files = staging_identity.map(|_| mcp_io::track_staging(&staging));
    let temporary = staging.join(filename);
    let mut prepared = PreparedExport {
        output,
        staging,
        staging_identity,
        temporary_directory_identity: None,
        staged_files,
        preserve_staging: false,
        temporary,
        metadata: Value::Null,
        replace,
        sequence: vec![],
        project_root: root.map(Path::to_path_buf),
        access,
    };
    if directory && prepared.access.is_some() {
        prepared.temporary_directory_identity =
            mcp_io::create_staging_dir(&prepared.temporary).map_err(|error| error.to_string())?;
    }
    prepared.metadata = encode(&prepared.temporary)?;
    prepared.check_staging()?;
    Ok(prepared)
}

pub fn prepare_live_save(
    d: &Document,
    output: PathBuf,
    revision: &str,
    replace: bool,
) -> Result<PreparedExport, String> {
    prepare_live_save_under(d, output, revision, replace, None, None)
}

pub fn prepare_live_save_under(
    d: &Document,
    output: PathBuf,
    revision: &str,
    replace: bool,
    root: Option<&Path>,
    identity: Option<(u64, u64)>,
) -> Result<PreparedExport, String> {
    let metadata = save_metadata(d, &output, revision)?;
    mcp_access::run(root, identity, || {
        prepare_output_under(output, replace, root, false, |temporary| {
            formats::save(temporary, d)?;
            Ok(metadata)
        })
    })
}

fn save_metadata(d: &Document, output: &Path, revision: &str) -> Result<Value, String> {
    if !matches!(extension(output), "pixel" | "ase" | "aseprite") {
        return Err("Choose a .pixel, .ase or .aseprite project path".into());
    }
    let mut metadata = inspect_revision(d, revision);
    metadata["output"] = json!(output);
    check_live_response_size(&metadata)?;
    Ok(metadata)
}

pub fn prepare_png_sequence(
    doc: &Document,
    output: PathBuf,
    args: &Value,
    revision: &str,
) -> Result<PreparedExport, String> {
    prepare_png_sequence_progress(doc, output, args, revision, false, None, |_, _| Ok(()))
}

fn sequence_names(doc: &Document) -> Vec<String> {
    (1..=doc.durations.len())
        .map(|index| format!("frame-{index:04}.png"))
        .collect()
}

fn check_sequence_outputs(
    output: &Path,
    names: &[String],
    files: &mcp_files::Files<'_>,
) -> Result<Vec<PathBuf>, String> {
    files.check_write(output)?;
    match mcp_io::metadata(output) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(format!(
                "Sequence output must be a directory: {}",
                output.display()
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    names
        .iter()
        .map(|name| {
            let frame = output.join(name);
            files.check_file(&frame)?;
            Ok(frame)
        })
        .collect()
}

fn prepare_png_sequence_progress(
    doc: &Document,
    output: PathBuf,
    args: &Value,
    revision: &str,
    replace: bool,
    root: Option<&Path>,
    mut progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<PreparedExport, String> {
    if args.get("frame").is_some() {
        return Err("PNG sequence export uses all timeline frames".into());
    }
    let names = sequence_names(doc);
    let files = mcp_files::Files {
        root,
        overwrite: replace,
    };
    files.check_write(&output)?;
    if replace {
        check_sequence_outputs(&output, &names, &files)?;
    }
    let filtered = filter_export_layers(doc, args)?;
    let staging_target = if replace && mcp_io::metadata(&output).is_ok_and(|entry| entry.is_dir()) {
        output.join("frames")
    } else {
        output.clone()
    };
    let mut prepared = prepare_output_under(staging_target, replace, root, true, |temporary| {
        if root.is_none() {
            mcp_io::create_dir(temporary).map_err(|e| e.to_string())?;
        }
        let mut metadata = Value::Null;
        for (frame, name) in names.iter().enumerate() {
            progress(frame, names.len())?;
            let (image, frame_metadata) =
                preview_composite(&filtered, frame, args, revision, MAX_PIXELS)?;
            mcp_io::write_image_as(temporary.join(name), &image, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            if frame == 0 {
                metadata = frame_metadata;
            }
        }
        progress(names.len(), names.len())?;
        metadata
            .as_object_mut()
            .ok_or("No frames to export")?
            .remove("frame");
        metadata["format"] = json!("png-sequence");
        metadata["frames"] = json!(names.len());
        Ok(metadata)
    })?;
    prepared.output = output;
    prepared.sequence = names;
    Ok(prepared)
}

pub fn prepare_live_export(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
) -> Result<PreparedExport, String> {
    prepare_live_export_with_progress(d, f, args, revision, |_, _| Ok(()))
}

pub fn prepare_live_export_with_progress(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    progress: impl FnMut(usize, usize) -> Result<(), String> + Send,
) -> Result<PreparedExport, String> {
    prepare_live_export_under(d, f, args, revision, None, None, progress)
}

pub fn prepare_live_export_under(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    root: Option<&Path>,
    identity: Option<(u64, u64)>,
    progress: impl FnMut(usize, usize) -> Result<(), String> + Send,
) -> Result<PreparedExport, String> {
    mcp_access::run(root, identity, || {
        prepare_live_export_inner(d, f, args, revision, root, progress)
    })
}

fn check_tileset_atlas_path(output: &Path) -> Result<(), String> {
    if !extension(output).eq_ignore_ascii_case("png") {
        return Err("Choose a .png path for the tileset atlas".into());
    }
    Ok(())
}

fn tileset_atlas_layout(
    d: &Document,
    args: &Value,
) -> Result<interchange::TilesetAtlasLayout, String> {
    if ["frame", "region", "scale"]
        .iter()
        .any(|key| args.get(key).is_some())
    {
        return Err("Tileset atlases use authored tiles at native size".into());
    }
    let layers = args
        .get("layers")
        .map(|value| {
            serde_json::from_value::<Vec<usize>>(value.clone()).map_err(|_| "Invalid layer list")
        })
        .transpose()?;
    let scope = layers.as_deref().map_or(
        interchange::TilesetAtlasScope::Visible,
        interchange::TilesetAtlasScope::Selected,
    );
    let columns = args
        .get("columns")
        .map(|_| number(args, "columns"))
        .transpose()?;
    interchange::plan_tileset_atlas(d, scope, columns)
}

fn write_tileset_atlas(
    path: &Path,
    d: &Document,
    args: &Value,
    revision: &str,
) -> Result<Value, String> {
    let layout = tileset_atlas_layout(d, args)?;
    let image = interchange::render_tileset_atlas(d, &layout);
    mcp_io::write_image_as(path, &image, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    let mut metadata = serde_json::to_value(layout).map_err(|error| error.to_string())?;
    metadata["format"] = json!("tileset-atlas");
    metadata["revision"] = json!(revision);
    Ok(metadata)
}

fn prepare_live_export_inner(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    root: Option<&Path>,
    mut progress: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<PreparedExport, String> {
    let output = path(args, "output")?;
    let replace = args["overwrite"] == true;
    if args["format"] == "png-sequence" {
        return prepare_png_sequence_progress(d, output, args, revision, replace, root, progress);
    }
    mcp_files::Files {
        root,
        overwrite: replace,
    }
    .check_file(&output)?;
    if args["format"] == "tileset-atlas" {
        check_tileset_atlas_path(&output)?;
        return prepare_output_under(output, replace, root, false, |temporary| {
            progress(0, 1)?;
            let metadata = write_tileset_atlas(temporary, d, args, revision)?;
            progress(1, 1)?;
            Ok(metadata)
        });
    }
    let kind = extension(&output).to_ascii_lowercase();
    let raster = formats::raster_format(&kind);
    if args.get("format").is_some_and(|value| {
        value != kind.as_str()
            && !(raster.is_some() && value.as_str().and_then(formats::raster_format) == raster)
    }) {
        return Err("Export format must match the output extension".into());
    }
    prepare_output_under(output, replace, root, false, |temporary| {
        match kind.as_str() {
            _ if let Some(format) = raster => {
                progress(0, 1)?;
                let sprite = filter_export_layers(d, args)?;
                let geometry = preview_geometry(&sprite, f, args, MAX_PIXELS)?;
                formats::validate_raster_size(format, geometry.width, geometry.height, geometry.scale)?;
                let (image, metadata) = preview_composite(&sprite, f, args, revision, MAX_PIXELS)?;
                formats::write_raster_image(temporary, &image, format)?;
                progress(1, 1)?;
                Ok(metadata)
            }
            "gif" | "mp4" => {
                if ["frame", "region"]
                    .iter()
                    .any(|key| args.get(key).is_some())
                {
                    return Err("GIF and MP4 export all frames and do not support cropping".into());
                }
                let scale = export_scale(args)?;
                let (width, height) = formats::animation_dimensions(d, scale)?;
                let filtered = filter_export_layers(d, args)?;
                if kind == "mp4" {
                    formats::export_mp4_progress(temporary, &filtered, scale, &mut progress)?;
                } else {
                    formats::export_gif_progress(temporary, &filtered, scale, &mut progress)?;
                }
                progress(d.durations.len(), d.durations.len())?;
                let (width, height) = if kind == "mp4" {
                    (width.next_multiple_of(2), height.next_multiple_of(2))
                } else {
                    (width, height)
                };
                Ok(
                    json!({"revision":revision,"format":kind,"width":width,"height":height,"scale":scale,"frames":d.durations.len()}),
                )
            }
            _ => Err(
                "Choose a supported raster, .gif or .mp4 path, or set format to png-sequence or tileset-atlas"
                    .into(),
            ),
        }
    })
}

impl PreparedExport {
    fn check_staging(&self) -> Result<(), String> {
        if let Some(identity) = self.staging_identity
            && mcp_io::directory_identity(&self.staging).map_err(|error| error.to_string())?
                != identity
        {
            return Err("The restricted export's staging directory changed".into());
        }
        if let Some(identity) = self.temporary_directory_identity
            && mcp_io::directory_identity(&self.temporary).map_err(|error| error.to_string())?
                != identity
        {
            return Err("The restricted export's temporary directory changed".into());
        }
        Ok(())
    }

    pub fn write(self) -> Result<Value, String> {
        if let Some(access) = self.access.clone() {
            access.run(|| self.write_inner())
        } else if self.project_root.is_some() {
            Err("Restricted MCP filesystem access was not established".into())
        } else {
            self.write_inner()
        }
    }

    fn write_inner(mut self) -> Result<Value, String> {
        let guard = mcp_files::Files {
            root: self.project_root.as_deref(),
            overwrite: self.replace,
        };
        guard.check_write(&self.output)?;
        guard.check_write(&self.staging)?;
        self.check_staging()?;
        if !self.sequence.is_empty() && self.replace {
            let files = check_sequence_outputs(&self.output, &self.sequence, &guard)?;
            mcp_io::create_dir_all(&self.output).map_err(|e| e.to_string())?;
            mcp_files::publish_files(&self, &files, true, true)?;
        } else if !self.sequence.is_empty() {
            mcp_io::create_dir(&self.output).map_err(|e| e.to_string())?;
            for (index, name) in self.sequence.iter().enumerate() {
                if let Err(error) =
                    mcp_io::publish_new(&self.temporary.join(name), &self.output.join(name))
                {
                    for written in &self.sequence[..index] {
                        let _ = mcp_io::remove_file(&self.output.join(written));
                    }
                    let _ = mcp_io::remove_dir(&self.output);
                    return Err(error.to_string());
                }
            }
        } else {
            if self.project_root.is_some() {
                guard.check_file(&self.output)?;
            }
            if self.replace {
                mcp_io::rename(&self.temporary, &self.output)
            } else {
                mcp_io::publish_new(&self.temporary, &self.output)
            }
            .map_err(|e| e.to_string())?;
        }
        self.metadata["output"] = json!(self.output);
        Ok(std::mem::take(&mut self.metadata))
    }
}

pub fn changed_cel_bounds(before: &Document, after: &Document) -> Value {
    let mut changes = Vec::new();
    for (layer, image_layer) in after.layers.iter().enumerate() {
        for (frame, pixels) in image_layer.cels.iter().enumerate() {
            let old = before
                .layers
                .get(layer)
                .and_then(|l| l.cels.get(frame))
                .filter(|_| before.width == after.width && before.height == after.height);
            if old.is_some_and(|old| old.shares_storage(pixels)) {
                continue;
            }
            let (mut x, mut y, mut right, mut bottom) = (after.width, after.height, 0, 0);
            let mut include = |i| {
                let px = i % after.width;
                let py = i / after.width;
                x = x.min(px);
                y = y.min(py);
                right = right.max(px + 1);
                bottom = bottom.max(py + 1);
            };
            pixels.visit_nonclear(|i, pixel| {
                if pixel != old.map_or(CLEAR, |old| old[i]) {
                    include(i);
                }
            });
            if let Some(old) = old {
                old.visit_nonclear(|i, _| {
                    if pixels[i] == CLEAR {
                        include(i);
                    }
                });
            }
            if right > x && bottom > y {
                changes.push(json!({"layer":layer,"frame":frame,"x":x,"y":y,"width":right-x,"height":bottom-y}));
            }
        }
    }
    json!(changes)
}

fn filter_export_layers<'a>(
    d: &'a Document,
    args: &Value,
) -> Result<std::borrow::Cow<'a, Document>, String> {
    let layers = export_layer_indices(args)?;
    workflows::filter_layers_for_export(d, layers.as_deref())
}

fn export_layer_indices(args: &Value) -> Result<Option<Vec<usize>>, String> {
    args.get("layers")
        .map(|indices| {
            serde_json::from_value(indices.clone()).map_err(|_| "Invalid layer list".into())
        })
        .transpose()
}

fn preview_image(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    limit: usize,
) -> Result<(image::RgbaImage, Value), String> {
    let sprite = filter_export_layers(d, args)?;
    preview_composite(&sprite, f, args, revision, limit)
}

struct RenderGeometry {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    scale: usize,
}

fn preview_geometry(
    sprite: &Document,
    f: usize,
    args: &Value,
    limit: usize,
) -> Result<RenderGeometry, String> {
    if f >= sprite.durations.len() {
        return Err("Invalid frame".into());
    }
    let (x, y, w, h) = if let Some(region) = args.get("region") {
        (
            number(region, "x")?,
            number(region, "y")?,
            number(region, "width")?,
            number(region, "height")?,
        )
    } else {
        (0, 0, sprite.width, sprite.height)
    };
    if w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|v| v > sprite.width)
        || y.checked_add(h).is_none_or(|v| v > sprite.height)
    {
        return Err("Render region must be inside the canvas".into());
    }
    let scale = if args.get("scale").is_some() {
        number(args, "scale")?
    } else {
        1
    };
    if scale == 0
        || w.saturating_mul(h)
            .saturating_mul(scale)
            .saturating_mul(scale)
            > limit
    {
        return Err(format!(
            "Render scale must be positive and output must fit {limit} pixels"
        ));
    }
    Ok(RenderGeometry {
        x,
        y,
        width: w,
        height: h,
        scale,
    })
}

fn preview_composite(
    sprite: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    limit: usize,
) -> Result<(image::RgbaImage, Value), String> {
    let RenderGeometry {
        x,
        y,
        width: w,
        height: h,
        scale,
    } = preview_geometry(sprite, f, args, limit)?;
    let pixels = crate::color::display_pixels(sprite, f).unwrap_or_else(|_| sprite.composite(f));
    let cropped = (y..y + h)
        .flat_map(|row| {
            pixels[row * sprite.width + x..row * sprite.width + x + w]
                .iter()
                .flatten()
                .copied()
        })
        .collect();
    let image = image::RgbaImage::from_raw(w as u32, h as u32, cropped).ok_or("Invalid image")?;
    let image = crate::workflows::scale_image(&image, scale)?;
    let layers: Vec<_> = sprite
        .layers
        .iter()
        .enumerate()
        .filter_map(|(i, _)| {
            let mut parent = Some(i);
            while let Some(j) = parent {
                if !sprite.layers[j].visible {
                    return None;
                }
                parent = sprite.layers[j].parent;
            }
            Some(i)
        })
        .collect();
    let metadata = json!({"revision":revision,"frame":f,"layers":layers,"scale":scale,
        "canvas_width":sprite.width,"canvas_height":sprite.height,"width":image.width(),"height":image.height(),
        "region":{"x":x,"y":y,"width":w,"height":h}});
    Ok((image, metadata))
}

#[cfg(test)]
mod deadline_regressions {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    struct Trickle;
    impl Read for Trickle {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_millis(5));
            out[0] = b' ';
            Ok(1)
        }
    }
    #[test]
    fn regression_bridge_read_deadline_is_not_reset_by_progress() {
        let start = Instant::now();
        let result = read_live_line(
            &mut Trickle,
            start + Duration::from_millis(25),
            &AtomicBool::new(false),
        );
        assert!(result.unwrap_err().contains("deadline"));
        assert!(start.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn regression_bridge_revocation_interrupts_active_read_and_response_wait() {
        let stop = AtomicBool::new(true);
        let mut input = b"{}\n".as_slice();
        assert!(
            read_live_line(&mut input, Instant::now() + Duration::from_secs(1), &stop)
                .unwrap_err()
                .contains("disabled")
        );
        assert_eq!(input, b"{}\n");
        struct RevokedDuringRead<'a>(&'a AtomicBool);
        impl Read for RevokedDuringRead<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                out[0] = b'{';
                self.0.store(true, Ordering::Relaxed);
                Ok(1)
            }
        }
        let active_stop = AtomicBool::new(false);
        assert!(
            read_live_line(
                &mut RevokedDuringRead(&active_stop),
                Instant::now() + Duration::from_secs(1),
                &active_stop,
            )
            .unwrap_err()
            .contains("disabled")
        );
        let (_tx, rx) = mpsc::channel();
        assert!(
            wait_live_response(&rx, Instant::now() + Duration::from_secs(1), &stop)
                .unwrap_err()
                .contains("disabled")
        );
    }
    #[test]
    fn regression_bridge_response_wait_uses_original_request_deadline() {
        let (_tx, rx) = mpsc::channel();
        let start = Instant::now();
        assert!(
            wait_live_response(
                &rx,
                start + Duration::from_millis(25),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .contains("deadline")
        );
        assert!(start.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn regression_bridge_write_deadline_is_not_reset_by_progress() {
        struct SlowWriter;
        impl Write for SlowWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                std::thread::sleep(Duration::from_millis(5));
                Ok(1)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let start = Instant::now();
        let result = write_live_bytes(
            &mut SlowWriter,
            &[b' '; 100],
            start + Duration::from_millis(25),
            &AtomicBool::new(false),
        );
        assert!(result.unwrap_err().contains("deadline"));
        assert!(start.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn regression_bridge_line_reader_accepts_newline_and_limits_input() {
        let stop = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            read_live_line(&mut b"{}\n".as_slice(), deadline, &stop).unwrap(),
            "{}\n"
        );
        assert!(
            read_live_line(&mut std::io::repeat(b' '), deadline, &stop)
                .unwrap_err()
                .contains("large")
        );
        assert!(!stop.load(Ordering::Relaxed));
    }
}
