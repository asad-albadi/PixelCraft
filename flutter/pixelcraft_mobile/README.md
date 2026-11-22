# PixelCraft for Android — test preview

Tablet-first Flutter interface using the existing Rust core through a bounded C
ABI and one serialized worker. Phones use compact sheets and the same engine.
Desktop, CLI and MCP retain their existing entry points. Android does not host
or invoke the CLI/MCP server. A Pixel 10 Pro XL (Android 17) and Samsung SM-X716B
tablet (Android 16) each passed all eight integration workflows. The user confirmed
the tablet's physical S Pen pressure, palm rejection, button erasing and navigation
checks.

## Build and install

Linux build host: Flutter 3.47.9/Dart 3.13.7, Java 17, Android SDK/API 36 build
tools, NDK 28.2.13676358, Rust 1.95.0. Minimum supported Android API: 24.

```sh
rustup target add --toolchain 1.95.0 x86_64-linux-android aarch64-linux-android
cd flutter/pixelcraft_mobile
flutter pub get --enforce-lockfile
flutter build apk --debug
adb install -r build/app/outputs/flutter-apk/app-debug.apk
```

Gradle builds arm64-v8a and x86_64 engines. Set `ANDROID_HOME`/`ANDROID_SDK_ROOT`
and optionally `ANDROID_NDK_HOME` for a nonstandard SDK location. The build helper
currently requires Linux. Local SDK paths, keys, generated libraries and APKs are
excluded from Git. The test identity is `io.github.asad_albadi.pixelcraft.preview`.

The Android test workflow builds and checks a universal **debug test APK**, not a
production release. Local updates signed with the same debug key preserve private
saves; CI debug keys can differ across runs, so CI APKs are not a guaranteed
upgrade channel. Export projects before any uninstall; uninstall removes private
saves. Production signing/store distribution needs a separate decision.

## Controls and mobile feature matrix

| Area | Available workflows |
| --- | --- |
| Canvas | Crisp nearest-neighbor pixels, pinch zoom/two-finger pan, Fit, grid |
| Drawing | Pencil, eraser, fill, line, rectangle, ellipse; brush sizes/shapes, filled shapes |
| Colors | Named colors, saturation/value picker, hue, opacity/checkerboard, RGB hex, eyedropper |
| Selection | Rectangle, select all, clipboard, move, clear, horizontal/vertical flip, 90° rotation, crop |
| Layers/cels | Visible layer rows, eye/lock, rename/opacity, reorder, groups, add/delete, linked/unlinked/cleared cels |
| Animation | Frames, names/durations, duplicate/delete, tags/directions/repeats, speed, onion skin; playback preserves editing target/history |
| Indexed | Shared RGB/grayscale/indexed conversions, supported palette indices retained in native saves |
| Tiles | Create populated tileset/layer from frame, choose tile, tap to stamp or erase cells; originals retained hidden |
| References | Up to four embedded PNG guides, position/scale/opacity/visibility, remove/convert to layer; retained in .pixel, excluded from artwork export |
| Zen | Hide panels/timeline, essential drawing strip, left/right placement, visible Exit, Back restores editor |
| Input | Touch; primary mouse drawing/middle pan/wheel zoom; stylus tip, optional pressure size, inverted tip/barrel eraser; palm touches ignored while pen owns stroke |
| Keyboard | B pencil, E eraser, G fill, I eyedropper, M selection; Ctrl+Z/Shift+Ctrl+Z/Ctrl+Y, Ctrl+S; Ctrl+Shift+F Zen; Escape cancels draft/exits Zen |
| Recovery | Private atomic autosave, background flush, Recover/Discard startup prompt, dirty baseline and active target restored |

Phone inspectors appear in sheets rather than squeezing the tablet side panel.
Colors/Preview remain on the left on wide layouts; Preview stays bottom-left.
Layer/frame tools are on the timeline; selection actions appear near the canvas.
Use File actions → Document tools for indexed modes, tilemaps and references.
Optional stylus pressure is in Brush options and is off by default for constant
pixel sizes. Synthetic/emulator input checks do not validate physical pens.

## Formats

| Format | Open | Export | Deliberate mobile limits |
| --- | --- | --- | --- |
| .pixel | Yes | Private Save / Save a copy | Supported artwork/indices/cels/metadata; mobile limits below; embedded references only |
| PNG / APNG | Yes | Active-frame static PNG | Supported indices/APNG frames retained on import; ancillary PNG metadata omitted |
| GIF | Yes | All-frame animation | Flattened RGBA on import; palette/alpha quantization and 10 ms timing; no editable layers/tags/names |
| .ase / .aseprite | Yes | Yes | Supported layers, indices, tags and timing; native frame names omitted by existing codec; no mobile tilemaps/tilesets/off-canvas cels |
| Sheet PNG + JSON | — | Two matching files | Save both suggested names; fixed cells/eight columns, bounds, durations, frame names and tags |
| Other desktop codecs/video | — | — | Not exposed; no dependency on desktop FFmpeg |

Choose an export folder to retain Android's granted access between runs; revoked
access is reported and requires choosing a folder again. Forget folder releases
this feature's grant. Sprite-sheet PNG/JSON saving uses one selected folder: save
PNG first, then JSON for the same artwork revision. The metadata uses the actual
provider PNG name, including duplicate-name suffixes; changed artwork requires
re-exporting PNG. Paired filenames must be portable ASCII basenames. Sharing the
two files individually requires recipients to retain their names.

Exports create copies and do not mark the editable project saved. Sharing reports
that the Android chooser opened, not that another app completed delivery. Share
URIs grant read-only access to a narrow private cache area, which is cleaned and
bounded. Android provider failures are reported; a failed provider write can
leave a partial new file. Save the editable `.pixel` project separately.

Private Save writes/syncs a temporary file before publication, then acknowledges
the native saved baseline only after success. Picker cancellation leaves artwork
intact. Imports prepare off-thread and replace the document only at an atomic
commit; Cancel import/Back while preparing discard the candidate and preserve
artwork/history. Provider reads use cancellation signals and a bounded worker
separate from private Save/recovery. Cancel completes its UI request immediately;
a provider ignoring cancellation can retain the single read slot until it exits,
so further opens report busy while private storage remains available. Native
inputs and returned provider URIs are validated.

## Memory and project limits

256 × 256 canvas; 32 layers; 64 frames; 16 MiB stored raster; 2 MiB native project
JSON after decompression; 8 MiB input/output; 32 Undo entries/32 MiB history target;
maximum two sessions. References: four at up to 256 × 256. Tilesets: eight, tile
dimensions 1–64, at most 256 tiles/source grid cells. Conservative interchange
preflights may reject valid desktop projects below nominal limits. Unsupported
or oversized imports preserve the current document. These are preview limits,
not desktop limits or claims of full desktop parity.

## Checks and evidence

```sh
flutter analyze
flutter test
flutter test integration_test/tablet_editor_test.dart -d emulator-5554
python3 ../../checks/android-ci-apk.py build/app/outputs/flutter-apk/app-debug.apk
```

See [progress](../../docs/android-progress.md),
[validation](../../docs/android-tablet-poc-validation.md), and
[UX research](../../docs/android-tablet-ux-research.md). A physical Pixel 10 Pro XL (Android 17) also passed all eight integration workflows.
Emulator/automated checks do not establish end-to-end finger latency, physical
tablet performance, stylus ergonomics or pressure hardware behavior. The tracked preview hardware gate has passed; this is not a production/store or full desktop-parity claim.
