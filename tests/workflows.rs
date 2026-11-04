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
fn extension_flip_and_color_commands_are_transactional() {
    let mut d = Document::new(3, 2);
    d.set_pixel(0, 0, 0, [10, 20, 30, 255]);
    let out = pixelcraft::extensions::run_source(
        &d,
        r#"
 app.command.Flip{target='mask',orientation='horizontal'}
 assert(app.pixelColor.rgbaR(app.activeImage:getPixel(2,0))==10)
 app.command.InvertColor{channels='rgb'}
 assert(app.pixelColor.rgbaR(app.activeImage:getPixel(2,0))==245)
 app.command.Undo()
 assert(app.pixelColor.rgbaR(app.activeImage:getPixel(2,0))==10)
 app.command.ReplaceColor{from=Color{r=10,g=20,b=30},to=Color{r=200,g=100,b=50},tolerance=0}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(out.layers[0].cels[0][2], [200, 100, 50, 255]);
    assert_eq!(out.layers[0].cels[0][0], CLEAR);
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
fn engine_presets_write_resources_with_animation_timing() {
    let root = std::env::temp_dir().join(format!("pixelcraft-engines-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut d = Document::new(2, 2);
    d.add_frame(0, false);
    d.durations = vec![70, 230];
    d.tags.push(FrameTag {
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
