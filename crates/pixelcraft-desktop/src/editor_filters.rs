use super::*;
use crate::api::workflows::filters::{Filter, apply_filter_until};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    CurrentCel,
    SelectedCels,
    ActiveLayer,
    CurrentFrame,
    AllCels,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::CurrentCel => "Current cel",
            Self::SelectedCels => "Selected timeline cels",
            Self::ActiveLayer => "All frames in active layer",
            Self::CurrentFrame => "All unlocked layers in current frame",
            Self::AllCels => "All unlocked layers and frames",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Settings {
    filter: Filter,
    scope: Scope,
    selection_only: bool,
    channels: [bool; 4],
}

type PreviewResult = Result<(Document, usize, Option<egui::ColorImage>), String>;

struct FilterWork {
    settings: Settings,
    receiver: mpsc::Receiver<PreviewResult>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for FilterWork {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct FilterPreview {
    document: Document,
    changed: usize,
    texture: Option<egui::TextureHandle>,
}

pub(super) struct FilterDialog {
    generation: u64,
    settings: Settings,
    show_preview: bool,
    worker: Option<FilterWork>,
    result: Option<Result<FilterPreview, String>>,
}

fn filter_name(filter: &Filter) -> &'static str {
    match filter {
        Filter::BrightnessContrast { .. } => "Brightness and Contrast",
        Filter::Curves(_) => "Curves",
        Filter::Convolution { .. } => "Convolution",
        Filter::Despeckle { .. } => "Despeckle",
    }
}

impl PixelCraft {
    pub(super) fn open_filters(&mut self) {
        self.stop_playback(false);
        self.editor_filter = Some(FilterDialog {
            generation: self.power.bridge_generation,
            settings: Settings {
                filter: Filter::BrightnessContrast {
                    brightness: 0,
                    contrast: 0,
                },
                scope: Scope::CurrentCel,
                selection_only: self.session.selection.is_some(),
                channels: [true, true, true, false],
            },
            show_preview: true,
            worker: None,
            result: None,
        });
    }

    pub(super) fn filter_texture(&self) -> Option<&egui::TextureHandle> {
        let dialog = self.editor_filter.as_ref()?;
        if !dialog.show_preview || dialog.generation != self.power.bridge_generation {
            return None;
        }
        dialog.result.as_ref()?.as_ref().ok()?.texture.as_ref()
    }

    pub(super) fn clear_filter_preview_cache(&mut self) {
        if let Some(dialog) = &mut self.editor_filter {
            dialog.worker = None;
            dialog.result = None;
        }
    }

    fn filter_targets(&self, scope: Scope) -> Vec<(usize, usize)> {
        let frames = self.session.doc.durations.len();
        let layers = self.session.doc.layers.len();
        let candidates: Box<dyn Iterator<Item = (usize, usize)>> = match scope {
            Scope::CurrentCel => {
                Box::new(std::iter::once((self.session.layer, self.session.frame)))
            }
            Scope::SelectedCels => self.session.selected_cels.as_ref().map_or_else(
                || Box::new(std::iter::empty()) as Box<dyn Iterator<Item = _>>,
                CelSelection::cells,
            ),
            Scope::ActiveLayer => Box::new((0..frames).map(|frame| (self.session.layer, frame))),
            Scope::CurrentFrame => Box::new((0..layers).map(|layer| (layer, self.session.frame))),
            Scope::AllCels => {
                Box::new((0..layers).flat_map(|layer| (0..frames).map(move |frame| (layer, frame))))
            }
        };
        candidates
            .filter(|&(layer, frame)| {
                frame < frames
                    && self.session.doc.editable(layer)
                    && self.session.doc.layers[layer].tileset.is_none()
                    && self.session.doc.layers[layer].adjustment.is_none()
            })
            .collect()
    }

    fn schedule_filter(&self, dialog: &mut FilterDialog, ctx: &egui::Context) {
        if let Some(worker) = &dialog.worker {
            if worker.settings != dialog.settings {
                worker.cancelled.store(true, Ordering::Relaxed);
            }
            let result = match worker.receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Disconnected) => Err("Filter worker stopped".into()),
                Err(mpsc::TryRecvError::Empty) => return,
            };
            if worker.settings == dialog.settings && !worker.cancelled.load(Ordering::Relaxed) {
                dialog.result = Some(result.map(|(document, changed, image)| FilterPreview {
                    document,
                    changed,
                    texture: image.map(|image| {
                        ctx.load_texture("filter-preview", image, egui::TextureOptions::NEAREST)
                    }),
                }));
                ctx.request_repaint();
            }
            dialog.worker = None;
        }
        if dialog.result.is_some() {
            return;
        }
        let source = self.session.doc.clone();
        let targets = self.filter_targets(dialog.settings.scope);
        let (width, height) = (source.width as i32, source.height as i32);
        let (bounds, mask) = if dialog.settings.selection_only {
            let [x, y, w, h] = self.selection_bounds();
            (
                [
                    x.clamp(0, width),
                    y.clamp(0, height),
                    x.saturating_add(w).clamp(0, width),
                    y.saturating_add(h).clamp(0, height),
                ],
                self.session.mask.clone(),
            )
        } else {
            ([0, 0, width, height], None)
        };
        let settings = dialog.settings.clone();
        let submitted = settings.clone();
        let frame = self.session.frame;
        let display = Arc::clone(&self.display_profile.bytes);
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        dialog.worker = Some(FilterWork {
            settings: submitted,
            receiver: rx,
            cancelled: Arc::clone(&cancelled),
        });
        std::thread::spawn(move || {
            let result = apply_filter_until(
                &source,
                &targets,
                bounds.map(|edge| edge as usize),
                mask.as_deref(),
                &settings.filter,
                settings.channels,
                || cancelled.load(Ordering::Relaxed),
            )
            .map(|(document, changed)| {
                let image = (changed != 0 && !cancelled.load(Ordering::Relaxed))
                    .then(|| Self::compose_frame_image(&document, frame, &display).0);
                (document, changed, image)
            });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    pub(super) fn filters_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.editor_filter.take() else {
            return;
        };
        if dialog.generation != self.power.bridge_generation {
            self.status = "Filter preview closed because the sprite changed".into();
            return;
        }
        let mut apply = false;
        let mut cancel = false;
        let before = dialog.settings.clone();
        let close = fluent::modal_dialog(ctx, "Filters", |ui| {
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 138.).max(110.))
                .show(ui, |ui| {
                    filter_controls(ui, &mut dialog.settings.filter);
                    ui.separator();
                    egui::ComboBox::from_id_salt("filter-scope")
                        .selected_text(dialog.settings.scope.label())
                        .show_ui(ui, |ui| {
                            for scope in [
                                Scope::CurrentCel,
                                Scope::SelectedCels,
                                Scope::ActiveLayer,
                                Scope::CurrentFrame,
                                Scope::AllCels,
                            ] {
                                ui.add_enabled_ui(
                                    scope != Scope::SelectedCels
                                        || self.session.selected_cels.is_some(),
                                    |ui| {
                                        ui.pc_selectable_value(
                                            &mut dialog.settings.scope,
                                            scope,
                                            scope.label(),
                                        );
                                    },
                                );
                            }
                        });
                    ui.add_enabled_ui(self.session.selection.is_some(), |ui| {
                        ui.pc_checkbox(&mut dialog.settings.selection_only, "Limit to selection")
                    });
                    ui.horizontal(|ui| {
                        for (enabled, name) in dialog
                            .settings
                            .channels
                            .iter_mut()
                            .zip(["Red", "Green", "Blue", "Alpha"])
                        {
                            ui.pc_checkbox(enabled, name);
                        }
                    });
                    ui.pc_checkbox(&mut dialog.show_preview, "Preview on canvas");
                });
            if before != dialog.settings {
                dialog.result = None;
            }
            let ready = match &dialog.result {
                Some(Ok(result)) if result.changed != 0 => true,
                Some(Ok(_)) => {
                    ui.small("No pixels changed");
                    false
                }
                Some(Err(error)) => {
                    ui.small(error);
                    false
                }
                None => {
                    ui.small("Calculating preview…");
                    false
                }
            };
            ui.horizontal(|ui| {
                let apply_button = ui.add_enabled(ready, egui::Button::new("Apply filter"));
                let cancel_button = ui.pc_button("Cancel");
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("filter-apply"), apply_button.rect);
                    data.insert_temp(egui::Id::new("filter-cancel"), cancel_button.rect);
                });
                apply = apply_button.clicked();
                cancel = cancel_button.clicked();
                apply || cancel
            })
            .inner
        });
        if apply {
            if let Some(Ok(result)) = dialog.result.take() {
                let label = filter_name(&dialog.settings.filter);
                self.remember_action(label);
                self.session.doc = result.document;
                self.status = format!("Applied {label}");
            }
        } else if !close && !cancel {
            self.schedule_filter(&mut dialog, ctx);
            self.editor_filter = Some(dialog);
        }
    }
}

fn filter_controls(ui: &mut egui::Ui, filter: &mut Filter) {
    egui::ComboBox::from_id_salt("filter-kind")
        .selected_text(filter_name(filter))
        .show_ui(ui, |ui| {
            for prototype in [
                Filter::BrightnessContrast {
                    brightness: 0,
                    contrast: 0,
                },
                Filter::Curves(vec![(0, 0), (255, 255)]),
                Filter::Convolution {
                    weights: [0, -1, 0, -1, 5, -1, 0, -1, 0],
                    divisor: 1,
                    bias: 0,
                },
                Filter::Despeckle { threshold: 16 },
            ] {
                let name = filter_name(&prototype);
                if ui
                    .selectable_label(name == filter_name(filter), name)
                    .clicked()
                    && name != filter_name(filter)
                {
                    *filter = prototype;
                }
            }
        });
    match filter {
        Filter::BrightnessContrast {
            brightness,
            contrast,
        } => {
            ui.add(egui::Slider::new(brightness, -100..=100).text("Brightness"));
            ui.add(egui::Slider::new(contrast, -100..=100).text("Contrast"));
        }
        Filter::Curves(points) => curve_controls(ui, points),
        Filter::Convolution {
            weights,
            divisor,
            bias,
        } => {
            ui.horizontal(|ui| {
                for (label, matrix, scale) in [
                    ("Blur", [1; 9], 9),
                    ("Sharpen", [0, -1, 0, -1, 5, -1, 0, -1, 0], 1),
                    ("Edges", [-1, -1, -1, -1, 8, -1, -1, -1, -1], 1),
                ] {
                    if ui.pc_button(label).clicked() {
                        *weights = matrix;
                        *divisor = scale;
                        *bias = 0;
                    }
                }
            });
            for row in weights.chunks_mut(3) {
                ui.horizontal(|ui| {
                    for weight in row {
                        ui.add(egui::DragValue::new(weight).range(-256..=256));
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(divisor)
                        .range(-1024..=1024)
                        .prefix("Divisor "),
                );
                ui.add(egui::DragValue::new(bias).range(-255..=255).prefix("Bias "));
            });
        }
        Filter::Despeckle { threshold } => {
            ui.add(egui::Slider::new(threshold, 0..=255).text("Threshold"));
            ui.small("Replace outliers with the median of their 3×3 neighborhood.");
        }
    }
}

fn curve_controls(ui: &mut egui::Ui, points: &mut Vec<(u8, u8)>) {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(255.), 150.),
        Sense::click(),
    );
    let point = |(x, y): (u8, u8)| {
        Pos2::new(
            rect.left() + f32::from(x) * rect.width() / 255.,
            rect.bottom() - f32::from(y) * rect.height() / 255.,
        )
    };
    if let Some(pointer) = response.interact_pointer_pos() {
        let nearest = points
            .iter()
            .position(|&item| point(item).distance(pointer) <= 7.);
        if response.clicked() {
            let raw_x = ((pointer.x - rect.left()) / rect.width() * 255.)
                .round()
                .clamp(0., 255.) as u8;
            let y = ((rect.bottom() - pointer.y) / rect.height() * 255.)
                .round()
                .clamp(0., 255.) as u8;
            let x = nearest.map_or(raw_x, |index| points[index].0);
            match points.binary_search_by_key(&x, |entry| entry.0) {
                Ok(index) => points[index].1 = y,
                Err(index) => points.insert(index, (x, y)),
            }
        } else if response.secondary_clicked()
            && let Some(index) = nearest
            && index > 0
            && index + 1 < points.len()
        {
            points.remove(index);
        }
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 0., ui.visuals().extreme_bg_color);
    painter.rect_stroke(
        rect,
        0.,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let stroke = Stroke::new(1.5, ui.visuals().text_color());
    painter.add(egui::Shape::line(
        points.iter().copied().map(point).collect(),
        stroke,
    ));
    for &entry in points.iter() {
        painter.circle_filled(point(entry), 3., stroke.color);
    }
    ui.small("Click to set a point. Right-click a point to remove it.");
    if ui.pc_small_button("Reset curve").clicked() {
        *points = vec![(0, 0), (255, 255)];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_dialog_previews_selected_cels_and_commits_only_on_apply() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(3, 1);
        doc.layers[0].cels[0].set(0, [90, 50, 80, 255]);
        doc.add_frame(0, true);
        app.replace(doc, None);
        let before = app.doc.clone();
        app.selected_cels = Some(CelSelection::Individual(
            [(0, 0), (0, 1)]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into(),
        ));
        let render = |app: &mut PixelCraft, events| {
            let mut result = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.))),
                    events,
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            result.textures_delta.clear();
            result
        };
        let click = |app: &mut PixelCraft, pos| {
            render(app, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                render(
                    app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        };
        for apply in [false, true] {
            app.action("filters");
            let settings = &mut app.editor_filter.as_mut().unwrap().settings;
            settings.scope = Scope::SelectedCels;
            settings.filter = Filter::BrightnessContrast {
                brightness: 20,
                contrast: 0,
            };
            render(&mut app, vec![]);
            let deadline = Instant::now() + Duration::from_secs(2);
            while app.filter_texture().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
                render(&mut app, vec![]);
            }
            assert!(app.filter_texture().is_some());
            assert_eq!(app.doc, before);
            assert!(app.undo.is_empty());
            for _ in 0..3 {
                render(&mut app, Vec::new());
            }
            let id = egui::Id::new(if apply {
                "filter-apply"
            } else {
                "filter-cancel"
            });
            let rect = ctx.data(|data| data.get_temp::<Rect>(id)).unwrap();
            click(&mut app, rect.center());
            assert!(
                app.editor_filter.is_none(),
                "the filter dialog stays open after clicking apply={apply}"
            );
            if !apply {
                assert_eq!(app.doc, before);
                assert!(app.undo.is_empty());
            }
        }
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.doc.layers[0].cels[0][0], [141, 101, 131, 255]);
        assert_eq!(app.doc.layers[0].cels[1][0], [141, 101, 131, 255]);
        app.undo();
        assert_eq!(app.doc, before);
    }
}
