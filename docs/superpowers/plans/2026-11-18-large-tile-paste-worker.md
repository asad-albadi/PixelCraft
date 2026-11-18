# Large Tile Paste/Import Worker Plan

**Goal:** Move expensive native tile paste/import preparation off the UI call while preserving destination identity, cancellation, ordering, validation and undo.

- [x] Red regression for deferred large tile paste and exact output/undo.
- [x] Extract pure paste builder; add cancellable preparation and bounded worker lifecycle. Keep small pastes synchronous.
- [x] Reject changed targets, supersede older operations, preserve clipboard and queued copy/cut paste semantics.
- [x] Test in-place/import/new-sprite paths, cancellation, stale site, repeated operations, error atomicity and mask/slot semantics.
- [x] Review, Windows regression pipeline and optimized scheduling benchmark; document limits and push.

## Verified checkpoint

- Large-paste immediate-commit regression reproduced before extraction. Private builder now handles validation, destination palette preparation, matching/import, placement and no-op detection; UI completion commits only against the captured destination identity.
- Scheduling includes source payload, expanded sprite canvas, destination logical pixels/definitions/placement lengths. A small-source/large-destination regression reproduced the missing offload before this accounting fix.
- Worker result rejection tests cover edits, polygon changes, tab switching and newer clipboard writes. Repeat pastes supersede older jobs; unchanged results add no history. Layer/sprite imports preserve expected mode and parent document.
- Grid mismatch and malformed clipboard tests verify rejection without document/history changes. Escape cancellation was reproduced and implemented for copy/cut/paste jobs.
- Native completion repaint gap was reproduced with an injected test-only response channel and drained egui repaint requests: starting a worker at the end of clipboard polling must request a further frame. Final busy check now includes both tile workers.
- Windows: final 874 all-target tests passed; formatting/check and strict offline locked all-target Clippy passed. Independent reviews found no actionable issue after fixes. Whitespace check passed.
- Final optimized 4.2M-pixel fixture: selected prep191us; whole prep79.9688ms; source snapshot65.3497ms; copy queue60.1us/completion64.2795ms; large paste queue57us/completion17.0855ms. Excludes project loading, viewport rendering and native clipboard serialization.
- Limits remain explicit: UI-side snapshot Vec cloning, reference-image rasterization, and cancellation latency during initial validation/selected-ID collection/merge hashing/equality need measurement. This checkpoint does not claim a strict cancellation deadline or global parity.
