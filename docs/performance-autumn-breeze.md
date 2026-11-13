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
