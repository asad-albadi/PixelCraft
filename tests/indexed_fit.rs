use pixelcraft::{automation, model::*};
use serde_json::json;

#[test]
fn indexed_fit_commands_choose_the_requested_color_space() {
    let mut doc = Document::new(1, 1);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.layers[0].cels[0].set(0, [128, 128, 128, 255]);
    for (criterion, slot) in [
        ("default", 2),
        ("rgb", 2),
        ("linear-rgb", 1),
        ("xyz", 1),
        ("lab", 2),
    ] {
        let converted = automation::apply_command_batch(
            &doc,
            &json!({
                "commands":[{"op":"convert-indexed", "fit_criteria":criterion}]
            }),
        )
        .unwrap();
        assert_eq!(
            converted.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
            slot,
            "{criterion}"
        );
    }
    for criterion in [json!("unknown"), json!(17), json!(null)] {
        let result = automation::apply_command_batch(
            &doc,
            &json!({
                "commands":[{"op":"convert-indexed", "fit_criteria":criterion}]
            }),
        );
        assert!(
            result.is_err(),
            "invalid criterion was accepted: {criterion}"
        );
    }
}

#[test]
fn indexed_fit_preserves_linked_hidden_and_tile_artwork_across_dithering_modes() {
    use pixelcraft::{contributions::DitheringMatrix, formats};
    let matrix = DitheringMatrix::bayer(2).unwrap();
    let mut doc = Document::new(2, 2);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.layers[0].cels[0] = vec![[128, 128, 128, 255]; 4].into();
    doc.layers[0].cel_meta[0].outside = vec![OutsidePixel {
        x: -1,
        y: 0,
        color: [128, 128, 128, 255],
        palette_slot: None,
    }];
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "fit".into(),
        width: 2,
        height: 2,
        tiles: vec![doc.layers[0].cels[0].clone().into()],
    });
    let source = doc.clone();
    for fit in IndexedFit::ALL {
        for dither in [
            IndexedDithering::None,
            IndexedDithering::Ordered(&matrix),
            IndexedDithering::FloydSteinberg,
        ] {
            let converted = doc
                .indexed_conversion_with_fit_until(None, 0, dither, fit, || false)
                .unwrap();
            assert_eq!(
                converted.layers[0].cels[0].backing_id(),
                converted.layers[0].cels[1].backing_id()
            );
            assert!(std::sync::Arc::ptr_eq(
                converted.layers[0].cel_meta[0].indices.as_ref().unwrap(),
                converted.layers[0].cel_meta[1].indices.as_ref().unwrap()
            ));
            let outside = &converted.layers[0].cel_meta[0].outside[0];
            assert_eq!(
                outside.color,
                converted.palette[outside.palette_slot.unwrap() as usize]
            );
            for color in converted.tilesets[0].tiles[0].iter() {
                assert!(converted.palette.contains(color));
            }
            let reopened = formats::decode_ase(&formats::encode_ase(&converted).unwrap()).unwrap();
            assert_eq!(reopened.layers[0].cels[1], converted.layers[0].cels[1]);
            assert_eq!(
                reopened.layers[0].cel_meta[0].outside,
                converted.layers[0].cel_meta[0].outside
            );
        }
    }
    assert_eq!(doc, source);
}

#[test]
fn indexed_fit_keeps_authored_duplicate_slots_and_frame_palettes() {
    use pixelcraft::contributions::DitheringMatrix;
    let matrix = DitheringMatrix::bayer(2).unwrap();
    let orange = [200, 100, 20, 137];
    let blue = [20, 100, 200, 100];
    let mut doc = Document::new(2, 1);
    doc.palette = vec![CLEAR, orange, orange];
    doc.set_color_mode(ColorMode::Indexed);
    doc.add_frame(0, true);
    doc.frame_palettes = vec![doc.palette.clone(), vec![CLEAR, blue, blue]];
    for (frame, color) in [orange, blue].into_iter().enumerate() {
        doc.layers[0].cels[frame] = vec![color, CLEAR].into();
        doc.layers[0].cel_meta[frame].indices = Some(std::sync::Arc::new(vec![2, 0]));
        doc.layers[0].cel_meta[frame].outside = vec![OutsidePixel {
            x: -1,
            y: 0,
            color,
            palette_slot: Some(2),
        }];
    }
    for fit in IndexedFit::ALL {
        for dither in [
            IndexedDithering::None,
            IndexedDithering::Ordered(&matrix),
            IndexedDithering::FloydSteinberg,
        ] {
            let converted = doc
                .indexed_conversion_with_fit_until(None, 2, dither, fit, || false)
                .unwrap();
            for (frame, color) in [orange, blue].into_iter().enumerate() {
                assert_eq!(converted.layers[0].cels[frame][0], color);
                assert_eq!(
                    converted.layers[0].cel_meta[frame]
                        .indices
                        .as_ref()
                        .unwrap()
                        .as_slice(),
                    &[0, 2]
                );
                assert_eq!(
                    converted.layers[0].cel_meta[frame].outside[0].palette_slot,
                    Some(0)
                );
            }
        }
    }
}

#[test]
fn indexed_fit_handles_alpha_mask_and_cancellation_without_changing_source() {
    let mut doc = Document::new(1, 1);
    doc.palette = vec![CLEAR, [0, 0, 0, 255]];
    doc.layers[0].cels[0].set(0, [0, 0, 0, 1]);
    for fit in [
        IndexedFit::Rgb,
        IndexedFit::LinearRgb,
        IndexedFit::Xyz,
        IndexedFit::Lab,
    ] {
        for background in [false, true] {
            doc.layers[0].background = background;
            for dither in [IndexedDithering::None, IndexedDithering::FloydSteinberg] {
                let converted = doc
                    .indexed_conversion_with_fit_until(None, 0, dither, fit, || false)
                    .unwrap();
                assert_eq!(
                    converted.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
                    if background { 0 } else { 1 }
                );
            }
        }
    }
    let mut alpha = Document::new(1, 1);
    alpha.palette = vec![CLEAR, [100, 100, 100, 255], [101, 100, 100, 137]];
    alpha.layers[0].cels[0].set(0, [100, 100, 100, 137]);
    for (fit, slot) in [(IndexedFit::Default, 2), (IndexedFit::Rgb, 1)] {
        let converted = alpha
            .indexed_conversion_with_fit_until(None, 0, IndexedDithering::None, fit, || false)
            .unwrap();
        assert_eq!(
            converted.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
            slot
        );
    }
    let mut doc = Document::new(512, 512);
    doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
    doc.layers[0].cels[0] = vec![[128, 128, 128, 255]; 512 * 512].into();
    let source = doc.clone();
    let calls = std::cell::Cell::new(0);
    assert!(
        doc.indexed_conversion_with_fit_until(
            None,
            0,
            IndexedDithering::FloydSteinberg,
            IndexedFit::Lab,
            || {
                calls.set(calls.get() + 1);
                calls.get() > 2
            }
        )
        .unwrap_err()
        .contains("cancelled")
    );
    assert_eq!(doc, source);
}

#[test]
fn indexed_fit_matches_compiled_reference_palette_decisions() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/indexed-fit-reference.json")).unwrap();
    let palette: Vec<Pixel> = serde_json::from_value(fixture["palette"].clone()).unwrap();
    let pixels: Vec<Pixel> = serde_json::from_value(fixture["pixels"].clone()).unwrap();
    let mut doc = Document::new(pixels.len(), 1);
    doc.palette = palette;
    doc.layers[0].cels[0] = pixels.into();
    for (criterion, indices) in fixture["indices"].as_object().unwrap() {
        let expected: Vec<u8> = serde_json::from_value(indices.clone()).unwrap();
        let converted = doc
            .indexed_conversion_with_fit_until(
                None,
                0,
                IndexedDithering::None,
                IndexedFit::parse(criterion).unwrap(),
                || false,
            )
            .unwrap();
        assert_eq!(
            converted.layers[0].cel_meta[0]
                .indices
                .as_ref()
                .unwrap()
                .as_slice(),
            expected,
            "{criterion}"
        );
    }
}
