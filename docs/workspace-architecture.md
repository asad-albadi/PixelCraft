# Workspace architecture

PixelCraft has one public executable and a compatibility library at the repository
root. The internal Cargo workspace separates reusable editing from desktop UI and
automation transports. This restructuring adds no mobile application or FFI.

| Package | Responsibility |
| --- | --- |
| `pixelcraft` | Existing public Rust imports and `pixelcraft` executable |
| `pixelcraft-core` | Documents, formats, rendering, editing workflows, editor sessions, history, selection and strokes |
| `pixelcraft-automation` | Validated commands, inspection, revisions, batches and export orchestration |
| `pixelcraft-cli` | Existing CLI argument parsing and shared-operation dispatch |
| `pixelcraft-mcp` | Existing MCP JSON-RPC, opt-in live bridge and transport checks |
| `pixelcraft-desktop` | Existing egui editor, native platform services, updates and window integration |

Adapters depend on shared commands and core. Core does not depend on the desktop,
CLI or MCP packages. Desktop uses the adapters to preserve all modes of the single
executable. Root library imports such as `pixelcraft::model`, `pixelcraft::formats`,
`pixelcraft::automation` and `pixelcraft::updates` retain compatibility reexports.

## Shared state and storage

`pixelcraft_core::editor::EditorSession` owns active artwork, saved artwork,
selection, current layer/frame and Undo/Redo history. The desktop retains
presentation, gestures, worker dispatch and platform services around that state.
Core history traversal notifies the host after each restored state so desktop
invalidation stays equivalent to the previous implementation.

`pixelcraft_core::storage` owns the one guarded I/O implementation, anchored file
publication, staging registry and platform access state. Codecs already need
these checks; making them depend on an MCP server would introduce a cycle.
MCP establishes access through the shared guard. Codecs and publication use the
same state; the unrestricted codec broker remains private to core. The names of
the existing internal `mcp_*` storage files are retained to keep the security move
reviewable; these files do not implement an MCP transport.

## Building and checking

Run commands from the repository root with Rust 1.95.0. Existing `cargo build`,
`cargo run`, `pixelcraft --help`, `pixelcraft --cli ...` and `pixelcraft --mcp ...`
entry points remain available. The executable is still written to
`target/debug/pixelcraft` or `target/release/pixelcraft` (with `.exe` on Windows).
Default Cargo checks include every workspace member.

```sh
cargo +1.95.0 build --locked
cargo +1.95.0 fmt --all --check
cargo +1.95.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.95.0 test --workspace --locked
python3 checks/workspace-boundaries.py
python3 checks/productivity-headless.py
```

Tests of native UI/clipboard implementations now target the desktop **library**:

```sh
cargo +1.95.0 test --locked -p pixelcraft-desktop --lib ui_icon_tests
cargo +1.95.0 test --locked -p pixelcraft-desktop --lib native_clipboard_roundtrip -- --ignored --test-threads=1
cargo +1.95.0 test --locked --test productivity -- --ignored --test-threads=1
```

Clipboard tests require the corresponding OS/session. The ignored productivity
tests exercise local live-bridge sockets and should use isolated user data.
Root integration tests still import the compatibility library; core session
regressions also run independently without a GUI dependency.

## Assets, packaging and versions

Assets, fixtures, examples, platform packaging and vendored patches stay at the
repository root. Embedded references in the desktop resolve to these original
files. Static WebP linking belongs to core's build script; the Windows executable
resource remains in the root build script. Package scripts still consume the
single root executable and produce the same seven desktop download formats.

All packages inherit their version from `[workspace.package]` in the root
`Cargo.toml`. For a future release, change that single version and let Cargo
refresh the workspace entries in `Cargo.lock`; do not update only the facade's
lockfile entry. About metadata and installer/package versions then stay aligned.

No external dependency versions or checksums changed during restructuring.
The dependency advisory snapshot's list includes the new internal package names;
the advisory records themselves were not refreshed or altered.
