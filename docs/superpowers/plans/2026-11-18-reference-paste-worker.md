# Large Reference Paste Worker Plan

**Goal:** Keep reference-image paste off the UI call for large tile or raster clipboard data, preserving source pixels, orientation, masks, placement, destination identity and undo.

- [x] Reproduce synchronous reference commit with existing editor actions.
- [x] Route reference preparation through the private paste worker and add cancellation to bitmap extraction.
- [x] Verify tiled and raster input, flips/masks, origin, stale targets, Escape and invalid input atomicity.
- [x] Review, Windows tests and optimized benchmark; document remaining snapshot/cancellation audit limits and push.

## Verified checkpoint

- Red editor test reproduced synchronous tile reference commit. References now use the existing private paste builder and publication safeguards for both raster and tile clipboard inputs.
- Bitmap extraction checks cancellation at bounded raster chunks and tile rows/placements. Native clipboard writers retain the existing non-cancelling wrapper.
- Original reference placement uses the raw selection start, preserved separately from normalized tile placement bounds. Flips and cell masks remain unchanged.
- Five new regressions cover deferred tile reference/undo, large raster alpha/mask/undo, tile orientation/mask, stale destination/Escape and cancelled extraction without input mutation.
- Windows: 879 all-target tests passed; formatting/check and strict all-target Clippy passed. Independent review found no actionable issue; whitespace check passed.
- Expanded optimized benchmark: 4,194,304-pixel raster reference queue2.0021ms, completion6.1308ms; ordinary large tile paste queue61.9us/completion17.3676ms; source copy queue44.4us/completion64.5817ms. Excludes loading, viewport rendering and native serialization. Reference queue includes current Vec snapshot clone cost.
- Remaining: snapshot clone costs and cancellation responsiveness during validation/hash phases, then other indexed image API and viewport conversion/source parity audits. Goal active.
