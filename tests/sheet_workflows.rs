use image::{Rgba, RgbaImage};
use pixelcraft::{
    atlas::{AtlasLayout, AtlasOptions, pack_atlas, save_packed_atlas},
    automation::{apply_commands, document_revision, rpc, run_cli},
    formats::{decode_project, encode_project, load},
    model::{DEFAULT_DURATION, Document},
    productivity::{GridImport, preview_grid},
    workflows::{SheetOptions, pack_sheet, unpack_sheet},
};
use serde_json::json;

#[test]
fn frame_renames_roundtrip_without_changing_export_keys() {
    let mut doc = Document::new(1, 1);
    doc.add_frame(0, false);
    doc.add_frame(0, false); // The second position now has export ID 2.
    let mut doc = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"set-frame","frame":1,"name":"Wind-up","duration":75},
            {"op":"set-frame","frame":2,"name":"Follow through","duration":90}
        ]}),
    )
    .unwrap();
    let (_, initial_atlas) = pack_atlas(
        &[("walk.pixel".into(), doc.clone())],
        &AtlasOptions::default(),
    )
    .unwrap();
    doc.remove_frame(0);
    doc.add_frame(0, false);
    let doc = decode_project(&encode_project(&doc).unwrap()).unwrap();

    let (sheet, meta) = pack_sheet(&doc, &SheetOptions::default()).unwrap();
    let frames = meta["frames"].as_array().unwrap();
    assert_eq!(frames[0]["frameName"], "Wind-up");
    assert_eq!(frames[0]["filename"], "frame_2");
    assert_eq!(frames[0]["frameId"], 2);
    assert!(frames[1].get("frameName").is_none());
    assert_eq!(frames[2]["frameName"], "Follow through");
    assert_eq!(frames[2]["filename"], "frame_1");
    let from_sheet = unpack_sheet(&sheet, &meta).unwrap();
    let (_, reexport) = pack_sheet(&from_sheet, &SheetOptions::default()).unwrap();
    assert_eq!(reexport["frames"], meta["frames"]);

    let (_, atlas) = pack_atlas(&[("walk.pixel".into(), doc)], &AtlasOptions::default()).unwrap();
    assert_eq!(atlas["frames"][0]["filename"], "0/walk.pixel/2");
    assert_eq!(
        atlas["frames"][0]["filename"],
        initial_atlas["frames"][1]["filename"]
    );
    assert_eq!(atlas["frames"][0]["frameId"], 2);
    assert_eq!(atlas["frames"][0]["sourceFrame"], 0);
    assert_eq!(atlas["frames"][0]["frameName"], "Wind-up");
    assert!(atlas["frames"][1].get("frameName").is_none());
    assert_eq!(atlas["frames"][2]["frameName"], "Follow through");
}

#[test]
fn headless_grid_import_skips_alpha_zero_cells_and_initializes_duration() {
    let root = std::env::temp_dir().join(format!("pixelcraft-grid-options-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("grid.png");
    let output = root.join("out.pixel");
    let mut image = RgbaImage::new(5, 1);
    image.put_pixel(0, 0, Rgba([100, 120, 130, 0])); // Hidden RGB should not keep the cell.
    image.put_pixel(2, 0, Rgba([10, 20, 30, 255]));
    image.put_pixel(4, 0, Rgba([40, 50, 60, 1])); // Any nonzero alpha keeps the cell.
    image.save(&input).unwrap();
    let preview = preview_grid(
        &image,
        &GridImport {
            width: 1,
            height: 1,
            spacing_x: 1,
            ..Default::default()
        },
        true,
    )
    .unwrap();
    assert_eq!(preview.cells, vec![(2, 0), (4, 0)]);
    assert_eq!(preview.skipped, 1);

    let args = json!({"input":input,"output":output,"options":{"width":1,"height":1,"spacing_x":1,"skip_transparent":true,"duration":175}});
    run_cli(&["import-grid".into(), args.to_string()]).unwrap();
    let doc = load(&output).unwrap();
    assert_eq!(doc.durations, vec![175, 175]);
    assert_eq!(doc.layers[0].cels[0][0], [10, 20, 30, 255]);
    assert_eq!(doc.layers[0].cels[1][0], [40, 50, 60, 1]);

    let mut layers = args.clone();
    layers["overwrite"] = json!(true);
    layers["options"]["layers"] = json!(true);
    let result = rpc(
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"import-grid","arguments":layers}}),
        None,
    )
    .unwrap();
    assert_ne!(result["result"]["isError"], true);
    let doc = load(&output).unwrap();
    assert_eq!(doc.durations, vec![175]);
    assert_eq!(doc.layers.len(), 2);

    let mut defaults = args;
    defaults["options"] = json!({"width":1,"height":1,"spacing_x":1});
    run_cli(&["import-grid".into(), defaults.to_string()]).unwrap();
    assert_eq!(load(&output).unwrap().durations, vec![DEFAULT_DURATION; 3]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn saving_atlas_preserves_previewed_pixels_and_placements() {
    let root =
        std::env::temp_dir().join(format!("pixelcraft-atlas-preview-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let output = root.join("preview.png");
    let mut doc = Document::new(2, 1);
    doc.set_pixel(0, 0, 0, [255, 100, 5, 255]);
    doc.set_pixel(0, 0, 1, [255, 100, 5, 255]);
    doc.add_frame(0, true);
    let packed = pack_atlas(
        &[("sprite".into(), doc)],
        &AtlasOptions {
            layout: AtlasLayout::Rows,
            power_of_two: true,
            width: 5,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(packed.0.dimensions(), (4, 4));
    assert_eq!(
        packed.1["frames"][0]["frame"],
        json!({"x":1,"y":1,"w":2,"h":1})
    );
    assert_eq!(
        packed.1["frames"][1]["frame"],
        packed.1["frames"][0]["frame"]
    );
    assert!(!output.exists());

    save_packed_atlas(&output, &packed).unwrap();
    assert_eq!(image::open(&output).unwrap().into_rgba8(), packed.0);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.with_extension("json")).unwrap()).unwrap();
    assert_eq!(json["meta"]["image"], "preview.png");
    assert_eq!(json["meta"]["size"], packed.1["meta"]["size"]);
    assert_eq!(json["frames"], packed.1["frames"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn single_source_atlas_roundtrip_does_not_reuse_retired_frame_ids() {
    let mut doc = Document::new(1, 1);
    doc.add_frame(0, false);
    let retired = doc.frame_export_id(1);
    doc.remove_frame(1);
    let (image, metadata) =
        pack_atlas(&[("sprite.pixel".into(), doc)], &AtlasOptions::default()).unwrap();

    let mut reopened = unpack_sheet(&image, &metadata).unwrap();
    assert_eq!(reopened.frame_export_id(0), 0);
    reopened.add_frame(0, false);
    assert_eq!(reopened.frame_export_id(1), retired + 1);
}

#[test]
fn multi_source_atlas_reopens_with_distinct_frame_identities() {
    let mut first = Document::new(1, 1);
    first.set_frame_name(0, "Takeoff").unwrap();
    let mut second = Document::new(1, 1);
    second.set_frame_name(0, "Landing").unwrap();

    let (image, metadata) = pack_atlas(
        &[
            ("first.pixel".into(), first),
            ("second.pixel".into(), second),
        ],
        &AtlasOptions::default(),
    )
    .unwrap();
    assert_eq!(metadata["frames"][0]["frameId"], 0);
    assert_eq!(metadata["frames"][1]["frameId"], 0);

    let reopened = unpack_sheet(&image, &metadata).unwrap();
    assert_ne!(reopened.frame_export_id(0), reopened.frame_export_id(1));
    assert_eq!(reopened.frame_name(0), Some("Takeoff"));
    assert_eq!(reopened.frame_name(1), Some("Landing"));
}

#[test]
fn named_frames_require_native_projects_for_save_but_allow_ase_export() {
    let root = std::env::temp_dir().join(format!("pixelcraft-named-ase-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let native = root.join("named.pixel");
    let ase = root.join("copy.aseprite");
    let mut doc = Document::new(1, 1);
    doc.set_frame_name(0, "Landing").unwrap();
    pixelcraft::formats::save(&native, &doc).unwrap();

    assert!(pixelcraft::formats::save(&ase, &doc).is_err());
    assert!(!ase.exists());
    run_cli(&[
        "export".into(),
        json!({"input":native,"output":ase,"format":"aseprite"}).to_string(),
    ])
    .unwrap();
    assert_eq!(load(&ase).unwrap().width, 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn lua_frame_insertions_keep_authored_names_on_their_frames() {
    let mut doc = Document::new(1, 1);
    doc.add_frame(0, false);
    doc.set_frame_name(0, "Takeoff").unwrap();
    doc.set_frame_name(1, "Landing").unwrap();
    let updated = pixelcraft::extensions::run_source(
        &doc,
        "local s=app.activeSprite; s:newEmptyFrame(2); s:newFrame(1)",
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(updated.frame_name(0), Some("Takeoff"));
    assert_eq!(updated.frame_name(1), Some("Takeoff"));
    assert_eq!(updated.frame_name(2), None);
    assert_eq!(updated.frame_name(3), Some("Landing"));
}
