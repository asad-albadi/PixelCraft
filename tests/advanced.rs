use pixelcraft::{formats, model::*};
#[test]
fn large_canvas_and_shared_history_are_independent() {
    let mut d = Document::new(2048, 2);
    let before = d.clone();
    assert!(d.layers[0].cels[0].shares_storage(&before.layers[0].cels[0]));
    d.stamp(0, 0, 1500, 1, [10, 20, 30, 255], 1);
    assert_eq!(before.composite(0)[3548], CLEAR);
    assert_eq!(d.composite(0)[3548], [10, 20, 30, 255]);
}
#[test]
fn color_modes_constrain_drawing_and_palette_changes_recolor_indices() {
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [0, 0, 255, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.stamp(0, 0, 0, 0, [230, 10, 10, 255], 1);
    assert_eq!(d.composite(0)[0], [255, 0, 0, 255]);
    d.set_palette_color(1, [0, 255, 0, 255]);
    assert_eq!(d.composite(0)[0], [0, 255, 0, 255]);
    d.set_color_mode(ColorMode::Grayscale);
    d.stamp(0, 0, 1, 0, [250, 0, 0, 200], 1);
    let p = d.composite(0)[1];
    assert_eq!(p[0], p[1]);
    assert_eq!(p[1], p[2]);
    assert_eq!(p[3], 200);
}
#[test]
fn groups_blend_and_cel_opacity_render_together() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [200, 100, 50, 255];
    let g = d.add_group("Shadows").unwrap();
    d.add_layer();
    let l = d.layers.len() - 1;
    d.layers[l].parent = Some(g);
    d.layers[g].blend = BlendMode::Multiply;
    d.layers[l].cels[0][0] = [128, 255, 255, 255];
    d.layers[l].cel_meta[0].opacity = 128;
    assert!((149..=151).contains(&d.composite(0)[0][0]));
    d.layers[g].visible = false;
    assert_eq!(d.composite(0)[0], [200, 100, 50, 255]);
}
#[test]
fn tiles_and_ranges_survive_native_project_roundtrip() {
    let mut d = Document::new(4, 2);
    d.tilesets.push(Tileset {
        name: "Tiles".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![[25, 50, 75, 255]; 4].into()],
    });
    d.add_tile_layer(0).unwrap();
    let l = d.layers.len() - 1;
    d.paint_tile(l, 0, 1, 0, 1).unwrap();
    d.tags.push(FrameTag {
        name: "Idle".into(),
        from: 0,
        to: 0,
        direction: 0,
        repeat: 0,
        color: [255, 0, 0],
    });
    d.add_frame(0, true);
    assert_eq!(d.tags[0].to, 1);
    assert_eq!(d.composite(1)[2], [25, 50, 75, 255]);
    let path = std::env::temp_dir().join(format!("pw-native-{}.pixel", std::process::id()));
    formats::save(&path, &d).unwrap();
    assert_eq!(formats::load(&path).unwrap(), d);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn square_diamond_brushes_and_small_ellipses_are_symmetric() {
    let mut d = Document::new(8, 8);
    d.brush_shape = BrushShape::Square;
    d.stamp(0, 0, 4, 4, [1, 2, 3, 255], 3);
    assert_eq!(d.composite(0).iter().filter(|p| p[3] > 0).count(), 9);
    let mut d = Document::new(8, 8);
    d.ellipse(0, 0, (2, 2), (5, 5), [1, 2, 3, 255], false, 1);
    let p = d.composite(0);
    for y in 2..=5 {
        for x in 2..=5 {
            assert_eq!(p[y * 8 + x], p[y * 8 + 7 - x]);
            assert_eq!(p[y * 8 + x], p[(7 - y) * 8 + x]);
        }
    }
    assert!(p.iter().any(|p| p[3] > 0));
}
#[test]
fn nested_group_cycle_is_rejected_and_reordering_keeps_parent_identity() {
    let mut d = Document::new(2, 2);
    let g = d.add_group("Parent").unwrap();
    d.layers[0].parent = Some(g);
    d.swap_layers(0, g);
    assert_eq!(d.layers[1].parent, Some(0));
    assert!(d.validate().is_ok());
    d.layers[0].parent = Some(0);
    assert!(d.validate().is_err());
}
#[test]
fn every_blend_mode_respects_transparent_inputs() {
    for mode in BlendMode::ALL {
        assert_eq!(
            blend([12, 34, 56, 255], CLEAR, 255, mode),
            [12, 34, 56, 255]
        );
        assert_eq!(
            blend(CLEAR, [12, 34, 56, 255], 255, mode),
            [12, 34, 56, 255]
        );
    }
}
#[test]
fn indexed_palette_slots_stay_distinct_even_when_colors_match() {
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    d.layers[0].cels[0][1] = [255, 0, 0, 255];
    d.layers[0].cel_meta[0].indices = Some(vec![1, 2].into());
    d.set_palette_color(2, [0, 255, 0, 255]);
    assert_eq!(d.composite(0), vec![[255, 0, 0, 255], [0, 255, 0, 255]]);
}
#[test]
fn cel_order_ties_are_resolved_by_z_index() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    d.layers[0].cel_meta[0].z_index = 1;
    d.add_layer();
    d.layers[1].cels[0][0] = [0, 0, 255, 255];
    assert_eq!(d.composite(0), vec![[255, 0, 0, 255]]);
}
