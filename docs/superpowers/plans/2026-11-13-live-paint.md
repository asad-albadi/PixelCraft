# Live Painting Implementation Plan

> **For agentic workers:** Use superpowers:test-driven-development for each behavior and superpowers:verification-before-completion for handoff.

**Goal:** Paint and inspect a layered 480×270 scene in the live editor, then export it at 4×.

**Architecture:** Extend the existing atomic command batch with bounded RGBA patches and native stamps. Prepare commands and encode previews on a worker, then commit on the editor thread after checking the live revision again.

**Tech Stack:** Rust, egui, serde JSON, PNG, MCP stdio and the existing local bridge.

**Spec:** GitHub issue #5 and the approved hybrid design from the research discussion.

## Tasks

- [x] Extend the atomic command batch with bounded raster patches, native stamps, region edits, polygons, and per-cel changed bounds. Cover clipping, alpha, malformed inputs, seeded replay, and unrelated cels.
- [x] Return revision-tagged PNGs and metadata with crop, layer filtering, and integer zoom. Reuse the existing scaler for live export.
- [x] Wake egui for incoming requests and worker completions. Recheck the revision, deadline, and gesture before committing. Cover stale commits and regular undo history.
- [x] Document the MCP workflow and exercise six passes, undo/redo, and exact 4× export over the loopback bridge on a 12-layer canvas.
- [ ] Enable the new desktop instance's live access, verify request-to-visible latency while panning and zooming, and compare the actual Eversong artwork with the reference after each pass.

## Review Focus

Exercise negative origins, malformed raster payloads, locked and non-image layers, a tab or gesture change during preparation, and exact output pixels at 4×.
