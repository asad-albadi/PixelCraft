> Historical checkpoint before Lua retirement in issue #23. Current native automation policy: [migration guide](/docs/native-automation-migration.md). Earlier preserve/defer-Lua statements below are superseded.

> Historical audit at `8a8ccd0`. Lua implementation is deferred and preserved on `discarded-lua-updates`. Current native implementation and verification are recorded in the [acceptance report](native-workflow-acceptance-2026-11-18.md).

# Source and Large-Project Audit: 2026-11-18

This is an evidence checkpoint for issue20, not a completion/parity report.

## Source freshness

- PixelCraft implementation checkpoint:8a8ccd0 on feature/issue-20-workflow-parity.
- OxyPixel audit checkout and remote HEAD:a292c5685499cdf14a7d4e3d4e664bbd5ccc1fe7 (unchanged).
- Aseprite audit checkout:9cf3c128f582044689656422c96d2d6904ce66f4; fetched current remote main:eccf4ec292f66c0b8a4f814954ce7db160a8eb43. `git diff` shows two intervening commits: command search/menu discovery and Lua dialog numeric formatting. The Image/Sprite constructor evidence files used in recent checkpoints did not change.

## Large-project Lua: confirmed incomplete

Read-only manual probe: tests/large_lua_workspace.rs, saved_projects_support_lua_without_losing_frames_or_artwork. Set PIXELCRAFT_LUA_PROJECTS to a JSON array of the two local project paths and run `cargo +1.95.0 test --offline --locked --test large_lua_workspace -- --ignored --nocapture`.

- Day fixture, Eversong Woods - Sunspire Sanctuary - Autumn Breeze - Separate Layers.pixel:480x270,23layers,24frames,71,539,200logical pixels and71,539,200physical allocation pixels reported by the current native model. Lua admission failed in410.2us with the one-million-stored-pixel invocation error.
- Night fixture, Autumn Breeze - Moonlit Sanctuary.pixel:480x270,34layers,24frames,105,753,600logical and105,753,600physical allocation pixels. Same Lua admission error in422.8us.
- The manual test deliberately remains red and ignored in routine CI until this workflow is implemented. Both source files are only loaded; no save/export or live editor mutation is requested. Total debug test time135.57s includes native loading of both files and is not Lua execution latency.

Root cause in current code: src/lua.rs ScriptRuntime::new/rebind rejects doc.logical_pixels()>MAX_SCRIPT_PIXELS (1,000,000). lua_api.lua expands raster representations into dense Lua pixel tables; copy/transaction snapshots and host/final serialization traverse this state. Native Document storage already supports these projects. Raising/removing the guard would expose large Lua allocations rather than fixing the data path; physical pixel counting alone would also reject both projects and would not prevent Lua expansion.

Established implementation principle: image_class.cpp ImageObj stores native image/cel/tileset identity and resolves native images for API operations. PixelCraft should retain its native COW buffers behind private runtime-scoped image proxies, with typed native final results and separately bounded incremental script allocations. The implementation tasks and security/identity/undo gates are in [archived native Lua implementation plan](https://github.com/asad-albadi/PixelCraft/blob/77a2c23/docs/superpowers/plans/2026-11-18-large-native-lua-workspaces.md).

Acceptance: both real fixtures pass read-only metadata/pixel access and transaction/Undo without dropped frames, metadata or changed composites; then in-memory point edits, undo/redo, geometry/host operations, modeless rebind/rollback and persistence must pass. Native/UI work must remain responsive. Full script and native allocation guards, existing owner identities, stale-reply rejection, file authorization and atomic failure behavior remain required.

## New source changes requiring workflow coverage

### Command discovery and search

Peer source cmd_run_command.cpp at the fetched head now includes parameterized menu commands absent from shortcut registrations, normalizes query/label/id case and whitespace, and retains parameter-distinct command variants. cmd_modify_selection.cpp trims labels lacking quantities. PixelCraft has native action routing, shortcut editing and CLI/MCP commands; the current source search did not locate an equivalent editor command-search interface. Do not count those separate interfaces as proof of searchable menu discovery.

Acceptance to investigate and implement: an editor command picker lists actionable menu entries even without bindings, retains distinct parameterized variants and installed extension commands, shows current shortcut labels, searches normalized labels/IDs without accidental duplicates, handles disabled actions, and executes through existing action/listener/history paths. Trim friendly labels. Test keyboard entry/navigation/cancel/activation and stale extension reloads. Exact matching/ranking need product-specific design; duplicating a peer ranking constant is not itself parity.

### Dialog numeric precision

Peer dialog_class.cpp at the fetched head changes Dialog.data number assignment from significant-digit formatting to fixed fractional precision. The current PixelCraft GUI DragValue sets only max_decimals, and the Lua Dialog.data field is an ordinary mutable table. Do not infer fixed-precision assignment/readback fidelity from basic dialog creation tests.

Acceptance: assigning a decimal widget through Dialog.data, reading it back and displaying it obey the declared fractional precision; changing decimals and numeric text behaves consistently; integer mode remains integer. Cover zero/trailing zeros, small/large magnitudes, negative values, invalid/nonfinite values and bounded formatting work. Preserve existing modeless callbacks and file/permission controls. This is a confirmed area for differential testing, not a fix completed in this checkpoint.

## Remaining scope

This checkpoint preceded the native closure work. Clipboard snapshots and cancellation, large-frame display workers, and original-project acceptance were subsequently implemented and verified in PR #21. Platform CI and distribution checks passed at `5f6ca78`; the local Linux baseline passed 771 tests. The acceptance report records current evidence and the follow-up review fixes. Physical tablet input and real Linux desktop playback remain unverified. Deferred Lua work and command discovery in #22 remain open.
