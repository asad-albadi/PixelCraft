# Indexed Error Diffusion Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task. Steps use checkbox syntax for tracking.

The goal is to add Floyd-Steinberg indexed conversion through the native model and existing palette conversion dialog.

The architecture extends the existing quantizer with serpentine error diffusion. Keep two error rows and merge visible samples with sorted hidden samples so the canvas boundary does not split the error field. Preserve existing conversion methods and the cancellable editor worker.

The implementation uses Rust, the document's copy-on-write Pixels storage, and egui. It implements the source parity follow-up in 2026-11-16-issue-20-workflow-parity.md for issue #20.

## Source evidence

- OxyPixel checkout a292c5685499cdf14a7d4e3d4e664bbd5ccc1fe7 was rechecked from the repository. Its editor, animation clock, and shortcut implementation were compared with the corresponding PixelCraft paths.
- Aseprite checkout 9cf3c128f582044689656422c96d2d6904ce66f4 implements error diffusion in src/render/error_diffusion.cpp and offers it in src/app/commands/cmd_change_pixel_format.cpp.
- PixelCraft src/model/indexed_conversion.rs supports nearest swatches and an optional ordered matrix. src/palette_conversion.rs offers None, Bayer matrices, and installed matrices. Neither has a diffusion conversion mode.

## Constraints and review focus

- Keep indexed_conversion and indexed_conversion_until callable with their current arguments and behavior.
- Respect existing palette, storage, and cancellation limits.
- Preserve authored duplicate palette slots when converting with the same palette.
- Preserve frame palettes, transparent-slot remapping, background semantics, linked artwork, tiles, and hidden pixels.
- Start a fresh diffusion field for each cel image or tile image.
- Fully transparent input stays transparent and absorbs incoming error. Do not spread arbitrary transparent RGB.
- Do not allocate a dense rectangle spanning sparse hidden artwork.
- Leave the source unchanged on failure or cancellation.

## Task 1. Add bounded diffusion

Create src/model/indexed_conversion/diffusion.rs with private unit tests. This module owns accumulation and scan ordering. The existing Quantizer continues to choose palette slots.

Use two rows of four signed channel errors, with a one-pixel border. The caller supplies the horizontal interval and image row phase. Add incoming error to each channel and clamp to 0 through 255 before nearest-color matching. Distribute corrected-minus-chosen error by 7/16 to the next pixel and 3/16, 5/16, and 1/16 to the next row. Mirror the neighbors on reverse rows. Reset across gaps of more than one row.

- [x] Write tests for a 2 by 2 constant midtone mapped to opaque black and white. Assert a deterministic alternating pattern. Test a transparent barrier and a gap of absent rows.
- [x] Run the new indexed_error_diffusion model regression and observe the missing native mode. Run the private diffusion tests once the primitive is integrated.
- [x] Implement integer channel arithmetic with memory proportional to the horizontal interval.
- [x] Pass the focused tests and include the primitive in the verified feature commit.

## Task 2. Integrate complete cel and tile conversion

Modify src/model/indexed_conversion.rs and src/model.rs. Extend tests/editor_expansion.rs.

Export IndexedDithering with lifetime parameter and variants None, Ordered holding a borrowed DitheringMatrix, and FloydSteinberg. Add Document::indexed_conversion_with_until taking palette, transparent slot, IndexedDithering, and a cancellation callback and returning Result<Document, String>. Existing conversion methods delegate using None or Ordered.

Merge a serpentine visible-pixel iterator with sorted hidden pixel indices by row and scan direction. Carry errors across the canvas boundary without visiting large empty rectangles. Use the stored cel origin as the row phase. For source cache reuse, include hidden samples and cel origin using full equality, together with the existing palette variant and background state. Keep converted indices and Pixels shared for identical inputs.

- [x] Write a model regression covering midtones, frame palettes, transparent-slot relocation, and duplicate authored indices.
- [x] Compare conversion before and after canvas expansion. Put adjacent midtones on opposite sides of the original boundary and assert the resulting colors and slots match.
- [x] Check shared converted backing storage for linked cels and an ASE save/reopen. Give an independent cel different hidden neighbors and verify separate output where diffusion requires it.
- [x] Test tile conversion with fresh fields per tile. Cancel during a large conversion and verify the source is unchanged.
- [x] Observe the new-mode regression failure and implement the mode through Task 1. Check cancellation at least every 8192 processed samples.
- [x] Run cargo +1.95.0 test --offline --locked --test editor_expansion and include this task in the verified feature commit.

## Task 3. Expose the conversion in the editor

Modify src/palette_conversion.rs and extend its existing editor tests.

Add Floyd-Steinberg to Conversion dithering. Keep ordered patterns available. Capture the chosen native mode in the existing palette worker and call Task 2's conversion method.

- [x] Select Floyd-Steinberg and click Convert to Indexed in the dialog regression. Assert the source changes only after the worker returns, a midtone becomes a mixture of swatches, and undo and redo restore exact documents.
- [x] Verify cancellation and rejection of results after source edits.
- [x] Observe the regression fail, implement the option and dispatch, and pass it.
- [x] Run cargo +1.95.0 test --offline --locked --bin pixelcraft palette_dialog and include this task in the verified feature commit.

## Task 4. Support script and automation entry points

Modify src/lua.rs, src/lua_api.lua, src/lua_compat.lua, src/lua_tilesets.lua, and src/automation.rs. Add tests/lua_indexed_diffusion.rs and extend tests/mcp_files.rs.

The existing host dispatch handles flipCanvas, cropSprite, and resizeSprite. Add convertIndexed through the same document budget and result validation path. Route app.command.ChangePixelFormat with format indexed and ditheringAlgorithm error-diffusion through the native conversion. Reuse applySpriteGeometry to apply the resulting document while preserving retained sprite, layer, frame, and image handles. Existing arguments keep their existing behavior.

- [x] Write a headless Lua regression comparing native and scripted conversion of a midtone image with a frame palette. Retain an image handle across conversion, edit through it, and verify undo and redo.
- [x] Exercise that script through the CLI Lua entry point. Exercise native conversion through file MCP document commands. Assert output colors and indices agree.
- [x] Observe the new option fail, implement the native host dispatch and Lua command routing, and pass the tests.
- [x] Reject malformed or oversized conversion requests before processing pixels. Reuse the native script pixel budget.

## Task 5. Verify and checkpoint

- [x] Run formatting, strict Clippy for all targets, and the Windows all-target suite using the established issue #20 environment.
- [x] Review the diff against these constraints and record test results and unresolved behavior in the main issue #20 plan.
- [x] Push verified changes to feature/issue-20-workflow-parity.
- [ ] Continue the broader source audit.


## Execution rulings and evidence

- Tasks 1 and 2 were implemented together after the public conversion regression failed with a missing mode and method. Their helpers are used by the native entry point.
- File MCP has no Lua endpoint. The command named convert-indexed uses the existing atomic commands and live-commands paths. It accepts none or floyd-steinberg dithering, a transparent slot, and an optional palette. Invalid modes fail before publication. The CLI Lua workflow was tested through the executable.
- Conversion changes the format reported by retained cel and tile image handles. Their live format properties now follow the owning sprite across conversion, undo, and redo. The large-console image regression caught an intermediate RGBA write regression, which was fixed.
- Source cache reuse includes hidden inputs and row phase, in addition to visible storage identity, authored indices, palette variant, and background state.
- Partial alpha can diffuse between visible swatches and transparency. Transparent source pixels absorb incoming error without spreading hidden RGB.
- Model regressions passed for hidden-neighbor continuity, frame palettes, shared buffers and ASE reopen, duplicate indices, partial alpha, tiles, and cancellation. GUI regressions passed for dropdown conversion, undo and redo, cancellation, and source edits. CLI Lua and actual file MCP regressions passed.

Verification at this checkpoint passed formatting, strict Clippy for all targets, and 765 Windows all-target tests. The broader issue #20 goal remains active.
