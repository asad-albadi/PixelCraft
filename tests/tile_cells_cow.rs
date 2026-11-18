use pixelcraft::{formats, model::*};

fn fixture() -> Document {
    let mut doc = Document::new(256, 256);
    doc.tilesets.push(Tileset {
        editor_id: Default::default(),
        name: "Cell".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[210, 30, 10, 255]].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.paint_tile(1, 0, 0, 0, 1).unwrap();
    doc.layers[1].tile_cels[0][100] = 0xe0000001;
    doc
}

#[test]
fn document_snapshots_share_placement_words_until_an_edit() {
    let doc = fixture();
    let mut snapshot = doc.clone();
    assert_eq!(
        doc.layers[1].tile_cels[0].as_ptr(),
        snapshot.layers[1].tile_cels[0].as_ptr(),
        "snapshot copied a placement array"
    );
    snapshot.paint_tile(1, 0, 1, 0, 1).unwrap();
    assert_ne!(
        doc.layers[1].tile_cels[0].as_ptr(),
        snapshot.layers[1].tile_cels[0].as_ptr()
    );
    assert_eq!(doc.layers[1].tile_cels[0][1], 0);
    assert_eq!(snapshot.layers[1].tile_cels[0][1], 1);
    assert_eq!(snapshot.layers[1].tile_cels[0][100], 0xe0000001);
}

#[test]
fn duplicate_frames_share_words_but_keep_independent_edits() {
    let mut doc = fixture();
    doc.add_frame(0, true);
    assert_eq!(doc.durations.len(), 2);
    assert_eq!(
        doc.layers[1].tile_cels[0].as_ptr(),
        doc.layers[1].tile_cels[1].as_ptr(),
        "frame copy duplicated words"
    );
    doc.paint_tile(1, 1, 1, 0, 1).unwrap();
    assert_eq!(doc.layers[1].tile_cels[0][1], 0);
    assert_eq!(doc.layers[1].tile_cels[1][1], 1);
}

#[test]
fn cow_placement_buffers_keep_native_json_and_ase_array_compatibility() {
    let doc = fixture();
    let json = serde_json::to_value(&doc).unwrap();
    assert!(json["layers"][1]["tile_cels"][0].is_array());
    let restored: Document = serde_json::from_value(json).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.layers[1].tile_cels, doc.layers[1].tile_cels);
    let ase = formats::decode_ase(&formats::encode_ase(&doc).unwrap()).unwrap();
    assert_eq!(ase.layers[1].tile_cels, doc.layers[1].tile_cels);
    assert_eq!(ase.composite(0), doc.composite(0));
}

#[test]
fn no_op_placement_paint_does_not_detach_a_shared_map() {
    let doc = fixture();
    let mut snapshot = doc.clone();
    let before = snapshot.layers[1].tile_cels[0].as_ptr();
    snapshot.paint_tile(1, 0, 0, 0, 1).unwrap();
    assert_eq!(
        snapshot.layers[1].tile_cels[0].as_ptr(),
        before,
        "no-op paint copied the map"
    );
}

#[test]
fn linked_placement_edits_share_the_updated_buffer_and_preserve_snapshots() {
    let mut doc = fixture();
    doc.add_frame(0, true);
    doc.link_cel(1, 1, 0).unwrap();
    let before = doc.clone();
    doc.paint_tile(1, 0, 1, 0, 1).unwrap();
    assert_eq!(doc.layers[1].tile_cels[0][1], 1);
    assert_eq!(doc.layers[1].tile_cels[1][1], 1);
    assert_eq!(
        doc.layers[1].tile_cels[0].as_ptr(),
        doc.layers[1].tile_cels[1].as_ptr(),
        "linked peers copied the same edited map"
    );
    assert_eq!(before.layers[1].tile_cels[0][1], 0);
    let snapshot = doc.clone();
    let mut hits = vec![false; 65536];
    hits[2] = true;
    doc.paint_matching_tiles(1, 0, 1, &hits).unwrap();
    assert_eq!(
        doc.layers[1].tile_cels[0].as_ptr(),
        doc.layers[1].tile_cels[1].as_ptr()
    );
    assert_eq!(doc.layers[1].tile_cels[1][2], 1);
    assert_eq!(snapshot.layers[1].tile_cels[0][2], 0);
}

#[test]
fn no_op_matching_mask_does_not_detach_a_shared_map() {
    let doc = fixture();
    let mut snapshot = doc.clone();
    let before = snapshot.layers[1].tile_cels[0].as_ptr();
    snapshot
        .paint_matching_tiles(1, 0, 1, &vec![false; 65536])
        .unwrap();
    assert_eq!(snapshot.layers[1].tile_cels[0].as_ptr(), before);
}
