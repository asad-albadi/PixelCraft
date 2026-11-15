use pixelcraft::{
    formats::{decode_ase, encode_ase},
    model::{CLEAR, ColorMode, Document, Pixels},
};

#[test]
fn palette_edit_keeps_sparse_and_empty_indexed_cels_compact() {
    let mut doc = Document::new(128, 128);
    let ink = [100, 20, 40, 255];
    let replacement = [20, 80, 100, 255];
    doc.palette = vec![CLEAR, ink];
    doc.set_color_mode(ColorMode::Indexed);
    doc.add_frame(0, false);
    doc.set_pixel(0, 1, 7, ink);

    doc.set_palette_color(1, replacement);

    assert!(matches!(doc.layers[0].cels[0], Pixels::Empty { .. }));
    assert!(matches!(doc.layers[0].cels[1], Pixels::Sparse(_)));
    assert_eq!(doc.layers[0].cels[1][7], replacement);
    assert_eq!(doc.layers[0].cels[1][8], CLEAR);
}

#[test]
fn linked_indexed_stroke_retains_palette_slot_in_ase_source_cel() {
    let mut doc = Document::new(8, 8);
    let ink = [100, 20, 40, 255];
    let replacement = [20, 80, 100, 255];
    doc.palette = vec![CLEAR, ink, ink];
    doc.set_color_mode(ColorMode::Indexed);
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();
    doc.drawing_index = Some(2);
    doc.set_pixel(0, 1, 7, ink);

    let mut saved = decode_ase(&encode_ase(&doc).unwrap()).unwrap();
    saved.set_palette_color(2, replacement);

    assert_eq!(saved.composite(0)[7], replacement);
    assert_eq!(saved.composite(1)[7], replacement);
}

#[test]
fn flipping_linked_empty_indexed_cels_keeps_nonzero_transparent_slot() {
    let mut doc = Document::new(16, 16);
    doc.palette = vec![[100, 20, 40, 255], CLEAR];
    doc.transparent_index = 1;
    doc.set_color_mode(ColorMode::Indexed);
    doc.add_frame(0, false);
    doc.link_cel(0, 1, 0).unwrap();

    doc.flip(0, 1, true);

    for frame in 0..2 {
        assert!(matches!(doc.layers[0].cels[frame], Pixels::Empty { .. }));
        assert!(doc.layers[0].cel_meta[frame].indices.is_none());
    }
}
