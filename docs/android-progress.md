# Android port progress (#29)

Progress is completed milestones / 14, rounded to the nearest percent. It is not
an estimate of effort or elapsed time. Only implementation **and** verification
complete a milestone. Physical-device evidence cannot be inferred from emulator
results. Stay on `feature/tablet-android-proof-of-concept`, preserve desktop/CLI/MCP.

Current: **93% (13/14)**.

- [x] 1. Shared Rust bridge, tablet canvas, drawing and Undo/Redo.
- [x] 2. Desktop-compatible saves and Android file picker.
- [x] 3. Autosave and recovery after interruptions.
- [x] 4. Brushes, colors, fill, shapes and eyedropper.
- [x] 5. Selections, clipboard and transforms.
- [x] 6. Layer and cel editing.
- [x] 7. Frame editing, animation playback and tags.
- [x] 8. Image import, export and sharing.
- [x] 9. Indexed colors, tilemaps and reference images.
- [x] 10. Phone layouts, keyboard/mouse/stylus input and accessibility.
- [x] 11. Performance, cancellation and mobile memory limits.
- [x] 12. Android build automation, test APKs and upgrade checks.
- [x] 13. Complete workflow tests, visual checks, reviews and desktop/CLI/MCP regressions.
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

## Selection, layer/cel and animation batch

Milestones 5–7 pass native and actual tablet workflow tests: selection clipboard,
flip/rotate/crop; layer/cel edits and links; frame names, timings, tags and
read-only playback. The bottom timeline now displays selectable layer/frame
intersections with inline visibility/lock and per-layer menus. Frame/Layer buttons
open context-specific panels, not the rejected four-tab property form. Selection
actions sit beside the canvas with an explicit path to all options. Preview stays
bottom-left. Tablet normal UI and layer panel inspected after reinstalling.

Review fixes: eye/lock preserve active drawing target; properties Apply disables
during autosave; selection options remain reachable; timeline scales header/cell
geometry with large text and scrolls labels/cells together. Regression tests
cover each behavior. Most recent workspace regression: 835 passed,16 ignored;
this precedes final advanced adapter changes and is rerun at the final gate.

## Bounded formats and advanced documents

Native and tablet integration now verify PNG/APNG/GIF/ASE decode/encode and
matching sheet metadata; failed imports preserve artwork. Staged imports permit
cancellation before atomic commit without losing Undo or dirty state. The tablet
exported a 48×48 RGBA PNG through SAF and showed the actual Android share preview.
Persisted export-folder and repeated-sheet pairing validation is the remaining
file-workflow gate before milestone 8 is ticked.

Milestone 9 verified: indexed conversions, populated tilemap creation/stamping,
embedded references with transforms/visibility/opacity/conversion/removal, native
save/reopen and Undo. Existing Rust model/workflows implement all artwork edits.
Limits and deliberate interchange differences are in the mobile README. Eight
tablet workflows now pass. The performance probe measured a 256-point stroke on
a 256×256 canvas at 10 ms and 50 decoded canvas updates during 800 ms of playback;
these are emulator worker/update measurements, NOT physical drawing latency or
display-presentation FPS guarantees. Phone validation continues separately.

## Phone, input, performance and delivery gates

Milestones 10–12 complete for the agreed emulator phase: Pixel 7 API35 runs all
eight workflows as well as the tablet. Phone controls open through compact menus
and inspector sheets. Widget layouts cover narrow and short windows, large text,
color/reference dialogs and Zen. Hardware keyboard, primary/middle mouse input,
synthetic stylus eraser/barrel/pressure and pen-versus-touch ownership are covered.
Pressure is optional/off by default; physical stylus validation is still milestone14.

Import cancellation stages a bounded candidate and commits only after an accepted
revision; cancellation preserves exact artwork/history/dirty baseline. Stroke
points remain batched and only canvas/Preview repaint during live drawing. Logical
raster/buffer/history budgets are tested; they are not guarantees of total process
RSS. Phone 256-point/256px stroke probe:15 ms,50 decoded canvas updates/800ms; tablet
probe:10 ms,50 updates. These are not display-presentation FPS measurements.

CI universal debug APK workflow added with pinned tools, read-only permissions,
no production keys, bothABIs,16KiB ELF/ZIP checks and signature verification. Local
universal build/verification passed. Both emulators updated with install -r and
retained private projects. CI's fresh debug keys can prevent cross-run updates;
this limitation and safe export-before-uninstall guidance are documented. Remote
CI is configured, not claimed executed. Physical devices/store/AAB stay deferred.

## Final emulator-phase gate

Milestones8 and13 now pass. Actual folder selection/grant persists through process
restart, Forget removes the stored grant, and repeated sheet exports bind JSON to
the provider's real PNG filename. A changed artwork revision requires exporting
PNG again before JSON. PNG exports and share previews verified visually; file
provider errors/revocation and cancellation are covered by policy/controller tests.
No live cloud-provider account validation is claimed.

Final:843 Rust tests passed/16 existing ignored; strict workspace Clippy, format
and boundaries passed;36 Flutter tests and clean analysis;8 tablet+8 phone
integration workflows passed; desktop standalone checks passed. Universal debug
APK built and verified for both64-bit ABIs/ELF+ZIP16KiB/signature, installed on
both emulators. Reviews have no remaining known material finding after fixes.

Only milestone14 (real tablet/phone/stylus validation) remains deferred by the
user. This is93% of the agreed milestone tracker, not full desktop feature parity
or a claim that issue29's hardware definition of done is met. Keep issue29 open.
Remote CI is configured but has not been triggered by this local-only work.

The completion audit temporarily reopened milestone13 for provider cancellation; the fix and verification are recorded below.

Completion audit fix: Cancel now completes a pending provider-read request
immediately and suppresses late callbacks; bounded provider read/write/cancel
workers are separate from private Save/recovery. A provider ignoring cancellation
may retain its single read worker, but cannot block cancellation feedback/private
storage. New opens report busy until that worker exits. Recovery is flushed
before reference import/export. Post-flush availability/document-generation guards
prevent duplicate operations and disposed editors opening pickers. The pending-
provider and recovery-write race regressions failed before fixes and now pass.
Final full Flutter suite:36 passed, analysis clean. Tablet8/8 rerun passed with
provider cancellation changes; phone8/8 passed for the same core workflows.
