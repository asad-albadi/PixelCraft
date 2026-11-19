//! Native command discovery. No executable extension registration or evaluation.
use super::*;
mod catalog;
pub(super) use catalog::{MENU_GROUPS, MenuItem};

#[derive(Default)]
pub(super) struct CommandSearch {
    pub open: bool,
    query: String,
    selected: Option<String>,
    focus: bool,
    document: Option<u64>,
    tab: usize,
    layer: Option<u64>,
    frame: u32,
}

#[derive(Clone)]
enum Target {
    Action(&'static str),
    Theme(egui::ThemePreference),
    Density(bool),
    Recent(PathBuf),
    Inspector,
    Preview,
}

pub(super) struct Entry {
    id: String,
    label: String,
    shortcut: String,
    enabled: bool,
    target: Target,
}

fn matches_query(label: &str, id: &str, query: &str) -> bool {
    let aliases = id
        .strip_prefix("action:")
        .map_or_else(String::new, |action| {
            catalog::LEGACY_ALIASES
                .iter()
                .filter(|(target, _)| *target == action)
                .map(|(_, alias)| *alias)
                .collect::<Vec<_>>()
                .join(" ")
        });
    let searchable = format!("{label} {id} {aliases}").to_lowercase();
    query
        .split_whitespace()
        .all(|word| searchable.contains(&word.to_lowercase()))
}

impl PixelCraft {
    pub(super) fn native_action_disabled_reason(&self, action: &str) -> Option<&'static str> {
        match action {
            "repeat-export" if !self.can_repeat_export() => {
                Some("No repeatable export is available for this document")
            }
            "undo" if self.undo.is_empty() => Some("Nothing to undo"),
            "redo" if self.redo.is_empty() => Some("Nothing to redo"),
            "deleteframe" if self.doc.durations.len() <= 1 => {
                Some("A sprite must keep at least one frame")
            }
            "deletelayer" if self.doc.layers.len() <= 1 => {
                Some("A sprite must keep at least one layer")
            }
            "clear" if !self.doc.editable(self.layer) => {
                Some("The active layer is locked or not editable")
            }
            "clear" if self.doc.layers[self.layer].tileset.is_some() => {
                Some("Clear is unavailable for tilemap layers")
            }
            "flipx" | "flipy" | "cut" | "paste" | "rotate" | "selectionflipx"
            | "selectionflipy"
                if !self.doc.editable(self.layer) =>
            {
                Some("The active layer is locked or not editable")
            }
            "crop"
            | "new-sprite-from-selection"
            | "grid-from-selection"
            | "export-selection-mask"
                if self.selection.is_none() =>
            {
                Some("Select an area first")
            }
            _ => None,
        }
    }

    pub(super) fn native_action_enabled(&self, action: &str) -> bool {
        self.native_action_disabled_reason(action).is_none()
    }

    fn command_catalog(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        fn append(
            entries: &mut Vec<Entry>,
            seen: &mut std::collections::HashSet<String>,
            editor: &PixelCraft,
            path: &str,
            items: &[MenuItem],
        ) {
            for item in items {
                match item {
                    MenuItem::Separator => {}
                    MenuItem::Submenu(label, children) => {
                        append(
                            entries,
                            seen,
                            editor,
                            &format!("{path} / {label}"),
                            children,
                        );
                    }
                    MenuItem::Action(label, action) => {
                        let id = format!("action:{action}");
                        if seen.insert(id.clone()) {
                            entries.push(Entry {
                                id,
                                label: format!("{path} / {label}"),
                                shortcut: ui_icons::shortcuts(&editor.custom_shortcuts, action),
                                enabled: editor.native_action_enabled(action),
                                target: Target::Action(action),
                            });
                        }
                    }
                }
            }
        }
        for &(group, items) in MENU_GROUPS {
            append(&mut entries, &mut seen, self, group, items);
        }
        for (action, label) in [
            ("swap", "Swap colors"),
            ("resetcolors", "Reset colors"),
            ("savepalette", "Save palette…"),
            ("loadpalette", "Load palette…"),
        ] {
            let id = format!("action:{action}");
            if seen.insert(id.clone()) {
                entries.push(Entry {
                    id,
                    label: format!("Colors / {label}"),
                    shortcut: ui_icons::shortcuts(&self.custom_shortcuts, action),
                    enabled: true,
                    target: Target::Action(action),
                });
            }
        }
        // Tools without a menu/shortcut still use the same native action routing.
        for tool in [
            Tool::Pencil,
            Tool::Spray,
            Tool::BlurBrush,
            Tool::JumbleBrush,
            Tool::Eraser,
            Tool::Fill,
            Tool::Picker,
            Tool::Line,
            Tool::Curve,
            Tool::Text,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Polygon,
            Tool::Contour,
            Tool::Select,
            Tool::SelectEllipse,
            Tool::Hand,
            Tool::Lasso,
            Tool::PolygonLasso,
            Tool::Move,
            Tool::Gradient,
            Tool::Wand,
        ] {
            let action = tool.action();
            let id = format!("action:{action}");
            if seen.insert(id.clone()) {
                entries.push(Entry {
                    id,
                    label: format!("Tools / {}", ui_icons::action_label(action)),
                    shortcut: ui_icons::shortcuts(&self.custom_shortcuts, action),
                    enabled: true,
                    target: Target::Action(action),
                });
            }
        }
        for (id, label, preference) in [
            ("system", "System", egui::ThemePreference::System),
            ("light", "Light", egui::ThemePreference::Light),
            ("dark", "Dark", egui::ThemePreference::Dark),
        ] {
            entries.push(Entry {
                id: format!("theme:{id}"),
                label: format!("View / Theme / {label}"),
                shortcut: String::new(),
                enabled: true,
                target: Target::Theme(preference),
            });
        }
        for (id, label, comfortable) in [
            ("compact", "Compact", false),
            ("comfortable", "Comfortable", true),
        ] {
            entries.push(Entry {
                id: format!("density:{id}"),
                label: format!("View / Density / {label}"),
                shortcut: String::new(),
                enabled: true,
                target: Target::Density(comfortable),
            });
        }
        entries.push(Entry {
            id: "view:inspector".into(),
            label: format!(
                "View / {} inspector",
                if self.inspector_open { "Hide" } else { "Show" }
            ),
            shortcut: String::new(),
            enabled: true,
            target: Target::Inspector,
        });
        entries.push(Entry {
            id: "view:preview".into(),
            label: format!(
                "View / {} Preview",
                if self.preview_settings.visible {
                    "Hide"
                } else {
                    "Show"
                }
            ),
            shortcut: String::new(),
            enabled: true,
            target: Target::Preview,
        });
        for (path, label, available) in self.recent_file_entries() {
            let id = format!("recent:{}", path.to_string_lossy());
            if seen.insert(id.clone()) {
                // Do not probe potentially unavailable network paths on every UI
                // frame. The existing explicit Open action owns file validation.
                entries.push(Entry {
                    id,
                    label: format!("File / Recent / {label}"),
                    shortcut: String::new(),
                    enabled: available,
                    target: Target::Recent(path),
                });
            }
        }
        entries
    }

    pub(super) fn open_command_search(&mut self) {
        self.command_search = CommandSearch {
            open: true,
            focus: true,
            document: Some(self.doc.editor_id.runtime_key()),
            tab: self.active_tab,
            layer: self
                .doc
                .layers
                .get(self.layer)
                .map(|l| l.editor_id.runtime_key()),
            frame: self.doc.frame_export_id(self.frame),
            ..Default::default()
        };
    }

    fn command_search_context_current(&self) -> bool {
        self.command_search.document == Some(self.doc.editor_id.runtime_key())
            && self.command_search.tab == self.active_tab
            && self.command_search.layer
                == self
                    .doc
                    .layers
                    .get(self.layer)
                    .map(|l| l.editor_id.runtime_key())
            && self.command_search.frame == self.doc.frame_export_id(self.frame)
    }

    fn execute_catalog_command(&mut self, ctx: &egui::Context, id: &str) -> bool {
        if self.command_search.open && !self.command_search_context_current() {
            self.command_search.open = false;
            self.status = "Document changed; reopen command search".into();
            return false;
        }
        let Some(entry) = self
            .command_catalog()
            .into_iter()
            .find(|entry| entry.id == id && entry.enabled)
        else {
            return false;
        };
        self.command_search.open = false;
        match entry.target {
            Target::Action(action) => self.action(action),
            Target::Theme(preference) => {
                self.set_theme_preference(ctx, preference);
                self.save_theme();
            }
            Target::Density(comfortable) => {
                self.set_editor_density(comfortable);
            }
            Target::Recent(path) => self.open_recent(path),
            Target::Inspector => self.inspector_open = !self.inspector_open,
            Target::Preview => self.toggle_preview_visibility(),
        }
        true
    }

    pub(super) fn show_command_search(&mut self, ctx: &egui::Context) {
        if !self.command_search.open {
            return;
        }
        if !self.command_search_context_current() {
            self.command_search.open = false;
            self.status = "Document changed; reopen command search".into();
            return;
        }
        let cancel = ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));
        if cancel {
            self.command_search.open = false;
            ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("command-query")));
            return;
        }
        let navigation = ctx.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown) {
                1
            } else if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp) {
                -1
            } else {
                0
            }
        });
        let enter = ctx.memory(|memory| memory.focused() == Some(egui::Id::new("command-query")))
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter));
        let entries = self.command_catalog();
        let mut activate = None;
        let dismiss = fluent::modal_dialog(ctx, "Search commands", |ui| {
            let focus = std::mem::take(&mut self.command_search.focus);
            let query = ui.add(
                egui::TextEdit::singleline(&mut self.command_search.query)
                    .id(egui::Id::new("command-query"))
                    .hint_text("Type a command or action ID")
                    .desired_width(f32::INFINITY),
            );
            if focus {
                query.request_focus();
            }
            #[cfg(test)]
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("command-query-bounds"), query.rect)
            });
            let filtered: Vec<_> = entries
                .iter()
                .filter(|entry| matches_query(&entry.label, &entry.id, &self.command_search.query))
                .collect();
            if focus || query.changed() {
                self.command_search.selected = filtered
                    .iter()
                    .find(|entry| entry.enabled)
                    .or_else(|| filtered.first())
                    .map(|entry| entry.id.clone());
            } else if self
                .command_search
                .selected
                .as_ref()
                .is_some_and(|selected| !filtered.iter().any(|entry| &entry.id == selected))
            {
                self.command_search.selected = None;
            }
            if navigation != 0 && !filtered.is_empty() {
                let position =
                    self.command_search.selected.as_ref().and_then(|selected| {
                        filtered.iter().position(|entry| &entry.id == selected)
                    });
                let next = position.map_or(
                    if navigation > 0 {
                        0
                    } else {
                        filtered.len() - 1
                    },
                    |position| {
                        (position as isize + navigation).rem_euclid(filtered.len() as isize)
                            as usize
                    },
                );
                self.command_search.selected = Some(filtered[next].id.clone());
            }
            ui.small("Up/Down choose · Enter run · Escape close");
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 180.).clamp(60., 360.))
                .show(ui, |ui| {
                    if filtered.is_empty() {
                        ui.weak("No matching commands");
                    }
                    for entry in filtered {
                        let selected = self.command_search.selected.as_ref() == Some(&entry.id);
                        let label = if entry.shortcut.is_empty() {
                            entry.label.clone()
                        } else {
                            format!("{}   {}", entry.label, entry.shortcut)
                        };
                        let response = ui.add_enabled(
                            entry.enabled,
                            egui::Button::new(label)
                                .selected(selected)
                                .wrap()
                                .min_size(Vec2::new(ui.available_width(), 28.)),
                        );
                        let reason = match entry.target {
                            Target::Action(action) => self.native_action_disabled_reason(action),
                            Target::Recent(_) if !entry.enabled => {
                                Some("This recent file is unavailable")
                            }
                            _ => None,
                        };
                        let response = if let Some(reason) = reason {
                            ui.weak(reason);
                            response.on_disabled_hover_text(reason)
                        } else {
                            response
                        };
                        #[cfg(test)]
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new(("command-entry", entry.id.clone())),
                                response.rect,
                            )
                        });
                        if selected && navigation != 0 {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            activate = Some(entry.id.clone());
                        }
                    }
                });
            if enter {
                activate = self.command_search.selected.clone();
            }
            false
        });
        if dismiss {
            self.command_search.open = false;
            activate = None;
        }
        if let Some(id) = activate {
            self.execute_catalog_command(ctx, &id);
        }
        if !self.command_search.open {
            ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("command-query")));
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor_with_context(ctx: &egui::Context) -> PixelCraft {
        let mut editor = PixelCraft::with_context(ctx, None);
        editor.replace(Document::new(8, 8), None);
        editor
    }
    fn editor() -> PixelCraft {
        editor_with_context(&egui::Context::default())
    }
    fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn draw(
        editor: &mut PixelCraft,
        ctx: &egui::Context,
        mut events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100., 800.))),
                events,
                ..Default::default()
            },
            |ui| editor.render(ui),
        );
        output.textures_delta.clear();
        output
    }

    #[test]
    fn disabled_search_results_explain_the_current_action_constraint() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.doc.layers[0].locked = true;
        for (action, reason) in [
            ("undo", "Nothing to undo"),
            ("deleteframe", "A sprite must keep at least one frame"),
            ("clear", "The active layer is locked or not editable"),
            ("crop", "Select an area first"),
        ] {
            editor.open_command_search();
            editor.command_search.query = format!("action:{action}");
            let output = draw(&mut editor, &ctx, vec![]);
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains(reason))
            }), "missing disabled explanation for {action}: {reason}");
        }
    }

    #[test]
    fn grouped_catalog_exposes_export_selection_and_tool_destinations() {
        let editor = editor();
        let entries = editor.command_catalog();
        for (id, path) in [
            ("action:pcx", "File / Export / Export PCX…"),
            ("action:sequence", "File / Import / Import PNG sequence…"),
            ("action:sheet", "File / Export sprite sheet…"),
            (
                "action:export-selection-mask",
                "Select / Mask / Export mask PNG…",
            ),
            ("action:repeat-export", "File / Repeat Last Export"),
            ("action:selectall", "Select / Select all"),
            ("action:rotate", "Select / Transform / Rotate selection 90°"),
            ("action:studio", "Tools / Drawing & selection studio…"),
            ("action:extensions", "Tools / Asset extensions…"),
            (
                "action:flipx",
                "Frame / Cel transform / Flip active cel horizontally",
            ),
        ] {
            let entry = entries.iter().find(|entry| entry.id == id).unwrap();
            assert_eq!(entry.label, path, "wrong destination for {id}");
        }
    }

    #[test]
    fn rendered_search_finds_legacy_routes_after_menu_reorganization() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        for (query, expected) in [
            ("Edit select all", "action:selectall"),
            ("Sprite flip active cel horizontally", "action:flipx"),
            ("Extensions asset", "action:extensions"),
            ("grid import tag batch atlas", "action:workflow"),
        ] {
            editor.open_command_search();
            editor.command_search.query = query.into();
            draw(&mut editor, &ctx, vec![]);
            assert_eq!(editor.command_search.selected.as_deref(), Some(expected));
        }
    }

    #[test]
    fn recent_activation_revalidates_a_removed_file_without_changing_document() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        let path =
            std::env::temp_dir().join(format!("pixelcraft-recent-{}.pixel", std::process::id()));
        formats::save(&path, &editor.doc).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while editor.recent_files.availability(&path) == recent_files::Availability::Checking {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            editor.recent_files.availability(&path),
            recent_files::Availability::Available
        );
        std::fs::remove_file(&path).unwrap();
        let before = editor.doc.clone();
        editor.open_recent(path);
        while editor.open_job.is_some() {
            editor.poll_open_document();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(editor.doc == before);
        assert!(editor.path.is_none());
    }
    #[test]
    fn playback_holds_the_search_target_and_resumes_without_catching_up() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.doc.add_frame(0, false);
        editor.playing = true;
        editor.last_tick = Instant::now() - Duration::from_secs(2);
        editor.open_command_search();
        draw(&mut editor, &ctx, vec![]);
        assert!(editor.playing);
        assert_eq!(editor.frame, 0);
        assert!(editor.command_search.open);
        assert!(editor.last_tick.elapsed() < Duration::from_secs(1));
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(editor.playing);
        assert_eq!(editor.frame, 0);
    }

    #[test]
    fn search_open_and_cancel_preserve_curve_and_floating_transform() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        draw(&mut editor, &ctx, vec![]);
        editor.tool = Tool::Curve;
        for point in [(1, 1), (2, 3), (4, 2)] {
            editor.curve_click(CanvasPane::Primary, point);
        }
        assert!(editor.curve_draft.is_some());
        editor.action("command-search");
        assert!(
            editor.curve_draft.is_some(),
            "opening search discarded a curve draft"
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(editor.curve_draft.is_some());
        editor.cancel_gesture();
        editor.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        editor.start_canvas_transform();
        assert!(editor.canvas_transform.is_some());
        editor.action("command-search");
        assert!(
            editor.canvas_transform.is_some(),
            "opening search discarded a floating transform"
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(editor.canvas_transform.is_some());
    }
    #[test]
    fn locked_flip_dispatch_keeps_redo_and_history() {
        let mut editor = editor();
        editor.remember();
        editor.doc.add_layer();
        editor.undo();
        editor.doc.layers[0].locked = true;
        let before = editor.doc.clone();
        let undo = editor.undo.len();
        let redo = editor.redo.len();
        assert!(redo > 0);
        editor.action("flipx");
        assert!(editor.doc == before);
        assert_eq!(editor.undo.len(), undo);
        assert_eq!(editor.redo.len(), redo);
    }
    #[test]
    fn search_rejects_changed_layer_or_frame_in_same_document() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.doc.add_layer();
        editor.open_command_search();
        editor.layer = 1;
        assert!(!editor.execute_catalog_command(&ctx, "action:deletelayer"));
        editor.doc.add_frame(0, false);
        editor.open_command_search();
        editor.frame = 1;
        assert!(!editor.execute_catalog_command(&ctx, "action:deleteframe"));
    }
    #[test]
    fn search_enter_respects_close_button_focus() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.open_command_search();
        editor.command_search.query = "action:layer".into();
        draw(&mut editor, &ctx, vec![]);
        ctx.memory_mut(|m| {
            m.request_focus(egui::Id::new(("editor-dialog", "Search commands")).with("close"))
        });
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(!editor.command_search.open);
        assert_eq!(editor.doc.layers.len(), 1);
    }
    #[test]
    fn search_retains_native_color_actions_menu_aliases_and_shortcut_aliases() {
        let mut editor = editor();
        editor
            .custom_shortcuts
            .insert("Alt+R".into(), "rect".into());
        let entries = editor.command_catalog();
        for action in ["swap", "resetcolors", "savepalette", "loadpalette"] {
            assert!(
                entries.iter().any(|e| e.id == format!("action:{action}")),
                "missing {action}"
            );
        }
        assert!(
            entries
                .iter()
                .find(|e| e.id == "action:rectangle")
                .unwrap()
                .shortcut
                .contains("Alt+R")
        );
        assert!(
            entries
                .iter()
                .filter(|e| e.id == "action:workflow")
                .any(|e| matches_query(&e.label, &e.id, "references"))
        );
    }

    #[test]
    fn rendered_command_search_keyboard_query_escape_and_activation_preserve_native_history() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.path = Some(PathBuf::from("original.pixel"));
        editor.tool = Tool::Eraser;
        editor.selection = Some((0, 0, 1, 1));
        let before = editor.doc.clone();
        draw(&mut editor, &ctx, vec![]);
        draw(
            &mut editor,
            &ctx,
            vec![key(
                Key::P,
                egui::Modifiers {
                    command: true,
                    ctrl: !cfg!(target_os = "macos"),
                    mac_cmd: cfg!(target_os = "macos"),
                    shift: true,
                    ..Default::default()
                },
            )],
        );
        assert!(
            editor.command_search.open,
            "default shortcut opens the modal"
        );
        draw(
            &mut editor,
            &ctx,
            vec![
                egui::Event::Text("action:layer".into()),
                key(Key::B, egui::Modifiers::NONE),
            ],
        );
        assert_eq!(editor.command_search.query, "action:layer");
        assert!(editor.tool == Tool::Eraser);
        assert_eq!(editor.doc, before);
        assert_eq!(editor.selection, Some((0, 0, 1, 1)));
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(!editor.command_search.open);
        assert_eq!(editor.selection, Some((0, 0, 1, 1)));
        assert!(editor.undo.is_empty());
        assert_eq!(editor.doc, before);
        editor.open_command_search();
        draw(&mut editor, &ctx, vec![]);
        draw(
            &mut editor,
            &ctx,
            vec![egui::Event::Text("action:layer".into())],
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(!editor.command_search.open);
        assert_eq!(editor.doc.layers.len(), 2);
        assert_eq!(editor.undo.len(), 1);
        assert_eq!(editor.path, Some(PathBuf::from("original.pixel")));
        editor.undo();
        assert_eq!(editor.doc, before);
    }

    #[test]
    fn command_search_rejects_disabled_unknown_and_obsolete_document_commands() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        let before = editor.doc.clone();
        editor.open_command_search();
        assert!(!editor.execute_catalog_command(&ctx, "action:undo"));
        assert!(!editor.execute_catalog_command(&ctx, "action:run-lua"));
        assert!(!editor.execute_catalog_command(&ctx, "theme:not-a-theme"));
        assert_eq!(editor.doc, before);
        assert!(editor.undo.is_empty());
        editor.open_document(Document::new(3, 3), None);
        let replacement = editor.doc.clone();
        assert!(!editor.execute_catalog_command(&ctx, "action:layer"));
        assert_eq!(editor.doc, replacement);
        assert!(editor.undo.is_empty());
    }

    #[test]
    fn command_search_arrow_navigation_and_parameterized_activation_use_current_bindings() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.open_command_search();
        draw(&mut editor, &ctx, vec![]);
        draw(&mut editor, &ctx, vec![egui::Event::Text("theme:".into())]);
        assert_eq!(
            editor.command_search.selected.as_deref(),
            Some("theme:system")
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::ArrowDown, egui::Modifiers::NONE)],
        );
        assert_eq!(
            editor.command_search.selected.as_deref(),
            Some("theme:light")
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::ArrowDown, egui::Modifiers::NONE)],
        );
        assert_eq!(
            editor.command_search.selected.as_deref(),
            Some("theme:dark")
        );
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(!editor.command_search.open);
        assert_eq!(editor.theme_preference, egui::ThemePreference::Dark);
        assert!(editor.undo.is_empty());
    }

    fn click(position: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn command_search_pointer_activation_and_disabled_rows_do_not_leak_to_canvas() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        editor.zoom = 32.;
        editor.pan = Vec2::new(160., 0.);
        let before = editor.doc.clone();
        editor.open_command_search();
        editor.command_search.query = "action:undo".into();
        draw(&mut editor, &ctx, vec![]);
        let disabled = ctx
            .data(|data| {
                data.get_temp::<Rect>(egui::Id::new(("command-entry", "action:undo".to_string())))
            })
            .unwrap()
            .center();
        draw(&mut editor, &ctx, click(disabled));
        assert!(editor.command_search.open);
        assert_eq!(editor.doc, before);
        assert!(editor.undo.is_empty());
        draw(
            &mut editor,
            &ctx,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        editor.open_command_search();
        editor.command_search.query = "action:layer".into();
        draw(&mut editor, &ctx, vec![]);
        let enabled = ctx
            .data(|data| {
                data.get_temp::<Rect>(egui::Id::new(("command-entry", "action:layer".to_string())))
            })
            .unwrap()
            .center();
        draw(&mut editor, &ctx, click(enabled));
        assert!(!editor.command_search.open);
        assert_eq!(editor.doc.layers.len(), 2);
        assert_eq!(editor.undo.len(), 1);
        editor.undo();
        assert_eq!(editor.doc, before);
        editor.open_command_search();
        draw(&mut editor, &ctx, vec![]);
        let pane = ctx
            .data(|data| data.get_temp::<Rect>(egui::Id::new("canvas-bounds")))
            .unwrap();
        let point = pane.center() + editor.pan;
        draw(&mut editor, &ctx, click(point));
        assert_eq!(editor.doc, before);
        assert!(editor.start.is_none());
    }

    #[test]
    fn closing_command_search_wins_over_simultaneous_enter_activation() {
        let ctx = egui::Context::default();
        let mut editor = editor_with_context(&ctx);
        let before = editor.doc.clone();
        editor.open_command_search();
        editor.command_search.query = "action:layer".into();
        draw(&mut editor, &ctx, vec![]);
        let close = ctx
            .data(|data| data.get_temp::<Rect>(egui::Id::new(("dialog-close", "Search commands"))))
            .unwrap()
            .center();
        let mut events = click(close);
        events.push(key(Key::Enter, egui::Modifiers::NONE));
        draw(&mut editor, &ctx, events);
        assert!(!editor.command_search.open);
        assert_eq!(editor.doc, before);
        assert!(editor.undo.is_empty());
    }

    #[test]
    fn native_catalog_includes_unbound_menu_commands_and_parameterized_variants() {
        let editor = editor();
        let commands = editor.command_catalog();
        let unbound = commands
            .iter()
            .find(|entry| entry.id == "action:png-sequence")
            .expect("menu commands without shortcuts must be discoverable");
        assert!(unbound.shortcut.is_empty());
        assert!(unbound.label.contains("PNG"));
        let themes: Vec<_> = commands
            .iter()
            .filter(|entry| entry.id.starts_with("theme:"))
            .collect();
        assert_eq!(themes.len(), 3);
        assert_eq!(
            commands
                .iter()
                .filter(|entry| entry.id.starts_with("density:"))
                .count(),
            2
        );
        let ids: std::collections::HashSet<_> = commands.iter().map(|entry| &entry.id).collect();
        assert_eq!(ids.len(), commands.len());
    }

    #[test]
    fn command_search_normalizes_case_whitespace_and_matches_ids() {
        assert!(matches_query(
            "File / Export PNG sequence…",
            "action:png-sequence",
            "  EXPORT   png "
        ));
        assert!(matches_query(
            "File / Export PNG sequence…",
            "action:png-sequence",
            "PNG-SEQUENCE"
        ));
        assert!(!matches_query(
            "File / Export PNG sequence…",
            "action:png-sequence",
            "painting-layer"
        ));
    }

    #[test]
    fn catalog_rebuilds_bindings_and_disabled_state_from_current_editor() {
        let mut editor = editor();
        assert!(
            !editor
                .command_catalog()
                .iter()
                .find(|entry| entry.id == "action:undo")
                .expect("undo is listed")
                .enabled
        );
        editor.remember();
        editor.doc.set_pixel(0, 0, 0, [2, 3, 4, 255]);
        editor
            .custom_shortcuts
            .insert("Ctrl+Alt+G".into(), "png-sequence".into());
        let commands = editor.command_catalog();
        assert!(
            commands
                .iter()
                .find(|entry| entry.id == "action:undo")
                .unwrap()
                .enabled
        );
        assert!(
            commands
                .iter()
                .find(|entry| entry.id == "action:png-sequence")
                .unwrap()
                .shortcut
                .contains("Alt")
        );
        editor.doc.layers[0].locked = true;
        assert!(
            !editor
                .command_catalog()
                .iter()
                .find(|entry| entry.id == "action:clear")
                .unwrap()
                .enabled
        );
    }
}
