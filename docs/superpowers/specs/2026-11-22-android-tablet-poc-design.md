# Android tablet proof of concept — issue #29

The approved scope is the tablet-first proof of concept from issue #29, not the
complete Android port. Reuse the Rust workspace and existing desktop behavior.
Keep the user's Pixel Tablet API 35 emulator visible and running between builds.

## Proof

Flutter displays actual Rust-composited pixels, drawing changes the shared
Document, gestures commit one Undo step, cancellation restores the stroke, and
native .pixel bytes save/reopen with equality. Include a usable narrow layout,
but do not claim real phone/tablet hardware, stylus or full mobile parity tested.

Add a small `pixelcraft-ffi` cdylib/rlib and `flutter/pixelcraft_mobile`. A versioned
C ABI uses non-reused numeric session/buffer handles. No foreign input pointers
are dereferenced by Rust: Dart fills caller-owned registered input buffers.
Borrowed buffer pointers are valid only until the owning buffer is freed; all
Dart native calls and memory copies run serially on one worker isolate. Registry
limits bound session and buffer allocations. Catch Rust unwinding at ABI entries;
invalid/stale handles fail without mutation. No transport or CLI dependency.

Drawing sends bounded binary batches of integer points and uses core line/brush
operations. Core EditorSession owns Undo and selection state. RGBA snapshots are
bounded to 256×256 for this prototype. Benchmark this full-frame path; dirty
updates and production-sized projects remain later issue work.

Opening untrusted native files must enforce compressed, decompressed and raster
allocation budgets before large allocation, not only validate after decode.
Keep desktop decode defaults unchanged. Android libc target gating is needed for
the existing Unix private-tree check; do not enable Linux MCP guards on Android.

## Tablet interface

Retain PixelCraft's teal accent (#008585), dark ink (#202227), calm panels
(#282c34), canvas surround (#171b21) and pale labels (#edf0f4). Use the existing
brand mark and simple platform typography. Canvas is the main visual feature,
with colors/layers left, drawing tools right and compact frame controls below.
Avoid decorative dashboard cards and developer details in product controls.
Touch targets stay reachable; narrow windows move inspectors into sheets.
Pinch navigation cancels an in-progress single-pointer stroke so it cannot leave
accidental marks. User artwork is never replaced without unsaved-work handling.

## Persistence and platform boundary

Quick Save/Reopen uses app-private staged files. Optional open/save-copy actions
use Android document intents, bounded streams and newly created documents; never
interpret content URIs as filesystem paths. Mark saved only after successful
publication and matching document revision. Do not promise atomic replacement
for document providers. Desktop storage and CLI/MCP contracts remain unchanged.

## Validation

Rust FFI tests cover handles, budgets, painting, cancellation, history and native
round trips. Compile x86_64 and arm64 Android libraries. Flutter tests cover
layout and input; emulator integration drives the real FFI and native storage.
Capture and inspect the visible tablet repeatedly, leave it running at the end,
and record device/emulator differences honestly. Re-run desktop workspace,
strict lint and CLI/MCP checks after shared changes. Review code and security
after each major batch and fix findings before proceeding.
