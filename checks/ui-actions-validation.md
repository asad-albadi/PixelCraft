# UI actions and timeline validation

Validated on the Linux build host on 2026-11-12.

- Local `pc_button` / `local_button` actions never look up editor shortcuts. Editor controls explicitly opt in with `pc_command_button` or `button_as` and a command ID. Script-provided button names, including “Open…” and “Save”, remain local.
- Shared button rendering owns its one tooltip and accessible name. Rebinding/removal is checked against the rendered toolbar, including the absence of duplicate tooltips.
- Timeline cell accessible names identify the layer, one-based frame number and empty/occupied state. AccessKit is enabled in the desktop integration so this metadata reaches platform accessibility APIs.
- Both themes at native scale factors 1.0, 1.5 and 2.0 pass cell clipping, accessibility-tree and Tab/Enter selection checks. The sample's three layers are completely visible. Software-rendered theme/scale images are in `ui-actions-snapshots/`; `rebuild_check` also captured the running native Linux app.
- `cargo fmt --all -- --check` and `cargo build --offline --locked` pass. The full `cargo test --offline --locked` run passes 213 tests, with two pre-existing loopback-network tests ignored. Logs: `ui-actions-full-tests.log`, `ui-actions-build.log`.
- Tests use `PIXELCRAFT_HOME=/tmp/pixelcraft-ui-actions-tests` and `XDG_CONFIG_HOME=/tmp/pixelcraft-ui-actions-config` because the host's normal preferences directory is read-only in the build sandbox.
- `rebuild_check` passes build, app screenshot and mechanics checks; result: `ui-actions-rebuild-check.json`.

## Windows status

Native Windows execution is pending: this host is Linux, with no Windows runtime or runner attached. No native Windows results are claimed. The Windows release workflow now explicitly runs `cargo test --locked --bin pixelcraft ui_icon_tests -- --nocapture` and uploads theme/scale PNGs as `PixelCraft-Windows-UI-review`.

On a Windows machine, run the release workflow (or its Windows prerequisites followed by that test command). For final native window-system verification, launch the editor at Windows display scaling 100%, 150% and 200%, in both light and dark themes. At each setting:

1. Hover Choose sprites, script Browse and script-defined Open/Save buttons; check that no unrelated editor shortcut appears. Rebind a tool shortcut and remove it; verify its tooltip updates once.
2. Tab and Shift+Tab through controls and timeline cells, and use Enter/Space to activate them. Confirm focus stays visible and cells select the expected frame and layer.
3. Use Narrator to inspect an empty and occupied timeline cell; verify layer, frame and state are announced.
4. Confirm all three default layers remain fully visible, then resize the window and check scrolling and control wrapping.

These native checks cover Windows focus, Narrator and per-monitor DPI behavior beyond the portable egui widget tests.
