# Aseprite compatibility

PixelCraft is independent software with partial Aseprite compatibility, not a drop-in implementation of the entire Aseprite API. No Aseprite executable, installation, settings, fonts, theme atlas, or other original assets are loaded by the app. Lua 5.4 is embedded; ICC conversion uses the pure Rust moxcms engine.

## Projects and rendering

Supported: .ase and .aseprite RGB, grayscale and indexed projects; raw, compressed and linked image cels; linked tilemap cels; 8-, 16- and 32-bit tile entries with ID and flip masks; legacy all-bits-set empty tiles; sparse tileset IDs; per-cel tilemap dimensions and signed pixel origins (including off-grid and off-canvas maps); embedded and external tilesets. Layer tileset references are ordinal indices, independently of tileset IDs. External ASE files resolve relative to the containing file and recursively by tileset ID, with cycle/depth checks and explicit missing-file errors. Saves embed resolved pixels, so reopening the saved project does not require its external source.

Source pixels, cel bounds, tile IDs, groups, legacy pass-through group compositing, palette animation, frame durations, tags/repeats, layer UUIDs, opaque user data/properties, slices and profile bytes survive supported round trips. Save is semantically lossless for the tested projects, not byte-identical: chunks are reordered, links can become independent cels, tileset encodings are normalized to 32-bit entries and compression is regenerated. Duplicate indexed colors inside tileset images and transparent off-canvas index identities are not guaranteed to retain their original slot. Future chunk semantics remain opaque. Rasterization of scaled reference cels / precise cel-extra bounds is not implemented.

ICC RGB/grayscale matrix and LUT profiles and fixed-gamma sRGB convert to sRGB for the canvas, onion skin and frame previews. Alpha and editable source values remain unchanged; PNG retains the source profile. Invalid or unsupported profiles report an error in the editor and display unconverted values. Display calibration to a monitor-specific ICC profile and proofing intents are not implemented. GIF has no ICC output.

## Lua objects and actions

Supported on the active document: app sprite/layer/frame/cel/image/site/range/editor snapshots, parameters, foreground/background colors, Color RGB/HSV/HSL constructors and channel edits, gray/index/packed pixel access, Point/Size/Rectangle, image cloning/pixels/drawing, palette edits, layer/group creation and deletion, frame insertion, tags, cel movement/replacement (including outside-canvas pixels), selection masks and boolean operations, named/nested transactions with rollback, script-local Undo/Redo, plugin commands and preferences. Successful script results enter the editor as an undoable change.

Commands: NewLayer (name/group parameters), NewFrame, NewEmptyFrame, RemoveLayer, Clear, SelectAll, DeselectMask, InvertMask, Undo, Redo, first/last/previous/next frame, ChangePixelFormat, SaveFile, SaveFileAs and SaveFileCopyAs. Unknown commands error. Lua methods and command parameters beyond this list are not universally implemented.

File-backed editor sessions support app.open, saveAs/saveCopyAs, PNG/GIF export, path helpers, directory queries/creation and package-local require/dofile/loadfile. Relative paths start in the script folder. Process execution and unrestricted Lua io are unavailable; os exposes only clock/time/date/difftime. Preferences live in PixelCraft's own config folder and plugin preferences are keyed by script path. Scripts run with allocation and instruction limits. Opening a file replaces the worker's active document; multiple independent Sprite objects, closing editor tabs from Lua, full Aseprite preference schemas and exact object/userdata identity are not implemented. The range currently represents the active cel, not every editor timeline multi-selection. app.params is accepted by the host API; the Run Script UI does not yet provide a parameter editor.

## Interactive extensions

Real egui controls support Dialog label, entry, number, slider, check/radio, combobox, color, shades, separator, file and button widgets; data, modify, show, close, repaint and callback handling. Modal dialogs pause the worker; wait=false dialogs continue through the event pump. Unnamed button callbacks work. Timers run periodically from elapsed editor time; they can be started/stopped and retain their callbacks. app.events supports color/site changes, sprite.events supports change, and handlers can be removed. refresh previews edits in the editor. Stop extension exits the interactive pump. Editor-backed sessions remain active while they own dialogs, timers or subscribed events.

Custom canvas/graphics-context widgets, exact original dialog layouts/tab groups, radio exclusivity, dialog bounds positioning, full editor mouse/paint events, hot-reloading and extension command menu placement are not implemented. Concurrent editor/script edits use document snapshots rather than independent per-operation history. Prefer finishing/stopping an extension before unrelated structural editing.

## Contributions

Install folders, ZIP and .aseprite-extension packages using original package.json contributions: scripts, themes, keys (also shortcuts), languages, palettes and ditheringMatrices. Leading ./ paths and theme directories with theme.xml are accepted. Package paths remain inside the package; extraction retains traversal, size, symlink and CRC checks.

- Themes: JSON and original XML color definitions adapt to PixelCraft's native controls. User-supplied theme folders remain installed; original sprite atlases, dimensions and fonts do not replace the native widgets.
- Shortcuts: original .aseprite-keys/XML command and tool bindings map supported actions. Unsupported commands/parameterized bindings are reported rather than silently assigned another action. Native JSON bindings also work.
- Languages: original section/key INI files are parsed and stored; matching menu headings are translated. Full original UI strings do not all have a counterpart.
- Dithering: original BMP/PNG matrix images load ordered thresholds and can be selected for subsequent indexed conversion. Palette quantization is PixelCraft's implementation; byte-for-byte Aseprite quantization is not claimed.

## Reproducible evidence

cargo test --offline runs without the original executable or network. tests/real_projects.rs opens ten upstream ASE projects (33 frames), compares every rendered pixel against checked-in RGBA reference sheets, checks tags/profile/opaque metadata, then saves/reopens ASE and native projects. Golden reference sheets were generated once with Aseprite 1.3.18.6-dev using RGB conversion before sheet export, to avoid indexed-PNG transparent-slot ambiguity. Running or testing PixelCraft does not regenerate them or invoke Aseprite. Source URLs, upstream Git blob IDs and SHA-256 hashes are recorded in tests/fixtures/upstream.json.

The real corpus includes RGB/gray/indexed tilemaps, linked cels, nested groups, z order, animated slices, tag repeats and nested user properties: 2x2tilemap2x2tile, 2x3tilemap-indexed, 3x2tilemap-grayscale, 2f-index-3x3, groups3abc, link, file-tests-props, z-order, slices-moving and tags3x123reps from https://github.com/aseprite/aseprite/tree/main/tests/sprites . These are test data, not application assets.

Four unchanged MIT-licensed scripts from https://github.com/aseprite/Aseprite-Script-Examples run in tests/real_extensions.rs: Random Palette, Random Pixels (RGB/gray/indexed), Move Cels (real dialog response and movement), and Wizard Example (three modeless dialog pages). Their license is retained beside the fixtures. Additional regression tests cover external file resolution/portable saves, compact tile encodings, legacy empty IDs, negative origins, ICC linear-light conversion and alpha, original contribution formats, dialogs/timers/events/parameters, file operations and failure rollback. The egui widget renderer is also exercised directly. This corpus is evidence for these features, not certification of arbitrary third-party extensions.

Build: cargo build --offline. Isolated runtime check: checks/standalone.sh, which mounts only the built binary and basic system libraries. Craft Anything rebuild_check additionally builds and captures the application window.
