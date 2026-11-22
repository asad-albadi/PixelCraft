# Android port progress (#29)

Progress is completed milestones / 14, rounded to the nearest percent. It is not
an estimate of effort or elapsed time. Only implementation **and** verification
complete a milestone. Physical-device evidence cannot be inferred from emulator
results. Stay on `feature/tablet-android-proof-of-concept`, preserve desktop/CLI/MCP.

Current: **29% (4/14)**.

- [x] 1. Shared Rust bridge, tablet canvas, drawing and Undo/Redo.
- [x] 2. Desktop-compatible saves and Android file picker.
- [x] 3. Autosave and recovery after interruptions.
- [x] 4. Brushes, colors, fill, shapes and eyedropper.
- [ ] 5. Selections, clipboard and transforms.
- [ ] 6. Layer and cel editing.
- [ ] 7. Frame editing, animation playback and tags.
- [ ] 8. Image import, export and sharing.
- [ ] 9. Indexed colors, tilemaps and reference images.
- [ ] 10. Phone layouts, keyboard/mouse/stylus input and accessibility.
- [ ] 11. Performance, cancellation and mobile memory limits.
- [ ] 12. Android build automation, test APKs and upgrade checks.
- [ ] 13. Complete workflow tests, visual checks, reviews and desktop/CLI/MCP regressions.
- [ ] 14. Physical tablet/phone and stylus validation.

## Design and execution

Continue the accepted issue #29 architecture. Expose existing Rust core and
validated automation functions through narrowly bounded ABI operations; avoid a
second editing engine. Flutter presents reachable panels/actions and owns input.
Native Android handles app-private recovery, provider access and sharing. Keep
buffer/session budgets and atomic edits; maintain native `.pixel` compatibility.
Use separate commits and tests per milestone. Run independent code/security review
loops for major batches, inspecting the still-visible tablet after installing each
functional batch. Record exact tests and known limitations here and in validation.

Ruling: the user explicitly authorized continuing the remaining functionality;
continue implementation without another permission/design handoff. The existing
issue supplies architecture and requirements. No release or signing secret changes
are authorized or required. Hardware-only testing remains pending until connected.

## Recovery batch evidence

Versioned bounded snapshots retain current/saved artwork and frame/layer; private
atomic autosave after edits and backgrounding; startup Recover/Discard decision.
Native malformed-envelope and dirty-baseline tests pass;9 Flutter tests pass;
actual emulator integration passes2 cases including same-controller autosave/Save
and recreated editor recovery. Also force-stopped and relaunched the normal app:
recovery banner appeared and restored the unsaved sprite visually. Review found
startup, save-clean identity and discard handoff races; fixed, rechecked and
covered startup blocking. Failed New/Open resumes autosave. Desktop `.pixel`
serialization is unchanged. Undo history is not persisted in recovery snapshots.

## Drawing tools batch evidence

Fill, line/rectangle/ellipse gestures, filled shapes, brush shapes/sizes,
composite eyedropper and saved named ARGB colors reuse core operations. Actual
emulator integration:3 cases pass;11 widget tests pass, including palette Undo
selection and large-text/signed-hex validation. Visually inspected new scrollable
tool rail and left controls with preview still pinned bottom-left. Reviewed and
fixed stale palette dropdown state, native brush preference synchronization,
disposed-dialog callbacks and strict color parsing. No remaining tool-review
findings. Selection adapter and document management are the next active batch.

User decision: use emulators for now; physical hardware milestone14 explicitly
deferred. Keep the denominator14 and report at most93% until that evidence exists.

## Zen and tablet UX batch (2026-11-22)

Added opt-in Zen presentation: canvas plus Exit, Undo/Redo, pencil, eraser, color,
brush size and a temporary tool menu. Colors/Preview/timeline are hidden until
exit; artwork, active edit target, history, grid visibility and zoom/pan survive.
Android Back and visible Exit cancel any draft before restoring the normal UI.
Controls can move left/right on tablets and wrap on narrow/short screens. System
navigation stays available. This is part of milestone 10, which remains open for
broader input/accessibility checks, not a separate completion milestone.

Research and source links: android-tablet-ux-research.md. Findings about hidden
exit and missing Undo are qualitative user reports, not population-wide evidence.
20 Flutter tests pass, including Zen view/grid preservation, Back, explicit Exit
during a shape, overlay touch isolation and narrow/short large-text layouts.
Flutter analysis is clean. Review found and fixed explicit-exit draft cancellation
and grid reset; reviewer rechecked both with no remaining Zen-specific findings.
Emulator interaction verification is recorded separately after the device run.

Device verification: all five API 35 tablet integration tests pass, including
real pointer drawing in Zen, one-step Undo, visible Exit and normal Preview
restoration. Other cases exercise durable save/reopen, recovery, Rust tools,
selection transforms, linked cels, names/tags/timing and read-only playback.
