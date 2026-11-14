use pixelcraft::{
    extensions::{Manager, Session, run_lua, run_source_with_host},
    model::Document,
};
use serde_json::json;
use std::{fs, path::PathBuf, sync::mpsc};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/third-party-lua")
        .join(path)
}

#[test]
fn gasparoken_duplicates_the_last_frame_across_layers() {
    let mut doc = Document::new(4, 3);
    doc.add_layer();
    doc.add_frame(0, false);
    doc.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    doc.stamp(0, 1, 1, 0, [0, 255, 0, 255], 1);
    doc.stamp(1, 1, 2, 1, [0, 0, 255, 255], 1);
    doc.layers[1].cel_meta[1].bounds = Some([1, 1, 2, 1]);

    let edited = run_lua(
        &doc,
        &fixture("gasparoken/duplicateLastFrame.lua"),
        0,
        1,
        None,
    )
    .unwrap();

    assert_eq!(edited.durations.len(), 3);
    assert_eq!(edited.layers.len(), doc.layers.len());
    for (original, edited_layer) in doc.layers.iter().zip(&edited.layers) {
        assert_eq!(&edited_layer.cels[..2], original.cels.as_slice());
        assert_eq!(edited_layer.cels[2], original.cels[1]);
    }
    assert_eq!(edited.layers[1].cel_meta[2].bounds, Some([1, 1, 2, 1]));
}

#[test]
fn middle_color_updates_the_foreground_through_the_editor_host() {
    let (updates, observed) = mpsc::channel();
    let doc = Document::new(2, 2);
    let edited = run_source_with_host(
        &doc,
        include_str!("fixtures/third-party-lua/christopherwk210/middle-color.lua"),
        0,
        0,
        None,
        Some([[3, 10, 101, 50], [200, 24, 60, 255]]),
        json!({}),
        Box::new(move |request| match request["op"].as_str() {
            Some("refresh" | "preferences") => {
                updates.send(request["colors"].clone()).unwrap();
                Ok(json!({}))
            }
            _ => panic!("Unexpected host request: {request}"),
        }),
    )
    .unwrap();

    let colors = observed.into_iter().collect::<Vec<_>>();
    assert!(!colors.is_empty());
    for update in colors {
        assert_eq!(update, json!([[101, 17, 80, 152], [200, 24, 60, 255]]));
    }
    assert_eq!(edited, doc);
}

#[test]
fn tile_index_overlay_package_registers_and_reports_the_missing_sprite_api() {
    let doc = Document::new(16, 16);
    let session = Session::new(&doc).unwrap();
    let manager = Manager::at(session.directory.clone()).unwrap();
    let id = manager
        .install(&fixture("pixeltica/TileIndexOverlay_v1.aseprite-extension"))
        .unwrap();
    let commands = manager.commands().unwrap();
    let [command] = commands.as_slice() else {
        panic!("Expected one installed command, got {commands:?}");
    };
    assert_eq!(id, "tile-index-overlay");
    assert_eq!(command.id, "TileIndexOverlay");
    assert_eq!(command.title, "New Tile Index Overlay Layer");

    let source = fs::read_to_string(&command.script).unwrap();
    let result = run_source_with_host(
        &doc,
        &source,
        0,
        0,
        Some(&command.id),
        None,
        json!({}),
        Box::new(|request| {
            assert_eq!(request["op"], "dialog");
            assert_eq!(request["dialog"]["title"], "Index Overlay Options");
            Ok(json!({"widget":"ok","data":{"ok":true}}))
        }),
    );
    fs::remove_dir_all(session.directory).unwrap();

    let error = result.unwrap_err();
    assert!(error.contains("global 'Sprite'"), "{error}");
}
