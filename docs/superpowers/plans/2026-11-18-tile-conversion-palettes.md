# Indexed Tile Conversion Palette Plan

**Goal:** Convert shared indexed tile rasters with the effective first-frame palette, and active-frame snapshots with their selected palette, matching source-audited conversion behavior.

**Source evidence:** Pinned Aseprite src/app/cmd/set_pixel_format.cpp converts tile images using frame zero, with other-frame variants left as TODO. PixelCraft snapshots move the selected frame to zero.

- [x] Red tests for RGB, grayscale, snapshot and direct mode conversion with differing base/first palettes.
- [x] Correct worker and direct conversions while retaining authored slot validation, stale/legacy fallback, mask handling, cancellation and COW reuse.
- [x] Verify short frame palettes, source immutability, sharing and native/ASE persistence.
- [x] Independent review, Windows tests, formatting and strict Clippy; document and push.

## Integration decisions and evidence

- Red/green tests confirmed worker/direct conversion used base colors instead of the effective first palette.
- Source-based default remains frame zero for shared tiles. Explicit tile-frame model entrypoints validate selection and retain each raster cel's own palette.
- GUI Preview/Apply mismatch on later frames was reproduced with real editor actions. Both now select the displayed frame palette; undo restores source.
- Automation convert-rgb/convert-grayscale accepts optional frame/cel_id and defaults to zero. Lua ChangePixelFormat passes the active frame to the native host. Both missing integrations were reproduced before fixing.
- Tests cover RGB/gray snapshots, direct mode conversion, short palette/stale fallback, shared buffers, cancellation, native/ASE persistence, invalid selection and source immutability.
- Independent source review found no actionable issue after all integrations. Final Windows pipeline result is recorded in the master plan.
- Shared tiles remain shared across frames after conversion; no automatic per-frame variant generation is claimed. The pinned source baseline also leaves such generation as TODO.
