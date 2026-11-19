# Menu and settings inventory

Baseline is v0.10.0 at 8ddddfc. This is the before/after map for issue #25 and the branch implementation plan.

## Ownership

| Scope | Stored state | User entry point |
| --- | --- | --- |
| App preference | Preferences and existing theme/shortcut configuration | Categorized Settings |
| Workspace/view | Inspector, Preview, split view, canvas zoom/pan and UI state | View and layout recovery |
| Document | Sprite/layer/frame properties, profile, tilemaps, grids and references | Relevant object properties with native Undo |
| Tool preference | Current brush, ink, sampler, flood and gradient configuration | Active tool options and brush manager |
| Operation | Import cell geometry, sheet layout, atlas packing and export scaling | Focused import/export task |

## Current command inventory and intended grouping

| Current menu and label | Stable action ID | Intended owner |
| --- | --- | --- |
| File / New sprite…   Ctrl+N | new | File |
| File / Open…   Ctrl+O | open | File |
| File / Import as Layer… | import-layer | File / Import |
| File / Close sprite   Ctrl+W | closetab | File |
| File / Import PNG sequence… | sequence | File / Import |
| File / Grid import, tag batch & atlas… | workflow | Focused import, export, brush, palette, reference or assistant route |
| File / Save   Ctrl+S | save | File |
| File / Save as… | saveas | File |
| File / Repeat Last Export | repeat-export | File |
| File / Export baked ASE copy… | export-baked-ase | File / Export |
| File / Export PNG… | png | File / Export |
| File / Export sprite sheet… | sheet | File |
| File / Export PNG sequence… | png-sequence | File / Export |
| File / Export animation GIF… | gif | File / Export |
| File / Export video MP4… | mp4 | File / Export |
| File / Export FLI… | fli | File / Export |
| File / Export FLC… | flc | File / Export |
| File / Export tileset atlas… | tileset-atlas | File / Export |
| File / Export Tiled map… | tiledexport | File / Export |
| File / Export BMP… | bmp | File / Export |
| File / Export JPEG… | jpeg | File / Export |
| File / Export WebP… | webp | File / Export |
| File / Export icon… | ico | File / Export |
| File / Export PCX… | pcx | File / Export |
| File / Export QOI… | qoi | File / Export |
| File / Export SVG… | svg | File / Export |
| File / Export CSS… | css | File / Export |
| File / Export TGA… | tga | File / Export |
| File / Open sample garden | demo | File |
| Edit / Search commands… | command-search | Edit |
| Edit / Undo   Ctrl+Z | undo | Edit |
| Edit / Redo   Ctrl+Y | redo | Edit |
| Edit / Undo History… | undo-history | Edit |
| Edit / Copy   Ctrl+C | copy | Edit |
| Edit / Copy Merged | copy-merged | Edit |
| Edit / Cut   Ctrl+X | cut | Edit |
| Edit / Paste   Ctrl+V | paste | Edit |
| Edit / Paste as New Sprite | paste-as-sprite | Edit |
| Edit / Paste as New Layer | paste-as-layer | Edit |
| Edit / Paste as Reference | paste-as-reference | Edit |
| Edit / Clear   Delete | clear | Edit |
| Edit / Replace Color… | replace-color | Edit |
| Edit / Filters… | filters | Edit |
| Edit / Select all   Ctrl+A | selectall | Select / selection or transform |
| Edit / Deselect   Esc | deselect | Select / selection or transform |
| Edit / Settings… | settings | Edit / Settings |
| Edit / Keyboard shortcuts… | shortcuts | Edit / Keyboard shortcuts and Settings / Keyboard Shortcuts |
| Edit / Drawing & selection studio… | studio | Tools / drawing and selection |
| Edit / Magic Wand | wand | Tools / drawing and selection |
| Edit / Brushes, references & workflow tools… | workflow | Focused import, export, brush, palette, reference or assistant route |
| Edit / Lasso selection | lasso | Tools / drawing and selection |
| Edit / Move selection | move | Tools / drawing and selection |
| Edit / Scale, skew, or rotate cels on canvas… | canvas-transform | Select / selection or transform |
| Edit / Rotate selection 90° | rotate | Select / selection or transform |
| Edit / Flip selection horizontally | selectionflipx | Select / selection or transform |
| Edit / Flip selection vertically | selectionflipy | Select / selection or transform |
| Edit / Crop to selection | crop | Select / selection or transform |
| Edit / New Sprite from Selection | new-sprite-from-selection | Select / selection or transform |
| Sprite / Properties, layers, tiles & ranges… | properties | Sprite / Properties, with direct Layer, Frame and Tileset routes |
| Sprite / Palette and Color Conversion… | palette-conversion | Sprite |
| Sprite / Color Profile… | color-profile | Sprite |
| Sprite / Canvas size… | resizecanvas | Sprite |
| Sprite / Sprite size… | spritesize | Sprite |
| Sprite / Duplicate sprite | duplicatesprite | Sprite |
| Sprite / Auto-trim sprite | trimsprite | Sprite |
| Sprite / Rotate sprite 90° clockwise | rotatespriteright | Sprite |
| Sprite / Rotate sprite 90° counterclockwise | rotatespriteleft | Sprite |
| Sprite / Rotate sprite 180° | rotatespritehalf | Sprite |
| Sprite / Flip sprite horizontally | flipspritex | Sprite |
| Sprite / Flip sprite vertically | flipspritey | Sprite |
| Sprite / Tileset browser… | tiles | Sprite |
| Sprite / Flip active cel horizontally | flipx | Frame / current cel transform |
| Sprite / Flip active cel vertically | flipy | Frame / current cel transform |
| Layer / New layer | layer | Layer |
| Layer / Duplicate layer or group | layer-duplicate | Layer |
| Layer / Merge down | layer-merge | Layer |
| Layer / Flatten visible layers | flatten-visible | Layer |
| Layer / Flatten all layers | flatten-layers | Layer |
| Layer / Layer via copy | layer-via-copy | Layer |
| Layer / Layer via cut | layer-via-cut | Layer |
| Layer / Convert to raster layer | convert-raster | Layer |
| Layer / Convert to background | convert-background | Layer |
| Layer / Convert to tilemap… | convert-tilemap | Layer |
| Layer / Delete layer | deletelayer | Layer |
| Layer / Move layer up | up | Layer |
| Layer / Move layer down | down | Layer |
| Frame / Duplicate frame | frame | Frame |
| Frame / New empty frame | blank | Frame |
| Frame / Delete frame | deleteframe | Frame |
| Frame / Reverse frames | reverse-frames | Frame |
| Frame / Play / stop   Space | play | Frame |
| Frame / Play once | play-once | Frame |
| Frame / Play selection | play-selection | Frame |
| Frame / Play selection once | play-selection-once | Frame |
| Frame / Play subtags | play-subtags | Frame |
| Frame / Play subtags once | play-subtags-once | Frame |
| Frame / Rewind on stop | rewind-on-stop | Frame |
| Frame / Previous frame   , | prev | Frame |
| Frame / Next frame   . | next | Frame |
| View / Toggle pixel grid | grid | View |
| View / Show drawing grid | drawing-grid | View |
| View / Snap to drawing grid | snap-to-grid | View |
| View / Drawing grid settings… | drawing-grid-settings | View |
| View / Set drawing grid from selection | grid-from-selection | View |
| View / Toggle onion skin | onion | View |
| View / Fit canvas | fit | View |
| View / Split canvas | split-canvas | View |
| View / Seamless canvas · both axes | seamless | View |
| View / Seamless canvas · horizontal | seamless-horizontal | View |
| View / Seamless canvas · vertical | seamless-vertical | View |
| Select / Select all | selectall | Select / selection or transform |
| Select / Elliptical marquee | elliptical-marquee | Select |
| Select / Polygonal lasso | polygonal-lasso | Select |
| Select / Deselect | deselect | Select / selection or transform |
| Select / Reselect | reselect | Select |
| Select / Invert selection | invert-selection | Select |
| Select / Color Range… | color-range | Select |
| Select / Grow, Shrink & Border… | modify-selection | Select |
| Select / Import mask PNG… | import-selection-mask | Select |
| Select / Export mask PNG… | export-selection-mask | Select |
| Extensions / Asset extensions… | extensions | Tools / Asset extensions |
| Help / Tools and file support | help | Help |
| Help / About | about | Help |

Dynamic entries include recent files, System/Light/Dark themes, Compact/Comfortable density, Inspector and Preview visibility, palette actions and active tools. Their existing target values, state predicates and shortcut aliases remain part of the shared search catalog.

## Persisted preference fields

| Field | Current default | Scope and destination |
| --- | --- | --- |
| recent | vec![] | App history / File Recent |
| onion | OnionOptions::default() | App preference / Animation |
| rewind_on_stop | false | App preference / Animation |
| preset | String::new() | Remembered operation choice / Sheet export |
| interface_scale | 1. | App preference / Appearance |
| comfortable | false | App preference / Appearance |
| checker_size | 8 | App preference / Canvas & Navigation |
| checker_colors | None | App preference / Canvas & Navigation |
| autosave | true | App preference / Files & Recovery |
| new_sprite_width | 32 | App default / Files & Recovery / New documents |
| new_sprite_height | 32 | App default / Files & Recovery / New documents |
| default_grid | false | App default / Files & Recovery / New documents |
| pixel_grid_color | [0, 0, 0, 45] | App preference / Canvas & Navigation |
| pixel_grid_auto_opacity | true | App preference / Canvas & Navigation |
| background_updates | false | App preference / Updates & About |
| sample_references | false | Persisted sampling option / References |
| eyedropper | EyedropperOptions::default() | Persisted tool option / Eyedropper options |
| gradient | GradientOptions::default() | Persisted tool option / Gradient options |
| tool_inks | Default::default() | Persisted tool option / Ink controls |
| brush_dynamics | Default::default() | Persisted tool option / Brush dynamics |
| import_profile | pixelcraft::color::ImportProfile::default() | App default / Color Management |
| display_profile | crate::display_profiles::DisplayChoice::default() | App preference / Color Management |
| whole_pixel_wheel_zoom | false | App preference / Canvas & Navigation |
| trim_canvas_content | true | App preference / Canvas & Navigation |
| fill_flood | FloodOptions::default() | Persisted tool option / Fill options |
| wand_flood | FloodOptions::default() | Persisted tool option / Wand options |

## Control families outside app Settings

| Current renderer | Controls and stored owner | Intended destination |
| --- | --- | --- |
| palette_arrangement_ui | Palette selection/order/ramp operations on current document, interpolation and palette slots | Palette tools |
| brush mode, brush library and brush dynamics | Brush shape/size, image alignment, ink, stamps, saved brushes, shading and dynamics | Active tool strip and Brushes |
| flood and selection options | Tolerance, contiguous/global matching, mask operation, flood options and color-channel sampling | Active tool options |
| reference_controls | Document reference image list, visibility, position, dimensions/aspect, opacity, locking, per-image sampling and link state | Reference images with native Undo |
| Reference sampling default | Preferences.sample_references, default false | Reference images / sampling preference |
| grid_import_ui | Source, cell size/count, order, spacing/offset, partial/empty cells and timing | File Import / Sprite sheet |
| Batch tag export | Selected source/output, format/preset, scale and native tag exports | File Export / Animation tags |
| atlas_export_ui | Inputs, append/remove, packing layout, dimensions/padding, metadata type and preview/save | File Export / Multi-file atlas |
| Assistants & headless tools | Current editor opt-in, live command and connection state | Tools / Assistant access |
| Sprite property controls | Document color mode, palette entry count, pixel aspect and drawing-grid origin/cell geometry, conversion launchers and metadata summary | Sprite properties |
| Layer/cel property controls | Document blend mode, parent group, cel opacity/order and New Group | Layer properties |
| Tile import and paint choices | Session tile_size, selected_tile and tile_pixel_mode, import/capture/new tile-layer actions | Tileset tools |
| Slice controls | Document slice names, frame keys, bounds, center and pivot metadata | Sprite slice properties |
| Animation range controls | Document tag names, endpoints and direction, preserved repeat metadata, add/remove; session active playback tag | Frame and animation-range properties |
| Wraparound & palette | Document wraparound and palette_snap, with Undo | View seamless drawing and Palette tools |
| Onion enable | Current view self.onion, default false | Frame/View onion toggle |
| Recent files & shortcuts | App history open/clear and shortcut editor link | File Recent and Keyboard Shortcuts |
| Sprite-sheet export | SheetOptions and selected source layers/tag/slice, export scale, packing/filter/template controls | File Export sprite sheet |
| Drawing studio | Existing selection, pivot/rotation, effects, brush and animation edit operations | Tools / Drawing studio with current tab routes |
| shortcut editor | Binding capture, conflict handling, per-command resets and defaults | Settings Keyboard Shortcuts / direct menu |
| asset extensions | Declarative package management and existing path/size checks | Settings Extensions & Automation / direct menu |
| About and update UI | App version, available update/install status and project/support links | Settings Updates & About / Help |

## Settings layout

Wide logical viewport

    Settings                          Search settings   [Reset interface scale]
    Appearance        |  Page title and scope
    Canvas & Navigation  Existing controls with accessible labels
    Animation         |  Matching controls highlighted/revealed
    Color Management  |  Page reset with scope
    Files & Recovery  |
    Keyboard Shortcuts
    Extensions & Automation
    Updates & About                   [Reset preferences...] [Done]

Narrow logical viewport uses a compact page selector and vertically scrolling content. Search, Close and interface-scale recovery have reachable locations. Search results include page/control paths and open the relevant destination.

## Reviewable batches and verification record

Execution and verification results are appended in the implementation plan. Menu aliases resolve through shared native routing. Preferences retain their existing storage keys, and new navigation metadata uses tolerant defaults.

## Nested control defaults

Defaults below are from the baseline model and renderer. They identify each nested configurable field so focused routes retain their controls.

| Type and source | Fields and default values |
| --- | --- |
| OnionOptions, src/productivity.rs | previous 1, next 0, previous_tint [150,185,220], next_tint [240,150,150], previous_opacity 64, next_opacity 64 |
| FloodOptions, src/productivity.rs | sample_merged false, stop_at_grid false, diagonal false, independently for Fill and Wand |
| BrushDynamics, src/freehand_stroke.rs | stabilizer 0, pressure_size false, pressure_opacity false |
| EyedropperOptions, src/productivity.rs | source Visible, channel ColorAlpha |
| GradientOptions, src/productivity_ui.rs | shape Linear, pattern Smooth |
| GridImport, src/productivity.rs | width 16, height 16, offset_x/offset_y 0, spacing_x/spacing_y 0, layers false |
| GridImportOptions, src/productivity.rs | skip_transparent false, duration 100ms, order RowMajor, include_partial false, columns None, rows None |
| SheetOptions, src/workflows.rs | scale 1, columns 8, rows 8, layout Rows, width/height None, padding 1, border_padding/shape_padding None, inner_padding 0, trim/trim_by_grid false, tag/frame_range/layers None, split_layers/split_grid false, filename_template/tag_template empty, preset Generic |
| AtlasOptions, src/atlas.rs | layout MaxRects, width 1024, padding 1, trim true, deduplicate true, power_of_two false, metadata Array |

The GridImport geometry definitions and default cell/spacing/offset values are in src/productivity.rs immediately before GridImportOptions. Existing renderers additionally own operation source/output paths, preview state, selected input files, presets and current target IDs. Those operation choices remain part of focused import/export flows.

## Configuration and reset contract

| Storage or state | Owner and default handling |
| --- | --- |
| workflow.json | Existing Preferences fields and new tolerant last-settings-page ID |
| theme.json | Theme preference, default System |
| custom-theme.json | Applied custom style override, managed explicitly by appearance reset |
| shortcuts.json | Current custom bindings, managed by the shortcut editor and its confirmed reset |
| preview.json | Preview visibility, size and zoom, managed by Preview controls and layout recovery |
| Inspector visibility | Session workspace state |
| Split canvas, canvas zoom/pan | Per-tab view state |
| Live assistant bridge | Explicit active-editor session opt-in, token and cancellation state |

Page resets restore the values owned by that page. Appearance restores System/default style, density and scale. Canvas restores display/navigation preferences. Animation restores onion defaults and rewind behavior. Color restores import/display defaults. Files restores new-document and recovery defaults. Updates restores its update preference.

Confirmed app-preference reset combines those app-page defaults. Recent files, remembered export presets, saved tool configuration, custom shortcut bindings and installed asset packages have their own management controls. Current documents, Undo/Redo, view grid toggles and in-progress operation drafts retain their current state. Reset layout owns workspace/Preview state independently. Confirmation copy states the exact reset scope.

Opening Settings, page navigation, preference loading and resets only manage their documented preferences. The assistant checkbox is the sole enable/disable control for the active editor bridge. Token privacy, revision checks and disable-time cancellation use existing bridge behavior.

## Early macOS feasibility result

The current eframe/winit application creates an AppKit application menu before the real PixelCraft app creator runs. The existing macOS objc2/AppKit dependencies support inserting a Settings item with Command-Comma into that menu.

A small native_app_menu controller can retain a main-thread Objective-C target and its owned menu items. Its callback uses a bounded channel plus request_repaint, and the egui update loop drains the event into the existing settings action. The controller clears the weak menu target and removes only its own items on drop. Headless app construction does not initialize AppKit.

Winit and the project use different Rust binding versions. The adapter obtains NSApplication independently through the project's bindings and exchanges no typed Objective-C wrappers with winit. Actual native menu/shortcut/teardown and packaged startup checks run in macOS CI.

## Visual and DPI coverage

Rendered test scenes cover the 1366×768 and 1920×1080 physical-size matrix with logical viewports derived from 100%,125%,150% and200% DPI. Resizing and synthetic native-pixels-per-point changes verify reflow, reachable navigation/footer actions and retained focus. Native monitor moves and physical tablet/keyboard behavior are separately reported hardware checks. Automated screenshots are visually inspected in light/dark themes.
