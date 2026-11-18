# Native workflow branch separation

The user approved preserving the full PR stack and issue #20 work on `discarded-lua-updates`, and moving all non-Lua changes to a separate branch based on `main`. Existing Lua support on `main` remains supported; further Lua development is stopped.

## Recoverable archive

- Archive branch: `discarded-lua-updates`, pushed at `77a2c23`.
- Includes PR #17 (`90c3480`), PR #18 (`875528d`), PR #19 (`056567d`), issue #20 through `3d5ab78`, the clipboard sharing work and the instruction to stop Lua development.
- All three stack branch tips are ancestors of the archive. They were already integrated into the issue #20 tip; no artificial merge was needed.
- Original branches and PRs were not deleted, rewritten, closed or force-pushed.
- A separate local backup ref, `refs/backup/pre-lua-split-20261118`, retains the pre-split uncommitted snapshot. An exact binary patch and commit/file inventory are also preserved outside the worktree under EngineChecks.

## Native branch

`feature/issue-20-native-workflows` starts directly from `main` at `5c4daae`. Its consolidated change keeps native editing, drawing, selection and transformation tools; layer/timeline/tilemap operations; recovery and background work; formats, color management and export workflows; engine integrations; direct CLI/MCP operations and filesystem protections; and native platform packaging.

Lua-specific workspace/console/listener APIs, dialogs/canvas extensions, new scripting tools, typed script property APIs and unfinished native Lua buffer/manifest transport are excluded. Their dedicated code, tests and plans remain in the archive. Mixed test files retain native tests and the original tests from `main`; only tests for the excluded newer Lua capabilities were moved out of this branch.

`src/lua_api.lua`, `src/lua_compat.lua` and `src/script_ui.rs` match `main`. `src/lua.rs` retains main's implementation with only the existing macOS package loader selector restored: macOS embeds the packaged Mach-O dylib instead of the Linux ELF library. No new Lua API or runtime integration was added.

Mixed commits required preserving independent native fixes rather than dropping entire commits. These include authored indexed clipboard slots, tile/frame palettes, ASE user-data namespace remapping, native identity preservation in editor/MCP operations, timed native spray sampling, native eyedropper controls, and COW placement maps. The generic ASE metadata remapper is in `src/ase_properties.rs`, independent of the scripting host. Background transform selection snapshots use `NativeSelectionState`.

## Verification

- Windows formatting/check and strict offline locked all-target Clippy passed.
- All 743 applicable Windows tests passed. The earlier 1,012 count included the newer Lua-specific coverage now preserved only in the archive; it is not this branch's test count.
- Existing main scripting regression tests passed with the native model changes.
- Standalone Windows verification passed drawing, native save/open, PNG/sheet/GIF exports, existing embedded plugin execution, theme/palette/shortcut application, disable and uninstall.
- Windows screenshot startup succeeded and was visually inspected.
- Fresh-context review found and fixed two platform leftovers: removed text-clipboard enum variants in a Linux-only match, and the macOS runtime selector. Both fixes were rechecked by the reviewer. Linux and macOS execution were not performed here; Linux execution remains user-deferred.
- The new clipboard snapshot regressions first failed on deep copies, then passed with shared immutable payloads and detach-on-edit behavior. Wire data remains compatible.
- Optimized Windows 4,194,304-pixel reference-paste fixture: UI queue 1.8406 ms before sharing, 47.3 us after; completion 6.1662 ms before, 3.889 ms after. This is one fixture, excluding project loading, native clipboard serialization and rendering. Full tileset preparation remains worker work (~79ms in this fixture); this is not a universal responsiveness claim.

## Remaining work

This split is not a declaration of complete issue #20 or editor parity. Continue non-Lua workflow/source audits, clipboard cancellation and selection snapshot costs, command discovery and viewport work. Physical tablet and non-Windows runtime validation remain unverified. New Lua development stays deferred unless explicitly resumed by the user. The earlier issue #20 tracker contains historical implementation checkpoints, including work now archived; use this scope document when assessing the native branch.
