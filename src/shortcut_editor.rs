//! Keyboard capture and conflict review for editor shortcut settings.
use super::*;
use pixelcraft::productivity::{save_shortcuts, shortcut_preset};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ShortcutChord {
    key: Key,
    command: bool,
    shift: bool,
    alt: bool,
}

impl ShortcutChord {
    pub(super) fn parse(binding: &str) -> Option<Self> {
        let (modifiers, key) = binding.rsplit_once('+').unwrap_or(("", binding));
        let mut chord = Self {
            key: Key::from_name(key.trim())?,
            command: false,
            shift: false,
            alt: false,
        };
        for modifier in modifiers
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if modifier.eq_ignore_ascii_case("Ctrl") || modifier.eq_ignore_ascii_case("Cmd") {
                chord.command = true;
            } else if modifier.eq_ignore_ascii_case("Shift") {
                chord.shift = true;
            } else if modifier.eq_ignore_ascii_case("Alt") {
                chord.alt = true;
            } else {
                return None;
            }
        }
        Some(chord)
    }

    fn new(key: Key, modifiers: egui::Modifiers) -> Self {
        Self {
            key,
            command: modifiers.command,
            shift: modifiers.shift,
            alt: modifiers.alt,
        }
    }

    fn binding(self) -> String {
        format!(
            "{}{}{}{}",
            if self.command { "Ctrl+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            self.key.name(),
        )
    }

    pub(super) fn matches(self, input: &egui::InputState) -> bool {
        input.key_pressed(self.key) && self == Self::new(self.key, input.modifiers)
    }

    fn reserved(self) -> bool {
        matches!(
            self.key,
            Key::Escape | Key::Tab | Key::Copy | Key::Cut | Key::Paste
        ) || (self.alt && self.key == Key::F4)
            || (cfg!(target_os = "macos") && self.command && self.key == Key::Q)
            || self.clipboard_action().is_some()
            || (cfg!(target_os = "macos") && self.navigation_action().is_some())
    }

    fn navigation_action(self) -> Option<&'static str> {
        match (self.key, self.command, self.shift, self.alt) {
            (Key::Comma, true, false, false) => Some("settings"),
            _ => None,
        }
    }

    fn clipboard_action(self) -> Option<&'static str> {
        match (self.key, self.command, self.shift) {
            (Key::C, true, _) => Some("copy"),
            (Key::X, true, _) => Some("cut"),
            (Key::V, true, _) => Some("paste"),
            (Key::Insert, true, _) if cfg!(target_os = "windows") => Some("copy"),
            (Key::Insert, _, true) if cfg!(target_os = "windows") => Some("paste"),
            (Key::Delete, _, true) if cfg!(target_os = "windows") => Some("cut"),
            _ => None,
        }
    }

    pub(super) fn allowed_for(self, action: &str) -> bool {
        !self.reserved()
            || self.clipboard_action() == Some(action)
            || self.navigation_action() == Some(action)
            || (self.key == Key::Escape
                && !self.command
                && !self.shift
                && !self.alt
                && action == "deselect")
    }
}

pub(super) struct ShortcutEdit {
    action: String,
    stage: ShortcutStage,
}

enum ShortcutStage {
    Capture {
        after_frame: u64,
        message: Option<String>,
    },
    Review {
        keys: Vec<ShortcutChord>,
        reset: bool,
    },
}

impl PixelCraft {
    pub(super) fn begin_shortcut_capture(&mut self, ctx: &egui::Context, action: String) {
        self.cancel_gesture();
        self.shortcut_edit = Some(ShortcutEdit {
            action,
            stage: ShortcutStage::Capture {
                after_frame: ctx.cumulative_frame_nr(),
                message: None,
            },
        });
    }

    pub(super) fn begin_shortcut_reset(&mut self, action: String) {
        let keys = shortcut_preset(self.power.shortcut_preset_name())
            .iter()
            .filter(|(_, target)| *target == &action)
            .filter_map(|(key, _)| ShortcutChord::parse(key))
            .collect();
        self.shortcut_edit = Some(ShortcutEdit {
            action,
            stage: ShortcutStage::Review { keys, reset: true },
        });
    }

    pub(super) fn save_shortcut_bindings(&mut self) {
        if let Some(path) = &self.shortcuts_path {
            let result = (|| {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(path, save_shortcuts(&self.custom_shortcuts)?)
                    .map_err(|e| e.to_string())
            })();
            if let Err(error) = result {
                self.status = format!("Could not save shortcuts: {error}");
            }
        }
    }

    fn shortcut_conflicts(&self, keys: &[ShortcutChord], action: &str) -> Vec<String> {
        self.custom_shortcuts
            .iter()
            .filter(|(_, owner)| owner.as_str() != action)
            .filter_map(|(binding, owner)| {
                let chord = ShortcutChord::parse(binding)?;
                keys.contains(&chord).then(|| {
                    format!(
                        "{} is assigned to {}",
                        chord.binding(),
                        ui_icons::action_label(owner)
                    )
                })
            })
            .collect()
    }

    pub(super) fn shortcut_edit_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut edit) = self.shortcut_edit.take() else {
            return;
        };
        let pressed = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(ShortcutChord::new(*key, *modifiers)),
                _ => None,
            })
        });
        let clipboard = ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)
                )
            })
        });
        if pressed.is_some_and(|chord| chord.key == Key::Escape) {
            return;
        }
        if let ShortcutStage::Capture {
            after_frame,
            message,
        } = &mut edit.stage
            && ctx.cumulative_frame_nr() > *after_frame
        {
            if clipboard {
                *message = Some("Clipboard shortcuts are reserved.".into());
            } else if let Some(chord) = pressed {
                if chord.reserved() {
                    *message = Some(format!("{} is reserved.", chord.binding()));
                } else {
                    edit.stage = ShortcutStage::Review {
                        keys: vec![chord],
                        reset: false,
                    };
                }
            }
        }

        let mut accept = false;
        let mut cancel = false;
        let dismiss = fluent::modal_dialog(ctx, "Keyboard shortcut", |ui| {
            ui.label(ui_icons::action_label(&edit.action));
            match &edit.stage {
                ShortcutStage::Capture { message, .. } => {
                    ui.label("Press a key or key combination. Escape cancels capture.");
                    if let Some(message) = message {
                        ui.label(message);
                    }
                }
                ShortcutStage::Review { keys, reset } => {
                    if *reset {
                        ui.label("Restore this command’s shortcuts from the active preset.");
                    }
                    for chord in keys {
                        ui.monospace(chord.binding());
                    }
                    let conflicts = self.shortcut_conflicts(keys, &edit.action);
                    for conflict in &conflicts {
                        ui.label(conflict);
                    }
                    accept = ui
                        .pc_button(match (*reset, conflicts.is_empty()) {
                            (true, true) => "Reset shortcut",
                            (true, false) => "Replace and reset",
                            (false, true) => "Assign shortcut",
                            (false, false) => "Replace shortcut",
                        })
                        .clicked();
                }
            }
            cancel = ui.pc_button("Cancel").clicked();
            accept || cancel
        });
        if accept && let ShortcutStage::Review { keys, reset } = edit.stage {
            self.custom_shortcuts.retain(|binding, action| {
                let Some(chord) = ShortcutChord::parse(binding) else {
                    return !reset || action != &edit.action;
                };
                !(keys.contains(&chord) || reset && action == &edit.action && !chord.reserved())
            });
            for chord in keys {
                if chord.allowed_for(&edit.action) {
                    self.custom_shortcuts
                        .insert(chord.binding(), edit.action.clone());
                }
            }
            self.save_shortcut_bindings();
        } else if !cancel && !dismiss {
            self.shortcut_edit = Some(edit);
        }
    }

    pub(super) fn shortcut_settings_dialog(&mut self, ctx: &egui::Context) {
        let mut done = false;
        let dismiss = fluent::modal_dialog(ctx, "Keyboard shortcuts", |ui| {
            self.shortcut_settings(ui);
            done = ui.pc_button("Done").clicked();
            done
        });
        if dismiss || done {
            self.shortcut_settings_open = false;
        }
    }

    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for name in ["PixelCraft", "Aseprite", "Photoshop"] {
                if ui.pc_button(name).clicked() {
                    self.custom_shortcuts = shortcut_preset(name);
                    self.power.set_shortcut_preset(name);
                    self.save_workflow_preferences();
                    self.save_shortcut_bindings();
                }
            }
        });
        ui.label(format!("Preset {}", self.power.shortcut_preset_name()));
        let actions: std::collections::BTreeSet<_> = ["PixelCraft", "Aseprite", "Photoshop"]
            .into_iter()
            .flat_map(|name| shortcut_preset(name).into_values())
            .chain(
                [
                    "copy-merged",
                    "paste-as-sprite",
                    "paste-as-layer",
                    "paste-as-reference",
                    "new-sprite-from-selection",
                ]
                .map(str::to_owned),
            )
            .chain(self.custom_shortcuts.values().cloned())
            .collect();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("shortcut-command")
                .selected_text(ui_icons::action_label(&self.shortcut_action))
                .show_ui(ui, |ui| {
                    for action in actions {
                        let label = ui_icons::action_label(&action).to_owned();
                        ui.pc_selectable_value(&mut self.shortcut_action, action, label);
                    }
                });
            if ui.pc_button("Capture shortcut…").clicked() {
                self.begin_shortcut_capture(ui.ctx(), self.shortcut_action.clone());
            }
        });
        let mut remove = None;
        for (binding, action) in &self.custom_shortcuts {
            if action != &self.shortcut_action {
                continue;
            }
            ui.horizontal(|ui| {
                ui.monospace(binding);
                if ShortcutChord::parse(binding).is_some_and(ShortcutChord::reserved) {
                    ui.small("Reserved binding");
                } else if ui.pc_small_button("Remove").clicked() {
                    remove = Some(binding.clone());
                }
            });
        }
        if let Some(binding) = remove {
            self.custom_shortcuts.remove(&binding);
            self.save_shortcut_bindings();
        }
        if ui.pc_button("Reset command shortcuts…").clicked() {
            self.begin_shortcut_reset(self.shortcut_action.clone());
        }
    }
}
