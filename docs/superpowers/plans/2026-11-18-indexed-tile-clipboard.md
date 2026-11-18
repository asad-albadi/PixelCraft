# Indexed Tile Clipboard Identity Plan

**Goal:** Preserve authored indexed tile slots in native and Lua clipboard matching and inserted definitions.

**Architecture:** Include optional authored indices in tile content keys and equality; preserve the full tile raster with fresh runtime identity when inserting. Lua keys use bounded packing with an explicit metadata boundary so variable index/metadata lengths cannot collide. Keep legacy RGBA-only behavior and existing metadata/placement semantics. Cross-palette interpretation remains a separate source-audit task.

- [x] Reproduce native wrong-slot reuse and metadata loss, plus Lua wrong-slot reuse using real copy/paste.
- [x] Preserve tile slot identity in both content matching paths and native inserted tile rasters.
- [x] Verify repeated paste reuse, flags, fresh runtime identity, undo/redo and native/ASE persistence.
- [x] Independent review, formatting, strict Clippy and Windows all-target tests; update audit evidence, commit and push.

## Evidence

- Both native and Lua tests failed before the fix because palette-slot-distinct tiles were incorrectly reused.
- Native insertion now clones the complete raster and replaces its runtime identity. Hash and exact comparison both include optional slot arrays.
- Lua keys include explicit presence and pixel/slot lengths; slot bytes are packed in groups of at most 256.
- Native editor actions and Lua scripts verify repeat reuse, undo/redo and persistence; native editor test also verifies flip flags. A separate 1,024-pixel indexed Lua clipboard case exercises bounded packing.
- Windows: 824 all-target tests passed, then the additional large-tile test passed. Formatting and strict all-target Clippy passed after that addition. No production changes followed the full suite.
- Independent review found no actionable correctness, security or performance finding in this scope.
- Cross-palette clipboard interpretation and new-sprite color mode remain open integrations; frame-paletted rendering remains a separate next requirement.
