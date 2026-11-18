# Indexed Tile Palette Remapping Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline; reproduce failures before implementation.

**Goal:** Preserve authored indexed tile identity through palette sorting, reversal and span ramps.

**Architecture:** Palette permutations remap tile indices using the existing shared-index cache. Span replacement uses the original base palette to resolve valid authored tile slots, then maps slot bytes and cached colors with the base ramp. Keep RGBA-only legacy fallback behavior, transparent masks, COW storage and shared buffers. Reject or safely reconcile stale slot/color data rather than resurrecting overwritten pixels.

**Tech Stack:** Rust palette model, existing ASE/native fixtures and Windows regression pipeline.

**Spec:** Remaining palette permutations/ramps integration in 2026-11-18-indexed-tile-slot-preservation.md and issue #20 source parity plan.

## Tasks

- [x] Reproduce reversal/sort, duplicate-color ramp and transparent-slot ramp failures with authored tile bytes.
- [x] Remap tile indices during permutations, retaining shared buffers and unchanged artwork.
- [x] Apply index-aware tile ramps atomically, retaining transparency and legacy fallback semantics.
- [x] Verify native/ASE persistence, conversion consistency, shared storage, stale metadata and invalid inputs; test editor undo where relevant.
- [x] Run independent review, formatting, strict Clippy and Windows all-target tests; document, commit and push. Clipboard, frame-paletted rendering, other image API and viewport work remain open.

## Verified checkpoint

- Regression-first failures confirmed for duplicate-color reversal/sort and ramp replacement.
- Shared remapping spans cel/tile index arrays; ramp caching preserves shared tile backing and unchanged arrays.
- Tests cover mask relocation, native/ASE round trips, conversion consistency, stale raw edits, legacy metadata and atomic invalid input rejection.
- Independent source review found no concrete correctness, security or important performance issue in this scope. GUI palette operations use these model methods and their existing history transaction; this checkpoint does not claim a new manual GUI capture.
- Windows all-target results are recorded in the master plan after completion.
- Remaining source-audit work includes tile clipboard identity, frame-paletted rendering, additional image APIs and viewport work; overall goal remains active.
