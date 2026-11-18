# Indexed Tile Slot Preservation Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, with failing regressions before implementation.

**Goal:** Preserve authored palette slot identity in indexed tile artwork across files, native edits/conversion and scripting.

**Architecture:** TileRaster owns optional shared index bytes alongside COW RGBA pixels, matching cel metadata semantics. Preserve legacy plain-raster serialization when indices are absent; use a bounded typed wrapper when they are present. ASE decoding captures raw indexed tile bytes, and encoding retains matching authored slots. Lua normalizes wrapped rasters to its existing private arrays while carrying private slot metadata through host calls and final serialization.

**Tech Stack:** Rust/serde, existing budgeted decoders, ASE codec, Lua compatibility layer.

**Spec:** Source audit indexed tile palette-slot identity follow-up in the issue #20 master plan. Pinned reference tileset images retain typed indexed storage.

## Invariants

- Equal RGBA colors do not imply equal palette slots.
- Legacy native/JSON projects still load; new index arrays remain bounded and match tile dimensions.
- Native project and recovery serialization preserve index bytes without exposing runtime editor IDs.
- Conversion to RGB/grayscale resolves indexed samples with the base palette and clears index metadata; indexed conversion preserves/remaps authored slots where applicable.
- Unchanged Lua tile images keep slots, numeric indexed painting selects explicit slots, and undo/redo and host conversions retain live handles.
- Final exports and revisions distinguish authored slot changes while preserving old RGBA-only behavior.

## Tasks

- [x] Reproduce no-edit ASE round-trip loss using raw duplicate-color indices in a compressed tileset fixture.
- [x] Add bounded TileRaster serialization and index memory accounting; capture and write ASE indices and preserve external indexed definitions.
- [x] Verify native saves/recovery, legacy representations and malformed metadata.
- [x] Preserve slot metadata through native conversions, duplication, drawing and geometry changes; test affected paths.
- [x] Normalize Lua wire rasters, live tile image reads/writes, snapshots, host operations and undo/redo; verify no-op and edited round trips.
- [ ] Review further palette, clipboard and transform paths for indexed tile identity. Keep any uncovered path open until verified.
- [x] Run formatting, strict Clippy and Windows tests, update docs, commit and push verified checkpoints.

## Verified checkpoint

The original failing compressed ASE fixture used palette slots [2,1] with equal RGBA colors and round-tripped as [1,1]. Optional shared indices now belong to each TileRaster. Legacy dense/sparse native representations still load, new wrappers use the existing budgeted pixel/index decoders, and index bytes count toward document memory. ASE, native projects and workspace recovery retain authored bytes. External imported definitions retain slot metadata while receiving fresh document-local runtime identities.

Native indexed conversion updates slot bytes and transparent-slot remaps; RGB/grayscale clear indices without resurrecting stale painted colors. Duplicates retain metadata with fresh editing identities. Shared painting updates selected slots in place without cloning a unique raster per pixel; reuse/create variants distinguish identical RGBA with different slots. Sprite flips and all three resize methods carry slots through geometry; the resize regression checks exact placement rather than length alone.

Lua uses private normalized raster arrays with typed wire wrappers. Reads and numeric writes, clone independence, image replacement, host conversions, live handles, undo/redo, and final serialization have regressions. Legacy 64x64 tile iteration initially exceeded the script budget because each pixel read recomputed every slot; fallback now quantizes one requested sample. Same-mode validation keeps absent legacy metadata absent, avoiding read-only document edits.

Formatting, strict all-target Clippy and 815 Windows all-target tests passed. A strengthened exact-slot resize assertion passed afterward without production changes. Independent Superpowers code review identified clone aliasing, stale replacement slots, conversion clobbering and quadratic reads; their red/green regressions pass. A final external-runtime-identity regression also reproduced and now passes. Re-review found no additional important regression or security finding in its inspected scope; this is not a complete branch security audit.

## Remaining integrations

Palette permutations/ramps, clipboard identity and deduplication, additional image API operations, frame-paletted tile rendering, cross-document external palette interpretation and strict shared-paint allocation preflight remain open audits. Viewport conversion work is still open separately. Do not claim complete tile-slot or editor parity from this checkpoint.
