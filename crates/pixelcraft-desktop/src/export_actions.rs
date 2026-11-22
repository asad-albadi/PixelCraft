use super::*;
use crate::api::workflows::SheetOptions;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[cfg(windows)]
type OutputKey = Vec<u16>;
#[cfg(not(windows))]
type OutputKey = PathBuf;
fn output_key(path: &Path) -> OutputKey {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut value: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        let prefix: Vec<_> = r"\\?\".encode_utf16().collect();
        if value.starts_with(&prefix) {
            value.drain(..prefix.len());
            let unc: Vec<_> = "UNC\\".encode_utf16().collect();
            if value.starts_with(&unc) {
                value.drain(..unc.len());
                value.splice(..0, [92, 92]);
            }
        }
        value
    }
    #[cfg(not(windows))]
    {
        absolute
    }
}

#[derive(Clone)]
pub(super) struct ExportRecipe {
    pub(super) path: PathBuf,
    pub(super) kind: ExportKind,
    pub(super) published: Arc<Vec<PathBuf>>,
}

#[derive(Clone)]
pub(super) enum ExportKind {
    Raster {
        frame_id: u32,
        scale: usize,
        sheet: bool,
    },
    Animation {
        format: &'static str,
        scale: usize,
    },
    Sequence {
        scale: usize,
    },
    BakedAse,
    Sheet {
        options: Box<SheetOptions>,
        slice: Option<String>,
        layers: Option<Vec<String>>,
        tag: Option<String>,
    },
    Tileset {
        columns: usize,
        layers: Option<Vec<String>>,
    },
    Tiled {
        frame_id: u32,
    },
    Tags {
        gif: bool,
        options: Box<SheetOptions>,
    },
    MultiAtlas {
        inputs: Vec<PathBuf>,
        options: crate::api::atlas::AtlasOptions,
        packed: Option<Arc<(image::RgbaImage, serde_json::Value)>>,
    },
    Palette {
        colors: Vec<Pixel>,
        extension: String,
    },
}

pub(super) struct ExportPreflight {
    document: u64,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    receiver: Receiver<Result<(ExportConfirmation, bool), String>>,
}

pub(super) struct ExportConfirmation {
    recipe: ExportRecipe,
    snapshot: Document,
    plan: crate::api::automation::EditorOutputPlan,
}

pub(super) struct ExportJob {
    origin: String,
    recipe: ExportRecipe,
    receiver: Receiver<Result<(), String>>,
}

impl ExportJob {
    #[cfg(test)]
    pub(super) fn recv_timeout(
        self,
        timeout: std::time::Duration,
    ) -> Result<(PathBuf, Result<(), String>), mpsc::RecvTimeoutError> {
        self.receiver
            .recv_timeout(timeout)
            .map(|result| (self.recipe.path, result))
    }
}

impl ExportRecipe {
    #[cfg(test)]
    fn output_plan(
        &self,
        doc: &Document,
    ) -> Result<crate::api::automation::EditorOutputPlan, String> {
        self.output_plan_cancellable(doc, || false)
    }
    fn output_plan_cancellable(
        &self,
        doc: &Document,
        cancelled: impl Fn() -> bool + Send + Sync,
    ) -> Result<crate::api::automation::EditorOutputPlan, String> {
        let path = &self.path;
        let directory = matches!(
            self.kind,
            ExportKind::Sequence { .. } | ExportKind::Tags { .. }
        );
        let paths = match &self.kind {
            ExportKind::Sheet { options, .. } => {
                crate::api::interchange::sheet_output_paths(path, options)
            }
            ExportKind::MultiAtlas { options, .. }
                if options.metadata != crate::api::atlas::AtlasMetadata::None =>
            {
                vec![path.clone(), path.with_extension("json")]
            }
            ExportKind::Tags { gif, options } => {
                crate::api::productivity::tag_export_paths(path, doc, *gif)
                    .into_iter()
                    .flat_map(|p| {
                        if *gif {
                            vec![p]
                        } else {
                            crate::api::interchange::sheet_output_paths(&p, options)
                        }
                    })
                    .collect()
            }
            ExportKind::Sequence { .. } => (1..=doc.durations.len())
                .map(|i| path.join(format!("frame-{i:04}.png")))
                .collect(),
            ExportKind::Tiled { .. } => {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                let mut files = vec![path.clone()];
                for i in 0..doc.tilesets.len() {
                    files.push(path.with_file_name(format!("{stem}-tiles-{i}.png")));
                    files.push(path.with_file_name(format!("{stem}-tiles-{i}.tsj")));
                }
                files
            }
            _ => vec![path.clone()],
        };
        crate::api::automation::EditorOutputPlan::with_cancel(
            path.clone(),
            paths,
            directory,
            cancelled,
        )
    }
    #[cfg(test)]
    fn write(&self, doc: &Document, confirmed: bool) -> Result<(), String> {
        let plan = self.output_plan(doc)?;
        let previous = plan.output().to_owned();
        plan.write(confirmed, |temporary| {
            self.encode(temporary, doc, &previous)
        })
    }
    fn encode(&self, path: &Path, doc: &Document, previous: &Path) -> Result<(), String> {
        use crate::api::{automation, interchange};
        let prepare = |args: serde_json::Value| {
            automation::prepare_live_export(doc, 0, &args, "")
                .and_then(|export| export.write())
                .map(|_| ())
        };
        let frame = |id| {
            (0..doc.durations.len())
                .find(|&frame| doc.frame_export_id(frame) == id)
                .ok_or_else(|| "The exported frame was deleted".to_owned())
        };
        match &self.kind {
            ExportKind::Palette { colors, extension } => {
                crate::api::productivity::save_palette(path, colors, extension)
            }
            ExportKind::Raster {
                frame_id,
                scale,
                sheet,
            } => formats::export_raster_scaled(path, doc, frame(*frame_id)?, *sheet, *scale),
            ExportKind::Animation { format, scale } => {
                automation::write_animation_export(format, path, doc, *scale)
            }
            ExportKind::Sequence { scale } => automation::encode_editor_sequence(path, doc, *scale),
            ExportKind::BakedAse => formats::export_baked_ase(path, doc),
            ExportKind::Sheet {
                options,
                slice,
                layers,
                tag,
            } => {
                let mut options = options.as_ref().clone();
                options.layers = resolve_layer_ids(doc, layers.as_deref())?;
                if let Some(name) = tag
                    && options
                        .tag
                        .and_then(|index| doc.tags.get(index))
                        .is_none_or(|tag| &tag.name != name)
                {
                    let mut matches = doc
                        .tags
                        .iter()
                        .enumerate()
                        .filter(|(_, tag)| &tag.name == name);
                    let (index, _) = matches
                        .next()
                        .ok_or("The exported tag was removed or renamed")?;
                    if matches.next().is_some() {
                        return Err("More than one tag has the saved export name".into());
                    }
                    options.tag = Some(index);
                }
                interchange::export_sheet_for_slice_with_previous(
                    path,
                    previous,
                    doc,
                    &options,
                    slice.as_deref(),
                )
            }
            ExportKind::Tileset { columns, layers } => {
                let mut args =
                    serde_json::json!({"output":path,"format":"tileset-atlas","overwrite":true});
                if let Some(layers) = resolve_layer_ids(doc, layers.as_deref())? {
                    args["layers"] = serde_json::json!(layers);
                }
                if *columns != 0 {
                    args["columns"] = serde_json::json!(columns);
                }
                prepare(args)
            }
            ExportKind::Tiled { frame_id } => {
                interchange::export_tiled(path, doc, frame(*frame_id)?)
            }
            ExportKind::Tags { gif, options } => {
                crate::api::productivity::batch_tags_with_previous(
                    path, previous, doc, *gif, options,
                )
                .map(|_| ())
            }
            ExportKind::MultiAtlas {
                inputs: _,
                options,
                packed,
            } => {
                use crate::api::atlas::save_packed_atlas_with_metadata;
                let packed = packed.as_ref().ok_or("Atlas preview is not ready")?;
                save_packed_atlas_with_metadata(path, packed, options.metadata)
            }
        }
    }
}

pub(super) fn load_atlas_inputs(inputs: &[PathBuf]) -> Result<Vec<(String, Document)>, String> {
    inputs
        .iter()
        .map(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok((name, formats::load(path)?))
        })
        .collect()
}

pub(super) fn export_layer_ids(doc: &Document, indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .filter_map(|&index| doc.layers.get(index))
        .map(|layer| layer.editor_id.to_string())
        .collect()
}

fn resolve_layer_ids(doc: &Document, ids: Option<&[String]>) -> Result<Option<Vec<usize>>, String> {
    ids.map(|ids| {
        let indices: std::collections::HashMap<_, _> = doc
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.editor_id.to_string(), index))
            .collect();
        ids.iter()
            .map(|id| {
                indices
                    .get(id)
                    .copied()
                    .ok_or_else(|| "An exported layer was deleted".to_owned())
            })
            .collect()
    })
    .transpose()
}

impl PixelCraft {
    pub(super) fn can_repeat_export(&self) -> bool {
        !self.export_busy() && self.last_export.is_some()
    }

    pub(super) fn repeat_export(&mut self) {
        if self.export_job.is_some() {
            self.status = "An export is already running".into();
        } else if let Some(recipe) = self.last_export.clone() {
            self.start_export_recipe(recipe, true);
        } else {
            self.status = "Export this sprite first".into();
        }
    }

    pub(super) fn export_busy(&self) -> bool {
        self.export_job.is_some()
            || self.export_preflight.is_some()
            || self.export_confirmation.is_some()
    }
    pub(super) fn export_dialog_open(&self) -> bool {
        self.export_confirmation.is_some()
            || self
                .export_preflight
                .as_ref()
                .is_some_and(|job| !job.cancelled.load(std::sync::atomic::Ordering::Relaxed))
    }
    pub(super) fn start_export_recipe(&mut self, recipe: ExportRecipe, repeat: bool) {
        self.request_export_recipe(recipe, repeat, &[]);
    }
    pub(super) fn request_export_recipe(
        &mut self,
        mut recipe: ExportRecipe,
        repeat: bool,
        confirmed: &[PathBuf],
    ) {
        if self.export_busy() {
            self.status = "An export is already running".into();
            return;
        }
        if repeat && let ExportKind::MultiAtlas { packed, .. } = &mut recipe.kind {
            *packed = None;
        }
        let mut approved = confirmed.to_vec();
        if repeat {
            approved.extend(recipe.published.iter().cloned());
        }
        self.spawn_export_preflight(recipe, approved, move |recipe, doc, cancelled| {
            if !repeat
                && matches!(recipe.kind, ExportKind::Sequence { .. })
                && std::fs::symlink_metadata(&recipe.path).is_ok()
            {
                return Err("Choose a new folder name for a new PNG sequence".into());
            }
            if let ExportKind::MultiAtlas {
                inputs,
                options,
                packed,
            } = &mut recipe.kind
                && packed.is_none()
            {
                let sources = load_atlas_inputs(inputs)?;
                *packed = Some(Arc::new(crate::api::atlas::pack_atlas(&sources, options)?));
            }
            recipe.output_plan_cancellable(doc, || {
                cancelled.load(std::sync::atomic::Ordering::Relaxed)
            })
        });
    }
    fn spawn_export_preflight(
        &mut self,
        recipe: ExportRecipe,
        approved: Vec<PathBuf>,
        prepare: impl FnOnce(
            &mut ExportRecipe,
            &Document,
            &std::sync::atomic::AtomicBool,
        ) -> Result<crate::api::automation::EditorOutputPlan, String>
        + Send
        + 'static,
    ) {
        if self.export_busy() {
            self.status = "An export is already running".into();
            return;
        }
        let snapshot = self.session.doc.clone();
        let document = snapshot.editor_id.runtime_key();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let (tx, receiver) = mpsc::channel();
        let wake = self.export_context.clone();
        self.status = "Checking export outputs…".into();
        self.export_preflight = Some(ExportPreflight {
            document,
            cancelled,
            receiver,
        });
        std::thread::spawn(move || {
            let mut recipe = recipe;
            let approved: std::collections::HashSet<_> =
                approved.iter().map(|path| output_key(path)).collect();
            let result = prepare(&mut recipe, &snapshot, &worker_cancelled).map(|plan| {
                let needs_confirmation = plan
                    .existing()
                    .iter()
                    .any(|path| !approved.contains(&output_key(path)));
                (
                    ExportConfirmation {
                        recipe,
                        snapshot,
                        plan,
                    },
                    needs_confirmation,
                )
            });
            let _ = tx.send(result);
            wake.request_repaint();
        });
    }
    fn poll_export_preflight(&mut self) {
        let Some(job) = &self.export_preflight else {
            return;
        };
        if job.document != self.session.doc.editor_id.runtime_key() {
            job.cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let result = job.receiver.try_recv();
        if matches!(result, Err(TryRecvError::Empty)) {
            return;
        }
        let job = self.export_preflight.take().unwrap();
        if job.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            self.status = "Export cancelled".into();
            return;
        }
        match result {
            Ok(Ok((request, needs_confirmation))) => {
                if needs_confirmation {
                    self.export_confirmation = Some(request);
                } else {
                    self.launch_export(request);
                }
            }
            Ok(Err(error)) => self.status = format!("Export failed: {error}"),
            Err(_) => self.status = "Export check stopped unexpectedly".into(),
        }
    }
    #[cfg(test)]
    pub(super) fn wait_export_preflight(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.export_preflight.is_some() {
            self.poll_export_preflight();
            assert!(Instant::now() < deadline, "export preflight stalled");
            std::thread::yield_now();
        }
    }

    fn launch_export(&mut self, request: ExportConfirmation) {
        let ExportConfirmation {
            mut recipe,
            snapshot: doc,
            plan,
        } = request;
        recipe.published = plan.shared_paths();
        let (tx, receiver) = mpsc::channel();
        self.status = format!(
            "Exporting {}",
            recipe
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        self.export_job = Some(ExportJob {
            origin: doc.editor_id.to_string(),
            recipe: recipe.clone(),
            receiver,
        });
        let previous = plan.output().to_owned();
        std::thread::spawn(move || {
            let _ =
                tx.send(plan.write(true, |temporary| recipe.encode(temporary, &doc, &previous)));
        });
    }
    pub(super) fn export_confirmation_dialog(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.export_preflight {
            if !job.cancelled.load(std::sync::atomic::Ordering::Relaxed)
                && fluent::modal_dialog(ctx, "Checking export outputs", |ui| {
                    ui.label("Checking files before export…");
                    ui.pc_button("Cancel").clicked()
                })
            {
                job.cancelled
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            return;
        }
        let Some(request) = self.export_confirmation.take() else {
            return;
        };
        if request.snapshot.editor_id.runtime_key() != self.session.doc.editor_id.runtime_key() {
            self.status = "Export confirmation closed because the active document changed".into();
            return;
        }
        let mut replace = false;
        let dismiss = fluent::modal_dialog(ctx, "Replace export outputs?", |ui| {
            ui.label("These files already exist. Replace the listed files?");
            let rows = request.plan.existing();
            let height = ui.text_style_height(&egui::TextStyle::Body);
            egui::ScrollArea::vertical().max_height(180.).show_rows(
                ui,
                height,
                rows.len(),
                |ui, range| {
                    for index in range {
                        ui.add(egui::Label::new(rows[index].display().to_string()).truncate())
                            .on_hover_text(rows[index].display().to_string());
                    }
                },
            );
            ui.horizontal(|ui| {
                replace = ui.pc_button("Replace listed files").clicked();
                replace || ui.pc_button("Cancel").clicked()
            })
            .inner
        });
        if replace {
            self.launch_export(request);
        } else if !dismiss {
            self.export_confirmation = Some(request);
        }
    }

    pub(super) fn poll_export(&mut self) {
        self.poll_export_preflight();
        let Some(job) = &self.export_job else { return };
        let result = job.receiver.try_recv();
        if matches!(result, Err(TryRecvError::Empty)) {
            return;
        }
        let job = self.export_job.take().unwrap();
        self.status = match result {
            Ok(Ok(())) => {
                let label = format!(
                    "Exported {}",
                    job.recipe
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                if !matches!(job.recipe.kind, ExportKind::Palette { .. }) {
                    if self.session.doc.editor_id.to_string() == job.origin {
                        self.last_export = Some(job.recipe);
                    } else if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .flatten()
                        .find(|tab| tab.doc.editor_id.to_string() == job.origin)
                    {
                        tab.last_export = Some(job.recipe);
                    }
                }
                label
            }
            Ok(Err(error)) => format!("Export failed: {error}"),
            Err(_) => "Export worker stopped unexpectedly".into(),
        };
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    fn root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("pixelcraft-pr26-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn wait(app: &mut PixelCraft) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.export_job.is_some() || app.export_preflight.is_some() {
            app.poll_export();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    fn recipe(path: PathBuf, kind: ExportKind) -> ExportRecipe {
        ExportRecipe {
            path,
            kind,
            published: Default::default(),
        }
    }
    #[cfg(windows)]
    #[test]
    fn sheet_metadata_survives_export_through_a_directory_alias() {
        use std::os::windows::process::CommandExt;
        let root = root("sheet-alias");
        let physical = root.join("physical");
        let alias = root.join("alias");
        std::fs::create_dir_all(&physical).unwrap();
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&alias)
            .arg(&physical)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(status.status.success());
        let mut doc = Document::new(2, 2);
        let export = recipe(
            alias.join("sheet.png"),
            ExportKind::Sheet {
                options: Box::new(SheetOptions {
                    preset: "Unity".into(),
                    ..Default::default()
                }),
                slice: None,
                layers: None,
                tag: None,
            },
        );
        let first = export.write(&doc, false);
        if let Err(error) = first {
            std::fs::remove_dir(&alias).unwrap();
            std::fs::remove_dir_all(&root).unwrap();
            panic!("alias export failed: {error}");
        }
        let meta_path = physical.join("sheet.png.meta");
        let generated = std::fs::read_to_string(&meta_path).unwrap();
        let guid_line = generated
            .lines()
            .find(|line| line.starts_with("guid: "))
            .unwrap();
        let previous = generated.replace(guid_line, "guid: 1234567890abcdef1234567890abcdef");
        assert_ne!(previous, generated);
        std::fs::write(&meta_path, &previous).unwrap();
        doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        export.write(&doc, true).unwrap();
        assert_eq!(
            std::fs::read(physical.join("sheet.png.meta")).unwrap(),
            previous.as_bytes(),
            "Unity identity must survive an alias re-export"
        );
        std::fs::remove_dir(&alias).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn case_sensitive_output_names_never_share_overwrite_authorization() {
        assert_ne!(
            output_key(Path::new("C:/art/001-a.gif")),
            output_key(Path::new("C:/art/001-A.gif"))
        );
    }
    #[test]
    fn palette_export_does_not_replace_the_artwork_repeat_target() {
        let root = root("palette-history");
        let png = root.join("art.png");
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.start_export("png", png.clone());
        wait(&mut app);
        assert_eq!(app.last_export.as_ref().unwrap().path, png);
        app.start_export_recipe(
            recipe(
                root.join("colors.hex"),
                ExportKind::Palette {
                    colors: vec![[255, 0, 0, 255]],
                    extension: "hex".into(),
                },
            ),
            false,
        );
        wait(&mut app);
        assert_eq!(
            app.last_export.as_ref().unwrap().path,
            png,
            "palette export stole Repeat Last Export"
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repeat_prompts_for_new_conflicts_outside_the_successful_manifest() {
        let root = root("repeat-manifest");
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(2, 2), None);
        app.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        let tag = |name: &str| FrameTag {
            editor_id: Default::default(),
            name: name.into(),
            from: 0,
            to: 0,
            direction: 0,
            repeat: 0,
            color: [1, 2, 3],
        };
        app.doc.tags.push(tag("first"));
        app.start_export_recipe(
            recipe(
                root.clone(),
                ExportKind::Tags {
                    gif: true,
                    options: Box::default(),
                },
            ),
            false,
        );
        wait(&mut app);
        assert!(app.last_export.is_some(), "{}", app.status);
        app.doc.tags.push(tag("second"));
        let foreign = crate::api::productivity::tag_export_paths(&root, &app.doc, true)[1].clone();
        std::fs::write(&foreign, b"foreign tag output").unwrap();
        app.repeat_export();
        wait(&mut app);
        assert!(
            app.export_confirmation.is_some(),
            "new conflict bypassed confirmation: {}",
            app.status
        );
        assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign tag output");
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn initial_atlas_publishes_its_preview_and_repeat_reloads_inputs() {
        let root = root("atlas-snapshot");
        let source = root.join("source.pixel");
        let output = root.join("atlas.png");
        let mut red = Document::new(2, 2);
        red.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        formats::save(&source, &red).unwrap();
        let options = crate::api::atlas::AtlasOptions {
            width: 16,
            padding: 0,
            trim: false,
            ..Default::default()
        };
        let packed = Arc::new(
            crate::api::atlas::pack_atlas(&[("source.pixel".into(), red.clone())], &options)
                .unwrap(),
        );
        let mut blue = red.clone();
        blue.set_pixel(0, 0, 0, [0, 0, 255, 255]);
        formats::save(&source, &blue).unwrap();
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.start_export_recipe(
            recipe(
                output.clone(),
                ExportKind::MultiAtlas {
                    inputs: vec![source],
                    options,
                    packed: Some(packed),
                },
            ),
            false,
        );
        wait(&mut app);
        assert_eq!(
            image::open(&output).unwrap().into_rgba8().get_pixel(0, 0).0,
            [255, 0, 0, 255]
        );
        app.repeat_export();
        wait(&mut app);
        assert_eq!(
            image::open(&output).unwrap().into_rgba8().get_pixel(0, 0).0,
            [0, 0, 255, 255]
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    fn draw(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 650.))),
                events,
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        output.textures_delta.clear();
        output
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
            .unwrap_or_else(|| panic!("missing {label}"))
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
    #[test]
    fn blocked_preflight_keeps_rendering_and_can_be_cancelled_without_spawning_more_work() {
        let root = root("preflight-gate");
        let path = root.join("new.png");
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let (started, ready) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let request = recipe(
            path.clone(),
            ExportKind::Raster {
                frame_id: app.doc.frame_export_id(0),
                scale: 1,
                sheet: false,
            },
        );
        app.spawn_export_preflight(request, Vec::new(), move |recipe, doc, cancelled| {
            started.send(()).unwrap();
            gate.recv().unwrap();
            recipe.output_plan_cancellable(doc, || {
                cancelled.load(std::sync::atomic::Ordering::Relaxed)
            })
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let frame = ctx.cumulative_frame_nr();
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        assert!(ctx.cumulative_frame_nr() > frame);
        draw(&mut app, &ctx, click(position(&output, "Cancel")));
        assert!(!app.export_dialog_open());
        assert!(app.export_busy());
        release.send(()).unwrap();
        wait(&mut app);
        assert!(!path.exists());
        assert!(app.export_preflight.is_none());
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sidecar_confirmation_cancel_and_replace_are_reachable_without_parent_modal_overlap() {
        let root = root("confirmation-ui");
        let path = root.join("sheet.png");
        let meta = path.with_extension("json");
        std::fs::write(&meta, b"{\"foreign\":true}").unwrap();
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        app.studio.sheet_open = true;
        let request = recipe(
            path.clone(),
            ExportKind::Sheet {
                options: Box::default(),
                slice: None,
                layers: None,
                tag: None,
            },
        );
        app.start_export_recipe(request.clone(), false);
        wait(&mut app);
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        position(&output, "Replace listed files");
        assert!(!output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text()=="Export PNG + metadata…")));
        draw(&mut app, &ctx, click(position(&output, "Cancel")));
        assert_eq!(std::fs::read(&meta).unwrap(), b"{\"foreign\":true}");
        assert!(!path.exists());
        app.start_export_recipe(request, false);
        wait(&mut app);
        draw(&mut app, &ctx, vec![]);
        let output = draw(&mut app, &ctx, vec![]);
        draw(
            &mut app,
            &ctx,
            click(position(&output, "Replace listed files")),
        );
        wait(&mut app);
        assert!(path.exists(), "{}", app.status);
        assert_ne!(std::fs::read(&meta).unwrap(), b"{\"foreign\":true}");
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sheet_and_atlas_export_do_not_replace_unconfirmed_sidecars() {
        for atlas in [false, true] {
            let root = root(if atlas {
                "atlas-consent"
            } else {
                "sheet-consent"
            });
            let path = root.join("output.png");
            let sidecar = path.with_extension("json");
            std::fs::write(&sidecar, b"{\"foreign\":true}").unwrap();
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.replace(Document::new(3, 3), None);
            app.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
            let kind = if atlas {
                let input = root.join("source.pixel");
                formats::save(&input, &app.doc).unwrap();
                ExportKind::MultiAtlas {
                    packed: None,
                    inputs: vec![input],
                    options: crate::api::atlas::AtlasOptions {
                        width: 32,
                        ..Default::default()
                    },
                }
            } else {
                ExportKind::Sheet {
                    options: Box::default(),
                    slice: None,
                    layers: None,
                    tag: None,
                }
            };
            app.start_export_recipe(
                ExportRecipe {
                    published: Default::default(),
                    path: path.clone(),
                    kind,
                },
                false,
            );
            wait(&mut app);
            assert!(
                std::fs::read(&sidecar).unwrap() == b"{\"foreign\":true}",
                "unconfirmed sidecar was overwritten; atlas={atlas}"
            );
            assert!(!path.exists());
            assert!(app.export_confirmation.is_some());
            drop(app);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn fresh_raster_recipe_cannot_clobber_a_file_that_was_not_approved() {
        let root = root("raster-consent");
        let path = root.join("new.png");
        let doc = Document::new(2, 2);
        let recipe = ExportRecipe {
            published: Default::default(),
            path: path.clone(),
            kind: ExportKind::Raster {
                frame_id: doc.frame_export_id(0),
                scale: 1,
                sheet: false,
            },
        };
        std::fs::write(&path, b"created by somebody else").unwrap();
        assert!(
            recipe.write(&doc, false).is_err(),
            "fresh export ignored its no-clobber intent"
        );
        assert!(std::fs::read(&path).unwrap() == b"created by somebody else");
        std::fs::remove_dir_all(root).unwrap();
    }
}
