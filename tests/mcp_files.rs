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
fn named_slice_sheet_uses_keyed_bounds_and_rejects_unresolvable_names() {
    use pixelcraft::model::{CLEAR, SliceKey, SpriteSlice};
    let workspace = Workspace::new();
    let root = workspace.project();
    let mut sprite = Document::new(4, 1);
    sprite.add_frame(0, false);
    sprite.durations = vec![70, 140];
    let red = [200, 20, 10, 255];
    let blue = [10, 40, 210, 255];
    sprite.set_pixel(0, 0, 0, red);
    sprite.set_pixel(0, 1, 3, blue);
    sprite.set_frame_name(1, "second").unwrap();
    let key = |frame, x| SliceKey {
        frame,
        bounds: [x, 0, 2, 1],
        center: None,
        pivot: Some([1, 0]),
    };
    sprite.slices = vec![
        SpriteSlice {
            name: "Other".into(),
            keys: vec![key(0, 1)],
        },
        SpriteSlice {
            name: "Portrait".into(),
            keys: vec![key(0, -1), key(1, 3)],
        },
    ];
    let path = root.join("art.pixel");
    formats::save(&path, &sprite).unwrap();
    let export = |name: &str, file: &str| {
        workspace.call(
            "export",
            json!({"input":"art.pixel","output":file,"format":"sheet",
          "options":{"slice":name,"columns":2,"padding":0,"preset":"Unity"}}),
        )
    };
    let reply = export("Portrait", "portrait.png");
    assert!(reply["isError"].is_null(), "{reply}");
    let png = image::open(root.join("portrait.png")).unwrap().into_rgba8();
    assert_eq!(png.dimensions(), (4, 1));
    assert_eq!(
        png.pixels().map(|p| p.0).collect::<Vec<_>>(),
        vec![CLEAR, red, blue, CLEAR]
    );
    assert!(root.join("portrait.png.meta").exists());
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("portrait.json")).unwrap()).unwrap();
    assert_eq!(metadata["frames"][1]["frameId"], sprite.frame_export_id(1));
    assert_eq!(metadata["frames"][1]["frameName"], "second");
    assert_eq!(metadata["frames"][1]["duration"], 140);
    assert_eq!(metadata["meta"]["slices"][0]["name"], "Portrait");
    assert_eq!(
        metadata["meta"]["slices"][0]["keys"][1]["pivot"],
        json!([1, 0])
    );
    assert_eq!(metadata["meta"]["slices"].as_array().unwrap().len(), 1);

    let request = json!({
        "input":path,"output":root.join("split"),"format":"sheet",
        "options":{"slice":"Portrait","split_slices":true,"padding":0}
    });
    let dry_run = automation::operation(
        "batch",
        &json!({"dry_run":true,"steps":[{"operation":"export","arguments":request}]}),
    )
    .unwrap();
    let files = dry_run.to_string();
    assert!(files.contains("002-Portrait.png") && files.contains("002-Portrait.json"));
    assert!(!files.contains("001-Other"));
    assert!(!root.join("split").exists());
    automation::run_cli(&["export".into(), request.to_string()]).unwrap();
    assert!(root.join("split/002-Portrait.png").exists());
    assert!(!root.join("split/001-Other.png").exists());

    let reply = export("Missing", "missing.png");
    assert_error(&reply, "Unknown slice");
    assert!(!root.join("missing.png").exists());
    sprite.slices[0].name = "Portrait".into();
    formats::save(&path, &sprite).unwrap();
    let reply = export("Portrait", "ambiguous.png");
    assert_error(&reply, "Ambiguous slice");
    assert!(!root.join("ambiguous.png").exists());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn sheet_splits_layers_and_grid_cells_into_independent_animation_samples() {
    use pixelcraft::model::{DrawingGrid, FrameTag, SliceKey, SpriteSlice};
    let workspace = Workspace::new();
    let root = workspace.project();
    let mut sprite = Document::new(5, 1);
    let group = sprite.add_group("Actors").unwrap();
    sprite.add_layer();
    sprite.layers[2].parent = Some(group);
    sprite.layers[group].opacity = 128;
    sprite.layers[0].name = "Sky".into();
    sprite.layers[2].name = "Actor".into();
    sprite.add_frame(0, false);
    sprite.durations = vec![75, 125];
    sprite.drawing_grid = Some(DrawingGrid {
        origin: [1, 0],
        size: [2, 1],
    });
    sprite.tags.push(FrameTag {
        name: "Step".into(),
        from: 0,
        to: 1,
        direction: 0,
        repeat: 0,
        color: [0; 3],
    });
    let colors = [[200, 80, 10, 255], [40, 150, 245, 255]];
    for (f, color) in colors.into_iter().enumerate() {
        sprite.set_pixel(0, f, 1, color);
        sprite.set_pixel(2, f, 3, color);
    }
    sprite.slices.push(SpriteSlice {
        name: "Stage".into(),
        keys: vec![SliceKey {
            frame: 0,
            bounds: [1, 0, 4, 1],
            center: None,
            pivot: None,
        }],
    });
    let input = root.join("art.pixel");
    formats::save(&input, &sprite).unwrap();

    let output = root.join("split.png");
    let export = || {
        workspace.call("export",json!({
        "input":"art.pixel","output":"split.png","format":"sheet","overwrite":true,
        "options":{"split_layers":true,"split_grid":true,"padding":0,"columns":8,"preset":"Unity"}
    }))
    };
    let reply = export();
    assert!(reply["isError"].is_null(), "{reply}");
    let im = image::open(&output).unwrap().into_rgba8();
    assert_eq!(im.dimensions(), (16, 1));
    assert_eq!(im.get_pixel(0, 0).0, colors[0]);
    assert_eq!(im.get_pixel(2, 0).0, colors[1]);
    assert_eq!(im.get_pixel(4, 0).0, [0; 4]);
    assert_eq!(im.get_pixel(8, 0).0, [0; 4]);
    assert_eq!(im.get_pixel(12, 0).0, [200, 80, 10, 128]);
    assert_eq!(im.get_pixel(14, 0).0, [40, 150, 245, 128]);
    let json_path = output.with_extension("json");
    let metadata: Value = serde_json::from_slice(&fs::read(&json_path).unwrap()).unwrap();
    let entries = metadata["frames"].as_array().unwrap();
    assert_eq!(entries.len(), 8);
    assert_eq!(entries[7]["sourceLayer"], 2);
    assert_eq!(entries[7]["sourceLayerName"], "Actor");
    assert_eq!(entries[7]["gridCell"], json!({"x":3,"y":0,"w":2,"h":1}));
    assert_eq!(entries[7]["sourceFrameId"], sprite.frame_export_id(1));
    assert_eq!(entries[7]["sourceFrame"], 1);
    assert_eq!(entries[7]["duration"], 125);
    assert_eq!(metadata["meta"]["frameTags"][3]["from"], 6);
    assert_eq!(metadata["meta"]["frameTags"][3]["to"], 7);
    let ids = |items: &[Value]| {
        items
            .iter()
            .map(|entry| entry["filename"].clone())
            .collect::<Vec<_>>()
    };
    let names = ids(entries);
    assert_eq!(
        names
            .iter()
            .map(Value::to_string)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    let unity_path = output.with_extension("png.meta");
    let unity_meta = fs::read(&unity_path).unwrap();
    sprite.set_pixel(2, 1, 3, [123, 32, 200, 255]);
    formats::save(&input, &sprite).unwrap();
    let reply = export();
    assert!(reply["isError"].is_null(), "{reply}");
    let updated: Value = serde_json::from_slice(&fs::read(json_path).unwrap()).unwrap();
    assert_eq!(names, ids(updated["frames"].as_array().unwrap()));
    assert_eq!(fs::read(unity_path).unwrap(), unity_meta);

    let reply = workspace.call("export",json!({
        "input":"art.pixel","output":"cropped.png","format":"sheet",
        "options":{"slice":"Stage","split_layers":true,"split_grid":true,"frame_range":[1,1],"padding":0}
    }));
    assert!(reply["isError"].is_null(), "{reply}");
    let img = image::open(root.join("cropped.png")).unwrap().into_rgba8();
    assert_eq!(img.dimensions(), (4, 1));
    assert_eq!(img.get_pixel(3, 0).0, [123, 32, 200, 128]);
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("cropped.json")).unwrap()).unwrap();
    assert_eq!(metadata["meta"]["sourceSlice"], "Stage");
    assert_eq!(metadata["frames"][1]["sourceFrame"], 1);

    for (options, message) in [
        (
            json!({"split_grid":true,"extrude":4294967295u64}),
            "extrusion",
        ),
        (json!({"split_grid":true,"scale":10000}), "image limits"),
    ] {
        let output = root.join("bad.png");
        let error = automation::operation(
            "batch",
            &json!({
                "dry_run":true,"steps":[{"operation":"export","arguments":{
                    "input":input,"output":output,"format":"sheet","options":options
                }}]
            }),
        )
        .unwrap_err();
        assert!(error.contains(message), "{error}");
        assert!(!output.exists());
    }
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn sheet_templates_keep_names_and_unity_reimport_ids_stable() {
    use pixelcraft::model::FrameTag;
    let w = Workspace::new();
    let root = w.project();
    let source = root.join("art.pixel");
    let mut sprite = Document::new(1, 1);
    sprite.add_frame(0, false);
    sprite.durations = vec![90, 110];
    sprite.set_frame_name(1, r#"Pose "B": #2"#).unwrap();
    sprite.tags.push(FrameTag {
        name: "Walk".into(),
        from: 0,
        to: 1,
        direction: 0,
        repeat: 0,
        color: [0; 3],
    });
    formats::save(&source, &sprite).unwrap();
    let export = || {
        w.call(
            "export",
            json!({"input":"art.pixel","output":"names.png","format":"sheet","overwrite":true,
        "options":{"preset":"Unity","filename_template":r#"{title}/{tag}/{frame001}: {frame_name}"#,
                   "tag_template":"{tag}_{from}_{to}"}}),
        )
    };
    let reply = export();
    assert!(reply["isError"].is_null(), "{reply}");
    let read =
        || serde_json::from_slice::<Value>(&fs::read(root.join("names.json")).unwrap()).unwrap();
    let metadata = read();
    assert_eq!(
        metadata["frames"][1]["filename"],
        r#"names/Walk/002: Pose "B": #2"#
    );
    assert_eq!(metadata["meta"]["frameTags"][0]["name"], "Walk_0_1");
    let sidecar = root.join("names.png.meta");
    let previous = fs::read_to_string(&sidecar).unwrap();
    let name = serde_json::to_string(metadata["frames"][1]["filename"].as_str().unwrap()).unwrap();
    assert!(previous.contains(&format!("      name: {name}\n")));
    assert!(previous.contains(&format!("      {name}: ")));
    // A hand-authored importer ID proves we read the quoted name back, even
    // when the default hashed GUID would have coincidentally matched.
    let prior_id = format!("      {name}: 21300002");
    assert!(previous.contains(&prior_id));
    fs::write(
        &sidecar,
        previous.replace(&prior_id, &format!("      {name}: 4123")),
    )
    .unwrap();
    sprite.set_pixel(0, 1, 0, [55, 120, 230, 255]);
    formats::save(&source, &sprite).unwrap();
    let reply = export();
    assert!(reply["isError"].is_null(), "{reply}");
    assert_eq!(
        read()["frames"][1]["filename"],
        metadata["frames"][1]["filename"]
    );
    assert!(
        fs::read_to_string(&sidecar)
            .unwrap()
            .contains(&format!("      {name}: 4123"))
    );
    for (template, message) in [("same", "Duplicate"), ("{unknown}", "Unknown")] {
        let reply = w.call(
            "export",
            json!({"input":"art.pixel","output":"bad.png","format":"sheet",
          "options":{"filename_template":template}}),
        );
        assert_error(&reply, message);
        assert!(!root.join("bad.png").exists());
    }
    let dry_run = automation::operation(
        "batch",
        &json!({
            "dry_run":true,"steps":[{"operation":"export","arguments":{
                "input":source,"output":root.join("planned.png"),"format":"sheet",
                "options":{"filename_template":"same"}
            }}]
        }),
    )
    .unwrap_err();
    assert!(
        dry_run.contains("Duplicate exported frame name"),
        "{dry_run}"
    );
    assert!(!root.join("planned.png").exists());

    let mut sprite = formats::load(&source).unwrap();
    sprite.add_layer();
    sprite.layers[1].name = "Ink".into();
    sprite.tags.push(FrameTag {
        name: "Loop".into(),
        from: 1,
        to: 1,
        direction: 0,
        repeat: 0,
        color: [0; 3],
    });
    formats::save(&source, &sprite).unwrap();
    let reply = w.call(
        "export",
        json!({
            "input":"art.pixel","output":"templated-splits.png","format":"sheet","layers":[1],
            "options":{"preset":"Godot","split_layers":true,
               "filename_template":"{layer}_{innertag}_{outertag}_{tagframe:02}_{frame_id}",
               "tag_template":"{layer}_{tag}_{from}_{to}"}
        }),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("templated-splits.json")).unwrap()).unwrap();
    assert_eq!(metadata["frames"][1]["filename"], "Ink_Loop_Walk_00_1");
    assert_eq!(metadata["meta"]["frameTags"][1]["name"], "Ink_Loop_1_1");
    assert!(
        fs::read_to_string(root.join("templated-splits.tres"))
            .unwrap()
            .contains("Ink_Loop_1_1")
    );
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn sprite_sheet_spacing_and_grid_trim_match_pixels_and_metadata() {
    use pixelcraft::model::{CLEAR, DrawingGrid};
    let workspace = Workspace::new();
    let root = workspace.project();
    let input = root.join("art.pixel");
    let red = [220, 45, 30, 255];
    let mut sprite = Document::new(2, 1);
    sprite.add_frame(0, false);
    sprite.set_pixel(0, 0, 0, red);
    sprite.set_pixel(0, 1, 1, red);
    formats::save(&input, &sprite).unwrap();
    let export = |file, options: Value| {
        let reply = workspace.call(
            "export",
            json!({
                "input":"art.pixel","output":file,"format":"sheet","options":options,
            }),
        );
        assert!(reply["isError"].is_null(), "{reply}");
    };
    export(
        "spaced.png",
        json!({
            "scale":2,"columns":2,"padding":0,"border_padding":2,"shape_padding":3,
            "inner_padding":1,"extrude":1
        }),
    );
    let sheet = image::open(root.join("spaced.png")).unwrap().into_rgba8();
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("spaced.json")).unwrap()).unwrap();
    assert_eq!(sheet.dimensions(), (38, 18));
    assert_eq!(sheet.get_pixel(4, 8).0, CLEAR);
    assert_eq!(sheet.get_pixel(6, 8).0, red);
    assert_eq!(sheet.get_pixel(26, 8).0, CLEAR);
    assert_eq!(sheet.get_pixel(28, 8).0, red);
    assert_eq!(
        metadata["frames"][0]["frame"],
        json!({"x":8,"y":8,"w":4,"h":2})
    );
    assert_eq!(
        metadata["frames"][1]["frame"],
        json!({"x":26,"y":8,"w":4,"h":2})
    );
    assert_eq!(metadata["meta"]["borderPadding"], 4);
    assert_eq!(metadata["meta"]["shapePadding"], 6);
    assert_eq!(metadata["meta"]["innerPadding"], 2);
    let reopened = formats::load(&root.join("spaced.json")).unwrap();
    assert_eq!(
        reopened.composite(0),
        vec![red, red, CLEAR, CLEAR, red, red, CLEAR, CLEAR]
    );

    let reply = workspace.call(
        "export",
        json!({
            "input":"art.pixel","output":"bad-spacing.png","format":"sheet",
            "options":{"inner_padding":257}
        }),
    );
    assert_error(&reply, "padding and extrusion");
    assert!(!root.join("bad-spacing.png").exists());

    let mut sprite = Document::new(7, 5);
    sprite.drawing_grid = Some(DrawingGrid {
        origin: [1, -1],
        size: [3, 3],
    });
    sprite.add_frame(0, false);
    sprite.set_pixel(0, 0, 3 * 7 + 5, red);
    sprite.set_pixel(0, 1, 0, red);
    formats::save(&input, &sprite).unwrap();
    export("grid.png", json!({"trim_by_grid":true,"padding":0}));
    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("grid.json")).unwrap()).unwrap();
    assert_eq!(
        metadata["frames"][0]["spriteSourceSize"],
        json!({"x":4,"y":2,"w":3,"h":3})
    );
    assert_eq!(
        metadata["frames"][1]["spriteSourceSize"],
        json!({"x":0,"y":0,"w":1,"h":2})
    );
    let reopened = formats::load(&root.join("grid.json")).unwrap();
    for frame in 0..2 {
        assert_eq!(reopened.composite(frame), sprite.composite(frame));
    }
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn sheet_layouts_fit_fixed_canvases_and_keep_frame_order() {
    let w = Workspace::new();
    let root = w.project();
    let input = root.join("art.pixel");
    let mut sprite = Document::new(2, 1);
    sprite.add_frame(0, false);
    sprite.add_frame(1, false);
    for f in 0..3 {
        sprite.set_pixel(0, f, 0, [f as u8 + 10, 40, 220, 255]);
    }
    formats::save(&input, &sprite).unwrap();
    for (name, options, size, positions) in [
        (
            "columns",
            json!({"layout":"columns","rows":2,"width":5,"height":4,"scale":2,"padding":0}),
            (10, 8),
            [(0, 0), (0, 2), (4, 0)],
        ),
        (
            "horizontal",
            json!({"layout":"horizontal","columns":1,"padding":0}),
            (6, 1),
            [(0, 0), (2, 0), (4, 0)],
        ),
        (
            "vertical",
            json!({"layout":"vertical","padding":0}),
            (2, 3),
            [(0, 0), (0, 1), (0, 2)],
        ),
        (
            "wrapped",
            json!({"layout":"rows","columns":5,"width":4,"padding":0}),
            (4, 2),
            [(0, 0), (2, 0), (0, 1)],
        ),
    ] {
        let output = format!("{name}.png");
        let result = w.call(
            "export",
            json!({"input":"art.pixel","output":output,"format":"sheet","options":options}),
        );
        assert!(result["isError"].is_null(), "{result}");
        let png = image::open(root.join(&output)).unwrap().into_rgba8();
        assert_eq!(png.dimensions(), size, "{name}");
        for (f, (x, y)) in positions.into_iter().enumerate() {
            assert_eq!(
                png.get_pixel(x, y).0,
                [f as u8 + 10, 40, 220, 255],
                "{name}"
            );
        }
        let restored = formats::load(&root.join(&output).with_extension("json")).unwrap();
        for f in 0..3 {
            assert_eq!(restored.composite(f)[0], [f as u8 + 10, 40, 220, 255]);
        }
    }

    let options = json!({"layout":"packed","width":2,"height":2,"padding":0,"trim":true});
    let result = w.call(
        "export",
        json!({"input":"art.pixel","output":"packed.png","format":"sheet","options":options}),
    );
    assert!(result["isError"].is_null(), "{result}");
    let png = image::open(root.join("packed.png")).unwrap().into_rgba8();
    assert_eq!(png.dimensions(), (2, 2));
    let restored = formats::load(&root.join("packed.json")).unwrap();
    for f in 0..3 {
        assert_eq!(restored.composite(f), sprite.composite(f));
    }
    // The variable-size case exercises MaxRects and keeps three different
    // trimmed shapes in timeline order even when their packed order differs.
    let mut varied = Document::new(4, 3);
    varied.add_frame(0, false);
    varied.add_frame(1, false);
    for (frame, [x, y]) in [[0, 0], [2, 1], [3, 2]].into_iter().enumerate() {
        varied.set_pixel(0, frame, 0, [31, 42, 53, 255]);
        varied.set_pixel(0, frame, y * 4 + x, [123, 80, 200, 255]);
    }
    formats::save(&input, &varied).unwrap();
    let result = w.call(
        "export",
        json!({"input":"art.pixel","output":"varied.png","format":"sheet",
        "options":{"layout":"packed","width":9,"height":10,"trim":true,
                   "border_padding":1,"shape_padding":1,"inner_padding":0}}),
    );
    assert!(result["isError"].is_null(), "{result}");
    let restored = formats::load(&root.join("varied.json")).unwrap();
    for f in 0..3 {
        assert_eq!(restored.composite(f), varied.composite(f));
    }
    formats::save(&input, &sprite).unwrap();
    let error = automation::operation("batch",&json!({"dry_run":true,"steps":[{
        "operation":"export","arguments":{"input":input,"output":root.join("small.png"),"format":"sheet",
          "options":{"layout":"vertical","width":2,"height":2,"padding":0}}
    }]})).unwrap_err();
    assert!(error.contains("fit"), "{error}");
    assert!(!root.join("small.png").exists());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn raster_interchange_exports_real_formats_and_preserves_supported_alpha() {
    let workspace = Workspace::new();
    let root = workspace.project();
    let source = root.join("art.pixel");
    let mut sprite = Document::new(2, 1);
    let ink = [13, 185, 71, 117];
    sprite.set_pixel(0, 0, 0, ink);
    sprite.set_pixel(0, 0, 1, [120, 20, 70, 255]);
    formats::save(&source, &sprite).unwrap();
    for (kind, signature) in [
        ("ico", b"\0\0\x01\0".as_slice()),
        ("qoi", b"qoif".as_slice()),
        ("tga", b"".as_slice()),
        ("webp", b"RIFF".as_slice()),
    ] {
        let filename = format!("sprite.{kind}");
        let reply = workspace.call(
            "export",
            json!({"input":"art.pixel","output":filename,"format":kind,"scale":2}),
        );
        assert!(reply["isError"].is_null(), "{reply}");
        let image = root.join(&filename);
        assert!(fs::read(&image).unwrap().starts_with(signature));
        if kind == "webp" {
            assert_eq!(&fs::read(&image).unwrap()[8..12], b"WEBP");
        }
        let project = root.join(format!("{kind}.pixel"));
        automation::run_cli(&[
            "import".into(),
            json!({"input":image,"output":project}).to_string(),
        ])
        .unwrap();
        let decoded = formats::load(&project).unwrap();
        assert_eq!((decoded.width, decoded.height), (4, 2));
        assert_eq!(
            decoded.composite(0),
            [ink, ink, [120, 20, 70, 255], [120, 20, 70, 255]].repeat(2),
            "{kind}"
        );
    }
    let live_path = root.join("live-crop.qoi");
    let request = json!({
        "output":live_path,"format":"qoi","scale":2,
        "region":{"x":0,"y":0,"width":1,"height":1}
    });
    automation::prepare_live_export(&sprite, 0, &request, "test-revision")
        .unwrap()
        .write()
        .unwrap();
    let live = formats::load(&live_path).unwrap();
    assert_eq!((live.width, live.height), (2, 2));
    assert_eq!(live.composite(0), vec![ink; 4]);

    let reply = workspace.call(
        "export",
        json!({"input":"art.pixel","output":"oversized.ico","scale":200}),
    );
    assert_error(&reply, "256");
    assert!(!root.join("oversized.ico").exists());

    formats::save(&source, &Document::new(8, 8)).unwrap();
    let reply = workspace.call(
        "export",
        json!({"input":"art.pixel","output":"sprite.jpg","format":"jpeg"}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let jpeg = root.join("sprite.jpg");
    assert!(fs::read(&jpeg).unwrap().starts_with(b"\xff\xd8"));
    assert_eq!(
        formats::load(&jpeg).unwrap().composite(0)[0],
        [255, 255, 255, 255]
    );
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn pcx_interchange_keeps_palette_slots_and_checks_scanline_packets() {
    use pixelcraft::model::ColorMode;

    let w = Workspace::new();
    let root = w.project();
    let mut header = vec![0u8; 128];
    header[..4].copy_from_slice(&[10, 5, 1, 8]);
    header[4..6].copy_from_slice(&7u16.to_le_bytes());
    header[8..10].copy_from_slice(&9u16.to_le_bytes());
    header[65] = 1;
    header[66] = 4;
    header[68] = 1;
    let mut bytes = header.clone();
    bytes.extend_from_slice(&[0, 5, 6, 0, 12]);
    let mut palette = [[0; 3]; 256];
    palette[0] = [191, 65, 13];
    palette[5] = [202, 150, 88];
    palette[6] = palette[5];
    bytes.extend(palette.into_iter().flatten());
    fs::write(root.join("indexed.pcx"), &bytes).unwrap();
    let loaded = formats::load(&root.join("indexed.pcx")).unwrap();
    assert_eq!((loaded.width, loaded.height), (3, 1));
    assert_eq!(loaded.color_mode, ColorMode::Indexed);
    assert_eq!(
        loaded.layers[0].cel_meta[0]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        [0, 5, 6]
    );
    assert_eq!(
        loaded.composite(0),
        vec![[191, 65, 13, 255], [202, 150, 88, 255], [202, 150, 88, 255]]
    );
    assert!(loaded.layers[0].background);
    let reply = w.call(
        "export",
        json!({"input":"indexed.pcx","output":"scaled.pcx","format":"pcx","scale":2}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let bytes2 = fs::read(root.join("scaled.pcx")).unwrap();
    assert_eq!(bytes2[65], 1);
    let scaled = formats::load(&root.join("scaled.pcx")).unwrap();
    assert_eq!((scaled.width, scaled.height), (6, 2));
    assert_eq!(
        scaled.layers[0].cel_meta[0]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        [0, 0, 5, 5, 6, 6].repeat(2)
    );

    let mut rgb = Document::new(3, 1);
    rgb.set_pixel(0, 0, 0, [240, 10, 70, 255]);
    rgb.set_pixel(0, 0, 1, [60, 180, 120, 128]);
    formats::save(&root.join("art.pixel"), &rgb).unwrap();
    let reply = w.call(
        "export",
        json!({"input":"art.pixel","output":"rgb.pcx","format":"pcx"}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let rgb_bytes = fs::read(root.join("rgb.pcx")).unwrap();
    assert_eq!(rgb_bytes[65], 3);
    assert_eq!(rgb_bytes[66], 4);
    assert_eq!(
        formats::load(&root.join("rgb.pcx")).unwrap().composite(0),
        vec![
            [240, 10, 70, 255],
            [157, 217, 187, 255],
            [255, 255, 255, 255]
        ]
    );

    let mut planar = header;
    planar[2] = 0;
    planar[3] = 1;
    planar[65] = 4;
    planar[66] = 2;
    planar[16 + 3..16 + 6].copy_from_slice(&[15, 25, 35]);
    planar[16 + 6..16 + 9].copy_from_slice(&[75, 85, 95]);
    planar[16 + 12..16 + 15].copy_from_slice(&[110, 120, 130]);
    planar.extend_from_slice(&[0b1000_0000, 0, 0b0100_0000, 0, 0b0010_0000, 0, 0, 0]);
    fs::write(root.join("planar.pcx"), planar).unwrap();
    let planar = formats::load(&root.join("planar.pcx")).unwrap();
    assert_eq!(
        planar.layers[0].cel_meta[0]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        [1, 2, 4]
    );
    assert_eq!(
        planar.composite(0),
        vec![[15, 25, 35, 255], [75, 85, 95, 255], [110, 120, 130, 255]]
    );

    let live_path = root.join("live.pcx");
    let args =
        json!({"output":live_path,"format":"pcx","region":{"x":0,"y":0,"width":1,"height":1}});
    automation::prepare_live_export(&rgb, 0, &args, "pcx-check")
        .unwrap()
        .write()
        .unwrap();
    assert_eq!(
        formats::load(&live_path).unwrap().composite(0),
        vec![[240, 10, 70, 255]]
    );

    let mut invalid = bytes;
    invalid[128] = 0xc5;
    fs::write(root.join("broken.pcx"), &invalid).unwrap();
    assert!(
        formats::load(&root.join("broken.pcx"))
            .unwrap_err()
            .contains("scanline")
    );
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn svg_and_css_export_scaled_rectangles_with_alpha() {
    use pixelcraft::model::CLEAR;
    let workspace = Workspace::new();
    let root = workspace.project();
    let mut sprite = Document::new(4, 3);
    let solid = [20, 40, 60, 255];
    let translucent = [170, 25, 45, 128];
    for index in [0, 1, 4, 5] {
        sprite.set_pixel(0, 0, index, solid);
    }
    sprite.set_pixel(0, 0, 11, translucent);
    let path = root.join("art.pixel");
    formats::save(&path, &sprite).unwrap();
    let exported = workspace.call(
        "export",
        json!({"input":"art.pixel","output":"art.svg","format":"svg","scale":2}),
    );
    assert!(exported["isError"].is_null(), "{exported}");
    let svg = fs::read_to_string(root.join("art.svg")).unwrap();
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let node = xml.root_element();
    assert_eq!(node.attribute("viewBox"), Some("0 0 8 6"));
    assert_eq!(node.attribute("shape-rendering"), Some("crispEdges"));
    let mut pixels = [CLEAR; 8 * 6];
    let rects: Vec<_> = node.children().filter(|n| n.has_tag_name("rect")).collect();
    assert_eq!(
        rects.len(),
        2,
        "adjacent equal pixels should share a rectangle"
    );
    for rect in rects {
        let position = |name| rect.attribute(name).unwrap().parse::<usize>().unwrap();
        let color = rect.attribute("fill").unwrap().trim_start_matches('#');
        let channel = |start| u8::from_str_radix(&color[start..start + 2], 16).unwrap();
        let alpha = rect.attribute("fill-opacity").map_or(255, |alpha| {
            (alpha.parse::<f64>().unwrap() * 255.0).round() as u8
        });
        for y in position("y")..position("y") + position("height") {
            for x in position("x")..position("x") + position("width") {
                assert_eq!(pixels[y * 8 + x], CLEAR);
                pixels[y * 8 + x] = [channel(0), channel(2), channel(4), alpha];
            }
        }
    }
    let expected = sprite.composite(0);
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(pixels[y * 8 + x], expected[y / 2 * 4 + x / 2]);
        }
    }
    let css_path = root.join("crop.css");
    automation::prepare_live_export(
        &sprite,
        0,
        &json!({
            "output":css_path,"format":"css","scale":3,"region":{"x":2,"y":1,"width":2,"height":2}
        }),
        "test",
    )
    .unwrap()
    .write()
    .unwrap();
    let css = fs::read_to_string(css_path).unwrap();
    assert!(css.contains(".pixel-art"));
    assert!(css.contains("width: 6px") && css.contains("height: 6px"));
    assert!(css.contains("linear-gradient(#aa192d80, #aa192d80) 3px 3px / 3px 3px no-repeat"));
    assert_eq!(css.matches("linear-gradient").count(), 1);

    let blank = Document::new(1, 1);
    formats::save(&path, &blank).unwrap();
    let reply = workspace.call("export", json!({"input":"art.pixel","output":"empty.css"}));
    assert!(reply["isError"].is_null(), "{reply}");
    let css = fs::read_to_string(root.join("empty.css")).unwrap();
    assert!(css.contains("background: transparent;") && !css.contains("linear-gradient"));
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn flic_import_keeps_indexed_deltas_palettes_and_timing() {
    use pixelcraft::model::ColorMode;
    fn chunk(kind: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(body.len() as u32 + 6).to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(body);
        out
    }
    fn frame(delay: u16, chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0; 16];
        out[4..6].copy_from_slice(&0xf1fau16.to_le_bytes());
        out[6..8].copy_from_slice(&(chunks.len() as u16).to_le_bytes());
        out[8..10].copy_from_slice(&delay.to_le_bytes());
        out.extend(chunks.iter().flatten().copied());
        let size = out.len() as u32;
        out[..4].copy_from_slice(&size.to_le_bytes());
        out
    }
    fn file(
        kind: u16,
        count: u16,
        width: u16,
        height: u16,
        speed: u16,
        payload: &[Vec<u8>],
    ) -> Vec<u8> {
        let mut out = vec![0; 128];
        for (offset, value) in [
            (4, kind),
            (6, count),
            (8, width),
            (10, height),
            (12, 8),
            (16, speed),
        ] {
            out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        out.extend(payload.iter().flatten().copied());
        let size = out.len() as u32;
        out[..4].copy_from_slice(&size.to_le_bytes());
        out
    }

    let w = Workspace::new();
    let root = w.project();
    let black = [0, 0, 0, 255];
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let green = [0, 255, 0, 255];
    let frame0 = frame(
        0,
        &[
            chunk(4, &[1, 0, 0, 3, 0, 0, 0, 255, 0, 0, 0, 0, 255]),
            chunk(15, &[1, 3, 1, 1, 253, 0, 1, 0]),
        ],
    );
    let frame1 = frame(
        120,
        &[
            chunk(4, &[1, 0, 1, 1, 0, 255, 0]),
            // SS2 skips the first row, writes its final pixel, then copies a pair.
            chunk(7, &[1, 0, 255, 255, 2, 128, 1, 0, 0, 1, 2, 0]),
        ],
    );
    let flc = file(
        0xaf12,
        2,
        3,
        2,
        75,
        &[
            chunk(0xf100, &[0; 10]),
            frame0,
            frame1,
            frame(0, &[chunk(13, &[])]), // ring frame is outside the authored timeline
        ],
    );
    fs::write(root.join("demo.flc"), flc).unwrap();
    let reply = w.call("import", json!({"input":"demo.flc","output":"demo.pixel"}));
    assert!(reply["isError"].is_null(), "{reply}");
    let loaded = formats::load(&root.join("demo.pixel")).unwrap();
    assert_eq!(loaded.color_mode, ColorMode::Indexed);
    assert_eq!(loaded.durations, [75, 120]);
    assert_eq!(loaded.composite(0), [red, red, red, black, red, black]);
    assert_eq!(
        loaded.composite(1),
        [green, green, green, blue, black, blue]
    );
    assert_eq!(
        loaded.layers[0].cel_meta[1]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        [1, 1, 1, 2, 0, 2]
    );

    let initial = frame(
        0,
        &[
            chunk(11, &[1, 0, 0, 2, 0, 0, 0, 63, 0, 0]),
            chunk(16, &[0, 1, 0]),
        ],
    );
    let next = frame(0, &[chunk(12, &[0, 0, 1, 0, 1, 0, 254, 1])]);
    let fli = file(0xaf11, 2, 3, 1, 5, &[initial, next]);
    fs::write(root.join("demo.fli"), fli).unwrap();
    let result = formats::load(&root.join("demo.fli")).unwrap();
    assert_eq!(result.durations, [71, 71]);
    assert_eq!(result.composite(1), [red, red, black]);

    let invalid = file(0xaf12, 1, 2, 1, 75, &[frame(0, &[chunk(15, &[1, 3, 0])])]);
    fs::write(root.join("bad.flc"), invalid).unwrap();
    let reply = w.call("import", json!({"input":"bad.flc","output":"bad.pixel"}));
    assert_error(&reply, "scanline");
    assert!(!root.join("bad.pixel").exists());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn flic_exports_authored_palettes_renders_and_timing() {
    use pixelcraft::model::ColorMode;
    let w = Workspace::new();
    let root = w.project();
    let path = root.join("art.pixel");
    let mut sprite = Document::new(3, 2);
    sprite.add_frame(0, false);
    sprite.durations = vec![70, 125];
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let white = [255; 4];
    sprite.color_mode = ColorMode::Indexed;
    sprite.layers[0].background = true;
    sprite.palette = vec![white, red, red];
    sprite.frame_palettes = vec![sprite.palette.clone(), vec![white, blue, blue]];
    for (frame, color) in [red, blue].into_iter().enumerate() {
        for (pixel, index) in [0, 1, 2, 2, 1, 0].into_iter().enumerate() {
            sprite.layers[0].cels[frame].set(pixel, if index == 0 { white } else { color });
        }
        sprite.layers[0].cel_meta[frame].indices = Some(vec![0, 1, 2, 2, 1, 0].into());
    }
    formats::save(&path, &sprite).unwrap();
    let reply = w.call(
        "export",
        json!({"input":"art.pixel","output":"scaled.flc","format":"flc","scale":2}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let exported = root.join("scaled.flc");
    let bytes = fs::read(&exported).unwrap();
    assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 0xaf12);
    assert_eq!(
        u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
        bytes.len()
    );
    let loaded = formats::load(&exported).unwrap();
    assert_eq!(loaded.durations, [70, 125]);
    assert_eq!((loaded.width, loaded.height), (6, 4));
    assert_eq!(loaded.palette_for(0)[1], red);
    assert_eq!(loaded.palette_for(1)[2], blue);
    assert_eq!(
        &loaded.layers[0].cel_meta[0].indices.as_deref().unwrap()[..6],
        &[0, 0, 1, 1, 2, 2]
    );

    let reply = w.call(
        "export",
        json!({"input":"art.pixel","output":"bad.fli","format":"fli"}),
    );
    assert_error(&reply, "320");
    assert!(!root.join("bad.fli").exists());

    let mut rgb = Document::new(17, 17);
    for y in 0..17 {
        for x in 0..17 {
            rgb.set_pixel(0, 0, y * 17 + x, [x as u8 * 15, y as u8 * 15, 41, 255]);
        }
    }
    formats::save(&path, &rgb).unwrap();
    let reply = w.call(
        "export",
        json!({"input":"art.pixel","output":"quantized.flc","format":"flc"}),
    );
    assert!(reply["isError"].is_null(), "{reply}");
    let palette_output = formats::load(&root.join("quantized.flc")).unwrap();
    assert_eq!(palette_output.palette.len(), 256);
    let error: usize = rgb
        .composite(0)
        .iter()
        .zip(palette_output.composite(0))
        .map(|(a, b)| {
            a.iter()
                .zip(b)
                .map(|(a, b)| usize::from(a.abs_diff(b)))
                .sum::<usize>()
        })
        .sum();
    assert!(
        error < 30 * 17 * 17,
        "unexpected error after color quantization: {error}"
    );

    let mut classic = Document::new(320, 200);
    classic.add_frame(0, false);
    classic.durations = vec![100, 200];
    classic.set_pixel(0, 0, 0, red);
    classic.set_pixel(0, 1, 0, blue);
    formats::save(&path, &classic).unwrap();
    let output = root.join("classic.fli");
    let args = json!({"output":output,"format":"fli"});
    automation::prepare_live_export(&classic, 0, &args, "flic-test")
        .unwrap()
        .write()
        .unwrap();
    let classic = formats::load(&output).unwrap();
    assert_eq!(classic.durations, [100, 100, 100]);
    assert_eq!(classic.composite(0)[..2], [red, white]);
    assert_eq!(classic.composite(1)[0], blue);
    assert_eq!(classic.composite(2)[0], blue);
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
