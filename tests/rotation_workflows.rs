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
fn rotsprite_rotates_large_selections_and_validates_limits_before_mutating() {
    let mut doc = Document::new(512, 512);
    let ink = [255, 0, 0, 255];
    doc.palette = vec![CLEAR, ink, ink];
    doc.set_color_mode(ColorMode::Indexed);
    doc.drawing_index = Some(2);
    for y in 256..259 {
        for x in 280..283 {
            doc.set_pixel(0, 0, y * 512 + x, ink);
        }
    }
    doc.drawing_index = None;
    let pivot = [256., 256.];
    let original = doc.clone();

    assert!(transform_rotsprite(&mut doc, 0, 0, [0, 0, 16_384, 16_384], None, 23., pivot).is_err());
    assert_eq!(doc, original);
    let (_, selected) =
        transform_rotsprite(&mut doc, 0, 0, [0, 0, 512, 512], None, 23., pivot).unwrap();
    let indices = doc.layers[0].cel_meta[0].indices.as_ref().unwrap();
    let moved: Vec<_> = indices
        .iter()
        .enumerate()
        .filter(|(_, slot)| **slot == 2)
        .collect();
    assert!(!moved.is_empty());
    assert!(moved.iter().all(|(index, _)| selected[*index]));
    assert_eq!(indices[256 * 512 + 280], 0);
}

#[test]
fn affine_rotation_matches_native_rotation_pixels_and_indexed_coverage() {
    let mut source = Document::new(16, 16);
    let ink = [180, 80, 40, 255];
    source.palette = vec![CLEAR, ink, ink];
    source.set_color_mode(ColorMode::Indexed);
    for (x, y, slot) in [(5, 7, 1), (6, 7, 1), (7, 6, 1), (7, 7, 2), (7, 8, 2)] {
        source.drawing_index = Some(slot);
        source.set_pixel(0, 0, y * 16 + x, ink);
    }
    source.drawing_index = None;
    for degrees in [0., 23., 45., 90., -90., 180.] {
        let mut native = source.clone();
        let mut affine = source.clone();
        let (_, expected_mask) =
            transform_rotsprite(&mut native, 0, 0, [4, 4, 8, 8], None, degrees, [8., 8.]).unwrap();
        let (_, actual_mask) = transform_affine_rotsprite(
            &mut affine,
            0,
            0,
            [4, 4, 8, 8],
            None,
            AffineTransform::rotation(degrees, [8., 8.]),
        )
        .unwrap();
        assert_eq!(
            affine.layers[0].cels[0], native.layers[0].cels[0],
            "{degrees}"
        );
        assert_eq!(
            affine.layers[0].cel_meta[0].indices, native.layers[0].cel_meta[0].indices,
            "{degrees}"
        );
        assert_eq!(actual_mask, expected_mask, "{degrees}");
    }
}

#[test]
fn affine_composition_keeps_pixels_until_final_sampling_and_validates_before_editing() {
    let mut source = Document::new(12, 12);
    let ink = [180, 80, 40, 255];
    source.set_pixel(0, 0, 6 * 12 + 10, ink);
    // First move the artwork entirely off-canvas, then rotate it back into view.
    let move_right = AffineTransform::about([2., 0.], [0., 1.], [0., 0.]);
    let rotate_back = AffineTransform::rotation(180., [12., 6.]);
    let matrix = move_right.then(rotate_back);
    let mut composed = source.clone();
    transform(
        &mut composed,
        0,
        0,
        [10, 6, 1, 1],
        None,
        Transform::Affine(matrix),
    )
    .unwrap();
    assert_eq!(composed.layers[0].cels[0][5 * 12 + 2], ink);
    assert_eq!(composed.layers[0].cels[0][5 * 12 + 3], ink);
    assert_eq!(composed.layers[0].cels[0][6 * 12 + 10], CLEAR);
    let before = composed.clone();
    for matrix in [
        AffineTransform::about([0., 0.], [0., 1.], [0., 0.]),
        AffineTransform::about([f64::NAN, 0.], [0., 1.], [0., 0.]),
        AffineTransform::about([1e9, 0.], [0., 1.], [0., 0.]),
    ] {
        assert!(
            transform(
                &mut composed,
                0,
                0,
                [0, 0, 12, 12],
                None,
                Transform::Affine(matrix)
            )
            .is_err()
        );
        assert_eq!(composed, before);
    }
}
