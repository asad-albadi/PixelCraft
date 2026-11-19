use pixelcraft::{model::*, workflows::*};
#[test]
fn transforms_only_selected_pixels_and_preserves_neighbors() {
    let mut d = Document::new(4, 3);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    d.set_pixel(0, 0, 1, [0, 255, 0, 255]);
    d.set_pixel(0, 0, 11, [0, 0, 255, 255]);
    transform(&mut d, 0, 0, [0, 0, 2, 1], None, Transform::FlipX).unwrap();
    assert_eq!(d.layers[0].cels[0][1], [255, 0, 0, 255]);
    assert_eq!(d.layers[0].cels[0][11], [0, 0, 255, 255]);
}
#[test]
fn sheet_metadata_roundtrips_frames_tags_and_padding() {
    let mut d = Document::new(3, 2);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    d.add_frame(0, false);
    d.set_pixel(0, 1, 5, [0, 255, 0, 255]);
    d.durations = vec![70, 230];
    d.tags.push(FrameTag {
        editor_id: Default::default(),
        name: "walk".into(),
        from: 0,
        to: 1,
        direction: 2,
        repeat: 0,
        color: [0, 0, 0],
    });
    let (im, meta) = pack_sheet(
        &d,
        &SheetOptions {
            columns: 2,
            padding: 2,
            trim: true,
            ..Default::default()
        },
    )
    .unwrap();
    let restored = unpack_sheet(&im, &meta).unwrap();
    assert_eq!(restored.composite(0), d.composite(0));
    assert_eq!(restored.composite(1), d.composite(1));
    assert_eq!(restored.durations, d.durations);
    assert_eq!(restored.tags[0].name, "walk");
}
#[test]
fn lasso_excludes_pixels_outside_polygon() {
    assert!(inside_polygon(&[(0, 0), (4, 0), (0, 4)], 1, 1));
    assert!(!inside_polygon(&[(0, 0), (4, 0), (0, 4)], 3, 3));
}
#[test]
fn crop_changes_all_frames_and_keeps_layer_names() {
    let mut d = Document::new(4, 3);
    d.layers[0].name = "Ink".into();
    d.set_pixel(0, 0, 5, [1, 2, 3, 255]);
    crop(&mut d, [1, 1, 2, 2]).unwrap();
    assert_eq!((d.width, d.height), (2, 2));
    assert_eq!(d.layers[0].name, "Ink");
    assert_eq!(d.layers[0].cels[0][0], [1, 2, 3, 255]);
    d.validate().unwrap();
}
#[test]
fn ramp_interpolates_both_endpoints() {
    let r = ramp([0, 0, 0, 255], [255, 128, 64, 255], 3);
    assert_eq!(r[1], [128, 64, 32, 255]);
    assert_eq!(r[2], [255, 128, 64, 255]);
}
#[test]
fn tiled_export_import_preserves_flips_and_empty_tiles() {
    let root = std::env::temp_dir().join(format!("pixelcraft-tiled-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(4, 2);
    d.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Ground".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], CLEAR].into()],
    });
    d.add_tile_layer(0).unwrap();
    d.paint_tile(1, 0, 1, 0, 0x80000001).unwrap();
    pixelcraft::interchange::export_tiled(&root.join("map.tmj"), &d, 0).unwrap();
    let loaded = pixelcraft::formats::load(&root.join("map.tmj")).unwrap();
    assert_eq!(loaded.composite(0), d.composite(0));
    assert_eq!(loaded.layers[0].tile_cels[0], vec![0, 0x80000001]);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn gif_import_preserves_frame_timing_and_transparency() {
    let path = std::env::temp_dir().join(format!("pixelcraft-anim-{}.gif", std::process::id()));
    let mut d = Document::new(2, 2);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    d.add_frame(0, false);
    d.set_pixel(0, 1, 3, [0, 255, 0, 255]);
    d.durations = vec![70, 230];
    pixelcraft::formats::export_gif(&path, &d).unwrap();
    let loaded = pixelcraft::formats::load(&path).unwrap();
    assert_eq!(loaded.durations, d.durations);
    assert_eq!(loaded.composite(0), d.composite(0));
    assert_eq!(loaded.composite(1), d.composite(1));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn animated_png_and_webp_import_complete_canvases() {
    for name in ["animation.png", "animation.webp"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflows")
            .join(name);
        let d = pixelcraft::formats::load(&path).unwrap();
        assert_eq!((d.width, d.height), (3, 2));
        assert_eq!(d.durations, vec![70, 230], "{name}");
        assert_eq!(d.composite(0)[0], [255, 0, 0, 255]);
        assert_eq!(d.composite(1)[5], [0, 255, 0, 255]);
        assert_eq!(d.composite(1)[0], CLEAR);
    }
}
#[test]
fn openraster_keeps_layers_opacity_and_stacking() {
    let d = pixelcraft::formats::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflows/layers.ora"),
    )
    .unwrap();
    assert_eq!(d.layers.len(), 2);
    assert_eq!(d.layers[0].name, "Red");
    assert_eq!(d.layers[1].name, "Green");
    assert_eq!(d.layers[1].opacity, 128);
    assert_eq!(d.composite(0)[0], [255, 0, 0, 255]);
    assert_eq!(d.composite(0)[5], [0, 255, 0, 128]);
}
#[test]
fn openraster_import_without_a_writable_temp_directory() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/workflows/layers.ora");
    // A regular file cannot be used as an extraction directory.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
        .env("TMPDIR", &path)
        .env("TEMP", &path)
        .env("TMP", &path)
        .arg("--cli")
        .arg("inspect")
        .arg("--input")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["layers"][0]["name"], "Red");
    assert_eq!(report["layers"][1]["name"], "Green");
}

#[test]
fn engine_presets_write_resources_with_animation_timing() {
    let root = std::env::temp_dir().join(format!("pixelcraft-engines-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(2, 2);
    d.add_frame(0, false);
    d.durations = vec![70, 230];
    d.tags.push(FrameTag {
        editor_id: Default::default(),
        name: "walk".into(),
        from: 0,
        to: 1,
        direction: 2,
        repeat: 0,
        color: [0; 3],
    });
    for preset in ["Godot", "Unity"] {
        pixelcraft::interchange::export_sheet(
            &root.join("sprite.png"),
            &d,
            &SheetOptions {
                preset: preset.into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    let godot = std::fs::read_to_string(root.join("sprite.tres")).unwrap();
    assert!(godot.contains("SpriteFrames"));
    assert!(godot.contains("0.07"));
    assert!(godot.contains("walk"));
    let unity = std::fs::read_to_string(root.join("sprite.png.meta")).unwrap();
    assert!(unity.contains("spriteMode: 2"));
    assert!(unity.contains("name: frame_1"));
    assert_eq!(
        pixelcraft::formats::load(&root.join("sprite.json"))
            .unwrap()
            .durations,
        d.durations
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn transforms_retain_duplicate_indexed_palette_slots() {
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.drawing_index = Some(2);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    d.drawing_index = None;
    transform(&mut d, 0, 0, [0, 0, 2, 1], None, Transform::FlipX).unwrap();
    assert_eq!(d.layers[0].cel_meta[0].indices.as_ref().unwrap()[1], 2);
}

#[test]
fn free_rotation_keeps_indexed_slot_and_rotates_about_off_center_pivot() {
    let mut doc = Document::new(5, 5);
    let ink = [110, 95, 80, 255];
    doc.palette = vec![CLEAR, ink, ink];
    doc.set_color_mode(ColorMode::Indexed);
    doc.drawing_index = Some(2);
    doc.set_pixel(0, 0, 1, ink);
    let mut mask = vec![false; 25];
    mask[1] = true;
    let t = Transform::RotateAround {
        degrees: 90.,
        pivot: [0.5, 0.5],
    };
    transform(&mut doc, 0, 0, [0, 0, 2, 1], Some(&mask), t).unwrap();
    assert_eq!(doc.composite(0)[5], ink);
    assert_eq!(doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[5], 2);
    assert!(transform_mask(&doc, [0, 0, 2, 1], &mask, t)[5]);
}
#[test]
fn cropping_offset_linked_cels_preserves_both_frames_in_ase() {
    use pixelcraft::formats::{decode_ase, encode_ase};
    let mut doc = Document::new(6, 1);
    let red = [255, 0, 0, 255];
    doc.set_pixel(0, 0, 1, red);
    doc.layers[0].cel_meta[0].bounds = Some([1, 0, 1, 1]);
    doc.add_frame(0, false);
    doc.layers[0].cel_meta[1].bounds = Some([3, 0, 1, 1]);
    doc.link_cel(0, 1, 0).unwrap();

    crop(&mut doc, [1, 0, 4, 1]).unwrap();
    assert_eq!(doc.composite(0), vec![red, CLEAR, CLEAR, CLEAR]);
    assert_eq!(doc.composite(1), vec![CLEAR, CLEAR, red, CLEAR]);
    let reopened = decode_ase(&encode_ase(&doc).unwrap()).unwrap();
    assert_eq!(reopened.composite(0), doc.composite(0));
    assert_eq!(reopened.composite(1), doc.composite(1));
}

#[test]
fn canvas_size_can_hide_and_recover_indexed_artwork_and_tiles() {
    let red = [255, 0, 0, 255];
    let mut doc = Document::new(6, 2);
    doc.palette = vec![CLEAR, red, red];
    doc.set_color_mode(ColorMode::Indexed);
    doc.drawing_index = Some(2);
    doc.set_pixel(0, 0, 0, red);
    doc.set_pixel(0, 0, 5, red);
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();
    doc.drawing_index = None;
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Tiles".into(),
        width: 2,
        height: 1,
        tiles: vec![vec![red, CLEAR].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0][0] = 0x8000_0001;
    doc.layers[1].tile_cels[0][2] = 1;
    let original = doc.clone();

    resize_canvas_with_trim(&mut doc, [2, 2], [-2, 0], false).unwrap();
    assert_eq!(doc.composite(0), [CLEAR; 4]);
    assert_eq!(doc.layers[0].cel_meta[0].outside.len(), 2);
    assert!(doc.layers[0].cel_meta[0].artwork_link.is_some());
    assert!(doc.same_cel_image_ids(&original));
    let mut native =
        pixelcraft::formats::decode_project(&pixelcraft::formats::encode_project(&doc).unwrap())
            .unwrap();
    let mut ase =
        pixelcraft::formats::decode_ase(&pixelcraft::formats::encode_ase(&doc).unwrap()).unwrap();
    for restored in [&mut doc, &mut native, &mut ase] {
        resize_canvas_with_trim(restored, [6, 2], [2, 0], false).unwrap();
        assert_eq!(restored.composite(0), original.composite(0));
        assert_eq!(restored.composite(1), original.composite(1));
        assert_eq!(
            restored.layers[1].tile_cels[0],
            original.layers[1].tile_cels[0]
        );
        let slots = restored.layers[0].cel_meta[0].indices.as_ref().unwrap();
        assert_eq!((slots[0], slots[5]), (2, 2));
    }
}

#[test]
fn untrimmed_canvas_retains_layer_mask_coverage_through_native_save() {
    let ink = [210, 61, 35, 255];
    let mut source = Document::new(6, 2);
    source.set_pixel(0, 0, 0, ink);
    source.set_pixel(0, 0, 5, ink);
    let mut mask = Pixels::empty(12);
    mask.set(0, [0, 0, 0, 127]);
    mask.set(5, [0, 0, 0, 255]);
    source.set_layer_mask(0, Some(mask)).unwrap();
    let expected = source.composite(0);
    let mut shrunk = source.clone();
    resize_canvas_with_trim(&mut shrunk, [2, 2], [-2, 0], false).unwrap();
    assert_eq!(shrunk.composite(0), [CLEAR; 4]);

    let mut moved = shrunk.clone();
    moved.layers[0].cel_meta[0].keyframe = Some(CelKeyframe {
        x: 2,
        ..CelKeyframe::default()
    });
    assert_eq!(moved.composite(0)[0], expected[0]);
    assert_eq!(
        moved.composite_view_region(0, [0, 0], [1, 1]).unwrap(),
        [expected[0]]
    );

    let reopened =
        pixelcraft::formats::decode_project(&pixelcraft::formats::encode_project(&shrunk).unwrap())
            .unwrap();
    for trim in [false, true] {
        let mut expanded = reopened.clone();
        resize_canvas_with_trim(&mut expanded, [6, 2], [2, 0], trim).unwrap();
        assert_eq!(expanded.composite(0), expected);
        assert_eq!(expanded.layers[0].mask, source.layers[0].mask);
    }

    let mut rotated = orient_sprite(&reopened, Orientation::TurnRight).unwrap();
    resize_canvas_with_trim(&mut rotated, [2, 6], [0, 2], false).unwrap();
    let expected_turn = orient_sprite(&source, Orientation::TurnRight).unwrap();
    assert_eq!(rotated.composite(0), expected_turn.composite(0));
    assert_eq!(rotated.layers[0].mask, expected_turn.layers[0].mask);

    let mut resampled = resample_sprite(&reopened, [4, 4]).unwrap();
    resize_canvas_with_trim(&mut resampled, [12, 4], [4, 0], false).unwrap();
    let expected_scale = resample_sprite(&source, [12, 4]).unwrap();
    assert_eq!(resampled.composite(0), expected_scale.composite(0));
    assert_eq!(resampled.layers[0].mask, expected_scale.layers[0].mask);

    let mut cleared = reopened;
    cleared.set_layer_mask(0, Some(Pixels::empty(4))).unwrap();
    resize_canvas_with_trim(&mut cleared, [6, 2], [2, 0], false).unwrap();
    assert_eq!(cleared.composite(0), [CLEAR; 12]);
}

#[test]
fn off_canvas_masks_follow_translated_tiles_in_full_and_cropped_renders() {
    let mut doc = Document::new(4, 1);
    let ink = [210, 61, 35, 255];
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Tiles".into(),
        width: 2,
        height: 1,
        tiles: vec![vec![ink; 2].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0][0] = 1;
    let mut mask = Pixels::empty(4);
    mask.set(0, [0, 0, 0, 127]);
    doc.set_layer_mask(1, Some(mask)).unwrap();
    let expected = doc.composite(0);
    resize_canvas_with_trim(&mut doc, [2, 1], [-2, 0], false).unwrap();
    assert_eq!(doc.composite(0), [CLEAR; 2]);
    doc.layers[1].cel_meta[0].keyframe = Some(CelKeyframe {
        x: 2,
        ..CelKeyframe::default()
    });
    assert_eq!(doc.composite(0), expected[..2]);
    assert_eq!(
        doc.composite_view_region(0, [0, 0], [1, 1]).unwrap(),
        expected[..1],
    );
}

#[test]
fn hidden_indexed_artwork_tracks_swatch_edits_reorders_and_ramps() {
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    let mut doc = Document::new(5, 1);
    doc.palette = vec![CLEAR, red, red, blue];
    doc.set_color_mode(ColorMode::Indexed);
    for (index, slot) in [(0, 1), (4, 2)] {
        doc.drawing_index = Some(slot);
        doc.set_pixel(0, 0, index, red);
    }
    resize_canvas_with_trim(&mut doc, [3, 1], [-1, 0], false).unwrap();
    let verify = |mut doc: Document, colors, slots| {
        resize_canvas_with_trim(&mut doc, [5, 1], [1, 0], false).unwrap();
        let pixels = &doc.layers[0].cels[0];
        let indices = doc.layers[0].cel_meta[0].indices.as_ref().unwrap();
        assert_eq!([pixels[0], pixels[4]], colors);
        assert_eq!([indices[0], indices[4]], slots);
    };

    let mut edited = doc.clone();
    edited.set_palette_color(1, green);
    let hidden = &edited.layers[0].cel_meta[0].outside;
    assert_eq!([hidden[0].color, hidden[1].color], [green, red]);
    edited.reorder_palette(1, 3).unwrap();
    for roundtrip in [
        edited.clone(),
        pixelcraft::formats::decode_ase(&pixelcraft::formats::encode_ase(&edited).unwrap())
            .unwrap(),
    ] {
        verify(roundtrip, [green, red], [3, 1]);
    }

    let mut ramp = doc.clone();
    let map = ramp.replace_palette_span(1, 3, 5).unwrap();
    let colors = [ramp.palette[map[1]], ramp.palette[map[2]]];
    assert_ne!(colors[0], colors[1]);
    verify(ramp, colors, [map[1] as u8, map[2] as u8]);

    let converted = doc.indexed_conversion(None, 2, None).unwrap();
    verify(converted, [red, red], [1, 0]);
    doc.set_color_mode(ColorMode::Rgba);
    assert!(
        doc.layers[0].cel_meta[0]
            .outside
            .iter()
            .all(|pixel| pixel.palette_slot.is_none())
    );
    doc.set_color_mode(ColorMode::Indexed);
    verify(doc, [red, red], [1, 1]);
}

#[test]
fn clipping_linked_cels_splits_image_identity_only_for_detached_placements() {
    let mut doc = Document::new(6, 1);
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    doc.set_pixel(0, 0, 1, red);
    doc.set_pixel(0, 0, 2, blue);
    doc.layers[0].cel_meta[0].bounds = Some([1, 0, 2, 1]);
    for (frame, x) in [3, 3, 1].into_iter().enumerate() {
        doc.add_frame(frame, false);
        doc.layers[0].cel_meta[frame + 1].bounds = Some([x, 0, 2, 1]);
        doc.link_cel(0, frame + 1, 0).unwrap();
    }
    let identity = doc.layers[0].cel_meta[0].image_id.runtime_key();
    assert!(
        doc.layers[0]
            .cel_meta
            .iter()
            .all(|cel| cel.image_id.runtime_key() == identity)
    );
    for canvas_size in [false, true] {
        let mut clipped = doc.clone();
        if canvas_size {
            resize_canvas(&mut clipped, [3, 1], [-2, 0]).unwrap();
        } else {
            crop(&mut clipped, [2, 0, 3, 1]).unwrap();
        }
        let frames = &clipped.layers[0].cel_meta;
        let ids: Vec<_> = frames
            .iter()
            .map(|cel| cel.image_id.runtime_key())
            .collect();
        assert_eq!(ids[0], identity);
        assert_eq!(ids[3], identity);
        assert_ne!(ids[1], identity, "detached cels share the source handle");
        assert_eq!(ids[1], ids[2]);
        assert_eq!(frames[0].artwork_link, frames[3].artwork_link);
        assert_eq!(frames[1].artwork_link, frames[2].artwork_link);
        assert_ne!(frames[0].artwork_link, frames[1].artwork_link);
        assert_eq!(clipped.composite(0), [blue, CLEAR, CLEAR]);
        assert_eq!(clipped.composite(1), [CLEAR, red, blue]);
        clipped.validate().unwrap();
    }
}

#[test]
fn cropping_shared_rasters_keeps_their_backing_shared() {
    let mut doc = Document::new(4, 2);
    doc.set_pixel(0, 0, 1, [255, 0, 0, 255]);
    doc.add_frame(0, true);
    assert!(doc.layers[0].cels[0].shares_storage(&doc.layers[0].cels[1]));

    crop(&mut doc, [1, 0, 3, 2]).unwrap();

    assert!(doc.layers[0].cels[0].shares_storage(&doc.layers[0].cels[1]));
    assert_eq!(doc.layers[0].cels[1][0], [255, 0, 0, 255]);
}

#[test]
fn crop_rejects_overflowing_coordinates() {
    let mut doc = Document::new(2, 2);
    let before = doc.clone();
    assert!(crop(&mut doc, [1, 0, i32::MAX, 1]).is_err());
    assert_eq!(doc, before);
}

#[test]
fn unity_reexport_keeps_sprite_ids_after_redraw_and_frame_insertion() {
    use pixelcraft::{formats, interchange};
    use serde_json::Value;

    let dir = std::env::temp_dir().join(format!("pixelcraft-reimport-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("hero.png");
    let json = dir.join("hero.json");
    let meta = dir.join("hero.png.meta");
    let options = SheetOptions {
        preset: "Unity".into(),
        columns: 2,
        ..Default::default()
    };
    let read = || -> (Value, String) {
        (
            serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap(),
            std::fs::read_to_string(&meta).unwrap(),
        )
    };

    let mut d = Document::new(4, 4);
    d.add_frame(0, false);
    d.add_frame(1, false);
    d.durations = vec![83, 120, 250];
    for (i, name) in ["idle", "walk", "jump"].into_iter().enumerate() {
        d.tags.push(FrameTag {
            editor_id: Default::default(),
            name: name.into(),
            from: i,
            to: i,
            direction: 0,
            repeat: 0,
            color: [0; 3],
        });
    }
    interchange::export_sheet(&png, &d, &options).unwrap();
    let (first, first_meta) = read();
    let sprite_guid = "00814b43dcb948909da78bd0b835435d";
    let imported_meta = first_meta.replace("21300002", "-7348592071075510482");
    // Unity may resave a sprite rectangle using IDs assigned by its Sprite Editor.
    let marker = "      internalID: -7348592071075510482";
    let imported_meta = if let Some((before, after)) = imported_meta.split_once(marker) {
        let mut lines = before.lines().collect::<Vec<_>>();
        if lines
            .last()
            .is_some_and(|line| line.starts_with("      spriteID: "))
        {
            lines.pop();
        }
        format!(
            "{}\n      spriteID: {sprite_guid}\n{marker}{after}",
            lines.join("\n")
        )
    } else {
        panic!("Missing sprite local ID");
    };
    std::fs::write(&meta, &imported_meta).unwrap();
    let first_png = std::fs::read(&png).unwrap();
    d.set_pixel(0, 1, 15, [45, 160, 200, 255]);
    interchange::export_sheet(&png, &d, &options).unwrap();
    assert_ne!(first_png, std::fs::read(&png).unwrap());
    let (redrawn, redrawn_meta) = read();
    assert_eq!(first, redrawn);
    assert!(redrawn_meta.contains(&format!("      spriteID: {sprite_guid}")));
    assert!(redrawn_meta.contains("      frame_1: -7348592071075510482"));

    d.add_frame(0, false);
    d.durations[2] = 95;
    let d = formats::decode_project(&formats::encode_project(&d).unwrap()).unwrap();
    interchange::export_sheet(&png, &d, &options).unwrap();
    let (after, after_meta) = read();
    assert_eq!(
        first_meta.lines().find(|line| line.starts_with("guid:")),
        after_meta.lines().find(|line| line.starts_with("guid:"))
    );
    for (old, new) in [(0, 0), (1, 2), (2, 3)] {
        let name = first["frames"][old]["filename"].as_str().unwrap();
        assert_eq!(after["frames"][new]["filename"], name);
        let entry = format!("      {name}: ");
        let old_id = imported_meta
            .lines()
            .find_map(|line| line.strip_prefix(&entry))
            .unwrap();
        let new_id = after_meta
            .lines()
            .find_map(|line| line.strip_prefix(&entry))
            .unwrap();
        assert_eq!(old_id, new_id);
    }
    assert_eq!(after["frames"][2]["duration"], 95);
    assert_eq!(after["meta"]["frameTags"][1]["name"], "walk");
    assert_eq!(after["meta"]["frameTags"][1]["from"], 2);
    let mut invalid = d.clone();
    invalid.tags[2].name = "walk".into();
    assert!(interchange::export_sheet(&png, &invalid, &options).is_err());
    assert_eq!((after, after_meta), read());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pixelcraft_sheet_reconstruction_preserves_frame_names_and_retired_ids() {
    let mut doc = Document::new(8, 8);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.add_frame(0, false);
    doc.add_frame(0, false);
    doc.remove_frame(1);

    let (sheet, metadata) = pack_sheet(&doc, &SheetOptions::default()).unwrap();
    let mut reconstructed = unpack_sheet(&sheet, &metadata).unwrap();
    assert_eq!(
        (0..reconstructed.durations.len())
            .map(|frame| reconstructed.frame_export_id(frame))
            .collect::<Vec<_>>(),
        [0, 3, 1, 2],
    );
    reconstructed.add_frame(3, false);
    assert_eq!(reconstructed.frame_export_id(4), 5);
}

#[test]
fn crop_keeps_tile_grid_phase_and_splits_links_when_placements_clip_differently() {
    use pixelcraft::formats::{decode_ase, encode_ase};
    let mut doc = Document::new(6, 2);
    let red = [200, 40, 30, 255];
    let blue = [20, 40, 200, 255];
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Road".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![red, blue, red, blue].into(), vec![blue; 4].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0].copy_from_slice(&[0x8000_0001, 2, 1]);
    doc.add_frame(0, false);
    doc.layers[1].cel_meta[1].tilemap = Some(TilemapGeometry {
        x: 2,
        y: 0,
        columns: 3,
        rows: 1,
    });
    doc.link_cel(1, 1, 0).unwrap();
    doc.add_frame(1, false);
    doc.layers[1].cel_meta[2].tilemap = None;
    doc.link_cel(1, 2, 0).unwrap();
    let expected: Vec<Vec<Pixel>> = (0..3)
        .map(|frame| {
            doc.composite(frame)
                .chunks_exact(6)
                .flat_map(|row| row[1..4].iter().copied())
                .collect()
        })
        .collect();
    crop(&mut doc, [1, 0, 3, 2]).unwrap();

    assert_eq!([doc.width, doc.height], [3, 2]);
    assert_eq!(doc.layers[1].tileset, Some(0));
    assert_eq!(doc.layers[1].cel_meta[0].tilemap.as_ref().unwrap().x, -1);
    assert_eq!(doc.layers[1].tile_cels[0], [0x8000_0001, 2]);
    assert_eq!(doc.layers[1].tile_cels[1], [0, 0x8000_0001]);
    let metadata = &doc.layers[1].cel_meta;
    assert!(metadata[0].artwork_link.is_some());
    assert_eq!(metadata[0].artwork_link, metadata[2].artwork_link);
    assert!(metadata[1].artwork_link.is_none());
    let reopened = decode_ase(&encode_ase(&doc).unwrap()).unwrap();
    for (frame, pixels) in expected.into_iter().enumerate() {
        assert_eq!(doc.composite(frame), pixels);
        assert_eq!(reopened.composite(frame), pixels);
    }
}

#[test]
fn canvas_trim_at_current_size_discards_hidden_raster_masks_and_tiles() {
    let mut doc = Document::new(6, 2);
    let ink = [210, 55, 20, 255];
    doc.set_pixel(0, 0, 0, ink);
    let mut mask = Pixels::empty(12);
    mask.set(0, [0, 0, 0, 255]);
    doc.set_layer_mask(0, Some(mask)).unwrap();
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Tile".into(),
        width: 2,
        height: 1,
        tiles: vec![vec![ink; 2].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0][0] = 1;
    doc.layers[1].tile_cels[0][1] = 0x8000_0001;
    resize_canvas_with_trim(&mut doc, [3, 2], [-2, 0], false).unwrap();
    let before = doc.composite(0);
    assert!(!doc.layers[0].cel_meta[0].outside.is_empty());
    assert!(!doc.layers[0].mask_outside.is_empty());

    resize_canvas_with_trim(&mut doc, [3, 2], [0, 0], true).unwrap();

    assert_eq!(doc.composite(0), before);
    assert!(doc.layers[0].cel_meta[0].outside.is_empty());
    assert!(doc.layers[0].mask_outside.is_empty());
    assert_eq!(doc.layers[1].tile_cels[0][0], 0x8000_0001);
    let trimmed = doc.clone();
    resize_canvas_with_trim(&mut doc, [3, 2], [0, 0], true).unwrap();
    assert_eq!(doc, trimmed);
    resize_canvas_with_trim(&mut doc, [6, 2], [2, 0], false).unwrap();
    assert_eq!(doc.composite(0)[0], CLEAR);
    assert_eq!(doc.composite(0)[2], ink);
}

#[test]
fn bilinear_resize_filters_a_cel_across_the_canvas_boundary() {
    let mut doc = Document::new(1, 1);
    let red = [210, 40, 20, 255];
    let blue = [30, 40, 200, 255];
    doc.set_pixel(0, 0, 0, blue);
    let cel = &mut doc.layers[0].cel_meta[0];
    cel.bounds = Some([-1, 0, 2, 1]);
    cel.outside.push(OutsidePixel {
        x: -1,
        y: 0,
        color: red,
        palette_slot: None,
    });

    let mut resized = resample_sprite_with(&doc, [2, 1], SpriteResampling::Bilinear).unwrap();
    assert_eq!(resized.composite(0), [[90, 40, 140, 255], blue]);
    resize_canvas_with_trim(&mut resized, [4, 1], [2, 0], false).unwrap();
    assert_eq!(
        resized.composite(0),
        [red, [150, 40, 80, 255], [90, 40, 140, 255], blue]
    );
}

#[test]
fn rotsprite_resizes_hidden_indexed_corners_and_reopens_them_in_ase() {
    use pixelcraft::formats::{decode_ase, encode_ase};
    let mut doc = Document::new(1, 3);
    let ink = [160, 50, 180, 255];
    doc.palette = vec![CLEAR, ink, ink];
    doc.set_color_mode(ColorMode::Indexed);
    doc.drawing_index = Some(1);
    doc.set_pixel(0, 0, 2, ink);
    doc.drawing_index = None;
    let cel = &mut doc.layers[0].cel_meta[0];
    cel.bounds = Some([-2, 0, 3, 3]);
    cel.outside = [(-1, 0), (-2, 1)]
        .map(|(x, y)| OutsidePixel {
            x,
            y,
            color: ink,
            palette_slot: Some(2),
        })
        .into();

    let scaled = resample_sprite_with(&doc, [2, 6], SpriteResampling::RotSprite).unwrap();
    let mut reopened = decode_ase(&encode_ase(&scaled).unwrap()).unwrap();
    assert!(
        reopened.layers[0].cel_meta[0]
            .outside
            .iter()
            .any(|p| { (p.x, p.y, p.palette_slot) == (-2, 2, Some(2)) })
    );
    resize_canvas_with_trim(&mut reopened, [6, 6], [4, 0], false).unwrap();
    let indices = reopened.layers[0].cel_meta[0].indices.as_ref().unwrap();
    assert_eq!(indices[14], 2);
    assert_eq!(indices[35], 1);
}

#[test]
fn polygon_containment_handles_extreme_integer_coordinates_without_overflow() {
    let points = [
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MIN),
        (i32::MAX, i32::MAX),
        (i32::MIN, i32::MAX),
    ];
    assert!(pixelcraft::workflows::inside_polygon(&points, 0, 0));
    assert!(!pixelcraft::workflows::inside_polygon(&points, i32::MAX, 0));
}

#[test]
fn long_polygon_containment_can_be_cancelled_during_edge_traversal() {
    let points = vec![(0, 0); 32768];
    let polls = std::cell::Cell::new(0);
    let result = pixelcraft::workflows::inside_polygon_until(&points, 1, 1, || {
        polls.set(polls.get() + 1);
        polls.get() >= 3
    });
    assert!(result.unwrap_err().contains("cancelled"));
    assert!(!pixelcraft::workflows::inside_polygon(&points, 1, 1));
}
