# Referenced Tile Color Preview Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline and verify a failing regression before implementation.

**Goal:** Avoid converting unused tilesets and tile definitions in an active-frame preview.

**Architecture:** During conversion_frame_snapshot, collect placed nonzero tile IDs across every layer in the selected frame. Keep each referenced tileset once and its used definitions in source order. Remap tileset indices and tile IDs while preserving transform bits, geometry, COW pixel buffers and retained ASE tileset annotations. Do not modify the source or final full-document conversion.

**Tech Stack:** Rust model snapshot, existing native converters, Windows cargo pipeline.

**Spec:** The remaining unused tile definition work in 2026-11-18-active-frame-color-preview.md and the source-audit performance follow-up in the issue #20 master plan.

## Invariants

- Multiple layers sharing a tileset contribute a union of referenced definitions and keep sharing one projected tileset.
- Source tile order determines stable projected IDs; zero stays empty and high transform bits survive remapping.
- Empty tilemaps retain valid tileset dimensions without copying unused definitions.
- Native preview rendering matches full conversion for RGB, grayscale and indexed matching/dithering, including reference cels in composite_view.
- Final Apply retains original definitions, placement IDs and export metadata because it operates on the original document.

## Tasks

- [x] Write and reproduce a failing regression with unused sets, other-frame-only definitions, shared placements and transform flags.
- [x] Implement bounded referenced-definition collection and ID remapping; verify empty maps and malformed current-frame references.
- [x] Compare native conversions and composite_view against the full animation across matching and dither modes.
- [x] Run a representative unused-tile benchmark, formatting, strict Clippy and Windows all-target tests.
- [x] Update docs, review, commit and push. Viewport-specific work remains open.

## Verification checkpoint

Formatting, strict all-target Clippy, and 801 Windows all-target tests passed. The first failing regression reproduced both unused-set and other-frame-only definition retention. Current tests verify shared sets, stable source-order compaction, transform and empty-ID flags, ASE annotation remapping, COW storage, empty maps, invalid selected-frame references, and exact composite_view results across every matching criterion and nearest/ordered/Floyd-Steinberg mode. The existing reference-layer regression now compares editor composite_view rather than export-only compositing. A GUI confirmation test proves full Apply retains all original definitions, placement IDs, frame names and other frames, with one-step undo.

The explicitly run ignored benchmark used 1,024 unique 64x64 definitions with one placed tile. Full luminance conversion took 255.562 ms versus 375.9 microseconds for referenced snapshot plus conversion (679.9x) in this debug run. It verifies exact rendered pixels and measures native conversion/snapshot preparation, excluding loading and texture upload. Viewport-specific work is still open.
