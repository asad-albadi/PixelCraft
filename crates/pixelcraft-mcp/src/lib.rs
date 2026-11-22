//! JSON-RPC MCP stdio and opt-in live-editor transport.
use pixelcraft_automation::*;
use pixelcraft_core::model::{DEFAULT_DURATION, MAX_DIMENSION};
use pixelcraft_core::storage::{mcp_access, mcp_files};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc,
};
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
                        "live-commands" | "commands" => "Atomic edits with the current revision. Convert to RGB with convert-rgb(optional frame). Convert grayscale with convert-grayscale(method=luma|hsv|hsl,optional frame). Frame selects the shared indexed tile palette and defaults to zero; raster cels use their own frame palettes. Convert the whole sprite with convert-indexed(dithering=none|floyd-steinberg,factor=0..1,fit_criteria=default|rgb|linear-rgb|xyz|lab,transparent,optional palette). Layer ops: add-group(name,parent), add-layer(name,parent), set-layer(layer,parent,blend); parent can be a group index or null for root, or use parent_id. Raster ops: fill(layer,frame,x,y,color,tolerance,contiguous), outline(layer,frame,color,corners), shadow(layer,frame,color,offset), blit, stroke, polygon, clear-region, copy-region, move-region, recolor-region. Set frame name or timing with set-frame(frame,name,duration). See the PixelCraft live painting documentation for full arguments.",
                        "create" | "live-create" => "Create a sprite with width and height. Optional background is four RGBA bytes. File create takes output, live-create takes the current revision.",
                        "live-preview-commands" => "Preview an atomic command batch at the supplied revision without changing the document or undo history.",
                        "batch" => "Run ordered file export steps, or set dry_run to list output paths without writing files.",
                        "live-render" => "Return a PNG of the active sprite or a crop/layer subset. Optional revision, frame, layers, region, and integer scale.",
                        "live-export" => "Export a revision-matched sprite as PNG, BMP, ICO, PCX, QOI, TGA, JPEG, WebP, SVG, CSS, GIF, MP4, FLI, FLC, a PNG sequence directory, or an authored tileset atlas PNG. Use format png-sequence or tileset-atlas for those exports. Layers apply to all formats. Region applies to image and PNG sequence exports. Frame applies to still images.",
                        "live-cel-targets" => "List up to 512 cel IDs for a layer, starting at the requested frame.",
                        "live-export-progress" => "Report the completed and total frames of a running live export by job_id.",
                        "live-cancel-export" => "Request cancellation of a running live export by job_id.",
                        "live-save" => "Save the revision-matched sprite to its current project path. Use live-save-as for untitled sprites.",
                        "live-save-as" => "Save the revision-matched sprite to a .pixel or .aseprite path and make it the current project path.",
                        _ => CLI_HELP,
                    };
                    let mut tool = json!({"name":name,"description":format!("PixelCraft {name}. {details}"),
                    "inputSchema":{"type":"object","properties":{"input":{"type":"string"},"output":{"type":"string"},"revision":{"type":"string"},"document_id":{"type":"string"},"edits":{"type":"array","items":{"type":"object"}},"options":{"type":"object"},"inputs":{"type":"array","items":{"type":"string"}},"format":{"type":"string"},"frame":{"type":"integer"},"palette":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"commands":{"type":"array","items":{"type":"object"}},"scale":{"type":"integer","minimum":1}},"additionalProperties":false}});
                if matches!(*name, "create" | "live-create") {
                    tool["inputSchema"]["properties"]["background"] = json!({
                        "type":"array","minItems":4,"maxItems":4,
                        "items":{"type":"integer","minimum":0,"maximum":255},
                        "description":"Optional starting background color in RGBA order."
                    });
                    tool["inputSchema"]["required"] = if *name == "create" {
                        json!(["width", "height", "output"])
                    } else {
                        json!(["width", "height", "revision"])
                    };
                }
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
                            "order":{"type":"string","enum":["row_major","column_major"],"default":"row_major"},
                            "include_partial":{"type":"boolean","default":false},
                            "columns":{"type":"integer","minimum":1,"maximum":MAX_DIMENSION},
                            "rows":{"type":"integer","minimum":1,"maximum":MAX_DIMENSION},
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

fn long_file_operation(op: &str) -> bool {
    matches!(op, "live-save" | "live-save-as" | "live-export")
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
