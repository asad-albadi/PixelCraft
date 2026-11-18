# Color Conversion Canvas Preview Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline with test-first changes.

**Goal:** Preview RGB, grayscale and indexed conversion on the canvas before applying an undoable edit.

**Architecture:** Extend the existing palette worker with a settings snapshot and a preview result containing the converted document and a display-managed nearest-neighbor texture. Keep at most one worker in flight, cancel superseded settings, discard its result, and coalesce changes before starting the next worker. Reuse the completed document on Apply so preview and applied pixels agree. Existing palette generation and palette-only application remain direct actions.

**Tech Stack:** Rust, egui, existing mpsc/atomic cancellation and native conversion methods.

**Spec:** The source-audit preview gap in 2026-11-16-issue-20-workflow-parity.md, pinned ColorModeWindow preview behavior in Aseprite cmd_change_pixel_format.cpp.

## Invariants

- Preview never modifies the source document, revision or undo history.
- Parameter, source frame and display-profile changes invalidate stale textures and results.
- Source-document changes close the dialog and drop pending work.
- Cancel and closing the window discard previews. Apply commits once with existing palette-selection bookkeeping.
- Source colors, frame palettes, hidden artwork, tiles and animation frames are processed by the native converter. Full conversion stays on the worker and its completed document is reused on Apply.
- Renderer draws the preview on the real canvas, including repeated views; it never substitutes preview pixels into exports or the live document.

## Tasks

- [x] Add failing GUI regressions proving preview-before-Apply, parameter refresh, compare toggle, cancel, stale result rejection, and source preservation.
- [x] Add preview state, worker snapshots, coalescing, display-managed rendering and canvas texture selection.
- [x] Connect RGB/grayscale properties entry points and conversion buttons, explicit Apply and preview toggle; retain generation and palette-only apply behavior.
- [x] Update existing conversion UI tests for confirmation; verify source/frame/profile invalidation and one-step undo/redo.
- [x] Document, run formatting, strict Clippy and Windows all-target tests, review, commit and push. Keep the overall parity audit active.

## Verification checkpoint

Formatting, strict all-target Clippy, and 793 Windows all-target tests passed. The first regression reproduced immediate source mutation before confirmation. New pointer regressions verify actual canvas preview mesh selection, original/preview comparison, Apply and one-step undo/redo, Cancel and Escape during pending work, frame and display-profile refresh, stale source rejection, and confirmation controls at 640 by 480. A deterministic delayed worker regression reproduced the cancelled-worker stall when settings return to their original values; the restart now passes without overlapping submissions.

## Remaining performance audit

This implementation converts the full document off the UI thread and reuses it on Apply, giving exact native preview/application agreement. The pinned reference limits preview conversion to the visible frame region. Limit preview work for large animations in a follow-up, with exact dither-origin, neighboring hidden-pixel, tilemap, profile and frame-palette behavior, and a worker for final full-document Apply. Do not treat this checkpoint as performance parity for large conversion previews.
