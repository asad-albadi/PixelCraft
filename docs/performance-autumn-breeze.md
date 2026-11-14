# Autumn Breeze performance investigation

The reproducer is a 480×270 project with 24 frames and 15 layers. It stores 46,656,000 pixel slots before compression. During live editing, the old Windows build stopped responding and MCP reached its 10-second deadline while the editor was busy.

## Findings

- Periodic recovery serialized the entire workspace to compressed JSON on the editor thread. The regression fixture blocked that thread for 43.37 seconds in the original debug build. The optimized build now queues the same recovery in 45.6 µs and the test waits for and verifies the saved backup.
- The timeline rebuilt up to 256 full-size frame textures on any document change. It now uses textures that the canvas and playback have already rendered, with a 64 MiB cache budget and room for at least one frame. Unvisited frames display their numbers.
- Project loading made small reads directly against the decompressor, and file MCP revisions allocated a complete JSON encoding. The codec now buffers I/O and file MCP hashes raster bytes directly. Existing native projects remain readable. The real project's inspect took 72.0 seconds in the original debug build and 2.20 seconds in the updated optimized build. This comparison includes the effect of compiler optimization.
- The editor runs ordinary file opening, saving, export, periodic recovery, and large artwork color scans on background workers. Save completion applies to the originating tab, and edits made during the save remain dirty.

## Measured scope

- The real project opened through the editor test harness and built frames at 2.8 ms p95 and 3.0 ms maximum in the optimized build. This measures CPU-side UI work, without GPU presentation.
- A 1,024×1,024, 15-layer, four-frame fixture near the 64-million-pixel limit passed 20 frame switches with interleaved edits and restored its original state after four undos. Its CPU frame build took 46.2 ms p95 and 60.9 ms maximum. It remains above a 16.7 ms budget for 60 Hz.
- Startup recovery, close-confirmation saves and first-time compositing of large frames still have synchronous work. Sustained native-window latency and GPU memory were not measured.


## Sparse timeline follow-up

On CachyOS x86-64 with an AMD Ryzen 7 PRO 4750U, optimized CLI builds created and reopened a 512×256, 120-frame timeline with a single painted pixel in its last frame. The released v0.6.0 AppImage was extracted and run through its AppRun entry point for comparison with the sparse-storage branch. The table records one run per build. Timings include process startup and revision inspection where applicable.

| Build | Create and save | Peak RAM while saving | Open and inspect | Peak RAM while opening | Native file |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.6.0 AppImage | 545 ms | 27,420 KiB | 1,415 ms | 173,064 KiB | 1,382,785 bytes |
| Sparse-storage branch | 7 ms | 11,164 KiB | 4 ms | 11,164 KiB | 1,754 bytes |

The frame-cache regression verifies reuse of an unaffected texture and refresh of both frames sharing edited artwork. The prefetch test checks uploaded pixels after replacing an obsolete result. These data are CPU-side measurements. Native input-to-present latency, missed display deadlines and peak VRAM still require a presentation benchmark.

## Hardware presentation and playback recovery

The native Linux Wayland benchmark used Hyprland, Mesa Vulkan on the integrated AMD Radeon GPU, and hardware timestamps from the compositor's presentation feedback. These are actual window presentations, which can include repeated animation frames. A presentation gap over 16.7 ms is a missed **presentation interval budget**, not a count of skipped sprite frames. The synthetic fixtures target 60 animation frames per second with a repeating 17/16/17 ms duration. Each result is one six-second capture after four seconds of warmup.

| Same input in both builds | Build | Presentation gap p95 / max | Gaps over 16.7 ms | Peak RSS | Sampled DRM VRAM |
| --- | --- | ---: | ---: | ---: | ---: |
| 512×256, 120 frames, 112 empty | v0.6.0 | 13.89 / 27.78 ms | 2 / 712 | 86.5 MiB | 23.1 MiB |
| Same sparse fixture | Branch | 13.89 / 13.90 ms | 0 / 675 | 76.0 MiB | 23.1 MiB |
| 1024×768, 12 translucent layers, 8 frames | v0.6.0 | 27.78 / 41.67 ms | 143 / 323 | 755.0 MiB | 23.1 MiB |
| Same dense fixture | Branch | 13.89 / 145.84 ms | 16 / 505 | 683.8 MiB | 23.1 MiB |
| Published Moonlit Sanctuary GIF, 24 flattened frames | v0.6.0 | 130.03 / 170.03 ms | — | 328.6 MiB | 15.0 MiB |
| Same GIF | Branch | 90.02 / 120.02 ms | — | 296.4 MiB | 15.0 MiB |

The dense fixture saw fewer late presentations but still had an individual stall. The GIF has its own slower animation timing and is not a 60 fps workload. The synthetic fixtures were measured on a 144 Hz output and the GIF on a 100 Hz output; comparisons use the same output within each input. Per-process DRM accounting also reported about 202 MiB of GPU-accessible system memory. Window presentation times do not measure input latency or prove every scheduled animation frame appeared.

Reproduce on a visible Wayland window with Mesa's Vulkan driver and per-process DRM accounting:

```sh
python3 checks/playback-presentation.py target/release/pixelcraft /tmp/dense-60.aseprite --create-fixture dense
python3 checks/playback-presentation.py /path/to/extracted-v0.6.0/AppRun /tmp/dense-60.aseprite --seconds 6 --warmup 4
python3 checks/playback-presentation.py target/release/pixelcraft /tmp/dense-60.aseprite --seconds 6 --warmup 4
```

The locally packaged Linux editor saved a native project and exported nine PNG frames while playback was active. After a separate, unsaved edit, it completed its periodic backup. The process was killed and restarted, and the recovered document matched the exact unsaved test pixel. The AppImage exposed the same 30 MCP tool schemas as the development executable; its preview PNG matched the applied edit. The original layered Autumn Breeze and Moonlit Sanctuary projects were excluded from this Linux run.

For an independent file I/O comparison, the same two synthetic ASE inputs were reopened and re-saved as ASE by Aseprite 1.3.18.6-dev and PixelCraft on the same machine. These single-process totals include startup, decoding and encoding. They are separate from the native storage and presentation benchmarks.

| Input | Aseprite time / peak RSS | PixelCraft time / peak RSS |
| --- | ---: | ---: |
| Sparse 120-frame ASE | 58 ms / 24.0 MiB | 47 ms / 12.8 MiB |
| Dense 8-frame ASE | 1.78 s / 309.1 MiB | 8.99 s / 600.9 MiB |

The dense ASE save remains significantly more expensive in PixelCraft. Use the native format while editing; export ASE when you need interchange.

## Windows follow-up

On Windows, optimized builds before and after the follow-up re-saved the same 1024×768, 12-layer, 8-frame dense ASE fixture. Three alternating CLI runs included process startup, import, encoding and disk write. Median time fell from 4.480 s to 0.731 s. All six outputs matched the input byte for byte. The encoder kept its existing compression level. The decoder reuses dense, canvas-aligned backing, and the encoder passes full-canvas RGBA bytes straight to zlib.

Playback now holds the displayed frame when the next frame misses prefetch and resumes with the first ready successor. The regression test forces this condition and checks the successor's full duration. Baked ASE export also uses the existing background export worker. Windows GPU presentation and the original Linux Wayland benchmark were outside this follow-up measurement.

### Windows display capture

PresentMon 2.6.0 captured seven seconds per case after a five-second startup delay on the same Ryzen 9 9950X Windows desktop. Each capture tracked the visible editor's process ID. The dense and sparse inputs use the same 60 fps fixtures as the Linux comparison. The original layered Moonlit Sanctuary has 150 ms frame durations.

| Input | Original PR build p95 / max | Playback follow-up p95 / max | Display gaps over 16.7 ms, original / follow-up |
| --- | ---: | ---: | ---: |
| Dense | 6.12 / 6.20 ms | 6.11 / 12.12 ms | 0 / 0 |
| Sparse | 6.12 / 6.35 ms | 6.11 / 12.12 ms | 0 / 0 |
| Layered Moonlit Sanctuary | 127.27 / 133.29 ms | 127.28 / 127.33 ms | Not a 60 fps input |

For each binary, open the fixture with `pixelcraft.exe --open PATH --play`. Run `PresentMon --process_id PID --delay 5 --timed 7 --terminate_after_timed --v1_metrics --output_file capture.csv` and summarize positive `msBetweenDisplayChange` values for displayed presents. These are display presents, including repeated sprite content. They do not count skipped sprite frames or measure input latency or VRAM. Neither Windows build reproduced the previously observed long Linux stall in this run. Dense peak process working set was 867 MiB on the original PR build and 565 MiB on the follow-up.
