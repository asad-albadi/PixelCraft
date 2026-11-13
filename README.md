<p align="center">
  <img src="assets/pixelcraft-logo.png" alt="PixelCraft's teal pixel P emblem with a gold pixel" width="128">
</p>

<h1 align="center">PixelCraft</h1>

<p align="center">
  <b>Pixel art, sprites and animation, in a standalone desktop editor.</b><br>
  Draw frame by frame, build with tiles, and keep your layers editable.<br>
  Open source, local, and yours.
</p>

<p align="center">
  <img alt="Built with Rust" src="https://img.shields.io/badge/built_with-Rust-b7410e?style=flat-square&amp;logo=rust">
  <img alt="Platform: Linux" src="https://img.shields.io/badge/platform-Linux-267477?style=flat-square&amp;logo=linux&amp;logoColor=white">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-3a3a3a?style=flat-square">
  <img alt="Status: early alpha" src="https://img.shields.io/badge/status-early%20alpha-d69e2e?style=flat-square">
</p>

<p align="center">
  <b>Made via <a href="https://github.com/storytold/craft-anything">CraftAnything</a> by <a href="https://getartcraft.com/">ArtCraft</a>.</b><br>
  An independent project created with Craft Anything's study, build and check workflow.
</p>

<p align="center">
  <a href="#get-started"><b>Build and run</b></a> ·
  <a href="https://github.com/storytold/craft-anything">Craft Anything</a> ·
  <a href="https://getartcraft.com/">ArtCraft</a> ·
  <a href="https://getartcraft.com/apps">The Crafting Apps</a>
</p>

<p align="center">
  <img src="shots/pixelcraft.png" alt="PixelCraft's current interface at 1920×1080, showing sprite tabs, the compact Colors selector, dominant-color swatches, mushroom garden artwork, the bottom-left sprite preview, drawing tools and animation timeline" width="100%">
  <br>
  <sub>The editor with its built-in garden sample: three layers, four frames, and editable pixel artwork.</sub>
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#get-started">Get started</a> ·
  <a href="#editing">Editing</a> ·
  <a href="#projects-and-interchange">File formats</a> ·
  <a href="#independent-extensions-and-storage">Extensions</a> ·
  <a href="#status">Status</a> ·
  <a href="#license-and-credits">License and credits</a>
</p>

## Features

<table>
  <tr>
    <td width="33%" valign="top">
      <h3>Draw pixel by pixel</h3>
      Pencil, fill, shapes, selections and five brush shapes. Work in RGB, grayscale or indexed color, with editable palettes and undo.
    </td>
    <td width="33%" valign="top">
      <h3>Bring sprites to life</h3>
      Layers, groups, blend modes and cel opacity. Preview frames, name animation ranges, loop playback and paint with tiles.
    </td>
    <td width="33%" valign="top">
      <h3>Keep your work editable</h3>
      Save native .pixel projects. Exchange supported ASE sprites, export PNG sheets and animated GIFs, and extend the editor with bundled Lua.
    </td>
  </tr>
</table>

PixelCraft runs independently of Aseprite. Its executable includes a Lua 5.4 runtime and uses its own preferences, extensions and project storage. Aseprite's executable, configuration and assets are not needed.

## Get started

### Windows installer (future releases)

Future tagged releases will include `PixelCraft-<version>-x86_64-Setup.exe` beside the Linux AppImage, with a SHA-256 checksum. On Windows 10/11 x64, open the installer, choose the install location and optional desktop shortcut, then launch PixelCraft from the Start menu. Installation is per user and requires no administrator access. Install newer versions over the existing installation; remove PixelCraft through Windows Settings → Apps. Uninstall keeps your projects, preferences and extensions in your user profile. Working OpenGL drivers are required. Installers are currently unsigned, so Windows may show a publisher warning.

### Linux AppImage

Download the `.AppImage` from this repository's GitHub **Releases**, make it executable in your file manager, and open it. From a terminal:

```bash
chmod +x PixelCraft-*-x86_64.AppImage
./PixelCraft-v0.1.0-x86_64.AppImage
```

Use the filename you downloaded. Releases target Linux x86-64 with glibc 2.35 or newer (Ubuntu 22.04+), a graphical desktop and working OpenGL drivers. On Wayland, the app uses your system's Wayland libraries alongside its graphics drivers; these libraries are deliberately excluded from the AppImage. File dialogs use the desktop's XDG portal. If FUSE is unavailable, run the AppImage with `--appimage-extract-and-run` before any app arguments. A SHA-256 checksum accompanies each release.

Maintainers: publish the contents of this `app/` directory as the repository root, including `.github/`. Push a version tag such as `v0.1.0` to run `.github/workflows/release.yml`. **Actions → Desktop releases → Run workflow** creates downloadable artifacts without publishing a release. Tagged runs publish both platforms and their checksums together after both jobs pass. The version tag controls artifact filenames; update `Cargo.toml` and `Cargo.lock` before tagging a new app version.

The Linux job builds on Ubuntu 22.04, embeds that runner's Lua 5.4 library, packages using [linuxdeploy](https://docs.appimage.org/packaging-guide/from-source/native-binaries.html), and checks packaged save/export/Lua functionality. The Windows job uses Windows Server 2022, stable MSVC Rust and Inno Setup 6. It compiles a checksum-verified [official Lua 5.4.9 source archive](https://www.lua.org/ftp/) into an embedded x64 DLL with a static C runtime, builds the WebP decoder DLLs and their dependencies from checksum-verified libwebp 1.6.0 source with a static C runtime, runs the Rust tests, and builds a per-user installer including those DLLs and their notices. Before uploading, it silently installs to a path containing spaces, exercises animated WebP import, saving, exports and Lua plugins from the installed executable with build-tool directories removed from PATH, repeats installation for the upgrade path, checks uninstall, and writes the installer checksum. Windows release verification runs on the Windows runner; local Linux checks cannot execute that installer. No Windows installer is produced until the workflow runs.

Before uploading a release, the workflow also opens the packaged AppImage on an Xvfb X11 display and a headless Weston Wayland display using Mesa software rendering. Each check must exit successfully and produce a nonblank editor screenshot within 60 seconds. Wayland runs without `DISPLAY` so it cannot fall back to X11. Screenshots and startup/compositor logs are saved in the `PixelCraft-startup-diagnostics` Actions artifact, including on failure. Packaging fails if any `libwayland-*.so*` files or symlinks enter the AppDir. These checks validate startup on the Ubuntu release runner; they do not cover every Linux distribution or graphics driver.

### Build from source

Build from the repository root with a Rust toolchain supporting edition 2024 and Cargo. Linux graphics and window-system libraries are required; a graphical desktop session is needed to open the editor. For Windows x64, use an x64 MSVC developer PowerShell with stable Rust and run `./packaging/build-windows-lua.ps1` and `./packaging/build-windows-webp.ps1` first (requires network access to download verified Lua source). The generated `runtime/lua54.dll` is embedded at compile time. The WebP script requires CMake and stages `libwebpdemux.dll`, `libwebp.dll` and `libsharpyuv.dll` beside the debug/release executables and test binaries; keep these DLLs beside the executable when distributing a source build. The Linux Lua runtime is checked in.

```bash
cargo build --release
cargo run --release
```

The first build downloads dependencies. After Cargo has cached them, add `--offline` to build or run without network access. The release executable is `target/release/pixelcraft` on Linux or `target/release/pixelcraft.exe` on Windows.

The editor opens with an editable garden sample. Choose **File → New sprite** to start a transparent canvas, or open an existing project. Save your editable work as **.pixel**; use PNG, sprite-sheet or GIF export for sharing.

## Recovery and expanded workflows

Open tabs and unsaved documents are backed up locally every 15 seconds and at a confirmed exit, using a compressed, atomically replaced workspace file in PixelCraft's preferences directory. The previous backup is retained as a fallback. On startup the editor restores tabs, active tab, saved baselines, drawing colors, selections and canvas views. Autosave does not overwrite project files; Ctrl+S still saves your project. A crash can lose changes since the last backup. Screenshot/check runs do not read or write your session.

**Edit → Drawing & selection studio** contains symmetry on either axis, a one-pixel pixel-perfect pencil, pressure-controlled brush size, numeric selection movement and nearest-neighbor resizing, 90° rotation, flips, crop, shading ramps and basic invert/grayscale/lighten/darken effects. Lasso **Q**, move **V** and gradient **D** also appear in the toolbar. Drag a gradient from the active color to the alternate color; both colors are editable in the studio. Pressure is applied when the window-system/tablet backend supplies force events; mouse strokes retain full brush size. Selection transforms and effects are undoable. Crop affects all image layers and frames; tile layers must be rasterized before cropping.

The **Colors** panel now supports renaming and deleting entries, and saving/loading named JSON palettes. Drawing colors and names are also preserved by session recovery. Shading ramps add named colors between the active and alternate colors.

**File → Export sprite sheet** opens a packing dialog: choose columns, transparent padding, border trimming, a named animation range, and an engine preset. Exports include PNG and JSON frame rectangles, original canvas sizes, trim offsets, millisecond durations and tags. Compact packing uses rows of trimmed frames without rotating them. Open the exported JSON to reconstruct full animation canvases, or use **File → Import PNG sequence** for naturally ordered numbered images of equal dimensions.

The **Godot** preset adds a Godot 4 SpriteFrames `.tres` with AtlasTexture frames, source margins, tag directions and per-frame timing. Put the PNG and resource together at the Godot project's `res://` root (adjust the texture path when placing them elsewhere). The **Unity** preset adds a multiple-sprite `.png.meta` importer with point filtering and stable sprite IDs; re-export retains an existing asset GUID. Copy the PNG and `.meta` together into Assets. Timing and tags remain in the JSON sidecar for Unity animation tooling. Generated files have automated structural coverage; a live Godot/Unity editor import has not been validated here.

**Sprite → Tileset browser** shows clickable tile thumbnails and selects the corresponding tile layer for painting. **View → Seamless preview** repeats the active frame in a 3×3 grid. Open finite orthogonal Tiled `.tmj` maps or `.tsj` image tilesets, and export the active frame through **File → Export Tiled map**; the export writes the map, external tilesets and PNG atlases with tile flips. Tiled image collections, infinite maps, object/group layers and compressed tile arrays are rejected explicitly. JSON array tile layers, tilesheet margins/spacing, layer offsets and opacity are supported.

Open animated GIF, APNG (including animated `.png`), WebP and layered OpenRaster `.ora` through **File → Open**. Animation timing and composited frame canvases are retained. OpenRaster keeps layers, groups, visibility, opacity, offsets and common source-over/multiply/screen/overlay/darken/lighten modes; other blend modes return an error. Its ZIP reader has a 32 MiB extraction budget. WebP uses `libwebpdemux` (libwebp) from the system; Linux AppImage packaging includes it. Other builds require a compatible runtime (`libwebpdemux.so.2`, `.dylib`, or `.dll`). No external image conversion program is invoked.

## Editing

New Sprite, Rename Layer, save confirmations, file errors, and **Help → Tools and file support** use a shared dialog. Drag the header to move it, or use the highlighted **×**, Escape, or the dimmed area around it to dismiss it. Dismissing a save confirmation keeps your edits open.

Pencil B, eraser E, fill G, eyedropper I, line L, rectangle U, ellipse O, selection M, hand H. The compact **Colors** selector starts with one white entry, **Primary**. **Add Color** creates and selects another named entry; click its color chip to edit it. Selecting an entry makes it the active drawing color for either mouse button. **Dominant colors** groups similar nontransparent artwork shades across layers and frames into up to 10 representative swatches, ranked by frequency. Results are cached until artwork changes. Click a swatch to select the drawing color while keeping saved named colors unchanged. Drawing colors are shared across sprite tabs. Alt-click samples into the active entry; X swaps with the alternate color. Scroll zooms around the pointer; middle-drag pans. Ctrl+Z undoes; Ctrl+Y redoes; Ctrl+S saves. Copy/cut/paste/delete work on image-layer selections. A stroke is one undo step.

Use **Sprite → Properties, layers, tiles & ranges** (also available beside the timeline) for:

- RGB, grayscale with alpha, and indexed modes with editable 1–256-color palettes. Drawing is constrained to the selected mode; indexed palette slots retain their identity even when colors match. Use the active color chip to choose a drawing color, or select a used artwork swatch to reuse it. Imported animated palettes are retained.
- Groups and nested group membership, all 19 ASE blend modes, separate layer/cel opacity and cel stacking order. Parent visibility and locks apply to children. Groups composite separately. Layer removal and reordering retain membership.
- Importing PNG tilesheets using a chosen tile size, capturing a selection as a tile, and creating tile layers. Select a tile ID in Properties and use Pencil/Eraser to place/remove whole tiles. Tile references, tilesets and flip flags are preserved.
- Named frame tags/ranges, editable endpoints and forward/reverse/ping-pong playback. Frame insertion/deletion adjusts ranges. Select **Use for playback** to loop one range. The timeline shows frame previews; previews are generated for the first 256 frames to limit memory.

The context bar offers round, square, diamond, horizontal and vertical brushes. Ellipses use a symmetric integer rasterizer, including small and degenerate bounds. Pixel-exact agreement with every original editor brush mask is not claimed.

Canvases support up to 16384 pixels per side and 64 million pixels per canvas and 128 million stored pixels across layers and frames. Dimensions and project structures are validated. Cel pixels and indexed buffers use copy-on-write sharing across frame copies and undo snapshots. Undo retains up to 64 steps with a 192 MB allocation budget; a single oversized step is retained. Unchanged frame textures are cached and the checkerboard is clipped to the viewport.

## Projects and interchange

**`.pixel` is the default editable project format**, a versioned, compressed native document containing all editor properties, tiles, ranges, metadata, profiles and off-canvas image pixels. Saving uses a temporary file and replacement. New/open/close protect unsaved changes.

`.ase` and `.aseprite` are optional, native import/export formats. RGB, grayscale and indexed cels, raw/zlib/linked cels, groups, blend modes, cel opacity/order, embedded tilesets, tilemaps with per-cel geometry, palettes, tags, layer flags/UUIDs and opaque metadata chunks are supported. Unknown chunks retain their bytes and object association. ASE layers are serialized in hierarchy order. Image artwork beyond the canvas is retained and written back; normal canvas exports clip it.

PNG import/export retains text, gamma, chromaticities, profiles, EXIF, pixel density and timestamps. Export an active-frame PNG, horizontal PNG sprite sheet or animated GIF. GIF has binary transparency and 10 ms timing precision. ICC profiles are retained losslessly, and RGB/grayscale profiles and fixed-gamma sRGB are converted to sRGB for display with moxcms. Source pixels are unchanged.

External tilesets resolve relative to the project and are embedded on save. Sparse tileset IDs, off-grid/off-canvas tilemap geometry, linked tilemaps, compact 8/16-bit entries and legacy empty-tile encodings are supported. Missing references and unsupported future encodings return explicit errors. Arbitrary future metadata remains opaque; the editor does not interpret every external metadata schema. Prefer `.pixel` for preserving PixelCraft's full editable state.

The ASE implementation follows the [published format specification](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md); no original program is required.

## Independent extensions and storage

**Extensions → Extensions & scripts** runs Lua scripts and installs `.lua`, `.zip`, `.aseprite-extension`, or folders containing `package.json`. Manifest contributions include `scripts`, `themes`, `palettes`, `keys`/`shortcuts`, `languages`, and `ditheringMatrices`, each with a `path`. Original theme directory paths beginning with `./` are accepted. Installed packages can be enabled, disabled and uninstalled here; registered plugin commands run in this interface. ZIP extraction supports stored/deflated entries, checks CRC and size, and rejects path traversal, symlinks and encryption. Installation is staged; existing packages are not overwritten.

`examples/pixelcraft-starter` is an original installable package demonstrating a Lua command, JSON theme, palette and shortcuts. It is also embedded into the standalone verification mode.

- Data and projects: `$XDG_DATA_HOME/pixelcraft/{extensions,projects}`, normally `~/.local/share/pixelcraft`.
- Preferences: `$XDG_CONFIG_HOME/pixelcraft`, normally `~/.config/pixelcraft`.
- `PIXELCRAFT_HOME` optionally sets an independent portable root, with preferences beneath `preferences/`.
- AppData is used when present as a fallback. There is no migration or lookup of another editor's settings.

New installs follow the system appearance. Choose **View → Theme → System, Light, or Dark** to change and remember the appearance; existing Light and Dark choices are preserved.
On Linux, System mode reads the desktop appearance portal and listens for changes while the editor is open (on both X11 and Wayland). If the desktop provides no appearance preference, System mode uses Light.

Themes accept original XML color definitions or JSON with `dark`, `panel`, `text`, `accent` (RGB arrays). Palettes are JSON arrays of RGBA bytes, GIMP `.gpl` palettes, or ASE palettes. Shortcuts accept original `.aseprite-keys`/XML files for supported commands and tools, or JSON maps such as `{"P":"pencil","Ctrl+S":"save"}`; supported actions include tools, file/edit commands and playback. Applied theme/shortcut settings are copied into PixelCraft's preferences and remembered.

Lua runs transactionally on a separate document copy; successful results import as one undoable change. Errors, unsupported APIs and resource limits leave the original document unchanged. Lua has a 256 MiB allocation budget, an instruction/CPU-time budget, and a one-million-stored-pixel invocation limit. Interactive editor sessions support dialogs, events, timers, preferences, file operations and local modules. Process execution and Lua io remain unavailable. Original language INI and BMP/PNG dithering contributions are supported; themes adapt XML colors to native widgets. See [the compatibility list](COMPATIBILITY.md) for the tested API and remaining limitations.

## Supported native Lua API

- `app.activeSprite`, `activeLayer`, `activeFrame`, `activeCel`, `activeImage`, `sprites`, `params`, `apiVersion`, `isUIAvailable`, `transaction(fn)`, `refresh()`, `alert()`.
- Sprite width/height/layers/frames/cels/palettes/colorMode, `newCel`, `deleteCel`, `newLayer`, `newGroup`, `newFrame`, `newEmptyFrame`, `newTag`.
- Layer name/opacity/visibility/editability/blendMode/parent/group/cels and `cel(frame)`; frame number/duration; cel image/opacity/frame/layer. Frame API durations are seconds.
- `Image(width,height)` or `Image(image)`, `drawPixel`, `getPixel`, `clear`, `clone`, `drawImage` (source-over), and `pixels()` iterators.
- `Color`, `Point`, `Size`, `Rectangle`, `ColorMode`, `BlendMode`, `AniDir`, packed RGBA functions under `app.pixelColor`.
- Palette `getColor`, `setColor`, `resize`; tag name/fromFrame/toFrame/aniDir. Pixel coordinates and palette indices are zero-based; frame/layer Lua handles are one-based.
- `init(plugin)`, `plugin:newCommand{id,title,onclick}`, menu grouping/separator declarations. Supported command calls: `NewLayer`, `NewFrame`, `NewEmptyFrame`. Additional selection, undo, file and interactive APIs are listed in [COMPATIBILITY.md](COMPATIBILITY.md). Unlisted commands return errors.

## Verification

`cargo test --offline` covers model behavior, real pointer editing, undo, rendering caches, palettes, groups, tilemaps, tags, file round trips, PNG metadata, embedded Lua, package lifecycle and error rollback. `cargo build --offline` builds the app; Craft Anything's `rebuild_check` builds and captures its window.

`target/debug/pixelcraft --standalone-check /tmp/workshop-check` exercises drawing, project save/open, PNG/sheet/GIF exports, embedded Lua plugin installation/execution, theme/palette/shortcut application, disable and uninstall without opening a window. `checks/standalone.sh` runs it in an otherwise empty Bubblewrap filesystem containing only the binary, basic Linux libraries and a writable output directory—no original editor, system Lua interpreter, home configuration or assets are mounted.

## Status

PixelCraft is an early-alpha desktop app with Linux AppImage and Windows installer release workflows. Drawing, animation, advanced document features, native project persistence and independent Lua extensions have automated coverage. The standalone check also exercises saving, exports and plugins in an isolated filesystem without Aseprite or an installed Lua interpreter.

Compatibility is partial and explicitly documented in [COMPATIBILITY.md](COMPATIBILITY.md). Ten real Aseprite projects (33 frames) have reference-image and metadata round-trip coverage, and four unchanged upstream scripts have automated coverage. Exact original widget skins, every API/command, multi-document Lua objects and monitor-specific ICC calibration are outside the supported surface. Other operating systems and architectures have not been validated.

## License and credits

PixelCraft is licensed under the [MIT License](LICENSE).

**Made via [CraftAnything](https://github.com/storytold/craft-anything) by [ArtCraft](https://getartcraft.com/).** Craft Anything provided the workflow used to study the reference application, build this independent editor and check the result.

PixelCraft was created using [Aseprite](https://github.com/aseprite/aseprite) as its basis and reference for pixel-art editing features, file compatibility and scripting behavior. Thank you to the Aseprite developers and contributors.

The bundled Lua runtime retains its [MIT license and attribution](runtime/LUA-LICENSE.txt). Rust dependencies retain their respective licenses; exact versions are recorded in [Cargo.lock](Cargo.lock). ASE interoperability follows the published file specification linked above. The sample garden artwork is original to this project.

<p align="center">
  <a href="https://github.com/storytold/craft-anything">Made with CraftAnything</a> ·
  <a href="https://getartcraft.com/">ArtCraft</a> ·
  <a href="https://getartcraft.com/apps">Explore the Crafting Apps</a>
</p>

## About and updates

**Help → About** shows the developer, installed version (from Cargo), latest stable GitHub release, and a download link when an update is available. The GitHub icon beside PixelCraft opens the project repository. A centered **New version found** link appears when a newer release is confirmed. Checks run in the background at startup, hourly while the editor is open, and through **Check for updates**; failed checks never announce an update.

The developer credit defaults to Asad, the author recorded in `Cargo.toml`. Tagged GitHub Actions builds automatically embed their repository and keep this developer credit. For local builds, set `PIXELCRAFT_REPOSITORY=owner/repository` and optionally `PIXELCRAFT_DEVELOPER` when building. The About dialog includes a read-only Developer and repository section, without editable fields or a save button. Previously saved developer and repository values in `about.json` are ignored; the build identity always supplies these fields. No repository URL was present in the source project, so local builds require the build-time repository setting before GitHub links and release checks can work.

The GitHub mark in `assets/github-mark.svg` is from [GitHub Octicons](https://github.com/primer/octicons), licensed under the MIT License; the PNG is a rendered copy.

## Workflow tools and automation

Open **Tools…** beside the brush controls, or **Edit → Brushes, references & workflow tools**.

- **Magic Wand (W)** selects matching pixels on the active layer. Choose contiguous or whole-layer matching, tolerance (0–255 per RGBA channel), and Replace/Add/Subtract. Shift-click adds; Ctrl/Cmd-click subtracts. Transparent pixels match regardless of hidden RGB. Selection masks work with painting, copy/cut/delete, shapes and transforms.
- **Shade** steps each painted pixel once per stroke through the document palette; enable the reverse option to step backward. **Dither** alternates the active and alternate colors on a checkerboard anchored to canvas coordinates. **Selection** stamps captured selection pixels, including partial alpha and transparent pixels that erase their destination. Capture deselects the source so the stamp can be painted elsewhere.
- **Wrap drawing & fill** makes the canvas a torus. Strokes can extend past its edges, and flood fill connects matching pixels across opposite edges. This setting is independent of the existing 3×3 seamless preview.
- **Reference images** imports PNG/JPEG overlays. Set X/Y, scale, visibility and opacity, or enable Move/scale and drag on canvas (Shift-drag scales). Finish moving to draw again. Images and placement are embedded in `.pixel` projects and recovery; artwork PNG, GIF, sprite sheets and atlases exclude them. Save `.pixel` to preserve editor-only reference data.
- **Palette tools** optionally snap RGB pencil, fill and shapes to the document palette. Import/export `.hex`, JASC `.pal`, GIMP `.gpl` and JSON. HEX accepts RRGGBB or RRGGBBAA; PAL/GPL contain RGB only. The existing named drawing-color JSON palette remains available in Colors.
- **Onion skin** supports 0–8 previous and 0–8 next frames, each side with its own tint and opacity. More distant frames fade by distance. Settings are remembered.
- **Grid sprite-sheet import** previews complete cells in row-major order with independent cell size, offset and spacing. Import into a new document as frames or layers; incomplete edge cells are omitted.
- **Batch tag export** writes one GIF or PNG/JSON sheet per animation tag. GIFs contain the requested forward, reverse or ping-pong playback order and original durations (subject to GIF's 10 ms precision). Sheets retain direction metadata and reuse the current columns/padding/trim and Generic/Godot/Unity options. Indexed filename prefixes avoid name collisions and unsafe tag names.
- **Multi-file atlas** packs every frame of selected sprites with Rows or MaxRects, optional trimming, padding, exact pixel deduplication and power-of-two dimensions. JSON keeps every frame's timing, source identity, original canvas size, trim offsets and source tags even when frames share one packed region. The width is an upper bound; power-of-two mode uses a width no larger than that bound. Existing image/pixel limits apply.
- **File → Recent files** remembers the last 15 successfully opened/saved files. Workflow tools also provide PixelCraft, Aseprite and Photoshop shortcut presets for supported commands and an editable binding list. Choosing a preset replaces the current bindings; edits and removals are saved.

### Headless CLI

No graphical display is needed. `pixelcraft --help` lists the operations and arguments. Run `--cli OPERATION` with a JSON argument object, or named flags; command errors go to stderr and exit with a nonzero status. Layer/frame indices are zero-based.

```sh
pixelcraft --cli inspect --input hero.pixel
pixelcraft --cli export --input hero.pixel --output hero.gif
pixelcraft --cli export --input hero.pixel --output hero.png --format sheet --options '{"preset":"Godot","columns":8,"trim":true}'
pixelcraft --cli import-grid --input sheet.png --output animation.pixel --options '{"width":16,"height":16,"offset_x":1,"spacing_x":2}'
pixelcraft --cli import-sequence --inputs '["walk1.png","walk2.png"]' --output walk.pixel
pixelcraft --cli batch-tags --input hero.pixel --output exports --format gif
pixelcraft --cli atlas --inputs '["hero.pixel","enemy.aseprite"]' --output atlas.png --options '{"layout":"MaxRects","width":1024,"padding":2,"trim":true,"deduplicate":true,"power_of_two":true}'
pixelcraft --cli palette-export --input hero.pixel --output colors.hex
pixelcraft --cli palette-import --input hero.pixel --palette colors.pal --output recolored.pixel
pixelcraft --cli palette-convert --input colors.hex --output colors.gpl
```

`import`/`export` convert formats using the same loaders and exporters as the editor. `render` writes a PNG when output is supplied, or returns an inline PNG otherwise. `edit` accepts `input`, `output`, the `revision` returned by inspect, and `edits: [{layer,frame,x,y,color:[r,g,b,a]}]`. Edits validate completely before saving; locked layers, invalid coordinates and stale revisions fail without changing the input.

### MCP and live editor access

Configure an MCP client to launch the executable with `--mcp`. The newline-delimited stdio JSON-RPC server advertises file inspection, PNG rendering, revision-checked edits, import/export, palettes, tag batches and atlases. Rendering returns an MCP image. File tools operate on explicitly supplied local paths and do not require the editor to be open.

Live access starts **disabled** and is never restored from preferences. Enable **Workflow tools → Assistants & headless tools → Allow assistants to access this live editor**. The editor displays an MCP command with `--live BRIDGE_CONFIG`. That instance adds `live-inspect`, `live-render`, `live-edit` and `live-undo`. The bridge listens on loopback with a random token in a private file, which is removed when access is disabled or the editor closes. Disable the checkbox to revoke access.

Live inspection returns a session/document revision. Supply that exact revision to edit or undo; user edits, undo/redo and document changes invalidate it. Assistant changes are one regular undo step. Active gestures and scripts block edits, and expired requests are discarded rather than applied after a stalled editor resumes. Reference overlays are excluded from assistant artwork rendering too.

#### Live pixel painting

Use `live-save` with the revision from `live-inspect` to write the active sprite to its current project path. Use `live-save-as` with `revision` and a new `output` path to name an untitled sprite or save a new copy. The editor adopts the new path after the file is saved. File MCP also supports `save` and `save-as` with an explicit `input` and its file revision.

In the **File** menu, choose **Export animation GIF…** or **Export video MP4…** and select a scale. Both use integer nearest-neighbor scaling. The same options are available through `live-export` with `revision`, `output`, and optional `scale`. The file MCP `export` tool takes `input` instead of a live revision. The extension selects PNG, GIF, or MP4 for live export. For example, a 480×270 sprite exported with `scale:4` produces 1920×1080.

For individual PNG frames, use **File → Export PNG sequence…**, choose an integer scale, and name a new folder under the destination. PixelCraft writes `frame-0001.png`, `frame-0002.png`, and so on in timeline order. PNGs retain transparency. Use `live-export` with `{"revision":"…","format":"png-sequence","output":"C:/path/new-folder","scale":4}`, or use file MCP `export` with `input` in place of `revision`. `output` must be a new directory under an existing parent. Pass `layers` to filter PNG, PNG sequence, GIF, or MP4 exports. PNG sequences also accept a `region` crop.


GIF keeps the timeline delays at GIF's centisecond precision. MP4 writes H.264 at 60 fps, rounds frame boundaries to video ticks, composites transparency onto black, and pads odd dimensions by one pixel for compatibility. Install FFmpeg with `libx264` and make it available on the editor or MCP process's PATH for video export.

Use `live-inspect` to get the active sprite revision and zero-based layer indices. Send an ordered `live-commands` batch with that revision. Batches are atomic and are visible in the editor as each batch commits. Use the revision returned by the batch for the next edit or preview. `changed_cels` describes the bounding rectangles of changed RGBA pixels in the resulting cel slots. Layer or frame structure and property changes are reflected in the returned sprite metadata.

Bulk paint targets an unlocked RGBA image layer and an explicit zero-based frame:

| Command | Arguments and behavior |
| --- | --- |
| `blit` | `x,y,width,height,data`. Data is straight RGBA8, row-major, encoded with `base64` (default) or `base64-zlib`. Destination clips to the canvas. `mode` is `source-over` (default), `replace` (transparent pixels clear), `multiply`, or `screen`. Patch dimensions are 1–1024 pixels on each axis. |
| `stroke` | `points:[[x,y],...]`, `color:[r,g,b,a]`, optional `size`, `end_size` for a taper, `brush_shape`, and integer `spacing` in raster steps. Set `placements:true` for explicit dab positions. Add `scatter`, `seed`, and `ramp` for reproducible palette variation. Use `alternate` for checker dithering, `shade:true` for palette stepping, or a `stamp:{width,height,data,encoding}` bitmap. Brush dabs default to `replace`, bitmap stamps default to `source-over`, and both accept `mode` and `opacity` (0–255). Supply `pressure:[0,...,255]` with one value per point to control a non-bitmap brush’s size, interpolated along the path. `mask:{x,y,width,height,data}` clips dabs to a packed selection. With no `points`, the brush places dabs at selected grid positions using `spacing` on both axes. |
| `polygon` | `points` and `color`, with optional `alternate` for checker dithering. Add `dither_level` (0–16) for a canvas-aligned 4×4 ordered pattern. Uses the editor's pixel-center fill rule. Add `edge_noise` (0–16 pixels) and `seed` to break up the inner edge reproducibly. |
| `clear-region` | `x,y,width,height`. Clears the clipped rectangle. Supply `mask` and/or `skip_transparent:true` to restrict clearing. |
| `copy-region`, `move-region` | Source `x,y,width,height` and destination `to_x,to_y`. Use `from_layer` and `from_frame` for a different source cel, and optionally `flip_x`, `flip_y`, `skip_transparent`, `opacity` (0–255 multiplier), or exact RGBA `colors:[[from,to],...]` mappings. The destination is clipped. A move clears the source rectangle before writing the saved source pixels. |
| `recolor-region` | `x,y,width,height`, `colors`, and optional `opacity` and `skip_transparent`. |

Bitmap stamps accept integer `scale` (1–16, up to 256×256 after scaling). Clear, copy, move, and recolor accept an optional base64 `mask` packed in row-major order with bit 0 first in each byte. Stroke masks use the same encoding in their `data` field. Each live request is limited to a 4 MB bridge message. Bulk operations also share a decoded work limit, so split large passes into a few meaningful batches.

Use `live-render` with `revision`, `frame`, optional `layers:[...]`, `region:{x,y,width,height}`, and integer `scale` to inspect the exact committed snapshot. It returns a PNG and render metadata. For a 480×270 canvas, call `live-export` with that revision, a new PNG `output` path, and `scale:4` for a 1920×1080 export. The export uses nearest-neighbor scaling. `live-undo` and `live-redo` accept the current revision and use the regular editor history.

`cargo test --offline` covers shared operations, masks, stamp transparency, shade replay, tag timing/direction, atlas reconstruction, revision guards, undo and expiry. `python3 checks/productivity-headless.py` checks the executable CLI and MCP with display variables removed. The separate loopback authentication test is ignored in the restricted build sandbox; run `cargo test --test productivity live_bridge_authentication -- --ignored` in a normal desktop environment to exercise the socket transport.


## Clipboard, display and export controls

Copy/Cut/Paste now exchanges RGBA images with the system clipboard, retaining transparent and partially transparent pixels. Linux Wayland uses `wl-copy`/`wl-paste` when installed (the `wl-clipboard` package), with XWayland clipboard fallback; Windows, macOS and X11 use native image clipboard access. Undo and Redo restore the selection rectangle, mask, layer and frame together with artwork.

In **Workflow tools**, **Export scaling** offers 1×, 2×, 4× or a custom positive integer. The same scale applies to PNG, BMP and sprite-sheet exports; sheet coordinates and source sizes scale with the PNG. **File → Export BMP** exports the active frame. Folders can be added to atlas inputs (including supported sprites in subfolders), and dropping files opens them as document tabs.

**Drawing & selection studio** adds Outline and Drop shadow using the active color, clipped to the active rectangle/mask. Shadow uses the studio's X/Y movement offsets. Reference controls add behind-art placement, fit-to-canvas, floating draggable previews with click-to-sample color, and copy-to-layer. Copying rasterizes the current reference placement and opacity as one Undo step. Enable **Sample reference colors** to include references in eyedropper sampling; transparency and behind-art placement are respected.

**Interface density & panels.** Compact is the default, with 14-point body/button text, 16-point section headings, 28-point controls, a 52-point tool rail and 30-point swatches. Dimensions are logical display units: 100% interface zoom follows the operating system's display scale. Choose **View → Density → Compact / Comfortable** (also in **Workflow tools → Interface & autosave**) to change spacing without changing text size. Your density and explicit interface zoom are remembered.

Drag the inspector's right edge or the timeline's top edge to resize them; their initial sizes are 264 and 200 logical points. The inspector header’s collapse button and **View → Show / Hide inspector** collapse and restore it. When hidden, a small restore button remains at the left edge. Windows and Linux use native window decorations; the operating system handles dragging, resizing, window controls and title-bar double-click behavior. Native close requests retain the save/discard/cancel confirmation for every unsaved tab. The inspector keeps its left dock on small windows; its preview stays pinned at the bottom while colors and properties scroll above it. Secondary drawing settings move into **Drawing…** when needed. Narrow timelines keep frame/layer creation and duration settings in **Layer & frame actions**. Tool and timeline contents scroll when they exceed the available space; the canvas fills the remaining area.

Custom switches, inspector section headings, and artwork swatches show hover and pressed feedback and a visible keyboard-focus outline. Artwork swatches announce their hex color and selected state to screen readers. Translucent swatches include alpha in the hex value. In the Edit menu, Undo and Redo are available when the active tab has the corresponding history.

**Palette order & ramps** moves swatches by zero-based position and adds intermediate shades between existing palette endpoints. Indexed swatch moves remap indices, including the transparent slot, so existing artwork keeps its colors. Ramps append shades without changing existing slots. Palette changes can be undone; documents with differently sized frame palettes report an error instead of altering artwork. **Interface & autosave** remembers interface scale, checker size/colors and recovery autosave on/off.

The app starts with wgpu and automatically retries OpenGL if graphics initialization fails. Set `PIXELCRAFT_OPENGL=1` to select OpenGL explicitly. Fallback never restarts an already running editor after an application error.

**Help → About** offers optional in-app update downloads for published x86-64 Windows/Linux assets. Downloads must include a matching SHA-256 sidecar and belong to the configured GitHub repository. Installation rechecks the bytes and requires confirmation. Windows starts the installer; Linux replaces the chosen AppImage atomically, retaining a `.previous` backup. Reopen the app after installation. This source build needs a build-time repository identity for release discovery; update networking/installation on other desktops has not been exercised locally.

Sprite-sheet JSON now retains tag repeat counts. Godot animations use unique names; repeat 0 loops indefinitely, while positive counts export a finite sequence with looping disabled. The exporter rejects finite sequences larger than one million entries.

### Assistant document commands

File tools add `create` (`width`, `height`, `output`, which must not already exist) and `commands` (`input`, `output`, `revision`, `commands`). Live tools add `live-create` and `live-commands`, both requiring the current live revision; they are one regular Undo step. `live-create` opens a separate untitled sprite, preserves the original tab, and supports Undo. A command batch validates transactionally and leaves the source unchanged on any error.

Command `op` values: `add-layer`, `set-layer`, `move-layer`, `remove-layer`, `add-frame`, `set-frame`, `remove-frame`, `add-tag`, `set-tag`, `remove-tag`, and `shape`. Layer commands accept zero-based `layer`; `set-layer` accepts `name`, `visible`, `locked`, `opacity`; move uses `to`. Add-frame inserts after `frame`, optionally `duplicate`; set-frame uses `duration` in milliseconds. Tag creation/update accepts `tag: {name,from,to,direction,repeat,color}` (direction 0–3, repeat 0 infinite, color RGB); update/remove uses `index`. Shape accepts `layer`, `frame`, `kind` (`line`, `rectangle`, `ellipse`), canvas coordinates `x,y,x2,y2`, RGBA `color` and optional `filled`. File raster export accepts `scale`; sheet export accepts `options.scale`.

Preview stays docked at the bottom-left independently of the inspector scroll area. Drag its top divider to resize it vertically. Choose Fit to show the whole sprite, or 1:1, 2×, 3×, 4× and 8× for sharp nearest-neighbor zoom measured in display pixels; drag overflowing artwork to pan. Preview height and zoom are remembered across restarts, and the image follows the active sprite and animation frame.

The bottom-left preview uses the inspector width and has a draggable top divider. Its zoom menu offers Fit, 1:1, 2×, 3×, 4× and 8×. Fit shows the entire sprite; fixed zoom preserves physical pixel sizes, with drag-to-pan for oversized artwork. Preview height and zoom are remembered separately from the main canvas.
