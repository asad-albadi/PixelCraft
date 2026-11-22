//! Cross-layer regressions retained through the compatibility facade.
use pixelcraft::model::*;
fn tilemap() -> Document {
    let mut doc = Document::new(3, 1);
    let tiles = [[100, 50, 30, 255], [10, 80, 90, 255], [44, 33, 120, 255]]
        .map(|color| Pixels::from(vec![color]));
    for name in ["Shared", "Other"] {
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: name.into(),
            width: 1,
            height: 1,
            tiles: tiles.iter().cloned().map(Into::into).collect(),
        });
    }
    doc.add_frame(0, false);
    doc.add_tile_layer(0).unwrap();
    doc.add_tile_layer(0).unwrap();
    doc.add_tile_layer(1).unwrap();
    doc.layers[1].tile_cels[0] = vec![0x8000_0001, 2, 3].into();
    doc.layers[1].tile_cels[1] = vec![0x4000_0003, 1, 0].into();
    doc.layers[2].tile_cels[1] = vec![2, 3, 1].into();
    doc.layers[3].tile_cels[1] = vec![1, 2, 3].into();
    doc
}
#[test]
fn automation_accepts_explicit_variant_mode_and_legacy_boolean() {
    let doc = tilemap();
    let mut edit = serde_json::json!({
        "op": "edit-tile-pixel", "layer": 1, "frame": 0,
        "x": 0, "y": 0, "pixel_x": 0, "pixel_y": 0, "color": [10, 80, 90, 255],
        "variant": true
    });
    let apply = |edit| {
        pixelcraft::automation::apply_command_batch(&doc, &serde_json::json!({"commands": [edit]}))
    };
    let legacy = apply(edit.clone()).unwrap();
    assert_eq!(legacy.tilesets[0].tiles.len(), 3);
    assert_eq!(legacy.layers[1].tile_cels[0][0], 0x8000_0002);
    edit["variant_mode"] = serde_json::json!("create-variant");
    let fresh = apply(edit.clone()).unwrap();
    assert_eq!(fresh.tilesets[0].tiles.len(), 4);
    assert_eq!(fresh.layers[1].tile_cels[0][0], 0x8000_0004);
    edit["variant_mode"] = serde_json::json!("invalid");
    assert!(apply(edit).is_err());
}
