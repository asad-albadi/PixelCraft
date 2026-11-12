# PixelCraft UI icons

Original geometric outline icons based on the visual direction in
[issue #3](https://github.com/asad-albadi/PixelCraft/issues/3).

- `svg/` contains transparent, editable 24×24 sources using `currentColor`.
- `png/16/`, `png/20/`, and `png/24/` contain transparent white RGBA icons.
  `png/32/`, `png/40/`, and `png/48/` supply high density variants.
  Tint them with the active theme's foreground color in egui. The smaller
  exports use slightly heavier strokes for readability.
- `manifest.json` lists names, labels, categories, and existing command mappings.
- `preview.html` opens locally and shows dark/light surfaces and actual sizes.

Run `python3 checks/export-ui-icons.py` from the repository root to regenerate
the PNGs. The authoring machine needs `rsvg-convert` from librsvg. The app build
does not require it.

Keep the foreground color independent of selection, hover, disabled, and focus
surfaces. These belong to the button component. Use the same semantic icon in
menus and toolbars, retain labels on menu items, and show current shortcuts in
tooltips. Display `stop` while playback is active. Switch `eye`/`eye-off`,
`lock`/`unlock`, and `live-on`/`live-off` to reflect actual state.

Use `close` for the independent close target inside each document tab. Use
`file-new` or a labelled plus for the adjacent New button and `folder-open` for
Open. Keep the tab's selection surface separate from the close target.

The app integrates these assets through `src/ui_icons.rs`. See
`docs/design/ui-icon-integration.md` for the action inventory and verification.

These original icons are covered by the repository's MIT license.
