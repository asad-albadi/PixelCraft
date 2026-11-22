//! Rendering for the same grouped catalog used by command search.
use super::*;

/// Menus can be taller than the logical viewport at large interface scales.
/// Reserve room for the menu bar and popup margins, and let every row scroll.
pub(crate) fn scroll_menu<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let max_height = (ui.ctx().content_rect().height() - 48.).max(32.);
    ui.set_max_height(max_height);
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .show(ui, content)
        .inner
}

impl PixelCraft {
    pub(crate) fn menu_items(
        &self,
        ui: &mut egui::Ui,
        items: &[command_search::MenuItem],
        action: &mut Option<&'static str>,
    ) {
        use command_search::MenuItem;
        for item in items {
            match *item {
                MenuItem::Separator => {
                    ui.separator();
                }
                MenuItem::Submenu(label, children) => {
                    let response = ui
                        .menu_button(label, |ui| {
                            scroll_menu(ui, |ui| self.menu_items(ui, children, action))
                        })
                        .response;
                    if response.gained_focus() {
                        response.scroll_to_me(None);
                    }
                }
                MenuItem::Action(label, a) => {
                    let selected = match a {
                        "grid" => self.grid,
                        "split-canvas" => self.split_canvas.is_some(),
                        "drawing-grid" => self.show_drawing_grid,
                        "snap-to-grid" => self.snap_to_grid,
                        "onion" => self.onion,
                        "rewind-on-stop" => self.rewind_on_stop(),
                        "play" => self.playing,
                        "seamless" => self.studio.seamless == WrapAxes::BOTH,
                        "seamless-horizontal" => self.studio.seamless == WrapAxes::HORIZONTAL,
                        "seamless-vertical" => self.studio.seamless == WrapAxes::VERTICAL,
                        _ => false,
                    };
                    let icon = if a == "play" && self.playing {
                        "stop"
                    } else {
                        a
                    };
                    let reason = self.native_action_disabled_reason(a);
                    let enabled = reason.is_none();
                    let response = ui
                        .add_enabled_ui(enabled, |ui| {
                            ui_icons::button_as(ui, icon, a, label, selected, true)
                        })
                        .inner;
                    let response = if let Some(reason) = reason {
                        response.on_disabled_hover_text(reason)
                    } else {
                        response
                    };
                    if response.gained_focus() {
                        response.scroll_to_me(None);
                    }
                    #[cfg(test)]
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new(("menu-action", a)), response.id)
                    });
                    if response.clicked() {
                        *action = Some(a);
                        ui.close();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(
        editor: &PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, Option<&'static str>) {
        draw_with_size(editor, ctx, events, Vec2::new(1100., 1000.))
    }

    fn draw_with_size(
        editor: &PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        size: Vec2,
    ) -> (egui::FullOutput, Option<&'static str>) {
        draw_group(editor, ctx, events, size, 0)
    }

    fn draw_group(
        editor: &PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        size: Vec2,
        group: usize,
    ) -> (egui::FullOutput, Option<&'static str>) {
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button(command_search::MENU_GROUPS[group].0, |ui| {
                        scroll_menu(ui, |ui| {
                            editor.menu_items(ui, command_search::MENU_GROUPS[group].1, &mut action)
                        });
                    });
                });
            },
        );
        output.textures_delta.clear();
        (output, action)
    }

    fn position(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == label).then(|| text.pos + text.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let labels: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            Some(text.galley.text())
                        } else {
                            None
                        }
                    })
                    .collect();
                panic!("missing rendered menu label: {label}; rendered: {labels:?}")
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
    fn disabled_menu_hover_explains_why_undo_is_unavailable() {
        let ctx = egui::Context::default();
        let editor = PixelCraft::with_context(&ctx, None);
        ctx.all_styles_mut(|style| {
            style.interaction.tooltip_delay = 0.;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let draw = |events| draw_group(&editor, &ctx, events, Vec2::new(1100., 1000.), 1);
        let (output, _) = draw(vec![]);
        draw(click(position(&output, "Edit")));
        // Tooltips deliberately suppress the brief interval after a click.
        for _ in 0..15 {
            draw(vec![]);
        }
        let (output, _) = draw(vec![]);
        draw(vec![egui::Event::PointerMoved(position(&output, "Undo"))]);
        draw(vec![]);
        let (output, action) = draw(vec![]);
        position(&output, "Nothing to undo");
        assert!(action.is_none());
    }

    fn open_small_export(editor: &PixelCraft, ctx: &egui::Context) -> egui::FullOutput {
        let draw = |events| draw_with_size(editor, ctx, events, Vec2::new(683., 384.));
        let (output, _) = draw(vec![]);
        draw(click(position(&output, "File")));
        let (output, _) = draw(vec![]);
        draw(vec![
            egui::Event::PointerMoved(position(&output, "New sprite…")),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: Vec2::new(0., -2000.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let (output, _) = draw(vec![]);
        draw(click(position(&output, "Export")));
        draw(vec![]).0
    }

    #[test]
    fn small_viewport_keyboard_focus_reveals_last_export_before_activation() {
        let ctx = egui::Context::default();
        let editor = PixelCraft::with_context(&ctx, None);
        let draw = |events| draw_with_size(&editor, &ctx, events, Vec2::new(683., 384.));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        open_small_export(&editor, &ctx);
        let tga_id = ctx
            .data(|data| data.get_temp::<egui::Id>(egui::Id::new(("menu-action", "tga"))))
            .unwrap();
        for _ in 0..40 {
            if ctx.memory(|memory| memory.focused()) == Some(tga_id) {
                break;
            }
            draw(vec![key(Key::Tab)]);
        }
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(tga_id),
            "Tab must reach the final export"
        );
        let (output, _) = draw(vec![]);
        let tga = position(&output, "Export TGA…");
        assert!(
            (0.0..384.).contains(&tga.y),
            "focused export is outside viewport: {tga:?}"
        );
        let (_, action) = draw(vec![key(Key::Enter)]);
        assert_eq!(action, Some("tga"));
    }

    #[test]
    fn small_viewport_scrolls_to_last_export_and_activates_it() {
        fn draw(
            editor: &PixelCraft,
            ctx: &egui::Context,
            events: Vec<egui::Event>,
        ) -> (egui::FullOutput, Option<&'static str>) {
            draw_with_size(editor, ctx, events, Vec2::new(683., 384.))
        }
        let ctx = egui::Context::default();
        let editor = PixelCraft::with_context(&ctx, None);
        let output = open_small_export(&editor, &ctx);
        let top = position(&output, "Export baked ASE copy…");
        draw(
            &editor,
            &ctx,
            vec![
                egui::Event::PointerMoved(top),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: Vec2::new(0., -2000.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let (output, _) = draw(&editor, &ctx, vec![]);
        let tga = position(&output, "Export TGA…");
        assert!(
            (0.0..384.).contains(&tga.y),
            "last export remains outside viewport: {tga:?}"
        );
        let (_, action) = draw(&editor, &ctx, click(tga));
        assert_eq!(action, Some("tga"));
    }

    #[test]
    fn rendered_export_submenu_preserves_uncommon_format_action() {
        let ctx = egui::Context::default();
        let editor = PixelCraft::with_context(&ctx, None);
        let (output, _) = draw(&editor, &ctx, vec![]);
        let file = position(&output, "File");
        draw(&editor, &ctx, click(file));
        let (output, _) = draw(&editor, &ctx, vec![]);
        let export = position(&output, "Export");
        draw(&editor, &ctx, click(export));
        let (output, _) = draw(&editor, &ctx, vec![]);
        let pcx = position(&output, "Export PCX…");
        let (_, action) = draw(&editor, &ctx, click(pcx));
        assert_eq!(action, Some("pcx"));
    }
}
