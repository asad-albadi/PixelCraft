# PixelCraft tablet preview

Android proof of concept for issue #29. The Flutter interface calls the existing
Rust editing engine through `pixelcraft-ffi` on one serialized worker isolate.
Desktop, CLI and MCP keep their existing entry points. This is a preview, not a
complete Android port or a production store package.

## Try it

Linux build host: Flutter 3.47.9/Dart 3.13.7, Java 17, Android SDK, NDK
28.2.13676358, Rust 1.95.0 and the Android targets below. Minimum Android API 24.

```sh
rustup target add --toolchain 1.95.0 x86_64-linux-android aarch64-linux-android
cd flutter/pixelcraft_mobile
flutter pub get
flutter run -d emulator-5554
```

Gradle builds the Rust library for x86_64 and arm64-v8a before packaging. Set
`ANDROID_HOME` (or `ANDROID_SDK_ROOT`) and optionally `ANDROID_NDK_HOME` for a
nonstandard SDK location. The native build helper currently supports Linux hosts.
Generated libraries, local SDK paths, signing files and APKs stay out of Git.
The debug preview has its own app ID: `io.github.asad_albadi.pixelcraft.preview`.

## Included

- Shared-engine demo artwork or a new 64 × 64 sprite.
- Draw/erase, named preset colors, brush sizes, layer/frame selection.
- One Undo entry per completed stroke, Redo, draft cancellation on multitouch.
- Pinch zoom, two-finger pan, Fit and optional pixel grid.
- Colors/layers left, preview bottom-left, drawing tools right, frames below.
- Save to tablet-private storage; Reopen saved project; Android document picker
  for opening `.pixel` projects and saving a copy elsewhere.
- Dark/light appearance, narrow-width inspector sheet, large-text layout checks.
- Unsaved-work confirmation for replacing a document or pressing Android Back.

Private Save writes and syncs a temporary file before publishing it. Saved state
is acknowledged only after I/O succeeds and the editor revision still matches.
Provider copies have the guarantees of the selected Android document provider;
a failed external write may leave a partial file. Picker cancellation leaves the
current artwork intact.

## Preview limits

256 × 256 maximum canvas; 32 layers; 64 frames; 16 MiB decoded raster budget;
2 MiB decompressed native project JSON; 8 MiB input/output file; bounded history
(32 entries / 32 MiB target). These limits apply only to this preview. Oversized
or unsupported projects are rejected without replacing the current document.
A stroke that would exceed memory or reopen limits rolls back rather than
recording an unsaveable edit.

Only native `.pixel` opening/saving is exposed. Desktop export formats, advanced
tools, animation playback/editing, CLI/MCP hosting on Android, stylus pressure,
full autosave/process-death recovery and physical-device validation remain later
issue #29 work. Save before leaving or closing the app; Android may kill a
background process even after an unsaved-work prompt is implemented.

## Checks

```sh
flutter analyze
flutter test
flutter test integration_test/tablet_editor_test.dart -d emulator-5554
```

The integration test uses the actual Rust library and Android private storage:
draw, Undo/Redo, cancel a two-finger draft, save/reopen identical pixels, reject
an invalid storage key and reject malformed project data. Widget tests cover
1280/390 logical widths at text scales 1/2 plus Back and pending-edit protection.
See `../../docs/android-tablet-poc-validation.md` for recorded results and limits.
