use pixelcraft::model::*;
#[test]
fn grayscale_conversion_uses_native_luma_truncation_and_preserves_alpha() {
    // Native rgba_to_graya_using_luma at 0x5f7170: 117.65 -> 117,
    // 1.8596 -> 1, and 0.7152 -> 0. Alpha is copied independently.
    let mut d = Document::new(3, 1);
    d.layers[0].cels[0] = vec![[200, 100, 50, 173], [1, 2, 3, 0], [0, 1, 0, 255]].into();
    d.set_color_mode(ColorMode::Grayscale);
    assert_eq!(
        &d.layers[0].cels[0][..],
        &[[117, 117, 117, 173], [1, 1, 1, 0], [0, 0, 0, 255]]
    );
}

#[test]
fn drawing_in_grayscale_uses_native_luma_truncation() {
    let mut d = Document::new(1, 1);
    d.set_color_mode(ColorMode::Grayscale);
    d.stamp(0, 0, 0, 0, [200, 100, 50, 173], 1);
    assert_eq!(d.composite(0)[0], [117, 117, 117, 173]);
}

#[test]
fn new_document_has_one_transparent_layer_and_100ms_frame() {
    let d = Document::new(8, 6);
    assert_eq!(d.layers.len(), 1);
    assert_eq!(d.durations, vec![100]);
    assert_eq!(d.composite(0), vec![CLEAR; 48]);
}
#[test]
fn continuous_strokes_include_both_ends_and_clip() {
    let mut d = Document::new(8, 8);
    let c = [40, 100, 220, 255];
    d.line(0, 0, (-2, 2), (7, 2), c, 1);
    assert_eq!(d.layers[0].cels[0].iter().filter(|p| **p == c).count(), 8);
    d.line(0, 0, (0, 0), (7, 7), c, 1);
    for n in 0..8 {
        assert_eq!(d.layers[0].cels[0][n * 8 + n], c);
    }
}
#[test]
fn fill_does_not_cross_diagonal_connections() {
    let mut d = Document::new(3, 3);
    let c = [255, 0, 0, 255];
    d.layers[0].cels[0] = vec![CLEAR, c, c, c, CLEAR, c, c, c, CLEAR].into();
    d.fill(0, 0, 0, 0, [0, 0, 255, 255]);
    assert_eq!(d.layers[0].cels[0][4], CLEAR);
    assert_eq!(d.layers[0].cels[0][8], CLEAR);
}
#[test]
fn duplicate_frame_copies_every_layer_without_linking_edits() {
    let mut d = Document::new(2, 2);
    d.add_layer();
    d.layers[1].cels[0][0] = [10, 20, 30, 255];
    d.add_frame(0, true);
    d.layers[1].cels[1][0] = CLEAR;
    assert_eq!(d.layers[1].cels[0][0], [10, 20, 30, 255]);
    assert_eq!(d.durations, vec![100, 100]);
}
#[test]
fn layers_composite_alpha_and_respect_visibility() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    d.add_layer();
    d.layers[1].cels[0][0] = [0, 0, 255, 128];
    assert_eq!(d.composite(0)[0], [127, 0, 128, 255]);
    d.layers[1].visible = false;
    assert_eq!(d.composite(0)[0], [255, 0, 0, 255]);
}

#[test]
fn odd_sized_round_brush_is_symmetric() {
    let mut d = Document::new(9, 9);
    d.stamp(0, 0, 4, 4, [2, 3, 4, 255], 5);
    let p = &d.layers[0].cels[0];
    for y in 0..9 {
        for x in 0..9 {
            assert_eq!(p[y * 9 + x], p[y * 9 + 8 - x]);
            assert_eq!(p[y * 9 + x], p[(8 - y) * 9 + x]);
        }
    }
}

#[test]
fn normal_blend_uses_native_integer_alpha_and_signed_color_division() {
    // Native 0x5e81e0: effective alpha 86, output alpha 152.
    // Signed division truncates toward zero, so blue is 30 - 16 = 14.
    assert_eq!(
        over([10, 20, 30, 100], [200, 100, 0, 128], 172),
        [117, 65, 14, 152]
    );
    // An empty destination retains source RGB even when opacity makes alpha zero.
    assert_eq!(over(CLEAR, [99, 88, 77, 128], 0), [99, 88, 77, 0]);
    // A transparent source leaves a nonempty destination alone.
    assert_eq!(
        over([10, 20, 30, 100], [200, 100, 0, 0], 255),
        [10, 20, 30, 100]
    );
}
