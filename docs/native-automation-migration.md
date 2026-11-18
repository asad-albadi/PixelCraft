# Lua retirement and native workflow migration

PixelCraft no longer executes Lua scripts or Lua-powered plugins. This includes the old Run .lua action, interpreter/compatibility APIs, plugin command discovery, script dialogs/callbacks and script-owned preferences. Previous claims about CLI Lua and a Lua console are removed. No new in-process interpreter replaces them.

Existing script files and preferences remain on disk for manual recovery. Script-only packages cannot be installed or enabled; legacy packages are listed as unsupported. Mixed packages keep their independent themes, palettes, shortcuts, languages and dithering matrices and show a warning for ignored script contributions. Scripts are never evaluated during install, discovery, startup or enable. User artwork, native project state and supported interchange metadata do not require Lua.

## Native replacements

- Document access: CLI/MCP create, inspect, import, save and save-as; live-inspect/live-create/live-save for an opted-in running editor.
- Drawing: edit for explicit RGBA pixel edits; commands with shape, stroke, polygon and blit. Seeded stroke/scatter and masks are native operations. Clients may generate parameters in their own language.
- Layers/frames/tags: commands add-layer/add-group/set-layer, add-frame/set-frame and tag operations. Stable document/layer/cel IDs and revision checks remain available through MCP.
- Moving, flipping and recoloring: commands move-region/copy-region with flip_x/flip_y, recolor-region with explicit color pairs, and clear-region with a mask. Region clearing also supports non-background indexed/grayscale layers and preserves the authored transparent slot; other bulk raster operations retain their RGBA restriction. Native selection/transform GUI commands remain available independently.
- Palettes/color: palette-import/export/convert and convert-rgb/convert-grayscale/convert-indexed. Source palettes, indexed slots and transparent-index semantics remain native.
- Export workflows: export, batch-tags, atlas and ordered batch with dry_run and explicit overwrite policy. Templates, engine sidecars and repeat export remain native features.
- Transactions/undo: file commands validate a complete private result before publication. Live command batches are one native undo edit; use live-undo/live-redo with the current revision. There is no client script callback executed inside the editor.
- Dialog wizards/modeless script UI and script object compatibility are intentionally discontinued. Clients gather parameters externally and call the native interface. This is not a one-to-one replacement for a Lua runtime API.

The public CLI help and MCP tool schemas define actual operations and limits. The interpreter is not replaced by an arbitrary shell-command endpoint. Live authentication/opt-in, project-root containment, revision/overwrite protection, work limits, progress and cancellation remain in force.

## Runnable example

Run `python examples/native-workflow.py PATH_TO_PIXELCRAFT NEW_OUTPUT_DIRECTORY`. It uses Python's standard library as an external client: creates a layered two-frame project, adds deterministic pixel edits through a typed native batch and exports an animated GIF. The directory must be new to avoid overwriting artwork. See the script for the exact JSON request shapes; the same operation/argument objects can be sent as MCP tools/call requests.

For a running editor, enable live MCP in Workflow tools, call live-inspect, and provide the returned revision to live-preview-commands/live-commands. Refresh revision and target IDs after user edits, undo/redo or structural changes. File-tool examples do not imply access to an editor that has not opted in.

## Coverage migration

Lua-specific constructor, compatibility, dialog and sandbox tests retire with the interpreter. Native drawing, color conversion, selection, palette, tile/format metadata, transaction, undo/redo and filesystem tests remain. Offset-cel clear, indexed transparency, flip/recolor and selected-cel editing have direct native regression coverage; selected-cel edits are also tested through real CLI and MCP processes. Existing live command/preview/undo tests cover the former script-result publication responsibilities.

The archive branch and historical plans remain as history, not supported Lua features. The current automation policy supersedes their earlier preserve/defer-Lua statements.
