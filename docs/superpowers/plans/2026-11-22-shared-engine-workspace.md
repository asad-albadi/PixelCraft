# Shared Engine Workspace Implementation Plan

**Goal:** Resolve issue #27 without changing desktop or automation behavior.
**Architecture:** Five internal crates beneath the root compatibility package.
Guarded storage stays below codecs; desktop UI and MCP transport stay above core.
**Tech stack:** Rust 1.95.0, Cargo workspace, existing egui and platform libraries.
**Spec:** ../specs/2026-11-22-shared-engine-workspace-design.md

## Global constraints

- Existing `pixelcraft` library imports and executable modes remain available.
- No Android, Flutter or FFI code, dependency upgrades or product redesign.
- One shared guard implementation/state; preserve file publication protections.
- Root assets, fixtures, examples, vendor patches and packaging remain usable.

## Review focus

Preserve restricted codec access across crates; validate with existing MCP file
escape/identity tests. Preserve atomic output staging; run publication regressions.
Preserve history and selection state; run GUI history regressions after extraction.
Preserve include paths and runtime libraries; build standalone and packages.
Preserve executable/test discovery; run headless checks through the root binary.

## Batch checklist

- [x] Baseline: record HEAD, suite totals and CLI help. Add a failing workspace
  boundary check using Cargo metadata for required members and dependency edges.
- [x] Core: move library engine modules to `crates/pixelcraft-core/src`; extract
  storage from `src/automation/{mcp_io,mcp_access,mcp_linux,mcp_windows,mcp_files,
  editor_output}.rs` and generic staging from `src/automation.rs`. Retain root
  reexports. Move static codec linking with codecs; keep icon resource on binary.
  Run workspace boundary check, core tests and root compatibility tests.
- [x] Adapters: move shared command/export interpretation to automation;
  `run_cli` and parsing to CLI; JSON-RPC/live bridge to MCP. Preserve public root
  automation reexports and existing wire schemas. Run full file/live/headless
  automation regression suite. Review security boundary changes.
- [ ] Desktop and editing: move GUI modules into desktop; root launcher calls its
  entry point. Extract shared selection, stroke and history state/operations with
  regressions; desktop must use them. Run all desktop UI tests and history checks.
- [ ] Integration: update CI test targets, build scripts and embedded asset paths;
  keep root default commands and `target/{debug,release}/pixelcraft`. Update
  developer docs and test workspace/package metadata and build packaging paths.
- [ ] Final: formatting, strict workspace all-target Clippy, complete tests,
  live tests, headless checks, standalone and graphical verification. Independent
  code/security reviews, fix findings, rerun affected checks and record evidence.

## Execution record

Ruling: use the user's approved issue and explicit implementation request as the
scope authority; carry out batches here and obtain independent review without
asking the user to approve the same restructuring again.

Baseline: `46576e1c15009e79bbc1021fb3321f7a100ce0f6`; Rust 1.95.0;
`cargo test --locked`: 806 passed, 0 failed, 16 opt-in tests ignored.
Full log: `/tmp/pixelcraft-27-baseline-tests.log` (local evidence, not portable).

Batch core: full workspace suite retained 806 passing tests and 16 opt-in ignores;
strict all-target workspace Clippy passed. Existing CLI help is byte-identical.
Independent review confirmed one guard state and unchanged staging semantics.
Moved a cross-layer tile command regression to root integration tests; kept
codec broker private. Default workspace members include moved unit/security tests.

Batch adapters: 806 tests passed, 16 opt-in ignores; strict workspace Clippy
passed. Released v0.11.0 and refactored executable return identical initialize
responses and full MCP tool schemas; CLI help remains byte-identical. Independent
review compared command bodies, live bridge, root guards and response limits: no
remaining actionable findings. Deadline tests moved to the MCP crate.
