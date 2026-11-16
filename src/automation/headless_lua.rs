use super::{export_scale, extension, mcp_files::Files, number, path};
use crate::{
    formats, lua,
    model::{Document, MAX_DIMENSION},
};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, BufRead, IsTerminal, Read, Write},
    path::PathBuf,
};

const MAX_SCRIPT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PARAMS_BYTES: usize = 64 * 1024;

pub(super) fn run(args: &Value) -> Result<Value, String> {
    let session = Session::new(args)?;
    let script = path(args, "script")?;
    let mut bytes = Vec::new();
    File::open(&script)
        .and_then(|file| file.take(MAX_SCRIPT_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|err| format!("Could not read {}: {err}", script.display()))?;
    if bytes.len() as u64 > MAX_SCRIPT_BYTES {
        return Err("Lua script exceeds 2 MiB".into());
    }
    let source =
        std::str::from_utf8(&bytes).map_err(|err| format!("Lua script must be UTF-8: {err}"))?;
    let (result, stdout) = lua::execute_headless(
        &session.doc,
        source,
        session.context,
        session.options.clone(),
    )?;
    session.save(&result)?;
    Ok(json!({"stdout":stdout,"output":session.output}))
}

const CONSOLE_HELP: &str = "PixelCraft Lua console. :save writes the output and exits. :quit discards changes.      :cancel clears pending input. Prefix an expression with = to print its value.";

pub(super) fn console(args: &Value) -> Result<(), String> {
    let session = Session::new(args)?;
    let lua = lua::Console::new(&session.doc, session.context, session.options.clone())?;
    let input = io::stdin();
    let output = io::stdout();
    let interactive = input.is_terminal() && output.is_terminal();
    let mut input = input.lock();
    let mut output = output.lock();
    if interactive {
        writeln!(output, "{CONSOLE_HELP}").map_err(io_error)?;
    }

    let mut source = String::new();
    loop {
        if interactive {
            let prompt = if source.is_empty() { "> " } else { ">> " };
            write!(output, "{prompt}").map_err(io_error)?;
            output.flush().map_err(io_error)?;
        }
        let mut line = String::new();
        let remaining = MAX_SCRIPT_BYTES - source.len() as u64;
        if (&mut input)
            .take(remaining + 1)
            .read_line(&mut line)
            .map_err(io_error)?
            == 0
        {
            return if source.is_empty() {
                Ok(())
            } else {
                Err("Incomplete Lua command at end of input".into())
            };
        }
        if line.len() as u64 > remaining {
            return Err("Lua command exceeds 2 MiB".into());
        }
        let command = line.trim();
        match command {
            ":quit" => return Ok(()),
            ":cancel" => {
                source.clear();
                continue;
            }
            ":help" => {
                writeln!(output, "{CONSOLE_HELP}").map_err(io_error)?;
                continue;
            }
            ":save" if source.is_empty() => {
                let result = if session.output.is_none() {
                    Err("Provide --output to save console changes".into())
                } else {
                    lua.snapshot().and_then(|result| session.save(&result))
                };
                match result {
                    Err(error) if interactive => {
                        eprintln!("{error}");
                        continue;
                    }
                    result => return result,
                }
            }
            _ => {}
        }
        if source.is_empty() && command.is_empty() {
            continue;
        }
        source.push_str(&line);
        let expression = source.trim_start().strip_prefix('=');
        let expression = expression.map(|expr| format!("print({expr}\n)"));
        match lua.eval(expression.as_deref().unwrap_or(&source))? {
            lua::ConsoleChunk::Incomplete => {}
            lua::ConsoleChunk::Complete {
                output: printed,
                error,
            } => {
                if !printed.is_empty() {
                    output.write_all(printed.as_bytes()).map_err(io_error)?;
                    output.flush().map_err(io_error)?;
                }
                if let Some(error) = error {
                    eprintln!("{error}");
                }
                source.clear();
            }
        }
    }
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

struct Session {
    doc: Document,
    context: (usize, usize),
    options: Value,
    output: Option<PathBuf>,
    files: Files<'static>,
    scale: usize,
}

impl Session {
    fn new(args: &Value) -> Result<Self, String> {
        if !args.is_object() {
            return Err("Expected a JSON argument object".into());
        }
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
        let options = json!({"params":params,"filename":input.as_deref().and_then(|p| p.to_str()).unwrap_or("")});
        Ok(Self {
            doc,
            context: (frame, layer),
            options,
            output,
            files,
            scale,
        })
    }

    fn save(&self, result: &str) -> Result<(), String> {
        let edited = crate::extensions::decode_script_document(result)?;
        if let Some(output) = &self.output {
            self.files.write_file(output, |staged| {
                if extension(staged) == "png" {
                    formats::export_raster_scaled(
                        staged,
                        &edited,
                        self.context.0,
                        false,
                        self.scale,
                    )
                } else {
                    formats::save(staged, &edited)
                }
            })?;
        }
        Ok(())
    }
}
