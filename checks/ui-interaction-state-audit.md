# UI interaction-state audit — 2026-11-13

## Verified behavior

- Preview uses the available inspector width and a vertically resizable bottom-left area. Fit preserves aspect ratio; 1:1 and integer zoom use physical pixels and nearest sampling. Panning is constrained to oversized artwork. Height/zoom restore from validated preferences, and frame/tab changes use current frame textures.
- Native window decorations are requested. The custom title bar and custom window resize/drag controls are removed. Inspector collapse belongs to its header and a restore control remains at the left edge. Existing close-request tests protect dirty active/background tabs and test cancellation/discard.
- Standard egui-backed buttons, menus, choices and timeline cells inherit hover, pressed, selected, disabled and keyboard-focus painting. Existing tests cover selected timeline cells, names, Tab/Enter activation, menus, tooltips and preview interactions.
- A new light/dark functional regression checks that icon buttons and custom switches reject disabled clicks, accept enabled clicks, and activate with Enter. This does not assert every visual state of every widget.

## Remaining visual/usability gaps

1. Custom Fluent switches paint on/off state but do not explicitly vary hover, pressed or keyboard-focus appearance.
2. Custom Colors/Properties headings likewise have no distinct hover/pressed/focus painting.
3. Artwork swatches show selection and a tooltip, but lack distinct hover/pressed/focus painting and accessible name/selected-state metadata.
4. Undo and Redo menu items remain enabled when their histories are empty. The handlers do not perform a history action in that state, but the menu should explain unavailability through disabled state.

The interaction-state redesign is therefore not complete. Do not interpret passing functional tests or screenshots as exhaustive visual-state verification. Native Windows frame interactions require the Windows build/manual testing; client-area headless screenshots do not include OS decorations.
