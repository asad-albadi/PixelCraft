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
fn paint_outline(ui: &egui::Ui, rect: Rect, radius: u8, width: f32) {
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(width + 1., ui.visuals().panel_fill),
        egui::StrokeKind::Outside,
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(width, ui.visuals().text_color()),
        egui::StrokeKind::Outside,
    );
}

fn paint_focus(ui: &egui::Ui, response: &egui::Response, radius: u8) {
    if response.enabled() && response.has_focus() {
        paint_outline(ui, response.rect.expand(1.), radius, 1.5);
    }
}

pub(super) fn modal_dialog(
    ctx: &egui::Context,
    title: &str,
    content: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    let id = egui::Id::new(("editor-dialog", title));
    let offset_id = id.with("offset");
    let offset = ctx.data(|data| data.get_temp::<Vec2>(offset_id).unwrap_or_default());
    let popup_open = egui::Popup::is_any_open(ctx);
    let modal = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).anchor(egui::Align2::CENTER_CENTER, offset))
        .frame(egui::Frame::window(&ctx.style_of(ctx.theme())))
        .backdrop_color(Color32::from_black_alpha(80))
        .show(ctx, |ui| {
            ui.set_width(360_f32.min((ctx.content_rect().width() - 32.).max(100.)));
            let (header, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.), Sense::hover());
            let close_rect = Rect::from_min_size(
                Pos2::new(header.right() - 34., header.top()),
                Vec2::splat(34.),
            );
            let close = ui.interact(close_rect, id.with("close"), Sense::click());
            close.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, close.enabled(), "Close dialog")
            });
            let visuals = ui.style().interact(&close);
            let fill = if close.is_pointer_button_down_on() {
                ui.visuals()
                    .selection
                    .bg_fill
                    .lerp_to_gamma(ui.visuals().text_color(), 0.16)
            } else if close.hovered() {
                visuals.bg_fill
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(close_rect.shrink(2.), 5, fill);
            paint_focus(ui, &close, 5);
            ui.painter().text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::proportional(20.),
                visuals.fg_stroke.color,
            );
            let close = close
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Close dialog");
            let title_rect = header.with_max_x(close_rect.left() - 8.);
            let drag = ui
                .interact(
                    header.with_max_x(close_rect.left()),
                    id.with("header"),
                    Sense::drag(),
                )
                .on_hover_and_drag_cursor(egui::CursorIcon::Grab);
            ui.painter()
                .with_clip_rect(title_rect.intersect(ui.clip_rect()))
                .text(
                    title_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::TextStyle::Heading.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
            ui.separator();
            let content_closed = content(ui);
            (close.clicked(), drag.drag_delta(), content_closed)
        });
    let (close, drag, content_closed) = modal.inner;
    if drag != Vec2::ZERO {
        let bounds = ctx.content_rect();
        let limit = ((bounds.size() - modal.response.rect.size()) * 0.5).max(Vec2::ZERO);
        let offset = (modal.response.rect.center() - bounds.center() + drag).clamp(-limit, limit);
        ctx.data_mut(|data| data.insert_temp(offset_id, offset));
        ctx.request_repaint();
    }
    let dismiss = close || (modal.is_top_modal && !popup_open && modal.should_close());
    if dismiss || content_closed {
        ctx.data_mut(|data| data.remove::<Vec2>(offset_id));
        ctx.request_repaint();
    }
    dismiss
}

fn heading(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add_space(4.);
    let id = ui.make_persistent_id(("inspector-section", text));
    let mut expanded = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(true));
    let background = ui.painter().add(egui::Shape::Noop);
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
    if response.enabled() {
        let fill = if response.is_pointer_button_down_on() {
            ui.visuals()
                .selection
                .bg_fill
                .lerp_to_gamma(ui.visuals().text_color(), 0.16)
        } else if response.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .set(background, egui::Shape::rect_filled(response.rect, 4, fill));
    }
    paint_focus(ui, &response, 4);
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
        let fill = if response.enabled() && response.is_pointer_button_down_on() {
            fill.lerp_to_gamma(Color32::BLACK, 0.45)
        } else if response.enabled() && response.hovered() {
            fill.lerp_to_gamma(ui.visuals().text_color(), 0.24)
        } else {
            fill
        };
        ui.painter().rect_filled(r.shrink(2.), 12, fill);
        paint_focus(ui, &response, 12);
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

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct PreviewSettings {
    pub visible: bool,
    pub height: f32,
    // Zero is Fit; all other values are physical pixels per sprite pixel.
    pub zoom: u8,
}
impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            visible: true,
            height: 240.,
            zoom: 0,
        }
    }
}

enum PreviewTime {
    Playing(f64),
    Paused(f64),
}

pub(super) struct PreviewPlayback {
    frame: usize,
    time: PreviewTime,
    speed: u16,
}

impl PreviewPlayback {
    fn new(frame: usize, now: f64, durations: &[u16], speed: u16) -> Self {
        Self {
            frame,
            time: PreviewTime::Playing(now + playback_seconds(durations[frame], speed)),
            speed,
        }
    }

    pub(super) fn set_speed(&mut self, now: f64, speed: u16) {
        let ratio = f64::from(self.speed) / f64::from(speed);
        self.time = match self.time {
            PreviewTime::Playing(deadline) => {
                PreviewTime::Playing(now + (deadline - now).max(0.) * ratio)
            }
            PreviewTime::Paused(remaining) => PreviewTime::Paused(remaining.max(0.) * ratio),
        };
        self.speed = speed;
    }

    fn toggle(&mut self, now: f64) {
        self.time = match self.time {
            PreviewTime::Playing(deadline) => PreviewTime::Paused(deadline - now),
            PreviewTime::Paused(remaining) => PreviewTime::Playing(now + remaining),
        };
    }

    fn update(&mut self, now: f64, durations: &[u16]) {
        if self.frame >= durations.len() {
            self.frame = durations.len() - 1;
            let duration = playback_seconds(durations[self.frame], self.speed);
            self.time = match self.time {
                PreviewTime::Playing(_) => PreviewTime::Playing(now + duration),
                PreviewTime::Paused(_) => PreviewTime::Paused(duration),
            };
        }
        if let PreviewTime::Playing(deadline) = &mut self.time
            && now >= *deadline
        {
            // Skip entire cycles after a delayed repaint, then find the current cel.
            let cycle_ms: u64 = durations.iter().map(|d| u64::from((*d).max(1))).sum();
            let cycle = cycle_ms as f64 / (10. * f64::from(self.speed));
            *deadline += ((now - *deadline) / cycle).floor() * cycle;
            while now >= *deadline {
                self.frame = (self.frame + 1) % durations.len();
                *deadline += playback_seconds(durations[self.frame], self.speed);
            }
        }
    }
}

fn harmony_color([red, green, blue, alpha]: Pixel, degrees: f32) -> Pixel {
    // Use straight sRGB channels for the hue wheel, independently of opacity.
    let mut hsv =
        egui::ecolor::Hsva::from_rgb([red, green, blue].map(|channel| f32::from(channel) / 255.));
    hsv.h = (hsv.h + degrees / 360.).rem_euclid(1.);
    let [red, green, blue] = hsv.to_rgb().map(|channel| (channel * 255.).round() as u8);
    [red, green, blue, alpha]
}

impl PixelCraft {
    pub(super) fn restore_preview_settings(&mut self, path: PathBuf) {
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(mut settings) = serde_json::from_slice::<PreviewSettings>(&bytes)
        {
            if !settings.height.is_finite() {
                settings.height = 240.;
            }
            settings.height = settings.height.clamp(120., 4096.);
            if ![0, 1, 2, 3, 4, 8].contains(&settings.zoom) {
                settings.zoom = 0;
            }
            self.preview_settings = settings;
        }
        self.preview_settings_path = Some(path);
    }
    fn save_preview_settings(&mut self) {
        if let Some(path) = &self.preview_settings_path {
            let result = std::fs::create_dir_all(path.parent().unwrap()).and_then(|()| {
                std::fs::write(path, serde_json::to_vec(&self.preview_settings).unwrap())
            });
            if let Err(error) = result {
                self.status = format!("Preview preference could not be saved: {error}");
            }
        }
    }

    pub(super) fn toggle_preview_visibility(&mut self) {
        self.preview_settings.visible = !self.preview_settings.visible;
        self.save_preview_settings();
    }

    pub(super) fn inspector_toggle(&mut self, ui: &mut egui::Ui) {
        let response = ui
            .small_button(if self.inspector_open { "‹" } else { "›" })
            .on_hover_text(if self.inspector_open {
                "Hide inspector"
            } else {
                "Show inspector"
            });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                if self.inspector_open {
                    "Hide inspector"
                } else {
                    "Show inspector"
                },
            )
        });
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("inspector-toggle"), response.rect));
        if response.clicked() {
            self.inspector_open = !self.inspector_open;
        }
    }

    pub(super) fn inspector_contents(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong("Inspector");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.inspector_toggle(ui);
            });
        });
        ui.separator();
        if self.preview_settings.visible {
            self.docked_preview(ui);
        }
        egui::ScrollArea::vertical()
            .id_salt("inspector_scroll")
            .show(ui, |ui| {
                self.palette(ui);
                ui.separator();
                self.fluent_properties(ui);
            });
    }

    pub(super) fn docked_preview(&mut self, ui: &mut egui::Ui) {
        // Keep the original bottom-left preview separate from scrolling controls.
        let max_height = (ui.available_height() - 36.).max(120.);
        let preview_height = self.preview_settings.height.clamp(120., max_height);
        let preview = egui::Panel::bottom("sidebar_preview")
            .exact_size(preview_height)
            .resizable(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let (handle, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.), Sense::drag());
                let response = response
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                    .on_hover_text("Drag to resize Preview");
                ui.painter().line_segment(
                    [handle.left_center(), handle.right_center()],
                    Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
                if response.dragged() {
                    self.preview_settings.height =
                        (preview_height - response.drag_delta().y).clamp(120., max_height);
                    ui.ctx().request_repaint();
                }
                if response.drag_stopped() {
                    self.save_preview_settings();
                }
                self.sprite_preview(ui);
            });
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::new("pinned-preview-bounds"),
                preview.response.rect,
            );
        });
        let _ = preview;
    }

    fn document_palette(&mut self, ui: &mut egui::Ui) {
        #[derive(Clone, Copy)]
        struct Swatch {
            tab: usize,
            slot: usize,
        }

        let mut reorder = None;
        ui.collapsing("Document palette", |ui| {
            ui.small("Drag a swatch onto another to move it.");
            let palette = if self.doc.color_mode == ColorMode::Indexed {
                self.doc.editing_palette(self.layer, self.frame)
            } else {
                self.doc.palette_for(self.frame).to_vec()
            };
            let side = 22.;
            let spacing = ui.spacing().item_spacing.x;
            let columns = ((ui.available_width() + spacing) / (side + spacing))
                .floor()
                .max(1.) as usize;
            egui::ScrollArea::vertical()
                .id_salt("document-palette-scroll")
                .max_height(174.)
                .show(ui, |ui| {
                    egui::Grid::new("document-palette-grid")
                        .min_col_width(0.)
                        .show(ui, |ui| {
                            for (slot, &[r, g, b, a]) in palette.iter().enumerate() {
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::splat(side),
                                    Sense::click_and_drag(),
                                );
                                #[cfg(test)]
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(egui::Id::new(("document-swatch", slot)), rect)
                                });
                                response.dnd_set_drag_payload(Swatch {
                                    tab: self.active_tab,
                                    slot,
                                });
                                let dragging =
                                    response
                                        .dnd_hover_payload::<Swatch>()
                                        .is_some_and(|swatch| {
                                            swatch.tab == self.active_tab && swatch.slot != slot
                                        });
                                if let Some(swatch) = response.dnd_release_payload::<Swatch>()
                                    && swatch.tab == self.active_tab
                                    && swatch.slot != slot
                                {
                                    reorder = Some((swatch.slot, slot));
                                }
                                let color = [r, g, b, a];
                                if response.clicked() {
                                    self.fg = color;
                                    self.palette_index = slot;
                                }
                                let selected = self.palette_index == slot && self.fg == color;
                                let name = self
                                    .doc
                                    .palette_names
                                    .get(slot)
                                    .filter(|name| !name.is_empty())
                                    .map(String::as_str);
                                let label = name.map_or_else(
                                    || {
                                        format!(
                                            "Swatch {} · #{r:02X}{g:02X}{b:02X}{a:02X}",
                                            slot + 1
                                        )
                                    },
                                    |name| format!("Swatch {} · {name}", slot + 1),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::SelectableLabel,
                                        response.enabled(),
                                        selected,
                                        &label,
                                    )
                                });
                                response.context_menu(|ui| {
                                    for (label, target) in [
                                        ("Move earlier", slot.checked_sub(1)),
                                        (
                                            "Move later",
                                            (slot + 1 < palette.len()).then_some(slot + 1),
                                        ),
                                    ] {
                                        if let Some(target) = target
                                            && ui.pc_button(label).clicked()
                                        {
                                            reorder = Some((slot, target));
                                            ui.close();
                                        }
                                    }
                                });
                                let painter = ui.painter();
                                if a < 255 {
                                    painter.rect_filled(rect, 3, Color32::GRAY);
                                    painter.line_segment(
                                        [rect.left_bottom(), rect.right_top()],
                                        Stroke::new(1., Color32::WHITE),
                                    );
                                }
                                painter.rect_filled(
                                    rect,
                                    3,
                                    Color32::from_rgba_unmultiplied(r, g, b, a),
                                );
                                painter.rect_stroke(
                                    rect,
                                    3,
                                    if selected || dragging {
                                        Stroke::new(2., self.colors().accent)
                                    } else {
                                        Stroke::new(
                                            1.,
                                            ui.visuals().widgets.inactive.bg_stroke.color,
                                        )
                                    },
                                    egui::StrokeKind::Inside,
                                );
                                paint_focus(ui, &response, 3);
                                response.on_hover_text(label);
                                if (slot + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });
        });
        if let Some((from, to)) = reorder {
            self.move_palette_swatch(from, to);
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
                        ui.menu_button("Color harmonies", |ui| {
                            for (label, angles) in [
                                ("Complementary", &[180.][..]),
                                ("Analogous", &[-30., 30.]),
                                ("Triadic", &[120., 240.]),
                                ("Split complementary", &[150., 210.]),
                            ] {
                                ui.menu_button(label, |ui| {
                                    for &angle in angles {
                                        let color = harmony_color(self.fg, angle);
                                        let hex = format!(
                                            "#{:02X}{:02X}{:02X}",
                                            color[0], color[1], color[2]
                                        );
                                        let swatch = Color32::from_rgba_unmultiplied(
                                            color[0], color[1], color[2], color[3],
                                        );
                                        if ui
                                            .horizontal(|ui| {
                                                ui.colored_label(swatch, "■");
                                                ui.pc_button(hex)
                                            })
                                            .inner
                                            .clicked()
                                        {
                                            self.set_active_color(color);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
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
                self.document_palette(ui);
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
                            if response.clicked() {
                                let selection_changed = self.fg != *p;
                                self.set_drawing_color(*p);
                                if selection_changed {
                                    ui.ctx().request_repaint();
                                }
                            }
                            let selected = *p == self.fg;
                            let mut name = format!("Use #{:02X}{:02X}{:02X}", p[0], p[1], p[2]);
                            if p[3] != 255 {
                                use std::fmt::Write;
                                let _ = write!(name, "{:02X}", p[3]);
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::SelectableLabel,
                                    response.enabled(),
                                    selected,
                                    &name,
                                )
                            });
                            ui.painter().rect_filled(
                                r,
                                5,
                                Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]),
                            );
                            let border = if selected {
                                Stroke::new(2., self.colors().accent)
                            } else {
                                Stroke::new(1., Color32::from_black_alpha(45))
                            };
                            ui.painter().rect_stroke(
                                r.shrink(0.5),
                                5,
                                border,
                                egui::StrokeKind::Inside,
                            );
                            if response.enabled() {
                                if response.is_pointer_button_down_on() {
                                    paint_outline(ui, r, 5, 2.);
                                } else if response.hovered() {
                                    paint_outline(ui, r, 5, 1.);
                                }
                            }
                            paint_focus(ui, &response, 5);
                            response.on_hover_text(name);
                            if i % 5 == 4 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }
    pub(super) fn sprite_preview(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 0)).show(ui, |ui| {
            let old_zoom = self.preview_settings.zoom;
            let now = ui.input(|input| input.time);
            if self.doc.durations.len() <= 1 {
                self.preview_playback = None;
            }
            if let Some(playback) = &mut self.preview_playback {
                playback.update(now, &self.doc.durations);
            }
            let controls = ui.horizontal(|ui| {
                ui.strong("Preview");
                egui::ComboBox::from_id_salt("preview_zoom")
                    .width(72.)
                    .selected_text(match old_zoom {
                        0 => "Fit".into(), 1 => "1:1".into(), n => format!("{n}×"),
                    })
                    .show_ui(ui, |ui| {
                        for (zoom, label) in [(0, "Fit"), (1, "1:1"), (2, "2×"),
                            (3, "3×"), (4, "4×"), (8, "8×")] {
                            ui.selectable_value(&mut self.preview_settings.zoom, zoom, label);
                        }
                    }).response.on_hover_text("Fit the whole sprite, or zoom in physical pixels. Drag artwork to pan.");
                let playing = self.preview_playback.as_ref()
                    .is_some_and(|playback| matches!(playback.time, PreviewTime::Playing(_)));
                let play = ui.add_enabled_ui(self.doc.durations.len() > 1, |ui| {
                    let (icon, label) = if playing {
                        ("pause", "Pause preview animation")
                    } else {
                        ("play", "Play preview animation")
                    };
                    ui_icons::local_icon_button(ui, icon, label, playing)
                }).inner;
                #[cfg(test)]
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("preview-play"), play.rect));
                if play.clicked() {
                    if let Some(playback) = &mut self.preview_playback {
                        playback.toggle(now);
                    } else {
                        self.preview_playback = Some(PreviewPlayback::new(self.frame, now, &self.doc.durations, self.playback_speed));
                    }
                }
            });
            if old_zoom != self.preview_settings.zoom {
                self.preview_pan = Vec2::ZERO;
                self.save_preview_settings();
            }
            let frame = if let Some(playback) = &self.preview_playback {
                if let PreviewTime::Playing(deadline) = playback.time {
                    ui.ctx().request_repaint_after(Duration::from_secs_f64((deadline - now).max(0.)));
                }
                playback.frame
            } else {
                self.frame
            };
            self.update_preview_texture(ui.ctx(), frame);
            let (r, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), ui.available_height().max(1.)), Sense::click_and_drag());
            let source = self.sprite_display_size();
            let scale = if self.preview_settings.zoom == 0 {
                (r.width() / source.x).min(r.height() / source.y)
            } else { self.preview_settings.zoom as f32 / ui.ctx().pixels_per_point() };
            let size = source * scale;
            let limit = ((size - r.size()) * 0.5).max(Vec2::ZERO);
            if response.dragged() && limit != Vec2::ZERO {
                self.preview_pan += response.drag_delta();
            }
            self.preview_pan = self.preview_pan.clamp(-limit, limit);
            let ppp = ui.ctx().pixels_per_point();
            let origin = r.center() + self.preview_pan - size * 0.5;
            let origin = if self.preview_settings.zoom == 0 { origin } else {
                Pos2::new((origin.x * ppp).round() / ppp, (origin.y * ppp).round() / ppp)
            };
            let image = Rect::from_min_size(origin, size);
            let response = response.on_hover_text("Click to center the canvas. Drag to pan the preview.");
            if let Some(picked) = response.interact_pointer_pos()
                && response.clicked()
                && image.contains(picked)
            {
                self.set_active_canvas_pan((image.center() - picked) * (self.active_canvas_view().zoom / scale));
                ui.ctx().request_repaint();
            }
            if limit != Vec2::ZERO {
                response.on_hover_cursor(egui::CursorIcon::Grab);
            }

            let painter = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
            // Paint only visible checker cells, regardless of sprite size or zoom.
            let cell = 8.;
            for y in 0..(r.height() / cell).ceil() as usize {
                for x in 0..(r.width() / cell).ceil() as usize {
                    painter.rect_filled(Rect::from_min_size(r.min + Vec2::new(x as f32, y as f32) * cell,
                        Vec2::splat(cell)), 0, if (x+y)%2 == 0 {
                            Color32::from_rgb(80,81,93)
                        } else { Color32::from_rgb(63,64,76) });
                }
            }
            if let Some(t) = &self.preview {
                painter.image(t.id(), image, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.,1.)), Color32::WHITE);
            }
            let camera = self.active_canvas_view();
            if let Some(canvas_size) = camera.size {
                let factor = scale / camera.zoom;
                let view = Rect::from_center_size(image.center() - camera.pan * factor, canvas_size * factor);
                let clip = r.intersect(image);
                if view.intersects(clip) {
                    // Clamp to the sprite so the navigator also shows an entirely visible sprite.
                    let border = view.intersect(image).shrink(0.5);
                    let painter = painter.with_clip_rect(clip);
                    painter.rect_stroke(border, 1, Stroke::new(3., Color32::BLACK), egui::StrokeKind::Inside);
                    painter.rect_stroke(border, 1, Stroke::new(1., Color32::WHITE), egui::StrokeKind::Inside);
                    #[cfg(test)]
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("preview-canvas-view"), border));
                }
            }
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("preview-viewport"), r);
                d.insert_temp(egui::Id::new("preview-image"), image);
                d.insert_temp(egui::Id::new("preview-controls"), controls.response.rect);
            });
            let _ = controls;
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
                        self.remember_action("Change layer opacity");
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
                        self.remember_action("Change layer blend mode");
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
        if self.tool == Tool::Fill {
            self.fill_controls(ui);
            toggle(ui, &mut self.grid, "Pixel grid");
            return;
        }
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
            self.remember_action("Change brush shape");
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
    fn shade_direction_controls(&mut self, ui: &mut egui::Ui) {
        if self.power.mode != pixelcraft::productivity::BrushMode::Shade
            || self.tool != Tool::Pencil
        {
            return;
        }
        for (reverse, icon, label, test_id) in [
            (
                true,
                "prev",
                "Shade toward previous palette color",
                "shade-previous",
            ),
            (
                false,
                "next",
                "Shade toward next palette color",
                "shade-next",
            ),
        ] {
            let response =
                ui_icons::local_icon_button(ui, icon, label, self.power.reverse == reverse);
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new(test_id), response.rect));
            let _ = test_id;
            if response.clicked() {
                self.power.reverse = reverse;
            }
        }
        ui.separator();
    }

    fn fluent_view_controls(&mut self, ui: &mut egui::Ui) {
        self.shade_direction_controls(ui);
        let current_zoom = self.active_canvas_view().zoom;
        let mut zoom = current_zoom;
        egui::ComboBox::from_id_salt("canvas_zoom")
            .width(90.)
            .selected_text(format!("{:.0}%", zoom * 100.))
            .show_ui(ui, |ui| {
                for scale in [0.25, 0.5, 1., 2., 4., 6., 8., 10., 12., 16., 24., 32.] {
                    ui.pc_selectable_value(&mut zoom, scale, format!("{:.0}%", scale * 100.));
                }
                ui.add(
                    egui::Slider::new(&mut zoom, self.minimum_canvas_zoom()..=32.)
                        .logarithmic(true)
                        .show_value(false),
                );
            });
        if zoom != current_zoom {
            self.set_active_canvas_zoom(zoom);
        }
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
                    if ui
                        .add_enabled_ui(self.can_repeat_export(), |ui| {
                            ui.pc_command_button("repeat-export", "Repeat Last Export")
                        })
                        .inner
                        .clicked()
                    {
                        self.action("repeat-export");
                        ui.close();
                    }
                    for (command, label) in [
                        ("png", "Export PNG…"),
                        ("sheet", "Sprite sheet…"),
                        ("png-sequence", "PNG sequence…"),
                        ("gif", "Animation GIF…"),
                        ("mp4", "Video MP4…"),
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
mod workflow_option_tests {
    use super::*;

    #[test]
    fn complementary_hue_preserves_alpha() {
        assert_eq!(harmony_color([255, 0, 0, 42], 180.), [0, 255, 255, 42]);
        assert_eq!(harmony_color([0, 0, 255, 255], 120.), [255, 0, 0, 255]);
        let opaque = harmony_color([255, 128, 0, 255], 180.);
        for alpha in [0, 1, 42] {
            let color = harmony_color([255, 128, 0, alpha], 180.);
            assert_eq!(&color[..3], &opaque[..3]);
            assert_eq!(color[3], alpha);
        }
    }

    #[test]
    fn changing_speed_rescales_both_live_and_paused_preview_timing() {
        let durations = [100, 100];
        let mut preview = PreviewPlayback::new(0, 0., &durations, 100);
        preview.set_speed(0.02, 200);
        preview.update(0.05, &durations);
        assert_eq!(preview.frame, 0);
        preview.update(0.061, &durations);
        assert_eq!(preview.frame, 1);
        preview.toggle(0.08);
        preview.set_speed(0.09, 50);
        preview.toggle(0.10);
        preview.update(0.218, &durations);
        assert_eq!(preview.frame, 1);
        preview.update(0.222, &durations);
        assert_eq!(preview.frame, 0);
        assert_eq!(durations, [100, 100]);
        assert_eq!(playback_seconds(100, 200), 0.05);
    }
}
