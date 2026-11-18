# Large Tile Clipboard Worker Plan

**Goal:** Keep large native tile copy/cut responsive without changing pixel data, operation order or undo semantics.

**Evidence:** Windows debug whole-set palette preparation (4.2M pixels) 914.8ms; optimized 80.2ms versus selected-one-tile 0.193ms. Expensive pixel loops execute on the UI call path.

**Architecture:** Keep small operations synchronous. Run large immutable COW source snapshots and optional cut results in one cancellable worker. Publish on the UI thread; reject stale cut targets, supersede older copy requests, and queue paste until copy is ready. Clipboard changes cancel pending publication. Reuse normal history/clipboard publication after completion. Large paste/import worker work remains a separate next checkpoint.

- [x] Red regression that large copy schedules instead of publishing synchronously.
- [x] Extract cancellable source snapshot builder; add bounded worker lifecycle/polling and copy/cut completion checks.
- [x] Verify exact output, supersession, source-edit/tab cancellation for cut, queued paste and one-step undo.
- [x] Independent review, Windows tests and optimized/source timing evidence; docs and push.

## Verification and limits

- Red tests confirmed large copies published synchronously and placement-heavy maps with tiny tilesets skipped workers. Scheduling now considers definition pixels plus placement count, and selection scans run off-thread for large cases.
- Immutable source snapshots include mask and polygon geometry. Cell collection and tile loops check cancellation; cancelled cuts discard private partial results.
- Reviewer-found old native paste race was reproduced and fixed by clearing its pending target before scheduling the newer copy.
- Reviewer-found polygon-only stale cut was reproduced and fixed by adding polygon geometry to ClipboardTarget. This also protects queued and system-clipboard paste destinations.
- Full Windows suite: 863 tests passed after fixes. A live-save test had one transient Windows AccessDenied failure in an earlier run; isolated rerun and the fresh complete suite passed without save-code changes.
- Two test-only additions then passed individually: queued Cut/Paste preserves two undo steps; tab switching cancels a pending cut without changing either document. Formatting and strict all-target Clippy passed after these additions. Independent review found no remaining actionable issue.
- Optimized 4.2M-pixel fixture: selected preparation 175.9us; whole preparation 79.9443ms; direct source snapshot 64.5782ms; UI queue 47.7us; worker completion including queue 64.0924ms. Excludes project loading, rendering and native clipboard serialization. This demonstrates source work left the UI call; it is not a claim that large import/paste is already asynchronous.
- Next: move expensive native tile paste/import preparation into a worker while preserving target freshness, document identity, queued operations and undo.
