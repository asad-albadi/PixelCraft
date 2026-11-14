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

#[test]
fn empty_animation_cels_use_no_pixel_budget() {
    let mut doc = Document::new(2048, 1024);
    for frame in 0..119 {
        doc.add_frame(frame, false);
    }
    assert_eq!(doc.durations.len(), 120);
    assert_eq!(doc.stored_pixels(), 0);
    assert_eq!(doc.layers[0].cels[119][2047], CLEAR);
    doc.validate().unwrap();
}

#[test]
fn isolated_pixels_stay_sparse_and_round_trip() {
    let mut doc = Document::new(2048, 1024);
    doc.set_pixel(0, 0, 20, [19, 32, 50, 255]);
    doc.set_pixel(0, 0, 2_000_000, [7, 8, 9, 0]);
    assert!(doc.stored_pixels() <= 48);
    let reopened = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(reopened, doc);
    assert_eq!(reopened.layers[0].cels[0][2_000_000], [7, 8, 9, 0]);
}

#[test]
fn linked_edits_survive_project_and_ase_round_trips() {
    let color = [17, 65, 125, 255];
    let mut doc = Document::new(16, 16);
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();
    let before = doc.clone();
    doc.stamp(0, 1, 3, 4, color, 1);
    assert_eq!(doc.composite(0)[67], color);
    assert_eq!(before.composite(0)[67], CLEAR);
    let native = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    let ase = formats::decode_ase(&formats::encode_ase(&doc).unwrap()).unwrap();
    for mut reopened in [native, ase] {
        assert_eq!(reopened.composite(0), doc.composite(0));
        assert_eq!(reopened.composite(1), doc.composite(1));
        reopened.set_pixel(0, 0, 68, color);
        assert_eq!(reopened.composite(1)[68], color);
        reopened.unlink_cel(0, 1).unwrap();
        reopened.set_pixel(0, 0, 68, CLEAR);
        assert_eq!(reopened.composite(1)[68], color);
    }
}

#[test]
fn timeline_move_is_atomic_when_overlapping_or_blocked() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let mut doc = Document::new(2, 2);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.add_layer();
    doc.set_pixel(0, 0, 0, red);
    doc.set_pixel(0, 1, 0, blue);
    let original = doc.clone();
    let range = CelRange {
        first_layer: 0,
        last_layer: 0,
        first_frame: 0,
        last_frame: 1,
    };
    doc.layers[1].locked = true;
    let locked = doc.clone();
    assert!(doc.move_cel_range(range, 1, 0, false).is_err());
    assert_eq!(doc, locked);
    doc.layers[1].locked = false;
    doc.move_cel_range(range, 0, 1, false).unwrap();
    assert_eq!(doc.composite(0)[0], CLEAR);
    assert_eq!(doc.composite(1)[0], red);
    assert_eq!(doc.composite(2)[0], blue);
    assert_eq!(original.composite(1)[0], blue);
}

#[test]
fn scoped_adjustment_roundtrip_keeps_source_pixels() {
    let mut doc = Document::new(2, 1);
    let source = [70, 80, 90, 255];
    let target = [150, 160, 170, 255];
    doc.set_pixel(0, 0, 0, source);
    doc.set_pixel(0, 0, 1, source);
    doc.add_layer();
    doc.layers[1].adjustment = Some(Adjustment::Palette {
        colors: vec![(source, target)],
    });
    doc.layers[1].mask = Some(Pixels::from(vec![[0, 0, 0, 255], CLEAR]));
    assert_eq!(doc.composite(0), vec![target, source]);
    let reopened = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(reopened.layers[0].cels[0][0], source);
    assert_eq!(reopened.composite(0), vec![target, source]);
    let baked = formats::decode_ase(&formats::encode_ase(&doc).unwrap()).unwrap();
    assert_eq!(baked.composite(0), vec![target, source]);
}

#[test]
fn cel_keyframes_sample_signed_positions_and_opacity() {
    let mut doc = Document::new(6, 2);
    doc.set_pixel(0, 0, 0, [30, 45, 55, 255]);
    doc.add_frame(0, true);
    doc.add_frame(1, true);
    doc.set_cel_keyframe(
        0,
        0,
        Some(CelKeyframe {
            x: -2,
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
            x: 4,
            y: 0,
            opacity: 127,
            hold: false,
        }),
    )
    .unwrap();
    assert_eq!(doc.composite(0)[0], CLEAR);
    assert_eq!(doc.composite(1)[1], [30, 45, 55, 191]);
    assert_eq!(doc.composite(2)[4], [30, 45, 55, 127]);
    let reopened = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(reopened.composite(1), doc.composite(1));
}

#[test]
fn native_v1_dense_cels_remain_readable() {
    use std::io::Write;
    let mut doc = Document::new(2, 2);
    doc.layers[0].cels[0] = vec![[9, 18, 27, 255]; 4].into();
    let mut encoded = flate2::write::ZlibEncoder::new(
        b"PIXEL-WORKSHOP\0\x01".to_vec(),
        flate2::Compression::fast(),
    );
    encoded
        .write_all(&serde_json::to_vec(&doc).unwrap())
        .unwrap();
    let reopened = formats::decode_project(&encoded.finish().unwrap()).unwrap();
    assert_eq!(reopened, doc);
}

#[test]
fn project_writes_shared_artwork_once_and_keeps_placements_independent() {
    let mut doc = Document::new(64, 64);
    for frame in 0..39 {
        doc.add_frame(frame, false);
    }
    let mut data = 37u32;
    for pixel in doc.layers[0].cels[0].iter_mut() {
        data = data.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let [r, g, b, _] = data.to_le_bytes();
        *pixel = [r, g, b, 255];
    }
    for frame in 1..40 {
        doc.link_cel(0, frame, 0).unwrap();
    }
    let encoded = formats::encode_project(&doc).unwrap();
    assert!(encoded.len() < 100_000);
    let opened = formats::decode_project(&encoded).unwrap();
    assert_eq!(opened, doc);
    assert!(opened.layers[0].cels[0].shares_storage(&opened.layers[0].cels[39]));

    let mut placed = Document::new(8, 8);
    placed.add_frame(0, false);
    placed.layers[0].cel_meta[0].bounds = Some([1, 1, 2, 2]);
    placed.layers[0].cel_meta[1].bounds = Some([3, 2, 2, 2]);
    placed.set_pixel(0, 0, 9, [1, 2, 3, 255]);
    placed.link_cel(0, 1, 0).unwrap();
    assert_eq!(placed.composite(1)[19], [1, 2, 3, 255]);
    let reopened = formats::decode_ase(&formats::encode_ase(&placed).unwrap()).unwrap();
    assert_eq!(reopened.composite(1), placed.composite(1));
}

#[test]
fn interpolated_cels_follow_the_loop_boundary() {
    let mut doc = Document::new(8, 8);
    for frame in 0..3 {
        doc.add_frame(frame, false);
    }
    doc.set_cel_keyframe(
        0,
        1,
        Some(CelKeyframe {
            x: 0,
            ..Default::default()
        }),
    )
    .unwrap();
    doc.set_cel_keyframe(
        0,
        3,
        Some(CelKeyframe {
            x: 6,
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(doc.sampled_cel_key(0, 0).x, 3);
    assert_eq!(doc.sampled_cel_key(0, 2).x, 3);
}

#[test]
fn animated_group_grade_toggles_without_baking_source() {
    let mut doc = Document::new(3, 1);
    let night = [40, 75, 110, 255];
    let daylight = [90, 125, 160, 255];
    let accent = [220, 160, 70, 255];
    doc.set_pixel(0, 0, 0, daylight);
    doc.set_pixel(0, 0, 1, daylight);
    let group = doc.add_group("Scene").unwrap();
    doc.layers[0].parent = Some(group);
    doc.add_layer();
    let grade = doc.layers.len() - 1;
    doc.layers[grade].parent = Some(group);
    doc.set_adjustment(
        grade,
        Some(Adjustment::Palette {
            colors: vec![(daylight, night)],
        }),
    )
    .unwrap();
    doc.set_layer_mask(grade, Some(vec![[0, 0, 0, 255], CLEAR, CLEAR].into()))
        .unwrap();
    doc.add_layer();
    let accent_layer = doc.layers.len() - 1;
    doc.set_pixel(accent_layer, 0, 2, accent);
    for frame in 0..23 {
        doc.add_frame(frame, true);
    }
    let restored = formats::decode_project(&formats::encode_project(&doc).unwrap()).unwrap();
    assert_eq!(restored, doc);
    for frame in 0..24 {
        assert_eq!(restored.composite(frame), vec![night, daylight, accent]);
    }
    doc.layers[grade].visible = false;
    assert_eq!(doc.composite(23), vec![daylight, daylight, accent]);
    assert_eq!(doc.layers[0].cels[23][0], daylight);
}

#[test]
fn translated_off_canvas_edits_invalidate_the_composited_frame() {
    let mut doc = Document::new(3, 1);
    doc.set_cel_keyframe(
        0,
        0,
        Some(CelKeyframe {
            x: 1,
            ..CelKeyframe::default()
        }),
    )
    .unwrap();
    let before = doc.clone();
    doc.layers[0].cel_meta[0].outside.push(OutsidePixel {
        x: -1,
        y: 0,
        color: [220, 90, 40, 255],
    });

    assert_ne!(before.composite(0), doc.composite(0));
    assert!(!before.same_frame_pixels(&doc, 0));
}

#[test]
fn locked_ancestor_blocks_cel_keyframes_and_layer_masks() {
    let mut doc = Document::new(2, 2);
    let group = doc.add_group("Locked scene").unwrap();
    doc.layers[0].parent = Some(group);
    doc.layers[group].locked = true;
    assert!(
        doc.set_cel_keyframe(0, 0, Some(CelKeyframe::default()))
            .is_err()
    );
    assert!(doc.set_layer_mask(0, Some(Pixels::empty(4))).is_err());
    assert!(doc.layers[0].cel_meta[0].keyframe.is_none());
    assert!(doc.layers[0].mask.is_none());
}

#[test]
fn group_compositing_cannot_exceed_the_workspace_budget() {
    let mut doc = Document::new(8000, 8000);
    let outer = doc.add_group("Outer").unwrap();
    let inner = doc.add_group("Inner").unwrap();
    doc.layers[inner].parent = Some(outer);
    doc.layers[0].parent = Some(inner);
    assert!(
        doc.validate()
            .unwrap_err()
            .contains("compositing workspace")
    );
}
#[test]
fn changing_dense_linked_cels_reuses_their_private_backing() {
    let mut doc = Document::new(64, 64);
    doc.layers[0].cels[0] = vec![[12, 34, 56, 255]; 64 * 64].into();
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();
    let snapshot = doc.clone();

    doc.set_pixel(0, 0, 0, [90, 80, 70, 255]);
    let backing = doc.layers[0].cels[0].backing_id();
    assert_ne!(backing, snapshot.layers[0].cels[0].backing_id());
    assert_eq!(backing, doc.layers[0].cels[1].backing_id());

    doc.set_pixel(0, 0, 1, [70, 90, 80, 255]);
    assert_eq!(doc.layers[0].cels[0].backing_id(), backing);
    assert_eq!(doc.layers[0].cels[1][1], [70, 90, 80, 255]);
    assert_eq!(snapshot.layers[0].cels[1][1], [12, 34, 56, 255]);
}
