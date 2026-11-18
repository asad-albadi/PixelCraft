# Frame-Paletted Indexed Tile Rendering Plan

**Goal:** Render authored indexed tile slots using the selected frame palette across layer sampling, compositing, regions and render-cache comparisons.

**Architecture:** Resolve valid tile slots against their base cached palette and sample the effective frame palette without allocations. Preserve stale/legacy raw pixels and mask transparency. Invalidate frame render comparisons on indexed tile palette inputs. Wire tileset previews to the same document sampling path.

- [x] Reproduce frame-palette render and cache failures.
- [x] Implement shared slot sampling and wire model rendering and preview.
- [x] Verify flip orientation, transparency, stale/legacy fallbacks, region/full consistency, native/ASE round trips and cache invalidation.
- [x] Review and Windows regression pipeline; document remaining conversion and clipboard integrations explicitly.

## Review findings resolved

- Same-mode validation/no-op Lua could replace newly masked authored slots with visible slots: accept valid masked and unmasked base caches.
- Slot-only edits were invisible to partial redraw damage: invalidate affected tile definitions when slot arrays change.
- Active-frame painting selected against the base palette: choose against the active frame palette while storing base-cache colors.
- Frame-only slots could fail: promote the missing base tail after a successful edit; checked ranges reject unpopulated mask slots before mutation.

## Source-audit clarification

Pinned Aseprite src/render/render.cpp uses sprite->palette(frame) for tile rendering. Its src/app/cmd/set_pixel_format.cpp converts tileset images using frame zero and explicitly leaves other-frame tileset generation as a TODO. Therefore full multi-frame tile-variant expansion is not evidenced as a required parity behavior. Follow-up should first correct PixelCraft conversion to the effective first palette (and active-frame snapshots), then assess any deliberate extension separately. Clipboard interpretation remains open.
