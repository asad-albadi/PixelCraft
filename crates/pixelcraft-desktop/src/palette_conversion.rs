use super::*;
use crate::api::workflows::generate_artwork_palette_until;
use crate::productivity_ui::GradientPattern;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteAction {
    Generate,
    Apply,
    Convert,
    Grayscale(GrayscaleMethod),
    Rgba,
}

enum PaletteResult {
    Generated(Vec<Pixel>),
    Updated(Box<Document>, &'static str),
    Preview {
        image: egui::ColorImage,
        #[cfg(test)]
        document: Box<Document>,
    },
}

#[derive(Clone, PartialEq, Eq)]
struct PaletteRequest {
    action: PaletteAction,
    palette: Option<Vec<Pixel>>,
    transparent: usize,
    matrix: GradientPattern,
    diffusion: bool,
    strength: u8,
    fit: IndexedFit,
    frame: usize,
    display: Arc<Vec<u8>>,
}

impl PaletteRequest {
    fn same_conversion(&self, other: &Self) -> bool {
        self.action == other.action
            && self.palette == other.palette
            && self.transparent == other.transparent
            && self.matrix == other.matrix
            && self.diffusion == other.diffusion
            && self.strength == other.strength
            && self.fit == other.fit
    }
}

struct PalettePreview {
    request: PaletteRequest,
    #[cfg(test)]
    document: Box<Document>,
    texture: egui::TextureHandle,
}

struct PaletteWork {
    request: Option<PaletteRequest>,
    receiver: mpsc::Receiver<Result<PaletteResult, String>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for PaletteWork {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub(super) struct PaletteConversion {
    generation: u64,
    limit: usize,
    all_frames: bool,
    generated: Option<Vec<Pixel>>,
    transparent: usize,
    matrix: GradientPattern,
    diffusion: bool,
    strength: u8,
    fit: IndexedFit,
    grayscale: GrayscaleMethod,
    target: Option<ColorMode>,
    applying: bool,
    show_preview: bool,
    requested: Option<PaletteRequest>,
    preview: Option<PalettePreview>,
    worker: Option<PaletteWork>,
    error: Option<String>,
}

impl PixelCraft {
    pub(super) fn open_palette_conversion(&mut self) {
        self.stop_playback(false);
        self.palette_conversion = Some(PaletteConversion {
            generation: self.power.bridge_generation,
            limit: 32,
            all_frames: false,
            generated: None,
            transparent: if self.doc.color_mode == ColorMode::Indexed {
                self.doc.transparent_index.min(255)
            } else {
                self.doc
                    .palette
                    .iter()
                    .position(|color| color[3] == 0)
                    .unwrap_or(0)
            },
            matrix: if self.doc.dithering.is_some() {
                GradientPattern::Installed
            } else {
                GradientPattern::Smooth
            },
            diffusion: false,
            strength: 100,
            fit: IndexedFit::Default,
            grayscale: GrayscaleMethod::Luminance,
            target: None,
            applying: false,
            show_preview: true,
            requested: None,
            preview: None,
            worker: None,
            error: None,
        });
    }

    pub(super) fn start_rgba_conversion(&mut self, ctx: &egui::Context) {
        self.open_palette_conversion();
        let mut dialog = self.palette_conversion.take().unwrap();
        dialog.target = Some(ColorMode::Rgba);
        self.schedule_conversion_preview(&mut dialog, ctx);
        self.palette_conversion = Some(dialog);
    }

    pub(super) fn start_grayscale_conversion(
        &mut self,
        method: GrayscaleMethod,
        ctx: &egui::Context,
    ) {
        self.open_palette_conversion();
        let mut dialog = self.palette_conversion.take().unwrap();
        dialog.grayscale = method;
        dialog.target = Some(ColorMode::Grayscale);
        self.schedule_conversion_preview(&mut dialog, ctx);
        self.palette_conversion = Some(dialog);
    }

    pub(super) fn palette_conversion_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.palette_conversion.take() else {
            return;
        };
        if dialog.generation != self.power.bridge_generation {
            self.status = "Palette operation cancelled because the sprite changed".into();
            return;
        }
        if let Some(worker) = &dialog.worker {
            match worker.receiver.try_recv() {
                Ok(result) => {
                    let request = worker.request.clone();
                    let stale = worker.cancelled.load(Ordering::Relaxed)
                        || request.as_ref().is_some_and(|request| {
                            self.palette_request(&dialog)
                                .as_ref()
                                .is_none_or(|current| {
                                    if dialog.applying {
                                        !request.same_conversion(current)
                                    } else {
                                        request != current
                                    }
                                })
                        });
                    dialog.worker = None;
                    dialog.applying = false;
                    if stale {
                        dialog.requested = None;
                        dialog.error = None;
                    } else {
                        match result {
                            Ok(PaletteResult::Generated(colors)) => {
                                dialog.transparent = 0;
                                dialog.generated = Some(colors);
                            }
                            Ok(PaletteResult::Updated(doc, label)) => {
                                self.commit_palette_document(doc, label);
                                return;
                            }
                            Ok(PaletteResult::Preview {
                                image,
                                #[cfg(test)]
                                document,
                            }) => {
                                dialog.preview = Some(PalettePreview {
                                    request: request.expect("Preview work has settings"),
                                    #[cfg(test)]
                                    document,
                                    texture: ctx.load_texture(
                                        "palette-conversion-preview",
                                        image,
                                        egui::TextureOptions::NEAREST,
                                    ),
                                });
                            }
                            Err(error) => dialog.error = Some(error),
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let cancelled = worker.cancelled.load(Ordering::Relaxed);
                    dialog.worker = None;
                    dialog.applying = false;
                    if cancelled {
                        dialog.requested = None;
                        dialog.error = None;
                    } else {
                        dialog.error = Some("Palette worker stopped".into());
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut action = None;
        let mut cancel = false;
        let mut apply = false;
        let close = fluent::modal_dialog(ctx, "Palette and Color Conversion", |ui| {
            ui.label("Preview a conversion on the canvas, then Apply conversion.");
            egui::ScrollArea::vertical().id_salt("palette-conversion-controls")
                .max_height((ctx.content_rect().height() - 250.).max(100.)).show(ui, |ui| {
            ui.add_enabled_ui(!dialog.applying && dialog.worker.as_ref().is_none_or(|work| work.request.is_some()), |ui| {
                ui.label("Generate from visible artwork");
                ui.add(egui::DragValue::new(&mut dialog.limit).range(2..=256).prefix("Maximum colors "));
                ui.pc_checkbox(&mut dialog.all_frames, "Sample all frames");
                if ui.add_enabled(dialog.worker.is_none(), egui::Button::new("Generate palette")).clicked() {
                    action = Some(PaletteAction::Generate);
                }
                if let Some(colors) = &mut dialog.generated {
                    ui.label("Generated swatches");
                    egui::ScrollArea::vertical().id_salt("generated-swatches").max_height(128.).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (index, color) in colors.iter_mut().enumerate() {
                                ui.push_id(index, |ui| {
                                    let mut edited = *color;
                                    if ui.color_edit_button_srgba_unmultiplied(&mut edited)
                                        .on_hover_text(format!("Edit swatch {index}"))
                                        .changed()
                                    {
                                        *color = edited;
                                    }
                                });
                            }
                        });
                    });
                    if ui.pc_button("Use document palette").clicked() {
                        dialog.generated = None;
                    }
                }
                ui.separator();
                let used = if dialog.generated.is_some() { "Generated palette" } else { "Document palette" };
                ui.label(used);
                ui.add(egui::DragValue::new(&mut dialog.transparent).range(0..=255).prefix("Transparent slot "));
                ui.small("Slot numbers start at 0. The transparent entry moves to this slot.");
                let candidate = dialog.generated.as_ref().unwrap_or(&self.doc.palette);
                if candidate.len() == 256
                    && (dialog.generated.is_some() || self.doc.color_mode != ColorMode::Indexed)
                    && !candidate.iter().any(|color| color[3] == 0)
                {
                    ui.small("The palette is full. The selected slot will be replaced with transparency.");
                }
                let label = |pattern: GradientPattern| {
                    if pattern == GradientPattern::Smooth { "None" } else { pattern.label() }
                };
                egui::ComboBox::from_label("Conversion dithering").selected_text(if dialog.diffusion { "Floyd-Steinberg" } else { label(dialog.matrix) }).show_ui(ui, |ui| {
                    for pattern in [GradientPattern::Smooth, GradientPattern::Bayer2, GradientPattern::Bayer4, GradientPattern::Bayer8, GradientPattern::Installed] {
                        ui.add_enabled_ui(pattern != GradientPattern::Installed || self.doc.dithering.is_some(), |ui| {
                            if ui.selectable_label(!dialog.diffusion && dialog.matrix == pattern, label(pattern)).clicked() {
                                dialog.matrix = pattern;
                                dialog.diffusion = false;
                            }
                        });
                    }
                    ui.pc_selectable_value(&mut dialog.diffusion, true, "Floyd-Steinberg");
                });
                egui::ComboBox::from_label("Color-fit criterion").selected_text(dialog.fit.label()).show_ui(ui, |ui| {
                    for fit in IndexedFit::ALL {
                        ui.pc_selectable_value(&mut dialog.fit, fit, fit.label());
                    }
                });
                if dialog.diffusion {
                    ui.add(egui::DragValue::new(&mut dialog.strength).range(0..=100).prefix("Diffusion strength ").suffix("%"));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(dialog.generated.is_some() && dialog.worker.is_none(), egui::Button::new("Apply palette")).clicked() {
                        action = Some(PaletteAction::Apply);
                    }
                    if ui.pc_button("Convert to Indexed").clicked() {
                        action = Some(PaletteAction::Convert);
                    }
                });
                ui.separator();
                egui::ComboBox::from_label("Grayscale method")
                    .selected_text(match dialog.grayscale {
                        GrayscaleMethod::Luminance => "Luminance",
                        GrayscaleMethod::HsvValue => "HSV Value",
                        GrayscaleMethod::HslLightness => "HSL Lightness",
                    })
                    .show_ui(ui,|ui| {
                        for (value,label) in [
                            (GrayscaleMethod::Luminance,"Luminance"),
                            (GrayscaleMethod::HsvValue,"HSV Value"),
                            (GrayscaleMethod::HslLightness,"HSL Lightness"),
                        ] {
                            ui.pc_selectable_value(&mut dialog.grayscale,value,label);
                        }
                    });
                if ui.pc_button("Convert to Grayscale").clicked() {
                    action = Some(PaletteAction::Grayscale(dialog.grayscale));
                }
                if ui.pc_button("Convert to RGB").clicked() {
                    action = Some(PaletteAction::Rgba);
                }
            });
            });
            if dialog.worker.is_some() {
                ui.small("Processing artwork…");
            }
            if let Some(error) = &dialog.error {
                ui.small(error);
            }
            ui.pc_checkbox(&mut dialog.show_preview, "Show conversion preview");
            let ready = !dialog.applying
                && dialog.worker.is_none()
                && dialog.preview.as_ref().is_some_and(|preview| {
                    Some(&preview.request) == self.palette_request(&dialog).as_ref()
                });
            apply = ui
                .add_enabled(ready, egui::Button::new("Apply conversion"))
                .clicked();
            cancel = ui.pc_button("Cancel").clicked();
            apply || cancel
        });
        if apply {
            if let Some(preview) = &dialog.preview {
                let action = preview.request.action;
                dialog.applying = true;
                self.schedule_palette(&mut dialog, action, ctx);
                self.palette_conversion = Some(dialog);
            }
        } else if !close && !cancel {
            if let Some(action) = action {
                match action {
                    PaletteAction::Generate | PaletteAction::Apply => {
                        dialog.target = None;
                        dialog.preview = None;
                        dialog.requested = None;
                        self.schedule_palette(&mut dialog, action, ctx);
                    }
                    PaletteAction::Convert => dialog.target = Some(ColorMode::Indexed),
                    PaletteAction::Grayscale(_) => dialog.target = Some(ColorMode::Grayscale),
                    PaletteAction::Rgba => dialog.target = Some(ColorMode::Rgba),
                }
            }
            self.schedule_conversion_preview(&mut dialog, ctx);
            self.palette_conversion = Some(dialog);
        }
    }

    fn commit_palette_document(&mut self, doc: Box<Document>, label: &'static str) {
        if self.doc != *doc {
            self.remember_action(label);
            self.doc = *doc;
            self.palette_index = self
                .doc
                .palette_for(self.frame)
                .iter()
                .position(|&color| color == self.fg)
                .unwrap_or(0);
            self.secondary_palette_index = self
                .doc
                .palette_for(self.frame)
                .iter()
                .position(|&color| color == self.bg);
        }
        self.status = label.into();
    }

    fn palette_request(&self, dialog: &PaletteConversion) -> Option<PaletteRequest> {
        let action = match dialog.target? {
            ColorMode::Indexed => PaletteAction::Convert,
            ColorMode::Grayscale => PaletteAction::Grayscale(dialog.grayscale),
            ColorMode::Rgba => PaletteAction::Rgba,
        };
        let indexed = action == PaletteAction::Convert;
        Some(PaletteRequest {
            action,
            palette: if indexed {
                dialog.generated.clone()
            } else {
                None
            },
            transparent: if indexed { dialog.transparent } else { 0 },
            matrix: if indexed && !dialog.diffusion {
                dialog.matrix
            } else {
                GradientPattern::Smooth
            },
            diffusion: indexed && dialog.diffusion,
            strength: if indexed && dialog.diffusion {
                dialog.strength
            } else {
                100
            },
            fit: if indexed {
                dialog.fit
            } else {
                IndexedFit::Default
            },
            frame: self.frame,
            display: self.display_profile.bytes.clone(),
        })
    }

    pub(super) fn palette_texture(&self) -> Option<&egui::TextureHandle> {
        let dialog = self.palette_conversion.as_ref()?;
        if !dialog.show_preview || dialog.generation != self.power.bridge_generation {
            return None;
        }
        let preview = dialog.preview.as_ref()?;
        (Some(&preview.request) == self.palette_request(dialog).as_ref())
            .then_some(&preview.texture)
    }

    pub(super) fn clear_palette_preview_cache(&mut self) {
        if let Some(dialog) = &mut self.palette_conversion {
            dialog.preview = None;
            dialog.requested = None;
            if let Some(worker) = &dialog.worker
                && worker.request.is_some()
                && !dialog.applying
            {
                worker.cancelled.store(true, Ordering::Relaxed);
            }
        }
    }

    fn schedule_conversion_preview(&self, dialog: &mut PaletteConversion, ctx: &egui::Context) {
        if dialog.applying {
            return;
        }
        let Some(request) = self.palette_request(dialog) else {
            return;
        };
        if let Some(worker) = &dialog.worker {
            if worker
                .request
                .as_ref()
                .is_some_and(|previous| previous != &request)
            {
                worker.cancelled.store(true, Ordering::Relaxed);
                dialog.preview = None;
            }
            return;
        }
        if dialog.requested.as_ref() == Some(&request) {
            return;
        }
        dialog.preview = None;
        dialog.requested = Some(request.clone());
        self.schedule_palette(dialog, request.action, ctx);
    }

    fn schedule_palette(
        &self,
        dialog: &mut PaletteConversion,
        action: PaletteAction,
        ctx: &egui::Context,
    ) {
        let request = self.palette_request(dialog);
        let previewing = request.is_some() && !dialog.applying;
        let source = if previewing {
            match self.doc.conversion_frame_snapshot(self.frame) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    dialog.error = Some(error);
                    return;
                }
            }
        } else {
            self.doc.clone()
        };
        let tile_frame = if previewing { 0 } else { self.frame };
        let frames = if dialog.all_frames {
            0..=source.durations.len() - 1
        } else {
            self.frame..=self.frame
        };
        let palette = dialog.generated.clone();
        let matrix = dialog.matrix.matrix(&source);
        let diffusion = dialog.diffusion;
        let fit = dialog.fit;
        let strength = f64::from(dialog.strength) / 100.;
        let (limit, transparent) = (dialog.limit, dialog.transparent);
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        dialog.worker = Some(PaletteWork {
            request: request.clone(),
            receiver: rx,
            cancelled: Arc::clone(&cancelled),
        });
        dialog.error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let is_cancelled = || cancelled.load(Ordering::Relaxed);
            let result = match action {
                PaletteAction::Generate => {
                    generate_artwork_palette_until(&source, frames, limit, is_cancelled)
                        .map(PaletteResult::Generated)
                }
                PaletteAction::Apply if source.color_mode != ColorMode::Indexed => {
                    let mut target = source;
                    target.palette = palette.expect("Apply requires a generated palette");
                    target.palette_names.clear();
                    target.frame_palettes.clear();
                    target.drawing_index = None;
                    target.validate().map(|()| {
                        PaletteResult::Updated(Box::new(target), "Applied artwork palette")
                    })
                }
                PaletteAction::Rgba => source
                    .rgba_conversion_with_tile_frame_until(tile_frame, is_cancelled)
                    .map(|doc| PaletteResult::Updated(Box::new(doc), "Converted RGB colors")),
                PaletteAction::Grayscale(method) => source
                    .grayscale_conversion_with_tile_frame_until(method, tile_frame, is_cancelled)
                    .map(|doc| PaletteResult::Updated(Box::new(doc), "Converted grayscale colors")),
                PaletteAction::Convert | PaletteAction::Apply => source
                    .indexed_conversion_with_fit_until(
                        palette,
                        transparent,
                        if diffusion {
                            IndexedDithering::FloydSteinbergWithFactor(strength)
                        } else {
                            matrix
                                .as_ref()
                                .map_or(IndexedDithering::None, IndexedDithering::Ordered)
                        },
                        fit,
                        is_cancelled,
                    )
                    .map(|doc| PaletteResult::Updated(Box::new(doc), "Converted indexed colors")),
            };
            let result = result.and_then(|result| match (previewing, &request, result) {
                (true, Some(request), PaletteResult::Updated(document, _label)) => {
                    if is_cancelled() {
                        return Err("Color conversion preview cancelled".into());
                    }
                    let image = Self::compose_frame_image(&document, 0, &request.display).0;
                    if is_cancelled() {
                        return Err("Color conversion preview cancelled".into());
                    }
                    Ok(PaletteResult::Preview {
                        image,
                        #[cfg(test)]
                        document,
                    })
                }
                (_, _, result) => Ok(result),
            });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(
        app: &mut PixelCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                events,
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        output.textures_delta.clear();
        output
    }

    fn click_label(app: &mut PixelCraft, ctx: &egui::Context, label: &str) {
        let output = paint(app, ctx, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        let button = |pressed| egui::Event::PointerButton {
            pos: position,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        paint(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(position),
                button(true),
                button(false),
            ],
        );
    }

    fn wait_preview(app: &mut PixelCraft, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            paint(app, ctx, vec![]);
            let dialog = app
                .palette_conversion
                .as_ref()
                .expect("preview dialog must stay open");
            assert!(dialog.error.is_none(), "{:?}", dialog.error);
            if dialog.worker.is_none() && app.palette_texture().is_some() {
                return;
            }
            assert!(Instant::now() < deadline, "preview worker timed out");
            std::thread::yield_now();
        }
    }

    fn wait_applied(app: &mut PixelCraft, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.palette_conversion.is_some() {
            paint(app, ctx, vec![]);
            if let Some(dialog) = &app.palette_conversion {
                assert!(dialog.error.is_none(), "{:?}", dialog.error);
            }
            assert!(Instant::now() < deadline, "full conversion timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn conversion_previews_before_apply_and_keeps_one_undo() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.layers[0].cels[0].set(0, [200, 100, 20, 137]);
        app.replace(doc.clone(), None);
        let generation = app.power.bridge_generation;
        app.start_grayscale_conversion(GrayscaleMethod::Luminance, &ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app
            .palette_conversion
            .as_ref()
            .is_some_and(|d| d.worker.is_some())
            && Instant::now() < deadline
        {
            paint(&mut app, &ctx, vec![]);
            std::thread::yield_now();
        }
        assert!(app.doc == doc, "preview must not change the document");
        assert_eq!(app.power.bridge_generation, generation);
        assert!(app.undo.is_empty());
        assert!(app.palette_conversion.is_some());
        let texture = app.palette_texture().expect("canvas preview texture").id();
        let output = paint(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == texture)
        ));
        click_label(&mut app, &ctx, "Show conversion preview");
        assert!(app.palette_texture().is_none());
        assert!(app.doc == doc);
        click_label(&mut app, &ctx, "Show conversion preview");
        assert_eq!(app.palette_texture().unwrap().id(), texture);
        click_label(&mut app, &ctx, "Apply conversion");
        wait_applied(&mut app, &ctx);
        assert!(app.palette_conversion.is_none());
        assert_eq!(app.doc.layers[0].cels[0][0], [115, 115, 115, 137]);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
        app.redo();
        assert_eq!(app.doc.layers[0].cels[0][0], [115, 115, 115, 137]);
    }

    #[test]
    fn palette_preview_converts_only_its_frame_then_applies_all_frames_in_worker() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(2, 1);
        doc.layers[0].cels[0] = vec![[200, 100, 20, 137]; 2].into();
        doc.add_frame(0, true);
        doc.layers[0].cels[1] = vec![[20, 100, 200, 100]; 2].into();
        app.replace(doc.clone(), None);
        app.frame = 1;
        app.start_grayscale_conversion(GrayscaleMethod::Luminance, &ctx);
        wait_preview(&mut app, &ctx);
        let preview = app
            .palette_conversion
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(
            preview.document.durations.len(),
            1,
            "preview must not process unrelated frames"
        );
        assert_eq!(preview.document.layers[0].cels[0][0], [90, 90, 90, 100]);
        let texture = preview.texture.id();
        app.palette_conversion.as_mut().unwrap().fit = IndexedFit::Lab;
        app.palette_conversion.as_mut().unwrap().strength = 42;
        assert_eq!(
            app.palette_texture().unwrap().id(),
            texture,
            "indexed controls must not invalidate grayscale"
        );
        paint(&mut app, &ctx, vec![]);
        assert!(app.palette_conversion.as_ref().unwrap().worker.is_none());
        assert!(app.doc == doc);
        click_label(&mut app, &ctx, "Apply conversion");
        assert!(
            app.doc == doc,
            "full conversion commits after the worker completes"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.palette_conversion.is_some() && Instant::now() < deadline {
            paint(&mut app, &ctx, vec![]);
            std::thread::yield_now();
        }
        assert!(app.palette_conversion.is_none());
        let expected = doc
            .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap();
        assert!(app.doc == expected);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
        app.redo();
        assert!(app.doc == expected);
    }

    #[test]
    fn applying_tile_preview_retains_original_definition_ids_and_other_frames() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.add_frame(0, false);
        doc.tilesets.push(Tileset {
            editor_id: Default::default(),
            name: "original definitions".into(),
            width: 1,
            height: 1,
            tiles: vec![
                vec![[200, 100, 20, 137]].into(),
                vec![[20, 100, 200, 100]].into(),
            ],
        });
        doc.layers[0].tileset = Some(0);
        doc.layers[0].tile_cels = vec![vec![2].into(), vec![1].into()];
        doc.set_frame_name(0, "blue").unwrap();
        doc.set_frame_name(1, "orange").unwrap();
        app.replace(doc.clone(), None);
        app.start_grayscale_conversion(GrayscaleMethod::Luminance, &ctx);
        wait_preview(&mut app, &ctx);
        let preview = app
            .palette_conversion
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(preview.document.tilesets[0].tiles.len(), 1);
        assert_eq!(preview.document.layers[0].tile_cels[0], vec![1]);
        assert!(app.doc == doc);
        click_label(&mut app, &ctx, "Apply conversion");
        wait_applied(&mut app, &ctx);
        let expected = doc
            .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap();
        assert!(app.doc == expected);
        assert_eq!(app.doc.tilesets[0].tiles.len(), 2);
        assert_eq!(app.doc.layers[0].tile_cels, vec![vec![2], vec![1]]);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }

    #[test]
    fn pending_full_conversion_survives_view_changes_but_rejects_source_edits() {
        for edit_source in [false, true] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let mut doc = Document::new(1, 1);
            doc.layers[0].cels[0].set(0, [200, 100, 20, 137]);
            doc.add_frame(0, true);
            app.replace(doc.clone(), None);
            app.open_palette_conversion();
            let mut dialog = app.palette_conversion.take().unwrap();
            dialog.target = Some(ColorMode::Grayscale);
            dialog.applying = true;
            let request = app.palette_request(&dialog).unwrap();
            let (tx, rx) = mpsc::channel();
            let cancelled = Arc::new(AtomicBool::new(false));
            dialog.worker = Some(PaletteWork {
                request: Some(request),
                receiver: rx,
                cancelled: cancelled.clone(),
            });
            app.palette_conversion = Some(dialog);
            app.frame = 1;
            app.set_display_profile_bytes(vec![0; 16]);
            assert!(
                !cancelled.load(Ordering::Relaxed),
                "display changes must not cancel an Apply operation"
            );
            let full = doc
                .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
                .unwrap();
            if edit_source {
                app.remember_action("Intervening source edit");
                app.doc.layers[0].cels[0].set(0, [12, 34, 56, 255]);
            }
            let expected = app.doc.clone();
            tx.send(Ok(PaletteResult::Updated(
                Box::new(full.clone()),
                "Converted grayscale colors",
            )))
            .ok()
            .unwrap();
            paint(&mut app, &ctx, vec![]);
            assert!(app.palette_conversion.is_none());
            if edit_source {
                assert!(app.doc == expected);
            } else {
                assert!(app.doc == full);
                assert_eq!(app.undo.len(), 1);
            }
        }
    }

    #[test]
    fn conversion_cancel_and_escape_discard_in_flight_previews() {
        for escape in [false, true] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let doc = Document::new(1, 1);
            app.replace(doc.clone(), None);
            app.open_palette_conversion();
            let mut dialog = app.palette_conversion.take().unwrap();
            dialog.target = Some(ColorMode::Grayscale);
            let request = app.palette_request(&dialog).unwrap();
            let (tx, rx) = mpsc::channel();
            let cancelled = Arc::new(AtomicBool::new(false));
            dialog.worker = Some(PaletteWork {
                request: Some(request),
                receiver: rx,
                cancelled: cancelled.clone(),
            });
            app.palette_conversion = Some(dialog);
            for _ in 0..3 {
                paint(&mut app, &ctx, vec![]);
            }
            if escape {
                paint(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                );
            } else {
                click_label(&mut app, &ctx, "Cancel");
            }
            assert!(app.palette_conversion.is_none());
            assert!(app.palette_texture().is_none());
            assert!(cancelled.load(Ordering::Relaxed));
            assert!(tx.send(Err("late cancellation".into())).is_err());
            assert!(app.doc == doc);
            assert!(app.undo.is_empty());
        }
    }

    #[test]
    fn palette_conversion_confirmation_fits_a_small_window() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.layers[0].cels[0].set(0, [200, 100, 20, 137]);
        app.replace(doc, None);
        app.start_grayscale_conversion(GrayscaleMethod::Luminance, &ctx);
        let render = |app: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640., 480.))),
                    events,
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            output
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.palette_conversion.as_ref().unwrap().preview.is_none() {
            render(&mut app, vec![]);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        for _ in 0..3 {
            render(&mut app, vec![]);
        }
        let output = render(&mut app, vec![]);
        let mut apply = None;
        for label in ["Apply conversion", "Cancel"] {
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing visible {label}"));
            assert!(
                ctx.content_rect().shrink(1.).contains(position),
                "{label} must stay in the viewport: {position:?}"
            );
            if label == "Apply conversion" {
                apply = Some(position);
            }
        }
        let pos = apply.unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        render(
            &mut app,
            vec![egui::Event::PointerMoved(pos), button(true), button(false)],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.palette_conversion.is_some() && Instant::now() < deadline {
            render(&mut app, vec![]);
            std::thread::yield_now();
        }
        assert!(app.palette_conversion.is_none());
        assert_eq!(app.doc.color_mode, ColorMode::Grayscale);
    }

    #[test]
    fn palette_preview_refreshes_parameters_frame_and_display_without_edits() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
        doc.layers[0].cels[0].set(0, [128, 128, 128, 255]);
        doc.add_frame(0, true);
        doc.layers[0].cels[1].set(0, [255; 4]);
        app.replace(doc.clone(), None);
        let generation = app.power.bridge_generation;
        app.open_palette_conversion();
        click_label(&mut app, &ctx, "Convert to Indexed");
        wait_preview(&mut app, &ctx);
        assert_eq!(
            app.palette_conversion
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .document
                .layers[0]
                .cel_meta[0]
                .indices
                .as_ref()
                .unwrap()[0],
            2
        );
        app.palette_conversion.as_mut().unwrap().fit = IndexedFit::LinearRgb;
        assert!(app.palette_texture().is_none());
        wait_preview(&mut app, &ctx);
        assert_eq!(
            app.palette_conversion
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .document
                .layers[0]
                .cel_meta[0]
                .indices
                .as_ref()
                .unwrap()[0],
            1
        );
        app.frame = 1;
        assert!(app.palette_texture().is_none());
        wait_preview(&mut app, &ctx);
        assert_eq!(
            app.palette_conversion
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .request
                .frame,
            1
        );
        app.set_display_profile_bytes(vec![0; 16]);
        assert!(app.palette_texture().is_none());
        wait_preview(&mut app, &ctx);
        assert_eq!(
            app.palette_conversion
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .request
                .display
                .as_slice(),
            &[0; 16]
        );
        assert!(app.doc == doc);
        assert_eq!(app.power.bridge_generation, generation);
        assert!(app.undo.is_empty());
        click_label(&mut app, &ctx, "Cancel");
        assert!(app.palette_conversion.is_none());
        assert!(app.palette_texture().is_none());
        assert!(app.doc == doc);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn palette_preview_coalesces_changes_even_when_settings_return_to_the_original() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
        doc.layers[0].cels[0].set(0, [128, 128, 128, 255]);
        app.replace(doc.clone(), None);
        app.open_palette_conversion();
        let mut dialog = app.palette_conversion.take().unwrap();
        dialog.target = Some(ColorMode::Indexed);
        let request = app.palette_request(&dialog).unwrap();
        let (tx, rx) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        dialog.worker = Some(PaletteWork {
            request: Some(request.clone()),
            receiver: rx,
            cancelled: cancelled.clone(),
        });
        dialog.requested = Some(request);
        app.palette_conversion = Some(dialog);
        for fit in [IndexedFit::LinearRgb, IndexedFit::Lab, IndexedFit::Default] {
            app.palette_conversion.as_mut().unwrap().fit = fit;
            paint(&mut app, &ctx, vec![]);
            let worker = app
                .palette_conversion
                .as_ref()
                .unwrap()
                .worker
                .as_ref()
                .unwrap();
            assert!(
                Arc::ptr_eq(&worker.cancelled, &cancelled),
                "must wait for the old worker before starting another"
            );
            assert!(cancelled.load(Ordering::Relaxed));
            assert!(app.palette_texture().is_none());
        }
        let stale = doc
            .grayscale_conversion_until(GrayscaleMethod::Luminance, || false)
            .unwrap();
        let image = PixelCraft::compose_frame_image(&stale, 0, &[]).0;
        tx.send(Ok(PaletteResult::Preview {
            document: Box::new(stale),
            image,
        }))
        .ok()
        .unwrap();
        wait_preview(&mut app, &ctx);
        let preview = app
            .palette_conversion
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap();
        assert_eq!(preview.document.color_mode, ColorMode::Indexed);
        assert_eq!(preview.request.fit, IndexedFit::Default);
        assert!(app.doc == doc);
        assert!(app.undo.is_empty());
        app.remember_action("Intervening edit");
        app.doc.layers[0].cels[0].set(0, [12, 34, 56, 255]);
        let edited = app.doc.clone();
        assert!(app.palette_texture().is_none());
        paint(&mut app, &ctx, vec![]);
        assert!(app.palette_conversion.is_none());
        assert!(app.doc == edited);
    }

    #[test]
    fn properties_color_conversions_use_the_worker_and_one_undo() {
        for (indexed, grayscale) in [(false, true), (true, false), (true, true)] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let mut doc = Document::new(512, 512);
            doc.layers[0].cels[0].set(0, [200, 100, 20, 137]);
            if indexed {
                doc.palette = vec![CLEAR, [200, 100, 20, 137]];
                doc.set_color_mode(ColorMode::Indexed);
                if grayscale {
                    doc.layers[0].cels[0].set(0, [7, 8, 9, 255]);
                }
            }
            app.replace(doc.clone(), None);
            app.properties_open = true;
            let render = |app: &mut PixelCraft, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                        events,
                        ..Default::default()
                    },
                    |ui| app.render(ui),
                );
                output.textures_delta.clear();
                output
            };
            let click = |app: &mut PixelCraft, label: &str| {
                let output = render(app, vec![]);
                let position = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == label => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing {label}"));
                let button = |pressed| egui::Event::PointerButton {
                    pos: position,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                };
                render(
                    app,
                    vec![
                        egui::Event::PointerMoved(position),
                        button(true),
                        button(false),
                    ],
                );
            };
            for _ in 0..3 {
                render(&mut app, vec![]);
            }
            click(&mut app, if indexed { "Indexed" } else { "Rgba" });
            click(&mut app, if grayscale { "Grayscale" } else { "Rgba" });
            assert!(
                !app.properties_open,
                "conversion should move to the worker dialog"
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while app
                .palette_conversion
                .as_ref()
                .is_some_and(|dialog| dialog.preview.is_none())
                && Instant::now() < deadline
            {
                render(&mut app, vec![]);
                std::thread::yield_now();
            }
            assert!(app.palette_texture().is_some());
            click_label(&mut app, &ctx, "Apply conversion");
            wait_applied(&mut app, &ctx);
            assert!(app.palette_conversion.is_none());
            assert_eq!(
                app.doc.color_mode,
                if grayscale {
                    ColorMode::Grayscale
                } else {
                    ColorMode::Rgba
                }
            );
            assert_eq!(
                app.doc.layers[0].cels[0][0],
                if grayscale {
                    [115, 115, 115, 137]
                } else {
                    [200, 100, 20, 137]
                }
            );
            assert_eq!(app.undo.len(), 1);
            app.undo();
            assert!(app.doc == doc);
        }
    }

    #[test]
    fn palette_dialog_chooses_color_fit_and_applies_one_undo() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
        doc.layers[0].cels[0].set(0, [128, 128, 128, 255]);
        app.replace(doc.clone(), None);
        app.action("palette-conversion");
        let render = |app: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                    events,
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            output
        };
        let click = |app: &mut PixelCraft, label: &str| {
            let output = render(app, vec![]);
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"));
            let button = |pressed| egui::Event::PointerButton {
                pos: position,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            render(
                app,
                vec![
                    egui::Event::PointerMoved(position),
                    button(true),
                    button(false),
                ],
            );
        };
        for _ in 0..3 {
            render(&mut app, vec![]);
        }
        click(&mut app, "Default");
        click(&mut app, "Linear RGB");
        click(&mut app, "Convert to Indexed");
        assert!(app.doc == doc, "conversion applies after worker completion");
        let deadline = Instant::now() + Duration::from_secs(5);
        while app
            .palette_conversion
            .as_ref()
            .is_some_and(|dialog| dialog.preview.is_none())
            && Instant::now() < deadline
        {
            render(&mut app, vec![]);
            std::thread::yield_now();
        }
        assert!(app.palette_texture().is_some());
        click_label(&mut app, &ctx, "Apply conversion");
        wait_applied(&mut app, &ctx);
        assert!(app.palette_conversion.is_none());
        assert_eq!(
            app.doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
            1
        );
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
        app.redo();
        assert_eq!(
            app.doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[0],
            1
        );
    }

    #[test]
    fn palette_dialog_grayscale_button_converts_all_methods_with_undo() {
        for method in [
            GrayscaleMethod::Luminance,
            GrayscaleMethod::HsvValue,
            GrayscaleMethod::HslLightness,
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let mut doc = Document::new(1, 1);
            doc.layers[0].cels[0].set(0, [200, 100, 20, 137]);
            app.replace(doc.clone(), None);
            app.action("palette-conversion");
            app.palette_conversion.as_mut().unwrap().grayscale = method;
            let render = |app: &mut PixelCraft, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                        events,
                        ..Default::default()
                    },
                    |ui| app.render(ui),
                );
                output.textures_delta.clear();
                output
            };
            for _ in 0..3 {
                render(&mut app, vec![]);
            }
            let output = render(&mut app, vec![]);
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Convert to Grayscale" => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            let button = |pressed| egui::Event::PointerButton {
                pos: position,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            render(
                &mut app,
                vec![
                    egui::Event::PointerMoved(position),
                    button(true),
                    button(false),
                ],
            );
            assert!(app.doc == doc);
            let deadline = Instant::now() + Duration::from_secs(3);
            while app
                .palette_conversion
                .as_ref()
                .is_some_and(|dialog| dialog.preview.is_none())
                && Instant::now() < deadline
            {
                render(&mut app, vec![]);
                std::thread::yield_now();
            }
            assert!(app.palette_texture().is_some());
            click_label(&mut app, &ctx, "Apply conversion");
            wait_applied(&mut app, &ctx);
            assert!(app.palette_conversion.is_none());
            let expected = doc.grayscale_conversion_until(method, || false).unwrap();
            assert!(app.doc.layers[0].cels[0] == expected.layers[0].cels[0]);
            assert_eq!(app.doc.color_mode, ColorMode::Grayscale);
            assert_eq!(app.undo.len(), 1);
            app.undo();
            assert!(app.doc == doc);
            app.redo();
            assert!(app.doc.layers[0].cels[0] == expected.layers[0].cels[0]);
        }
    }

    #[test]
    fn palette_dialog_discards_diffusion_after_cancel_or_source_edits() {
        for action in [
            PaletteAction::Convert,
            PaletteAction::Rgba,
            PaletteAction::Grayscale(GrayscaleMethod::HsvValue),
        ] {
            for cancel in [false, true] {
                let ctx = egui::Context::default();
                let mut app = PixelCraft::with_context(&ctx, None);
                let mut doc = Document::new(128, 128);
                doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
                doc.layers[0].cels[0] = vec![[128, 128, 128, 255]; 128 * 128].into();
                if matches!(action, PaletteAction::Rgba) {
                    doc.set_color_mode(ColorMode::Indexed);
                }
                app.replace(doc.clone(), None);
                app.action("palette-conversion");
                let mut dialog = app.palette_conversion.take().unwrap();
                dialog.diffusion = true;
                app.schedule_palette(&mut dialog, action, &ctx);
                app.palette_conversion = Some(dialog);
                if cancel {
                    app.palette_conversion = None;
                } else {
                    app.remember_action("Draw pixel");
                    app.doc.set_pixel(0, 0, 0, [240, 40, 30, 255]);
                }
                let expected = app.doc.clone();
                let mut rendered = ctx.run_ui(egui::RawInput::default(), |ui| app.render(ui));
                rendered.textures_delta.clear();
                assert!(app.palette_conversion.is_none());
                assert!(app.doc == expected);
                assert_eq!(app.doc.color_mode, doc.color_mode);
            }
        }
    }

    #[test]
    fn palette_dialog_diffusion_converts_in_worker_and_supports_undo_redo() {
        for strength in [0, 100] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let mut doc = Document::new(2, 2);
            doc.palette = vec![CLEAR, [0, 0, 0, 255], [255; 4]];
            doc.layers[0].cels[0] = vec![[128, 128, 128, 255]; 4].into();
            app.replace(doc.clone(), None);
            app.action("palette-conversion");
            let render = |app: &mut PixelCraft, events| {
                let mut rendered = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                        events,
                        ..Default::default()
                    },
                    |ui| app.render(ui),
                );
                rendered.textures_delta.clear();
                rendered
            };
            let click = |app: &mut PixelCraft, label: &str| {
                let rendered = render(app, vec![]);
                let pos = rendered
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == label => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing dialog choice {label}"));
                render(
                    app,
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
                    ],
                );
            };
            for _ in 0..3 {
                render(&mut app, vec![]);
            }
            click(&mut app, "None");
            click(&mut app, "Floyd-Steinberg");
            app.palette_conversion.as_mut().unwrap().strength = strength;
            click(&mut app, "Convert to Indexed");
            assert!(app.doc == doc);
            assert!(app.undo.is_empty());
            let deadline = Instant::now() + Duration::from_secs(3);
            while app
                .palette_conversion
                .as_ref()
                .is_some_and(|dialog| dialog.preview.is_none())
                && Instant::now() < deadline
            {
                let mut rendered = ctx.run_ui(egui::RawInput::default(), |ui| app.render(ui));
                rendered.textures_delta.clear();
                std::thread::yield_now();
            }
            assert!(app.palette_texture().is_some());
            click_label(&mut app, &ctx, "Apply conversion");
            wait_applied(&mut app, &ctx);
            assert!(app.palette_conversion.is_none());
            assert_eq!(
                app.doc.layers[0].cel_meta[0]
                    .indices
                    .as_deref()
                    .unwrap()
                    .as_slice(),
                if strength == 0 { [2; 4] } else { [2, 1, 1, 2] }
            );
            let converted = app.doc.clone();
            assert_eq!(app.undo.len(), 1);
            app.undo();
            assert!(app.doc == doc);
            app.redo();
            assert!(app.doc == converted);
        }
    }

    #[test]
    fn palette_dialog_generates_edits_and_applies_conversion_as_one_undo() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(2, 1);
        doc.layers[0].cels[0].set(0, [100, 40, 20, 255]);
        app.replace(doc, None);
        let source = app.doc.clone();
        app.action("palette-conversion");
        let render = |app: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180., 840.))),
                    events,
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            output.textures_delta.clear();
            output
        };
        let click = |app: &mut PixelCraft, label: &str| {
            let output = render(app, Vec::new());
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            let button = |pressed| egui::Event::PointerButton {
                pos,
                pressed,
                modifiers: Default::default(),
                button: egui::PointerButton::Primary,
            };
            render(
                app,
                vec![egui::Event::PointerMoved(pos), button(true), button(false)],
            );
        };
        for _ in 0..3 {
            render(&mut app, vec![]);
        }
        click(&mut app, "Generate palette");
        let wait = |app: &mut PixelCraft| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while app
                .palette_conversion
                .as_ref()
                .is_some_and(|dialog| dialog.worker.is_some())
            {
                assert!(Instant::now() < deadline, "Palette worker timed out");
                std::thread::sleep(Duration::from_millis(1));
                render(app, vec![]);
            }
        };
        wait(&mut app);
        assert_eq!(app.doc, source);
        let dialog = app.palette_conversion.as_mut().unwrap();
        assert_eq!(
            dialog.generated.as_deref(),
            Some(&[CLEAR, [100, 40, 20, 255]][..])
        );
        let edited = [110, 40, 30, 255];
        dialog.generated.as_mut().unwrap()[1] = edited;
        dialog.transparent = 1;
        for _ in 0..3 {
            render(&mut app, vec![]);
        }
        click(&mut app, "Convert to Indexed");
        wait(&mut app);
        assert!(app.palette_texture().is_some());
        click_label(&mut app, &ctx, "Apply conversion");
        wait_applied(&mut app, &ctx);
        assert!(app.palette_conversion.is_none());
        assert_eq!(app.doc.color_mode, ColorMode::Indexed);
        assert_eq!(app.doc.transparent_index, 1);
        assert_eq!(app.doc.layers[0].cels[0].to_vec(), [edited, CLEAR]);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(app.doc, source);
    }
    #[test]
    fn indexed_tile_preview_matches_apply_on_a_later_frame() {
        for gray in [false, true] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            let red = [200, 100, 20, 255];
            let mut doc = Document::new(1, 1);
            doc.palette = vec![CLEAR, red];
            doc.set_color_mode(ColorMode::Indexed);
            let mut tile: TileRaster = vec![red].into();
            tile.indices = Some(Arc::new(vec![1]));
            doc.tilesets.push(Tileset {
                editor_id: Default::default(),
                name: "frame colored".into(),
                width: 1,
                height: 1,
                tiles: vec![tile],
            });
            doc.add_tile_layer(0).unwrap();
            doc.paint_tile(1, 0, 0, 0, 1).unwrap();
            doc.add_frame(0, true);
            doc.frame_palettes = vec![doc.palette.clone(), vec![CLEAR, [10, 20, 240, 255]]];
            app.replace(doc.clone(), None);
            app.frame = 1;
            if gray {
                app.start_grayscale_conversion(GrayscaleMethod::HsvValue, &ctx);
            } else {
                app.start_rgba_conversion(&ctx);
            }
            wait_preview(&mut app, &ctx);
            let expected = app
                .palette_conversion
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .document
                .composite(0);
            assert_eq!(app.doc, doc);
            click_label(&mut app, &ctx, "Apply conversion");
            wait_applied(&mut app, &ctx);
            assert_eq!(
                app.doc.composite(1),
                expected,
                "applied shared tile must match preview"
            );
            app.undo();
            assert_eq!(app.doc, doc);
        }
    }
}
