# PixelCraft issue #20 implementation plan

> Use Superpowers test-driven development and verify each batch before recording it as complete.

**Goal.** Complete issue #20, then audit the implemented workflows against both reference editors again. Repeat the audit and implementation cycle for confirmed gaps. Prioritize the deeper editing and scripting workflows in the final parity pass.

**Base.** PR #19 at `056567df4cb519c8e7cc2374c4fc034433a57970`. Work on `feature/issue-20-workflow-parity`.

## Order of work

1. Restore sprite-sheet import fidelity (27–29). Add red tests in `tests/sheet_workflows.rs`, keep frame order through JSON parsing, normalize repeat and slice metadata in `src/workflows.rs`, and rerun sheet, file, and all-target tests.
2. Move recovery decoding off the startup path and exercise recovery choices (1–2). Verify with representative large workspaces and malformed backups.
3. Implement document, layer, timeline, and tilemap operations (11–12, 19–26), keeping frame IDs, tile flip flags, links, indexed slots, and undo behavior intact.
4. Fill drawing, selection, reference, and editor-control gaps (5–10, 13–18, 48–50, 52–55). Verify rendered editor interactions as well as model behavior.
5. Fill remaining file, export, Lua, and MCP gaps (30–47, 51). Test public entry points, reimport results, malformed inputs, and permission boundaries.
6. Add the remaining platform packages and smoke tests (3–4).
7. Re-scan peer source at pinned revisions, verify any new gaps and add their testable acceptance criteria to this tracker. Repeat until the agreed workflows have parity. Record any remaining test coverage limits explicitly.

Run focused red/green tests for each change, then project formatting, strict Clippy, and all-target tests at logical batch boundaries. Test Windows startup, editor playback, restricted MCP, and game engine reimport as relevant to the affected workflows. Check Linux and macOS where platform behavior is part of acceptance.

## Checkpoints

- PR #19 baseline. 418 Windows all-target tests passed.
- Items 27–29. Three focused regressions failed on the baseline, passed after the fix, and the Windows all-target suite passed 421 tests. Formatting and strict Clippy passed.

- Items 1–2 in progress. Workspace decoding now runs off the UI thread. The editor presents Restore workspace and Discard backup, preserving artwork opened during recovery. Windows release-candidate debug startup became responsive in 0.73 seconds against a copied 33,561,892-byte recovery snapshot. Formatting, strict Clippy and 422 Windows all-target tests passed. Per-document choice and raw-image salvage remain open.

- Item 1 complete and item 2 in progress. The recovery window lists all backed-up documents and can restore or discard each independently. After a decision, autosave preserves pending backups alongside open artwork. A discard removes the previous snapshot that could otherwise resurrect the deleted entry. Formatting, strict Clippy and 424 Windows tests passed. Raw-image salvage remains open.

- Item 2 complete. The recovery window can extract readable image cels as layers or frames into a new unsaved sprite, including hidden and off-canvas pixels. A bounded fallback reads valid raster records from otherwise damaged backups, one tab at a time. The focused regressions went red before the fixes, then passed. Formatting, strict Clippy, and 427 Windows all-target tests passed. Raster records that cannot be decoded remain unavailable.

- Item 11 complete. Sprite → Canvas size supports nine anchors and custom offsets across all layers and frames. Tile grids retain phase and flip flags and cover the resized canvas. Tests cover exact indexed slots, animation timing and IDs, tile expansion and shrink, exposed off-canvas pixels, masks, slice translation, invalid input atomicity and editor undo/redo. Formatting, strict Clippy, and 431 Windows all-target tests passed.

## Recovery and distribution

- [x] **1. Restore large workspaces in the background.** Show the editor promptly, report progress, and let users choose what to restore or discard.
- [x] **2. Recover individual documents and raw images.** Browse backups, recover or delete them independently, and salvage readable images as layers or frames.
- [ ] **3. Ship macOS builds.** Add a native Lua runtime, macOS CI and startup tests, and release packages for Apple Silicon and Intel.
- [ ] **4. Expand Windows and Linux packaging.** Publish and smoke-test a standalone Windows executable, a Linux tarball, and a Debian package.

## Drawing, selections, and color

- [ ] **5. Add drawing tools.** Implement spray with size and rate controls, curves, filled freehand contours, polygons, text, blur, and jumble.
- [ ] **6. Expand brush and ink controls.** Add per-tool inks such as lock alpha and copy color, saved brushes and shading ramps, image-brush alignment, and stroke stabilization and dynamics.
- [ ] **7. Add dithered gradients.** Expose gradient patterns and dithering in the gradient tool.
- [ ] **8. Add selection operations and reusable masks.** Include elliptical marquee, polygonal lasso, intersection, invert, reselect, Color Range, grow, shrink, border, and mask import and export.
- [ ] **9. Add on-canvas scale and skew handles.** Support interactive transforms on a selection and across the selected timeline cels.
- [ ] **10. Expand copy and paste.** Add Copy Merged, Paste as New Sprite, Layer or Reference, and New Sprite from Selection.
- [x] **11. Resize the canvas with anchors and offsets.** Apply resizing across layers and frames, including tilemaps.
- [ ] **12. Add whole-sprite operations.** Resample, auto-trim, duplicate, rotate, and flip sprites across layers and frames.
- [ ] **13. Expand filters.** Add brightness and contrast, curves, convolution, and despeckle with previews and selectable targets.
- [ ] **14. Generate palettes and improve indexed conversion.** Build an editable palette from artwork and expose conversion dithering and transparent-index controls.
- [ ] **15. Expand palette and picker tools.** Add sorting, selected-entry reversal, hue interpolation, and RGB, RYB, and normal-map wheels.
- [ ] **16. Extend ICC color management.** Assign and convert document profiles, choose import policies, and select a monitor, sRGB, or custom display profile.
- [ ] **17. Add eyedropper sampling controls.** Choose the sampling source and pick specific color or alpha channels.
- [ ] **18. Make symmetry axes movable.** Position each axis independently on the canvas.

## Layers, timeline, and tilemaps

- [ ] **19. Duplicate and merge layers.** Duplicate complete layers and group trees across frames and merge layers down with correct blending and opacity.
- [ ] **20. Expand layer organization.** Add flatten all or visible, layer via copy or cut, compatible layer type conversions, and collapsible groups.
- [ ] **21. Expand timeline editing.** Drag ranges of cels, frames, and layers. Support nonadjacent selection, frame reordering, and frame reversal.
- [ ] **22. Expand tile management.** Add direct flips and rotation, reusable tile duplication, an always-create variant mode, tile deletion and reordering with remapping, and tile index display.
- [ ] **23. Make seamless preview editable.** Paint on the repeated canvas and choose horizontal, vertical, or both axes.
- [ ] **24. Expand playback and onion skin controls.** Add selected-range playback, play once, subtags, rewind on stop, and draggable onion ranges.
- [ ] **25. Add an Undo History panel.** Inspect entries and jump to a point in the active document’s history.
- [ ] **26. Add split views and a configurable drawing grid.** Show multiple views of the same document, configure grid geometry and snapping, and set the grid from a selection.

## Files and export

- [x] **27. Preserve frame and tag order in JSON-hash sheet imports.** The Windows CLI imported frames with delays of 101, 102, and 110 ms as 101, 110, and 102 ms while keeping the same tag indexes. Add a regression for numbered keys such as `walk 1`, `walk 2`, and `walk 10`.
- [x] **28. Accept string-valued tag repeat counts.** The Windows CLI imported `"repeat": "3"` as infinite. Verify that both string and numeric forms preserve a finite count of three.
- [x] **29. Import object-valued JSON slice geometry.** Accept named fields in slice bounds, center, and pivot. The Windows CLI currently rejects a valid sheet with that geometry.
- [ ] **30. Export a standalone tileset atlas.** Export authored tilesets from selected or visible tilemap layers.
- [ ] **31. Expand batch export automation.** Compose multiple operations in one invocation, preview outputs without writing files, and support headless Lua with parameters and an interactive console.
- [ ] **32. Expand export filtering, splitting, and naming.** Include layer and group filters, per-layer and grid splits, named slices and frame ranges, and filename and tag templates.
- [ ] **33. Expand sheet layout controls.** Set border, shape, and inner padding separately. Add grid-aware trim, more sheet layout types, and explicit sheet dimensions.
- [ ] **34. Add Repeat Last Export.** Reuse the previous export destination and settings from the editor.
- [ ] **35. Expand interchange formats.** Add PSD and PSB import, FLI/FLC, ICO, PCX, QOI, and TGA reading and writing, JPEG and WebP writing, and SVG and CSS output.
- [ ] **36. Render scaled reference cels with precise bounds.** Honor the stored floating-point bounds of imported reference layers.
- [ ] **37. Preserve indexed and grayscale PNG modes.** Retain palette indices, transparency, and grayscale data where applicable on import and export.
- [ ] **38. Apply imported pixel aspect ratios and grid defaults.** Use the stored values as editable sprite properties.
- [ ] **39. Expand grid sheet import.** Add column-major order, optional trailing partial cells, and deriving cell dimensions from row and column counts.
- [ ] **40. Export Unity and Godot sidecars together.** Generate both engine sidecars and JSON from the same sheet export.
- [ ] **41. Import a raster file into the current document.** Add a direct Import as Layer action with defined crop and transparent-padding behavior.
- [ ] **42. Include global frame tags in packed atlases.** Write rebased `meta.frameTags` while preserving per-source tags.
- [ ] **43. Make atlas input lists editable.** Append files from successive selections, add paths manually, and remove individual inputs.

## Lua and extensions

- [ ] **44. Edit multiple independent documents from Lua.** Open, manage, and close sprites and switch the active editor document.
- [ ] **45. Expand Lua editor APIs.** Expose native tool invocation, clipboard access, editable slices and tilesets, typed properties, and consistent object identity.
- [ ] **46. Expand extension panels and lifecycle.** Support custom drawing canvases, tabbed dialogs and bounds, more editor events, command menu registration and removal, and reload.
- [ ] **47. Define permissioned system APIs for scripts.** Specify what file, process, and network access can be granted, and test that denied operations stay blocked.

## Editor and MCP workflow controls

- [ ] **48. Capture shortcuts from keypresses.** Show conflicts, protect reserved bindings, allow cancellation, and reset individual shortcuts.
- [ ] **49. Expand per-reference controls.** Lock individual references, toggle sampling per image, set aspect-linked dimensions, and show a placeholder when a linked source is missing.
- [ ] **50. Replace a selected palette span with a ramp.** Update the chosen range in place with explicit indexed-slot remapping.
- [ ] **51. Extend MCP editor operations.** Expose contiguous fill with tolerance, automatic outlines and shadows, background color on create, and group, parent, and blend editing.
- [ ] **52. Combine marquee selections with masks.** Support adding and subtracting with modifier keys while preserving the existing selection.
- [ ] **53. Paint with independent primary and secondary buttons.** Support direct secondary-color painting and palette assignment with the right mouse button.
- [ ] **54. Allow typed file paths in the GUI.** Provide path entry alongside native file browsing.
- [ ] **55. Hide Preview independently.** Persist Preview visibility independently of the Inspector.

## Verification

Add regression fixtures for the three import defects. Test editing operations against layered, animated, indexed, and tilemap documents where applicable, including undo and redo. Exercise GUI, CLI, and MCP entry points where each item applies. Run Windows and Linux checks and add macOS startup and packaging checks with the macOS work.
