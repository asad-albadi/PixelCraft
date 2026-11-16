use super::{export_scale, extension, mcp_files::Files, number, path};
use crate::{
    formats, lua,
    model::{Document, MAX_DIMENSION},
};
use serde_json::{Value, json};
use std::{fs::File, io::Read};

const MAX_SCRIPT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PARAMS_BYTES: usize = 64 * 1024;

pub(super) fn run(args: &Value) -> Result<Value, String> {
    if !args.is_object() {
        return Err("Expected a JSON argument object".into());
    }
    let script = path(args, "script")?;
    let input = args.get("input").map(|_| path(args, "input")).transpose()?;
    let output = args
        .get("output")
        .map(|_| path(args, "output"))
        .transpose()?;
    let files = Files {
        root: None,
        overwrite: args
            .get("overwrite")
            .map(|value| value.as_bool().ok_or("overwrite must be a boolean"))
            .transpose()?
            .unwrap_or(false),
    };
    if let Some(output) = &output {
        if !matches!(extension(output), "pixel" | "ase" | "aseprite" | "png") {
            return Err("Lua output must be .pixel, .ase, .aseprite, or .png".into());
        }
        files.check_file(output)?;
    }
    let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() || params.to_string().len() > MAX_PARAMS_BYTES {
        return Err("Lua params must be a JSON object of at most 64 KiB".into());
    }
    let frame = args
        .get("frame")
        .map(|_| number(args, "frame"))
        .transpose()?
        .unwrap_or(0);
    let layer = args
        .get("layer")
        .map(|_| number(args, "layer"))
        .transpose()?
        .unwrap_or(0);
    let scale = export_scale(args)?;
    let mut bytes = Vec::new();
    File::open(&script)
        .and_then(|file| file.take(MAX_SCRIPT_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|err| format!("Could not read {}: {err}", script.display()))?;
    if bytes.len() as u64 > MAX_SCRIPT_BYTES {
        return Err("Lua script exceeds 2 MiB".into());
    }
    let source =
        std::str::from_utf8(&bytes).map_err(|err| format!("Lua script must be UTF-8: {err}"))?;
    let doc = if let Some(input) = &input {
        formats::load(input)?
    } else {
        let width = args
            .get("width")
            .map(|_| number(args, "width"))
            .transpose()?
            .unwrap_or(16);
        let height = args
            .get("height")
            .map(|_| number(args, "height"))
            .transpose()?
            .unwrap_or(16);
        if !(1..=MAX_DIMENSION).contains(&width)
            || !(1..=MAX_DIMENSION).contains(&height)
            || width.saturating_mul(height) > lua::MAX_SCRIPT_PIXELS
        {
            return Err("Invalid Lua canvas dimensions".into());
        }
        Document::new(width, height)
    };
    let options =
        json!({"params":params,"filename":input.as_deref().and_then(|p| p.to_str()).unwrap_or("")});
    let (result, stdout) = lua::execute_headless(&doc, source, (frame, layer), options)?;
    let edited = crate::extensions::decode_script_document(&result)?;
    if let Some(output) = &output {
        files.write_file(output, |staged| {
            if extension(staged) == "png" {
                formats::export_raster_scaled(staged, &edited, frame, false, scale)
            } else {
                formats::save(staged, &edited)
            }
        })?;
    }
    Ok(json!({"stdout":stdout,"output":output}))
}
