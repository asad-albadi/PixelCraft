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

#[test]
fn export_batch_previews_exact_outputs_and_runs_the_ordered_operations() {
    let w = Workspace::new();
    let input = w.project().join("art.pixel");
    let sheet = w.project().join("sheet.png");
    let sequence = w.project().join("sequence");
    let tags = w.project().join("tags");
    let palette = w.project().join("colors.gpl");
    let mut doc = formats::load(&input).unwrap();
    doc.tags.push(pixelcraft::model::FrameTag {
        name: "Idle".into(),
        from: 0,
        to: 0,
        direction: 0,
        repeat: 0,
        color: [10, 20, 30],
    });
    formats::save(&input, &doc).unwrap();
    let steps = json!([
        {"operation":"export","arguments":{"input":input,"output":sheet,"format":"sheet"}},
        {"operation":"export","arguments":{"input":input,"output":sequence,"format":"png-sequence"}},
        {"operation":"batch-tags","arguments":{"input":input,"output":tags,"options":{"preset":"Unity"}}},
        {"operation":"palette-export","arguments":{"input":input,"output":palette}},
    ]);
    let cli = |args: Value| automation::run_cli(&["batch".into(), args.to_string()]);
    let preview = cli(json!({"steps":steps,"dry_run":true})).unwrap();
    let flag_preview = automation::run_cli(&[
        "batch".into(),
        "--steps".into(),
        steps.to_string(),
        "--dry-run".into(),
        "true".into(),
    ])
    .unwrap();
    assert_eq!(flag_preview, preview);
    assert_eq!(
        preview["steps"][0]["outputs"],
        json!([sheet, sheet.with_extension("json")])
    );
    assert_eq!(
        preview["steps"][1]["outputs"],
        json!([sequence.join("frame-0001.png")])
    );
    assert!(
        !sheet.exists()
            && !sheet.with_extension("json").exists()
            && !sequence.exists()
            && !tags.exists()
            && !palette.exists()
    );
    let result = cli(json!({"steps":steps})).unwrap();
    assert_eq!(result["steps"][0]["operation"], "export");
    assert_eq!(
        result["steps"][1]["outputs"],
        preview["steps"][1]["outputs"]
    );
    assert_eq!(
        preview["steps"][2]["outputs"],
        json!([
            tags.join("001-Idle.png"),
            tags.join("001-Idle.json"),
            tags.join("001-Idle.png.meta"),
        ])
    );
    assert_eq!(preview["steps"][3]["outputs"], json!([palette]));
    assert!(tags.join("001-Idle.png.meta").is_file() && palette.is_file());
    assert!(sheet.is_file() && sheet.with_extension("json").is_file());
    assert!(sequence.join("frame-0001.png").is_file());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn restricted_export_batch_rejects_unsafe_later_steps_before_any_writes() {
    let w = Workspace::new();
    let step = |output| json!({"operation":"export","arguments":{"input":"art.pixel","output":output,"format":"png"}});
    let reply = w.call(
        "batch",
        json!({"steps":[step("first.png"),step("../escape.png")]}),
    );
    assert_error(&reply, "project root");
    assert!(!w.project().join("first.png").exists());
    let reply = w.call(
        "batch",
        json!({"steps":[step("same.png"),step("same.png")]}),
    );
    assert_error(&reply, "Duplicate output");
    assert!(!w.project().join("same.png").exists());
    let mut invalid = step("bad.png");
    invalid["arguments"]["region"] = json!({"x":1,"y":1,"width":8,"height":8});
    let reply = w.call("batch", json!({"steps":[step("first.png"),invalid]}));
    assert_error(&reply, "inside the canvas");
    assert!(!w.project().join("first.png").exists());
    let preview = w.call("batch", json!({"steps":[step("first.png")],"dry_run":true}));
    assert!(preview["isError"].is_null(), "{preview}");
    assert!(!w.project().join("first.png").exists());
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
#[cfg(any(target_os = "linux", windows))]
#[test]
fn restricted_mcp_tileset_atlas_respects_root_and_overwrite() {
    let w = Workspace::new();
    let mut doc = Document::new(1, 1);
    doc.tilesets.push(pixelcraft::model::Tileset {
        name: "Author palette".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[20, 60, 200, 255]].into()],
    });
    doc.add_tile_layer(0).unwrap();
    formats::save(&w.project().join("tiles.pixel"), &doc).unwrap();
    let args = json!({"input":"tiles.pixel","output":"atlas.png","format":"tileset-atlas"});
    let reply = w.call("export", args.clone());
    assert!(reply["isError"].is_null(), "{reply}");
    let bytes = fs::read(w.project().join("atlas.png")).unwrap();
    let reply = w.call("export", args);
    assert_error(&reply, "overwrite");
    assert_eq!(fs::read(w.project().join("atlas.png")).unwrap(), bytes);
    assert_error(
        &w.call(
            "export",
            json!({
                "input":"tiles.pixel","output":"../escaped.png","format":"tileset-atlas",
            }),
        ),
        "project root",
    );
    assert!(!w.0.join("escaped.png").exists());
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

#[cfg(any(target_os = "linux", windows))]
#[test]
fn restricted_split_slice_exports_update_all_unity_sidecars() {
    use pixelcraft::model::{SliceKey, SpriteSlice};

    let workspace = Workspace::new();
    let root = workspace.project();
    let input = root.join("art.pixel");
    let mut sprite = Document::new(2, 2);
    sprite.set_pixel(0, 0, 0, [235, 40, 20, 255]);
    sprite.slices = ["left", "right"]
        .into_iter()
        .enumerate()
        .map(|(x, name)| SpriteSlice {
            name: name.into(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: [x as i32, 0, 1, 2],
                center: None,
                pivot: None,
            }],
        })
        .collect();
    formats::save(&input, &sprite).unwrap();

    let export = |overwrite| {
        workspace.call(
            "export",
            json!({"input":"art.pixel","output":"sheets","format":"sheet","overwrite":overwrite,
                "options":{"split_slices":true,"preset":"Unity","padding":0,"extrude":1}}),
        )
    };
    let first = export(false);
    assert!(first["isError"].is_null(), "{first}");
    let left = root.join("sheets/001-left.png");
    let right = root.join("sheets/002-right.png");
    let identities = |image: &std::path::Path| {
        let meta = fs::read_to_string(image.with_extension("png.meta")).unwrap();
        meta.lines()
            .filter(|line| line.starts_with("guid:") || line.contains("internalID:"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let first_ids = [identities(&left), identities(&right)];
    assert!(first_ids.iter().all(|ids| ids.len() == 2));
    let blocked = export(false);
    assert_error(&blocked, "already exists");
    let recolored = [25, 195, 85, 255];
    sprite.set_pixel(0, 0, 0, recolored);
    formats::save(&input, &sprite).unwrap();
    let updated = export(true);
    assert!(updated["isError"].is_null(), "{updated}");
    assert_eq!([identities(&left), identities(&right)], first_ids);
    assert_eq!(
        image::open(&left).unwrap().into_rgba8().get_pixel(1, 1).0,
        recolored
    );
    assert_eq!(fs::read_dir(root.join("sheets")).unwrap().count(), 6);
    assert!(!fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".pixelcraft-output-")
    }));
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn sheet_filters_groups_and_frame_ranges_in_cli_and_mcp() {
    use pixelcraft::model::{CLEAR, FrameTag, SliceKey, SpriteSlice};

    let workspace = Workspace::new();
    let root = workspace.project();
    let mut sprite = Document::new(3, 1);
    let group = sprite.add_group("Characters").unwrap();
    sprite.add_layer();
    let actor = sprite.layers.len() - 1;
    sprite.layers[actor].parent = Some(group);
    sprite.add_layer();
    let hidden = sprite.layers.len() - 1;
    sprite.layers[hidden].parent = Some(group);
    sprite.layers[hidden].visible = false;
    sprite.layers[group].opacity = 128;
    sprite.add_frame(0, false);
    sprite.add_frame(1, false);
    sprite.durations = vec![80, 120, 160];
    sprite.set_frame_name(2, "jump apex").unwrap();
    sprite.set_pixel(0, 2, 0, [230, 90, 10, 255]);
    sprite.set_pixel(actor, 2, 1, [10, 80, 230, 255]);
    sprite.set_pixel(hidden, 2, 2, [60, 180, 40, 255]);
    sprite.tags.push(FrameTag {
        name: "Jump".into(),
        from: 1,
        to: 2,
        repeat: 3,
        direction: 2,
        color: [10, 20, 30],
    });
    sprite.slices.push(SpriteSlice {
        name: "Actor".into(),
        keys: vec![SliceKey {
            frame: 0,
            bounds: [1, 0, 2, 1],
            center: None,
            pivot: None,
        }],
    });
    let input = root.join("art.pixel");
    formats::save(&input, &sprite).unwrap();

    let output = root.join("filtered.png");
    automation::run_cli(&[
        "export".into(),
        json!({"input":input,"output":output,"format":"sheet","layers":[group],
            "options":{"padding":0,"tag":0,"frame_range":[2,2]}})
        .to_string(),
    ])
    .unwrap();
    let image = image::open(&output).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (3, 1));
    assert_eq!(
        image.pixels().map(|p| p.0).collect::<Vec<_>>(),
        vec![CLEAR, [10, 80, 230, 128], CLEAR]
    );
    let metadata: Value =
        serde_json::from_slice(&fs::read(output.with_extension("json")).unwrap()).unwrap();
    assert_eq!(metadata["frames"][0]["frameId"], sprite.frame_export_id(2));
    assert_eq!(metadata["frames"][0]["frameName"], "jump apex");
    assert_eq!(metadata["frames"][0]["duration"], 160);
    assert_eq!(metadata["frames"][0]["sourceFrame"], 2);
    assert_eq!(
        metadata["meta"]["frameTags"],
        json!([{"name":"Jump","from":0,"to":0,"repeat":3,"direction":"pingpong"}])
    );
    assert_eq!(metadata["meta"]["slices"][0]["keys"][0]["frame"], 0);

    let plan = automation::operation(
        "batch",
        &json!({"dry_run":true,"steps":[{"operation":"export","arguments":{
            "input":input,"output":root.join("planned.png"),"format":"sheet","layers":[group],
            "options":{"frame_range":[2,2]}}}]}),
    )
    .unwrap();
    assert!(plan.to_string().contains("planned.json"));
    assert!(!root.join("planned.png").exists());

    let reply = workspace.call(
        "export",
        json!({"input":"art.pixel","output":"slices","format":"sheet",
        "options":{"padding":0,"layers":[hidden],"frame_range":[2,2],"split_slices":true}}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let slice = image::open(root.join("slices/001-Actor.png"))
        .unwrap()
        .into_rgba8();
    assert_eq!(slice.dimensions(), (2, 1));
    assert_eq!(
        slice.pixels().map(|p| p.0).collect::<Vec<_>>(),
        vec![CLEAR, [60, 180, 40, 128]]
    );

    for (options, message) in [
        (json!({"layers":[999]}), "valid layer"),
        (json!({"frame_range":[2,1]}), "frame range"),
        (json!({"tag":0,"frame_range":[0,0]}), "overlap"),
    ] {
        let reply = workspace.call(
            "export",
            json!({"input":"art.pixel","output":"bad.png","format":"sheet","options":options}),
        );
        assert_error(&reply, message);
        assert!(!root.join("bad.png").exists());
    }
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn atlas_metadata_can_be_keyed_or_omitted_without_touching_existing_sidecars() {
    let w = Workspace::new();
    let reply = w.call(
        "atlas",
        json!({"inputs":["art.pixel"], "output":"keyed.png","options":{"metadata":"hash"}}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let keyed: Value =
        serde_json::from_slice(&fs::read(w.project().join("keyed.json")).unwrap()).unwrap();
    assert_eq!(keyed["frames"]["0/art.pixel/0"]["frameId"], 0);

    let sidecar = w.project().join("image-only.json");
    fs::write(&sidecar, b"keep").unwrap();
    let reply = w.call(
        "atlas",
        json!({"inputs":["art.pixel"],"output":"image-only.png","options":{"metadata":"none"}}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    assert!(w.project().join("image-only.png").is_file());
    assert_eq!(fs::read(sidecar).unwrap(), b"keep");
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
