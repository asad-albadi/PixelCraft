//! Fluent presentation of existing editor state and commands.
use super::*;

pub(super) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "Fluent",
            include_bytes!("../assets/fonts/NotoSans-Regular.ttf").as_slice(),
        ),
        (
            "Fluent bold",
            include_bytes!("../assets/fonts/NotoSans-Bold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Fluent".into());
    fonts.families.insert(
        egui::FontFamily::Name("Fluent bold".into()),
        vec!["Fluent bold".into()],
    );
    ctx.set_fonts(fonts);
}
fn heading(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add_space(4.);
    let id = ui.make_persistent_id(("inspector-section", text));
    let mut expanded = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(true));
    let row = ui.horizontal(|ui| {
        ui.label(
            RichText::new(text)
                .size(16.)
                .family(egui::FontFamily::Name("Fluent bold".into())),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let icon = ui_icons::texture(
                ui.ctx(),
                if expanded {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                16.,
            );
            ui.add(egui::Image::new((icon.id(), Vec2::splat(16.))).tint(ui.visuals().text_color()));
        });
    });
    let response = ui.interact(row.response.rect, id, Sense::click());
    if response.clicked() {
        expanded = !expanded;
        ui.ctx().data_mut(|d| d.insert_persisted(id, expanded));
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::CollapsingHeader,
            ui.is_enabled(),
            expanded,
            text,
        )
    });
    ui.add_space(4.);
    expanded
}

pub(super) fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        let (r, mut response) = ui.allocate_exact_size(Vec2::new(48., 26.), Sense::click());
        if response.clicked() {
            *value = !*value;
            response.mark_changed();
        }
        let fill = if *value {
            Color32::from_rgb(0, 133, 133)
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        ui.painter().rect_filled(r.shrink(2.), 12, fill);
        ui.painter().circle_filled(
            Pos2::new(
                if *value {
                    r.right() - 13.
                } else {
                    r.left() + 13.
                },
                r.center().y,
            ),
            8.,
            Color32::WHITE,
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
        });
        response
    })
    .inner
}

impl PixelCraft {
    pub(super) fn inspector_contents(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        // Keep the original bottom-left preview separate from scrolling controls.
        let preview_height = (ui.available_width() * 0.44).clamp(100., 160.) + 40.;
        let preview = egui::Panel::bottom("sidebar_preview")
            .exact_size(preview_height)
            .resizable(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.sprite_preview(ui));
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("pinned-preview-bounds"),
                preview.response.rect,
            );
        });
        let _ = preview;
        egui::ScrollArea::vertical()
            .id_salt("inspector_scroll")
            .show(ui, |ui| {
                self.palette(ui);
                ui.separator();
                self.fluent_properties(ui);
            });
    }

    pub(super) fn fluent_resize(&self, ui: &mut egui::Ui) {
        if ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let r = ui.ctx().content_rect();
        for (index, rect, direction, cursor) in [
            (
                0,
                Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + 4.)),
                egui::ResizeDirection::North,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                1,
                Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 4.), r.max),
                egui::ResizeDirection::South,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                2,
                Rect::from_min_max(r.min, Pos2::new(r.left() + 4., r.bottom())),
                egui::ResizeDirection::West,
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                3,
                Rect::from_min_max(Pos2::new(r.right() - 4., r.top()), r.max),
                egui::ResizeDirection::East,
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                4,
                Rect::from_min_size(r.max - Vec2::splat(10.), Vec2::splat(10.)),
                egui::ResizeDirection::SouthEast,
                egui::CursorIcon::ResizeNwSe,
            ),
        ] {
            let response = ui
                .interact(rect, egui::Id::new(("window-resize", index)), Sense::drag())
                .on_hover_cursor(cursor);
            if response.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
    }
    pub(super) fn fluent_titlebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("fluent_title")
            .exact_size(if self.power.comfortable() { 36. } else { 30. })
            .frame(
                egui::Frame::NONE
                    .fill(if self.dark_mode {
                        Color32::from_rgb(39, 43, 50)
                    } else {
                        Color32::from_rgb(226, 234, 241)
                    })
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let (mark, _) = ui.allocate_exact_size(Vec2::splat(22.), Sense::hover());
                    for (y, row) in BRAND_MARK.iter().enumerate() {
                        for (x, c) in row.bytes().enumerate() {
                            if c != b'.' {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        mark.min + Vec2::new(x as f32 * 3., y as f32 * 3.),
                                        Vec2::splat(3.),
                                    ),
                                    0,
                                    if c == b'g' {
                                        Color32::from_rgb(241, 190, 93)
                                    } else {
                                        Color32::from_rgb(0, 149, 150)
                                    },
                                );
                            }
                        }
                    }
                    ui.label(
                        RichText::new("PixelCraft")
                            .family(egui::FontFamily::Name("Fluent bold".into())),
                    );
                    let inspector_toggle = ui.small_button("☰").on_hover_text("Toggle inspector");
                    #[cfg(test)]
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("inspector-toggle"), inspector_toggle.rect)
                    });
                    if inspector_toggle.clicked() {
                        self.inspector_open = !self.inspector_open;
                    }
                    let space = (ui.available_width() - 120.).max(0.);
                    let (drag, response) =
                        ui.allocate_exact_size(Vec2::new(space, 24.), Sense::click_and_drag());
                    if response.drag_started() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    if response.double_clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    let _ = drag;
                    for (label, command) in [
                        ("−", egui::ViewportCommand::Minimized(true)),
                        ("□", egui::ViewportCommand::Maximized(!maximized)),
                        ("×", egui::ViewportCommand::Close),
                    ] {
                        if ui
                            .add_sized([32., 24.], egui::Button::new(label).frame(false))
                            .clicked()
                        {
                            if label == "×" {
                                self.close_window(ui.ctx());
                            } else {
                                ui.ctx().send_viewport_cmd(command);
                            }
                        }
                    }
                });
            });
    }
    fn close_window(&mut self, ctx: &egui::Context) {
        if self.dirty() {
            self.pending = Some(Pending::Close);
        } else {
            self.finish_pending(ctx, Pending::Close);
        }
    }
    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                if !heading(ui, "Colors") {
                    return;
                }
                let narrow = ui.available_width() < 300.;
                ui.horizontal(|ui| {
                    let mut selected = self.active_color;
                    egui::ComboBox::from_id_salt("named_colors")
                        .width(if narrow { 90. } else { 150. })
                        .selected_text(&self.named_colors[selected].0)
                        .show_ui(ui, |ui| {
                            for (index, (name, _)) in self.named_colors.iter().enumerate() {
                                ui.pc_selectable_value(&mut selected, index, name);
                            }
                        });
                    if selected != self.active_color {
                        self.select_named_color(selected);
                    }
                    ui.scope(|ui| {
                        ui.spacing_mut().interact_size = Vec2::new(32., 28.);
                        if ui
                            .color_edit_button_srgba_unmultiplied(&mut self.fg)
                            .changed()
                        {
                            self.set_active_color(self.fg);
                        }
                    });
                    if !narrow {
                        ui.add(egui::Label::new(format!(
                            "#{:02X}{:02X}{:02X}",
                            self.fg[0], self.fg[1], self.fg[2]
                        )));
                    }
                });
                if narrow {
                    ui.label(format!(
                        "#{:02X}{:02X}{:02X}",
                        self.fg[0], self.fg[1], self.fg[2]
                    ));
                }
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    if ui_icons::local_button(ui, "plus", "Add color").clicked() {
                        self.add_named_color();
                    }
                    ui.menu_button("…", |ui| {
                        ui.label("Color name");
                        ui.text_edit_singleline(&mut self.named_colors[self.active_color].0);
                        if self.named_colors.len() > 1
                            && ui.pc_small_button("Delete color").clicked()
                        {
                            self.named_colors.remove(self.active_color);
                            self.select_named_color(
                                self.active_color.min(self.named_colors.len() - 1),
                            );
                            ui.close();
                        }
                        if ui
                            .pc_command_button("savepalette", "Save palette…")
                            .clicked()
                        {
                            self.studio_action("savepalette");
                            ui.close();
                        }
                        if ui
                            .pc_command_button("loadpalette", "Load palette…")
                            .clicked()
                        {
                            self.studio_action("loadpalette");
                            ui.close();
                        }
                    });
                });
                ui.add_space(6.);
                ui.label("From artwork");
                let colors = self.sprite_colors();
                if colors.is_empty() {
                    ui.small("Draw to add colors here.");
                }
                let side = ((ui.available_width() - 16.) / 5.).clamp(20., 30.);
                egui::Grid::new("sprite_colors")
                    .min_col_width(0.)
                    .spacing(Vec2::splat(4.))
                    .show(ui, |ui| {
                        for (i, p) in colors.iter().enumerate() {
                            let (r, response) =
                                ui.allocate_exact_size(Vec2::splat(side), Sense::click());
                            ui.painter().rect_filled(
                                r,
                                5,
                                Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]),
                            );
                            ui.painter().rect_stroke(
                                r.shrink(0.5),
                                5,
                                Stroke::new(
                                    if *p == self.fg { 2. } else { 1. },
                                    if *p == self.fg {
                                        self.colors().accent
                                    } else {
                                        Color32::from_black_alpha(45)
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            if response.clicked() {
                                self.set_drawing_color(*p);
                            }
                            response.on_hover_text(format!(
                                "Use #{:02X}{:02X}{:02X}",
                                p[0], p[1], p[2]
                            ));
                            if i % 5 == 4 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }
    pub(super) fn sprite_preview(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                if !heading(ui, "Preview") {
                    return;
                }
                self.preview = Some(self.frame_texture(ui.ctx(), self.frame));
                let side = (ui.available_width() * 0.44).clamp(100., 160.);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - side) * 0.5);
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                    for y in 0..6 {
                        for x in 0..6 {
                            let q = Rect::from_min_size(
                                r.min + Vec2::new(x as f32, y as f32) * side / 6.,
                                Vec2::splat(side / 6.),
                            );
                            ui.painter().rect_filled(
                                q,
                                0,
                                if (x + y) % 2 == 0 {
                                    Color32::from_rgb(80, 81, 93)
                                } else {
                                    Color32::from_rgb(63, 64, 76)
                                },
                            );
                        }
                    }
                    let size = Vec2::new(self.doc.width as f32, self.doc.height as f32)
                        * (side / self.doc.width.max(self.doc.height) as f32);
                    if let Some(t) = &self.preview {
                        ui.painter().image(
                            t.id(),
                            Rect::from_center_size(r.center(), size),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                            Color32::WHITE,
                        );
                    }
                });
            });
    }
    pub(super) fn fluent_properties(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                if !heading(ui, "Properties") {
                    return;
                }
                let mut opacity = self.doc.layers[self.layer].opacity;
                ui.horizontal(|ui| {
                    ui.label("Opacity");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{}%", opacity as u32 * 100 / 255));
                    });
                });
                ui.spacing_mut().slider_width = ui.available_width() - 10.;
                ui.scope(|ui| {
                    ui.visuals_mut().selection.bg_fill = self.colors().accent;
                    ui.visuals_mut().widgets.inactive.bg_fill = self.colors().accent;
                    ui.visuals_mut().widgets.inactive.corner_radius = egui::CornerRadius::same(12);
                    if ui
                        .add(egui::Slider::new(&mut opacity, 0..=255).show_value(false))
                        .changed()
                    {
                        self.remember();
                        self.doc.layers[self.layer].opacity = opacity;
                    }
                });
                ui.add_space(4.);
                ui.horizontal(|ui| {
                    ui.label("Blend");
                    let mut blend = self.doc.layers[self.layer].blend;
                    egui::ComboBox::from_id_salt("inspector_blend")
                        .width((ui.available_width() - 20.).max(90.))
                        .selected_text(format!("{blend:?}"))
                        .show_ui(ui, |ui| {
                            for b in BlendMode::ALL {
                                ui.pc_selectable_value(&mut blend, b, format!("{b:?}"));
                            }
                        });
                    if blend != self.doc.layers[self.layer].blend {
                        self.remember();
                        self.doc.layers[self.layer].blend = blend;
                    }
                });
                ui.add_space(8.);
                if ui.pc_button("Properties & color mode…").clicked() {
                    self.properties_open = true;
                }
            });
    }
    pub(super) fn fluent_context(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("context")
            .exact_size(if self.power.comfortable() { 44. } else { 38. })
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Secondary brush settings remain reachable in overflow, so
                    // primary view actions never get squeezed off the window.
                    if ui.available_width() < 1050. {
                        ui.menu_button("Drawing…", |ui| {
                            ui.vertical(|ui| self.fluent_brush_controls(ui));
                        });
                    } else {
                        let width = (ui.available_width() - 310.).max(80.);
                        ui.allocate_ui_with_layout(
                            Vec2::new(width, 30.),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                egui::ScrollArea::horizontal()
                                    .id_salt("context_drawing_controls")
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.horizontal_centered(|ui| self.fluent_brush_controls(ui));
                                    });
                            },
                        );
                    }
                    self.fluent_view_controls(ui);
                });
            });
    }
    fn fluent_brush_controls(&mut self, ui: &mut egui::Ui) {
        let short = !self.power.comfortable();
        let tool_icon = match self.tool {
            Tool::Pencil => "pencil",
            Tool::Eraser => "eraser",
            Tool::Fill => "fill",
            Tool::Picker => "eyedropper",
            Tool::Line => "line",
            Tool::Rect => "rectangle",
            Tool::Ellipse => "ellipse",
            Tool::Select => "select",
            Tool::Hand => "hand",
            Tool::Lasso => "lasso",
            Tool::Move => "move",
            Tool::Gradient => "gradient",
            Tool::Wand => "wand",
        };
        egui::containers::menu::MenuButton::from_button(
            egui::Button::new((
                egui::Image::new((
                    ui_icons::texture(ui.ctx(), tool_icon, 20.).id(),
                    Vec2::splat(20.),
                )),
                self.tool.name(),
                egui::Atom::grow(),
                egui::Image::new((
                    ui_icons::texture(ui.ctx(), "chevron-down", 16.).id(),
                    Vec2::splat(16.),
                )),
            ))
            .image_tint_follows_text_color(true)
            .min_size(Vec2::new(
                if short { 100. } else { 120. },
                if short { 28. } else { 32. },
            )),
        )
        .ui(ui, |ui| self.tools(ui));
        ui.add_sized(
            [if short { 50. } else { 90. }, if short { 28. } else { 32. }],
            egui::DragValue::new(&mut self.brush)
                .range(MIN_BRUSH_SIZE..=MAX_BRUSH_SIZE)
                .suffix(" px"),
        );
        let mut shape = self.doc.brush_shape;
        egui::ComboBox::from_id_salt("brush_shape")
            .width(90.)
            .selected_text(format!("{shape:?}"))
            .show_ui(ui, |ui| {
                for v in [
                    BrushShape::Round,
                    BrushShape::Square,
                    BrushShape::Diamond,
                    BrushShape::Horizontal,
                    BrushShape::Vertical,
                ] {
                    ui.pc_selectable_value(&mut shape, v, format!("{v:?}"));
                }
            });
        if shape != self.doc.brush_shape {
            self.remember();
            self.doc.brush_shape = shape;
        }
        self.brush_mode_ui(ui);
        ui.separator();
        toggle(ui, &mut self.studio.mirror_x, "Symmetry").on_hover_text(
            "Mirror drawing horizontally; vertical symmetry is in Sprite Properties.",
        );
        ui.separator();
        toggle(ui, &mut self.grid, "Pixel grid");
        if matches!(self.tool, Tool::Rect | Tool::Ellipse) {
            ui.pc_checkbox(&mut self.filled, "Filled");
        }
    }
    fn fluent_view_controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("canvas_zoom")
            .width(90.)
            .selected_text(format!("{:.0}%", self.zoom * 100.))
            .show_ui(ui, |ui| {
                for zoom in [0.25, 0.5, 1., 2., 4., 6., 8., 10., 12., 16., 24., 32.] {
                    ui.pc_selectable_value(&mut self.zoom, zoom, format!("{:.0}%", zoom * 100.));
                }
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.02..=32.)
                        .logarithmic(true)
                        .show_value(false),
                );
            });
        if ui.pc_command_button("fit", "Fit").clicked() {
            self.action("fit");
        }
        ui.scope(|ui| {
            ui.visuals_mut().widgets.inactive.weak_bg_fill = self.colors().accent;
            ui.visuals_mut().widgets.inactive.fg_stroke.color = Color32::WHITE;
            ui.menu_button(
                (
                    egui::Image::new((
                        ui_icons::texture(ui.ctx(), "export", 20.).id(),
                        Vec2::splat(20.),
                    ))
                    .tint(Color32::WHITE),
                    RichText::new("Export").color(Color32::WHITE),
                ),
                |ui| {
                    for (command, label) in [
                        ("png", "Export PNG…"),
                        ("sheet", "Sprite sheet…"),
                        ("gif", "Animation GIF…"),
                    ] {
                        if ui.pc_command_button(command, label).clicked() {
                            self.action(command);
                            ui.close();
                        }
                    }
                },
            );
        });
        if ui
            .pc_small_button("…")
            .on_hover_text("Advanced drawing tools")
            .clicked()
        {
            self.power.open = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_close_checks_all_tabs_for_unsaved_artwork() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
        app.open_document(Document::new(8, 8), None);
        app.close_window(&ctx);
        assert_eq!(app.active_tab, 0);
        assert!(matches!(app.pending, Some(Pending::Close)));
        assert!(app.dirty());
    }
}
