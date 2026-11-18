# Native workflow acceptance — 2026-11-18

Scope: `feature/issue-20-native-workflows`, based directly on main `5c4daae`; PR #21. This supersedes historical completion counts that included newer Lua work. Main's Lua API files are byte-identical; its loader differs only for the packaged macOS dylib selector. New Lua work is archived on `discarded-lua-updates`.

## Requirement reconciliation

Each numbered entry below maps the original issue #20 requirement to current implementation and representative current-source coverage. These are not claims of exhaustive support for every file, input, device or operating system. The full Windows suite exercises model, rendered/pointer GUI, CLI, file MCP and live bridge paths where implemented; the linked test is a representative entry rather than the only coverage.

1. **Restore large workspaces in the background.** Implemented; Windows regression suite passed. Implementation: [src/studio.rs](../src/studio.rs); coverage: [src/studio.rs](../src/studio.rs), `startup_recovery_waits_for_a_decision_and_keeps_existing_artwork`.

2. **Recover individual documents and raw images.** Implemented; Windows regression suite passed. Implementation: [src/studio/recovery_images.rs](../src/studio/recovery_images.rs); coverage: [src/studio.rs](../src/studio.rs), `recovery_salvages_healthy_rasters_when_other_backup_records_are_damaged`.

3. **Ship macOS builds.** Implemented; native-platform CI pending. Implementation: [packaging/build-macos-bundle.sh](../packaging/build-macos-bundle.sh); coverage: [.github/workflows/macos-build.yml](../.github/workflows/macos-build.yml), `native architecture, bundle relocation, runtime and graphical startup CI`.

4. **Expand Windows and Linux packaging.** Implemented; native-platform CI pending. Implementation: [packaging](../packaging); coverage: [checks/windows-portable.ps1](../checks/windows-portable.ps1), `Windows standalone/graphics checks and Linux tarball/Debian release packaging`.

5. **Add drawing tools.** Implemented; Windows regression suite passed. Implementation: [src/sprite_text.rs](../src/sprite_text.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `sprite_text_previews_and_places_multiline_pixels_as_one_undoable_edit`.

6. **Expand brush and ink controls.** Implemented; Windows regression suite passed. Implementation: [src/productivity_ui/brush_library.rs](../src/productivity_ui/brush_library.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `tablet_pressure_controls_size_and_opacity_without_building_on_overlap`.

7. **Add dithered gradients.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `gradient_controls_apply_patterns_to_preview_release_and_radial_strokes`.

8. **Add selection operations and reusable masks.** Implemented; Windows regression suite passed. Implementation: [src/selection_tools.rs](../src/selection_tools.rs); coverage: [tests/productivity.rs](../tests/productivity.rs), `selection_morphology_clips_edges_and_mask_png_roundtrips_holes`.

9. **Add on-canvas scale and skew handles.** Implemented; Windows regression suite passed. Implementation: [src/canvas_transform.rs](../src/canvas_transform.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `canvas_transform_handles_edit_selected_cels_with_one_undo`.

10. **Expand copy and paste.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/main.rs](../src/main.rs), `clipboard_commands_create_distinct_destinations_and_keep_selection_holes`.

11. **Resize the canvas with anchors and offsets.** Implemented; Windows regression suite passed. Implementation: [src/workflows/canvas_resize.rs](../src/workflows/canvas_resize.rs); coverage: [tests/workflows.rs](../tests/workflows.rs), `crop_keeps_tile_grid_phase_and_splits_links_when_placements_clip_differently`.

12. **Add whole-sprite operations.** Implemented; Windows regression suite passed. Implementation: [src/workflows/sprite_transform.rs](../src/workflows/sprite_transform.rs); coverage: [src/studio.rs](../src/studio.rs), `whole_sprite_actions_apply_as_one_edit_and_duplicate_in_a_new_tab`.

13. **Expand filters.** Implemented; Windows regression suite passed. Implementation: [src/editor_filters.rs](../src/editor_filters.rs); coverage: [tests/editor_expansion.rs](../tests/editor_expansion.rs), `filter_targets_selection_and_reads_neighborhood_from_original_pixels`.

14. **Generate palettes and improve indexed conversion.** Implemented; Windows regression suite passed. Implementation: [src/palette_conversion.rs](../src/palette_conversion.rs); coverage: [tests/editor_expansion.rs](../tests/editor_expansion.rs), `indexed_error_diffusion_preserves_transparency_frame_palettes_and_links`.

15. **Expand palette and picker tools.** Implemented; Windows regression suite passed. Implementation: [src/color_wheel.rs](../src/color_wheel.rs); coverage: [tests/editor_expansion.rs](../tests/editor_expansion.rs), `palette_sort_and_noncontiguous_reverse_preserve_indexed_artwork`.

16. **Extend ICC color management.** Implemented; Windows regression suite passed. Implementation: [src/display_profiles.rs](../src/display_profiles.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `display_profile_changes_canvas_preview_and_prefetch_without_editing_artwork`.

17. **Add eyedropper sampling controls.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `eyedropper_source_and_channels_target_each_drawing_color`.

18. **Make symmetry axes movable.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `symmetry_handles_move_independently_and_reflect_pencil_strokes`.

19. **Duplicate and merge layers.** Implemented; Windows regression suite passed. Implementation: [src/model/layer_operations.rs](../src/model/layer_operations.rs); coverage: [src/model/layer_operations.rs](../src/model/layer_operations.rs), `duplicates_nested_groups_across_frames_with_fresh_targets_and_mapped_owners`.

20. **Expand layer organization.** Implemented; Windows regression suite passed. Implementation: [src/model/layer_conversion.rs](../src/model/layer_conversion.rs); coverage: [src/model/layer_operations.rs](../src/model/layer_operations.rs), `flatten_visible_bakes_every_frame_and_retains_hidden_subtrees`.

21. **Expand timeline editing.** Implemented; Windows regression suite passed. Implementation: [src/model/cel_selection.rs](../src/model/cel_selection.rs); coverage: [src/main.rs](../src/main.rs), `nonadjacent_cels_keep_gaps_during_moves_and_history`.

22. **Expand tile management.** Implemented; Windows regression suite passed. Implementation: [src/model/tile_operations.rs](../src/model/tile_operations.rs); coverage: [src/main.rs](../src/main.rs), `tileset_editor_reorders_duplicates_and_deletes_with_brush_history`.

23. **Make seamless preview editable.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `seamless_preview_repeats_both_axes_and_png_export_writes_one_tile`.

24. **Expand playback and onion skin controls.** Implemented; Windows regression suite passed. Implementation: [src/subtag_playback.rs](../src/subtag_playback.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `playback_waits_for_a_cache_miss_without_skipping_the_frame`.

25. **Add an Undo History panel.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/main.rs](../src/main.rs), `undo_history_panel_clicks_restore_only_the_active_tab`.

26. **Add split views and a configurable drawing grid.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/main.rs](../src/main.rs), `selection_sets_the_grid_and_pointer_starts_on_a_grid_intersection`.

27. **Preserve frame and tag order in JSON-hash sheet imports.** Implemented; Windows regression suite passed. Implementation: [src/workflows.rs](../src/workflows.rs); coverage: [tests/sheet_workflows.rs](../tests/sheet_workflows.rs), `imported_hash_sheet_preserves_serialized_frame_order_and_tags`.

28. **Accept string-valued tag repeat counts.** Implemented; Windows regression suite passed. Implementation: [src/workflows.rs](../src/workflows.rs); coverage: [tests/sheet_workflows.rs](../tests/sheet_workflows.rs), `imported_sheet_accepts_string_and_numeric_repeats`.

29. **Import object-valued JSON slice geometry.** Implemented; Windows regression suite passed. Implementation: [src/workflows.rs](../src/workflows.rs); coverage: [tests/sheet_workflows.rs](../tests/sheet_workflows.rs), `imported_sheet_accepts_named_slice_coordinates`.

30. **Export a standalone tileset atlas.** Implemented; Windows regression suite passed. Implementation: [src/interchange/tileset_atlas.rs](../src/interchange/tileset_atlas.rs); coverage: [tests/formats.rs](../tests/formats.rs), `tileset_atlas_file_and_live_exports_write_the_same_pixels_and_layout`.

31. **Expand batch export automation.** Native portion implemented; Lua deferred. Implementation: [src/automation.rs](../src/automation.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `export_batch_previews_exact_outputs_and_runs_the_ordered_operations`.

32. **Expand export filtering, splitting, and naming.** Implemented; Windows regression suite passed. Implementation: [src/interchange/sheet_templates.rs](../src/interchange/sheet_templates.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `sheet_templates_keep_names_and_unity_reimport_ids_stable`.

33. **Expand sheet layout controls.** Implemented; Windows regression suite passed. Implementation: [src/workflows/sheet_layout.rs](../src/workflows/sheet_layout.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `sheet_layouts_fit_fixed_canvases_and_keep_frame_order`.

34. **Add Repeat Last Export.** Implemented; Windows regression suite passed. Implementation: [src/export_actions.rs](../src/export_actions.rs); coverage: [src/main.rs](../src/main.rs), `repeat_last_export_uses_the_last_successful_settings_for_each_tab`.

35. **Expand interchange formats.** Implemented; Windows regression suite passed. Implementation: [src/interchange.rs](../src/interchange.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `photoshop_preview_preserves_visible_clipping_and_adjustment_appearance`.

36. **Render scaled reference cels with precise bounds.** Implemented; Windows regression suite passed. Implementation: [src/model.rs](../src/model.rs); coverage: [tests/expanded_compatibility.rs](../tests/expanded_compatibility.rs), `ase_reference_cels_use_precise_bounds_without_exporting_as_artwork`.

37. **Preserve indexed and grayscale PNG modes.** Implemented; Windows regression suite passed. Implementation: [src/formats.rs](../src/formats.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `cropped_and_sequence_png_exports_preserve_document_color_modes`.

38. **Apply imported pixel aspect ratios and grid defaults.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/main.rs](../src/main.rs), `imported_pixel_aspect_maps_pointer_and_grid_in_sprite_coordinates`.

39. **Expand grid sheet import.** Implemented; Windows regression suite passed. Implementation: [src/workflows.rs](../src/workflows.rs); coverage: [tests/sheet_workflows.rs](../tests/sheet_workflows.rs), `grid_import_count_order_and_partial_cells_match_between_cli_and_mcp`.

40. **Export Unity and Godot sidecars together.** Implemented; Windows regression suite passed. Implementation: [src/automation.rs](../src/automation.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `combined_engine_sheet_plans_and_reexports_both_sidecars_transactionally`.

41. **Import a raster file into the current document.** Implemented; Windows regression suite passed. Implementation: [src/studio.rs](../src/studio.rs); coverage: [src/main.rs](../src/main.rs), `raster_layer_import_crops_and_pads_only_the_target_frame_and_undoes`.

42. **Include global frame tags in packed atlases.** Implemented; Windows regression suite passed. Implementation: [src/atlas.rs](../src/atlas.rs); coverage: [tests/sheet_workflows.rs](../tests/sheet_workflows.rs), `packed_atlas_tags_use_global_frame_order_and_keep_source_metadata`.

43. **Make atlas input lists editable.** Implemented; Windows regression suite passed. Implementation: [src/atlas.rs](../src/atlas.rs); coverage: [tests/editor_expansion.rs](../tests/editor_expansion.rs), `folder_atlas_inputs_are_sorted_and_skip_unrelated_files`.

44. **Deferred — Edit multiple independent documents from Lua.** User explicitly stopped newer Lua development. Archive retains the work; main support remains. No completion claimed.

45. **Deferred — Expand Lua editor APIs.** User explicitly stopped newer Lua development. Archive retains the work; main support remains. No completion claimed.

46. **Deferred — Expand extension panels and lifecycle.** User explicitly stopped newer Lua development. Archive retains the work; main support remains. No completion claimed.

47. **Deferred — Define permissioned system APIs for scripts.** User explicitly stopped newer Lua development. Archive retains the work; main support remains. No completion claimed.

48. **Capture shortcuts from keypresses.** Implemented; Windows regression suite passed. Implementation: [src/shortcut_editor.rs](../src/shortcut_editor.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `shortcut_capture_checks_conflicts_reserved_keys_escape_and_command_reset`.

49. **Expand per-reference controls.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [tests/productivity.rs](../tests/productivity.rs), `reference_controls_keep_native_aspect_and_skip_unsampleable_overlays`.

50. **Replace a selected palette span with a ramp.** Implemented; Windows regression suite passed. Implementation: [src/model/palette_span.rs](../src/model/palette_span.rs); coverage: [tests/editor_expansion.rs](../tests/editor_expansion.rs), `replacing_palette_span_remaps_indexed_pixels_and_keeps_duplicate_outside_slots`.

51. **Extend MCP editor operations.** Implemented; Windows regression suite passed. Implementation: [src/automation.rs](../src/automation.rs); coverage: [tests/mcp_files.rs](../tests/mcp_files.rs), `mcp_create_and_commands_apply_editor_effects_and_group_changes_atomically`.

52. **Combine marquee selections with masks.** Implemented; Windows regression suite passed. Implementation: [src/selection_mask.rs](../src/selection_mask.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `marquee_drag_modifiers_preserve_mask_holes_and_history`.

53. **Paint with independent primary and secondary buttons.** Implemented; Windows regression suite passed. Implementation: [src/main.rs](../src/main.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `secondary_palette_swatch_and_mouse_button_keep_their_own_indexed_stroke`.

54. **Allow typed file paths in the GUI.** Implemented; Windows regression suite passed. Implementation: [src/file_path_dialog.rs](../src/file_path_dialog.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `file_menu_accepts_typed_paths_and_confirms_export_overwrite`.

55. **Hide Preview independently.** Implemented; Windows regression suite passed. Implementation: [src/productivity_ui.rs](../src/productivity_ui.rs); coverage: [src/ui_icon_tests.rs](../src/ui_icon_tests.rs), `preview_visibility_is_independent_and_survives_restart`.

## Verification and practical limits

- Windows: formatting, strict locked offline all-target Clippy and **762 tests** passed at `6cad071`. Existing main Lua tests are included; archived new Lua tests are excluded. Five new viewport regressions and twelve cancellation regressions supplement the earlier snapshot tests.
- Native day/night project acceptance: full document equality on disposable save/reopen, every one of24 frame composites preserved, PNG reimport equality, successful GIF/MP4 exports, unchanged original SHA256 hashes. Both exported images were visually inspected. Two ordered GUI/cache loops (48 presentations each) passed. The pumped-frame benchmark is not a wall-clock playback capture. See the closure plan for measured timings and source hashes.
- Native CI on PR21: Windows standalone executable/startup, Linux compilation/tests and both macOS architecture bundles/startup. Results must be recorded from the actual run; this document does not predeclare them green.
- Linux desktop playback is user-deferred. Physical tablet input, monitor-specific ICC behavior and OS-specific clipboard interaction on other machines are unverified; synthetic pressure and profile conversion tests are covered.
- PSD/PSB raster import preserves supported layers/masks/groups/high-depth samples and uses a merged preview for unsupported visual effects where available. This is not native editing of every Photoshop effect or color mode. Export/import formats retain their existing documented representational limits.
- Recovery can salvage readable records, not recreate missing/corrupt pixel payloads. Workload limits remain intentionally bounded rather than removed to disguise performance costs.
- Item31 remains partially open because its requested Lua console/headless script portion was explicitly deferred. Items44–47 are deferred, not completed. Platform packaging validation remains separate from feature implementation.

## Repeated current-source comparison

Fetched both audit repositories again on2026-11-18. Remote heads remained `eccf4ec292f66c0b8a4f814954ce7db160a8eb43` and `a292c5685499cdf14a7d4e3d4e664bbd5ccc1fe7`; no newer changes since the last source scan. Inspected menu/command search implementation at the former head and native routing, shortcut editor, CLI/MCP surfaces here. Native editor command discovery is still a confirmed additional gap: routing/shortcuts are not a searchable command picker. Tracked as [#22](https://github.com/asad-albadi/PixelCraft/issues/22); it was not one of the original55 items. Decimal precision in newer Lua dialogs stays archived/deferred with Lua.

The repeated audit closes neither universal feature parity nor device/platform coverage. The concrete remaining original scope is deferred Lua plus any native-platform CI failures discovered on PR21. Additional command discovery needs its own implementation and acceptance, rather than silently being counted as complete.

## Final source/test review checkpoint

Fresh risk-focused correctness/security review found one detached-preview playback defect; it was reproduced and fixed with delayed-worker clock/GUI regressions and a paused-preview assertion. The final Windows suite passed **764 tests**, strict Clippy and formatting. This review was not exhaustive inspection of all158 changed files; the final fix was verified by tests without a second reviewer. No additional actionable security defect was reported in the inspected native paths.

Real-project ASE acceptance additionally passed all24 composites, durations and layer counts after save/reopen, with optimized save times505ms day/634ms night. Full native acceptance and source hashes remain unchanged. Final-head platform outcomes are recorded on [PR21 checks](https://github.com/asad-albadi/PixelCraft/pull/21/checks) and its final review comment; earlier CI is not substituted for the final revision.
