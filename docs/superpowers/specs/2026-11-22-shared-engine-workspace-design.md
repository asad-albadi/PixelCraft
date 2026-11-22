# Shared engine workspace: issue #27

## Approved scope

The written requirements are issue #27 and the user's instruction to implement
them in reviewable, tested batches. This changes repository ownership, not product
features. There is no Android, Flutter, FFI, mobile dependency or new release.

## Boundaries

Keep the root `pixelcraft` package as a public library compatibility facade and
the sole `pixelcraft` executable. Introduce `pixelcraft-core`,
`pixelcraft-automation`, `pixelcraft-cli`, `pixelcraft-mcp` and
`pixelcraft-desktop`. The root facade delegates existing entry points; consumers
must not need to rename imports or invocations.

Core owns documents, formats, rendering, reusable editing, history and selections.
Automation owns shared validated transactions and session behavior. CLI parses
arguments; MCP owns JSON-RPC and the live bridge. Desktop owns egui presentation,
clipboard, platform menus and window integration.

The dependency order is adapters → shared operations → core. The desktop may use
CLI/MCP adapters to preserve the single-executable dispatch. No reverse edges.

Codecs already require guarded I/O, anchored publication and the codec-runtime
broker. Keep exactly one implementation and one instance of their thread-local
guard/staging state below the codecs, in `core::storage`. MCP calls that storage
boundary; the core does not depend on a server. Preserve fail-closed access and
all file identity, overwrite, cancellation and cleanup checks. Moving the guard
does not broaden the broker's permitted work.

Shared asset files, fixtures, examples, packaging, vendored patches and integration
tests stay at repository root. Build scripts resolve those paths explicitly.
Dependencies retain their existing versions. Shared crates exclude desktop UI,
clipboard and window-system dependencies. Keep host-specific update integration
outside the core where dependency direction permits.

## Preservation evidence

Record the baseline suite and representative headless/GUI behavior before moves.
After each batch run relevant tests, formatting and strict lint. Final validation
uses the entire workspace, existing CLI/MCP checks, standalone save/load/export,
live bridge tests and native graphical captures. CI must cover Windows and both
macOS architectures as well as Linux. Record unrun platform checks honestly.

Retain supported file round trips, export metadata/IDs, live revision semantics,
Undo/Redo, preferences, session recovery, UI layout, assets and all seven package
formats. Preserve opt-in tests and document how to run their new targets.

## Migration

Use small commits: baseline/contract; core/storage extraction; command and
transport adapters; desktop/shared editing extraction; packaging/docs/CI;
whole-branch review fixes and verification. Do not replace proven editing or
security algorithms while relocating them. New boundary helpers need regressions.
