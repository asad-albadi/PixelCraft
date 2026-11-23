use pixelcraft_core::{formats, model::Document};

#[test]
fn bounded_decode_rejects_expansion_and_raster_allocation_before_open() {
    let mut doc = Document::new(64, 64);
    doc.layers[0].cels[0] = vec![[12, 34, 56, 255]; 64 * 64].into();
    let bytes = formats::encode_project(&doc).unwrap();
    assert!(formats::decode_project_bounded(&bytes, 32, 64 * 64 * 4).is_err());
    assert!(formats::decode_project_bounded(&bytes, 1_000_000, 16).is_err());
    assert_eq!(
        formats::decode_project_bounded(&bytes, 1_000_000, 1_000_000).unwrap(),
        doc
    );
    assert_eq!(formats::decode_project(&bytes).unwrap(), doc);
}

#[test]
fn invalid_budgets_and_nonproject_data_are_errors() {
    assert!(formats::decode_project_bounded(b"not a project", 4096, 4096).is_err());
    let bytes = formats::encode_project(&Document::new(1, 1)).unwrap();
    assert!(formats::decode_project_bounded(&bytes, 0, 4096).is_err());
    assert!(formats::decode_project_bounded(&bytes, 4096, 0).is_err());
}
