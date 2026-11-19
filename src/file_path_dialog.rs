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
    replace: bool,
    focus: bool,
    focus_requested: bool,
    pub(super) scale: Option<usize>,
    target: Option<(u64, Option<u32>)>,
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

    fn supports_scale(&self) -> bool {
        matches!(self, Self::Export { kind, .. } if matches!(kind.as_str(),
            "png" | "bmp" | "jpeg" | "ico" | "pcx" | "qoi" | "tga" | "webp" |
            "svg" | "css" | "sheet" | "png-sequence" | "gif" | "mp4" | "fli" | "flc"))
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
        let scale = purpose.supports_scale().then_some(self.studio.sheet.scale);
        let target = (!matches!(purpose, FilePathPurpose::Open)).then(|| {
            let frame = matches!(purpose, FilePathPurpose::ImportLayer)
                || matches!(&purpose, FilePathPurpose::Export {kind,..} if !matches!(kind.as_str(),"gif"|"mp4"|"fli"|"flc"|"png-sequence"|"baked-ase"|"sheet"));
            (self.doc.editor_id.runtime_key(),frame.then(||self.doc.frame_export_id(self.frame)))
        });
        self.file_path = Some(FilePathDialog {
            purpose,
            target,
            directory,
            input,
            error: None,
            overwrite: None,
            replace: false,
            focus: true,
            focus_requested: false,
            scale,
        });
    }

    pub(super) fn file_path_dialog(&mut self, ctx: &egui::Context) {
        let mut dialog = self.file_path.take().unwrap();
        if dialog.target.is_some_and(|(document, frame)| {
            document != self.doc.editor_id.runtime_key()
                || frame.is_some_and(|frame| frame != self.doc.frame_export_id(self.frame))
        }) {
            self.pending = None;
            self.status = "File dialog closed because the active document or frame changed".into();
            return;
        }
        let mut chosen = None;
        let mut cancelled = false;
        let title = dialog.purpose.title();
        let footer_key = egui::Id::new(("file-path-footer-ids", title));
        let close = fluent::modal_dialog(ctx, title, |ui| {
            if let Some(path) = &dialog.overwrite {
                ui.label("A file already exists at this path.");
                ui.label(path.display().to_string());
                if let Some(scale) = dialog.scale {
                    ui.small(format!("Export scale: {scale}×"));
                }
                return ui
                    .push_id(("file-path-footer", title), |ui| {
                        ui.horizontal(|ui| {
                            let replace = ui.pc_button("Replace file");
                            let change = ui.pc_button("Change path");
                            let cancel = ui.pc_button("Cancel");
                            ctx.data_mut(|data| {
                                data.insert_temp(footer_key, vec![replace.id, change.id, cancel.id])
                            });
                            if replace.clicked() {
                                chosen = dialog.overwrite.take();
                                dialog.replace = true;
                                true
                            } else if change.clicked() {
                                dialog.overwrite = None;
                                dialog.replace = false;
                                dialog.focus = true;
                                dialog.focus_requested = false;
                                false
                            } else {
                                cancelled = cancel.clicked();
                                cancelled
                            }
                        })
                        .inner
                    })
                    .inner;
            }
            let mut outside = ctx
                .data(|data| data.get_temp::<Vec<egui::Id>>(footer_key))
                .unwrap_or_default();
            outside.push(egui::Id::new(("editor-dialog", title)).with("close"));
            let enter=egui::ScrollArea::vertical()
                .id_salt(("file-path-options",title))
                .max_height((ctx.content_rect().height()-150.).max(48.))
                .show(ui,|ui| {
            ui.label(if dialog.purpose.is_folder() {
                "Folder path"
            } else {
                "File path"
            });
            let mut entry = egui::TextEdit::singleline(&mut dialog.input)
                .desired_width(ui.available_width())
                .show(ui);
            if dialog.focus {
                // A parent modal can still own focus during its closing pass.
                // Retire the request only after the next pass observes delivery.
                if dialog.focus_requested && entry.response.has_focus() {
                    dialog.focus = false;
                    dialog.focus_requested = false;
                } else {
                    entry.response.request_focus();
                    dialog.focus_requested = true;
                    entry.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(dialog.input.chars().count()),
                    )));
                    entry.state.store(ui.ctx(), entry.response.id);
                }
            }
            ui.small(format!("Relative paths use {}", dialog.directory.display()));
            if let Some(error) = &dialog.error {
                ui.label(error);
            }
            if let Some(scale)=&mut dialog.scale {
                Self::export_scale_value_ui(ui,scale);
                let samples=if matches!(&dialog.purpose,FilePathPurpose::Export {kind,..} if kind=="sheet") {self.doc.durations.len()} else {1};
                let mut width=self.doc.width.saturating_mul(samples).saturating_mul(*scale);
                let mut height=self.doc.height.saturating_mul(*scale);
                if matches!(&dialog.purpose,FilePathPurpose::Export {kind,..} if kind=="mp4") {width=width.next_multiple_of(2);height=height.next_multiple_of(2);}
                ui.small(format!("{width} × {height} pixels"));
            }
            let enter=entry.response.lost_focus() && ui.input(|i|i.key_pressed(Key::Enter));
            fluent::reveal_focused_control(ui,&outside);
            enter
            }).inner;
            ui.separator();
            ui.push_id(("file-path-footer", title), |ui| {
                ui.horizontal(|ui| {
                    let action = ui.pc_button(dialog.purpose.action());
                    let browse = ui.pc_button("Browse…");
                    let cancel = ui.pc_button("Cancel");
                    ctx.data_mut(|data| {
                        data.insert_temp(footer_key, vec![action.id, browse.id, cancel.id])
                    });
                    if action.clicked() || enter {
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
                    if browse.clicked()
                        && let Some(path) = dialog.browse()
                    {
                        match dialog.validate(path.clone()) {
                            Ok(resolved)
                                if dialog.purpose.output_extension().is_some()
                                    && std::fs::symlink_metadata(&resolved).is_ok() =>
                            {
                                dialog.overwrite = Some(resolved);
                                dialog.replace = false;
                            }
                            Ok(path) => {
                                dialog.replace = false;
                                chosen = Some(path);
                            }
                            Err(error) => dialog.error = Some(error),
                        }
                    }
                    cancelled = cancel.clicked();
                    chosen.is_some() || cancelled
                })
                .inner
            })
            .inner
        });
        if let Some(path) = chosen {
            match dialog.purpose {
                FilePathPurpose::Open => self.open_recent(path),
                FilePathPurpose::ImportLayer => self.open_as_raster_layer(path),
                FilePathPurpose::Save => {
                    if self.save_to_path_with_overwrite(path, dialog.replace)
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
                    self.start_export_with_consent(
                        &kind,
                        path,
                        dialog.scale.unwrap_or(self.studio.sheet.scale),
                        dialog.replace,
                    );
                }
            }
        } else if !close && !cancelled {
            self.file_path = Some(dialog);
        }
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;
    fn frame(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.))),
            events,
            ..Default::default()
        };
        input.time = Some(ctx.cumulative_frame_nr() as f64 * 0.1);
        let mut output = ctx.run_ui(input, |ui| app.render(ui));
        output.textures_delta.clear();
        output
    }
    fn position(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible export control {label}"))
    }
    fn click(pos: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }
    fn settled(app: &mut PixelCraft, ctx: &egui::Context) -> egui::FullOutput {
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![])
    }
    fn wait_export(app: &mut PixelCraft) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while (app.export_job.is_some() || app.export_preflight.is_some())
            && Instant::now() < deadline
        {
            app.poll_export();
            // Yield while preflight is pending too; busy polling can starve the
            // filesystem worker on loaded Windows CI hosts.
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            app.export_job.is_none() && app.export_preflight.is_none(),
            "export/preflight worker did not finish"
        );
        assert!(app.status.starts_with("Exported "), "{}", app.status);
    }
    #[test]
    fn cancelled_animation_scales_never_change_the_sheet_draft() {
        for kind in ["gif", "mp4", "fli", "flc", "png-sequence"] {
            for second_stage in [false, true] {
                let ctx = egui::Context::default();
                let mut app = PixelCraft::with_context(&ctx, None);
                app.studio.sheet.scale = 4;
                app.action(kind);
                let output = settled(&mut app, &ctx);
                frame(&mut app, &ctx, click(position(&output, "2×")));
                let mut output = settled(&mut app, &ctx);
                if second_stage {
                    frame(
                        &mut app,
                        &ctx,
                        click(position(&output, "Choose export location…")),
                    );
                    output = settled(&mut app, &ctx);
                    assert_eq!(app.file_path.as_ref().unwrap().scale, Some(2));
                }
                frame(&mut app, &ctx, click(position(&output, "Cancel")));
                assert_eq!(
                    app.studio.sheet.scale, 4,
                    "{kind}, second stage={second_stage}"
                );
                assert!(app.export_job.is_none());
            }
        }
    }
    #[test]
    fn stale_save_confirmation_cannot_overwrite_with_another_document() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(2, 2), None);
        app.doc.set_pixel(0, 0, 0, [10, 20, 30, 255]);
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-stale-dialog-{}.pixel",
            std::process::id()
        ));
        std::fs::write(&path, b"keep original").unwrap();
        app.action("saveas");
        app.file_path.as_mut().unwrap().input = path.to_string_lossy().into_owned();
        let output = settled(&mut app, &ctx);
        frame(&mut app, &ctx, click(position(&output, "Save")));
        let output = settled(&mut app, &ctx);
        let replace = position(&output, "Replace file");
        app.pending = Some(Pending::New);
        app.open_document(Document::new(3, 3), None);
        frame(&mut app, &ctx, click(replace));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"keep original",
            "stale confirmation overwrote the selected file"
        );
        assert!(app.file_path.is_none());
        assert!(app.pending.is_none());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn stale_animation_dialog_closes_before_selecting_an_output() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.action("gif");
        settled(&mut app, &ctx);
        app.open_document(Document::new(3, 3), None);
        settled(&mut app, &ctx);
        assert!(app.animation_export.is_none());
        assert!(app.file_path.is_none());
    }

    #[test]
    fn png_bmp_and_sequence_export_scale_is_selected_in_the_path_flow_and_preserved_on_repeat() {
        for kind in ["png", "bmp", "png-sequence"] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.replace(Document::new(3, 2), None);
            app.doc.set_pixel(0, 0, 0, [200, 10, 30, 255]);
            if kind == "png-sequence" {
                app.doc.add_frame(0, true);
            }
            app.studio.sheet.scale = 1;
            app.action(kind);
            let mut output = settled(&mut app, &ctx);
            if kind == "png-sequence" {
                frame(
                    &mut app,
                    &ctx,
                    click(position(&output, "Choose export location…")),
                );
                output = settled(&mut app, &ctx);
            }
            position(&output, "Export scale");
            frame(&mut app, &ctx, click(position(&output, "2×")));
            let root = std::env::temp_dir().join(format!(
                "pixelcraft-export-scale-{kind}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let destination = if kind == "png-sequence" {
                root.clone()
            } else {
                root.join(format!("sprite.{kind}"))
            };
            app.file_path.as_mut().unwrap().input = destination.to_string_lossy().into_owned();
            output = settled(&mut app, &ctx);
            let export = position(&output, "Export");
            assert!(Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.)).contains(export));
            frame(&mut app, &ctx, click(export));
            assert!(
                app.export_job.is_some() || app.export_preflight.is_some(),
                "{kind} export click did not start worker: path_open={}, error={:?}, overwrite={:?}, status={}, click={export:?}",
                app.file_path.is_some(),
                app.file_path
                    .as_ref()
                    .and_then(|dialog| dialog.error.as_ref()),
                app.file_path
                    .as_ref()
                    .and_then(|dialog| dialog.overwrite.as_ref()),
                app.status
            );
            wait_export(&mut app);
            let image_path = if kind == "png-sequence" {
                root.join(&app.sequence_name).join("frame-0001.png")
            } else {
                destination
            };
            assert_eq!(
                image::image_dimensions(&image_path).unwrap(),
                (6, 4),
                "{kind} selected output scale"
            );
            if kind == "png-sequence" {
                assert_eq!(
                    image::image_dimensions(root.join(&app.sequence_name).join("frame-0002.png"))
                        .unwrap(),
                    (6, 4)
                );
            }
            app.studio.sheet.scale = 4;
            app.doc.set_pixel(0, 0, 0, [10, 210, 50, 255]);
            app.action("repeat-export");
            wait_export(&mut app);
            let rendered = image::open(&image_path).unwrap().into_rgba8();
            assert_eq!(
                rendered.dimensions(),
                (6, 4),
                "repeat must retain accepted scale for {kind}"
            );
            assert_eq!(rendered.get_pixel(0, 0).0, [10, 210, 50, 255]);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn cancelling_an_image_export_keeps_the_sheet_operation_draft() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.studio.sheet.scale = 4;
        app.action("png");
        let output = settled(&mut app, &ctx);
        frame(&mut app, &ctx, click(position(&output, "2×")));
        let output = settled(&mut app, &ctx);
        frame(&mut app, &ctx, click(position(&output, "Cancel")));
        assert!(app.file_path.is_none());
        assert_eq!(app.studio.sheet.scale, 4);
        assert!(app.export_job.is_none());
    }

    #[test]
    fn sheet_export_keyboard_navigation_reveals_bottom_export_control() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(4, 4), None);
        app.action("sheet");
        let output = settled(&mut app, &ctx);
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let target = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Export PNG + metadata…"))
            .map(|(id, _)| *id)
            .expect("sheet export button accessible");
        let tab = || egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        for _ in 0..400 {
            if ctx
                .memory(|m| m.focused())
                .is_some_and(|id| id.accesskit_id() == target)
            {
                break;
            }
            frame(&mut app, &ctx, vec![tab()]);
        }
        let focus = ctx
            .memory(|m| m.focused())
            .expect("sheet export is keyboard reachable");
        assert_eq!(focus.accesskit_id(), target);
        for _ in 0..12 {
            frame(&mut app, &ctx, vec![]);
        }
        let response = ctx.read_response(focus).unwrap();
        assert!(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(683., 384.)).contains_rect(response.rect),
            "focused sheet export offscreen {:?}",
            response.rect
        );
        assert!(
            response
                .interact_rect
                .contains_rect(response.rect.shrink(1.)),
            "focused sheet export clipped {:?}/{:?}",
            response.rect,
            response.interact_rect
        );
    }

    #[test]
    fn typed_image_export_confirms_overwrite_and_keeps_the_selected_local_scale() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(3, 2), None);
        app.doc.set_pixel(0, 0, 0, [210, 30, 80, 255]);
        let root =
            std::env::temp_dir().join(format!("pixelcraft-overwrite-scale-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("Typed sprite.png");
        std::fs::write(&path, b"preserve until confirmed").unwrap();
        app.action("png");
        settled(&mut app, &ctx);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Text(format!("\"{}\"", path.display()))],
        );
        assert_eq!(
            app.file_path.as_ref().unwrap().input,
            format!("\"{}\"", path.display())
        );
        let output = settled(&mut app, &ctx);
        frame(&mut app, &ctx, click(position(&output, "2×")));
        let output = settled(&mut app, &ctx);
        frame(&mut app, &ctx, click(position(&output, "Export")));
        assert!(app.export_job.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve until confirmed");
        let output = settled(&mut app, &ctx);
        position(&output, "Export scale: 2×");
        app.studio.sheet.scale = 4;
        frame(&mut app, &ctx, click(position(&output, "Replace file")));
        wait_export(&mut app);
        let exported = image::open(&path).unwrap().into_rgba8();
        assert_eq!(exported.dimensions(), (6, 4));
        assert_eq!(exported.get_pixel(0, 0).0, [210, 30, 80, 255]);
        assert_eq!(app.studio.sheet.scale, 4);
        assert_eq!((app.doc.width, app.doc.height), (3, 2));
        assert!(app.undo.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
