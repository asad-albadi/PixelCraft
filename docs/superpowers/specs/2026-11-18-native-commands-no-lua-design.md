# Native command discovery and Lua retirement

Approved scope: user requested implementation of issues22 and23, Windows testing, code/security review, fixes and a pushed PR from current main. Base902176f. New branch feature/native-command-search-no-lua in the existing isolated worktree.

## Design

Retire executable Lua support entirely, preserving declarative asset extensions (themes, palettes, shortcuts, languages, dithering matrices). Parse legacy script contributions only to report unsupported content; never resolve, read or execute their script paths. Reject new script-only installs, allow independent assets in mixed packages with a clear warning, and leave installed files/preferences intact unless the user explicitly uninstalls them. Remove interpreter payloads, host APIs, script UI/jobs and build/CI requirements. Keep WebP and its shared dynamic loader dependency.

Audit script tests by behavior. Retire tests solely about Lua objects/compatibility/runtime restrictions; preserve native model/format/automation coverage and port worthwhile editing cases through actual CLI/MCP operations. Existing inspect/create/commands/edit/paint/batch/import/export already provide most automation. Add native operations only for demonstrated missing useful workflows, with the existing validation, revision, resource and filesystem protections.

Command search uses a shared native menu catalog, not the shortcut map as its source. Reuse existing action IDs, keep parameter-distinct theme/density/recent-file variants with typed targets, deduplicate identical commands, and rebuild labels/shortcuts/availability from current state. Default Ctrl+Shift+P opens a modal with normalized label/ID search, keyboard selection and Enter/Escape. Disabled or stale entries never execute. Existing history/validation/action routing owns edits; no plugin script registrations remain under issue23. Query/navigation keys and clicks cannot leak to the canvas or other shortcuts. Native menus and search share the same enabled predicate.

No alternative script engine, arbitrary command execution endpoint, automatic deletion of user scripts or project format migration. Archive branches/history remain. User authorized this combined scope and PR; routine implementation choices use the existing Rust/egui architecture without another approval pause.

## Acceptance

Issue22: unbound and parameterized commands, normalized search, deduplication, current bindings, active-tab targeting, disabled/stale action rejection, modal input/focus/cancellation, one-step undo and rendered pointer/keyboard GUI tests.

Issue23: no Lua loader or supported execution path; obsolete CLI/MCP requests fail without writes; asset-only/mixed/legacy packages remain safe; marker scripts never execute during install/list/enable/discovery; native workflow regression coverage; packages build/run without Lua; updated docs/migration/cookbook. Preserve resource/auth/revision/root/overwrite boundaries.

Run Windows formatting, strict all-target Clippy, full native suite, standalone/clipboard/UI checks and both original Autumn Breeze projects with disposable outputs and source hashes. Final independent correctness/security review and fixes; native platform/package CI on the PR. Physical tablet/real desktop Linux playback coverage remains explicitly limited.
