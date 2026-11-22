# Android Tablet Proof of Concept Implementation Plan

**Goal:** Run shared-engine drawing, Undo/Redo and native save/reopen on the visible tablet emulator.
**Architecture:** Flutter interface → versioned C ABI → core EditorSession, on a serialized Dart worker isolate.
**Tech stack:** Existing Rust 1.95.0/core, Flutter 3.47.9/Dart 3.13.7, installed Android NDK 28.2.13676358, API 35 tablet emulator.
**Spec:** ../specs/2026-11-22-android-tablet-poc-design.md

## Constraints

- Existing desktop, CLI and MCP behavior must remain intact.
- This completes a PoC milestone, not all of issue #29 or real-device acceptance.
- Native memory, session count, point batches and project decoding are bounded.
- Keep emulator visible; do not kill it during cleanup or final handoff.

## Batches

- [x] Baseline/setup: open tablet, branch from merged main, document scope and record workspace tests.
- [x] Native boundary: test-first bounded project decoder and FFI handles/strokes/history/round trips; add Android target libc gate, x86_64/arm64 build script and header. Run host/Android builds and independent code/security review.
- [ ] Tablet application: Flutter worker/bridge, canvas and gestures, responsive controls, native private storage and document picker; widget tests and platform error handling. Build/install and inspect on the visible emulator, reviewing/fixing findings.
- [ ] Integration: emulator-driven drawing/Undo/save/reopen, memory/latency observations and screenshots. Complete desktop, CLI/MCP and strict lint regressions; record limitations and review results. Keep tablet open with working app.

## Evidence and decisions

Ruling: execute the user-approved issue's PoC milestone directly in reviewable
batches; keep the remaining issue checklist open rather than claiming full port.

The existing tablet initially could not connect to Xwayland because the X0 socket
path was missing while its X0_ listener remained active. Restored a symlink to
that active same-user listener and launched the emulator visibly with Qt xcb.
No emulator data was wiped. Device: emulator-5554, 2560×1600, density 320, API 35.

Native batch: baseline 811 tests passed; bounded decoder 2 tests and native ABI
5 tests passed; core/FFI strict Clippy passed; x86_64 and arm64 Android libraries
built. Independent review found runtime sparse allocation growth and save/reopen
budget mismatch; both fixed and covered by rollback/round-trip tests. Final
native review found no blockers. Desktop limits retain their original defaults.
