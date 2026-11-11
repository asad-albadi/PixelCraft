//! Headless commands and a newline-delimited JSON-RPC MCP stdio server.
use crate::{atlas::*, formats, interchange, model::*, productivity::*, workflows::SheetOptions};
use serde_json::{Value, json};
use std::{
    hash::{Hash, Hasher},
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc,
};
pub fn document_revision(d: &Document) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(d).unwrap_or_default().hash(&mut h);
    format!("{:016x}", h.finish())
}
pub fn inspect(d: &Document) -> Value {
    json!({"revision":document_revision(d),"width":d.width,"height":d.height,"frames":d.durations,"layers":d.layers.iter().map(|l|json!({"name":l.name,"locked":l.locked,"group":l.group})).collect::<Vec<_>>(),"tags":d.tags,"palette":d.palette,"references":d.references.iter().map(|r|json!({"name":r.name,"x":r.x,"y":r.y,"scale":r.scale,"opacity":r.opacity})).collect::<Vec<_>>()})
}
pub fn apply_edits(d: &Document, v: &Value) -> Result<Document, String> {
    if v["revision"].as_str() != Some(&document_revision(d)) {
        return Err("Stale revision; inspect the document again".into());
    }
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
        "atlas" => {
            let paths = v["inputs"].as_array().ok_or("Expected inputs array")?;
            let mut docs = vec![];
            for p in paths {
                let p = PathBuf::from(p.as_str().ok_or("Input must be a path")?);
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
                "inspect" => Ok(inspect(&d)),
                "render" | "export" | "import" => {
                    if op == "render" && v.get("output").is_none() {
                        let f = v["frame"].as_u64().unwrap_or(0) as usize;
                        return render_document(&d, f);
                    }
                    let out = path(v, "output")?;
                    let kind = v["format"].as_str().unwrap_or_else(|| extension(&out));
                    match kind {
                        "gif" => formats::export_gif(&out, &d)?,
                        "sheet" => interchange::export_sheet(
                            &out,
                            &d,
                            &options::<SheetOptions>(&v["options"])?,
                        )?,
                        "pixel" | "ase" | "aseprite" => formats::save(&out, &d)?,
                        "png" => {
                            let f = v["frame"].as_u64().unwrap_or(0) as usize;
                            if f >= d.durations.len() {
                                return Err("Invalid frame".into());
                            }
                            formats::export_png(&out, &d, f, false)?;
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
                "edit" => {
                    let d = apply_edits(&d, v)?;
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
pub const CLI_HELP: &str = "PixelCraft headless tools\n  pixelcraft --cli OPERATION '{JSON arguments}'\n  pixelcraft --cli OPERATION --input FILE --output FILE [--options '{JSON}']\n  pixelcraft --mcp [--live BRIDGE_CONFIG]\nOperations: inspect, render, edit, export, import, import-sequence, import-grid, batch-tags, atlas, palette-import, palette-export, palette-convert\nPaths: input, output; atlas and import-sequence use inputs (JSON array), palette-import uses palette.\nExport: format=png|gif|sheet|pixel|aseprite; frame is zero-based.\nGrid options: width,height,offset_x,offset_y,spacing_x,spacing_y,layers.\nSheet options: columns,padding,trim,tag,preset (Generic/Godot/Unity).\nAtlas options: layout (Rows/MaxRects),width,padding,trim,deduplicate,power_of_two.\nEdits: revision from inspect, edits:[{layer,frame,x,y,color:[r,g,b,a]}].\nMCP exposes these operations and live-inspect, live-render, live-edit, live-undo. Live access must first be enabled in the editor's Workflow tools.\n";
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
            v[key] = if ["inputs", "options", "edits", "frame"].contains(&key) {
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
    if r.get("id").is_none() {
        return None;
    }
    let method = r["method"].as_str().unwrap_or("");
    let result: Result<Value, String> = match method {
        "initialize" => Ok(
            json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"PixelCraft","version":env!("CARGO_PKG_VERSION")}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let mut names = OPERATIONS.to_vec();
            if live.is_some() {
                names.extend(["live-inspect", "live-render", "live-edit", "live-undo"]);
            }
            Ok(
                json!({"tools":names.iter().map(|name|json!({"name":name,"description":format!("PixelCraft {name}. File operations use input/output paths; edit requires revision from inspect. {}",CLI_HELP),"inputSchema":{"type":"object","properties":{"input":{"type":"string"},"output":{"type":"string"},"revision":{"type":"string"},"edits":{"type":"array","items":{"type":"object"}},"options":{"type":"object"},"inputs":{"type":"array","items":{"type":"string"}},"format":{"type":"string"},"frame":{"type":"integer"},"palette":{"type":"string"}},"additionalProperties":false}})).collect::<Vec<_>>()}),
            )
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
                Ok(v) if v["mimeType"] == "image/png" => {
                    json!({"content":[{"type":"image","mimeType":"image/png","data":v["data"]},{"type":"text","text":json!({"revision":v["revision"]}).to_string()}]})
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
                        let deadline = started + std::time::Duration::from_secs(10);
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
                            check_live_request(deadline, &stopping)?;
                            let (res_tx, res_rx) = mpsc::channel();
                            tx.send(LiveRequest {
                                deadline,
                                operation: v["operation"].as_str().unwrap_or("").into(),
                                arguments: v["arguments"].clone(),
                                response: res_tx,
                            })
                            .map_err(|e| e.to_string())?;
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
        .set_read_timeout(Some(std::time::Duration::from_secs(12)))
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

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn render_document(d: &Document, f: usize) -> Result<Value, String> {
    if f >= d.durations.len() {
        return Err("Invalid frame".into());
    }
    let im = image::RgbaImage::from_raw(
        d.width as u32,
        d.height as u32,
        d.composite(f).into_iter().flatten().collect(),
    )
    .ok_or("Invalid image")?;
    let mut png = std::io::Cursor::new(vec![]);
    im.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(
        json!({"revision":document_revision(d),"mimeType":"image/png","data":base64(&png.into_inner())}),
    )
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
