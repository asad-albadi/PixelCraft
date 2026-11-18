use pixelcraft::model::{Document, Tileset};
use std::cell::Cell;

fn tile_document() -> Document {
    let mut doc = Document::new(16384, 1);
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "One tile".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[10, 20, 30, 255]].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc
}
#[test]
fn cancellation_interrupts_native_validation_before_a_late_invalid_placement() {
    let mut doc = tile_document();
    doc.layers[1].tile_cels[0][16383] = 2;
    assert!(doc.validate().is_err());
    let polls = Cell::new(0);
    let error = doc
        .validate_until(|| {
            polls.set(polls.get() + 1);
            polls.get() >= 3
        })
        .unwrap_err();
    assert!(
        error.contains("cancelled"),
        "Native validation did not poll cancellation: {error}"
    );
    assert_eq!(doc.layers[1].tile_cels[0][16383], 2);
}
#[test]
fn native_storage_accounting_can_be_abandoned_without_changing_the_document() {
    let doc = tile_document();
    let result = doc.stored_pixels_until(|| true);
    assert!(result.is_err(), "Native accounting ignored cancellation");
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(doc.layers[1].tile_cels[0][16383], 0);
    assert_eq!(doc.stored_pixels(), 16385);
}
