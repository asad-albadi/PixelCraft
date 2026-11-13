//! Headless commands and a newline-delimited JSON-RPC MCP stdio server.
use crate::{atlas::*, formats, interchange, model::*, productivity::*, workflows::SheetOptions};
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
            .entry(std::sync::Arc::as_ptr(&slot.0))
            .or_insert_with(|| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                slot.len().hash(&mut h);
                h.write(slot.as_slice().as_flattened());
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
    inspect_revision(d, &document_revision(d))
}
pub fn inspect_revision(d: &Document, revision: &str) -> Value {
    json!({"revision":revision,"width":d.width,"height":d.height,"frames":d.durations,"layers":d.layers.iter().enumerate().map(|(i,l)|json!({"index":i,"name":l.name,"locked":l.locked,"group":l.group,"visible":l.visible,"parent":l.parent})).collect::<Vec<_>>(),"tags":d.tags,"palette":d.palette,"references":d.references.iter().map(|r|json!({"name":r.name,"x":r.x,"y":r.y,"scale":r.scale,"opacity":r.opacity})).collect::<Vec<_>>()})
}
pub fn apply_edits(d: &Document, v: &Value) -> Result<Document, String> {
    if v["revision"].as_str() != Some(&document_revision(d)) {
        return Err("Stale revision; inspect the document again".into());
    }
    apply_pixel_batch(d, v)
}
pub fn apply_pixel_batch(d: &Document, v: &Value) -> Result<Document, String> {
    let edits = v["edits"].as_array().ok_or("Expected edits array")?;
    if edits.len() > 1_000_000 {
        return Err("Too many edits".into());
    }
    let mut out = d.clone();
    for e in edits {
        let l = number(e, "layer")?;
        let f = number(e, "frame")?;
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
    let commands = v["commands"].as_array().ok_or("Expected commands array")?;
    if commands.len() > 1024 {
        return Err("Too many commands".into());
    }
    let mut out = d.clone();
    let mut paint_budget = 4_000_000;
    for c in commands {
        if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            return Err("Live request expired; no changes applied".into());
        }
        let layer = || {
            let l = number(c, "layer")?;
            if l >= out.layers.len() {
                Err("Invalid layer".to_string())
            } else {
                Ok(l)
            }
        };
        let frame = || {
            let f = number(c, "frame")?;
            if f >= out.durations.len() {
                Err("Invalid frame".to_string())
            } else {
                Ok(f)
            }
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
                if let Some(a) = c.get("opacity") {
                    target.opacity =
                        serde_json::from_value(a.clone()).map_err(|_| "Invalid opacity")?;
                }
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
            "set-frame" => {
                let f = frame()?;
                let duration = number(c, "duration")?;
                if !(1..=65535).contains(&duration) {
                    return Err("Duration must be 1–65535 ms".into());
                }
                out.durations[f] = duration as u16;
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
fn options<T: serde::de::DeserializeOwned + Default>(v: &Value) -> Result<T, String> {
    if v.is_null() {
        Ok(T::default())
    } else {
        serde_json::from_value(v.clone()).map_err(|e| e.to_string())
    }
}
pub fn operation(op: &str, v: &Value) -> Result<Value, String> {
    match op {
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
            let output = path(v, "output")?;
            if output.exists() {
                return Err("Output already exists".into());
            }
            formats::save(&output, &d)?;
            Ok(inspect(&d))
        }
        "atlas" => {
            let paths = v["inputs"].as_array().ok_or("Expected inputs array")?;
            let mut docs = vec![];
            let paths = paths
                .iter()
                .map(|p| p.as_str().map(PathBuf::from).ok_or("Input must be a path"))
                .collect::<Result<Vec<_>, _>>()?;
            for p in expand_atlas_inputs(&paths)? {
                docs.push((
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    formats::load(&p)?,
                ));
            }
            let out = path(v, "output")?;
            export_atlas(&out, &docs, &options::<AtlasOptions>(&v["options"])?)?;
            Ok(json!({"output":out}))
        }
        "import-sequence" => {
            let paths = v["inputs"]
                .as_array()
                .ok_or("Expected inputs array")?
                .iter()
                .map(|p| p.as_str().map(PathBuf::from).ok_or("Input must be a path"))
                .collect::<Result<Vec<_>, _>>()?;
            let d = interchange::load_sequence(&paths)?;
            formats::save(&path(v, "output")?, &d)?;
            Ok(inspect(&d))
        }
        "import-grid" => {
            let im = read_image(&path(v, "input")?)?;
            let d = import_grid(&im, &options::<GridImport>(&v["options"])?)?;
            let out = path(v, "output")?;
            formats::save(&out, &d)?;
            Ok(inspect(&d))
        }
        "palette-convert" => {
            let input = path(v, "input")?;
            let output = path(v, "output")?;
            let p = parse_palette(
                &std::fs::read(&input).map_err(|e| e.to_string())?,
                extension(&input),
            )?;
            std::fs::write(&output, write_palette(&p, extension(&output))?)
                .map_err(|e| e.to_string())?;
            Ok(json!({"colors":p.len(),"output":output}))
        }
        _ => {
            let input = path(v, "input")?;
            let mut d = formats::load(&input)?;
            match op {
                "save" | "save-as" => {
                    let revision = document_revision(&d);
                    if v["revision"] != revision {
                        return Err("Stale revision; inspect the document again".into());
                    }
                    let output = if op == "save" {
                        input
                    } else {
                        path(v, "output")?
                    };
                    prepare_live_save(&d, output, &revision, op == "save")?.write()
                }
                "inspect" => Ok(inspect(&d)),
                "render" | "export" | "import" => {
                    if op == "render" && v.get("output").is_none() {
                        let f = if v.get("frame").is_some() {
                            number(v, "frame")?
                        } else {
                            0
                        };
                        return render_preview(&d, f, v, &document_revision(&d), MAX_PIXELS);
                    }
                    let out = path(v, "output")?;
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
                        let (image, mut metadata) =
                            preview_image(&d, frame, v, &document_revision(&d), MAX_PIXELS)?;
                        image
                            .save_with_format(&out, image::ImageFormat::Png)
                            .map_err(|e| e.to_string())?;
                        metadata["output"] = json!(out);
                        return Ok(metadata);
                    }
                    let kind = v["format"].as_str().unwrap_or_else(|| extension(&out));
                    match kind {
                        "gif" | "mp4" => write_animation_export(kind, &out, &d, export_scale(v)?)?,
                        "sheet" => interchange::export_sheet(
                            &out,
                            &d,
                            &options::<SheetOptions>(&v["options"])?,
                        )?,
                        "pixel" | "ase" | "aseprite" => formats::save(&out, &d)?,
                        "png" | "bmp" => {
                            let f = v["frame"].as_u64().unwrap_or(0) as usize;
                            if f >= d.durations.len() {
                                return Err("Invalid frame".into());
                            }
                            formats::export_raster_scaled(
                                &out,
                                &d,
                                f,
                                false,
                                v["scale"].as_u64().unwrap_or(1) as usize,
                            )?;
                        }
                        _ => return Err("Unknown export format".into()),
                    }
                    Ok(json!({"output":out}))
                }
                "batch-tags" => {
                    let paths = batch_tags(
                        &path(v, "output")?,
                        &d,
                        v["format"] == "gif",
                        &options::<SheetOptions>(&v["options"])?,
                    )?;
                    Ok(json!({"outputs":paths}))
                }
                "palette-export" => {
                    let out = path(v, "output")?;
                    std::fs::write(&out, write_palette(&d.palette, extension(&out))?)
                        .map_err(|e| e.to_string())?;
                    Ok(json!({"output":out}))
                }
                "palette-import" => {
                    let p = path(v, "palette")?;
                    let palette = parse_palette(
                        &std::fs::read(&p).map_err(|e| e.to_string())?,
                        extension(&p),
                    )?;
                    d.import_palette(palette)?;
                    formats::save(&path(v, "output")?, &d)?;
                    Ok(inspect(&d))
                }
                "edit" | "commands" => {
                    let d = if op == "commands" {
                        apply_commands(&d, v)?
                    } else {
                        apply_edits(&d, v)?
                    };
                    formats::save(&path(v, "output")?, &d)?;
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
pub const CLI_HELP: &str = "PixelCraft headless tools\n  pixelcraft --cli OPERATION '{JSON arguments}'\n  pixelcraft --cli OPERATION --input FILE --output FILE [--options '{JSON}']\n  pixelcraft --mcp [--live BRIDGE_CONFIG]\nOperations: create, commands, inspect, render, edit, save, save-as, export, import, import-sequence, import-grid, batch-tags, atlas, palette-import, palette-export, palette-convert\nPaths: input, output; atlas and import-sequence use inputs (JSON array), palette-import uses palette.\nExport: format=png|bmp|gif|mp4|sheet|pixel|aseprite; scale is a positive integer; frame is zero-based. MP4 requires FFmpeg with libx264 on PATH.\nSave takes input and revision; save-as also takes output.\nGrid options: width,height,offset_x,offset_y,spacing_x,spacing_y,layers.\nSheet options: scale,columns,padding,trim,tag,preset (Generic/Godot/Unity).\nAtlas options: layout (Rows/MaxRects),width,padding,trim,deduplicate,power_of_two.\nEdits: revision from inspect, edits:[{layer,frame,x,y,color:[r,g,b,a]}].\nCreate: width,height,output (must not exist). Commands: revision,commands:[{op,...}]. Ops: add-layer(name), set-layer(layer,name,visible,locked,opacity), move-layer(layer,to), remove-layer(layer), add-frame(frame,duplicate), set-frame(frame,duration ms), remove-frame(frame), add-tag(tag), set-tag(index,tag), remove-tag(index), shape(layer,frame,kind=line|rectangle|ellipse,x,y,x2,y2,color RGBA,filled). Tags: name,from,to,direction (0 forward/1 reverse/2 pingpong/3 reverse pingpong),repeat (0 infinite),color RGB. All indices start at 0. File commands use input/output; live commands require the live-inspect revision and are one Undo step. Live-create takes width,height and a live revision; Undo restores the previous artwork.\nMCP exposes these operations and live-inspect, live-render, live-edit, live-commands, live-create, live-undo, live-redo, live-save, live-save-as and live-export. Live access must first be enabled in the editor's Workflow tools.\n";
pub fn run_cli(args: &[String]) -> Result<Value, String> {
    let op = args.first().ok_or(CLI_HELP)?;
    let mut v = json!({});
    if args.get(1).is_some_and(|s| s.starts_with('{')) {
        if args.len() != 2 {
            return Err("Unexpected extra arguments".into());
        }
        v = serde_json::from_str(&args[1]).map_err(|e| e.to_string())?;
    } else {
        let mut i = 1;
        while i < args.len() {
            let key = args[i]
                .strip_prefix("--")
                .ok_or("Expected --argument value")?;
            let value = args.get(i + 1).ok_or("Missing argument value")?;
            v[key] = if [
                "inputs", "options", "edits", "commands", "frame", "width", "height", "scale",
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
    operation(op, &v)
}
pub fn serve_mcp(live: Option<&Path>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(stdout,"{}",json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}})).map_err(|e|e.to_string())?;
                continue;
            }
        };
        let Some(response) = rpc(&request, live) else {
            continue;
        };
        writeln!(stdout, "{response}").map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn rpc(r: &Value, live: Option<&Path>) -> Option<Value> {
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
                    "live-render",
                    "live-export",
                    "live-save",
                    "live-save-as",
                    "live-edit",
                    "live-commands",
                    "live-create",
                    "live-undo",
                    "live-redo",
                ]);
            }
            Ok(json!({"tools":names.iter().map(|name| {
                    let details = match *name {
                        "live-commands" | "commands" => "Atomic edits with the current revision. Raster ops: blit (layer,frame,x,y,width,height,data,encoding,mode); stroke (layer,frame,points [[x,y],...],color RGBA,size,end_size,spacing,placements,scatter,seed,ramp,stamp,alternate,shade,opacity,mode); polygon (layer,frame,points,color,alternate); clear-region, copy-region, move-region, recolor-region. Raster data is straight RGBA8, row-major, encoding base64 or base64-zlib. See the PixelCraft live painting documentation for full arguments.",
                        "live-render" => "Return a PNG of the active sprite or a crop/layer subset. Optional revision, frame, layers, region, and integer scale.",
                        "live-export" => "Export a revision-matched sprite to a new PNG, GIF or MP4 file. Use integer scale for nearest-neighbor enlargement. Frame, layers and region apply to PNG only.",
                        "live-save" => "Save the revision-matched sprite to its current project path. Use live-save-as for untitled sprites.",
                        "live-save-as" => "Save the revision-matched sprite to a new .pixel or .aseprite path and make it the current project path.",
                        _ => CLI_HELP,
                    };
                    let mut tool = json!({"name":name,"description":format!("PixelCraft {name}. {details}"),
                    "inputSchema":{"type":"object","properties":{"input":{"type":"string"},"output":{"type":"string"},"revision":{"type":"string"},"edits":{"type":"array","items":{"type":"object"}},"options":{"type":"object"},"inputs":{"type":"array","items":{"type":"string"}},"format":{"type":"string"},"frame":{"type":"integer"},"palette":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"commands":{"type":"array","items":{"type":"object"}},"scale":{"type":"integer","minimum":1}},"additionalProperties":false}});
                if matches!(*name, "render" | "live-render" | "live-export") {
                    let props = &mut tool["inputSchema"]["properties"];
                    props["layers"] = json!({"type":"array","items":{"type":"integer","minimum":0}});
                    props["region"] = json!({"type":"object","required":["x","y","width","height"],
                        "properties":{"x":{"type":"integer","minimum":0},"y":{"type":"integer","minimum":0},"width":{"type":"integer","minimum":1},"height":{"type":"integer","minimum":1}},
                        "additionalProperties":false});
                }
                if *name == "save" {
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
            let value = if name.starts_with("live-") {
                live.ok_or_else(|| "Live editor access is disabled".to_string())
                    .and_then(|path| live_call(path, name, args))
            } else {
                operation(name, args)
            };
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

pub struct LiveRequest {
    pub deadline: std::time::Instant,
    pub operation: String,
    pub arguments: Value,
    pub response: mpsc::Sender<Result<Value, String>>,
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
            json!({"address":addr.to_string(),"token":token})
        )
        .map_err(|e| e.to_string())?;
        let (tx, requests) = mpsc::channel();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopping.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
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
                            if long_file_operation(v["operation"].as_str().unwrap_or("")) {
                                deadline = started + std::time::Duration::from_secs(300);
                            }
                            check_live_request(deadline, &stopping)?;
                            let (res_tx, res_rx) = mpsc::channel();
                            tx.send(LiveRequest {
                                deadline,
                                operation: v["operation"].as_str().unwrap_or("").into(),
                                arguments: v["arguments"].clone(),
                                response: res_tx,
                            })
                            .map_err(|e| e.to_string())?;
                            wake();
                            wait_live_response(&res_rx, deadline, &stopping)
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
                                    bytes.push(b'\n');
                                    let _ =
                                        write_live_bytes(&mut stream, &bytes, deadline, &stopping);
                                }
                            }
                        }
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
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
fn wait_live_response(
    receiver: &mpsc::Receiver<Result<Value, String>>,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Value, String> {
    loop {
        check_live_request(deadline, stop)?;
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
fn live_call(config: &Path, op: &str, args: &Value) -> Result<Value, String> {
    let v: Value = serde_json::from_slice(&std::fs::read(config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
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
        json!({"token":v["token"],"operation":op,"arguments":args})
    )
    .map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::BufReader::new(stream)
        .take(8_000_000)
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
    temporary: PathBuf,
    metadata: Value,
    replace: bool,
}

impl Drop for PreparedExport {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
        let _ = std::fs::remove_dir(&self.staging);
    }
}

fn prepare_output(
    output: PathBuf,
    replace: bool,
    encode: impl FnOnce(&Path) -> Result<Value, String>,
) -> Result<PreparedExport, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if !replace && output.try_exists().map_err(|e| e.to_string())? {
        return Err("Output already exists. Choose a new path.".into());
    }
    let parent = output.parent().ok_or("Invalid output path")?;
    let filename = output.file_name().ok_or("Invalid output filename")?;
    let staging = loop {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = parent.join(format!(".pixelcraft-output-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => break dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let temporary = staging.join(filename);
    let mut prepared = PreparedExport {
        output,
        staging,
        temporary,
        metadata: Value::Null,
        replace,
    };
    prepared.metadata = encode(&prepared.temporary)?;
    Ok(prepared)
}

pub fn prepare_live_save(
    d: &Document,
    output: PathBuf,
    revision: &str,
    replace: bool,
) -> Result<PreparedExport, String> {
    if !matches!(extension(&output), "pixel" | "ase" | "aseprite") {
        return Err("Choose a .pixel, .ase or .aseprite project path".into());
    }
    prepare_output(output, replace, |temporary| {
        formats::save(temporary, d)?;
        Ok(inspect_revision(d, revision))
    })
}

pub fn prepare_live_export(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
) -> Result<PreparedExport, String> {
    let output = path(args, "output")?;
    let kind = extension(&output).to_ascii_lowercase();
    if args.get("format").is_some_and(|v| v != kind.as_str()) {
        return Err("Export format must match the output extension".into());
    }
    prepare_output(output, false, |temporary| match kind.as_str() {
        "png" => {
            let (image, metadata) = preview_image(d, f, args, revision, MAX_PIXELS)?;
            image
                .save_with_format(temporary, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            Ok(metadata)
        }
        "gif" | "mp4" => {
            if ["frame", "layers", "region"]
                .iter()
                .any(|key| args.get(key).is_some())
            {
                return Err("Animation exports use all frames and visible layers".into());
            }
            let scale = export_scale(args)?;
            let (width, height) = formats::animation_dimensions(d, scale)?;
            export_animation(&kind, temporary, d, scale)?;
            let (width, height) = if kind == "mp4" {
                (width.next_multiple_of(2), height.next_multiple_of(2))
            } else {
                (width, height)
            };
            Ok(
                json!({"revision":revision,"format":kind,"width":width,"height":height,"scale":scale,"frames":d.durations.len()}),
            )
        }
        _ => Err("Choose a .png, .gif or .mp4 output path".into()),
    })
}

impl PreparedExport {
    pub fn write(mut self) -> Result<Value, String> {
        if self.replace {
            std::fs::rename(&self.temporary, &self.output)
        } else {
            std::fs::hard_link(&self.temporary, &self.output)
        }
        .map_err(|e| e.to_string())?;
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
            for (i, pixel) in pixels.iter().enumerate() {
                if *pixel == old.map_or(CLEAR, |old| old[i]) {
                    continue;
                }
                let px = i % after.width;
                let py = i / after.width;
                x = x.min(px);
                y = y.min(py);
                right = right.max(px + 1);
                bottom = bottom.max(py + 1);
            }
            if right > x && bottom > y {
                changes.push(json!({"layer":layer,"frame":frame,"x":x,"y":y,"width":right-x,"height":bottom-y}));
            }
        }
    }
    json!(changes)
}

fn preview_image(
    d: &Document,
    f: usize,
    args: &Value,
    revision: &str,
    limit: usize,
) -> Result<(image::RgbaImage, Value), String> {
    if f >= d.durations.len() {
        return Err("Invalid frame".into());
    }
    let mut filtered;
    let sprite = if let Some(indices) = args.get("layers") {
        let indices: Vec<usize> =
            serde_json::from_value(indices.clone()).map_err(|_| "Invalid layer list")?;
        if indices.is_empty()
            || indices.len() > d.layers.len()
            || indices.iter().any(|&i| i >= d.layers.len())
        {
            return Err("Choose at least one valid layer".into());
        }
        let selected: std::collections::HashSet<_> = indices.into_iter().collect();
        filtered = d.clone();
        for layer in &mut filtered.layers {
            layer.visible = false;
        }
        for index in 0..d.layers.len() {
            let mut ancestor = Some(index);
            while let Some(i) = ancestor {
                if selected.contains(&i) {
                    let mut parent = Some(index);
                    while let Some(j) = parent {
                        filtered.layers[j].visible = true;
                        parent = d.layers[j].parent;
                    }
                    break;
                }
                if !d.layers[i].visible {
                    break;
                }
                ancestor = d.layers[i].parent;
            }
        }
        &filtered
    } else {
        d
    };
    let (x, y, w, h) = if let Some(region) = args.get("region") {
        (
            number(region, "x")?,
            number(region, "y")?,
            number(region, "width")?,
            number(region, "height")?,
        )
    } else {
        (0, 0, d.width, d.height)
    };
    if w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|v| v > d.width)
        || y.checked_add(h).is_none_or(|v| v > d.height)
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
    let pixels = crate::color::display_pixels(sprite, f).unwrap_or_else(|_| sprite.composite(f));
    let cropped = (y..y + h)
        .flat_map(|row| {
            pixels[row * d.width + x..row * d.width + x + w]
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
        "canvas_width":d.width,"canvas_height":d.height,"width":image.width(),"height":image.height(),
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
