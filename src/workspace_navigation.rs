//! Workspace recovery is separate from document and tool preferences.
use super::*;

impl PixelCraft {
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
        // Recovery defaults must not steal a valid binding saved by an older
        // version. AppKit's standard Settings shortcut is owned by the OS.
        let available = |chord: &str, action: &str| {
            self.custom_shortcuts.iter().all(|(binding, target)| {
                ShortcutChord::parse(binding) != ShortcutChord::parse(chord) || target == action
            })
        };
        let recovery_available = available("Ctrl+Shift+0", "reset-interface-scale");
        let settings_available = cfg!(target_os = "macos") || available("Ctrl+Comma", "settings");
        if recovery_available
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Num0)
            })
        {
            self.reset_interface_scale(ctx);
        }
        if settings_available
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
            if cfg!(target_os = "macos") && key == Key::Comma {
                continue;
            }
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
