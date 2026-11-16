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
fn imported_hash_sheet_preserves_serialized_frame_order_and_tags() {
    let metadata = serde_json::from_str(r#"{
        "frames": {
            "walk 1":  {"frame":{"x":0,"y":0,"w":1,"h":1},"sourceSize":{"w":1,"h":1},"duration":101},
            "walk 2":  {"frame":{"x":1,"y":0,"w":1,"h":1},"sourceSize":{"w":1,"h":1},"duration":102},
            "walk 10": {"frame":{"x":2,"y":0,"w":1,"h":1},"sourceSize":{"w":1,"h":1},"duration":110}
        },
        "meta":{"frameTags":[{"name":"pair","from":0,"to":1}]}
    }"#).unwrap();
    let image = RgbaImage::from_fn(3, 1, |x, _| Rgba([x as u8, 0, 0, 255]));
    let sprite = unpack_sheet(&image, &metadata).unwrap();

    assert_eq!(sprite.durations, [101, 102, 110]);
    assert_eq!((sprite.tags[0].from, sprite.tags[0].to), (0, 1));
    assert_eq!(sprite.layers[0].cels[1][0], [1, 0, 0, 255]);
}

#[test]
fn imported_sheet_accepts_string_and_numeric_repeats() {
    let frame = json!({"frame":{"x":0,"y":0,"w":1,"h":1},"sourceSize":{"w":1,"h":1}});
    let meta = json!({"frames":[frame],"meta":{"frameTags":[
        {"name":"string","from":0,"to":0,"repeat":"3"},
        {"name":"number","from":0,"to":0,"repeat":4},
        {"name":"omitted","from":0,"to":0}
    ]}});
    let sprite = unpack_sheet(&RgbaImage::new(1, 1), &meta).unwrap();
    assert_eq!(
        sprite.tags.iter().map(|tag| tag.repeat).collect::<Vec<_>>(),
        [3, 4, 0]
    );
}

#[test]
fn imported_sheet_accepts_named_slice_coordinates() {
    let frame = json!({"frame":{"x":0,"y":0,"w":4,"h":3},"sourceSize":{"w":4,"h":3}});
    let meta = json!({"frames":[frame],"meta":{"slices":[
        {"name":"character","keys":[{"frame":0,"bounds":{"x":0,"y":0,"w":4,"h":3},
            "center":{"x":1,"y":1,"w":2,"h":1},"pivot":{"x":2,"y":-1}}]},
        {"name":"native","keys":[{"frame":0,"bounds":[0,0,4,3],"pivot":[2,1]}]}
    ]}});
    let sprite = unpack_sheet(&RgbaImage::new(4, 3), &meta).unwrap();
    let key = &sprite.slices[0].keys[0];
    assert_eq!(key.bounds, [0, 0, 4, 3]);
    assert_eq!(key.center, Some([1, 1, 2, 1]));
    assert_eq!(key.pivot, Some([2, -1]));
    assert_eq!(sprite.slices[1].keys[0].pivot, Some([2, 1]));
}

#[test]
fn oversized_hash_sheet_is_rejected_before_decoding_frame_entries() {
    let frames: serde_json::Map<_, _> = (0..=u16::MAX)
        .map(|index| (format!("frame_{index}"), serde_json::Value::Null))
        .collect();
    let metadata = json!({"frames":frames,"meta":{"app":"PixelCraft","sources":[]}});
    let error = unpack_sheet(&RgbaImage::new(1, 1), &metadata).expect_err("reject excess frames");
    assert_eq!(error, "Sheet exceeds limits");
}

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
fn grid_import_count_order_and_partial_cells_match_between_cli_and_mcp() {
    let root = std::env::temp_dir().join(format!("pixelcraft-grid-counts-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("source.png");
    let output = root.join("out.pixel");
    let mut image = RgbaImage::new(7, 5);
    for (x, y, color) in [
        (1, 0, [10, 0, 0, 255]),
        (1, 3, [20, 0, 0, 255]),
        (5, 0, [30, 0, 0, 255]),
        (5, 3, [40, 0, 0, 255]),
        (6, 4, [50, 0, 0, 255]),
    ] {
        image.put_pixel(x, y, Rgba(color));
    }
    image.save(&input).unwrap();
    let args = json!({"input":input,"output":output,"options":{
        "columns":2,"rows":2,"offset_x":1,"spacing_x":1,
        "order":"column_major","include_partial":true,"duration":130
    }});
    let settings = serde_json::from_value(args["options"].clone()).unwrap();
    let preview = pixelcraft::productivity::preview_grid_with_options(&image, &settings).unwrap();
    assert_eq!(preview.cell_size, [3, 3]);
    assert_eq!(preview.cells, [(1, 0), (1, 3), (5, 0), (5, 3)]);
    run_cli(&["import-grid".into(), args.to_string()]).unwrap();
    let doc = load(&output).unwrap();
    assert_eq!([doc.width, doc.height], [3, 3]);
    assert_eq!(doc.durations, [130; 4]);
    assert_eq!(
        (0..4)
            .map(|frame| doc.composite(frame)[0][0])
            .collect::<Vec<_>>(),
        [10, 20, 30, 40]
    );
    assert_eq!(doc.composite(3)[4], [50, 0, 0, 255]);
    assert_eq!(doc.composite(3)[8], [0; 4]);
    let mut layers = args.clone();
    layers["output"] = json!(root.join("layers.pixel"));
    layers["options"]["layers"] = json!(true);
    let response = rpc(
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"import-grid","arguments":layers}}),
        None,
    ).unwrap();
    assert_ne!(response["result"]["isError"], true, "{response}");
    let imported = load(&root.join("layers.pixel")).unwrap();
    assert_eq!(
        imported
            .layers
            .iter()
            .map(|layer| layer.cels[0][0][0])
            .collect::<Vec<_>>(),
        [10, 20, 30, 40]
    );
    let mut no_partial = args;
    no_partial["output"] = json!(root.join("whole.pixel"));
    no_partial["options"]["include_partial"] = json!(false);
    run_cli(&["import-grid".into(), no_partial.to_string()]).unwrap();
    let whole = load(&root.join("whole.pixel")).unwrap();
    assert_eq!([whole.width, whole.height], [2, 2]);
    assert_eq!(whole.durations, [130; 4]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn grid_import_keeps_indexed_png_slots_and_pads_partial_cells() {
    use pixelcraft::model::ColorMode;
    let root = std::env::temp_dir().join(format!("pixelcraft-grid-indices-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("indexed.png");
    let output = root.join("out.pixel");
    let mut encoder = png::Encoder::new(std::fs::File::create(&input).unwrap(), 3, 1);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_palette(vec![30, 100, 80, 30, 100, 80]);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[1, 0, 1]).unwrap();
    writer.finish().unwrap();
    run_cli(&[
        "import-grid".into(),
        json!({
            "input":input,"output":output,"options":{"width":2,"height":1,"include_partial":true}
        })
        .to_string(),
    ])
    .unwrap();
    let doc = load(&output).unwrap();
    assert_eq!(doc.color_mode, ColorMode::Indexed);
    assert_eq!(doc.transparent_index, 2);
    assert_eq!(
        doc.layers[0].cel_meta[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        [1, 0]
    );
    assert_eq!(
        doc.layers[0].cel_meta[1]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        [1, 2]
    );
    assert_eq!(doc.composite(1), [[30, 100, 80, 255], [0; 4]]);
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

#[test]
fn grid_suggestion_is_limited_to_divisible_square_cell_strips() {
    assert_eq!(GridImport::suggest_strip(96, 24), Some((24, 24)));
    assert_eq!(GridImport::suggest_strip(20, 60), Some((20, 20)));
    assert_eq!(GridImport::suggest_strip(99, 24), None);
    assert_eq!(GridImport::suggest_strip(32, 32), None);
}

#[test]
fn filename_keyed_atlas_reopens_in_authored_frame_order() {
    use pixelcraft::atlas::{AtlasMetadata, save_packed_atlas_with_metadata};
    let mut doc = Document::new(1, 1);
    doc.add_frame(0, false);
    doc.add_frame(0, false);
    doc.durations = vec![100, 200, 300];
    doc.set_frame_name(1, "Second").unwrap();
    let packed = pack_atlas(&[("walk.pixel".into(), doc)], &AtlasOptions::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("pixelcraft-hash-atlas-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ordered.png");
    save_packed_atlas_with_metadata(&path, &packed, AtlasMetadata::Hash).unwrap();
    let reopened = load(&path.with_extension("json")).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(reopened.durations, vec![100, 200, 300]);
    assert_eq!(reopened.frame_name(1), Some("Second"));
}
