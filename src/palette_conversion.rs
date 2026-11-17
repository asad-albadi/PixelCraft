use super::*;
use crate::productivity_ui::GradientPattern;
use pixelcraft::workflows::generate_artwork_palette_until;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Copy)]
enum PaletteAction {
    Generate,
    Apply,
    Convert,
}

enum PaletteResult {
    Generated(Vec<Pixel>),
    Updated(Box<Document>, &'static str),
}

struct PaletteWork {
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
            worker: None,
            error: None,
        });
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
                    dialog.worker = None;
                    match result {
                        Ok(PaletteResult::Generated(colors)) => {
                            dialog.transparent = 0;
                            dialog.generated = Some(colors);
                        }
                        Ok(PaletteResult::Updated(doc, label)) => {
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
                            return;
                        }
                        Err(error) => dialog.error = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    dialog.worker = None;
                    dialog.error = Some("Palette worker stopped".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut action = None;
        let mut cancel = false;
        let close = fluent::modal_dialog(ctx, "Palette and Indexed Color", |ui| {
            ui.add_enabled_ui(dialog.worker.is_none(), |ui| {
                ui.label("Generate from visible artwork");
                ui.add(egui::DragValue::new(&mut dialog.limit).range(2..=256).prefix("Maximum colors "));
                ui.pc_checkbox(&mut dialog.all_frames, "Sample all frames");
                if ui.pc_button("Generate palette").clicked() {
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
                egui::ComboBox::from_label("Conversion dithering").selected_text(label(dialog.matrix)).show_ui(ui, |ui| {
                    for pattern in [GradientPattern::Smooth, GradientPattern::Bayer2, GradientPattern::Bayer4, GradientPattern::Bayer8, GradientPattern::Installed] {
                        ui.add_enabled_ui(pattern != GradientPattern::Installed || self.doc.dithering.is_some(), |ui| {
                            ui.pc_selectable_value(&mut dialog.matrix, pattern, label(pattern));
                        });
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(dialog.generated.is_some(), egui::Button::new("Apply palette")).clicked() {
                        action = Some(PaletteAction::Apply);
                    }
                    if ui.pc_button("Convert to Indexed").clicked() {
                        action = Some(PaletteAction::Convert);
                    }
                });
            });
            if dialog.worker.is_some() {
                ui.small("Processing artwork…");
            }
            if let Some(error) = &dialog.error {
                ui.small(error);
            }
            cancel = ui.pc_button("Cancel").clicked();
            cancel
        });
        if !close && !cancel {
            if let Some(action) = action {
                self.schedule_palette(&mut dialog, action, ctx);
            }
            self.palette_conversion = Some(dialog);
        }
    }

    fn schedule_palette(
        &self,
        dialog: &mut PaletteConversion,
        action: PaletteAction,
        ctx: &egui::Context,
    ) {
        let source = self.doc.clone();
        let frames = if dialog.all_frames {
            0..=source.durations.len() - 1
        } else {
            self.frame..=self.frame
        };
        let palette = dialog.generated.clone();
        let matrix = dialog.matrix.matrix(&source);
        let (limit, transparent) = (dialog.limit, dialog.transparent);
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        dialog.worker = Some(PaletteWork {
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
                PaletteAction::Convert | PaletteAction::Apply => source
                    .indexed_conversion_until(palette, transparent, matrix.as_ref(), is_cancelled)
                    .map(|doc| PaletteResult::Updated(Box::new(doc), "Converted indexed colors")),
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(app.palette_conversion.is_none());
        assert_eq!(app.doc.color_mode, ColorMode::Indexed);
        assert_eq!(app.doc.transparent_index, 1);
        assert_eq!(app.doc.layers[0].cels[0].to_vec(), [edited, CLEAR]);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(app.doc, source);
    }
}
