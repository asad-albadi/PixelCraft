# PR #26 Windows review and regression verification

Reviewed the latest `feature/discoverable-menus-searchable-settings` head (`ac391176`) and applied fixes on the same branch. Local host: Windows, Rust 1.95.0, isolated `PIXELCRAFT_HOME`. No installed user settings or original artwork were replaced.

## Findings corrected

- Animation exports keep their scale in the export draft; cancelling either dialog does not change sprite-sheet preferences. Tag GIF exports respect that scale and retain timing/order/repeat counts.
- Removed/rebound Settings and interface recovery shortcuts stay removed/rebound. Shortcut capture takes precedence, including native Settings dispatch.
- Properties dialogs hold playback while open. Rejected layer/tileset creation preserves Undo and Redo.
- Save/export/import drafts carry their originating document/frame identity; switching documents invalidates stale drafts instead of saving the wrong artwork.
- Palette loading checks the byte budget before parsing and stops at the color limit. Failed palette imports do not record history. Palette export preserves the artwork Repeat Last Export target.
- File saves use exclusive identity-checked staging, avoiding predictable temporary-file hard-link truncation.
- Every export output, including sheet/atlas metadata, engine sidecars and sequence members, is planned before publication. Existing destinations require explicit application confirmation; a native picker result does not grant blanket overwrite permission. Files appearing later cannot be silently replaced.
- Repeat authorizes only the last successful output manifest. Newly added tag outputs require confirmation; Windows filename case is preserved so case-sensitive directories cannot merge two approvals.
- The first atlas export writes its accepted preview. Repeat reloads sources. Filesystem preflight/atlas preparation runs off the UI thread, supports cancellation, and bounds pending work; long confirmation lists are virtualized.
- Sheet/tag exports read previous metadata from the canonical destination in the output plan, so directory aliases work inside the native guard and preserve existing Unity GUIDs. A Windows junction regression injects a distinct GUID and verifies it survives re-export.
- New file publication does not require hard-link support. Existing drive roots are valid directory export destinations. PNG sequences validate their document once, rather than once per frame.
- Native Windows/Linux output guards anchor writes and cleanup. Linux directory creation/removal uses directory descriptors; strict MCP Landlock rules remain enabled. macOS without the native guard accepts private output trees and rejects shared writable ancestors/ACLs rather than pretending shared folders are protected.

## Verification

- Formatting and strict all-target Clippy pass.
- Windows all-target suite: **803 passed, 0 failed, 17 opt-in tests ignored**. Added regressions reproduced reported failures before fixes where applicable.
- Rendered UI checks: four pass, with 64 saved captures. Settings covers 1366×768 and 1920×1080 at 100%, 125%, 150% and 200% in light/dark; 12 focused destinations cover Full HD/100% and 1366×768/200%, both themes. Representative saved images were inspected visually.
- Normal Windows GPU startup and forced software OpenGL fallback both render successfully; their actual executable captures were visually inspected.
- Fresh Windows executable passes the isolated single-executable portable check, the no-Lua payload check, native client workflow, and headless CLI/MCP checks (grid import, revision-checked editing, Unity metadata, atlas, palette conversion, tag sheets, PNG render and disabled live tools).
- Repeated independent source-only code/security reviews, including the later canonical-path fix; final reviewers reported no remaining actionable findings in the reviewed fixes. These reviews are not runtime penetration testing.

- Both original Autumn Breeze projects passed optimized native load, all-frame render, native save/reopen equality, PNG/GIF/MP4 export and ASE save/reopen frame equality. Day: 23 layers, 24 frames; night: 34 layers, 24 frames; both 480×270. Native saves took 1.72/2.54 seconds and ASE saves 0.62/0.80 seconds on this run. These are observations under concurrent test load, not performance guarantees.
- Both projects completed two ordered loops / 48 frame presentations through the real GUI playback cache. The cache test drives frame advancement; it does not measure physical-screen frame pacing or claim a human-observed playback capture.
- Original SHA-256 values remain `5bb6660d7440de4181de4ae0ba54f45ff047991a621b6dd276d9e2eef990d0ff` (day) and `a21a70ec2cb5e1f7aa87118cf1fe4f5ef88e72d9ce6c51214a14d0fed3aa0689` (night). All acceptance outputs are disposable copies.

Cross-platform CI must validate gated Linux/macOS code; Windows results do not substitute for those runs.

## CI follow-up

Hosted CI exposed a macOS-only strict-Clippy return warning, corrected without changing exclusive publication behavior. Hosted Windows and macOS also exposed duplicate confirmation caused by comparing a selected temporary-directory alias with the canonical export destination. Overwrite prompts now capture and display the physical parent before Replace; exact manifest comparison remains case-sensitive. Save As additionally requires that captured destination to equal the anchored writer destination. The rendered Windows regression covers PNG export and native Save As across stable aliases, retargeted aliases, and replacement of the physical parent itself. Changed destinations require fresh consent or fail without altering either file.

GUI worker wait helpers also yield while preflight is pending and explicitly assert completion before reading output. The final full Windows suite uses a fresh temporary directory to avoid collisions with stale process-ID fixtures from prior local runs. Final hosted CI status is recorded on the PR.

## Scope limits

Synthetic DPI/widget rendering is not physical monitor hot-plug testing. OS file-picker interaction and clipboard mutation were not automated locally. Unsupported-hard-link behavior is injected; no physical exFAT volume was available. Linux/macOS desktop testing was not performed on this Windows host. The user's separate Linux laptop validation remains separate from automated CI.
