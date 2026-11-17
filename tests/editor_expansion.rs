use pixelcraft::{automation, model::*, workflows::*};
use serde_json::json;
#[test]
fn repeat_counts_survive_sheet_roundtrip() {
    let mut d = Document::new(2, 2);
    d.tags.push(FrameTag {
        name: "once".into(),
        from: 0,
        to: 0,
        direction: 0,
        repeat: 3,
        color: [0; 3],
    });
    let (im, meta) = pack_sheet(&d, &SheetOptions::default()).unwrap();
    assert_eq!(unpack_sheet(&im, &meta).unwrap().tags[0].repeat, 3);
}
#[test]
fn scaled_sheet_has_matching_coordinates_and_alpha() {
    let mut d = Document::new(2, 1);
    d.set_pixel(0, 0, 1, [20, 40, 60, 127]);
    let (im, meta) = pack_sheet(
        &d,
        &SheetOptions {
            scale: 4,
            trim: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(meta["frames"][0]["sourceSize"]["w"], 8);
    let restored = unpack_sheet(&im, &meta).unwrap();
    assert_eq!(restored.composite(0)[4], [20, 40, 60, 127]);
    assert_eq!(restored.composite(0)[7], [20, 40, 60, 127]);
    assert!(
        pack_sheet(
            &d,
            &SheetOptions {
                scale: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn sprite_sheet_preflights_sparse_frames_and_scaled_atlas_bounds() {
    let mut doc = Document::new(8_000, 8_000);
    for frame in 0..2 {
        doc.add_frame(frame, false);
    }
    assert_eq!(doc.stored_pixels(), 0);
    let error = pack_sheet(&doc, &SheetOptions::default()).expect_err("reject oversized sheet");
    assert!(error.contains("Sprite sheet exceeds image limits"));
    let error = pack_sheet(
        &doc,
        &SheetOptions {
            trim: true,
            ..Default::default()
        },
    )
    .expect_err("reject unbounded trim scan");
    assert!(error.contains("pixel work limit"));

    let doc = Document::new(1, 1);
    let error = pack_sheet(
        &doc,
        &SheetOptions {
            scale: MAX_DIMENSION,
            ..Default::default()
        },
    )
    .expect_err("reject oversized scaled image");
    assert!(error.contains("Scaled export exceeds image limits"));
}

#[test]
fn tagged_sheet_clips_overlapping_tags_and_carries_forward_slice_keys() {
    let mut doc = Document::new(2, 2);
    for frame in 0..5 {
        doc.add_frame(frame, false);
    }
    for (name, from, to) in [("selected", 2, 4), ("overlap", 0, 3), ("outside", 5, 5)] {
        doc.tags.push(FrameTag {
            name: name.into(),
            from,
            to,
            direction: 0,
            repeat: 0,
            color: [0; 3],
        });
    }
    doc.slices.push(SpriteSlice {
        name: "held".into(),
        keys: [0, 1, 4, 5]
            .into_iter()
            .map(|frame| SliceKey {
                frame,
                bounds: [frame as i32, 0, 1, 1],
                center: None,
                pivot: None,
            })
            .collect(),
    });
    let (_, meta) = pack_sheet(
        &doc,
        &SheetOptions {
            tag: Some(0),
            scale: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let restored = &meta["meta"];
    let tags = restored["frameTags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!((&tags[0]["from"], &tags[0]["to"]), (&json!(0), &json!(2)));
    assert_eq!((&tags[1]["from"], &tags[1]["to"]), (&json!(0), &json!(1)));
    let slices: Vec<SpriteSlice> = serde_json::from_value(restored["slices"].clone()).unwrap();
    let keys = &slices[0].keys;
    assert_eq!(
        keys.iter()
            .map(|key| (key.frame, key.bounds))
            .collect::<Vec<_>>(),
        vec![(0, [2, 0, 2, 2]), (2, [8, 0, 2, 2])]
    );
}
#[test]
fn assistant_commands_are_atomic_and_revision_guarded() {
    let d = Document::new(3, 3);
    let args = json!({"revision":automation::document_revision(&d),"commands":[{"op":"add-layer","name":"Ink"},{"op":"shape","layer":1,"frame":0,"kind":"rectangle","x":0,"y":0,"x2":2,"y2":2,"filled":true,"color":[9,8,7,128]}]});
    let out = automation::apply_commands(&d, &args).unwrap();
    assert_eq!(out.layers[1].cels[0][4], [9, 8, 7, 128]);
    assert_eq!(d.layers.len(), 1);
    assert!(automation::apply_commands(&out, &args).is_err());
}
#[test]
fn palette_reorder_preserves_indexed_artwork() {
    let mut d = Document::new(2, 1);
    d.import_palette(vec![CLEAR, [255, 0, 0, 255], [0, 255, 0, 255]])
        .unwrap();
    d.set_color_mode(ColorMode::Indexed);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    let before = d.composite(0);
    d.reorder_palette(1, 2).unwrap();
    assert_eq!(before, d.composite(0));
    assert_eq!(d.layers[0].cel_meta[0].indices.as_ref().unwrap()[0], 2);
}
#[test]
fn effects_respect_mask_and_keep_source_alpha() {
    let mut d = Document::new(3, 3);
    d.set_pixel(0, 0, 4, [255, 0, 0, 128]);
    let mut mask = vec![false; 9];
    mask[4] = true;
    mask[5] = true;
    spatial_effect(&mut d, 0, 0, &mask, [0, 0, 0, 255], Some((1, 0))).unwrap();
    assert_eq!(d.layers[0].cels[0][5], [0, 0, 0, 128]);
    assert_eq!(d.layers[0].cels[0][4], [255, 0, 0, 128]);
    assert_eq!(d.layers[0].cels[0][3], CLEAR);
}
#[test]
fn outline_with_corners_fills_diagonals_inside_selection() {
    let mut sides = Document::new(3, 3);
    sides.set_pixel(0, 0, 4, [255, 0, 0, 128]);
    let mut corners = sides.clone();
    let outline = [0, 0, 0, 255];
    let mut mask = vec![true; 9];
    mask[8] = false;
    spatial_effect(&mut sides, 0, 0, &mask, outline, None).unwrap();
    spatial_outline(&mut corners, 0, 0, &mask, outline, true).unwrap();
    assert_eq!(sides.layers[0].cels[0][0], CLEAR);
    assert_eq!(corners.layers[0].cels[0][0], [0, 0, 0, 128]);
    assert_eq!(corners.layers[0].cels[0][8], CLEAR);
    assert_eq!(corners.layers[0].cels[0][4], [255, 0, 0, 128]);
}

#[test]
fn godot_finite_repeats_and_unique_names() {
    let root =
        std::env::temp_dir().join(format!("pixelcraft-godot-expansion-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(1, 1);
    d.add_frame(0, false);
    for (name, repeat) in [("walk", 2), ("walk", 1), ("walk_2", 0)] {
        d.tags.push(FrameTag {
            name: name.into(),
            from: 0,
            to: 1,
            direction: 2,
            repeat,
            color: [0; 3],
        });
    }
    pixelcraft::interchange::export_sheet(
        &root.join("test.png"),
        &d,
        &SheetOptions {
            preset: "Godot".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let tres = std::fs::read_to_string(root.join("test.tres")).unwrap();
    assert_eq!(tres.matches("\"loop\": false").count(), 2);
    assert_eq!(tres.matches("\"loop\": true").count(), 1);
    for name in ["walk", "walk_2", "walk_2_2"] {
        assert!(tres.contains(&format!("&\"{name}\"")));
    }
    assert_eq!(tres.matches("\"duration\"").count(), 8);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn oversized_godot_header_leaves_existing_exports_untouched() {
    let root =
        std::env::temp_dir().join(format!("pixelcraft-godot-sidecar-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let png = root.join("sheet.png");
    let json = root.join("sheet.json");
    let tres = root.join("sheet.tres");
    std::fs::write(&png, b"previous PNG").unwrap();
    std::fs::write(&json, b"previous JSON").unwrap();
    let header = format!("[gd_resource {}]\n", "x".repeat(8192));
    std::fs::write(&tres, &header).unwrap();
    let result = pixelcraft::interchange::export_sheet(
        &png,
        &Document::new(1, 1),
        &SheetOptions {
            preset: "Godot".into(),
            ..Default::default()
        },
    );
    let png_bytes = std::fs::read(&png).unwrap();
    let json_bytes = std::fs::read(&json).unwrap();
    let tres_bytes = std::fs::read(&tres).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        result
            .expect_err("reject oversized header")
            .contains("header")
    );
    assert_eq!(png_bytes, b"previous PNG");
    assert_eq!(json_bytes, b"previous JSON");
    assert_eq!(tres_bytes, header.as_bytes());
}

#[test]
fn godot_limits_expanded_keys_across_tags_before_writing() {
    let root = std::env::temp_dir().join(format!("pixelcraft-godot-budget-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let png = root.join("sheet.png");
    let mut doc = Document::new(1, 1);
    for frame in 0..7 {
        doc.add_frame(frame, false);
    }
    for name in ["one", "two"] {
        doc.tags.push(FrameTag {
            name: name.into(),
            from: 0,
            to: 7,
            direction: 0,
            repeat: 62_501,
            color: [0; 3],
        });
    }
    let result = pixelcraft::interchange::export_sheet(
        &png,
        &doc,
        &SheetOptions {
            preset: "Godot".into(),
            ..Default::default()
        },
    );
    let wrote_files = std::fs::read_dir(&root).unwrap().next().is_some();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        result
            .expect_err("reject total key count")
            .contains("million")
    );
    assert!(!wrote_files);
}
#[test]
fn updater_rejects_corruption_and_wrong_filename() {
    use pixelcraft::updates::verify_checksum;
    let sum = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(verify_checksum(b"abc", &format!("{sum}  app.exe"), "app.exe").is_ok());
    assert!(verify_checksum(b"bad", &format!("{sum}  app.exe"), "app.exe").is_err());
    assert!(verify_checksum(b"abc", &format!("{sum}  other.exe"), "app.exe").is_err());
}
#[test]
fn filter_targets_selection_and_reads_neighborhood_from_original_pixels() {
    use pixelcraft::workflows::filters::{Filter, apply_filter};
    let mut doc = Document::new(3, 3);
    let base = [30, 64, 90, 255];
    doc.layers[0].cels[0] = vec![base; 9].into();
    doc.set_pixel(0, 0, 4, [220, 6, 2, 255]);
    doc.add_frame(0, true);
    let source = doc.clone();
    let mut mask = vec![false; 9];
    mask[4] = true;
    let (filtered, changes) = apply_filter(
        &doc,
        &[(0, 0)],
        [1, 1, 2, 2],
        Some(&mask),
        &Filter::Despeckle { threshold: 25 },
        [true, false, true, false],
    )
    .unwrap();
    assert_eq!(changes, 1);
    assert_eq!(filtered.layers[0].cels[0][4], [30, 6, 90, 255]);
    assert_eq!(filtered.layers[0].cels[0][3], base);
    assert_eq!(filtered.layers[0].cels[1][4], [220, 6, 2, 255]);
    assert_eq!(doc, source);
}

#[test]
fn filter_curves_brightness_and_kernel_share_the_expected_channel_math() {
    use pixelcraft::workflows::filters::{Filter, apply_filter};
    let mut doc = Document::new(3, 1);
    doc.layers[0].cels[0] = vec![[0, 50, 80, 255], [90, 50, 80, 255], [0, 50, 80, 255]].into();
    let red = [true, false, false, false];
    let apply = |filter| {
        apply_filter(&doc, &[(0, 0)], [0, 0, 3, 1], None, &filter, red)
            .unwrap()
            .0
    };
    let curved = apply(Filter::Curves(vec![(0, 0), (90, 180), (255, 255)]));
    assert_eq!(curved.layers[0].cels[0][1], [180, 50, 80, 255]);
    let bright = apply(Filter::BrightnessContrast {
        brightness: 20,
        contrast: 0,
    });
    assert_eq!(bright.layers[0].cels[0][1], [141, 50, 80, 255]);
    let flat = apply(Filter::BrightnessContrast {
        brightness: 0,
        contrast: -100,
    });
    assert_eq!(flat.layers[0].cels[0][1], [128, 50, 80, 255]);
    let blur = apply(Filter::Convolution {
        weights: [1; 9],
        divisor: 9,
        bias: 0,
    });
    assert_eq!(blur.layers[0].cels[0].to_vec(), [[30, 50, 80, 255]; 3]);
}

#[test]
fn artwork_palette_and_indexed_conversion_keep_transparency_and_authored_slots() {
    use pixelcraft::contributions::DitheringMatrix;
    use pixelcraft::workflows::generate_artwork_palette;
    let mut d = Document::new(4, 2);
    let dark = [20, 30, 40, 255];
    let light = [200, 210, 220, 255];
    let translucent = [80, 90, 100, 180];
    d.layers[0].cels[0] = vec![dark, dark, light, CLEAR, dark, translucent, CLEAR, CLEAR].into();
    assert_eq!(
        generate_artwork_palette(&d, 0..=0, 4).unwrap(),
        [CLEAR, dark, translucent, light]
    );
    let reduced = generate_artwork_palette(&d, 0..=0, 3).unwrap();
    assert_eq!(reduced.len(), 3);
    assert_eq!(reduced[0], CLEAR);
    let matrix = DitheringMatrix::bayer(2).unwrap();
    let converted = d
        .indexed_conversion(Some(vec![CLEAR, dark, light]), 2, Some(&matrix))
        .unwrap();
    assert_eq!(converted.color_mode, ColorMode::Indexed);
    assert_eq!(converted.transparent_index, 2);
    assert_eq!(converted.palette, [light, dark, CLEAR]);
    let indices = converted.layers[0].cel_meta[0].indices.as_ref().unwrap();
    assert_eq!([indices[0], indices[2], indices[3]], [1, 0, 2]);
    let moved = converted.indexed_conversion(None, 1, None).unwrap();
    assert_eq!(moved.layers[0].cels[0], converted.layers[0].cels[0]);
    let indices = moved.layers[0].cel_meta[0].indices.as_ref().unwrap();
    assert_eq!([indices[0], indices[2], indices[3]], [2, 0, 1]);
    assert_eq!(d.color_mode, ColorMode::Rgba);
}

#[test]
fn indexed_conversion_dithers_midtones_without_changing_endpoints_or_frame_palettes() {
    use pixelcraft::contributions::DitheringMatrix;
    let mut d = Document::new(4, 2);
    let black = [0, 0, 0, 255];
    let white = [255; 4];
    let red = [255, 0, 0, 255];
    d.palette = vec![CLEAR, black, white];
    d.add_frame(0, false);
    d.frame_palettes = vec![d.palette.clone(), vec![CLEAR, black, red]];
    let gray = [125, 125, 125, 255];
    d.layers[0].cels[0] = vec![gray, gray, gray, gray, gray, gray, black, CLEAR].into();
    d.layers[0].cels[1].set(0, red);
    let plain = d.indexed_conversion(None, 0, None).unwrap();
    assert_eq!(
        plain.layers[0].cels[0].to_vec(),
        [black, black, black, black, black, black, black, CLEAR]
    );
    let matrix = DitheringMatrix::bayer(2).unwrap();
    let ordered = d.indexed_conversion(None, 0, Some(&matrix)).unwrap();
    assert_eq!(
        ordered.layers[0].cels[0].to_vec(),
        [white, black, white, black, black, white, black, CLEAR]
    );
    assert_eq!(ordered.layers[0].cels[1][0], red);
    assert_eq!(ordered.frame_palettes, d.frame_palettes);
}

#[test]
fn palette_sort_and_noncontiguous_reverse_preserve_indexed_artwork() {
    use pixelcraft::model::PaletteSort;
    let mut doc = Document::new(4, 1);
    doc.set_color_mode(ColorMode::Indexed);
    let red = [190, 30, 20, 255];
    let green = [10, 70, 20, 255];
    let blue = [20, 30, 180, 255];
    doc.import_palette(vec![CLEAR, red, blue, green]).unwrap();
    for (index, color) in [CLEAR, red, blue, green].into_iter().enumerate() {
        doc.set_pixel(0, 0, index, color);
    }
    let colors = doc.layers[0].cels[0].clone();
    assert_eq!(
        doc.sort_palette(&[], PaletteSort::Hue).unwrap(),
        [0, 1, 3, 2]
    );
    assert_eq!(doc.palette, [CLEAR, red, green, blue]);
    assert_eq!(doc.reverse_palette_entries(&[3, 1]).unwrap(), [0, 3, 2, 1]);
    assert_eq!(doc.palette, [CLEAR, blue, green, red]);
    assert!(doc.layers[0].cels[0].shares_storage(&colors));
    assert_eq!(
        doc.layers[0].cel_meta[0]
            .indices
            .as_ref()
            .unwrap()
            .as_slice(),
        [0, 3, 1, 2]
    );
}

#[test]
fn hue_palette_ramps_follow_short_arc_and_keep_alpha() {
    use pixelcraft::workflows::{RampInterpolation, ramp_interpolated};
    let colors = ramp_interpolated(
        [255, 0, 128, 100],
        [255, 128, 0, 200],
        3,
        RampInterpolation::Hue,
    );
    assert_eq!(
        colors,
        [[255, 0, 128, 100], [255, 0, 0, 150], [255, 128, 0, 200]]
    );
}

#[test]
fn ramps_append_without_recoloring_indexed_frames() {
    let mut d = Document::new(1, 1);
    d.import_palette(vec![CLEAR, [255, 0, 0, 255], [0, 0, 255, 255]])
        .unwrap();
    d.set_color_mode(ColorMode::Indexed);
    d.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    let before = d.layers[0].clone();
    d.palette_ramp(1, 2, 5).unwrap();
    assert_eq!(d.palette.len(), 6);
    assert_eq!(d.layers[0], before);
}
#[test]
fn replacing_palette_span_remaps_indexed_pixels_and_keeps_duplicate_outside_slots() {
    let dark = [20, 40, 60, 255];
    let duplicate = [150, 60, 20, 255];
    let light = [220, 240, 200, 255];
    let mut d = Document::new(6, 1);
    d.set_color_mode(ColorMode::Indexed);
    d.import_palette(vec![
        dark,
        duplicate,
        [12, 32, 44, 255],
        light,
        CLEAR,
        duplicate,
    ])
    .unwrap();
    d.add_frame(0, false);
    d.frame_palettes = vec![d.palette.clone(), d.palette.clone()];
    d.frame_palettes[1][0] = [0, 0, 0, 255];
    d.frame_palettes[1][3] = [200, 200, 200, 255];
    d.set_color_mode(ColorMode::Indexed);
    for frame in 0..2 {
        for (pixel, slot) in [0, 1, 2, 3, 4, 5].into_iter().enumerate() {
            d.drawing_index = Some(slot);
            let color = d.palette_for(frame)[slot];
            d.set_pixel(0, frame, pixel, color);
        }
    }
    d.drawing_index = Some(5);
    d.palette_names = ["Dark", "Old one", "Old two", "Light", "Clear", "Keep"]
        .map(str::to_owned)
        .to_vec();
    let remap = d.replace_palette_span(0, 3, 3).unwrap();
    assert_eq!(remap, [0, 1, 1, 2, 3, 4]);
    assert_eq!(d.transparent_index, 3);
    assert_eq!(d.drawing_index, Some(4));
    assert_eq!(
        d.palette,
        [dark, [120, 140, 130, 255], light, CLEAR, duplicate]
    );
    assert_eq!(d.palette_names, ["Dark", "", "Light", "Clear", "Keep"]);
    assert_eq!(d.frame_palettes[1][1], [100, 100, 100, 255]);
    for frame in 0..2 {
        assert_eq!(
            d.layers[0].cel_meta[frame]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            &[0, 1, 1, 2, 3, 4]
        );
        let expected: Vec<_> = remap
            .iter()
            .map(|&slot| d.palette_for(frame)[slot])
            .collect();
        assert_eq!(d.layers[0].cels[frame].to_vec(), expected);
    }
    d.validate().unwrap();
}

#[test]
fn palette_span_keeps_linked_storage_and_rejects_invalid_transparency_atomically() {
    let mut d = Document::new(3, 1);
    d.set_color_mode(ColorMode::Indexed);
    let mid = [140, 100, 60, 255];
    let end = [200, 180, 160, 255];
    d.import_palette(vec![[40, 20, 10, 255], CLEAR, mid, end])
        .unwrap();
    d.set_pixel(0, 0, 1, mid);
    d.layers[0].link_duplicates = true;
    d.add_frame(0, true);
    assert!(d.layers[0].cels[0].shares_storage(&d.layers[0].cels[1]));
    let before = d.clone();
    assert!(d.replace_palette_span(0, 3, 2).is_err());
    assert_eq!(d, before);
    assert_eq!(d.replace_palette_span(1, 3, 2).unwrap(), [0, 1, 2, 2]);
    assert_eq!(d.layers[0].cels[0][1], end);
    assert_eq!(d.layers[0].cels[1][1], end);
    assert_eq!(d.layers[0].cels[0][0], CLEAR);
    assert!(d.layers[0].cels[0].shares_storage(&d.layers[0].cels[1]));
    assert_eq!(d.layers[0].cel_meta[0].indices.as_ref().unwrap()[1], 2);
    assert_eq!(d.layers[0].cel_meta[1].indices.as_ref().unwrap()[1], 2);
    let mut d = before;
    assert_eq!(d.replace_palette_span(0, 3, 3).unwrap(), [0, 1, 2, 2]);
    assert_eq!(d.palette, [[40, 20, 10, 255], CLEAR, end]);
    assert_eq!(d.transparent_index, 1);
    assert_eq!(d.layers[0].cels[0][0], CLEAR);
    assert_eq!(d.layers[0].cels[0][1], end);
}

#[test]
fn varying_frame_palettes_reject_ramps_atomically() {
    for palette in [vec![CLEAR], vec![[10, 20, 30, 255]; 256]] {
        let mut d = Document::new(1, 1);
        d.frame_palettes = vec![palette];
        d.validate().unwrap();
        let before = d.clone();
        assert!(d.palette_ramp(0, 1, 5).is_err());
        assert_eq!(d, before);
    }
}
#[test]
fn folder_atlas_inputs_are_sorted_and_skip_unrelated_files() {
    use pixelcraft::productivity::expand_atlas_inputs;
    let root = std::env::temp_dir().join(format!("pixelcraft-folder-input-{}", std::process::id()));
    std::fs::create_dir_all(root.join("sub")).unwrap();
    for file in ["b.png", "a.pixel", "notes.txt", "sub/c.BMP"] {
        std::fs::write(root.join(file), []).unwrap();
    }
    let files = expand_atlas_inputs(std::slice::from_ref(&root)).unwrap();
    assert_eq!(
        files,
        vec![
            root.join("a.pixel"),
            root.join("b.png"),
            root.join("sub/c.BMP")
        ]
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn bmp_and_png_scale_pixel_blocks_without_filtering() {
    let root = std::env::temp_dir().join(format!("pixelcraft-raster-scale-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(2, 1);
    d.set_pixel(0, 0, 0, [20, 40, 60, 127]);
    d.set_pixel(0, 0, 1, [70, 80, 90, 255]);
    for format in ["bmp", "png"] {
        let path = root.join(format!("scaled.{format}"));
        pixelcraft::formats::export_raster_scaled(&path, &d, 0, false, 2).unwrap();
        let im = image::open(path).unwrap().to_rgba8();
        assert_eq!(im.dimensions(), (4, 2));
        assert_eq!(im.get_pixel(1, 1).0, [20, 40, 60, 127]);
        assert_eq!(im.get_pixel(2, 1).0, [70, 80, 90, 255]);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn reference_copy_uses_placement_and_opacity_in_new_layer() {
    use pixelcraft::productivity::*;
    let mut d = Document::new(4, 2);
    let r = ReferenceImage {
        name: "Guide".into(),
        width: 1,
        height: 1,
        pixels: vec![[200, 100, 50, 128]].into(),
        x: 1.,
        y: 0.,
        scale: 2.,
        opacity: 128,
        visible: true,
        behind: true,
        floating: false,
        options: Default::default(),
    };
    reference_to_layer(&mut d, &r, 0).unwrap();
    assert_eq!(d.layers.len(), 2);
    assert_eq!(d.layers[1].cels[0][1], [200, 100, 50, 64]);
    assert_eq!(d.layers[1].cels[0][6], [200, 100, 50, 64]);
    assert_eq!(d.layers[1].cels[0][0], CLEAR);
}
