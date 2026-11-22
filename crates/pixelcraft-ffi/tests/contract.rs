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
