# Android functional port implementation plan

**Goal:** Expose the existing Rust editor workflows safely on Android, tablet first.
**Architecture:** Existing Flutter host → serialized worker → bounded C ABI → shared core/automation. Kotlin owns Android storage and sharing.
**Spec:** GitHub issue #29 and ../../android-progress.md (accepted scope and tracker).

## Constraints and review focus

Desktop and CLI/MCP behavior remain compatible; no duplicate editing engine.
Keep the tablet visible. Do not claim physical devices tested. Keep issue #29 open
until its acceptance criteria actually pass. No production signing keys or release.
Test cancellation, stale work, failed writes, recovery freshness, locked targets,
oversized files and native allocations before presenting a feature as complete.

## Batches

- [ ] Recovery: failing native snapshot/restore tests; versioned bounded snapshot
  that preserves dirty baseline, frame/layer; native private atomic recovery file;
  debounced autosave after committed edits and background flush; restore prompt,
  preserve recovery on failed decode/cancel, discard only explicitly. Controller
  and actual emulator restart tests; review and commit.
- [ ] Tools/colors: failing transactional command tests; bounded selection/tool
  operations using core workflows; expand tablet tool rail and color editor with
  reachable dialogs. Test shapes/fill/sample, locks/Undo and visual interaction.
- [ ] Selections/layers/cels: bounded editing commands, copy/paste and transforms;
  layer operations and linked/sparse cel behavior; tests and visual review.
- [ ] Animation: shared timeline mutations/tags, playback honoring durations and
  speed, rendering without changing edit history; tests and tablet check.
- [ ] Formats: bounded byte-stream import/export, Android picker/share, format
  matrix and error handling; cross-host metadata/animation tests and review.
- [ ] Advanced documents/input/layout: indexed/tile/reference workflows; phone
  sheets, keyboard/mouse/stylus, orientation/resizing/accessibility; tests.
- [ ] Performance/delivery: dirty render updates, cancellation/stale generations,
  mobile budgets, reproducible Android CI APKs, upgrade tests and docs.
- [ ] Final regression/review: workspace/Clippy/headless/live/standalone, Android
  integration, phone/tablet visual matrix and code/security review loops. Record
  physical-hardware acceptance separately; it requires connected devices.
