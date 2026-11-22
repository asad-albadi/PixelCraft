use super::*;
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_LIBRARY_BYTES: usize = 24_000_000;
const MAX_PRESET_PIXELS: usize = 1_000_000;

#[derive(Clone, Serialize, Deserialize)]
struct SavedBrush {
    size: i32,
    shape: BrushShape,
    mode: BrushMode,
    reverse: bool,
    ink: PaintInk,
    #[serde(default)]
    dynamics: BrushDynamics,
    stamp: Option<(usize, usize, Vec<Pixel>)>,
    #[serde(default)]
    alignment: ImageBrushAlignment,
    #[serde(default)]
    source_origin: (i32, i32),
    ramp: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct BrushLibrary {
    brushes: BTreeMap<String, SavedBrush>,
    ramps: BTreeMap<String, Vec<Pixel>>,
    #[serde(skip)]
    active_ramp: Option<String>,
    #[serde(skip)]
    name: String,
}

pub(super) fn path() -> PathBuf {
    extensions::config_directory().join("brush-library.json")
}

fn name_key(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 96 || name.chars().any(char::is_control) {
        return Err("Enter a name of up to 96 bytes".into());
    }
    Ok(name)
}

impl BrushLibrary {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|error| error.to_string())?
            .take(MAX_LIBRARY_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > MAX_LIBRARY_BYTES {
            return Err("Brush library exceeds its file size limit".into());
        }
        let library: Self = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        library.validate()?;
        Ok(library)
    }

    fn validate(&self) -> Result<(), String> {
        let mut pixels = 0usize;
        for (name, brush) in &self.brushes {
            name_key(name)?;
            if !(MIN_BRUSH_SIZE..=MAX_BRUSH_SIZE).contains(&brush.size) {
                return Err("Invalid saved brush size".into());
            }
            if let Some((width, height, data)) = &brush.stamp {
                if *width == 0
                    || *height == 0
                    || *width > MAX_DIMENSION
                    || *height > MAX_DIMENSION
                    || width.checked_mul(*height) != Some(data.len())
                {
                    return Err("Invalid saved image brush dimensions".into());
                }
                pixels = pixels.saturating_add(data.len());
            }
        }
        if pixels > MAX_PRESET_PIXELS {
            return Err("Saved image brushes exceed the library's pixel budget".into());
        }
        for (name, colors) in &self.ramps {
            name_key(name)?;
            if !(2..=256).contains(&colors.len()) {
                return Err("A shading ramp needs 2 to 256 distinct colors".into());
            }
        }
        Ok(())
    }

    pub(super) fn write(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_LIBRARY_BYTES {
            return Err("Brush library exceeds its file size limit".into());
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), sequence));
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let result = file.write_all(&bytes).and_then(|()| file.sync_all());
            drop(file);
            let result = result.and_then(|()| std::fs::rename(&temporary, path));
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
            result
        };
        write().map_err(|error| error.to_string())
    }

    pub(super) fn active_ramp(&self) -> Option<&[Pixel]> {
        let name = self.active_ramp.as_ref()?;
        self.ramps.get(name).map(Vec::as_slice)
    }
}

impl PixelCraft {
    fn edit_brush_library(&mut self, edit: impl FnOnce(&mut BrushLibrary)) -> Result<(), String> {
        let mut next = self.power.brush_library.clone();
        edit(&mut next);
        next.validate()?;
        if self.shot.is_none() {
            next.write(&path())?;
        }
        self.power.brush_library = next;
        Ok(())
    }

    pub(super) fn capture_brush_preset(&mut self, name: &str) -> Result<(), String> {
        let name = name_key(name)?.to_owned();
        let mode = self.power.mode;
        let stamp = if mode == BrushMode::Selection {
            Some(
                self.power
                    .stamp
                    .as_ref()
                    .ok_or("Capture a selection as a brush first")?,
            )
        } else {
            None
        };
        if stamp.is_some_and(|(_, _, data)| data.len() > MAX_PRESET_PIXELS) {
            return Err("This image brush exceeds the library's pixel budget".into());
        }
        let brush = SavedBrush {
            size: self.brush,
            shape: self.session.doc.brush_shape,
            mode,
            reverse: self.power.reverse,
            ink: self
                .power
                .preferences
                .tool_inks
                .get(Tool::Pencil.action())
                .copied()
                .unwrap_or_default(),
            stamp: stamp.cloned(),
            dynamics: self.brush_dynamics(),
            alignment: self.power.image_brush_alignment,
            source_origin: self.power.image_brush_origin,
            ramp: self.power.brush_library.active_ramp.clone(),
        };
        self.edit_brush_library(|library| {
            library.brushes.insert(name, brush);
        })
    }

    pub(super) fn apply_brush_preset(&mut self, name: &str) -> Result<(), String> {
        let brush = self
            .power
            .brush_library
            .brushes
            .get(name)
            .ok_or("Brush preset is unavailable")?
            .clone();
        self.cancel_gesture();
        self.tool = Tool::Pencil;
        self.brush = brush.size;
        if self.session.doc.brush_shape != brush.shape {
            self.remember_action("Change brush shape");
            self.session.doc.brush_shape = brush.shape;
        }
        self.power.mode = brush.mode;
        self.power.reverse = brush.reverse;
        if let Some(stamp) = brush.stamp {
            self.power.stamp = Some(stamp);
            self.power.image_brush_alignment = brush.alignment;
            self.power.image_brush_origin = brush.source_origin;
        }
        self.power.brush_library.active_ramp = brush
            .ramp
            .filter(|name| self.power.brush_library.ramps.contains_key(name));
        self.set_tool_ink(brush.ink);
        self.set_brush_dynamics(brush.dynamics);
        Ok(())
    }

    pub(super) fn capture_shading_ramp(
        &mut self,
        name: &str,
        from: usize,
        to: usize,
    ) -> Result<(), String> {
        let name = name_key(name)?.to_owned();
        let palette = self.session.doc.palette_for(self.session.frame);
        let mut range = palette
            .get(from.min(to)..=from.max(to))
            .ok_or("Choose swatches inside the current palette")?
            .to_vec();
        if from > to {
            range.reverse();
        }
        let mut seen = std::collections::HashSet::new();
        range.retain(|color| color[3] != 0 && seen.insert(*color));
        if range.len() < 2 {
            return Err("Choose at least two distinct visible swatches".into());
        }
        self.edit_brush_library(|library| {
            library.ramps.insert(name, range);
        })
    }

    pub(super) fn select_shading_ramp(&mut self, name: Option<&str>) -> Result<(), String> {
        if name.is_some_and(|name| !self.power.brush_library.ramps.contains_key(name)) {
            return Err("Shading ramp is unavailable".into());
        }
        self.cancel_gesture();
        self.tool = Tool::Pencil;
        self.power.mode = BrushMode::Shade;
        self.power.brush_library.active_ramp = name.map(ToOwned::to_owned);
        self.power.shade_seen.clear();
        Ok(())
    }

    pub(crate) fn brush_library_ui(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Brush library", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.power.brush_library.name)
                    .hint_text("Preset name")
                    .char_limit(96),
            );
            let name = self.power.brush_library.name.trim().to_owned();
            let save_brush = if self.power.brush_library.brushes.contains_key(&name) {
                "Update brush"
            } else {
                "Save current brush"
            };
            if ui.pc_button(save_brush).clicked() {
                self.status = self
                    .capture_brush_preset(&name)
                    .err()
                    .unwrap_or_else(|| format!("Saved brush {name}"));
            }

            ui.separator();
            ui.label("Shading ramp from palette");
            let last = self
                .session
                .doc
                .palette_for(self.session.frame)
                .len()
                .saturating_sub(1);
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.power.ramp_from)
                        .range(0..=last)
                        .prefix("From "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.power.ramp_to)
                        .range(0..=last)
                        .prefix("To "),
                );
            });
            let save_ramp = if self.power.brush_library.ramps.contains_key(&name) {
                "Update ramp"
            } else {
                "Save shading ramp"
            };
            if ui.pc_button(save_ramp).clicked() {
                self.status = self
                    .capture_shading_ramp(&name, self.power.ramp_from, self.power.ramp_to)
                    .err()
                    .unwrap_or_else(|| format!("Saved ramp {name}"));
            }

            ui.separator();
            let brushes: Vec<_> = self.power.brush_library.brushes.keys().cloned().collect();
            let ramps: Vec<_> = self.power.brush_library.ramps.keys().cloned().collect();
            egui::ScrollArea::vertical()
                .max_height(270.)
                .show(ui, |ui| {
                    for name in brushes {
                        ui.horizontal(|ui| {
                            if ui.pc_button(&name).on_hover_text("Load brush").clicked() {
                                self.status = self
                                    .apply_brush_preset(&name)
                                    .err()
                                    .unwrap_or_else(|| format!("Loaded brush {name}"));
                                ui.close();
                            }
                            if ui
                                .pc_button("Remove")
                                .on_hover_text("Remove saved brush")
                                .clicked()
                            {
                                self.status = self
                                    .edit_brush_library(|library| {
                                        library.brushes.remove(&name);
                                    })
                                    .err()
                                    .unwrap_or_else(|| format!("Removed brush {name}"));
                            }
                        });
                    }
                    if !ramps.is_empty() && ui.pc_button("Shade in palette order").clicked() {
                        let _ = self.select_shading_ramp(None);
                        ui.close();
                    }
                    for name in ramps {
                        ui.horizontal(|ui| {
                            let active =
                                self.power.brush_library.active_ramp.as_deref() == Some(&name);
                            if ui
                                .selectable_label(active, &name)
                                .on_hover_text("Use shading ramp")
                                .clicked()
                            {
                                self.status = self
                                    .select_shading_ramp(Some(&name))
                                    .err()
                                    .unwrap_or_else(|| format!("Using shading ramp {name}"));
                                ui.close();
                            }
                            if ui
                                .pc_button("Remove")
                                .on_hover_text("Remove saved ramp")
                                .clicked()
                            {
                                self.status = self
                                    .edit_brush_library(|library| {
                                        library.ramps.remove(&name);
                                        if library.active_ramp.as_deref() == Some(&name) {
                                            library.active_ramp = None;
                                        }
                                    })
                                    .err()
                                    .unwrap_or_else(|| format!("Removed ramp {name}"));
                            }
                        });
                    }
                });
        });
    }
}
