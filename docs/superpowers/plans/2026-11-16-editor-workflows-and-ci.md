# Editor workflows and CI implementation plan

Implement all 16 tasks in [issue #16](https://github.com/asad-albadi/PixelCraft/issues/16) as independently reviewable batches.

- [ ] Sprite sheets and atlases. Store optional per-frame labels in native projects and export label metadata alongside immutable frame identities. Preview and save the same packed atlas. Let grid import skip transparent cells and set initial frame timing. Check native round trips, reimport identities, and grid import through the CLI.
- [ ] Timeline and preview. Give preview playback a separate frame and clock. Map preview clicks to the canvas and show its view bounds. Show timeline tag membership and edit repeat counts. Check editor interaction and exported tag metadata.
- [ ] Drawing and navigation. Add selection-aware Replace Color, palette drag reordering, corner outlines, shade direction controls, and optional whole-pixel wheel zoom. Check undo, indexed color, selection scope, and cursor anchoring.
- [ ] Preferences and recent files. Centralize application defaults and update preferences. Reuse the existing SHA-256 verification for opt-in background downloads. Check settings persistence and reset behavior, duplicate basenames, missing paths, and update scheduling.
- [ ] MCP file access. Apply optional project-root confinement and explicit overwrite authorization at the MCP boundary. Include multi-file outputs and keep intentional revision-checked saves. Check traversal, symlinks, and existing sidecars.
- [ ] Pull request CI. Add Linux and Windows formatting, lint, and test checks with a Windows application startup smoke test. Keep release artifacts in the tagged release workflow.

Run targeted tests before each commit. After integration, run the full Rust suite, strict Clippy, headless checks, and the graphical checks available on the workstation.
