use pixelcraft::{automation, formats, model::Document};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-mcp-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("project")).unwrap();
        formats::save(&path.join("project/art.pixel"), &Document::new(2, 2)).unwrap();
        formats::save(&path.join("outside.pixel"), &Document::new(1, 1)).unwrap();
        Self(path)
    }

    fn project(&self) -> PathBuf {
        self.0.join("project")
    }

    fn call(&self, op: &str, args: Value) -> Value {
        let mut server = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
            .arg("--mcp")
            .arg("--project-root")
            .arg(self.project())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(
            server.stdin.take().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":op,"arguments":args}})
        )
        .unwrap();
        let reply = server.wait_with_output().unwrap();
        assert!(
            reply.status.success(),
            "{}",
            String::from_utf8_lossy(&reply.stderr)
        );
        serde_json::from_slice::<Value>(&reply.stdout).unwrap()["result"].clone()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_error(reply: &Value, message: &str) {
    assert_eq!(reply["isError"], true, "{reply}");
    assert!(
        reply["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(message),
        "{reply}"
    );
}

fn file_call(op: &str, args: Value) -> Value {
    automation::rpc(
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":op,"arguments":args}}),
        None,
    )
    .unwrap()["result"]
        .clone()
}

#[test]
fn project_root_resolves_relative_paths_and_checks_each_array_entry() {
    let w = Workspace::new();
    let local = w.call("inspect", json!({"input":"art.pixel"}));
    assert!(local["isError"].is_null(), "{local}");
    let escaped = w.call(
        "atlas",
        json!({"inputs":["art.pixel", "../outside.pixel"],"output":"atlas.png"}),
    );
    assert_error(&escaped, "project root");
    assert!(!w.project().join("atlas.png").exists());
    assert_error(
        &w.call(
            "create",
            json!({"width":1,"height":1,"output":w.0.join("escape.pixel")}),
        ),
        "project root",
    );
    assert!(!w.0.join("escape.pixel").exists());
}

#[test]
#[cfg(unix)]
fn project_root_checks_parent_and_final_symlink_targets() {
    use std::os::unix::fs::symlink;
    let w = Workspace::new();
    symlink(&w.0, w.project().join("outside-dir")).unwrap();
    symlink(w.0.join("outside.pixel"), w.project().join("link.pixel")).unwrap();
    for input in ["link.pixel", "outside-dir/outside.pixel"] {
        assert_error(&w.call("inspect", json!({"input":input})), "project root");
    }
    let outside = w.0.join("outside.png");
    fs::write(&outside, b"outside").unwrap();
    symlink(&outside, w.project().join("image.png")).unwrap();
    assert_error(
        &w.call(
            "export",
            json!({"input":"art.pixel","output":"image.png","overwrite":true}),
        ),
        "project root",
    );
    assert_eq!(fs::read(&outside).unwrap(), b"outside");
    let sidecar = w.0.join("outside.json");
    fs::write(&sidecar, b"outside").unwrap();
    symlink(&sidecar, w.project().join("sheet.json")).unwrap();
    assert_error(
        &w.call(
            "export",
            json!({"input":"art.pixel","output":"sheet.png","format":"sheet","overwrite":true}),
        ),
        "project root",
    );
    assert!(!w.project().join("sheet.png").exists());
    assert_eq!(fs::read(&sidecar).unwrap(), b"outside");
}

#[test]
fn project_root_checks_images_referenced_by_input_documents() {
    let w = Workspace::new();
    image::RgbaImage::new(1, 1)
        .save(w.0.join("outside.png"))
        .unwrap();
    fs::write(
        w.project().join("sheet.json"),
        json!({"meta":{"image":"../outside.png","size":{"w":1,"h":1}},"frames":[]}).to_string(),
    )
    .unwrap();
    assert_error(
        &w.call("inspect", json!({"input":"sheet.json"})),
        "project root",
    );
    fs::write(
        w.project().join("tiles.tsj"),
        json!({"type":"tileset","tilewidth":1,"tileheight":1,"tilecount":1,"columns":1,"image":"../outside.png"}).to_string(),
    ).unwrap();
    assert_error(
        &w.call("inspect", json!({"input":"tiles.tsj"})),
        "project root",
    );
    fs::write(
        w.project().join("map.tmj"),
        json!({
            "orientation":"orthogonal","width":1,"height":1,"tilewidth":1,"tileheight":1,
            "tilesets":[{"firstgid":1,"source":"tiles.tsj"}],"layers":[]
        })
        .to_string(),
    )
    .unwrap();
    assert_error(
        &w.call("inspect", json!({"input":"map.tmj"})),
        "project root",
    );
}

#[test]
fn mcp_preflights_every_sheet_output_and_cli_can_reexport() {
    let w = Workspace::new();
    let input = w.project().join("art.pixel");
    let output = w.project().join("sheet.png");
    let metadata = w.project().join("sheet.json");
    fs::write(&metadata, b"keep").unwrap();
    let mut args = json!({"input":input,"output":output,"format":"sheet"});
    assert_error(&file_call("export", args.clone()), "Output already exists");
    assert!(!output.exists());
    assert_eq!(fs::read(&metadata).unwrap(), b"keep");
    args["overwrite"] = json!(true);
    let exported = file_call("export", args.clone());
    assert!(exported["isError"].is_null(), "{exported}");
    assert!(output.exists());
    assert_ne!(fs::read(&metadata).unwrap(), b"keep");
    args.as_object_mut().unwrap().remove("overwrite");
    assert_error(&file_call("export", args.clone()), "Output already exists");
    automation::operation("export", &args).unwrap();
}

#[test]
fn mcp_batch_preflights_later_tag_outputs() {
    let w = Workspace::new();
    let mut d = Document::new(1, 1);
    for name in ["first", "second"] {
        d.tags.push(pixelcraft::model::FrameTag {
            name: name.into(),
            from: 0,
            to: 0,
            direction: 0,
            repeat: 0,
            color: [0, 0, 0],
        });
    }
    let input = w.project().join("tagged.pixel");
    formats::save(&input, &d).unwrap();
    let dir = w.project().join("tags");
    fs::create_dir(&dir).unwrap();
    let collision = dir.join("002-second.json");
    fs::write(&collision, b"keep").unwrap();
    let args = json!({"input":input,"output":dir});
    assert_error(&file_call("batch-tags", args), "Output already exists");
    assert!(!dir.join("001-first.png").exists());
    assert_eq!(fs::read(collision).unwrap(), b"keep");
}

#[test]
fn mcp_requires_overwrite_and_a_matching_revision_for_file_save() {
    let w = Workspace::new();
    let input = w.project().join("art.pixel");
    let revision = automation::document_revision(&formats::load(&input).unwrap());
    assert_error(
        &file_call("save", json!({"input":input,"revision":revision})),
        "Output already exists",
    );
    assert_error(
        &file_call(
            "save",
            json!({"input":input,"revision":"stale","overwrite":true}),
        ),
        "Stale revision",
    );
    let saved = file_call(
        "save",
        json!({"input":input,"revision":revision,"overwrite":true}),
    );
    assert!(saved["isError"].is_null(), "{saved}");
}

#[test]
fn mcp_preflights_engine_sidecars_and_preserves_unity_identity_on_overwrite() {
    let w = Workspace::new();
    let metadata = w.project().join("sheet.png.meta");
    let old = "fileFormatVersion: 2\nguid: 123456789012345678901234567890ab\n";
    fs::write(&metadata, old).unwrap();
    let mut args = json!({"input":"art.pixel","output":"sheet.png","format":"sheet","options":{"preset":"Unity"}});
    assert_error(&w.call("export", args.clone()), "Output already exists");
    assert!(!w.project().join("sheet.png").exists());
    args["overwrite"] = json!(true);
    let exported = w.call("export", args);
    assert!(exported["isError"].is_null(), "{exported}");
    assert!(
        fs::read_to_string(metadata).unwrap().starts_with(old),
        "re-export should keep the existing Unity GUID"
    );
}

#[test]
fn mcp_sequence_overwrite_replaces_only_generated_frames() {
    let w = Workspace::new();
    let dir = w.project().join("frames");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("frame-0001.png"), b"original").unwrap();
    fs::write(dir.join("notes.txt"), b"keep").unwrap();
    let mut args = json!({"input":"art.pixel","output":"frames","format":"png-sequence"});
    assert_error(&w.call("export", args.clone()), "Output already exists");
    assert_eq!(fs::read(dir.join("frame-0001.png")).unwrap(), b"original");
    args["overwrite"] = json!(true);
    let exported = w.call("export", args);
    assert!(exported["isError"].is_null(), "{exported}");
    assert_eq!(image::open(dir.join("frame-0001.png")).unwrap().width(), 2);
    assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"keep");
}

#[test]
fn mcp_preflights_atlas_metadata_before_writing_atlas_image() {
    let w = Workspace::new();
    let metadata = w.project().join("atlas.json");
    fs::write(&metadata, b"keep").unwrap();
    let args = json!({"inputs":["art.pixel"],"output":"atlas.png"});
    assert_error(&w.call("atlas", args), "Output already exists");
    assert!(!w.project().join("atlas.png").exists());
    assert_eq!(fs::read(metadata).unwrap(), b"keep");
}

#[test]
fn restricted_mcp_rejects_outside_paths_before_probing_them() {
    let w = Workspace::new();
    for input in ["../outside.pixel", "../missing.pixel"] {
        assert_error(
            &w.call("inspect", json!({"input":input})),
            "Path escapes the project root",
        );
    }
}

#[cfg(unix)]
#[test]
fn restricted_mcp_rejects_special_files_as_inputs() {
    let w = Workspace::new();
    let _socket = std::os::unix::net::UnixListener::bind(w.project().join("socket.pixel")).unwrap();
    assert_error(
        &w.call("inspect", json!({"input":"socket.pixel"})),
        "Input must be a regular file",
    );
}

#[cfg(windows)]
#[test]
fn restricted_mcp_rejects_external_unc_roots_before_probing_them() {
    let w = Workspace::new();
    for input in [
        r"\\192.0.2.1\share\outside.pixel",
        r"\\?\UNC\192.0.2.1\share\outside.pixel",
    ] {
        assert_error(
            &w.call("inspect", json!({"input":input})),
            "Path escapes the project root",
        );
    }
}

#[test]
fn restricted_mp4_export_passes_an_open_destination_to_ffmpeg() {
    if !Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|run| run.status.success())
    {
        return;
    }
    let w = Workspace::new();
    let reply = w.call(
        "export",
        json!({"input":"art.pixel","output":"video.mp4","format":"mp4"}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let video = fs::read(w.project().join("video.mp4")).unwrap();
    assert_eq!(&video[4..8], b"ftyp");
    assert!(video.windows(4).any(|part| part == b"moof"));
}
