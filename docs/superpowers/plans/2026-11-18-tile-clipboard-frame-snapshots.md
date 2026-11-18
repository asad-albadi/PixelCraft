# Tile Clipboard Frame Snapshot Plan

**Goal:** Copy the visible frame's tile colors and retain source color mode/mask when pasting as a new sprite.

- [x] Reproduce native frame-color bitmap/new-sprite failures and Lua detached tileset snapshot failure.
- [x] Capture indexed tile colors using effective frame palette without changing source; preserve authored slots and source mode/mask in backward-compatible clipboard descriptors.
- [x] Native new-sprite paste restores mode/mask before adding tiles; Lua snapshots retain descriptor metadata across detach/copy.
- [x] Review, Windows all-target pipeline, docs and push.

Cross-palette destination slot remapping remains the following integration step; this checkpoint must not claim it complete.

## Verification

- Native and Lua tests reproduced ignored frame colors before the fix.
- Native indexed snapshot sampling retains shared result buffers and writes only changed pixels. Sampling is unconditional for indexed tiles because mask changes can affect pixels even with equal palettes.
- Reviewer-found equal-palette mask skip was reproduced and fixed. Bitmap and native paste-as-sprite regressions verify mask behavior.
- Backward-compatible descriptors default to RGBA/mask zero; new descriptors retain source mode/mask through serialization.
- 845 Windows all-target tests passed. A subsequent mask assertion for new-sprite paste also passed; formatting and strict all-target Clippy passed after that test-only addition.
- Independent review found no remaining actionable issue in this snapshot scope.
- Remaining integration: remap source slots into an existing destination palette, Lua palette fallback for empty frame entries, and measure/move expensive unique-tile snapshot work off the UI thread as warranted. This does not claim complete cross-palette paste.
