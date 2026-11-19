use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WorkflowPage {
    #[default]
    Index,
    Palette,
    Brushes,
    References,
    ImportGrid,
    Tags,
    Atlas,
    Assistants,
    Recent,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PropertyPage {
    #[default]
    Sprite,
    Layer,
    Frame,
    Tileset,
}

impl WorkflowPage {
    fn title(self) -> &'static str {
        match self {
            Self::Index => "Workflow tools",
            Self::Palette => "Palette tools",
            Self::Brushes => "Brushes and selection",
            Self::References => "Reference images",
            Self::ImportGrid => "Import sprite sheet",
            Self::Tags => "Export animation tags",
            Self::Atlas => "Export multi-file atlas",
            Self::Assistants => "Assistant access",
            Self::Recent => "Recent files",
        }
    }
}
impl PixelCraft {
    pub(crate) fn open_workflow_destination(&mut self, id: &str) -> bool {
        let page = match id {
            "workflow" => WorkflowPage::Index,
            "palette-tools" => WorkflowPage::Palette,
            "brushes" => WorkflowPage::Brushes,
            "references" => WorkflowPage::References,
            "import-grid" => WorkflowPage::ImportGrid,
            "batch-tags" => WorkflowPage::Tags,
            "atlas" => WorkflowPage::Atlas,
            "assistants" | "assistant-access" => WorkflowPage::Assistants,
            "recent-files" => WorkflowPage::Recent,
            _ => return false,
        };
        self.power.workflow_page = page;
        self.power.open = true;
        true
    }
    pub(crate) fn open_property_destination(&mut self, id: &str) -> bool {
        let page = match id {
            "properties" | "sprite-properties" => PropertyPage::Sprite,
            "layer-properties" => PropertyPage::Layer,
            "frame-properties" => PropertyPage::Frame,
            "tileset-properties" => PropertyPage::Tileset,
            _ => return false,
        };
        self.power.property_page = page;
        self.power.property_target = self.current_property_target();
        self.properties_open = self.power.property_target.is_some();
        true
    }
    pub(crate) fn current_property_target(&self) -> Option<(u64, u64, u32, usize, usize)> {
        let layer = self.doc.layers.get(self.layer)?;
        self.doc.durations.get(self.frame)?;
        Some((
            self.doc.editor_id.runtime_key(),
            layer.editor_id.runtime_key(),
            self.doc.frame_export_id(self.frame),
            self.layer,
            self.frame,
        ))
    }
    pub(crate) fn capture_property_tile(&mut self) -> Result<(), String> {
        let (ax, ay, bx, by) = self.selection.unwrap_or((
            0,
            0,
            self.tile_size.min(self.doc.width) as i32 - 1,
            self.tile_size.min(self.doc.height) as i32 - 1,
        ));
        let (left, top, right, bottom) = (
            ax.min(bx).max(0),
            ay.min(by).max(0),
            ax.max(bx).min(self.doc.width as i32 - 1),
            ay.max(by).min(self.doc.height as i32 - 1),
        );
        if left > right || top > bottom {
            return Err("The selection does not overlap the canvas".into());
        }
        let (width, height) = ((right - left + 1) as usize, (bottom - top + 1) as usize);
        let pixels = self.doc.layer_pixels(self.layer, self.frame);
        let mut captured = Vec::with_capacity(width * height);
        for y in top..=bottom {
            let start = y as usize * self.doc.width + left as usize;
            captured.extend_from_slice(&pixels[start..start + width]);
        }
        let mut edited = self.doc.clone();
        edited.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "Captured tile".into(),
            width,
            height,
            tiles: vec![captured.into()],
        });
        edited.validate()?;
        self.remember_action("Capture selection as tile");
        self.doc = edited;
        self.selected_tile = 1;
        Ok(())
    }
    pub(super) fn workflow_dialog(&mut self, ctx: &egui::Context) {
        if !self.power.open {
            return;
        }
        let title = self.power.workflow_page.title();
        let footer_key = egui::Id::new(("workflow-footer-id", title));
        let close = fluent::modal_dialog_sized(ctx, title, 560., |ui| {
            let mut outside = vec![egui::Id::new(("editor-dialog", title)).with("close")];
            if let Some(id) = ctx.data(|data| data.get_temp::<egui::Id>(footer_key)) {
                outside.push(id);
            }
            if self.power.workflow_page != WorkflowPage::Index {
                let back = ui.pc_small_button("‹ All workflow tools");
                outside.push(back.id);
                if back.clicked() {
                    self.power.workflow_page = WorkflowPage::Index;
                }
            }
            let _scroll = egui::ScrollArea::vertical()
                .id_salt(("workflow", title))
                .max_height((ctx.content_rect().height() - 170.).max(48.))
                .show(ui, |ui| {
                    self.workflow_ui(ui, ctx);
                    fluent::reveal_focused_control(ui, &outside);
                    #[cfg(test)]
                    ctx.data_mut(|data| {
                        data.insert_temp(egui::Id::new("workflow-body-clip"), ui.clip_rect())
                    });
                });
            #[cfg(test)]
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("workflow-body-offset"), _scroll.state.offset)
            });
            ui.separator();
            let done = ui
                .push_id(("workflow-footer", title), |ui| ui.pc_button("Done"))
                .inner;
            ctx.data_mut(|data| data.insert_temp(footer_key, done.id));
            done.clicked()
        });
        if close {
            self.power.open = false;
        }
    }
    fn workflow_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.power.workflow_page {
            WorkflowPage::Index => {
                ui.label("Choose a task. Each tool keeps its own options.");
                for (id, label) in [
                    ("palette-tools", "Palette tools"),
                    ("brushes", "Brushes and selection"),
                    ("references", "Reference images"),
                    ("import-grid", "Import sprite sheet"),
                    ("batch-tags", "Export animation tags"),
                    ("atlas", "Export multi-file atlas"),
                    ("assistants", "Assistant access"),
                    ("recent-files", "Recent files"),
                ] {
                    if ui.pc_button(label).clicked() {
                        self.open_workflow_destination(id);
                    }
                }
            }
            WorkflowPage::Palette => {
                self.palette_arrangement_ui(ui);
                let mut wrap = self.doc.wraparound;
                let mut snap = self.doc.palette_snap;
                ui.pc_checkbox(&mut wrap, "Wrap drawing & fill across canvas edges");
                ui.pc_checkbox(
                    &mut snap,
                    "Snap pencil, fill & shapes to palette in RGB mode",
                );
                if wrap != self.doc.wraparound || snap != self.doc.palette_snap {
                    self.remember_action("Change drawing options");
                    self.doc.wraparound = wrap;
                    self.doc.palette_snap = snap;
                }
                ui.small("The seamless canvas uses the axes selected under View.");
                ui.horizontal(|ui| {
                    if ui.pc_button("Import palette…").clicked() {
                        self.palette_file(false);
                    }
                    if ui.pc_button("Export palette…").clicked() {
                        self.palette_file(true);
                    }
                });
                ui.small("HEX supports RGBA. JASC PAL and GIMP GPL store RGB.");
            }
            WorkflowPage::Brushes => {
                if ui_icons::button(ui, "wand", "Magic Wand", false, false).clicked() {
                    self.action("wand");
                }
                ui.pc_checkbox(&mut self.power.contiguous, "Contiguous matching");
                ui.add(
                    egui::Slider::new(&mut self.power.tolerance, 0..=255).text("Color tolerance"),
                );
                ui.horizontal(|ui| {
                    for op in [
                        SelectionOp::Replace,
                        SelectionOp::Add,
                        SelectionOp::Subtract,
                    ] {
                        ui.pc_selectable_value(&mut self.power.selection_op, op, format!("{op:?}"));
                    }
                });
                ui.small("Shift-click adds; Ctrl/Cmd-click subtracts.");
                self.brush_mode_ui(ui);
                if ui.pc_button("Capture selection as stamp").clicked() {
                    self.capture_stamp();
                }
                ui.pc_checkbox(
                    &mut self.power.reverse,
                    "Shade toward previous palette color",
                );
                ui.small("Dither alternates active/alternate colors on the canvas grid. Shade changes each pixel once per stroke.");
                self.brush_dynamics_controls(ui);
                self.brush_library_ui(ui);
            }
            WorkflowPage::References => self.reference_controls(ui),
            WorkflowPage::ImportGrid => self.grid_import_ui(ui),
            WorkflowPage::Tags => {
                self.export_scale_ui(ui);
                ui.small("One file per tag. Frame timing and forward/reverse/ping-pong order are preserved.");
                egui::ComboBox::from_label("Sheet engine preset")
                    .selected_text(&self.studio.sheet.preset)
                    .show_ui(ui, |ui| {
                        for preset in SheetOptions::PRESETS {
                            ui.pc_selectable_value(
                                &mut self.studio.sheet.preset,
                                preset.into(),
                                preset,
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    for (gif, label) in [(true, "Export tag GIFs…"), (false, "Export tag sheets…")]
                    {
                        if ui
                            .add_enabled_ui(self.export_job.is_none(), |ui| ui.pc_button(label))
                            .inner
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            let kind = ExportKind::Tags {
                                gif,
                                options: Box::new(self.studio.sheet.clone()),
                            };
                            self.start_export_recipe(ExportRecipe { path, kind }, false);
                        }
                    }
                });
            }
            WorkflowPage::Atlas => self.atlas_export_ui(ui),
            WorkflowPage::Assistants => {
                ui.label("The MCP server and CLI can work on files without opening the editor.");
                ui.monospace("pixelcraft --help\npixelcraft --mcp");
                let mut enabled = self.power.bridge.is_some();
                if ui
                    .pc_checkbox(&mut enabled, "Allow assistants to access this live editor")
                    .changed()
                {
                    if enabled {
                        let config = std::env::temp_dir()
                            .join(format!("pixelcraft-live-{}.json", std::process::id()));
                        let wake = ctx.clone();
                        match LiveBridge::start_with_wake(config, move || wake.request_repaint()) {
                            Ok(b) => {
                                self.power.bridge = Some(b);
                                self.power.bridge_generation += 1;
                                self.status =
                                    "Live editor access enabled. Edits are undoable.".into();
                            }
                            Err(e) => self.status = format!("Could not enable bridge: {e}"),
                        }
                    } else {
                        if let Some(progress) = self.power.bridge_export.take() {
                            progress.cancel();
                        }
                        self.power.bridge = None;
                        self.power.bridge_work = None;
                        self.status = "Live editor access disabled".into();
                    }
                }
                if let Some(b) = &self.power.bridge {
                    ui.label("Live access is active only while this checkbox is enabled.");
                    ui.label("Use this MCP server command:");
                    ui.monospace(format!(
                        "pixelcraft --mcp --live \"{}\"",
                        b.config.display()
                    ));
                    ui.small("The private bridge file contains an access token. Inspect returns a revision; stale edits are rejected. Ctrl+Z undoes assistant edits.");
                }
            }
            WorkflowPage::Recent => {
                let mut chosen = None;
                for (path, label, available) in self.recent_file_entries() {
                    if ui
                        .add_enabled_ui(available, |ui| {
                            ui_icons::button(ui, "open", &label, false, false)
                        })
                        .inner
                        .on_hover_text(path.display().to_string())
                        .on_disabled_hover_text(
                            "This file is no longer available at its saved path",
                        )
                        .clicked()
                    {
                        chosen = Some(path);
                    }
                }
                if let Some(path) = chosen {
                    self.power.open = false;
                    self.open_recent(path);
                }
                if ui.pc_button("Clear recent files").clicked() {
                    self.power.preferences.recent.clear();
                    self.save_workflow_preferences();
                }
                ui.separator();
                if ui.pc_button("Keyboard shortcuts…").clicked() {
                    self.power.open = false;
                    self.action("shortcuts");
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workflow_routes_select_the_named_task_without_enabling_live_access() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let original = app.doc.clone();
        for (id, page) in [
            ("palette-tools", WorkflowPage::Palette),
            ("brushes", WorkflowPage::Brushes),
            ("references", WorkflowPage::References),
            ("import-grid", WorkflowPage::ImportGrid),
            ("batch-tags", WorkflowPage::Tags),
            ("atlas", WorkflowPage::Atlas),
            ("assistants", WorkflowPage::Assistants),
            ("workflow", WorkflowPage::Index),
        ] {
            assert!(app.open_workflow_destination(id), "missing {id}");
            assert!(app.power.open);
            assert_eq!(app.power.workflow_page, page);
            assert!(app.power.bridge.is_none());
            assert_eq!(app.doc, original);
            assert!(app.undo.is_empty());
        }
        assert!(!app.open_workflow_destination("nonexistent"));
    }
    #[test]
    fn property_routes_remember_the_current_document_and_object() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        for (id, page) in [
            ("properties", PropertyPage::Sprite),
            ("sprite-properties", PropertyPage::Sprite),
            ("layer-properties", PropertyPage::Layer),
            ("frame-properties", PropertyPage::Frame),
            ("tileset-properties", PropertyPage::Tileset),
        ] {
            assert!(app.open_property_destination(id), "missing {id}");
            assert_eq!(app.power.property_page, page);
            assert_eq!(
                app.power.property_target,
                Some((
                    app.doc.editor_id.runtime_key(),
                    app.doc.layers[app.layer].editor_id.runtime_key(),
                    app.doc.frame_export_id(app.frame),
                    app.layer,
                    app.frame
                ))
            );
            assert!(app.properties_open);
        }
        assert!(!app.open_property_destination("nonexistent"));
    }
    fn frame(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.))),
                events,
                ..Default::default()
            },
            |ui| {
                if app.power.open {
                    app.power_dialogs(ui.ctx());
                } else {
                    app.properties_dialog(ui.ctx());
                }
            },
        );
        output.textures_delta.clear();
        output
    }
    fn text_position(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
        output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                (text.galley.text() == label).then_some(text.pos + text.galley.size() * 0.5)
            } else {
                None
            }
        })
    }
    fn click(pos: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    #[test]
    fn focused_workflows_render_only_their_task_and_keep_done_reachable() {
        for (id, marker) in [
            ("palette-tools", "Palette tools"),
            ("brushes", "Color tolerance"),
            ("references", "Reference images"),
            ("import-grid", "Choose sprite sheet…"),
            ("batch-tags", "Export tag GIFs…"),
            ("atlas", "Add sprites…"),
            ("assistants", "Allow assistants to access this live editor"),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_workflow_destination(id);
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![]);
            assert!(
                text_position(&output, marker).is_some(),
                "missing {id} control {marker}"
            );
            let painted = text_position(&output, "Done").unwrap();
            let done = ctx
                .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", ""))))
                .unwrap()
                .center();
            assert!(
                (done - painted).length() < 32.,
                "button/label discrepancy {done:?}/{painted:?}"
            );
            assert!(
                (0.0..384.).contains(&done.y),
                "{id} footer outside viewport {done:?}"
            );
            if id != "assistants" {
                assert!(
                    text_position(&output, "Allow assistants to access this live editor").is_none()
                );
            }
            frame(&mut app, &ctx, click(done));
            assert!(!app.power.open, "Done did not close {id} at {done:?}");
            assert!(app.power.bridge.is_none());
        }
    }
    #[test]
    fn object_properties_render_their_scope_and_keep_done_reachable() {
        for (id, marker, absent) in [
            (
                "sprite-properties",
                "Color & project",
                "Named animation ranges",
            ),
            ("layer-properties", "Active layer & cel", "Color & project"),
            (
                "frame-properties",
                "Named animation ranges",
                "Color & project",
            ),
            (
                "tileset-properties",
                "Tilesets & tile layers",
                "Named animation ranges",
            ),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_property_destination(id);
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![]);
            assert!(
                text_position(&output, marker).is_some(),
                "missing {id} section"
            );
            assert!(
                text_position(&output, absent).is_none(),
                "unrelated section in {id}"
            );
            let painted = text_position(&output, "Done").unwrap();
            let done = ctx
                .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", ""))))
                .unwrap()
                .center();
            assert!(
                (done - painted).length() < 32.,
                "button/label discrepancy {done:?}/{painted:?}"
            );
            assert!(
                (0.0..384.).contains(&done.y),
                "footer outside viewport {id}: {done:?}"
            );
            frame(&mut app, &ctx, click(done));
            assert!(!app.properties_open, "Done did not close {id} at {done:?}");
        }
    }
    #[test]
    fn properties_close_if_same_index_now_refers_to_a_different_layer_or_document() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.open_property_destination("layer-properties");
        app.doc.layers[app.layer].editor_id = EditorId::default();
        frame(&mut app, &ctx, vec![]);
        assert!(
            !app.properties_open,
            "replaced layer retained an old properties target"
        );
        app.open_property_destination("sprite-properties");
        app.doc.editor_id = EditorId::default();
        frame(&mut app, &ctx, vec![]);
        assert!(
            !app.properties_open,
            "new document retained an old properties target"
        );
        assert!(app.undo.is_empty());
    }

    #[test]
    fn capturing_partly_off_canvas_selection_stays_in_bounds_and_is_undoable() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(4, 4), None);
        app.doc.set_pixel(0, 0, 0, [10, 20, 30, 255]);
        let original = app.doc.clone();
        app.selection = Some((-2, -3, 1, 1));
        app.capture_property_tile().unwrap();
        let tile = app.doc.tilesets.last().unwrap();
        assert_eq!((tile.width, tile.height), (2, 2));
        assert_eq!(tile.tiles[0][0], [10, 20, 30, 255]);
        app.doc.validate().unwrap();
        app.undo();
        assert_eq!(app.doc, original);
    }
    #[test]
    fn capturing_selection_outside_canvas_reports_error_without_mutating() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let original = app.doc.clone();
        app.selection = Some((i32::MAX, i32::MAX, i32::MAX, i32::MAX));
        assert!(app.capture_property_tile().is_err());
        assert_eq!(app.doc, original);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn dragged_modal_close_stays_reachable_after_viewport_shrinks() {
        let ctx = egui::Context::default();
        let title = "Resize test";
        let draw = |size: Vec2, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ui| {
                    fluent::modal_dialog_sized(ui.ctx(), title, 550., |ui| {
                        ui.label("Short content");
                        false
                    });
                },
            );
            out.textures_delta.clear();
        };
        let size = Vec2::new(1920., 1080.);
        for _ in 0..3 {
            draw(size, vec![]);
        }
        let close = || {
            ctx.data(|d| {
                d.get_temp::<Rect>(egui::Id::new(("dialog-close", title)))
                    .unwrap()
            })
        };
        let start = close().center() - Vec2::new(100., 0.);
        draw(
            size,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let end = start + Vec2::new(500., 300.);
        draw(size, vec![egui::Event::PointerMoved(end)]);
        draw(
            size,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        for _ in 0..3 {
            draw(Vec2::new(683., 384.), vec![]);
        }
        assert!(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.)).contains_rect(close()),
            "close outside smaller viewport {:?}",
            close()
        );
    }

    #[test]
    fn properties_close_safely_if_current_indices_are_invalidated() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.open_property_destination("layer-properties");
        app.layer = usize::MAX;
        frame(&mut app, &ctx, vec![]);
        assert!(!app.properties_open);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn group_properties_allow_unlocked_groups_and_respect_ancestor_locks() {
        for (own_lock, parent_lock) in [(false, false), (true, false), (false, true)] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = PixelCraft::with_context(&ctx, None);
            let parent = app.doc.add_group("Parent").unwrap();
            let group = app.doc.add_group("Child").unwrap();
            app.doc.layers[group].parent = Some(parent);
            app.doc.layers[parent].locked = parent_lock;
            app.doc.layers[group].locked = own_lock;
            app.layer = group;
            app.open_property_destination("layer-properties");
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![]);
            assert!(text_position(&output, "Blend mode").is_some());
            let selectors: Vec<_> = output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| node.role() == egui::accesskit::Role::ComboBox)
                .collect();
            assert_eq!(selectors.len(), 2, "Blend and parent group selectors");
            for (_, node) in selectors {
                assert_eq!(
                    node.is_disabled(),
                    own_lock || parent_lock,
                    "group lock state {own_lock}/{parent_lock}"
                );
            }
            assert!(app.undo.is_empty());
        }
    }

    fn assert_tab_reveals_deep_control_and_keeps_footer(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        label: &str,
        prefix: &str,
    ) {
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![]);
        let output = frame(app, ctx, vec![]);
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let target = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("missing accessible control {label}"));
        let done = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Done"))
            .map(|(id, _)| *id)
            .unwrap();
        let tab = || egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        for _ in 0..220 {
            if ctx
                .memory(|m| m.focused())
                .is_some_and(|id| id.accesskit_id() == target)
            {
                break;
            }
            frame(app, ctx, vec![tab()]);
        }
        let focused = ctx.memory(|m| m.focused()).expect("focus reaches target");
        assert_eq!(focused.accesskit_id(), target, "Tab reaches {label}");
        for _ in 0..12 {
            frame(app, ctx, vec![]);
        }
        let rect = ctx.read_response(focused).unwrap().rect;
        let clip = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new(format!("{prefix}-body-clip"))))
            .unwrap();
        assert!(
            clip.contains_rect(rect),
            "focused control {label} must be fully shown {rect:?} inside {clip:?}"
        );
        let offset = ctx
            .data(|d| d.get_temp::<Vec2>(egui::Id::new(format!("{prefix}-body-offset"))))
            .unwrap();
        frame(app, ctx, vec![tab()]);
        assert_eq!(
            ctx.memory(|m| m.focused()).unwrap().accesskit_id(),
            done,
            "next Tab goes to fixed footer"
        );
        let footer = ctx
            .read_response(ctx.memory(|m| m.focused()).unwrap())
            .unwrap()
            .rect;
        for _ in 0..12 {
            frame(app, ctx, vec![]);
        }
        assert_eq!(
            ctx.data(|d| d.get_temp::<Vec2>(egui::Id::new(format!("{prefix}-body-offset"))))
                .unwrap(),
            offset,
            "footer focus must not move body scroll"
        );
        assert_eq!(
            ctx.read_response(ctx.memory(|m| m.focused()).unwrap())
                .unwrap()
                .rect,
            footer
        );
        assert!(Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.)).contains_rect(footer));
    }
    #[test]
    fn tab_reveals_deep_workflow_control_then_keeps_fixed_footer_stable() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.open_workflow_destination("brushes");
        assert_tab_reveals_deep_control_and_keeps_footer(
            &mut app,
            &ctx,
            "Brush library",
            "workflow",
        );
    }
    #[test]
    fn tab_reveals_deep_animation_control_then_keeps_fixed_footer_stable() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(4, 4), None);
        app.doc.tags = (0..20)
            .map(|i| FrameTag {
                editor_id: Default::default(),
                name: format!("Animation {i}"),
                from: 0,
                to: 0,
                direction: 0,
                repeat: 0,
                color: [110, 170, 220],
            })
            .collect();
        app.open_property_destination("frame-properties");
        assert_tab_reveals_deep_control_and_keeps_footer(
            &mut app,
            &ctx,
            "Use all frames for playback",
            "properties",
        );
    }
}
