# Indexed Color Fit Criteria Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement these tasks inline, with failing regressions before implementation.

**Goal:** Expose explicit RGB, linear RGB, CIE XYZ, and CIE Lab palette matching in native conversion, GUI, Lua, CLI, and MCP.

**Architecture:** Keep the existing squared RGBA metric as Default. Add a typed criterion and a backwards-compatible native entry point. Precompute palette coordinates per quantizer and retain the bounded pixel cache. Nearest selection uses the chosen space; ordered-pair projection uses the same coordinates, while Floyd-Steinberg carries errors in source RGBA. Reuse existing workers, cancellation, palette variants, and undo transactions.

**Tech Stack:** Rust, egui, Lua compatibility shim, existing Windows cargo pipeline. No new dependencies.

**Spec:** Issue #20 source-audit follow-up in 2026-11-16-issue-20-workflow-parity.md; Aseprite doc/rgbmap_base.cpp and app/commands/cmd_change_pixel_format.cpp at 9cf3c128.

## Constraints

- Existing native signatures and default output remain unchanged.
- Explicit metrics use sRGB decoding, D65 XYZ and Lab, and alpha distance scaled by 1/128 as in the audited implementation.
- Foreground transparent-slot behavior and exact authored slot identity stay intact.
- Lua fitCriteria accepts documented metric names; CLI/MCP use fit_criteria. Reject malformed names and types.
- Conversion remains cancellable, atomic, and undoable; do not run artwork work in the UI thread.

## Tasks

- [x] Write a failing command regression distinguishing RGB/linear/XYZ/Lab decisions and rejecting malformed criteria.
- [x] Implement IndexedFit and palette-space matching with an additive native conversion API. Verify native/default equivalence, ordered and diffusion paths, hidden artwork, palette variants, tiles, links, alpha, and cancellation.
- [x] Add the editor dropdown and route its worker through the typed API. Test actual UI selection and undo/redo.
- [x] Add Lua fitCriteria routing and CLI/MCP fit_criteria; verify saved indices, retained Lua handles, undo/redo, and malformed options.
- [x] Update docs, run formatting, strict Clippy and all Windows targets, review the diff, commit, and push the checkpoint. Keep the broader parity audit active.

## Source validation

An isolated MSVC harness compiled the original rgbToOtherSpace and findBestfit method bodies from the pinned source. Its 138 RGBA inputs include opaque, nearly transparent, partial-alpha, exact-palette and gray colors. PixelCraft matches all 552 direct-palette decisions across the four explicit metrics. The fixture records the source revision and method path. The temporary harness is in C:/Users/asad_/Projects/EngineChecks/issue20-indexed-fit-oracle; no third-party implementation is vendored into PixelCraft. The existing default metric is intentionally retained for compatibility.

## Verification checkpoint

Formatting, strict all-target Clippy, and 788 Windows all-target tests passed. This includes the compiled-reference fixture, actual GUI dropdown and deferred worker/undo behavior, Lua nearest and error diffusion, MCP/CLI file output, cancellation and existing default conversion regressions. The broader source parity audit remains active.
