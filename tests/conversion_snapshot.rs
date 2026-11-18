use pixelcraft::{contributions::DitheringMatrix, model::*};

#[test]
fn conversion_snapshot_keeps_frame_palettes_hidden_neighbors_and_sampled_positions() {
    let mut doc = Document::new(4, 2);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.layers[0].cels[0] = vec![[128, 128, 128, 255]; 8].into();
    doc.layers[0].cel_meta[0].outside = vec![OutsidePixel {
        x: -1,
        y: -1,
        color: [128, 128, 128, 255],
        palette_slot: None,
    }];
    doc.layers[0].cel_meta[0].bounds = Some([-1, -1, 5, 3]);
    doc.add_frame(0, true);
    doc.add_frame(1, true);
    doc.set_cel_keyframe(
        0,
        0,
        Some(CelKeyframe {
            x: 0,
            y: 0,
            opacity: 255,
            hold: false,
        }),
    )
    .unwrap();
    doc.set_cel_keyframe(
        0,
        2,
        Some(CelKeyframe {
            x: 2,
            y: 0,
            opacity: 100,
            hold: false,
        }),
    )
    .unwrap();
    doc.frame_palettes = vec![
        doc.palette.clone(),
        vec![CLEAR, [0, 0, 0, 255], [255, 0, 0, 255]],
    ];
    doc.layers[0].cels[1] = vec![[128, 0, 0, 255]; 8].into();
    doc.layers[0].cel_meta[1].outside[0].color = [128, 0, 0, 255];
    let snapshot = doc.conversion_frame_snapshot(1).unwrap();
    assert_eq!(snapshot.durations.len(), 1);
    assert_eq!(
        snapshot.layers[0].cels[0].backing_id(),
        doc.layers[0].cels[1].backing_id()
    );
    assert_eq!(snapshot.palette, doc.palette);
    assert_eq!(snapshot.palette_for(0), doc.palette_for(1));
    assert_eq!(snapshot.sampled_cel_key(0, 0), doc.sampled_cel_key(0, 1));
    assert_eq!(snapshot.composite(0), doc.composite(1));
    let matrix = DitheringMatrix::bayer(2).unwrap();
    for fit in IndexedFit::ALL {
        for dither in [
            IndexedDithering::None,
            IndexedDithering::Ordered(&matrix),
            IndexedDithering::FloydSteinberg,
        ] {
            let full = doc
                .indexed_conversion_with_fit_until(None, 0, dither, fit, || false)
                .unwrap();
            let preview = snapshot
                .indexed_conversion_with_fit_until(None, 0, dither, fit, || false)
                .unwrap();
            assert_eq!(preview.layers[0].cels[0], full.layers[0].cels[1]);
            assert_eq!(
                preview.layers[0].cel_meta[0].outside,
                full.layers[0].cel_meta[1].outside
            );
            assert_eq!(preview.composite(0), full.composite(1));
        }
    }
    for method in [
        GrayscaleMethod::Luminance,
        GrayscaleMethod::HsvValue,
        GrayscaleMethod::HslLightness,
    ] {
        assert_eq!(
            snapshot
                .grayscale_conversion_until(method, || false)
                .unwrap()
                .composite(0),
            doc.grayscale_conversion_until(method, || false)
                .unwrap()
                .composite(1)
        );
    }
    assert!(doc.conversion_frame_snapshot(3).is_err());
}

#[test]
fn indexed_frame_snapshot_and_tile_conversion_match_full_animation() {
    let mut doc = Document::new(2, 1);
    doc.palette = vec![CLEAR, [200, 100, 20, 137], [255; 4]];
    doc.layers[0].cels[0] = vec![[200, 100, 20, 137]; 2].into();
    doc.set_color_mode(ColorMode::Indexed);
    doc.add_frame(0, true);
    doc.link_cel(0, 1, 0).unwrap();
    doc.frame_palettes = vec![
        doc.palette.clone(),
        vec![CLEAR, [20, 100, 200, 100], [255; 4]],
    ];
    doc.layers[0].cels[1] = vec![[20, 100, 200, 100]; 2].into();
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "base palette".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[200, 100, 20, 137]].into()],
    });
    let mut tilemap = Layer::new("placed tile".into(), 2, 1, 2);
    tilemap.tileset = Some(0);
    tilemap.tile_cels = vec![vec![0, 1].into(), vec![1, 0].into()];
    doc.layers.push(tilemap);
    let snapshot = doc.conversion_frame_snapshot(1).unwrap();
    assert!(snapshot.layers[0].cel_meta[0].artwork_link.is_none());
    let full = doc.rgba_conversion_until(|| false).unwrap();
    let preview = snapshot.rgba_conversion_until(|| false).unwrap();
    assert_eq!(preview.composite(0), full.composite(1));
    let full = doc
        .indexed_conversion_with_fit_until(
            None,
            0,
            IndexedDithering::FloydSteinberg,
            IndexedFit::Lab,
            || false,
        )
        .unwrap();
    let preview = snapshot
        .indexed_conversion_with_fit_until(
            None,
            0,
            IndexedDithering::FloydSteinberg,
            IndexedFit::Lab,
            || false,
        )
        .unwrap();
    assert_eq!(preview.tilesets[0].tiles, full.tilesets[0].tiles);
    assert_eq!(preview.composite(0), full.composite(1));
    let mut broken = doc.clone();
    broken.layers[0].cel_meta.clear();
    assert!(broken.conversion_frame_snapshot(1).is_err());
}

#[test]
fn conversion_snapshot_renders_groups_masks_reference_geometry_and_tilemaps_identically() {
    let mut doc = Document::new(4, 2);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.layers[0].cels[0] = vec![[180, 90, 30, 200]; 8].into();
    doc.add_frame(0, true);
    let group = doc.add_group("group").unwrap();
    doc.layers[group].opacity = 173;
    doc.layers[0].parent = Some(group);
    doc.set_layer_mask(0, Some(vec![[0, 0, 0, 128]; 8].into()))
        .unwrap();
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "tiles".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[90, 130, 70, 220]].into()],
    });
    let mut tiles = Layer::new("tilemap".into(), 4, 2, 2);
    tiles.parent = Some(group);
    tiles.tileset = Some(0);
    for frame in 0..2 {
        tiles.tile_cels[frame] = vec![1, 1, 0, 0, 0, 0, 0, 1].into();
        tiles.cel_meta[frame].tilemap = Some(TilemapGeometry {
            x: -1,
            y: 0,
            columns: 4,
            rows: 2,
        });
    }
    doc.layers.push(tiles);
    let mut reference = Layer::new("reference".into(), 4, 2, 2);
    reference.ase_flags = 64;
    reference.parent = Some(group);
    for frame in 0..2 {
        reference.cels[frame] = vec![[20, 100, 200, 137]; 8].into();
        reference.cel_meta[frame].bounds = Some([0, 0, 2, 1]);
        reference.cel_meta[frame].reference_bounds = Some([65_536, 0, 2 * 65_536, 65_536]);
    }
    doc.layers.push(reference);
    doc.color_profile = vec![1, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    doc.validate().unwrap();
    let snapshot = doc.conversion_frame_snapshot(1).unwrap();
    assert_eq!(snapshot.composite_view(0), doc.composite_view(1));
    let full = doc
        .grayscale_conversion_until(GrayscaleMethod::HslLightness, || false)
        .unwrap();
    let preview = snapshot
        .grayscale_conversion_until(GrayscaleMethod::HslLightness, || false)
        .unwrap();
    assert_eq!(preview.composite_view(0), full.composite_view(1));
    assert_eq!(
        pixelcraft::color::display_pixels(&preview, 0).unwrap(),
        pixelcraft::color::display_pixels(&full, 1).unwrap()
    );
    let full = doc
        .indexed_conversion_with_fit_until(
            None,
            0,
            IndexedDithering::FloydSteinberg,
            IndexedFit::Lab,
            || false,
        )
        .unwrap();
    let preview = snapshot
        .indexed_conversion_with_fit_until(
            None,
            0,
            IndexedDithering::FloydSteinberg,
            IndexedFit::Lab,
            || false,
        )
        .unwrap();
    assert_eq!(preview.composite_view(0), full.composite_view(1));
}

#[test]
#[ignore = "manual large-animation conversion timing"]
fn benchmark_active_frame_conversion_against_full_animation() {
    let mut doc = Document::new(480, 270);
    for _ in 1..24 {
        doc.add_frame(doc.durations.len() - 1, false);
    }
    for _ in 1..8 {
        doc.add_layer();
    }
    for (layer_index, layer) in doc.layers.iter_mut().enumerate() {
        for (frame, cel) in layer.cels.iter_mut().enumerate() {
            let mut pixels = vec![[200, 100, 20, 255]; 480 * 270];
            let changed = (frame * 17 + layer_index) % pixels.len();
            pixels[changed] = [20, 100, 200, 137];
            *cel = pixels.into();
        }
    }
    doc.validate().unwrap();
    let start = std::time::Instant::now();
    let full = doc
        .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
        .unwrap();
    let full_time = start.elapsed();
    let start = std::time::Instant::now();
    let preview = doc
        .conversion_frame_snapshot(23)
        .unwrap()
        .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
        .unwrap();
    let preview_time = start.elapsed();
    assert_eq!(preview.composite(0), full.composite(23));
    eprintln!(
        "debug native conversion, 480x270, 8 layers, 24 unique animation frames: full={full_time:?}, frame snapshot+conversion={preview_time:?}, ratio={:.2}x",
        full_time.as_secs_f64() / preview_time.as_secs_f64()
    );
}

#[test]
#[ignore = "manual conversion timing on supplied saved projects"]
fn benchmark_saved_projects_conversion_preview() {
    let paths: Vec<String> = serde_json::from_str(
        &std::env::var("PIXELCRAFT_CONVERSION_BENCH_PROJECTS")
            .expect("Set a JSON array of project paths"),
    )
    .unwrap();
    for path in paths {
        let doc = pixelcraft::formats::load(std::path::Path::new(&path)).unwrap();
        let started = std::time::Instant::now();
        let full = doc
            .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap();
        let full_time = started.elapsed();
        for frame in [0, doc.durations.len() / 2, doc.durations.len() - 1] {
            let started = std::time::Instant::now();
            let preview = doc
                .conversion_frame_snapshot(frame)
                .unwrap()
                .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
                .unwrap();
            let frame_time = started.elapsed();
            assert_eq!(
                preview.composite(0),
                full.composite(frame),
                "{} frame {}",
                path,
                frame
            );
            eprintln!(
                "saved project {}, {}x{}, {} layers, {} frames, selected frame {}: full={full_time:?}, snapshot+conversion={frame_time:?}",
                std::path::Path::new(&path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy(),
                doc.width,
                doc.height,
                doc.layers.len(),
                doc.durations.len(),
                frame
            );
        }
    }
}

#[test]
fn frame_conversion_snapshot_keeps_only_referenced_tiles_and_preserves_flags() {
    let mut doc = Document::new(4, 1);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.add_frame(0, false);
    doc.tilesets = vec![
        Tileset {
            editor_id: Default::default(),
            name: "unused".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![[128, 128, 128, 255]; 2].into(); 1024],
        },
        Tileset {
            editor_id: Default::default(),
            name: "shared".into(),
            width: 2,
            height: 1,
            tiles: vec![
                vec![[200, 20, 10, 255], [100, 10, 5, 137]].into(),
                vec![[0, 255, 0, 255]; 2].into(),
                vec![[10, 20, 200, 255], [5, 10, 100, 100]].into(),
            ],
        },
    ];
    doc.layers[0].tileset = Some(1);
    doc.layers[0].tile_cels = vec![
        vec![3 | 0x8000_0000, 1 | 0x4000_0000].into(),
        vec![2, 2].into(),
    ];
    let mut other = Layer::new("other placement".into(), 4, 1, 2);
    other.tileset = Some(1);
    other.tile_cels = vec![vec![3, 0].into(), vec![2, 2].into()];
    doc.layers.push(other);
    doc.ase_tilesets = vec![AseTilesetInfo {
        index: 1,
        id: 71,
        flags: 2,
        base_index: 5,
        external: None,
    }];
    doc.validate().unwrap();
    let original = doc.clone();
    let snapshot = doc.conversion_frame_snapshot(0).unwrap();
    assert_eq!(
        snapshot.tilesets.len(),
        1,
        "unreferenced sets must not be converted"
    );
    assert_eq!(
        snapshot.tilesets[0].tiles.len(),
        2,
        "other-frame-only definitions must not be converted"
    );
    assert_eq!(snapshot.layers[0].tileset, Some(0));
    assert_eq!(snapshot.layers[1].tileset, Some(0));
    assert_eq!(
        snapshot.layers[0].tile_cels[0],
        vec![2 | 0x8000_0000, 1 | 0x4000_0000]
    );
    assert_eq!(snapshot.layers[1].tile_cels[0], vec![2, 0]);
    assert_eq!(
        snapshot.tilesets[0].tiles[1].backing_id(),
        doc.tilesets[1].tiles[2].backing_id()
    );
    assert_eq!(snapshot.ase_tilesets[0].index, 0);
    assert_eq!(snapshot.ase_tilesets[0].id, 71);
    assert_eq!(snapshot.composite_view(0), doc.composite_view(0));
    let matrix = DitheringMatrix::bayer(2).unwrap();
    for fit in IndexedFit::ALL {
        for mode in [
            IndexedDithering::None,
            IndexedDithering::Ordered(&matrix),
            IndexedDithering::FloydSteinberg,
        ] {
            let full = doc
                .indexed_conversion_with_fit_until(None, 0, mode, fit, || false)
                .unwrap();
            let preview = snapshot
                .indexed_conversion_with_fit_until(None, 0, mode, fit, || false)
                .unwrap();
            assert_eq!(preview.composite_view(0), full.composite_view(0));
        }
    }
    assert_eq!(
        snapshot
            .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap()
            .composite_view(0),
        doc.grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap()
            .composite_view(0)
    );
    assert_eq!(
        snapshot
            .rgba_conversion_until(|| false)
            .unwrap()
            .composite_view(0),
        doc.rgba_conversion_until(|| false)
            .unwrap()
            .composite_view(0)
    );
    assert_eq!(doc, original);
}

#[test]
fn frame_snapshot_keeps_empty_tilemaps_and_rejects_missing_definitions() {
    let mut doc = Document::new(1, 1);
    doc.tilesets = vec![
        Tileset {
            editor_id: Default::default(),
            name: "unused".into(),
            width: 1,
            height: 1,
            tiles: vec![],
        },
        Tileset {
            editor_id: Default::default(),
            name: "empty map".into(),
            width: 1,
            height: 1,
            tiles: vec![vec![[255; 4]].into(), vec![[0, 0, 0, 255]].into()],
        },
    ];
    doc.layers[0].tileset = Some(1);
    doc.layers[0].tile_cels[0] = vec![0x8000_0000].into();
    let snapshot = doc.conversion_frame_snapshot(0).unwrap();
    assert_eq!(snapshot.tilesets.len(), 1);
    assert!(snapshot.tilesets[0].tiles.is_empty());
    assert_eq!(snapshot.layers[0].tile_cels[0], vec![0x8000_0000]);
    snapshot.validate().unwrap();
    assert_eq!(snapshot.composite_view(0), doc.composite_view(0));
    doc.layers[0].tile_cels[0][0] = 3;
    assert!(doc.conversion_frame_snapshot(0).is_err());
    doc.layers[0].tileset = Some(99);
    assert!(doc.conversion_frame_snapshot(0).is_err());
}

#[test]
#[ignore = "manual unused-tile conversion timing"]
fn benchmark_referenced_tile_conversion_preview() {
    let mut doc = Document::new(64, 64);
    for set in 0..2 {
        let tiles = (0..512)
            .map(|tile| {
                let mut pixels = vec![[180, 90, 30, 200]; 64 * 64];
                pixels[(tile + set) % (64 * 64)] = [20, 100, 200, 137];
                pixels.into()
            })
            .collect();
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: format!("set {set}"),
            width: 64,
            height: 64,
            tiles,
        });
    }
    doc.layers[0].tileset = Some(1);
    doc.layers[0].tile_cels[0] = vec![301 | 0x8000_0000].into();
    doc.validate().unwrap();
    let started = std::time::Instant::now();
    let full = doc
        .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
        .unwrap();
    let full_time = started.elapsed();
    let started = std::time::Instant::now();
    let snapshot = doc.conversion_frame_snapshot(0).unwrap();
    assert_eq!(snapshot.tilesets.len(), 1);
    assert_eq!(snapshot.tilesets[0].tiles.len(), 1);
    let preview = snapshot
        .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
        .unwrap();
    let preview_time = started.elapsed();
    assert_eq!(preview.composite_view(0), full.composite_view(0));
    eprintln!(
        "debug native conversion, 1024 unique 64x64 definitions, 1 placed: full={full_time:?}, referenced snapshot+conversion={preview_time:?}, ratio={:.2}x",
        full_time.as_secs_f64() / preview_time.as_secs_f64()
    );
}
