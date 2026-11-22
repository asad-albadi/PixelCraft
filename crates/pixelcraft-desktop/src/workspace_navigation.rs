//! Workspace recovery is separate from document and tool preferences.
use super::*;

impl PixelCraft {
    fn navigation_chord_available(&self, chord: &str, action: &str) -> bool {
        let chord = ShortcutChord::parse(chord);
        let mut matches = self
            .custom_shortcuts
            .iter()
            .filter(|(binding, _)| ShortcutChord::parse(binding) == chord);
        matches.next().is_some_and(|(_, target)| target == action)
            && matches.all(|(_, target)| target == action)
    }
    pub(super) fn settings_shortcut_available(&self) -> bool {
        self.shortcut_edit.is_none() && self.navigation_chord_available("Ctrl+Comma", "settings")
    }
    /// Opening a destination does not cancel an unfinished canvas operation.
    pub(super) fn navigation_action(&mut self, action: &str) -> bool {
        if self.open_settings_destination(action)
            || self.open_workflow_destination(action)
            || self.open_property_destination(action)
        {
            return true;
        }
        match action {
            "settings" => self.settings_open = true,
            "shortcuts" => self.shortcut_settings_open = true,
            "extensions" => {
                self.refresh_extensions();
                self.extensions_open = true;
            }
            "help" => self.help = true,
            "about" => self.about = true,
            "reset-layout" => {
                self.reset_layout_requested = true;
                self.inspector_open = true;
                self.preview_settings = fluent::PreviewSettings::default();
                self.preview_pan = Vec2::ZERO;
                self.retarget_curve_to_primary();
                if self.gesture_canvas == Some(CanvasPane::Secondary) {
                    self.gesture_canvas = Some(CanvasPane::Primary);
                }
                self.split_canvas = None;
                self.active_canvas = CanvasPane::Primary;
                self.save_preview_settings();
                self.status = "Restored Inspector, bottom-left Preview and single canvas".into();
            }
            "reset-interface-scale" => {
                // Applied at the beginning of render, before laying out controls.
                self.reset_scale_requested = true;
            }
            _ => return false,
        }
        true
    }

    pub(super) fn navigation_shortcuts(&mut self, ctx: &egui::Context) {
        // Shortcut capture owns every chord, including native recovery defaults.
        if self.shortcut_edit.is_some() {
            return;
        }
        let recovery_available =
            self.navigation_chord_available("Ctrl+Shift+0", "reset-interface-scale");
        let settings_available = self.settings_shortcut_available();
        let exact_press = |chord: &str| {
            ctx.input(|input| ShortcutChord::parse(chord).is_some_and(|chord| chord.matches(input)))
        };
        if recovery_available
            && exact_press("Ctrl+Shift+0")
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Num0)
            })
        {
            self.reset_interface_scale(ctx);
        }
        if settings_available
            && exact_press("Ctrl+Comma")
            && (!self.dialog_open() || self.settings_open)
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Comma))
        {
            self.action("settings");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_navigation_defaults_remain_removed_after_reload_and_render() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.custom_shortcuts
            .retain(|_, a| a != "settings" && a != "reset-interface-scale");
        app.custom_shortcuts = crate::api::productivity::load_shortcuts(
            &crate::api::productivity::save_shortcuts(&app.custom_shortcuts).unwrap(),
        )
        .unwrap();
        ctx.set_zoom_factor(2.);
        for (key, modifiers) in [
            (Key::Comma, egui::Modifiers::COMMAND),
            (Key::Num0, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100., 800.))),
                    events: vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            assert!(!app.settings_open);
            assert_eq!(ctx.zoom_factor(), 2.);
        }
        assert!(!app.settings_shortcut_available());
    }
    #[test]
    fn playback_holds_properties_until_the_user_closes_them() {
        for action in ["properties", "layer-properties", "frame-properties"] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.doc.add_frame(0, false);
            app.action("play");
            app.action(action);
            app.last_tick = Instant::now() - Duration::from_secs(2);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100., 800.))),
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            assert!(app.properties_open, "{action} closed during playback");
            assert_eq!(app.frame, 0);
            assert!(app.playing);
        }
    }

    #[test]
    fn navigation_defaults_do_not_steal_custom_chords_with_extra_modifiers() {
        for (binding, key, modifiers) in [
            (
                "Ctrl+Alt+Comma",
                Key::Comma,
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
            ),
            (
                "Ctrl+Shift+Comma",
                Key::Comma,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            ),
            (
                "Ctrl+Alt+Shift+0",
                Key::Num0,
                egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT,
            ),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.replace(Document::new(16, 16), None);
            app.custom_shortcuts.insert(binding.into(), "frame".into());
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    app.navigation_shortcuts(ui.ctx());
                    app.shortcuts(ui.ctx());
                },
            );
            output.textures_delta.clear();
            assert!(!app.settings_open, "navigation stole {binding}");
            assert_eq!(app.doc.durations.len(), 2, "navigation stole {binding}");
        }
    }

    #[test]
    fn layout_recovery_restores_panels_without_editing_artwork_or_history() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(16, 16), None);
        app.doc.layers[0].cels[0][0] = [35, 78, 102, 255];
        app.remember_action("Before layout reset");
        let before = serde_json::to_vec(&app.doc).unwrap();
        let history = app.undo.len();
        app.inspector_open = false;
        app.preview_settings.visible = false;
        app.preview_settings.height = 1000.;
        app.preview_settings.zoom = 8;
        app.preview_pan = Vec2::new(45., 70.);
        app.action("split-canvas");
        app.action("reset-layout");
        assert!(app.inspector_open);
        assert!(app.preview_settings.visible);
        assert_eq!(app.preview_settings.height, 240.);
        assert_eq!(app.preview_settings.zoom, 0);
        assert_eq!(app.preview_pan, Vec2::ZERO);
        assert!(app.split_canvas.is_none());
        assert_eq!(serde_json::to_vec(&app.doc).unwrap(), before);
        assert_eq!(app.undo.len(), history);
        assert!(app.power.bridge.is_none());
    }

    #[test]
    fn navigation_keeps_unfinished_curve_and_floating_transform() {
        for action in [
            "settings",
            "settings-recovery",
            "workflow",
            "references",
            "assistants",
            "properties",
            "layer-properties",
            "frame-properties",
            "tileset-properties",
            "reset-layout",
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.replace(Document::new(16, 16), None);
            app.doc.set_pixel(0, 0, 0, [255, 20, 10, 255]);
            app.start_canvas_transform();
            app.curve_click(CanvasPane::Primary, (1, 1));
            let before = serde_json::to_vec(&app.doc).unwrap();
            let undo = app.undo.len();
            app.action(action);
            assert!(
                app.canvas_transform.is_some(),
                "{action} discarded transform"
            );
            assert!(app.curve_draft.is_some(), "{action} discarded curve");
            assert_eq!(serde_json::to_vec(&app.doc).unwrap(), before);
            assert_eq!(app.undo.len(), undo);
            assert!(app.power.bridge.is_none());
        }
    }

    #[test]
    fn layout_recovery_restores_resized_panel_dimensions() {
        use egui::containers::panel::PanelState;
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let draw = |app: &mut PixelCraft| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920., 1080.))),
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
        };
        draw(&mut app);
        ctx.data_mut(|data| {
            data.insert_persisted(
                egui::Id::new("palette_panel"),
                PanelState {
                    outer_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(400., 800.)),
                },
            );
            data.insert_persisted(
                egui::Id::new("timeline"),
                PanelState {
                    outer_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1500., 400.)),
                },
            );
        });
        draw(&mut app);
        assert!(
            PanelState::load(&ctx, egui::Id::new("palette_panel"))
                .unwrap()
                .size()
                .x
                > 390.
        );
        assert!(
            PanelState::load(&ctx, egui::Id::new("timeline"))
                .unwrap()
                .size()
                .y
                > 390.
        );
        app.action("reset-layout");
        for _ in 0..3 {
            draw(&mut app);
        }
        assert_eq!(
            PanelState::load(&ctx, egui::Id::new("palette_panel"))
                .unwrap()
                .size()
                .x,
            264.
        );
        assert_eq!(
            PanelState::load(&ctx, egui::Id::new("timeline"))
                .unwrap()
                .size()
                .y,
            200.
        );
    }

    #[test]
    fn layout_recovery_keeps_secondary_curve_drawable() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(16, 16), None);
        app.action("split-canvas");
        app.stroke_color = [25, 180, 240, 255];
        app.curve_click(CanvasPane::Secondary, (1, 1));
        app.action("reset-layout");
        app.curve_click(CanvasPane::Primary, (8, 8));
        assert!(app.curve_draft.is_some());
        app.curve_click(CanvasPane::Primary, (2, 7));
        app.curve_click(CanvasPane::Primary, (7, 2));
        assert!(app.curve_draft.is_none());
        assert!(!app.undo.is_empty());
        assert!(app.doc.composite(0).iter().any(|pixel| *pixel != CLEAR));
    }

    #[test]
    fn navigation_chords_preserve_preexisting_custom_assignments() {
        for (binding, key, shift, action) in [
            ("Ctrl+Comma", Key::Comma, false, "layer"),
            ("Ctrl+Shift+0", Key::Num0, true, "frame"),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.replace(Document::new(16, 16), None);
            app.custom_shortcuts.insert(binding.into(), action.into());
            let modifiers = egui::Modifiers::COMMAND
                | if shift {
                    egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::NONE
                };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    app.navigation_shortcuts(ui.ctx());
                    app.shortcuts(ui.ctx());
                },
            );
            output.textures_delta.clear();
            assert!(!app.settings_open);
            assert_eq!(
                if shift {
                    app.doc.durations.len()
                } else {
                    app.doc.layers.len()
                },
                2,
                "lost legacy binding {binding}"
            );
        }
    }

    #[test]
    fn recovery_shortcut_works_while_settings_has_text_focus() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(16, 16), None);
        app.settings_open = true;
        ctx.set_zoom_factor(3.);
        let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key {
                        key: Key::Num0,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let mut text = String::from("typing");
                ui.text_edit_singleline(&mut text).request_focus();
                app.navigation_shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        let mut settled = ctx.run_ui(egui::RawInput::default(), |_| {});
        settled.textures_delta.clear();
        assert_eq!(ctx.zoom_factor(), 1.);
        assert!(app.settings_open);
        assert!(app.power.bridge.is_none());
    }
}
