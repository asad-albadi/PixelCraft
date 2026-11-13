# Compact layout validation

- Interface defaults to 100% zoom independently of native display scaling. Older preferences default to Compact; explicit zoom and density choices persist.
- Real egui widget-tree matrix: physical 1366×768 and 1920×1080, each at native scales 100%, 125%, 150%, 175%, 200%. Logical available sizes are physical dimensions divided by scale. Assertions verify native pixels per point, unchanged interface zoom, reachable primary actions and a fully visible first timeline row. Actual font/icon mesh software renders are in `shots/compact/light-*-matrix-*.png`.
- Pointer tests drag the inspector wider, drag the timeline taller and shorter than its default, collapse/reopen the inspector while preserving its width, and keep the preview anchored at the bottom-left in the smallest display case.
- Existing accessible timeline cell names, keyboard activation, document tabs, menus, themes and tooltips remain covered. Comfortable preferences round-trip and preserve 14-point body text.
- `cargo build --offline --locked`, `cargo fmt --check`, strict all-target Clippy and full Cargo tests: see compact-build.log, compact-clippy.log and compact-tests.log.
- Craft Anything `rebuild_check`: build and native startup/screenshot passed. Native screenshot: `../checks/round-0/screenshot.png` relative to app/. The report retains the pre-existing unverified original mechanics/runtime comparison and broader file/extension compatibility question; this UI change does not establish those comparisons.

At 1366×768 with 200% native scale (683×384 logical points), the inspector remains docked on the left with an independently pinned preview, secondary drawing controls use overflow, the tool rail and timeline scroll, and the timeline starting height is constrained to preserve some canvas space. Scroll viewports intentionally clip off-screen content; scroll to reach subsequent tools and layers.

Preview restoration: 68 editor tests passed (3 ignored), strict Clippy and formatting passed. A fresh optimized native startup produced `shots/compact/preview-restored-1080.png`. A manual 200-frame warm CPU UI benchmark measured approximately 5.96 ms/frame in debug and 0.43 ms/frame in release; these timings exclude GPU presentation and input latency.
