# Native Destination Tile Palette Plan

**Goal:** Reconcile clipboard tile colors and authored slots with the destination frame before native matching/insertion.

- [x] Reproduce stale destination slots with reordered palettes and active frame palettes using editor copy/paste/undo.
- [x] Remap indexed pixels by exact slot when compatible, otherwise nearest visible destination entry; preserve transparency, store destination base colors and promote frame-only base slots atomically.
- [x] Clear indices for RGB/gray destinations and convert grayscale colors. Retain source metadata and shared remap results.
- [x] Verify repeated reuse, native/ASE persistence, mode boundaries, invalid input atomicity; review and Windows full pipeline.
- [x] Document and push. Lua destination remapping and large-copy responsiveness remain follow-ups.

## Verified behavior

- Red/green desktop regression confirmed reordered destination slots were stale. Native paste now selects effective frame entries and stores destination base-cache colors; matching and repeated reuse happen after reconciliation.
- Exact compatible duplicate and transparent nonmask slots are retained. Source mask pixels map to the destination mask; incompatible opaque colors use nearest visible RGBA distance.
- Frame-only destination entries extend base cache storage after successful preparation. Rejected sources leave the document unchanged.
- RGB/grayscale destinations clear authored index data; grayscale maps colors using the existing native luma conversion.
- Review found preparation of unused definitions could reject valid mask-only paste. Red desktop regression confirmed it; in-place preparation now uses only selected in-bounds IDs and empty placements return without an edit.
- Six added tests cover editor paste/repeat/undo/ASE reopen, unused definitions and empty placements, duplicate slots/sharing/mask relocation, nonmask transparent identity, nearest colors/mode boundaries and atomic promotion/rejection.
- Windows formatting/check, strict offline locked all-target Clippy and 851 all-target tests passed. Final independent review found no actionable finding.
- Next: Lua destination reconciliation and large-copy responsiveness, including source snapshot costs.
