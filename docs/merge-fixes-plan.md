# Merge findings implementation plan

Scope: the ten findings in the task; all changes stay in app/.

1. Reproduce palette import through shared automation and the editor extension path; reserve a transparent palette entry and reindex existing pixels without losing alpha.
2. Reproduce reference loss on ASE Save; reject lossy saves in formats and offer Save as Pixel / Discard / Cancel in the editor. Mark saved only after a successful write.
3. Reproduce byte-at-a-time live bridge reads and revocation; bound input and total request time and poll cancellation during reads and response waits.
4. Store the pointer origin with the initial reference transform, and compute movement/scaling from total displacement; cover multiple pointer updates.
5. Reproduce stale wand-mask cropping with Copy/Delete/Fill and clear selection state after successful crop.
6. Overlay partial shortcut maps on defaults; exercise legacy startup merge and the starter extension including Ctrl+O and Ctrl+V.
7. Reproduce wrapped shade overdraw and deduplicate destination indices per dab.
8. Reproduce a tall narrow sprite with maximum atlas width; validate actual packed output rather than free search space.
9. Reproduce a valid large ping-pong source exceeding the document budget only after playback expansion; stream source frames and check output order and timing.
10. Update TLS dependencies and scan advisories; preserve a reproducible report of environmental blockers if network or scanner tools are unavailable.

Each behavior gets regression tests, a failing run, a minimal fix, and a passing run. Final checks: cargo test, ignored live authentication/lifecycle and slow-read tests, cargo build, headless CLI/MCP script, dependency advisory scan, rebuild_check.
