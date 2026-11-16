use pixelcraft::{automation, formats, model::Document};
use serde_json::{Value, json};
#[cfg(any(target_os = "linux", windows))]
use std::io::Write;
use std::{
    fs,
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

    #[cfg(any(target_os = "linux", windows))]
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

#[cfg(any(target_os = "linux", windows))]
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
#[cfg(target_os = "linux")]
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

#[cfg(any(target_os = "linux", windows))]
#[test]
fn project_root_rejects_hard_linked_inputs() {
    let w = Workspace::new();
    let outside = w.0.join("outside.pixel");
    let linked = w.project().join("linked.pixel");
    fs::hard_link(&outside, &linked).unwrap();
    assert_error(
        &w.call("render", json!({"input":"linked.pixel"})),
        "hard link",
    );
    assert!(formats::load(&linked).is_ok());
}

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(windows)]
#[test]
fn project_root_accepts_ordinary_absolute_tiled_companion_paths() {
    let workspace = Workspace::new();
    let sibling = workspace.0.join("project-other");
    fs::create_dir(&sibling).unwrap();
    for (index, directory) in [workspace.project(), sibling].into_iter().enumerate() {
        let image_path = directory.join("tiles.png");
        image::RgbaImage::new(1, 1).save(&image_path).unwrap();
        let canonical = image_path.canonicalize().unwrap();
        let spelling = canonical.to_str().unwrap();
        let ordinary = if let Some(unc) = spelling.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            spelling.strip_prefix(r"\\?\").unwrap().to_owned()
        };
        fs::write(
            workspace.project().join("tiles.tsj"),
            json!({
                "type":"tileset", "tilewidth":1, "tileheight":1, "tilecount":1,
                "columns":1, "image":ordinary.replace('\\', "/")
            })
            .to_string(),
        )
        .unwrap();
        let result = workspace.call("inspect", json!({"input":"tiles.tsj"}));
        if index == 0 {
            assert!(result["isError"].is_null(), "{result}");
        } else {
            assert_error(&result, "project root");
        }
    }
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn malformed_tiled_geometry_returns_mcp_error() {
    let w = Workspace::new();
    image::RgbaImage::new(2, 2)
        .save(w.project().join("tiles.png"))
        .unwrap();
    image::RgbaImage::new(16_385, 1)
        .save(w.project().join("wide.png"))
        .unwrap();
    let tiles = json!({
        "type":"tileset", "tilewidth":1, "tileheight":1, "tilecount":1,
        "columns":1, "image":"tiles.png"
    });
    let mut oversized = tiles.clone();
    oversized["tilewidth"] = json!(16_385);
    oversized["image"] = json!("wide.png");
    let mut huge_margin = tiles.clone();
    huge_margin["margin"] = json!(u64::MAX);
    let mut huge_spacing = tiles.clone();
    huge_spacing["spacing"] = json!(u64::MAX);
    let mut huge_tile_table = tiles.clone();
    huge_tile_table["tilecount"] = json!(64_000_000);
    for (input, value, error) in [
        ("wide.tsj", oversized, "Tileset exceeds limits"),
        ("margin.tsj", huge_margin, "Tile outside image"),
        ("spacing.tsj", huge_spacing, "Invalid tileset spacing"),
        ("huge-table.tsj", huge_tile_table, "storage budget"),
    ] {
        fs::write(w.project().join(input), value.to_string()).unwrap();
        assert_error(&w.call("inspect", json!({"input":input})), error);
    }

    fs::write(w.project().join("tiles.tsj"), tiles.to_string()).unwrap();
    let map = json!({
        "orientation":"orthogonal", "width":1, "height":1, "tilewidth":1, "tileheight":1,
        "tilesets":[{"firstgid":1, "source":"tiles.tsj"}],
        "layers":[{"type":"tilelayer", "width":1, "height":1, "data":[1]}]
    });
    let mut invalid_gid = map.clone();
    invalid_gid["tilesets"][0]["firstgid"] = json!(u32::MAX);
    let mut overlapping = map.clone();
    overlapping["tilesets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"firstgid":1,"source":"tiles.tsj"}));
    let mut invalid_offset = map;
    invalid_offset["layers"][0]["x"] = json!(i64::MAX);
    invalid_offset["layers"][0]["offsetx"] = json!(1);
    for (input, value, error) in [
        ("gid.tmj", invalid_gid, "Invalid first global tile ID"),
        ("offset.tmj", invalid_offset, "Invalid tile layer offset"),
        (
            "overlap.tmj",
            overlapping,
            "Overlapping global tile ID ranges",
        ),
    ] {
        fs::write(w.project().join(input), value.to_string()).unwrap();
        assert_error(&w.call("inspect", json!({"input":input})), error);
    }
}

#[test]
fn tiled_map_keeps_layer_order_when_tileset_ids_are_unsorted() {
    let w = Workspace::new();
    let dir = w.project();
    let mut image = image::RgbaImage::new(1, 1);
    for (name, color) in [
        ("red.png", [255, 0, 0, 255]),
        ("green.png", [0, 255, 0, 255]),
    ] {
        image.put_pixel(0, 0, image::Rgba(color));
        image.save(dir.join(name)).unwrap();
    }
    let tileset = |image, firstgid| {
        json!({
            "firstgid":firstgid, "tilewidth":1, "tileheight":1, "tilecount":1,
            "columns":1, "image":image,
        })
    };
    let map = json!({
        "orientation":"orthogonal", "width":2, "height":1, "tilewidth":1, "tileheight":1,
        "tilesets":[tileset("red.png", 10), tileset("green.png", 1)],
        "layers":[{"type":"tilelayer", "width":2, "height":1, "data":[1, 10]}]
    });
    let path = dir.join("map.tmj");
    fs::write(&path, map.to_string()).unwrap();
    let decoded = formats::load(&path).unwrap();
    assert_eq!(decoded.layers[0].tileset, Some(0));
    assert_eq!(decoded.layers[1].tileset, Some(1));
    assert_eq!(
        decoded.composite(0),
        vec![[0, 255, 0, 255], [255, 0, 0, 255]]
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

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(target_os = "linux")]
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
fn restricted_atlas_folder_skips_junctions_but_direct_access_rejects_them() {
    let workspace = Workspace::new();
    let outside = workspace.0.join("outside");
    fs::create_dir(&outside).unwrap();
    formats::save(&outside.join("other.pixel"), &Document::new(1, 1)).unwrap();
    let junction = Command::new("cmd")
        .current_dir(&workspace.0)
        .args(["/d", "/c", "mklink", "/j", r"project\junction", "outside"])
        .output()
        .unwrap();
    assert!(junction.status.success(), "{junction:?}");

    let result = workspace.call("atlas", json!({"inputs":["."],"output":"atlas.png"}));
    assert!(result["isError"].is_null(), "{result}");
    let metadata: Value =
        serde_json::from_slice(&fs::read(workspace.project().join("atlas.json")).unwrap()).unwrap();
    let sources = metadata["meta"]["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["name"], "art.pixel");
    assert_error(
        &workspace.call("inspect", json!({"input":"junction/other.pixel"})),
        "project root",
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

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(any(target_os = "linux", windows))]
#[test]
fn restricted_webp_import_keeps_the_shipped_decoder_available() {
    let w = Workspace::new();
    fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflows/animation.webp"),
        w.project().join("animation.webp"),
    )
    .unwrap();
    let reply = w.call("inspect", json!({"input":"animation.webp"}));
    assert!(reply["isError"].is_null(), "{reply}");
    let sprite: Value =
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(sprite["frames"].as_array().unwrap().len(), 2);
}

#[cfg(not(any(target_os = "linux", windows)))]
#[test]
fn restricted_mcp_refuses_to_start_when_containment_is_unsupported() {
    let workspace = Workspace::new();
    let result = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
        .arg("--mcp")
        .arg("--project-root")
        .arg(workspace.project())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{error}");
    assert!(error.contains("unavailable on this platform"), "{error}");
}
