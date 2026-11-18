# Active Frame Color Preview Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, with failing regressions before implementation.

**Goal:** Remove unrelated animation frames from color preview work and keep final conversion asynchronous and atomic.

**Architecture:** Add a native conversion-only frame snapshot that constructs one frame directly, without cloning every frame's metadata. Retain the base palette for tiles, the selected frame palette for raster cels, hidden diffusion neighbors, layer hierarchy/masks and sampled cel keyframes. Preview converts and renders this one-frame document at frame zero. Explicit Apply schedules the original full document on the worker and commits only if its source revision and conversion options still match. Ignore unrelated controls when identifying preview work.

**Tech Stack:** Rust, egui, current native converters, COW pixels and mpsc workers.

**Spec:** Large-animation color conversion preview follow-up in the issue #20 master plan. The source reference limits preview conversion to the visible frame region; this step removes unrelated frames while retaining full-frame dither context for exact results. Viewport-specific optimization remains an audit item.

## Invariants

- Preview source retains exactly one frame and no unrelated frame metadata; source pixels stay shared until converted.
- Preview pixels match full conversion of that original frame, including frame palettes, cel keyframe interpolation, reference geometry, tilemaps, masks and hidden diffusion neighbors.
- Preview does not change the live document, revision or history.
- Apply converts all frames on a worker; cancel, source edits and changed conversion options reject stale work.
- Frame and display-profile changes invalidate preview imagery but do not discard a valid pending whole-document conversion.
- Unused controls do not restart conversion work.

## Tasks

- [x] Reproduce full-animation preview work and unnecessary refreshes in GUI tests; verify Apply remains asynchronous and affects every frame.
- [x] Add and test conversion_frame_snapshot and exact native conversion/render equivalence across modes and dithering.
- [x] Route preview and Apply through their correct worker stages, including coalescing and stale result checks. Update confirmation tests.
- [x] Benchmark representative large animations, report full versus selected-frame conversion work, and retain the scope of unverified viewport and real-project performance checks.
- [x] Run formatting, strict Clippy and all Windows tests; update docs, review, commit and push.

## Verification checkpoint

Formatting, strict all-target Clippy, and 798 Windows all-target tests passed. Two additional ignored manual benchmarks were run explicitly. The new GUI regression first reproduced a two-frame preview and now verifies that only the selected frame is converted, unrelated controls keep the same preview, and Apply leaves source pixels untouched until a full-document worker completes. Pending Apply survives frame/display changes and rejects intervening source edits. Snapshot tests cover palette variants, hidden neighbors at negative coordinates and shifted diffusion phase, sampled keyframes, groups, masks, reference bounds, tilemaps, tile base palettes and display profiles.

## Debug benchmark evidence

The synthetic 480x270 animation with 8 layers and 24 unique frames took 2.242 s for full luminance conversion and 93.8 ms for snapshot plus selected-frame conversion (23.9x). The saved daytime project has 23 layers and 24 frames: full conversion 2.565 s, selected-frame snapshot plus conversion 103.2-105.1 ms. Moonlit Sanctuary has 34 layers and 24 frames: 3.668 s full, 150.5-153.6 ms selected frame. Frames 0, 12 and 23 matched full conversion pixel-for-pixel on both saved projects. These debug-build timings measure native conversion and snapshot preparation, excluding file loading, canvas compositing and texture upload. Original projects were read only.

Run the representative benchmark with cargo test --test conversion_snapshot benchmark_active_frame -- --ignored --nocapture. Run saved-project checks by setting PIXELCRAFT_CONVERSION_BENCH_PROJECTS to a JSON array of project paths, then cargo test --test conversion_snapshot benchmark_saved_projects -- --ignored --nocapture.

## Remaining scope

Viewport-specific conversion remains open, especially for very large single frames and unused tile definitions. Preserve exact dithering origin/context and native rendering when bounding that work. The overall issue #20 parity goal remains active.
