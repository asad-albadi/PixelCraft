# Editor expansion

Extend existing document operations and UI, retaining transactional validation and Undo.

1. Regression coverage: selection history; repeat metadata, Godot loops/names; scaling; palette remapping; effects; assistant operations.
2. Artwork: history snapshots include rectangle/mask, RGBA system clipboard, masked outline/shadow, palette permutation and interpolated ramps.
3. Interchange: bounded integer nearest-neighbor PNG/sheet scaling with matching metadata, BMP, deterministic folder atlas inputs, dropped files.
4. Workflow UI: persisted scale/checker/autosave controls and enhanced reference placement, floating previews/sampling and rasterization.
5. Assistants: transactional create/layer/frame/tag/shape commands with revision guards and live Undo.
6. Runtime: wgpu first with OpenGL fallback; optional background verified update downloads and explicit install confirmation.
7. Validate: Cargo formatting, tests, build, headless checks, Craft Anything rebuild_check; resolve failures.
