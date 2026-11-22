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
