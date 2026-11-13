# UI interaction-state audit — 2026-11-13

## Verified behavior

- Preview uses the available inspector width and a vertically resizable bottom-left area. Fit preserves aspect ratio; 1:1 and integer zoom use physical pixels and nearest sampling. Panning is constrained to oversized artwork. Height/zoom restore from validated preferences, and frame/tab changes use current frame textures.
- Native window decorations are requested. The custom title bar and custom window resize/drag controls are removed. Inspector collapse belongs to its header and a restore control remains at the left edge. Existing close-request tests protect dirty active/background tabs and test cancellation/discard.
- Standard egui-backed buttons, menus, choices and timeline cells inherit hover, pressed, selected, disabled and keyboard-focus painting. Existing tests cover selected timeline cells, names, Tab/Enter activation, menus, tooltips and preview interactions.
- A new light/dark functional regression checks that icon buttons and custom switches reject disabled clicks, accept enabled clicks, and activate with Enter. This does not assert every visual state of every widget.

## Interaction feedback fixes

- Custom switches and Colors and Properties headings distinguish hover, press, and keyboard focus in both themes.
- Artwork swatches preserve the selection border during hover and press and show a separate focus outline. Accessibility metadata includes the hex color, alpha for translucent colors, and selected state.
- Edit menu Undo and Redo availability follows the active tab's history.
- Regression tests verify swatch names and selection updates and Edit menu state after an edit, undo, and tab switch.
