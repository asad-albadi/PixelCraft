# Native command discovery and Lua retirement implementation plan

> Use superpowers:executing-plans inline with test-driven development; independent correctness/security review at the final gate.

**Goal:** Complete issues22/23 and publish a tested native-only automation/command-search PR.
**Architecture:** Shared native menu/action catalog; asset-only extension manager; typed CLI/MCP operations with current native workers/security boundaries.
**Tech Stack:** Rust1.95, egui0.36, existing serde/native model/CLI/MCP, GitHub CI.
**Spec:** docs/superpowers/specs/2026-11-18-native-commands-no-lua-design.md and issues22/23.

## Global constraints

- No Lua execution/runtime or replacement in-process interpreter. Keep declarative extension assets and WebP.
- Never mutate the original day/night artwork; acceptance writes disposable outputs only.
- Preserve native data/format behavior, security boundaries, undo and background responsiveness.
- Existing user approval covers implementation and a PR, not merging main or deleting archive branches.

## Review focus

- Installed mixed/legacy packages cannot execute content or break native asset discovery.
- Retiring Lua tests must not silently drop useful native editing/format/security coverage.
- Modal input cannot leak or execute a disabled/stale command in another tab.
- Catalog variants preserve parameters and never dispatch unvalidated executable/plugin paths.
- Native packaging and clipboard behavior remain correct without embedded Lua or obsolete shared build steps.

## Tasks and interfaces

- [x] 1. Asset-only extension migration. Modify src/extensions.rs; add tests/lua_retirement.rs. Installed::script_warning reports unsupported script contributions, Manager install/list/set_enabled handle legacy/mixed packages without execution. Observe rejection/marker tests RED then GREEN; retain archive-safety and declarative-asset tests. Commit.
- [x] 2. Remove Lua host/runtime, preserve native workflows. Remove Lua modules/script UI/jobs/payloads/tests solely about scripting. Port useful mixed tests to native CLI/MCP/model paths; add tests/native_automation_migration.rs for target edits, palette/tag/batch, obsolete-request atomic rejection. Update shared build/CI/standalone/package checks and migration/cookbook docs. Full Windows gates and zero active runtime references; commit.
- [ ] 3. Native command catalog/search. Create src/command_search.rs; extract shared menu groups/availability; typed dynamic variants and execution. Add default binding and modal state/UI/input gating. Observe catalog and rendered modal/undo/disabled/stale tests RED then GREEN. Full Windows gates; commit.
- [ ] 4. Real-project and installed/portable Windows acceptance. Run both day/night native save/reopen/frame/export/ASE and GUI/cache probes; protect originals by hashes. Check startup, clipboard, no-Lua package payloads and asset workflows. Record results.
- [ ] 5. Independent code/security reviews, fixes with RED/GREEN regressions, full final Windows checks, push PR and native/package CI. Update issues/checklists with actual results and coverage limits; no merge.

## Execution ledger

- Start902176f; clean linked worktree already isolated. Created requested branch from freshly pulled main. Baseline Windows all-target suite running in EngineChecks/issue22-23-baseline.log.
- Ruling: issues22/23 and explicit implementation request define approved design scope; use shared native catalog after removing Lua rather than preserving script commands. Cost if wrong: additional UI/transport requirements would need a follow-up; no interpreter is recreated.
- Pre-flight: task1 asset warnings are consumed by task2 GUI and packaging; task2 removes script registrations before task3 catalog, so issue22's old extension-command wording is superseded by issue23. Existing native action routing and shortcut persistence are shared; tests pin these interfaces.

### Task1 complete

- Baseline771 Windows tests passed. Three focused retirement regressions observed RED then GREEN: raw Lua installation rejected; legacy script-only packages inactive/non-enableable with files preserved; mixed packages retain assets without resolving script paths. Asset contribution regressions also pass (5 focused tests total).
- Manager command discovery is temporarily an inert compatibility wrapper for this intermediate commit; task2 removes its callers/type and wrapper with the host. Runtime APIs remain until task2. Existing script execution tests intentionally depend on the retiring host and are classified/ported in that task rather than misreported as native regressions.

### Task2 complete

- Removed interpreter/FFI/API/compatibility modules, script GUI/jobs/host/session transport and command discovery, runtime binaries/source archive/licenses, script-only fixtures and runtime build/CI steps. WebP and its shared libloading consumer remain. Asset GUI reports retired scripts; existing files/preferences are left intact.
- Retired76 script-only integration tests,6 mixed-file Lua harness tests,3 script-session GUI tests and1 interpreter unit test (86 retired); added9 native asset/migration regressions across tasks1–2. Baseline771 -> current694. Native model/format/GUI/CLI/MCP security coverage is retained; live command/preview/undo tests replace script-result responsibilities.
- Porting clear tests reproduced an actual indexed-mode CLI/MCP gap. Native clear-region now accepts non-background indexed/grayscale layers and writes the authored transparent index. Offset geometry and slot1 transparency regressions pass. Other bulk raster operations retain their existing RGBA restriction. Flip/recolor and public CLI/MCP selected-cel workflows pass atomically without scripts.
- CLI help retirement observed RED then GREEN. Obsolete script requests fail without writes. Asset install/list/enable marker tests never execute script content.
- Windows fmt, strict all-target Clippy and694 all-target tests passed; no-Lua executable payload scan, native standalone smoke and runnable external Python CLI cookbook passed. Local generated Lua DLL was moved to an EngineChecks backup before building, so no build/runtime payload was available or required.
- Active docs, migration guide, external CLI example and platform packaging checks updated. Historical scope/audit checkpoints explicitly marked superseded.
