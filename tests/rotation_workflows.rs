use pixelcraft::{model::*, workflows::*};

#[test]
fn rotsprite_smooths_a_corner_and_preserves_indexed_slots() {
    let mut doc = Document::new(7, 7);
    let ink = [0, 100, 200, 255];
    doc.palette = vec![CLEAR, ink, ink];
    doc.set_color_mode(ColorMode::Indexed);
    for (x, y, slot) in [(2, 2, 1), (4, 2, 1), (2, 3, 1), (3, 3, 1), (4, 3, 2)] {
        doc.drawing_index = Some(slot);
        doc.set_pixel(0, 0, y * 7 + x, ink);
    }
    doc.drawing_index = None;
    let bounds = [2, 2, 3, 3];
    let pivot = [3.5, 3.5];
    let mut fast = doc.clone();
    let fast_bounds = transform(
        &mut fast,
        0,
        0,
        bounds,
        None,
        Transform::RotateAround {
            degrees: 30.,
            pivot,
        },
    )
    .unwrap();

    let (rotated_bounds, selection) =
        transform_rotsprite(&mut doc, 0, 0, bounds, None, 30., pivot).unwrap();
    assert_eq!(rotated_bounds, fast_bounds);
    assert_eq!(fast.layers[0].cels[0][2 * 7 + 3], CLEAR);
    assert_eq!(doc.layers[0].cels[0][2 * 7 + 3], ink);
    assert!(selection[2 * 7 + 3]);
    let indices = doc.layers[0].cel_meta[0].indices.as_ref().unwrap();
    assert_eq!(indices[2 * 7 + 3], 1);
    assert_eq!(indices[4 * 7 + 4], 2);
}

#[test]
fn rotsprite_does_not_sample_unselected_artwork() {
    let mut doc = Document::new(8, 8);
    let selected = [100, 150, 25, 255];
    let excluded = [10, 80, 200, 255];
    let mut mask = vec![false; 64];
    for index in [27, 28, 36] {
        mask[index] = true;
        doc.set_pixel(0, 0, index, selected);
    }
    doc.set_pixel(0, 0, 35, excluded);
    let (_, rotated) =
        transform_rotsprite(&mut doc, 0, 0, [3, 3, 2, 2], Some(&mask), 30., [0., 0.]).unwrap();
    assert_eq!(doc.layers[0].cels[0][35], excluded);
    assert!(!rotated[35]);
    for (index, pixel) in doc.layers[0].cels[0].iter().enumerate() {
        if index != 35 {
            assert_eq!(rotated[index], *pixel == selected);
        }
    }
}

#[test]
fn rotsprite_rejects_large_nonorthogonal_work_before_mutating_artwork() {
    let mut doc = Document::new(512, 512);
    doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
    let original = doc.clone();
    let selection = [0, 0, 512, 512];
    let pivot = [256., 256.];
    assert!(transform_rotsprite(&mut doc, 0, 0, selection, None, 23., pivot).is_err());
    assert!(doc == original);
    assert!(transform_rotsprite(&mut doc, 0, 0, selection, None, 90., pivot).is_ok());
}
