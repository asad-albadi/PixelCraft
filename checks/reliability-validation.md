# Reliability fixes — 2026-11-12

Implemented a single persistent native clipboard worker with one in-flight operation, a two-second UI deadline, late-result rejection, and bounded queues. A timed-out native call cannot accumulate replacement threads or queued operations. Internal copying remains available; a busy native clipboard permits internal pasting. Paste results are cancelled after document, tab, frame, layer, or selection changes. Wayland helper pipe IO runs outside the UI thread and helpers are killed/reaped after 1.5 seconds. Native arboard calls cannot be forcibly interrupted; the worker remains unavailable until such a call returns, while the UI and internal clipboard remain usable.

Atlas file dialogs and recursive folder expansion use the same supported-extension constant, including ORA, APNG, TMJ and TSJ, case insensitive. MCP live-create always opens an untitled tab, preserves the existing tab's filename and history, and provides Undo in the new tab. Reference sampling defaults off and is explicit in Workflow tools and floating references; sampling composites reference pixel alpha and overlay opacity in render order around the artwork, ignoring hidden and floating references on the canvas.

Clippy's original 24 library issues were addressed along with the binary/test warnings subsequently exposed. Existing public APIs retain their argument lists using narrowly scoped, reasoned `expect` annotations on three functions. Other warnings were fixed directly; no crate-wide warning suppression was added.

## Executed verification

- Initial strict lint reproduced 24 library failures: `strict-lint-before.log`.
- Atlas regression failed before the shared filter change: `reliability-red.log`.
- MCP tab/filename regression failed with the original replacement behavior: `mcp-create-red.log`.
- Full offline suite: 247 passed, 4 explicitly ignored (`reliability-tests.log`); writable isolated settings were required by the sandbox.
- Strict all-target Clippy: passed (`reliability-clippy.log`).
- Offline build and rebuild_check: passed; rebuild_check also confirmed a rendered 938x990 editor and mechanics checks (`reliability-build.log`, `reliability-rebuild-check.json`).
- Linux update installation test passed, checking bytes, executable permissions, previous-version backup and tampering rejection.

## Platform limits and remaining checks

The explicit native Linux clipboard test could not reach the sandbox's X11 server (`native-clipboard-linux.log`). Forced WGPU failure reached the automatic OpenGL restart, but the sandbox lacked a compositor for the restarted UI (`graphics-fallback-linux.log`). Ordinary graphical startup passed through rebuild_check. A host-side clipboard command through Craft Anything was rejected because approval policy is never. Windows is not installed on this host; no Windows runtime result is claimed.

Release CI now runs strict lint and native clipboard tests on both Linux and Windows, plus forced automatic graphics fallback on both platforms. The Windows-only update-launch test uses a freshly compiled harmless executable and verifies tampering rejection. Existing Windows installer CI already tests installation, upgrade, standalone operation and uninstall with user-data retention. These Windows and native desktop CI checks have been added but have not been run here.

Windows CI graphics checks use a pinned, SHA-256-verified Mesa software OpenGL driver from [mesa-dist-win 25.3.3](https://github.com/pal1000/mesa-dist-win/releases/tag/25.3.3) beside an isolated test executable. This driver is not packaged with PixelCraft. Normal startup is tested separately, and startup logs/screenshots are retained as CI artifacts.
