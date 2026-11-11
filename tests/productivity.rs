use image::{Rgba, RgbaImage};
use pixelcraft::{atlas::*, automation::*, model::*, productivity::*};
#[test]
fn wand_tolerance_connectivity_and_combinations() {
    let p = vec![
        [10, 0, 0, 255],
        [14, 0, 0, 255],
        [90, 0, 0, 255],
        [10, 0, 0, 255],
    ];
    let a = matching(&p, 4, 1, (0, 0), 5, true, false);
    assert_eq!(a, vec![true, true, false, false]);
    let b = matching(&p, 4, 1, (0, 0), 0, false, false);
    assert_eq!(b, vec![true, false, false, true]);
    let mut s = a;
    combine_selection(&mut s, &b, SelectionOp::Add);
    assert!(s[3]);
    combine_selection(&mut s, &b, SelectionOp::Subtract);
    assert_eq!(s, vec![false, true, false, false]);
}
#[test]
fn wrap_fill_brush_and_palette_snapping() {
    let mut d = Document::new(4, 1);
    d.layers[0].cels[0] = vec![
        [1, 0, 0, 255],
        [2, 0, 0, 255],
        [2, 0, 0, 255],
        [1, 0, 0, 255],
    ]
    .into();
    d.wraparound = true;
    d.fill(0, 0, 0, 0, [8, 0, 0, 255]);
    assert_eq!(d.layers[0].cels[0][3], [8, 0, 0, 255]);
    d.palette = vec![[0, 0, 0, 255], [255, 0, 0, 255]];
    d.palette_snap = true;
    d.stamp(0, 0, -1, 0, [240, 0, 0, 255], 1);
    assert_eq!(d.layers[0].cels[0][3], [255, 0, 0, 255]);
    let o = BrushOptions {
        mode: BrushMode::Dither,
        alternate: [0, 0, 0, 255],
        ..Default::default()
    };
    paint_brush(&mut d, 0, 0, (0, 0), [255, 0, 0, 255], 4, &o);
    assert_ne!(d.layers[0].cels[0][0], d.layers[0].cels[0][1]);
}
#[test]
fn palette_formats_roundtrip_and_reject_bad_input() {
    let p = vec![[12, 34, 56, 255], [255, 0, 128, 255]];
    for ext in ["hex", "pal", "gpl"] {
        assert_eq!(
            parse_palette(&write_palette(&p, ext).unwrap(), ext).unwrap(),
            p
        );
    }
    assert!(parse_palette(b"JASC-PAL\n0100\n2\n999 0 0\n", "pal").is_err());
}
#[test]
fn grid_import_spacing_layers_and_tag_direction() {
    let mut im = RgbaImage::new(7, 3);
    im.put_pixel(1, 1, Rgba([9, 8, 7, 255]));
    im.put_pixel(4, 1, Rgba([1, 2, 3, 255]));
    let g = GridImport {
        width: 2,
        height: 2,
        offset_x: 1,
        offset_y: 1,
        spacing_x: 1,
        spacing_y: 0,
        layers: false,
    };
    let d = import_grid(&im, &g).unwrap();
    assert_eq!(d.durations.len(), 2);
    assert_eq!(d.composite(1)[0], [1, 2, 3, 255]);
    let l = import_grid(&im, &GridImport { layers: true, ..g }).unwrap();
    assert_eq!(l.layers.len(), 2);
    let t = FrameTag {
        name: "x".into(),
        from: 1,
        to: 3,
        direction: 3,
        repeat: 0,
        color: [0; 3],
    };
    assert_eq!(tag_frames(&t), vec![3, 2, 1, 2]);
}
#[test]
fn atlas_deduplicates_preserves_sources_and_power_of_two() {
    let mut a = Document::new(3, 2);
    a.set_pixel(0, 0, 1, [255, 0, 0, 255]);
    a.add_frame(0, true);
    a.durations[1] = 250;
    let b = a.clone();
    for layout in [AtlasLayout::Rows, AtlasLayout::MaxRects] {
        let (im, m) = pack_atlas(
            &[("a".into(), a.clone()), ("b".into(), b.clone())],
            &AtlasOptions {
                layout,
                trim: true,
                deduplicate: true,
                power_of_two: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(im.width().is_power_of_two() && im.height().is_power_of_two());
        assert_eq!(m["frames"].as_array().unwrap().len(), 4);
        assert_eq!(m["frames"][0]["frame"], m["frames"][3]["frame"]);
        assert_eq!(m["frames"][1]["duration"], 250);
        assert_eq!(m["frames"][0]["spriteSourceSize"]["x"], 1);
    }
}
#[test]
fn stale_bridge_edits_are_atomic() {
    let d = Document::new(2, 2);
    let revision = document_revision(&d);
    let edit = serde_json::json!({"revision":revision,"edits":[{"layer":0,"frame":0,"x":1,"y":1,"color":[1,2,3,255]}]});
    let updated = apply_edits(&d, &edit).unwrap();
    assert_eq!(updated.composite(0)[3], [1, 2, 3, 255]);
    assert!(apply_edits(&updated, &edit).is_err());
    let invalid = serde_json::json!({"revision":revision,"edits":[{"layer":0,"frame":0,"x":8,"y":1,"color":[1,2,3,255]}]});
    assert!(apply_edits(&d, &invalid).is_err());
}
#[test]
fn references_survive_native_save_but_never_composite() {
    let mut d = Document::new(2, 2);
    d.references.push(ReferenceImage {
        name: "guide".into(),
        width: 1,
        height: 1,
        pixels: vec![[255, 0, 0, 255]].into(),
        x: 1.,
        y: 2.,
        scale: 2.,
        opacity: 128,
        visible: true,
    });
    let path =
        std::env::temp_dir().join(format!("pixelcraft-reference-{}.pixel", std::process::id()));
    pixelcraft::formats::save(&path, &d).unwrap();
    assert_eq!(pixelcraft::formats::load(&path).unwrap(), d);
    assert!(d.composite(0).iter().all(|p| p[3] == 0));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn batch_tags_keeps_sheet_direction_and_gif_playback_timing() {
    let root = std::env::temp_dir().join(format!("pixelcraft-batch-{}", std::process::id()));
    let mut d = Document::new(2, 1);
    for f in 0..3 {
        if f > 0 {
            d.add_frame(f - 1, false);
        }
        d.set_pixel(0, f, 0, [(f * 80) as u8, 0, 0, 255]);
        d.durations[f] = [70, 130, 210][f];
    }
    d.tags.push(FrameTag {
        name: "../walk".into(),
        from: 0,
        to: 2,
        direction: 3,
        repeat: 0,
        color: [0; 3],
    });
    let o = pixelcraft::workflows::SheetOptions {
        preset: "Godot".into(),
        ..Default::default()
    };
    let sheets = batch_tags(&root, &d, false, &o).unwrap();
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(sheets[0].with_extension("json")).unwrap()).unwrap();
    assert_eq!(
        meta["meta"]["frameTags"][0]["direction"],
        "pingpong_reverse"
    );
    assert_eq!(meta["frames"].as_array().unwrap().len(), 3);
    assert!(sheets[0].with_extension("tres").is_file());
    let gifs = batch_tags(&root, &d, true, &o).unwrap();
    let loaded = pixelcraft::formats::load(&gifs[0]).unwrap();
    assert_eq!(loaded.durations, vec![210, 130, 70, 130]);
    assert_eq!(loaded.composite(0)[0], d.composite(2)[0]);
    assert!(gifs[0].parent().unwrap() == root);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn batch_tag_gifs_preserve_repeat_counts_and_direction() {
    use image::AnimationDecoder;
    let root = std::env::temp_dir().join(format!("pixelcraft-tag-repeat-{}", std::process::id()));
    let mut d = Document::new(1, 1);
    for f in 0..3 {
        if f > 0 {
            d.add_frame(f - 1, false);
        }
        d.set_pixel(0, f, 0, [(f * 80) as u8, 0, 0, 255]);
        d.durations[f] = [70, 130, 210][f];
    }
    for (direction, durations, colors) in [
        (0, vec![70, 130, 210], vec![0, 80, 160]),
        (1, vec![210, 130, 70], vec![160, 80, 0]),
        (2, vec![70, 130, 210, 130], vec![0, 80, 160, 80]),
        (3, vec![210, 130, 70, 130], vec![160, 80, 0, 80]),
    ] {
        for repeat in [0, 1, 2, u16::MAX] {
            d.tags = vec![FrameTag {
                name: "walk".into(),
                from: 0,
                to: 2,
                direction,
                repeat,
                color: [0; 3],
            }];
            let paths = batch_tags(&root, &d, true, &Default::default()).unwrap();
            let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
                std::fs::File::open(&paths[0]).unwrap(),
            ))
            .unwrap();
            let actual = match decoder.loop_count() {
                image::metadata::LoopCount::Finite(n) => n.get(),
                image::metadata::LoopCount::Infinite => 0,
            };
            assert_eq!(
                actual, repeat as u32,
                "direction {direction}, repeat {repeat}"
            );
            let loaded = pixelcraft::formats::load(&paths[0]).unwrap();
            assert_eq!(loaded.durations, durations);
            assert_eq!(
                (0..loaded.durations.len())
                    .map(|f| loaded.composite(f)[0][0])
                    .collect::<Vec<_>>(),
                colors,
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn indexed_shade_steps_from_and_preserves_duplicate_palette_indices() {
    let mut d = Document::new(1, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255]];
    d.set_color_mode(ColorMode::Indexed);
    let options = BrushOptions {
        mode: BrushMode::Shade,
        ..Default::default()
    };
    for (start, reverse, expected) in [(2, false, 3), (1, false, 2), (2, true, 1), (3, true, 2)] {
        d.drawing_index = Some(start);
        d.set_pixel(0, 0, 0, d.palette[start]);
        // The selected drawing index must not override the shade destination.
        d.drawing_index = Some(1);
        paint_brush(
            &mut d,
            0,
            0,
            (0, 0),
            CLEAR,
            1,
            &BrushOptions {
                reverse,
                ..options.clone()
            },
        );
        assert_eq!(d.layers[0].cels[0][0], d.palette[expected]);
        assert_eq!(
            d.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
            expected as u8
        );
        assert_eq!(d.drawing_index, Some(1));
    }
}
#[test]
fn maxrects_reconstructs_variable_frames_without_overlap() {
    let mut sources = vec![];
    for n in 1..=9 {
        let mut d = Document::new(n + 1, 11 - n);
        for i in 0..d.width * d.height {
            d.set_pixel(0, 0, i, [n as u8, (i % 256) as u8, 0, 255]);
        }
        sources.push((format!("sprite{n}"), d));
    }
    let (im, meta) = pack_atlas(
        &sources,
        &AtlasOptions {
            width: 32,
            padding: 2,
            trim: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut occupied = std::collections::HashSet::new();
    for (f, (_, d)) in meta["frames"].as_array().unwrap().iter().zip(&sources) {
        let x = f["frame"]["x"].as_u64().unwrap() as u32;
        let y = f["frame"]["y"].as_u64().unwrap() as u32;
        for cy in 0..d.height {
            for cx in 0..d.width {
                assert!(occupied.insert((x + cx as u32, y + cy as u32)));
                assert_eq!(
                    im.get_pixel(x + cx as u32, y + cy as u32).0,
                    d.composite(0)[cy * d.width + cx]
                );
            }
        }
    }
}
#[test]
fn onion_combines_both_directions_and_respects_zero_opacity() {
    let mut d = Document::new(3, 1);
    d.set_pixel(0, 0, 0, [255, 255, 255, 255]);
    d.add_frame(0, false);
    d.add_frame(1, false);
    d.set_pixel(0, 2, 2, [255, 255, 255, 255]);
    let o = OnionOptions {
        previous: 8,
        next: 8,
        previous_tint: [255, 0, 0],
        next_tint: [0, 0, 255],
        previous_opacity: 200,
        next_opacity: 100,
    };
    let p = onion_pixels(&d, 1, &o);
    assert_eq!(p[0], [255, 0, 0, 200]);
    assert_eq!(p[2], [0, 0, 255, 100]);
    let p = onion_pixels(
        &d,
        1,
        &OnionOptions {
            previous_opacity: 0,
            next_opacity: 0,
            ..o
        },
    );
    assert!(p.iter().all(|p| p[3] == 0));
}
#[test]
fn shade_stamp_and_dither_use_palette_order_alpha_and_canvas_phase() {
    let mut d = Document::new(6, 1);
    d.palette = vec![[10, 0, 0, 255], [100, 0, 0, 255], [200, 0, 0, 255]];
    d.set_pixel(0, 0, 0, d.palette[1]);
    let o = BrushOptions {
        mode: BrushMode::Shade,
        ..Default::default()
    };
    paint_brush(&mut d, 0, 0, (0, 0), [0; 4], 1, &o);
    assert_eq!(d.composite(0)[0], d.palette[2]);
    paint_brush(
        &mut d,
        0,
        0,
        (0, 0),
        [0; 4],
        1,
        &BrushOptions { reverse: true, ..o },
    );
    assert_eq!(d.composite(0)[0], d.palette[1]);
    let stamp = BrushOptions {
        mode: BrushMode::Selection,
        stamp: Some((2, 1, vec![[9, 8, 7, 128], CLEAR])),
        ..Default::default()
    };
    d.set_pixel(0, 0, 4, [255; 4]);
    paint_brush(&mut d, 0, 0, (4, 0), [255; 4], 1, &stamp);
    assert_eq!(d.composite(0)[3], [9, 8, 7, 128]);
    assert_eq!(d.composite(0)[4], CLEAR);
}
#[test]
fn palettes_accept_uppercase_extensions() {
    let p = vec![[1, 2, 3, 255]];
    for ext in ["PAL", "HEX", "GPL"] {
        assert_eq!(
            parse_palette(&write_palette(&p, ext).unwrap(), ext).unwrap(),
            p
        );
    }
}
#[test]
fn atlas_respects_maximum_width() {
    let d = Document::new(20, 2);
    assert!(
        pack_atlas(
            &[("wide".into(), d)],
            &AtlasOptions {
                width: 16,
                trim: false,
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn maxrects_tries_height_limit_before_rejecting_a_valid_atlas() {
    let sources = vec![
        ("a".into(), Document::new(6, 9000)),
        ("b".into(), Document::new(6, 1000)),
    ];
    let (im, _) = pack_atlas(
        &sources,
        &AtlasOptions {
            width: 10,
            padding: 0,
            trim: false,
            deduplicate: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(im.height(), 10000);
}
#[test]
fn mcp_lists_file_tools_without_live_access_and_returns_errors() {
    let list = rpc(
        &serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        None,
    )
    .unwrap();
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "atlas"));
    assert!(!tools.iter().any(|t| t["name"] == "live-edit"));
    let result=rpc(&serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"live-edit","arguments":{}}}),None).unwrap();
    assert_eq!(result["result"]["isError"], true);
    assert!(
        rpc(
            &serde_json::json!({"method":"notifications/initialized"}),
            None
        )
        .is_none()
    );
}
#[test]
fn mcp_render_provides_a_png_image() {
    let d = Document::new(2, 2);
    let root = std::env::temp_dir().join(format!(
        "pixelcraft-mcp-render-{}.pixel",
        std::process::id()
    ));
    pixelcraft::formats::save(&root, &d).unwrap();
    let result=rpc(&serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"render","arguments":{"input":root}}}),None).unwrap();
    assert_eq!(result["result"]["content"][0]["type"], "image");
    assert_eq!(result["result"]["content"][0]["mimeType"], "image/png");
    assert!(
        result["result"]["content"][0]["data"]
            .as_str()
            .unwrap()
            .starts_with("iVBOR")
    );
    std::fs::remove_file(root).unwrap();
}
#[test]
#[ignore = "Requires loopback network access, unavailable in the build sandbox"]
fn live_bridge_authentication_and_opt_in_lifecycle() {
    use std::io::{BufRead, Write};
    use std::time::Duration;
    let config = std::env::temp_dir().join(format!(
        "pixelcraft-bridge-test-{}.json",
        std::process::id()
    ));
    let bridge = LiveBridge::start(config.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&config).unwrap().permissions().mode() & 0o077,
            0
        );
    }
    let mut bad = std::net::TcpStream::connect(v["address"].as_str().unwrap()).unwrap();
    writeln!(
        bad,
        "{}",
        serde_json::json!({"token":"wrong","operation":"live-edit","arguments":{}})
    )
    .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(bad).read_line(&mut line).unwrap();
    assert!(line.contains("Invalid bridge token"));
    assert!(bridge.requests.try_recv().is_err());
    let config2 = config.clone();
    let handle = std::thread::spawn(move || {
        rpc(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"live-inspect","arguments":{}}}),Some(&config2)).unwrap()
    });
    let request = bridge
        .requests
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert_eq!(request.operation, "live-inspect");
    request
        .response
        .send(Ok(serde_json::json!({"revision":"test-revision"})))
        .unwrap();
    let response = handle.join().unwrap();
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("test-revision")
    );
    drop(bridge);
    assert!(!config.exists());
}
#[test]
fn palette_snapping_preserves_stamp_pixels_and_selection_transforms() {
    let mut d = Document::new(4, 1);
    d.palette = vec![[0, 0, 0, 255], [255, 255, 255, 255]];
    d.set_pixel(0, 0, 0, [123, 45, 67, 128]);
    d.palette_snap = true;
    let o = BrushOptions {
        mode: BrushMode::Selection,
        stamp: Some((1, 1, vec![[123, 45, 67, 128]])),
        ..Default::default()
    };
    paint_brush(&mut d, 0, 0, (2, 0), [255; 4], 1, &o);
    assert_eq!(d.composite(0)[2], [123, 45, 67, 128]);
    pixelcraft::workflows::transform(
        &mut d,
        0,
        0,
        [0, 0, 1, 1],
        None,
        pixelcraft::workflows::Transform::Move(1, 0),
    )
    .unwrap();
    assert_eq!(d.composite(0)[1], [123, 45, 67, 128]);
}
#[test]
fn jpeg_reference_import_decodes_and_artwork_export_omits_it() {
    let root = std::env::temp_dir().join(format!("pixelcraft-jpeg-ref-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let jpeg = root.join("guide.jpg");
    image::RgbImage::from_pixel(2, 2, image::Rgb([200, 100, 30]))
        .save(&jpeg)
        .unwrap();
    let r = import_reference(&jpeg).unwrap();
    assert_eq!((r.width, r.height), (2, 2));
    let mut d = Document::new(2, 2);
    d.references.push(r);
    let png = root.join("art.png");
    pixelcraft::formats::export_png(&png, &d, 0, false).unwrap();
    assert!(
        image::open(png)
            .unwrap()
            .to_rgba8()
            .pixels()
            .all(|p| p.0[3] == 0)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn regression_palette_import_preserves_transparent_and_white_pixels() {
    let root = std::env::temp_dir().join(format!("palette-alpha-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255; 4]];
    d.set_pixel(0, 0, 1, [255; 4]);
    d.set_color_mode(ColorMode::Indexed);
    pixelcraft::formats::save(&root.join("in.pixel"), &d).unwrap();
    std::fs::write(root.join("colors.hex"), "ffffff\n000000\n").unwrap();
    operation("palette-import", &serde_json::json!({"input":root.join("in.pixel"),"output":root.join("out.pixel"),"palette":root.join("colors.hex")})).unwrap();
    let out = pixelcraft::formats::load(&root.join("out.pixel")).unwrap();
    assert_eq!(out.composite(0), vec![CLEAR, [255; 4]]);
    assert_ne!(
        out.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
        out.layers[0].cel_meta[0].indices.as_ref().unwrap()[1]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn regression_ase_save_rejects_reference_loss() {
    let mut d = Document::new(1, 1);
    d.references.push(ReferenceImage {
        name: "guide".into(),
        width: 1,
        height: 1,
        pixels: vec![[255; 4]].into(),
        x: 0.,
        y: 0.,
        scale: 1.,
        opacity: 255,
        visible: true,
    });
    for ext in ["ase", "aseprite"] {
        let path =
            std::env::temp_dir().join(format!("reference-save-{}.{}", std::process::id(), ext));
        std::fs::write(&path, b"existing file").unwrap();
        assert!(pixelcraft::formats::save(&path, &d).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing file");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn regression_wrapped_shade_dab_steps_each_pixel_once() {
    for mode in [ColorMode::Rgba, ColorMode::Indexed] {
        for reverse in [false, true] {
            let mut d = Document::new(2, 1);
            d.palette = vec![
                CLEAR,
                [10, 0, 0, 255],
                [20, 0, 0, 255],
                [30, 0, 0, 255],
                [40, 0, 0, 255],
            ];
            d.layers[0].cels[0] = vec![[20, 0, 0, 255]; 2].into();
            d.set_color_mode(mode);
            d.wraparound = true;
            d.brush_shape = BrushShape::Square;
            paint_brush(
                &mut d,
                0,
                0,
                (0, 0),
                CLEAR,
                8,
                &BrushOptions {
                    mode: BrushMode::Shade,
                    reverse,
                    ..Default::default()
                },
            );
            assert_eq!(
                d.composite(0),
                vec![
                    if reverse {
                        [10, 0, 0, 255]
                    } else {
                        [30, 0, 0, 255]
                    };
                    2
                ]
            );
        }
    }
}

#[test]
fn regression_maxrects_checks_packed_extent_not_search_area() {
    let d = Document::new(1, 4000);
    let (image, _) = pack_atlas(
        &[("narrow".into(), d)],
        &AtlasOptions {
            width: 16384,
            trim: false,
            padding: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(image.dimensions(), (1, 4000));
}

#[test]
fn regression_large_ping_pong_gif_streams_repeated_frames() {
    use image::AnimationDecoder;
    let mut d = Document::new(1024, 1024);
    for _ in 1..33 {
        d.add_frame(0, true);
    }
    d.tags.push(FrameTag {
        name: "ping".into(),
        from: 0,
        to: 32,
        direction: 2,
        repeat: 2,
        color: [0; 3],
    });
    d.durations[1] = 230;
    d.set_pixel(0, 1, 0, [255, 0, 0, 255]);
    assert!(d.validate().is_ok());
    let root = std::env::temp_dir().join(format!("large-ping-{}", std::process::id()));
    let paths = batch_tags(&root, &d, true, &Default::default()).unwrap();
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(&paths[0]).unwrap(),
    ))
    .unwrap();
    let mut count = 0;
    for frame in decoder.into_frames() {
        let frame = frame.unwrap();
        if count == 1 || count == 63 {
            assert_eq!(frame.delay().numer_denom_ms(), (230, 1));
            assert_eq!(frame.buffer().get_pixel(0, 0).0, [255, 0, 0, 255]);
        }
        count += 1;
    }
    assert_eq!(count, 64);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "Requires loopback network access"]
fn regression_live_bridge_slow_read_deadline_and_revocation() {
    use std::{
        io::{Read, Write},
        time::{Duration, Instant},
    };
    let config = std::env::temp_dir().join(format!("bridge-slow-{}.json", std::process::id()));
    let bridge = LiveBridge::start(config.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    let address = v["address"].as_str().unwrap().to_string();
    let mut slow = std::net::TcpStream::connect(&address).unwrap();
    let sender = std::thread::spawn(move || {
        for _ in 0..35 {
            if slow.write_all(b" ").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let mut valid = std::net::TcpStream::connect(&address).unwrap();
    writeln!(
        valid,
        "{}",
        serde_json::json!({"token":v["token"],"operation":"live-inspect","arguments":{}})
    )
    .unwrap();
    let result = bridge.requests.recv_timeout(Duration::from_secs(3));
    assert!(
        result.is_ok(),
        "byte-at-a-time input blocked authenticated client"
    );
    result
        .unwrap()
        .response
        .send(Ok(serde_json::json!({"ok":true})))
        .unwrap();
    // Revocation must close the active read, not just remove its config file.
    let mut active = std::net::TcpStream::connect(&address).unwrap();
    active.write_all(b"{").unwrap();
    active
        .set_read_timeout(Some(Duration::from_millis(600)))
        .unwrap();
    std::thread::sleep(Duration::from_millis(80));
    let start = Instant::now();
    drop(bridge);
    assert!(start.elapsed() < Duration::from_millis(500));
    let mut byte = [0];
    assert!(matches!(active.read(&mut byte), Ok(0)));
    assert!(!config.exists());
    sender.join().unwrap();
}

#[test]
fn regression_partial_legacy_and_starter_shortcuts_keep_defaults() {
    for bytes in [
        br#"{"P":"pencil"}"#.as_slice(),
        include_bytes!("../examples/pixelcraft-starter/shortcuts.json").as_slice(),
    ] {
        let keys = load_shortcuts(bytes).unwrap();
        assert_eq!(keys.get("Ctrl+O").map(String::as_str), Some("open"));
        assert_eq!(keys.get("Ctrl+V").map(String::as_str), Some("paste"));
        assert_eq!(keys.get("P").map(String::as_str), Some("pencil"));
    }
}

#[test]
fn regression_sparse_import_palette_never_turns_opaque_pixels_transparent() {
    for palette in [vec![[255; 4]], vec![[255, 255, 255, 0], [0, 0, 0, 255]]] {
        let mut d = Document::new(2, 1);
        d.palette = vec![CLEAR, [0, 0, 0, 255]];
        d.set_pixel(0, 0, 1, [0, 0, 0, 255]);
        d.set_color_mode(ColorMode::Indexed);
        // Transparent pixels can retain hidden RGB in imported source documents.
        d.layers[0].cels[0][0] = [255, 255, 255, 0];
        d.import_palette(palette).unwrap();
        assert_eq!(d.composite(0)[0][3], 0);
        assert_eq!(d.composite(0)[1][3], 255);
    }
}

#[test]
fn shortcut_removals_survive_default_merging() {
    let keys =
        load_shortcuts(br#"{"Ctrl+O":null,"B":null,"P":"pencil","Ctrl+S":"saveas"}"#).unwrap();
    assert!(!keys.contains_key("Ctrl+O"));
    assert!(!keys.contains_key("B"));
    assert_eq!(keys.get("Ctrl+V").map(String::as_str), Some("paste"));
    assert_eq!(keys.get("P").map(String::as_str), Some("pencil"));
    assert_eq!(keys.get("Ctrl+S").map(String::as_str), Some("saveas"));
}

#[test]
fn saved_shortcuts_preserve_removals_and_rebinding() {
    let mut keys = load_shortcuts(br#"{"P":"pencil"}"#).unwrap();
    keys.remove("Ctrl+O");
    keys.remove("B");
    let saved = save_shortcuts(&keys).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&saved).unwrap()["Ctrl+O"],
        serde_json::Value::Null
    );
    assert_eq!(load_shortcuts(&saved).unwrap(), keys);
    keys.insert("Ctrl+O".into(), "saveas".into());
    assert_eq!(
        load_shortcuts(&save_shortcuts(&keys).unwrap()).unwrap(),
        keys
    );
    let empty = std::collections::BTreeMap::new();
    assert!(
        load_shortcuts(&save_shortcuts(&empty).unwrap())
            .unwrap()
            .is_empty()
    );
}
