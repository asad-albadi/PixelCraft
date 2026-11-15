# Animation, editing, and MCP implementation plan

The six GitHub issues define the acceptance criteria. Keep existing project reads, undo snapshots, and index-based MCP clients working through the migration.

1. **#7** Introduce compact empty raster cels in `model.rs`, with regression tests for long blank timelines and copy-on-write snapshots. Next, store painted bounds and intentional artwork links separately from cel placement. Extend native and ASE codecs and verify legacy projects, signed offsets, indexed slots, tile links, and invalid references.
2. **#8** Make the existing frame cache in `main.rs` reuse unaffected frames. Compare the pixels with fresh composites before adding bounded worker prefetch. Measure playback, memory, and recovery on repeatable fixtures.
3. **#9** Add a shared rectangular timeline selection and transactional range commands. Add timing, link operations, onion skin filtering, and keyframes with deterministic sampling.
4. **#10** Extend `automation.rs` and the live bridge with stable targets, staged edits, progress, and cancellation. Cover revision conflicts, revocation, and output cleanup.
5. **#11** Use shared masks and adjustment data in the existing compositor for editor, preview, and export. Cover group boundaries and alpha, native persistence, and baked exports.
6. **#12** Add rotation about a pivot, slice authoring, and shared and per-placement tile editing. Check interoperability with representative ASE and plugin fixtures.

Run targeted failing regressions before implementation and the locked suite and strict Clippy after each integration. Use real projects for parity and synthetic fixtures for capacity and failure cases.
