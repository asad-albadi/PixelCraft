# Menu/settings validation — issue #25

## Scope and platform evidence

Implemented on `feature/discoverable-menus-searchable-settings` in reviewed commits. Local verification uses Linux. Windows and both native macOS architectures are checked by the pull-request workflow; their run results are recorded on the PR. Physical monitor changes and an external artist usability study were not performed.

The final Linux suite passes **779 tests**, with 16 existing opt-in tests ignored. The three ignored live-bridge network tests were run separately and pass. Formatting, strict all-target Clippy, CLI/MCP headless workflows and the standalone executable check pass.

Commands used:

```sh
cargo +1.95.0 fmt --check
cargo +1.95.0 clippy --locked --all-targets -- -D warnings
cargo +1.95.0 test --locked
cargo +1.95.0 test --locked --test productivity -- --ignored
python3 checks/productivity-headless.py
pixelcraft --standalone-check <temporary-directory>
```

Each batch received independent code/security review. Findings were reproduced before fixes and rechecked. No outstanding actionable code or security findings remain from those reviews; native AppKit execution is validated on macOS CI rather than claimed from Linux source inspection.

## Task walkthroughs

| Task | Current route | Evidence |
| --- | --- | --- |
| Interface size and recovery | Edit → Settings → Appearance; Reset interface scale | Exact control search/focus; ordinary and 300% app zoom recovery; keyboard focus; synthetic DPI transitions. |
| Recovery/autosave | Settings → Files & Recovery; search “autosave” | Actual checkbox click, saved configuration reload, category restoration and current-document preservation. |
| Onion defaults | Settings → Animation; search “next tint” | Individual count/tint/opacity focus destinations, accessibility names, keyboard reveal and saved defaults. |
| Current document profile | Sprite → Color Profile | Existing conversion/assignment preview, cancellation, native validation and Undo checks remain in the full suite. App display/import defaults have their own Color Management page. |
| Fill tolerance | Fill → Fill options in the active tool bar | Existing rendered fill/wand controls and flood/selection regressions pass; tool preferences survive app reset. |
| Sheet import | File → Import → Import sprite sheet | Focused task rendering and narrow-window controls; native grid-import/CLI/MCP fixtures verify cells, ordering and limits. |
| Sheet/image export | File → Export sprite sheet; File → Export → format | Actual PNG/BMP/sequence files at selected scale; typed paths; overwrite confirmation; Repeat retains accepted scale; deep sheet controls remain keyboard-reachable. |
| Asset packages | Tools → Asset extensions; linked from Settings | Rendered open/close/return and list refresh; existing package install/apply/disable/uninstall tests and standalone check. The OS file-picker interaction itself was not automated. |
| Assistant access | Tools → Assistant access; linked from Settings | Destination stays opt-in; GUI routes/resets leave bridge disabled; existing revision, expiry, revocation, authentication and Undo tests pass. |
| Workspace recovery | View → Layout → Restore default layout | Inspector/Preview visibility, Inspector/timeline dimensions, Preview settings and split-canvas reset; artwork/history preserved; secondary-pane curve remains drawable. |
| Context properties | Sprite/Layer/Frame properties; Tileset tools | Correct object scopes, actual Done clicks, group/ancestor locks, target replacement/removal guards, clipped tile capture and Undo. |
| Linked preferences dialogs | Settings → shortcuts/assets/assistant/About | Actual rendered open/close returns to the originating category, without stacking or losing a curve/transform draft. |

These are automated/agent walkthroughs. No first-click success rates or user-observed mistakes are invented.

## Display and visual checks

- Rendered interaction checks cover **1366×768 and 1920×1080**, each at **100%, 125%, 150% and 200%** native scale, in light and dark mode.
- The actual Linux executable was opened under isolated Xvfb displays with `WINIT_X11_SCALE_FACTOR`, producing 16 native Settings captures at those physical dimensions. These are synthetic native-DPI checks, not physical monitor/hardware tests.
- Another 36 native Full HD captures cover the editor and 17 focused destinations in both themes. Captures used actual command search/action dispatch, not a mockup.
- The same running rendered editor is resized and moved through synthetic DPI values; an oversized 300% interface scale is recovered through the keyboard.
- Native captures were visually inspected and independently reviewed. They motivated the Settings body-height cap, one-shot selected-category reveal and meaningful action icons. Inspector remains left, tools right and Preview bottom-left.
- Focus, Tab/Enter, scrolling, checked/selected and disabled states, pointer activation, dialog close/backdrop behavior and fixed footers are covered by rendered interaction tests. Static screenshots do not alone prove hover or pressed behavior.

Representative Full HD editor and Settings screenshots are stored in `shots/`, and the README/guide select light or dark images using the viewer's GitHub theme.

## Review findings resolved

- Long menus clipped uncommon exports; bounded scrolling plus pointer, wheel and Tab/Enter tests now reach the final export.
- Disabled actions and unavailable recent files lacked explanations; one availability predicate now supplies both menu and search reasons.
- Newly added IDs could shadow precise command queries; exact IDs now rank first.
- Broad reset crossed into tools/view/export state; category/app resets now respect the inventory's ownership contract.
- Test editors shared configuration; storage is now explicitly injected, while native startup restores the existing user path before applying preferences.
- Focus could move outside scrolling content; Settings, workflow, properties and sheet dialogs reveal it while keeping fixed chrome stable.
- New dialogs exposed a shared Done-return bug; return handling and existing dialog workflows were verified.
- Old toolbar/property shortcuts reopened unrelated pages; they now use canonical task/object routes.
- Group metadata incorrectly used an image-only edit predicate; it now respects ancestor locks while allowing unlocked groups.
- Off-canvas tile capture could panic; selection bounds are clipped before indexing and the candidate document is validated before Undo.
- Image scale was hidden behind the sheet draft; actual export flows now expose a local draft, preserve cancellation and retain accepted scale on Repeat.
- An animation-to-file-dialog transition could lose typed input; focus delivery is acknowledged before retiring the initial request.
- Fast shortcut taps lost their modifiers; matching now uses press-event modifiers, including exact custom chords. macOS menu shortcuts yield to explicit custom assignments.

## Native macOS check

The platform adapter retains its AppKit target, clears its weak menu target before dropping it and coalesces callbacks through a bounded queue. It owns no editor pointer across FFI. The packaged macOS smoke check verifies a synthetic Command-Comma event, menu activation, custom-override enable/disable behavior, bounded delivery and the production UI event-drain route, then captures Settings. Both macOS architectures run this check in CI.
