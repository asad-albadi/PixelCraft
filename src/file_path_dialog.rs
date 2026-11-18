//! Path entry for editor file commands. Each command still offers its native picker.
use super::*;

pub(super) enum FilePathPurpose {
    Open,
    ImportLayer,
    Save,
    Export {
        kind: String,
        extension: &'static str,
    },
}

pub(super) struct FilePathDialog {
    purpose: FilePathPurpose,
    directory: PathBuf,
    pub(super) input: String,
    error: Option<String>,
    overwrite: Option<PathBuf>,
    focus: bool,
}

impl FilePathPurpose {
    fn title(&self) -> &'static str {
        match self {
            Self::Open => "Open file",
            Self::ImportLayer => "Import image as layer",
            Self::Save => "Save sprite as",
            Self::Export { kind, .. } if kind == "png-sequence" => "Export sequence folder",
            Self::Export { .. } => "Export file",
        }
    }

    fn action(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::ImportLayer => "Import",
            Self::Save => "Save",
            Self::Export { .. } => "Export",
        }
    }

    fn is_folder(&self) -> bool {
        matches!(self, Self::Export { kind, .. } if kind == "png-sequence")
    }

    fn output_extension(&self) -> Option<&'static str> {
        match self {
            Self::Save => Some("pixel"),
            Self::Export { extension, .. } if !self.is_folder() => Some(extension),
            _ => None,
        }
    }

    fn accepts_extension(&self, ext: &str) -> bool {
        match self {
            Self::Save => matches!(ext, "pixel" | "ase" | "aseprite"),
            Self::Export { kind, extension } => {
                ext.eq_ignore_ascii_case(extension)
                    || (kind == "jpeg" && ext.eq_ignore_ascii_case("jpeg"))
                    || (kind == "baked-ase" && ext.eq_ignore_ascii_case("ase"))
            }
            _ => true,
        }
    }
}

impl FilePathDialog {
    fn browse(&self) -> Option<PathBuf> {
        let picker = rfd::FileDialog::new().set_directory(&self.directory);
        match &self.purpose {
            FilePathPurpose::Open => picker
                .add_filter(
                    "Project, sprite or image",
                    &[
                        "pixel", "aseprite", "ase", "png", "gif", "webp", "apng", "ora", "json",
                        "tmj", "tsj", "bmp", "jpg", "jpeg", "ico", "pcx", "qoi", "tga", "fli",
                        "flc", "psd", "psb",
                    ],
                )
                .pick_file(),
            FilePathPurpose::ImportLayer => picker
                .add_filter(
                    "Raster image",
                    pixelcraft::productivity::RASTER_LAYER_EXTENSIONS,
                )
                .pick_file(),
            FilePathPurpose::Save => picker
                .add_filter("PixelCraft project", &["pixel"])
                .add_filter("ASE sprite", &["aseprite", "ase"])
                .set_file_name(self.suggested_name())
                .save_file(),
            FilePathPurpose::Export { extension, .. } if !self.purpose.is_folder() => picker
                .add_filter("Export", &[*extension])
                .set_file_name(self.suggested_name())
                .save_file(),
            FilePathPurpose::Export { .. } => picker.pick_folder(),
        }
    }

    fn suggested_name(&self) -> String {
        Path::new(self.input.trim().trim_matches('"'))
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    fn validate(&self, mut path: PathBuf) -> Result<PathBuf, String> {
        if self.purpose.is_folder() {
            return path
                .is_dir()
                .then_some(path)
                .ok_or_else(|| "Choose an existing folder.".into());
        }
        if path.is_dir() {
            return Err("Choose a file path.".into());
        }
        if let Some(extension) = self.purpose.output_extension() {
            match path.extension() {
                None => {
                    path.set_extension(extension);
                }
                Some(ext)
                    if ext
                        .to_str()
                        .is_some_and(|ext| self.purpose.accepts_extension(ext)) => {}
                _ if matches!(self.purpose, FilePathPurpose::Save) => {
                    return Err("Use a .pixel, .ase, or .aseprite filename.".into());
                }
                _ => return Err(format!("Use a .{extension} filename.")),
            }
        }
        Ok(path)
    }

    fn typed(&self) -> Result<PathBuf, String> {
        let text = self.input.trim();
        let text = text
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(text);
        if text.is_empty() || text.contains('\0') {
            return Err("Enter a file or folder path.".into());
        }
        self.validate(self.directory.join(text))
    }
}

impl PixelCraft {
    pub(super) fn begin_file_path(&mut self, purpose: FilePathPurpose, input: String) {
        let directory = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| extensions::projects_directory().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        self.file_path = Some(FilePathDialog {
            purpose,
            directory,
            input,
            error: None,
            overwrite: None,
            focus: true,
        });
    }

    pub(super) fn file_path_dialog(&mut self, ctx: &egui::Context) {
        let mut dialog = self.file_path.take().unwrap();
        let mut chosen = None;
        let mut cancelled = false;
        let close = fluent::modal_dialog(ctx, dialog.purpose.title(), |ui| {
            if let Some(path) = &dialog.overwrite {
                ui.label("A file already exists at this path.");
                ui.label(path.display().to_string());
                return ui
                    .horizontal(|ui| {
                        if ui.pc_button("Replace file").clicked() {
                            chosen = dialog.overwrite.take();
                            true
                        } else if ui.pc_button("Change path").clicked() {
                            dialog.overwrite = None;
                            dialog.focus = true;
                            false
                        } else {
                            cancelled = ui.pc_button("Cancel").clicked();
                            cancelled
                        }
                    })
                    .inner;
            }

            ui.label(if dialog.purpose.is_folder() {
                "Folder path"
            } else {
                "File path"
            });
            let mut entry = egui::TextEdit::singleline(&mut dialog.input)
                .desired_width(ui.available_width())
                .show(ui);
            if std::mem::take(&mut dialog.focus) {
                entry.response.request_focus();
                entry
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(dialog.input.chars().count()),
                    )));
                entry.state.store(ui.ctx(), entry.response.id);
            }
            ui.small(format!("Relative paths use {}", dialog.directory.display()));
            if let Some(error) = &dialog.error {
                ui.label(error);
            }
            let enter = entry.response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.horizontal(|ui| {
                if ui.pc_button(dialog.purpose.action()).clicked() || enter {
                    match dialog.typed() {
                        Ok(path)
                            if dialog.purpose.output_extension().is_some()
                                && std::fs::symlink_metadata(&path).is_ok() =>
                        {
                            dialog.overwrite = Some(path);
                        }
                        Ok(path) => chosen = Some(path),
                        Err(error) => dialog.error = Some(error),
                    }
                }
                if ui.pc_button("Browse…").clicked()
                    && let Some(path) = dialog.browse()
                {
                    match dialog.validate(path.clone()) {
                        Ok(resolved)
                            if resolved != path
                                && dialog.purpose.output_extension().is_some()
                                && std::fs::symlink_metadata(&resolved).is_ok() =>
                        {
                            dialog.overwrite = Some(resolved);
                        }
                        Ok(path) => chosen = Some(path),
                        Err(error) => dialog.error = Some(error),
                    }
                }
                cancelled = ui.pc_button("Cancel").clicked();
                chosen.is_some() || cancelled
            })
            .inner
        });
        if let Some(path) = chosen {
            match dialog.purpose {
                FilePathPurpose::Open => self.open_recent(path),
                FilePathPurpose::ImportLayer => self.open_as_raster_layer(path),
                FilePathPurpose::Save => {
                    if self.save_to_path(path)
                        && let Some(pending) = self.pending.take()
                    {
                        self.finish_pending(ctx, pending);
                    }
                }
                FilePathPurpose::Export { kind, .. } => {
                    let path = if kind == "png-sequence" {
                        path.join(self.sequence_name.trim())
                    } else {
                        path
                    };
                    self.start_export(&kind, path);
                }
            }
        } else if !close && !cancelled {
            self.file_path = Some(dialog);
        }
    }
}
