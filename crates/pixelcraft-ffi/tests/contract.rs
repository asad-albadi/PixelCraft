use pixelcraft_ffi::*;
use serde_json::Value;
use std::sync::Mutex;

static TEST: Mutex<()> = Mutex::new(());

fn bytes(id: u64) -> (u32, Vec<u8>) {
    assert_ne!(id, 0);
    let kind = pc_buffer_kind(id);
    let len = pc_buffer_len(id);
    // SAFETY: this test owns the live buffer and copies before freeing it.
    let data = unsafe { std::slice::from_raw_parts(pc_buffer_data(id), len) }.to_vec();
    assert_eq!(pc_buffer_free(id), 1);
    (kind, data)
}
fn state(h: u64, op: u32, a: u32, b: u32, input: u64) -> Value {
    let (kind, data) = bytes(pc_request(h, op, a, b, input));
    assert_eq!(kind, 1, "{}", String::from_utf8_lossy(&data));
    serde_json::from_slice(&data).unwrap()
}
fn input(data: &[u8]) -> u64 {
    let id = pc_buffer_alloc(data.len());
    assert_ne!(id, 0);
    // SAFETY: test owns this input buffer, which is large enough and live.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), pc_buffer_data(id) as *mut u8, data.len())
    };
    id
}

#[test]
fn stale_handles_and_invalid_dimensions_fail_without_reusing_ids() {
    let _test = TEST.lock().unwrap();
    assert_eq!(pc_abi_version(), 1);
    assert_eq!(bytes(pc_request(0, 0, 0, 64, 0)).0, 0);
    assert_eq!(bytes(pc_request(0, 0, 8192, 8192, 0)).0, 0);
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    assert_eq!(pc_session_close(h), 1);
    assert_eq!(pc_session_close(h), 0);
    assert_eq!(bytes(pc_request(h, 2, 0, 0, 0)).0, 0);
    let next = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    assert_ne!(h, next);
    pc_session_close(next);
    assert_eq!(pc_buffer_alloc(64 * 1024 * 1024), 0);
}

#[test]
fn stroke_is_one_undo_step_and_cancellation_restores_redo() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let points = input(&[1i32.to_le_bytes(), 1i32.to_le_bytes()].concat());
    let color = u32::from_le_bytes([200, 30, 100, 255]);
    assert_eq!(pc_stroke(h, 0, points, color, 1), 0);
    assert_eq!(pc_stroke(h, 2, 0, color, 1), 0);
    let rendered = bytes(pc_request(h, 7, 0, 0, 0));
    assert_eq!(rendered.0, 2);
    assert_eq!(&rendered.1[(8 + 1) * 4..(8 + 2) * 4], &[200, 30, 100, 255]);
    assert_eq!(state(h, 2, 0, 0, 0)["undo"], 1);
    assert_eq!(state(h, 3, 0, 0, 0)["redo"], 1);
    assert_eq!(pc_stroke(h, 0, points, color, 1), 0);
    assert_eq!(pc_stroke(h, 3, 0, color, 1), 0);
    assert_eq!(state(h, 2, 0, 0, 0)["redo"], 1);
    assert_eq!(state(h, 4, 0, 0, 0)["undo"], 1);
    pc_buffer_free(points);
    pc_session_close(h);
}

#[test]
fn native_roundtrip_and_failed_open_preserve_artwork_and_save_revision() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 1, 0, 0, 0)["session"].as_u64().unwrap();
    let before = bytes(pc_request(h, 7, 0, 0, 0)).1;
    let encoded = bytes(pc_request(h, 5, 0, 0, 0));
    assert_eq!(encoded.0, 3);
    let file = input(&encoded.1);
    state(h, 0, 64, 64, 0);
    state(h, 6, 0, 0, file);
    assert_eq!(bytes(pc_request(h, 7, 0, 0, 0)).1, before);
    let malformed = input(b"not a project");
    assert_eq!(bytes(pc_request(h, 6, 0, 0, malformed)).0, 0);
    assert_eq!(bytes(pc_request(h, 7, 0, 0, 0)).1, before);
    assert_eq!(bytes(pc_request(h, 12, u32::MAX, 0, 0)).0, 0);
    pc_buffer_free(file);
    pc_buffer_free(malformed);
    pc_session_close(h);
}

#[test]
fn recovery_preserves_unsaved_baseline_and_rejects_corruption_atomically() {
    let _lock = TEST.lock().unwrap();
    let h = state(0, 0, 16, 16, 0)["session"].as_u64().unwrap();
    let point = input(&[2i32.to_le_bytes(), 3i32.to_le_bytes()].concat());
    assert_eq!(pc_stroke(h, 0, point, u32::MAX, 1), 0);
    assert_eq!(pc_stroke(h, 2, 0, 0, 1), 0);
    pc_buffer_free(point);
    let (kind, snapshot) = bytes(pc_request(h, 15, 0, 0, 0));
    assert_eq!(kind, 5);
    let saved = input(&snapshot);
    let other = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let (kind, state) = bytes(pc_request(other, 16, 0, 0, saved));
    assert_eq!(kind, 1);
    let state: serde_json::Value = serde_json::from_slice(&state).unwrap();
    assert_eq!(state["width"], 16);
    assert_eq!(state["dirty"], true);
    let before = bytes(pc_request(other, 7, 0, 0, 0));
    let broken = input(&snapshot[..snapshot.len() - 1]);
    assert_eq!(bytes(pc_request(other, 16, 0, 0, broken)).0, 0);
    assert_eq!(bytes(pc_request(other, 7, 0, 0, 0)), before);
    pc_buffer_free(saved);
    pc_buffer_free(broken);
    pc_session_close(h);
    pc_session_close(other);
}

#[test]
fn mobile_commands_are_atomic_revision_checked_and_use_shared_tool_history() {
    let _lock = TEST.lock().unwrap();
    let h = state(0, 0, 16, 16, 0)["session"].as_u64().unwrap();
    let request = input(br#"{"revision":1,"commands":[{"op":"shape","layer":0,"frame":0,"kind":"rectangle","x":2,"y":2,"x2":8,"y2":8,"color":[100,20,50,255],"filled":false},{"op":"fill","layer":0,"frame":0,"x":3,"y":3,"color":[30,200,80,255]}]}"#);
    let result = state(h, 17, 0, 0, request);
    assert_eq!(result["undo"], 1);
    let after = bytes(pc_request(h, 7, 0, 0, 0)).1;
    assert_eq!(
        &after[(3 * 16 + 3) * 4..(3 * 16 + 3) * 4 + 4],
        &[30, 200, 80, 255]
    );
    assert_eq!(
        bytes(pc_request(h, 17, 0, 0, request)).0,
        0,
        "stale revision must fail"
    );
    assert_eq!(bytes(pc_request(h, 7, 0, 0, 0)).1, after);
    state(h, 3, 0, 0, 0);
    assert!(bytes(pc_request(h, 7, 0, 0, 0)).1.iter().all(|x| *x == 0));
    pc_buffer_free(request);
    pc_session_close(h);
}

#[test]
fn brush_preferences_and_composite_sampling_do_not_create_history() {
    let _lock = TEST.lock().unwrap();
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let preference = state(h, 18, 1, 0, 0);
    assert_eq!(preference["dirty"], false);
    assert_eq!(preference["undo"], 0);
    let (kind, sample) = bytes(pc_request(h, 19, 2, 2, 0));
    assert_eq!(kind, 6);
    assert_eq!(
        serde_json::from_slice::<Value>(&sample).unwrap()["color"],
        serde_json::json!([0, 0, 0, 0])
    );
    assert_eq!(bytes(pc_request(h, 19, 8, 0, 0)).0, 0);
    pc_session_close(h);
}

#[test]
fn selections_clip_brushes_and_copy_paste_transform_uses_shared_pixels_and_undo() {
    let _lock = TEST.lock().unwrap();
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let act = |packet: Value| {
        let input = input(&serde_json::to_vec(&packet).unwrap());
        let out = state(h, 20, 0, 0, input);
        pc_buffer_free(input);
        out
    };
    let selected = act(serde_json::json!({"revision":1,"action":"select","bounds":[2,2,3,3]}));
    let points = input(
        &[
            0i32.to_le_bytes(),
            2i32.to_le_bytes(),
            7i32.to_le_bytes(),
            2i32.to_le_bytes(),
        ]
        .concat(),
    );
    assert_eq!(pc_stroke(h, 0, points, u32::MAX, 1), 0);
    assert_eq!(pc_stroke(h, 2, 0, 0, 1), 0);
    pc_buffer_free(points);
    let rendered = bytes(pc_request(h, 7, 0, 0, 0)).1;
    assert_eq!(&rendered[(2 * 8 + 2) * 4..(2 * 8 + 2) * 4 + 4], &[255; 4]);
    assert_eq!(&rendered[(2 * 8 + 1) * 4..(2 * 8 + 1) * 4 + 4], &[0; 4]);
    let revision = state(h, 2, 0, 0, 0)["revision"].clone();
    let copied = act(serde_json::json!({"revision":revision,"action":"copy"}));
    let pasted =
        act(serde_json::json!({"revision":copied["revision"],"action":"paste","x":4,"y":4}));
    let out = bytes(pc_request(h, 7, 0, 0, 0)).1;
    assert_eq!(&out[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 4], &[255; 4]);
    assert_eq!(pasted["undo"], 2);
    let _ = selected;
    pc_session_close(h);
}

#[test]
fn playback_renders_frames_without_changing_edit_target_and_uses_core_tag_order() {
    let _lock = TEST.lock().unwrap();
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let packet=input(br#"{"revision":1,"commands":[{"op":"add-frame","frame":0,"duplicate":false},{"op":"set-frame","frame":0,"duration":75},{"op":"set-frame","frame":1,"duration":125},{"op":"shape","layer":0,"frame":0,"kind":"line","x":1,"y":1,"x2":1,"y2":1,"color":[200,10,20,255]},{"op":"add-tag","tag":{"name":"attack","from":0,"to":1,"direction":1,"repeat":2,"color":[40,80,120]}}]}"#);
    state(h, 17, 0, 0, packet);
    pc_buffer_free(packet);
    state(h, 9, 1, 0, 0);
    let before = state(h, 2, 0, 0, 0);
    let frame = bytes(pc_request(h, 7, 0, 1, 0));
    assert_eq!(&frame.1[(8 + 1) * 4..(8 + 1) * 4 + 4], &[200, 10, 20, 255]);
    assert_eq!(state(h, 2, 0, 0, 0), before);
    let onion = bytes(pc_request(h, 21, 1, 0, 0));
    assert_eq!(onion.0, 2);
    assert_eq!(&onion.1[(8 + 1) * 4..(8 + 1) * 4 + 4], &[150, 185, 220, 64]);
    assert_eq!(state(h, 2, 0, 0, 0), before);
    let sequence = bytes(pc_request(h, 22, 0, 1, 0));
    assert_eq!(sequence.0, 6);
    let sequence: Value = serde_json::from_slice(&sequence.1).unwrap();
    assert_eq!(sequence["sequence"], serde_json::json!([1, 0]));
    assert_eq!(sequence["durations"], serde_json::json!([125, 75]));
    pc_session_close(h);
}

#[test]
fn mobile_formats_export_reimport_and_fail_atomically() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 1, 0, 0, 0)["session"].as_u64().unwrap();
    let before = state(h, 2, 0, 0, 0);
    let pixels = bytes(pc_request(h, 7, 0, 0, 0)).1;
    for format in 1..=5 {
        let (kind, encoded) = bytes(pc_request(h, 24, format, 0, 0));
        assert_eq!(
            kind,
            3,
            "format {format}: {}",
            String::from_utf8_lossy(&encoded)
        );
        assert!(!encoded.is_empty());
        assert_eq!(bytes(pc_request(h, 24, format, 0, 0)).1, encoded);
        if format <= 3 {
            let data = input(&encoded);
            let imported = state(h, 23, format, 0, data);
            assert_eq!(imported["dirty"], false);
            assert_eq!(imported["undo"], 0);
            assert_eq!(imported["width"], before["width"]);
            assert_eq!(bytes(pc_request(h, 7, 0, 0, 0)).1, pixels);
            pc_buffer_free(data);
        }
    }
    let snapshot = state(h, 2, 0, 0, 0);
    let malformed = input(b"malformed");
    for format in 1..=4 {
        assert_eq!(bytes(pc_request(h, 23, format, 0, malformed)).0, 0);
        assert_eq!(state(h, 2, 0, 0, 0), snapshot);
    }
    let mut oversized =
        pixelcraft_core::formats::encode_ase(&pixelcraft_core::model::Document::new(1, 1)).unwrap();
    oversized[8..10].copy_from_slice(&257u16.to_le_bytes());
    let oversized = input(&oversized);
    assert_eq!(bytes(pc_request(h, 23, 3, 0, oversized)).0, 0);
    assert_eq!(state(h, 2, 0, 0, 0), snapshot);
    pc_buffer_free(oversized);
    assert_eq!(bytes(pc_request(0, 23, 1, 0, malformed)).0, 0);
    pc_buffer_free(malformed);
    pc_session_close(h);
}

#[test]
fn advanced_tiles_references_are_atomic_undoable_and_persisted() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 0, 8, 8, 0)["session"].as_u64().unwrap();
    let packet = input(br#"{"revision":1,"commands":[{"op":"shape","layer":0,"frame":0,"kind":"line","x":0,"y":0,"x2":0,"y2":0,"color":[255,0,0,255]}]}"#);
    let current = state(h, 17, 0, 0, packet);
    pc_buffer_free(packet);
    let act = |value: Value| {
        let packet = input(&serde_json::to_vec(&value).unwrap());
        let result = state(h, 25, 0, 0, packet);
        pc_buffer_free(packet);
        result
    };
    let tiled = act(
        serde_json::json!({"revision":current["revision"],"action":"create-tileset","tile_width":4,"tile_height":4}),
    );
    assert_eq!(tiled["layer"], 1);
    assert_eq!(tiled["layers"][1]["tileset"], 0);
    assert_eq!(tiled["tilesets"][0]["count"], 1);
    let painted = act(
        serde_json::json!({"revision":tiled["revision"],"action":"paint-tile","x":4,"y":0,"tile":1}),
    );
    assert_eq!(
        &bytes(pc_request(h, 7, 0, 0, 0)).1[4 * 4..5 * 4],
        &[255, 0, 0, 255]
    );
    let png = bytes(pc_request(h, 24, 1, 0, 0)).1;
    let packet = input(&png);
    let reference = state(
        h,
        26,
        painted["revision"].as_u64().unwrap() as u32,
        0,
        packet,
    );
    pc_buffer_free(packet);
    assert_eq!(reference["references"][0]["width"], 8);
    assert_eq!(bytes(pc_request(h, 27, 0, 0, 0)).0, 2);
    let updated = act(
        serde_json::json!({"revision":reference["revision"],"action":"reference-set","index":0,"x":1.0,"y":2.0,"scale":2.0,"opacity":128,"visible":true}),
    );
    assert_eq!(updated["references"][0]["opacity"], 128);
    let saved = bytes(pc_request(h, 5, 0, 0, 0)).1;
    let packet = input(&saved);
    let reopened = state(h, 6, 0, 0, packet);
    pc_buffer_free(packet);
    assert_eq!(reopened["references"], updated["references"]);
    assert_eq!(reopened["tilesets"], updated["tilesets"]);
    let converted = act(
        serde_json::json!({"revision":reopened["revision"],"action":"reference-to-layer","index":0}),
    );
    assert_eq!(converted["layers"].as_array().unwrap().len(), 3);
    let removed = act(
        serde_json::json!({"revision":converted["revision"],"action":"reference-remove","index":0}),
    );
    assert!(removed["references"].as_array().unwrap().is_empty());
    assert_eq!(
        state(h, 3, 0, 0, 0)["references"].as_array().unwrap().len(),
        1
    );
    let snapshot = state(h, 2, 0, 0, 0);
    let packet = input(br#"{"revision":0,"action":"reference-remove","index":0}"#);
    assert_eq!(bytes(pc_request(h, 25, 0, 0, packet)).0, 0);
    pc_buffer_free(packet);
    assert_eq!(state(h, 2, 0, 0, 0), snapshot);
    let packet = input(b"malformed");
    assert_eq!(
        bytes(pc_request(
            h,
            26,
            snapshot["revision"].as_u64().unwrap() as u32,
            0,
            packet
        ))
        .0,
        0
    );
    pc_buffer_free(packet);
    assert_eq!(state(h, 2, 0, 0, 0), snapshot);
    pc_session_close(h);
}

#[test]
fn prepared_import_cancels_preserves_active_state_and_rejects_stale_commit() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 1, 0, 0, 0)["session"].as_u64().unwrap();
    state(h, 9, 1, 0, 0);
    let selected = state(h, 2, 0, 0, 0);
    let edit=input(&serde_json::to_vec(&serde_json::json!({"revision":selected["revision"],"commands":[{"op":"set-frame","frame":1,"duration":120}]})).unwrap());
    let before = state(h, 17, 0, 0, edit);
    pc_buffer_free(edit);
    let project =
        pixelcraft_core::formats::encode_project(&pixelcraft_core::model::Document::new(3, 2))
            .unwrap();
    let project = input(&project);
    let original_artwork = bytes(pc_request(h, 5, 0, 0, 0)).1;
    let (kind, prepared) = bytes(pc_request(h, 28, 0, 0, project));
    assert_eq!(kind, 6, "{}", String::from_utf8_lossy(&prepared));
    assert_eq!(state(h, 2, 0, 0, 0), before);
    assert_eq!(bytes(pc_request(h, 5, 0, 0, 0)).1, original_artwork);
    assert_eq!(bytes(pc_request(h, 28, 0, 0, project)).0, 0);
    assert_eq!(state(h, 2, 0, 0, 0), before);
    assert_eq!(
        serde_json::from_slice::<Value>(&prepared).unwrap()["prepared"],
        true
    );
    assert_eq!(state(h, 2, 0, 0, 0), before);
    assert_eq!(bytes(pc_request(h, 28, 0, 0, project)).0, 0);
    assert_eq!(state(h, 30, 0, 0, 0), before);
    assert_eq!(
        bytes(pc_request(
            h,
            29,
            before["revision"].as_u64().unwrap() as u32,
            0,
            0
        ))
        .0,
        0
    );
    assert_eq!(state(h, 2, 0, 0, 0), before);
    assert_eq!(bytes(pc_request(h, 5, 0, 0, 0)).1, original_artwork);
    assert_eq!(
        bytes(pc_request(
            h,
            29,
            before["revision"].as_u64().unwrap() as u32,
            0,
            0
        ))
        .0,
        0
    );
    assert_eq!(bytes(pc_request(h, 28, 0, 0, project)).0, 6);
    let changed = state(h, 3, 0, 0, 0);
    assert_eq!(
        bytes(pc_request(
            h,
            29,
            changed["revision"].as_u64().unwrap() as u32,
            0,
            0
        ))
        .0,
        0
    );
    assert_eq!(state(h, 2, 0, 0, 0), changed);
    state(h, 30, 0, 0, 0);
    let malformed = input(b"not artwork");
    assert_eq!(bytes(pc_request(h, 28, 1, 0, malformed)).0, 0);
    assert_eq!(state(h, 2, 0, 0, 0), changed);
    assert_eq!(bytes(pc_request(h, 28, 0, 0, project)).0, 6);
    let imported = state(h, 29, changed["revision"].as_u64().unwrap() as u32, 0, 0);
    assert_eq!(imported["width"], 3);
    assert_eq!(imported["height"], 2);
    assert_eq!(imported["dirty"], false);
    assert_eq!(imported["undo"], 0);
    assert_eq!(imported["redo"], 0);
    assert_eq!(imported["frame"], 0);
    assert_eq!(imported["layer"], 0);
    pc_buffer_free(project);
    pc_buffer_free(malformed);
    pc_session_close(h);
}

#[test]
fn variable_pressure_segments_keep_one_undo_step_and_cancel_all() {
    let _test = TEST.lock().unwrap();
    let h = state(0, 0, 16, 16, 0)["session"].as_u64().unwrap();
    assert_eq!(bytes(pc_request(h, 31, 3, 0, 0)).0, 0);
    let start = input(&[2i32.to_le_bytes(), 2i32.to_le_bytes()].concat());
    let end = input(&[8i32.to_le_bytes(), 8i32.to_le_bytes()].concat());
    assert_eq!(pc_stroke(h, 0, start, u32::MAX, 1), 0);
    let before = state(h, 2, 0, 0, 0);
    assert_eq!(bytes(pc_request(h, 31, 0, 0, 0)).0, 0);
    assert_eq!(bytes(pc_request(h, 31, 33, 0, 0)).0, 0);
    assert_eq!(state(h, 31, 3, 0, 0), before);
    assert_eq!(pc_stroke(h, 1, end, u32::MAX, 1), 0);
    assert_eq!(
        &bytes(pc_request(h, 7, 0, 0, 0)).1[(8 * 16 + 9) * 4..(8 * 16 + 10) * 4],
        &[255, 255, 255, 255]
    );
    assert_eq!(pc_stroke(h, 2, 0, 0, 1), 0);
    assert_eq!(state(h, 2, 0, 0, 0)["undo"], 1);
    state(h, 3, 0, 0, 0);
    let original = bytes(pc_request(h, 7, 0, 0, 0)).1;
    assert_eq!(pc_stroke(h, 0, start, u32::MAX, 1), 0);
    state(h, 31, 5, 0, 0);
    assert_eq!(pc_stroke(h, 1, end, u32::MAX, 1), 0);
    assert_eq!(pc_stroke(h, 3, 0, 0, 1), 0);
    assert_eq!(bytes(pc_request(h, 7, 0, 0, 0)).1, original);
    assert_eq!(state(h, 2, 0, 0, 0)["undo"], 0);
    pc_buffer_free(start);
    pc_buffer_free(end);
    pc_session_close(h);
}
