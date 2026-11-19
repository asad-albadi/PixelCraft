# Menu and settings discovery implementation plan

## Goal

Implement the approved issue #25 information architecture in independently testable batches. Artists find app preferences through categories and search, use short task-based menus, and open tool/document operations directly.

## Spec

The implementation brief is [issue #25](https://github.com/asad-albadi/PixelCraft/issues/25), approved for execution on 2026-11-19. The inventory and before/after map are recorded in docs/menu-settings-inventory.md.

## Architecture

Use the existing Rust/egui Fluent UI, native window decorations, command dispatch, Preferences storage and native operation renderers. Extend the shared menu catalog with grouping and submenu metadata. Add settings page/control metadata and focused workflow/property routes through the existing command/search entry points. App preferences, workspace/view state, document properties, tool options and export/import options have explicit owners.

## Constraints

- Existing action IDs and custom bindings continue to resolve. New paths retain recognizable search aliases.
- Inspector is left, drawing tools right, and Preview bottom-left.
- Existing document validation, history, permissions, live opt-in/revision checks and native CLI/MCP behavior own operations.
- Old preferences load with defaults for new navigation metadata. Search query and focus are session UI state.
- Rendering follows the available logical viewport and current DPI. Category navigation, Close and scale recovery stay reachable.
- Implement and test in this worktree. Commit each reviewed batch. Push/PR/release are separate follow-up actions.

## Review focus

1. Every original action remains reachable through menu or command search, including uncommon export formats.
2. Search identifies actual controls and opens the correct category. Page changes and keyboard handling preserve values and current edits.
3. Scoped resets and default values preserve correct preference ownership and document histories.
4. Context/property dialogs guard current document/layer/frame identities before editing.
5. Small logical viewports, fractional DPI, and oversized custom scale permit recovery and keyboard navigation.

## Batch 1 — Inventory and execution baseline

- [x] Run the existing full Linux suite against the starting tree. Passed 724 tests, 0 failures, 16 ignored after retrying a local linker failure before tests.
- [x] Inventory all catalog commands and all visible preference/tool/workflow/property control families. Reviewed 123 menu entries, 26 persisted fields, nested defaults and configuration scopes.
- [x] Record scopes, current routes, proposed destinations, configuration ownership, reset contract and a settings wireframe.
- [x] Assess native macOS menu wiring before implementation. Existing AppKit bindings support a bounded main-thread Settings event adapter.
- [x] Commit the plan and inventory. Independent review corrected scope, reset/storage details and early native-platform assessment.

## Batch 2 — Grouped menu catalog

Files are src/command_search/catalog.rs, src/command_search.rs and the menu renderer in src/main.rs. A focused menu renderer module can be introduced if it reduces the existing large method.

- [x] Add a failing regression for uncommon export and selection/tool actions being discoverable through grouped menu paths, alongside existing disabled/action routing tests.
- [x] Add separators and one-level submenus to the shared catalog. Group File import/export, Edit history/clipboard/transforms, Sprite transforms, Layer operations, Frame playback and View grids/layout.
- [x] Use the File, Edit, Sprite, Layer, Frame, Select, View, Tools, Help baseline. Select owns selection commands. Tools owns drawing and asset destinations.
- [x] Flatten the same catalog into search entries with stable IDs, old label aliases, current bindings and current availability.
- [x] Preserve dynamic recent-file, theme, density, Inspector and Preview entries.
- [x] Run targeted menu/search and rendered interaction regressions, formatting and Clippy.
- [x] Review correctness and security, fix findings and recheck. Commit.

Verification: isolated staged snapshot passed 18 command-search tests, 4 rendered menu tests, formatting and strict all-target Clippy. Regressions exercise pointer activation, wheel-scroll and Tab/Enter reachability of uncommon exports at 683×384. Independent review found and verified fixes for clipped menus, hidden keyboard focus and missing disabled explanations. All 120 original action IDs remain.

## Batch 3 — Categorized and searchable settings

Files are src/productivity_ui.rs and a focused src/productivity_ui/settings.rs module. Reuse the existing Preferences struct and preference read/write helpers.

- [ ] Add failing tests for finding recovery via autosave, opening the exact page/control, persistence, navigation and old-preference loading.
- [ ] Add eight page destinations from the issue. Page/control metadata includes stable IDs, labels, synonyms and ownership. Persist the last page as a tolerant string ID.
- [ ] Replace the accordion list with responsive category navigation and results. Search labels, descriptions and aliases using metadata. Open and reveal the matching control.
- [ ] Render existing controls in their assigned pages, including theme/density, canvas/navigation, onion defaults, color/import policies, new-document/recovery defaults and update preferences.
- [ ] Reuse shortcut, asset and About destinations with explicit navigation. Live assistant access remains scoped to the active editor.
- [ ] Add per-page reset and confirmed app-preference reset with precise scope. Provide immediate reachable interface-scale recovery.
- [ ] Add setting/page entries to command search and Command-Comma settings access on macOS.
- [ ] Run state-persistence, focus, keyboard, pointer, small-viewport and search regressions. Review and fix. Commit.

## Batch 4 — Focused workflow and property destinations

Files are src/productivity_ui.rs, src/main.rs, src/fluent.rs, src/studio.rs and catalog routes.

- [ ] Add failing route tests for palette tools, brushes, references, sheet import, tag export, atlas and assistant access.
- [ ] Give these workflows named page destinations and direct actions through current dispatch. Reuse existing operation renderers and native guards.
- [ ] Route Sprite, Layer, Frame and Tileset properties to their relevant section.
- [ ] Retarget Palette tools and toolbar/context shortcuts to their actual task. Tool options remain in the active tool strip.
- [ ] Move export scale to export controls, and label document settings and new-document defaults by scope.
- [ ] Verify opening/closing/reopening routes, document switches, dirty drafts, locked layers and Undo/Redo.
- [ ] Review/fix and commit.

## Batch 5 — Recovery and native platform integration

Files are settings/menu routing, src/fluent.rs, shortcut handling and a macOS-only integration module if supported by the current native stack.

- [ ] Add failing regressions for layout reset, scale recovery, confirmed reset scope and current-document preservation.
- [ ] Implement View layout recovery using existing Inspector/Preview/split-view state and stored configuration.
- [ ] Verify scale recovery through keyboard and settings at minimum logical sizes.
- [ ] Implement the early-assessed native AppKit Settings adapter with retained target/menu ownership and bounded event delivery through the existing settings action.
- [ ] Run relevant regressions and platform compilation/CI where available. Review/fix and commit.

## Batch 6 — Visual validation and final review

- [ ] Run task paths for scale, recovery, onion defaults, document profile, fill tolerance, sprite-sheet import/export, asset management, assistant access and layout recovery.
- [ ] Capture rendered UI at 1366×768 and 1920×1080, each at 100%, 125%, 150% and 200% display scaling. Exercise resizing, synthetic monitor-DPI changes, keyboard focus, hover/pressed/disabled/selected states and light/dark themes. Record physical monitor/hardware checks separately.
- [ ] Visually inspect real rendered screenshots and fix clipping, oversized spacing, unreachable controls or inconsistent feedback.
- [ ] Update README/navigation documentation and representative screenshots.
- [ ] Run formatting, strict all-target Clippy, full suite, native CLI/MCP workflows, live bridge, clipboard and graphical startup checks.
- [ ] Run independent whole-branch code/security and visual review, fix reproduced findings and re-review.
- [ ] Reconcile every issue checkbox with actual evidence and report completed work and any genuine platform/hardware verification limits.
