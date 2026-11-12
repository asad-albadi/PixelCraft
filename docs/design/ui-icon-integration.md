# Shared icons in the editor

Implementation of [issue #3](https://github.com/asad-albadi/PixelCraft/issues/3),
using the bundled original MIT outline icons and the adjacent visual concept.

`src/ui_icons.rs` centralizes semantic command aliases, accessible action names,
texture caching, button states, menus, checkboxes and dialog action mappings.
The machine-readable [action inventory](ui-action-inventory.json) lists command
and dialog labels, semantic icons, shortcut sources, locations and state pairs.
The complete source artwork, categories and labels remain in
`assets/icons/ui/manifest.json`. Its local `preview.html` displays every source
at 16/20/24 logical pixels on dark and light surfaces with interaction samples.

| Surface | Integration |
| --- | --- |
| Drawing toolbar | All 13 tools use the shared pack, 34px targets, selected surfaces, keyboard focus and current-key tooltips. The toolbar scrolls at small heights. |
| Menus | File/Edit/Sprite/Layer/Frame/View/Extensions/Help command icons; leading check gutter, separate semantic icon, right-aligned live shortcuts, matching Recent/Theme submenu arrows. Destructive commands have separators. |
| Document row | Theme-accent active tab, distinct inactive tabs, capped/truncated labels with full-name hover, separate 28px close target, dirty dot, immediate matching-height New/Open actions. A horizontal scroll area retains long/many tabs. |
| Timeline | Play switches to Stop; previous/next, frame duplication, layer creation, deletion, ordering, rename, onion skin and properties reuse command icons. Visibility, padlocks, layer kinds and occupied/empty cels reflect real state. |
| Colors | Add/remove, palette saving/loading and project properties; named choices keep text and selected state. Artwork, swatches, color chips, logo and thumbnails retain their own colors. |
| Dialogs/workflows | Confirmation, cancellation, discard, extension install/uninstall, transforms, selection operations, brush modes/shapes, palette interchange, references, atlas, sheet import and batch export. |
| Settings/assistants | Theme/shortcut choices, updates and external links use shared icons. Live permission retains its explicit text checkbox and switches live-on/live-off. Errors and save warnings include labeled feedback icons. |
| Script dialogs | Built-in OK/stop/browse and script action buttons use the same button component; arbitrary script-supplied text remains visible. |

## Theme and display behavior

White alpha masks are embedded in the executable and lazily uploaded once per
icon/density. Buttons tint through egui's current widget text style, including
hover, disabled, selected and focus states. Checkboxes and feedback use the
active visual colors. Custom foreground colors also update selection strokes.
There are no network or symbol-font dependencies for these action icons.

16/20/24px sources plus 32/40/48px density exports cover 100/150/200% scaling;
texture selection follows `Context::pixels_per_point`, so changing DPI changes
the cache key without changing logical hit targets. Regenerate all exports with
`python3 checks/export-ui-icons.py`; librsvg is only needed for authoring.

Shortcut hints come from the same editable map used by keyboard dispatch.
Tooltips omit unassigned keys and display rebound keys. Value choices, including
user-named colors, groups and tags, never inherit command bindings from their labels. Menu labels have no
hardcoded shortcut suffixes. Stateful playback uses the `play` binding even
when its artwork and label say Stop. Dialog-only confirmation, discard and
reference/numeric movement do not claim unrelated drawing shortcuts.

## Verification and limits

`ui_icon_tests` exercises keyboard activation and minimum hit targets at
100/150/200%, independent close/New behavior with dirty artwork, long names,
multiple tabs, narrow windows, dark/light/custom colors, File/Edit/Frame menus
and primary dialogs. Asset checks decode every icon at all six densities and
validate transparent white masks. Shortcut tests check rebinding/removal and value-choice names; the
existing model, UI, theme, recovery, export and command tests are retained.

The PNGs under `shots/ui-icons/` are test-only software renders of the actual
egui layout, tessellated geometry, font atlas and image textures. They are
layout/theme/scaling evidence, not platform screenshots. The native renderer's
startup screenshot is separately produced by `rebuild_check` and copied to
`shots/ui-icons/native-editor.png`.

To regenerate software previews from this directory:

```sh
PIXELCRAFT_HOME=/tmp/pixelcraft-icon-review \
PIXELCRAFT_UI_REVIEW_DIR="$PWD/shots/ui-icons" \
cargo --config 'profile.test.package.pixelcraft.opt-level=2' test --offline \
  --bin pixelcraft ui_icon_tests
```

The container's approval policy prevents separate direct desktop launches;
`rebuild_check` supplies the permitted native startup capture. Windows,
physical mixed-DPI monitor transitions and OS screen-reader output were not
available here. Widgets retain egui action labels and toggle metadata; those
platform integrations still need a desktop validation pass. PixelCraft had no
canvas/layer context menus to migrate; this change does not introduce commands.
