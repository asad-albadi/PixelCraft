use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Appearance,
    Canvas,
    Animation,
    Color,
    Files,
    Shortcuts,
    Integrations,
    Updates,
}
impl Page {
    pub(crate) const ALL: [Self; 8] = [
        Self::Appearance,
        Self::Canvas,
        Self::Animation,
        Self::Color,
        Self::Files,
        Self::Shortcuts,
        Self::Integrations,
        Self::Updates,
    ];
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Canvas => "canvas",
            Self::Animation => "animation",
            Self::Color => "color",
            Self::Files => "files",
            Self::Shortcuts => "shortcuts",
            Self::Integrations => "integrations",
            Self::Updates => "updates",
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Canvas => "Canvas & Navigation",
            Self::Animation => "Animation",
            Self::Color => "Color Management",
            Self::Files => "Files & Recovery",
            Self::Shortcuts => "Keyboard Shortcuts",
            Self::Integrations => "Extensions & Automation",
            Self::Updates => "Updates & About",
        }
    }
    fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.id() == id)
            .unwrap_or(Self::Appearance)
    }
    fn has_defaults(self) -> bool {
        !matches!(self, Self::Shortcuts | Self::Integrations)
    }
}

/// Shared destinations for Settings and the editor's command search.
/// IDs remain stable; labels and aliases are presentation metadata.
pub(crate) struct SettingsEntry {
    pub(crate) id: &'static str,
    pub(crate) page: Page,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) aliases: &'static str,
}
impl SettingsEntry {
    pub(crate) fn path(&self) -> String {
        if self.id == format!("settings-{}", self.page.id()) {
            format!("Settings / {}", self.page.label())
        } else {
            format!("Settings / {} / {}", self.page.label(), self.label)
        }
    }
    fn matches(&self, query: &str) -> bool {
        let haystack = format!(
            "{} {} {} {}",
            self.label,
            self.description,
            self.aliases,
            self.path()
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}
macro_rules! entry {
    ($id:literal, $page:ident, $label:literal, $description:literal, $aliases:literal) => {
        SettingsEntry {
            id: $id,
            page: Page::$page,
            label: $label,
            description: $description,
            aliases: $aliases,
        }
    };
}
pub(crate) const SETTINGS_ENTRIES: &[SettingsEntry] = &[
    entry!(
        "settings-appearance",
        Appearance,
        "Appearance",
        "App theme, density and interface size",
        "style dark light system zoom"
    ),
    entry!(
        "settings-canvas",
        Canvas,
        "Canvas & Navigation",
        "Canvas display and wheel navigation preferences",
        "checkerboard pixel grid zoom trim"
    ),
    entry!(
        "settings-animation",
        Animation,
        "Animation",
        "Onion skin defaults and playback preferences",
        "previous next tint opacity rewind"
    ),
    entry!(
        "settings-color",
        Color,
        "Color Management",
        "Display and image import profile defaults",
        "ICC sRGB monitor policy"
    ),
    entry!(
        "settings-files",
        Files,
        "Files & Recovery",
        "Recovery and new-document defaults",
        "autosave default size width height"
    ),
    entry!(
        "settings-shortcuts",
        Shortcuts,
        "Keyboard Shortcuts",
        "Open the existing shortcut editor",
        "bindings hotkeys keys"
    ),
    entry!(
        "settings-integrations",
        Integrations,
        "Extensions & Automation",
        "Asset packages and active-editor assistant access",
        "plugins automation bridge MCP"
    ),
    entry!(
        "settings-updates",
        Updates,
        "Updates & About",
        "Update preferences and app information",
        "version download install"
    ),
    entry!(
        "settings-theme",
        Appearance,
        "Theme",
        "Follow System appearance or choose Light or Dark",
        "style colors"
    ),
    entry!(
        "settings-density",
        Appearance,
        "Density",
        "Choose Compact or Comfortable spacing",
        "padding spacing"
    ),
    entry!(
        "settings-interface-scale",
        Appearance,
        "Interface scale",
        "100% follows system display scaling",
        "DPI zoom font text size recovery reset interface scale"
    ),
    entry!(
        "settings-checkerboard",
        Canvas,
        "Checkerboard",
        "Transparency checker size and custom colors",
        "background squares transparency"
    ),
    entry!(
        "settings-pixel-grid",
        Canvas,
        "Pixel grid",
        "Grid color, opacity and fading at low zoom",
        "fade alpha"
    ),
    entry!(
        "settings-wheel-zoom",
        Canvas,
        "Whole-pixel zoom",
        "Wheel zoom and Preview Fit use whole pixels or exact divisors",
        "navigation scroll canvas"
    ),
    entry!(
        "settings-trim-canvas",
        Canvas,
        "Trim canvas content",
        "Clip drawing to the canvas bounds",
        "outside overflow"
    ),
    entry!(
        "settings-onion",
        Animation,
        "Onion skin defaults",
        "Previous and next frame counts, tint and opacity",
        "ghost animation frames"
    ),
    entry!(
        "settings-onion-next",
        Animation,
        "Next frames",
        "Number of following frames in onion skin",
        "ghost animation next count"
    ),
    entry!(
        "settings-onion-previous-tint",
        Animation,
        "Previous tint",
        "Tint for preceding onion frames",
        "ghost color previous"
    ),
    entry!(
        "settings-onion-next-tint",
        Animation,
        "Next tint",
        "Tint for following onion frames",
        "ghost color next"
    ),
    entry!(
        "settings-onion-previous-opacity",
        Animation,
        "Previous opacity",
        "Opacity for preceding onion frames",
        "ghost alpha previous"
    ),
    entry!(
        "settings-onion-next-opacity",
        Animation,
        "Next opacity",
        "Opacity for following onion frames",
        "ghost alpha next"
    ),
    entry!(
        "settings-checker-colors",
        Canvas,
        "Custom checker colors",
        "Enable custom transparency checkerboard colors",
        "background squares"
    ),
    entry!(
        "settings-pixel-grid-fade",
        Canvas,
        "Fade pixel grid at low zoom",
        "Automatically reduce grid opacity when zoomed out",
        "automatic alpha navigation"
    ),
    entry!(
        "settings-default-grid",
        Files,
        "Show pixel grid by default",
        "Initial pixel grid visibility for new documents",
        "new sprite default grid"
    ),
    entry!(
        "settings-new-sprite-height",
        Files,
        "New sprite height",
        "Default height in pixels for new documents",
        "new sprites dimensions size"
    ),
    entry!(
        "settings-rewind",
        Animation,
        "Rewind on stop",
        "Return playback to its start when stopped",
        "animation playback"
    ),
    entry!(
        "settings-display-profile",
        Color,
        "Display profile",
        "Choose sRGB, monitor or custom ICC display conversion",
        "color screen"
    ),
    entry!(
        "settings-import-profile",
        Color,
        "Image import profile",
        "Preserve, convert or assign sRGB to imported images",
        "color policy ICC"
    ),
    entry!(
        "settings-recovery",
        Files,
        "Enable recovery autosave",
        "Keep recovery copies of open documents",
        "autosave backup crash recover"
    ),
    entry!(
        "settings-new-sprites",
        Files,
        "New sprites",
        "Default size and pixel grid for newly created documents",
        "width height dimensions default documents"
    ),
    entry!(
        "settings-assets",
        Integrations,
        "Asset extensions",
        "Manage installed declarative asset packages",
        "plugins palettes brushes themes"
    ),
    entry!(
        "settings-assistant-access",
        Integrations,
        "Assistant access",
        "Open active-editor opt-in controls; opening this page never enables access",
        "automation bridge MCP headless token"
    ),
    entry!(
        "settings-background-updates",
        Updates,
        "Background updates",
        "Download and verify updates in the background",
        "automatic install release"
    ),
    entry!(
        "settings-about",
        Updates,
        "About PixelCraft",
        "App version and verified update installation",
        "help information release support"
    ),
];

#[derive(Default)]
pub(super) struct SettingsState {
    query: String,
    reveal: Option<&'static str>,
    confirm_reset: bool,
    return_from: Option<Page>,
    sidebar_layout: Option<(Page, u32)>,
}

fn settings_color_button(response: egui::Response, label: &str) -> egui::Response {
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ColorButton, response.enabled(), label)
    });
    if response.gained_focus() {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    response
}

impl Preferences {
    pub(super) fn reset_settings_page(&mut self, page: Page) {
        let d = Self::default();
        match page {
            Page::Appearance => {
                self.interface_scale = d.interface_scale;
                self.comfortable = d.comfortable;
            }
            Page::Canvas => {
                self.checker_size = d.checker_size;
                self.checker_colors = d.checker_colors;
                self.pixel_grid_color = d.pixel_grid_color;
                self.pixel_grid_auto_opacity = d.pixel_grid_auto_opacity;
                self.whole_pixel_wheel_zoom = d.whole_pixel_wheel_zoom;
                self.trim_canvas_content = d.trim_canvas_content;
            }
            Page::Animation => {
                self.onion = d.onion;
                self.rewind_on_stop = d.rewind_on_stop;
            }
            Page::Color => {
                self.import_profile = d.import_profile;
                self.display_profile = d.display_profile;
            }
            Page::Files => {
                self.autosave = d.autosave;
                self.new_sprite_width = d.new_sprite_width;
                self.new_sprite_height = d.new_sprite_height;
                self.default_grid = d.default_grid;
            }
            Page::Updates => self.background_updates = d.background_updates,
            Page::Shortcuts | Page::Integrations => {}
        }
    }
}

impl PixelCraft {
    pub(crate) fn reset_interface_scale(&mut self, ctx: &egui::Context) {
        self.power.preferences.interface_scale = 1.;
        ctx.set_zoom_factor(1.);
        self.save_workflow_preferences();
    }

    pub(crate) fn open_settings_destination(&mut self, id: &str) -> bool {
        let Some(entry) = SETTINGS_ENTRIES.iter().find(|entry| entry.id == id) else {
            return false;
        };
        self.power.preferences.last_settings_page = entry.page.id().into();
        self.power.settings.reveal = if id == format!("settings-{}", entry.page.id()) {
            None
        } else {
            Some(entry.id)
        };
        self.power.settings.query.clear();
        self.power.settings.confirm_reset = false;
        self.power.settings.return_from = None;
        self.power.settings.sidebar_layout = None;
        self.settings_open = true;
        self.save_workflow_preferences();
        true
    }

    pub(super) fn categorized_settings_dialog(&mut self, ctx: &egui::Context) {
        if let Some(page) = self.power.settings.return_from
            && !self.shortcut_settings_open
            && !self.extensions_open
            && !self.about
            && !self.power.open
        {
            self.power.settings.return_from = None;
            self.power.preferences.last_settings_page = page.id().into();
            self.settings_open = true;
        }
        if !self.settings_open {
            self.power.settings.sidebar_layout = None;
            return;
        }
        let closed = fluent::modal_dialog_sized(ctx, "Settings", 860., |ui| {
            self.settings_ui(ui);
            !self.settings_open
        });
        if closed {
            self.settings_open = false;
            self.power.settings.confirm_reset = false;
            self.power.settings.sidebar_layout = None;
        }
    }

    pub(super) fn categorized_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.small("Application preferences");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close = ui.pc_button("Done");
                #[cfg(test)]
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new("settings-done"), close.rect);
                });
                if close.clicked() {
                    self.settings_open = false;
                    self.power.settings.confirm_reset = false;
                }
            });
        });
        ui.horizontal_wrapped(|ui| {
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.power.settings.query)
                    .hint_text("Search settings")
                    .desired_width((ui.available_width() - 70.).max(70.)),
            );
            search.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Search settings")
            });
            if !self.power.settings.query.is_empty() && ui.pc_button("Clear").clicked() {
                self.power.settings.query.clear();
                search.request_focus();
            }
        });
        let recovery = ui.pc_button("Reset interface scale").on_hover_text(
            "Restore 100% interface size immediately. Document and canvas zoom stay unchanged.",
        );
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui::Id::new("settings-scale-recovery"), recovery.rect);
        });
        if recovery.clicked() {
            self.reset_interface_scale(ui.ctx());
        }
        ui.separator();
        if self.power.settings.confirm_reset {
            ui.heading("Reset app preferences?");
            egui::ScrollArea::vertical().id_salt("settings-reset-scope").max_height((ui.ctx().content_rect().height() - 300.).max(35.)).show(ui, |ui| {
                ui.label("Restores appearance, canvas navigation, animation defaults, color, new-document/recovery and update preferences. Recent files, export presets, tools, shortcuts, assets, documents, Undo, current view and assistant access are preserved.");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.pc_button("Reset app preferences").clicked() {
                    self.power.preferences.reset_settings();
                    self.reset_settings_appearance(ui.ctx());
                    self.background_update_attempt = None;
                    self.power.settings.confirm_reset = false;
                    self.save_workflow_preferences();
                }
                if ui.pc_button("Cancel").clicked() {
                    self.power.settings.confirm_reset = false;
                }
            });
            return;
        }
        let page = Page::from_id(&self.power.preferences.last_settings_page);
        let query = self.power.settings.query.trim().to_owned();
        let content_height = (ui.ctx().content_rect().height() - 280.).clamp(40., 440.);
        if !query.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt("settings-results")
                .max_height(content_height)
                .show(ui, |ui| {
                    let results: Vec<_> = SETTINGS_ENTRIES
                        .iter()
                        .filter(|e| e.matches(&query))
                        .collect();
                    if results.is_empty() {
                        ui.label("No settings found. Try another search or Clear.");
                    }
                    for entry in results {
                        let response = ui.pc_button(entry.path()).on_hover_text(entry.description);
                        #[cfg(test)]
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(
                                egui::Id::new(("settings-result", entry.id)),
                                (response.id, response.rect),
                            );
                        });
                        if response.gained_focus() {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            self.open_settings_destination(entry.id);
                        }
                        ui.small(entry.description);
                    }
                });
        } else if ui.available_width() >= 640. {
            let sidebar_layout = (page, content_height.to_bits());
            let reveal_selected = self.power.settings.sidebar_layout != Some(sidebar_layout);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(190.);
                    egui::ScrollArea::vertical()
                        .id_salt("settings-sidebar")
                        .max_height(content_height)
                        .show(ui, |ui| {
                            #[cfg(test)]
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new("settings-sidebar-clip"),
                                    ui.clip_rect(),
                                )
                            });
                            for destination in Page::ALL {
                                let response =
                                    ui.selectable_label(page == destination, destination.label());
                                #[cfg(test)]
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new(("settings-sidebar", destination.id())),
                                        (response.id, response.rect),
                                    )
                                });
                                if response.gained_focus()
                                    || (reveal_selected && page == destination)
                                {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    self.select_settings_page(destination);
                                }
                            }
                        });
                });
                let (divider, _) =
                    ui.allocate_exact_size(Vec2::new(1., content_height), egui::Sense::hover());
                ui.painter().vline(
                    divider.center().x,
                    divider.y_range(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    self.settings_page_scroll(ui, page, content_height);
                });
            });
            self.power.settings.sidebar_layout = Some(sidebar_layout);
        } else {
            self.power.settings.sidebar_layout = None;
            let mut selected = page;
            let _selector = egui::ComboBox::from_id_salt("settings-page-selector")
                .selected_text(page.label())
                .show_ui(ui, |ui| {
                    for destination in Page::ALL {
                        ui.pc_selectable_value(&mut selected, destination, destination.label());
                    }
                });
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    egui::Id::new("settings-page-selector"),
                    _selector.response.rect,
                );
            });
            if selected != page {
                self.select_settings_page(selected);
            }
            self.settings_page_scroll(ui, selected, (content_height - 30.).max(40.));
        }
        ui.separator();
        if ui.pc_button("Reset preferences…").clicked() {
            self.power.settings.confirm_reset = true;
        }
    }

    fn select_settings_page(&mut self, page: Page) {
        self.power.preferences.last_settings_page = page.id().into();
        self.power.settings.reveal = None;
        self.power.settings.confirm_reset = false;
        self.save_workflow_preferences();
    }

    fn settings_page_scroll(&mut self, ui: &mut egui::Ui, page: Page, height: f32) {
        let before = serde_json::to_value(&self.power.preferences).ok();
        egui::ScrollArea::vertical()
            .id_salt(("settings-page", page.id()))
            .max_height(height)
            .show(ui, |ui| {
                ui.heading(page.label());
                ui.small("Application preferences. Changes apply immediately.");
                self.settings_page_controls(ui, page);
                if page.has_defaults() {
                    ui.add_space(12.);
                    let reset = ui.pc_button("Reset this page");
                    #[cfg(test)]
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("settings-page-reset"), (reset.id, reset.rect))
                    });
                    if reset.gained_focus() {
                        reset.scroll_to_me(Some(egui::Align::Center));
                    }
                    if reset.clicked() {
                        self.power.preferences.reset_settings_page(page);
                        if page == Page::Appearance {
                            self.reset_settings_appearance(ui.ctx());
                        }
                        if page == Page::Updates {
                            self.background_update_attempt = None;
                        }
                    }
                }
            });
        if before != serde_json::to_value(&self.power.preferences).ok() {
            self.power.apply_density(ui.ctx());
            ui.ctx().set_zoom_factor(self.power.interface_scale());
            self.save_workflow_preferences();
        }
    }

    fn reset_settings_appearance(&mut self, ctx: &egui::Context) {
        let path = self
            .theme_path
            .as_ref()
            .and_then(|p| p.parent())
            .map_or_else(extensions::config_directory, std::path::Path::to_path_buf)
            .join("custom-theme.json");
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                self.status = format!(
                    "Appearance reset, but the saved custom theme could not be removed: {e}"
                )
            }
        }
        Self::apply_theme(ctx, true);
        Self::apply_theme(ctx, false);
        self.set_theme_preference(ctx, egui::ThemePreference::System);
        self.save_theme();
        self.power.apply_density(ctx);
        ctx.set_zoom_factor(self.power.interface_scale());
    }

    fn settings_control(
        &mut self,
        ui: &mut egui::Ui,
        id: &'static str,
        render: impl FnOnce(&mut Self, &mut egui::Ui) -> egui::Response,
    ) {
        ui.push_id(id, |ui| {
            let highlight = self.power.settings.reveal == Some(id);
            let response = egui::Frame::new()
                .fill(if highlight {
                    ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                } else {
                    Color32::TRANSPARENT
                })
                .inner_margin(4.)
                .show(ui, |ui| render(self, ui))
                .inner;
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new(id), response.rect);
                d.insert_temp(egui::Id::new((id, "focus-id")), response.id);
            });
            if response.gained_focus() {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if highlight {
                response.scroll_to_me(Some(egui::Align::Center));
                response.request_focus();
                self.power.settings.reveal = None;
            }
        });
    }

    fn leave_settings_for(&mut self, page: Page) {
        self.power.settings.return_from = Some(page);
        self.settings_open = false;
    }

    fn settings_page_controls(&mut self, ui: &mut egui::Ui, page: Page) {
        match page {
            Page::Appearance => {
                self.settings_control(ui, "settings-theme", |app, ui| {
                    let mut choice = app.theme_preference;
                    let response = egui::ComboBox::from_label("Theme")
                        .selected_text(match choice {
                            egui::ThemePreference::System => "System",
                            egui::ThemePreference::Light => "Light",
                            egui::ThemePreference::Dark => "Dark",
                        })
                        .show_ui(ui, |ui| {
                            for (label, value) in [
                                ("System", egui::ThemePreference::System),
                                ("Light", egui::ThemePreference::Light),
                                ("Dark", egui::ThemePreference::Dark),
                            ] {
                                ui.pc_selectable_value(&mut choice, value, label);
                            }
                        })
                        .response;
                    if choice != app.theme_preference {
                        app.set_theme_preference(ui.ctx(), choice);
                        app.save_theme();
                    }
                    response
                });
                self.settings_control(ui, "settings-density", |app, ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Density");
                        let response = ui.pc_selectable_value(
                            &mut app.power.preferences.comfortable,
                            false,
                            "Compact",
                        );
                        let comfortable = ui.pc_selectable_value(
                            &mut app.power.preferences.comfortable,
                            true,
                            "Comfortable",
                        );
                        #[cfg(test)]
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(
                                egui::Id::new("settings-density-comfortable"),
                                (comfortable.id, comfortable.rect, ui.clip_rect()),
                            )
                        });
                        let _ = comfortable;
                        response
                    })
                    .inner
                });
                ui.small("100% interface zoom follows system display scaling.");
                self.settings_control(ui, "settings-interface-scale", |app, ui| {
                    ui.add(
                        egui::Slider::new(&mut app.power.preferences.interface_scale, 0.5..=3.)
                            .text("Interface scale"),
                    )
                });
            }
            Page::Canvas => {
                self.settings_control(ui, "settings-checkerboard", |app, ui| {
                    let p = &mut app.power.preferences;
                    ui.add(
                        egui::DragValue::new(&mut p.checker_size)
                            .range(1..=64)
                            .prefix("Checker size "),
                    )
                });
                self.settings_control(ui, "settings-checker-colors", |app, ui| {
                    let p = &mut app.power.preferences;
                    let mut custom = p.checker_colors.is_some();
                    let response = ui.pc_checkbox(&mut custom, "Custom checker colors");
                    if custom {
                        let colors = p
                            .checker_colors
                            .get_or_insert([[210, 202, 202], [184, 176, 182]]);
                        ui.horizontal(|ui| {
                            for (index, color) in colors.iter_mut().enumerate() {
                                let response = settings_color_button(
                                    ui.color_edit_button_srgb(color),
                                    ["Checker color 1", "Checker color 2"][index],
                                );
                                #[cfg(test)]
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new(("settings-checker-color", index)),
                                        (response.id, response.rect, ui.clip_rect()),
                                    )
                                });
                                let _ = response;
                            }
                        });
                    } else {
                        p.checker_colors = None;
                    }
                    response
                });
                self.settings_control(ui, "settings-pixel-grid", |app, ui| {
                    let p = &mut app.power.preferences;
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Pixel grid color and opacity");
                        settings_color_button(
                            ui.color_edit_button_srgba_unmultiplied(&mut p.pixel_grid_color),
                            "Pixel grid color",
                        )
                    })
                    .inner
                });
                self.settings_control(ui, "settings-pixel-grid-fade", |app, ui| {
                    ui.pc_checkbox(
                        &mut app.power.preferences.pixel_grid_auto_opacity,
                        "Fade pixel grid at low zoom",
                    )
                });
                self.settings_control(ui, "settings-wheel-zoom", |app, ui| {
                    ui.pc_checkbox(
                        &mut app.power.preferences.whole_pixel_wheel_zoom,
                        "Whole-pixel zoom",
                    )
                });
                self.settings_control(ui, "settings-trim-canvas", |app, ui| {
                    ui.pc_checkbox(
                        &mut app.power.preferences.trim_canvas_content,
                        "Trim canvas content",
                    )
                });
            }
            Page::Animation => {
                self.settings_control(ui, "settings-onion", |app, ui| {
                    ui.add(
                        egui::Slider::new(&mut app.power.preferences.onion.previous, 0..=8)
                            .text("Previous frames"),
                    )
                });
                self.settings_control(ui, "settings-onion-next", |app, ui| {
                    ui.add(
                        egui::Slider::new(&mut app.power.preferences.onion.next, 0..=8)
                            .text("Next frames"),
                    )
                });
                self.settings_control(ui, "settings-onion-previous-tint", |app, ui| {
                    ui.horizontal(|ui| {
                        ui.label("Previous tint");
                        settings_color_button(
                            ui.color_edit_button_srgb(
                                &mut app.power.preferences.onion.previous_tint,
                            ),
                            "Previous tint",
                        )
                    })
                    .inner
                });
                self.settings_control(ui, "settings-onion-previous-opacity", |app, ui| {
                    ui.add(
                        egui::Slider::new(
                            &mut app.power.preferences.onion.previous_opacity,
                            0..=255,
                        )
                        .text("Previous opacity"),
                    )
                });
                self.settings_control(ui, "settings-onion-next-tint", |app, ui| {
                    ui.horizontal(|ui| {
                        ui.label("Next tint");
                        settings_color_button(
                            ui.color_edit_button_srgb(&mut app.power.preferences.onion.next_tint),
                            "Next tint",
                        )
                    })
                    .inner
                });
                self.settings_control(ui, "settings-onion-next-opacity", |app, ui| {
                    ui.add(
                        egui::Slider::new(&mut app.power.preferences.onion.next_opacity, 0..=255)
                            .text("Next opacity"),
                    )
                });
                self.settings_control(ui, "settings-rewind", |app, ui| {
                    ui.pc_checkbox(&mut app.power.preferences.rewind_on_stop, "Rewind on stop")
                });
            }
            Page::Color => {
                self.settings_control(ui, "settings-display-profile", |app, ui| {
                    let mut display = app.power.preferences.display_profile.clone();
                    // Keep the existing profile chooser and its ICC validation/conversion behavior.
                    let response = ui.scope(|ui| {
                        let id = ui.make_persistent_id("Display profile");
                        app.display_profile_ui(ui, &mut display);
                        ui.ctx().read_response(id)
                    });
                    let response = response.inner.unwrap_or(response.response);
                    app.power.preferences.display_profile = display;
                    response
                });
                self.settings_control(ui, "settings-import-profile", |app, ui| {
                    use crate::api::color::ImportProfile;
                    let label = |p| match p {
                        ImportProfile::Preserve => "Preserve embedded profile",
                        ImportProfile::ConvertToSrgb => "Convert to sRGB",
                        ImportProfile::AssignSrgb => "Assign sRGB",
                    };
                    let policy = &mut app.power.preferences.import_profile;
                    ui.label("Image import profile");
                    egui::ComboBox::from_id_salt("Image import profile")
                        .width(ui.available_width().min(220.))
                        .selected_text(label(*policy))
                        .show_ui(ui, |ui| {
                            for p in [
                                ImportProfile::Preserve,
                                ImportProfile::ConvertToSrgb,
                                ImportProfile::AssignSrgb,
                            ] {
                                ui.pc_selectable_value(policy, p, label(p));
                            }
                        })
                        .response
                });
                ui.small("Imported layers match the destination profile. Assign sRGB treats source values as sRGB.");
            }
            Page::Files => {
                self.settings_control(ui, "settings-recovery", |app, ui| {
                    ui.pc_checkbox(
                        &mut app.power.preferences.autosave,
                        "Enable recovery autosave",
                    )
                });
                ui.heading("New sprites");
                self.settings_control(ui, "settings-new-sprites", |app, ui| {
                    ui.label("Default width");
                    let width = ui.add(
                        egui::DragValue::new(&mut app.power.preferences.new_sprite_width)
                            .range(1..=MAX_DIMENSION)
                            .suffix(" px"),
                    );
                    #[cfg(test)]
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("default-sprite-width"), width.rect)
                    });
                    width
                });
                self.settings_control(ui, "settings-new-sprite-height", |app, ui| {
                    let p = &mut app.power.preferences;
                    ui.label("Default height");
                    let height_limit =
                        MAX_DIMENSION.min(MAX_PIXELS / p.new_sprite_width.clamp(1, MAX_DIMENSION));
                    ui.add(
                        egui::DragValue::new(&mut p.new_sprite_height)
                            .range(1..=height_limit)
                            .suffix(" px"),
                    )
                });
                self.settings_control(ui, "settings-default-grid", |app, ui| {
                    ui.pc_checkbox(
                        &mut app.power.preferences.default_grid,
                        "Show pixel grid by default",
                    )
                });
                ui.small("Applies to new documents. The current document and view are unchanged.");
            }
            Page::Shortcuts => {
                if ui.pc_button("Open keyboard shortcuts…").clicked() {
                    self.leave_settings_for(page);
                    self.action("shortcuts");
                }
                ui.small(
                    "Edit bindings, resolve conflicts and reset shortcuts in the shortcut editor.",
                );
            }
            Page::Integrations => {
                self.settings_control(ui, "settings-assets", |app, ui| {
                    let r = ui.pc_button("Asset extensions…");
                    if r.clicked() {
                        app.leave_settings_for(page);
                        app.action("extensions");
                    }
                    r
                });
                self.settings_control(ui, "settings-assistant-access", |app, ui| {
                    let r = ui.pc_button("Assistant access…");
                    if r.clicked() {
                        app.leave_settings_for(page);
                        app.action("assistant-access");
                    }
                    r
                });
                ui.small("Assistant access is an explicit opt-in for the active editor. Opening Settings never enables it.");
            }
            Page::Updates => {
                self.settings_control(ui, "settings-background-updates", |app, ui| {
                    let r = ui.pc_checkbox(
                        &mut app.power.preferences.background_updates,
                        "Download and verify updates in the background",
                    );
                    if r.changed() {
                        app.background_update_attempt = None;
                    }
                    r
                });
                self.settings_control(ui, "settings-about", |app, ui| {
                    let r = ui.pc_button("About PixelCraft…");
                    if r.clicked() {
                        app.leave_settings_for(page);
                        app.action("about");
                    }
                    r
                });
                ui.small("Install verified updates from About PixelCraft.");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rendered_text(app: &mut PixelCraft, ctx: &egui::Context, size: Vec2) -> String {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| app.settings_ui(ui),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn frame(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        size: Vec2,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        // The editor applies density before opening any dialog.
        app.power.apply_density(ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                time: Some(ctx.cumulative_frame_nr() as f64 * 0.1),
                ..Default::default()
            },
            |ui| app.settings_dialog(ui.ctx()),
        );
        output.textures_delta.clear();
        output
    }
    fn settled_frame(app: &mut PixelCraft, ctx: &egui::Context, size: Vec2) -> egui::FullOutput {
        for _ in 0..3 {
            frame(app, ctx, size, vec![]);
        }
        frame(app, ctx, size, vec![])
    }
    fn click(p: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }
    fn position(output: &egui::FullOutput, text: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == text => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                let visible: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some((t.pos, t.galley.job.text.as_str())),
                        _ => None,
                    })
                    .collect();
                panic!("missing rendered text {text}; visible: {visible:?}")
            })
    }
    fn rect(ctx: &egui::Context, id: &str) -> Rect {
        ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(id)))
            .unwrap_or_else(|| panic!("missing rect {id}"))
    }
    #[test]
    fn autosave_search_opens_and_focuses_real_recovery_checkbox() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.settings_open = true;
        app.power.preferences.autosave = false;
        app.power.settings.query = "autosave".into();
        let size = Vec2::new(980., 720.);
        frame(&mut app, &ctx, size, vec![]);
        let output = frame(&mut app, &ctx, size, vec![]);
        let result = position(
            &output,
            "Settings / Files & Recovery / Enable recovery autosave",
        );
        frame(&mut app, &ctx, size, click(result));
        assert_eq!(app.power.preferences.last_settings_page, "files");
        assert!(app.power.settings.query.is_empty());
        frame(&mut app, &ctx, size, vec![]);
        let focus = ctx
            .data(|d| d.get_temp::<egui::Id>(egui::Id::new(("settings-recovery", "focus-id"))))
            .unwrap();
        assert_eq!(ctx.memory(|m| m.focused()), Some(focus));
        let target = rect(&ctx, "settings-recovery");
        assert!(ctx.content_rect().contains_rect(target));
        frame(&mut app, &ctx, size, click(target.center()));
        assert!(app.power.preferences.autosave);
        let loaded: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&app.power.preferences).unwrap()).unwrap();
        assert_eq!(loaded.last_settings_page, "files");
        assert!(loaded.autosave);
        assert!(app.power.bridge.is_none());
    }
    #[test]
    fn individual_settings_destinations_focus_their_actual_controls() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        for id in [
            "settings-checker-colors",
            "settings-pixel-grid-fade",
            "settings-onion-next",
            "settings-onion-previous-tint",
            "settings-onion-previous-opacity",
            "settings-onion-next-tint",
            "settings-onion-next-opacity",
            "settings-new-sprite-height",
            "settings-default-grid",
            "settings-display-profile",
        ] {
            assert!(
                app.open_settings_destination(id),
                "missing destination {id}"
            );
            frame(&mut app, &ctx, Vec2::new(980., 720.), vec![]);
            let focus = ctx
                .data(|d| d.get_temp::<egui::Id>(egui::Id::new((id, "focus-id"))))
                .unwrap();
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(focus),
                "did not focus {id}"
            );
        }
    }
    #[test]
    fn next_tint_search_reveals_the_color_control_instead_of_a_different_slider() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.settings_open = true;
        app.power.settings.query = "next tint".into();
        let size = Vec2::new(980., 720.);
        frame(&mut app, &ctx, size, vec![]);
        let output = frame(&mut app, &ctx, size, vec![]);
        let result = position(&output, "Settings / Animation / Next tint");
        frame(&mut app, &ctx, size, click(result));
        frame(&mut app, &ctx, size, vec![]);
        let id = ctx
            .data(|d| {
                d.get_temp::<egui::Id>(egui::Id::new(("settings-onion-next-tint", "focus-id")))
            })
            .unwrap();
        assert_eq!(ctx.memory(|m| m.focused()), Some(id));
        assert!(
            ctx.content_rect()
                .contains_rect(rect(&ctx, "settings-onion-next-tint"))
        );
    }
    #[test]
    fn restored_category_reveals_on_open_and_resize_but_allows_manual_sidebar_scroll() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.power.preferences.last_settings_page = "files".into();
        app.settings_open = true;
        let size = Vec2::new(683., 384.);
        for _ in 0..6 {
            frame(&mut app, &ctx, size, vec![]);
        }
        let selected = |ctx: &egui::Context| {
            let (_, rect) = ctx
                .data(|d| {
                    d.get_temp::<(egui::Id, Rect)>(egui::Id::new(("settings-sidebar", "files")))
                })
                .unwrap();
            let clip = ctx
                .data(|d| d.get_temp::<Rect>(egui::Id::new("settings-sidebar-clip")))
                .unwrap();
            (rect, clip)
        };
        let (rect, clip) = selected(&ctx);
        assert!(
            clip.contains_rect(rect),
            "restored Files category is clipped: {rect:?} in {clip:?}"
        );
        frame(
            &mut app,
            &ctx,
            size,
            vec![
                egui::Event::PointerMoved(Pos2::new(rect.center().x, clip.center().y)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: Vec2::new(0., 300.),
                    modifiers: Default::default(),
                },
            ],
        );
        for _ in 0..10 {
            frame(&mut app, &ctx, size, vec![]);
        }
        let (rect, clip) = selected(&ctx);
        assert!(
            !clip.contains_rect(rect),
            "manual sidebar scrolling must not be undone"
        );
        let resized = Vec2::new(683., 420.);
        for _ in 0..6 {
            frame(&mut app, &ctx, resized, vec![]);
        }
        let (rect, clip) = selected(&ctx);
        assert!(
            clip.contains_rect(rect),
            "resized sidebar must reveal its selected category"
        );
    }
    #[test]
    fn narrow_settings_keep_navigation_done_and_scale_recovery_reachable() {
        for size in [
            Vec2::new(420., 384.),
            Vec2::new(320., 384.),
            Vec2::new(683., 384.),
            Vec2::new(960., 540.),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_settings_destination("settings-recovery");
            for _ in 0..3 {
                frame(&mut app, &ctx, size, vec![]);
            }
            let bounds = Rect::from_min_size(Pos2::ZERO, size);
            for id in ["settings-done", "settings-scale-recovery"] {
                assert!(
                    bounds.contains_rect(rect(&ctx, id)),
                    "{id} outside {size:?}: {:?}",
                    rect(&ctx, id)
                );
            }
            if size.x < 640. {
                assert!(bounds.contains_rect(rect(&ctx, "settings-page-selector")));
            }
            let output = frame(&mut app, &ctx, size, vec![]);
            assert!(
                bounds.contains(position(&output, "Reset preferences…")),
                "preference reset footer is outside {size:?}"
            );
            let done = rect(&ctx, "settings-done").center();
            frame(&mut app, &ctx, size, click(done));
            assert!(!app.settings_open);
        }
    }
    fn tab() -> Vec<egui::Event> {
        [true, false]
            .into_iter()
            .map(|pressed| egui::Event::Key {
                key: Key::Tab,
                physical_key: Some(Key::Tab),
                pressed,
                repeat: false,
                modifiers: Default::default(),
            })
            .collect()
    }
    #[test]
    fn tab_navigation_reveals_bottom_page_and_search_results() {
        for region in ["page", "search", "sidebar"] {
            let search = region == "search";
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_settings_destination("settings-files");
            if search {
                app.power.settings.query = "settings".into();
            }
            let size = Vec2::new(683., 384.);
            for _ in 0..3 {
                frame(&mut app, &ctx, size, vec![]);
            }
            let id = match region {
                "search" => egui::Id::new(("settings-result", "settings-about")),
                "sidebar" => egui::Id::new(("settings-sidebar", "updates")),
                _ => egui::Id::new("settings-page-reset"),
            };
            let mut reached = false;
            for _ in 0..100 {
                frame(&mut app, &ctx, size, tab());
                let (focus, target) = ctx.data(|d| d.get_temp::<(egui::Id, Rect)>(id)).unwrap();
                if ctx.memory(|m| m.focused()) == Some(focus) {
                    for _ in 0..5 {
                        frame(&mut app, &ctx, size, vec![]);
                    }
                    let (_, target) = ctx
                        .data(|d| d.get_temp::<(egui::Id, Rect)>(id))
                        .unwrap_or((focus, target));
                    assert!(
                        Rect::from_min_size(Pos2::ZERO, size).contains_rect(target),
                        "Tab focused hidden target {id:?}: {target:?}, region={region}"
                    );
                    reached = true;
                    break;
                }
            }
            assert!(reached, "Tab never reached target region={region}");
        }
    }
    #[test]
    fn narrow_pages_reveal_controls_inside_the_viewport() {
        for destination in [
            "settings-interface-scale",
            "settings-pixel-grid",
            "settings-onion-next-opacity",
            "settings-import-profile",
            "settings-background-updates",
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_settings_destination(destination);
            let size = Vec2::new(320., 384.);
            for _ in 0..4 {
                frame(&mut app, &ctx, size, vec![]);
            }
            assert!(
                Rect::from_min_size(Pos2::ZERO, size).contains_rect(rect(&ctx, destination)),
                "{destination} is outside the narrow viewport: {:?}",
                rect(&ctx, destination)
            );
            assert!(
                Rect::from_min_size(Pos2::ZERO, size).contains_rect(rect(&ctx, "settings-done"))
            );
        }
    }
    #[test]
    fn narrow_reset_confirmation_keeps_cancel_and_confirm_reachable() {
        for size in [Vec2::new(320., 384.), Vec2::new(683., 384.)] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_settings_destination("settings-animation");
            app.power.settings.confirm_reset = true;
            for _ in 0..2 {
                frame(&mut app, &ctx, size, vec![]);
            }
            let output = frame(&mut app, &ctx, size, vec![]);
            for label in ["Cancel", "Reset app preferences"] {
                let p = position(&output, label);
                assert!(
                    Rect::from_min_size(Pos2::ZERO, size).contains(p),
                    "{label} outside {size:?}: {p:?}"
                );
            }
            let cancel = position(&output, "Cancel");
            frame(&mut app, &ctx, size, click(cancel));
            assert!(!app.power.settings.confirm_reset);
        }
    }
    #[test]
    fn empty_search_can_be_cleared_without_touching_preferences() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.settings_open = true;
        let before = serde_json::to_value(&app.power.preferences).unwrap();
        app.power.settings.query = "zzznomatch".into();
        let size = Vec2::new(420., 500.);
        frame(&mut app, &ctx, size, vec![]);
        let output = frame(&mut app, &ctx, size, vec![]);
        position(&output, "No settings found. Try another search or Clear.");
        let clear = position(&output, "Clear");
        frame(&mut app, &ctx, size, click(clear));
        assert!(app.power.settings.query.is_empty());
        assert_eq!(
            serde_json::to_value(&app.power.preferences).unwrap(),
            before
        );
    }
    #[test]
    fn resets_require_confirmation_and_preserve_document_view_and_operation_drafts() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-settings-reset-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        app.theme_path = Some(directory.join("theme.json"));
        std::fs::write(directory.join("custom-theme.json"), b"{}").unwrap();
        app.power.preferences = Preferences {
            checker_size: 23,
            autosave: false,
            sample_references: true,
            preset: "remember".into(),
            new_sprite_width: 61,
            ..Default::default()
        };
        app.power.preferences.brush_dynamics.stabilizer = 7;
        app.grid = true;
        app.studio.sheet.scale = 4;
        let document = app.doc.clone();
        app.open_settings_destination("settings-files");
        let size = Vec2::new(980., 720.);
        frame(&mut app, &ctx, size, vec![]);
        let output = settled_frame(&mut app, &ctx, size);
        let reset_page = position(&output, "Reset this page");
        frame(&mut app, &ctx, size, click(reset_page));
        assert!(app.power.preferences.autosave);
        assert_eq!(app.power.preferences.new_sprite_width, 32);
        assert_eq!(app.power.preferences.checker_size, 23);
        let output = settled_frame(&mut app, &ctx, size);
        let reset = position(&output, "Reset preferences…");
        frame(&mut app, &ctx, size, click(reset));
        assert_eq!(
            app.power.preferences.checker_size, 23,
            "opening confirmation cannot reset"
        );
        let output = settled_frame(&mut app, &ctx, size);
        let cancel = position(&output, "Cancel");
        frame(&mut app, &ctx, size, click(cancel));
        assert_eq!(app.power.preferences.checker_size, 23);
        let output = settled_frame(&mut app, &ctx, size);
        let reset = position(&output, "Reset preferences…");
        frame(&mut app, &ctx, size, click(reset));
        let output = settled_frame(&mut app, &ctx, size);
        let confirm = position(&output, "Reset app preferences");
        frame(&mut app, &ctx, size, click(confirm));
        assert_eq!(app.power.preferences.checker_size, 8);
        assert!(app.power.preferences.sample_references);
        assert_eq!(app.power.preferences.brush_dynamics.stabilizer, 7);
        assert_eq!(app.power.preferences.preset, "remember");
        assert!(app.grid);
        assert_eq!(app.studio.sheet.scale, 4);
        assert_eq!(app.doc, document);
        assert!(app.power.bridge.is_none());
        assert_eq!(app.theme_preference, egui::ThemePreference::System);
        assert!(!directory.join("custom-theme.json").exists());
        assert_eq!(
            std::fs::read_to_string(directory.join("theme.json")).unwrap(),
            "\"system\""
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn settings_color_buttons_have_contextual_accessible_names_and_keep_selected_states() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.power.preferences.checker_colors = Some([[1, 2, 3], [4, 5, 6]]);
        app.power.preferences.comfortable = true;
        for (destination, labels) in [
            (
                "settings-canvas",
                vec!["Checker color 1", "Checker color 2", "Pixel grid color"],
            ),
            ("settings-animation", vec!["Previous tint", "Next tint"]),
            ("settings-appearance", vec![]),
        ] {
            app.open_settings_destination(destination);
            let output = settled_frame(&mut app, &ctx, Vec2::new(980., 900.));
            let tree = output.platform_output.accesskit_update.as_ref().unwrap();
            for label in labels {
                assert!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == egui::accesskit::Role::ColorWell
                            && node.label() == Some(label)),
                    "missing accessible color button {label}"
                );
            }
            if destination == "settings-canvas" {
                let checkbox = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("Custom checker colors"))
                    .unwrap();
                assert_eq!(checkbox.1.toggled(), Some(egui::accesskit::Toggled::True));
            }
            if destination == "settings-appearance" {
                let comfortable = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("Comfortable"))
                    .unwrap();
                assert_eq!(
                    comfortable.1.toggled(),
                    Some(egui::accesskit::Toggled::True)
                );
            }
        }
    }
    #[test]
    fn tab_navigation_reveals_wrapped_density_and_second_checker_swatch() {
        for checker in [false, true] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.power.preferences.checker_colors = Some([[1, 2, 3], [4, 5, 6]]);
            app.power.preferences.comfortable = false;
            app.open_settings_destination(if checker {
                "settings-canvas"
            } else {
                "settings-appearance"
            });
            let size = Vec2::new(320., 384.);
            settled_frame(&mut app, &ctx, size);
            let target = if checker {
                egui::Id::new(("settings-checker-color", 1_usize))
            } else {
                egui::Id::new("settings-density-comfortable")
            };
            let mut reached = false;
            for _ in 0..100 {
                frame(&mut app, &ctx, size, tab());
                let (id, _, _) = ctx
                    .data(|d| d.get_temp::<(egui::Id, Rect, Rect)>(target))
                    .unwrap();
                if ctx.memory(|m| m.focused()) == Some(id) {
                    settled_frame(&mut app, &ctx, size);
                    let (_, rect, clip) = ctx
                        .data(|d| d.get_temp::<(egui::Id, Rect, Rect)>(target))
                        .unwrap();
                    assert!(
                        clip.contains_rect(rect),
                        "focused compound child is clipped, checker={checker}: {rect:?} in {clip:?}"
                    );
                    reached = true;
                    break;
                }
            }
            assert!(reached, "did not reach compound child checker={checker}");
        }
    }
    fn editor_frame(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100., 900.))),
                events,
                time: Some(ctx.cumulative_frame_nr() as f64 * 0.1),
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        output.textures_delta.clear();
        output
    }
    fn settled_editor_frame(app: &mut PixelCraft, ctx: &egui::Context) -> egui::FullOutput {
        for _ in 0..3 {
            editor_frame(app, ctx, vec![]);
        }
        editor_frame(app, ctx, vec![])
    }
    #[test]
    fn settings_links_replace_the_dialog_and_return_without_changing_preferences() {
        for (page, label, target) in [
            (
                "settings-shortcuts",
                "Open keyboard shortcuts…",
                "shortcuts",
            ),
            ("settings-integrations", "Asset extensions…", "extensions"),
            (
                "settings-integrations",
                "Assistant access…",
                "assistant-access",
            ),
            ("settings-updates", "About PixelCraft…", "about"),
        ] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = PixelCraft::with_context(&ctx, None);
            let directory = std::env::temp_dir().join(format!(
                "pixelcraft-settings-links-{}-{target}",
                std::process::id()
            ));
            app.extension_manager = Some(extensions::Manager::at(directory.clone()).unwrap());
            app.power.preferences.checker_size = 23;
            app.start_canvas_transform();
            app.curve_click(CanvasPane::Primary, (1, 1));
            app.open_settings_destination(page);
            let preferences = serde_json::to_value(&app.power.preferences).unwrap();
            let document = app.doc.clone();
            let output = settled_editor_frame(&mut app, &ctx);
            if target == "extensions" {
                app.extension_error = "stale list error".into();
            }
            editor_frame(&mut app, &ctx, click(position(&output, label)));
            assert!(
                !app.settings_open,
                "linked dialog must replace Settings: {target}"
            );
            assert!(match target {
                "shortcuts" => app.shortcut_settings_open,
                "extensions" => app.extensions_open,
                "assistant-access" => app.power.open,
                _ => app.about,
            });
            if target == "extensions" {
                assert!(
                    app.extension_error.is_empty(),
                    "the canonical extensions route refreshes its list"
                );
            }
            let output = settled_editor_frame(&mut app, &ctx);
            let close = match target {
                "shortcuts" => ctx
                    .data(|d| {
                        d.get_temp::<Rect>(egui::Id::new(("dialog-close", "Keyboard shortcuts")))
                    })
                    .unwrap()
                    .center(),
                "assistant-access" => ctx
                    .data(|d| {
                        d.get_temp::<Rect>(egui::Id::new(("dialog-close", "Assistant access")))
                    })
                    .unwrap()
                    .center(),
                _ => {
                    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
                    let node = tree
                        .nodes
                        .iter()
                        .find(|(_, n)| n.label() == Some("Close window"))
                        .expect("linked window has an accessible close control");
                    let bounds = node.1.bounds().unwrap();
                    Pos2::new(
                        ((bounds.x0 + bounds.x1) * 0.5) as f32,
                        ((bounds.y0 + bounds.y1) * 0.5) as f32,
                    )
                }
            };
            editor_frame(&mut app, &ctx, click(close));
            settled_editor_frame(&mut app, &ctx);
            assert!(app.settings_open, "closing {target} returns to Settings");
            assert!(
                !app.shortcut_settings_open
                    && !app.extensions_open
                    && !app.power.open
                    && !app.about
            );
            assert_eq!(
                serde_json::to_value(&app.power.preferences).unwrap(),
                preferences
            );
            assert_eq!(app.doc, document);
            assert!(
                app.canvas_transform.is_some(),
                "{target} discarded a transform draft"
            );
            assert!(
                app.curve_draft.is_some(),
                "{target} discarded a curve draft"
            );
            assert!(app.power.bridge.is_none());
            std::fs::remove_dir_all(directory).unwrap();
        }
    }
    #[test]
    fn categorized_settings_render_navigation_and_search() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let text = rendered_text(&mut app, &ctx, Vec2::new(1000., 740.));
        assert!(
            text.contains("Canvas & Navigation"),
            "missing category navigation: {text}"
        );
        assert!(text.contains("Search settings"));
        assert!(text.contains("Reset interface scale"));
    }
    #[test]
    fn unknown_navigation_id_preserves_preferences() {
        let p: Preferences = serde_json::from_str(
            r#"{"last_settings_page":"future-page","interface_scale":1.5,"autosave":false}"#,
        )
        .unwrap();
        let value = serde_json::to_value(&p).unwrap();
        assert_eq!(value["last_settings_page"], "future-page");
        assert_eq!(p.interface_scale, 1.5);
        assert!(!p.autosave);
    }
    #[test]
    fn app_reset_preserves_saved_tool_preferences() {
        let mut p = Preferences {
            sample_references: true,
            preset: "custom".into(),
            ..Default::default()
        };
        p.brush_dynamics.stabilizer = 7;
        p.reset_settings();
        assert!(
            p.sample_references,
            "app reset must preserve reference sampling"
        );
        assert_eq!(p.brush_dynamics.stabilizer, 7);
        assert_eq!(p.preset, "custom");
    }
}
