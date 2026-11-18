# Native issue20 closure implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline with test-driven development. Preserve this ledger across context compaction.

**Goal:** Finish and verify outstanding non-Lua work on feature/issue-20-native-workflows and prepare a reviewed PR with accurate issue20 closure evidence.

**Architecture:** Keep native workers operating on immutable COW snapshots and publish only current targets. Poll cancellation within expensive data traversal, retaining existing non-cancelling wrappers for file/clipboard consumers. Validate the exact native branch against remaining original requirements and saved-project workflows.

**Tech Stack:** Rust1.95, egui, existing serde/native COW storage, existing CLI/MCP and GitHub CI. No new Lua APIs or integration.

**Spec:** docs/branch-split-2026-11-18.md; issue20 active non-Lua requirements; docs/source-audit-2026-11-18.md. User explicitly instructed to finish pending work here and ignore Lua. Linux desktop testing is user-deferred.

## Global constraints

- Keep main's existing Lua capabilities unchanged; only the existing macOS runtime selection compatibility is retained.
- Preserve native/ASE/clipboard serialized data and authored indexed slots, metadata, tile flips, history and stale-target protections.
- Never mutate either original Autumn Breeze project during acceptance; use read-only loads and disposable output copies.
- No arbitrary limit increases or timing-only unit assertions to mask expensive work.
- Do not merge main or close the issue before actual requirements/evidence are reconciled. PR creation and branch pushes are authorized; retain original stack/archive branches.

## Review focus

- Selection edits after capture must detach without changing worker/history snapshots; unchanged comparisons must avoid scanning identical shared masks.
- Cancellation during validation, selected-ID collection, tile hashing/equality and document checks must discard private results with no publication.
- Source/destination palettes and outside artwork must survive all clipboard cases, including duplicate slots and metadata namespaces.
- Viewport display conversion must preserve source samples and avoid stale previews across active tabs/frames/profiles.
- Verification counts must distinguish native tests from archived Lua additions; unexecuted platform/hardware checks remain explicit.

## Tasks

- [x] 1. Reproduce selection-mask snapshot copies; introduce transparent COW GUI mask storage and migrate capture/history/recovery consumers without changing wire arrays. Verify snapshot independence, stale-target rejection and serde recovery.
- [x] 2. Reproduce non-cancellable clipboard traversal; implement bounded polling in validation/ID/hash/equality/merge paths and native document validation where used by workers. Test atomic cancellation at each expensive phase and run full Windows gates.
- [ ] 3. Audit viewport/display conversion against current native source, add regression cases for confirmed defects, fix and test. Record verified behaviors instead of assuming the old tracker proves them.
- [ ] 4. Run native acceptance on both day/night projects: load, playback/representative render, save/reopen disposable copies and export; record timing, content/metadata equality and source-file hashes.
- [ ] 5. Reconcile all active original issue20 items to native code/tests/GUI/CLI/MCP evidence, explicitly defer44-47 and Lua part31, record platform/tablet coverage limits, and repeat current source audit for confirmed extra native gaps. Command discovery can be tracked separately from the original55 closure scope.
- [ ] 6. Fresh correctness/security review, required fixes, final Windows gates, native PR/CI including macOS packaging, accurate issue checklist and pushed evidence. Leave merge approval as the final reviewable step.

## Execution ledger

- Start: ea78ccc; clean native worktree;743 Windows tests plus standalone/startup checks passed at split checkpoint. Shared clipboard payload optimization already measured; GUI selection masks still use owned Vec values in target/history/worker captures. Clipboard validation and tile merge hashing/equality still contain unpolled scans. No live process at start.

### Task1 checkpoint

- RED real editor regression showed clipboard/history selection captures allocating different full-mask arrays. Introduced transparent COW SelectionMask for GUI mask state, immutable worker targets, history, tabs, recovery and rotation previews. Equality short-circuits shared backing; edits detach via normal mutable access. Core mutable stencil operations still deliberately produce owned vectors.
- The same regression now passes, including stale clipboard target detection and undo restoration. A wire regression verifies legacy boolean-array serialization and independent edits after deserialize/clone. Existing recovery/marquee/canvas tests remain unchanged in behavior; expected mask assertions now compare slices. No Lua source/API changes.
- Verification: formatting/check, strict offline locked all-target Clippy,745 Windows all-target tests and diff whitespace check passed. Final independent review remains the closure-plan gate.

### Task2 checkpoint

- Reproduced uninterruptible clipboard validation, tile hashing/comparison, large-tile selection traversal and full-document validation. Bounded cancellation now covers these traversals plus ID collection, metadata merge, palette preparation and native storage accounting. All cancelled jobs discard private results.
- Removed completion-time whole-document equality from tile copy/paste: placement COW backing changes plus palette/tileset changes establish whether publication is necessary, including linked peers with different grids. True no-ops do not publish.
- Reproduced and fixed integer overflow in polygon containment at extreme integer coordinates; polygon traversal is cancellable. Normal selection semantics are preserved.
- Twelve regressions observed RED then GREEN. Formatting, strict offline locked all-target Clippy,757 Windows all-target tests and whitespace checks passed (EngineChecks/issue20-native-cancellation-final-tests.log). No Lua edits.
