use super::*;
use crate::api::color::{
    assign_document_profile, convert_document_profile_until, read_icc_profile,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

struct ProfileWork {
    result: mpsc::Receiver<Result<Document, String>>,
    cancelled: Arc<AtomicBool>,
    label: &'static str,
}

impl Drop for ProfileWork {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub(super) struct ProfileDialog {
    generation: u64,
    selected: Option<PathBuf>,
    pending: Option<ProfileWork>,
    error: Option<String>,
}

impl PixelCraft {
    pub(super) fn open_profile_dialog(&mut self) {
        self.stop_playback(false);
        self.profile_dialog = Some(ProfileDialog {
            generation: self.power.bridge_generation,
            selected: None,
            pending: None,
            error: None,
        });
    }

    pub(super) fn document_profile_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.profile_dialog.take() else {
            return;
        };
        if dialog.generation != self.power.bridge_generation {
            return;
        }
        if let Some(job) = &dialog.pending {
            let label = job.label;
            match job.result.try_recv() {
                Ok(result) => {
                    dialog.pending = None;
                    match result {
                        Ok(next) => {
                            if next != self.doc {
                                self.remember_action(label);
                                self.doc = next;
                            }
                            self.status = label.into();
                            return;
                        }
                        Err(error) => dialog.error = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    dialog.error = Some("ICC worker stopped".into());
                    dialog.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut operation = None;
        let mut cancel = false;
        let close = fluent::modal_dialog(ctx, "Color Profile", |ui| {
            let current = match self.doc.color_profile.first().copied() {
                Some(2) => "Embedded ICC profile",
                Some(1)
                    if self
                        .doc
                        .color_profile
                        .get(2)
                        .is_some_and(|flags| flags & 1 != 0) =>
                {
                    "sRGB primaries with a fixed gamma"
                }
                _ => "sRGB",
            };
            ui.label(format!("Current profile · {current}"));
            ui.add_enabled_ui(dialog.pending.is_none(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.pc_button("Choose ICC profile…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("ICC profile", &["icc", "icm"])
                            .pick_file()
                    {
                        dialog.selected = Some(path);
                    }
                    if ui.pc_button("Use sRGB").clicked() {
                        dialog.selected = None;
                    }
                });
                let target = dialog
                    .selected
                    .as_deref()
                    .and_then(Path::file_name)
                    .map_or_else(|| "sRGB".into(), |name| name.to_string_lossy());
                ui.label(format!("Target profile · {target}"));
                ui.horizontal(|ui| {
                    if ui
                        .pc_button("Assign profile")
                        .on_hover_text(
                            "Keep the stored pixel values and change their interpretation.",
                        )
                        .clicked()
                    {
                        operation = Some(false);
                    }
                    if ui
                        .pc_button("Convert profile")
                        .on_hover_text("Transform pixels and palettes to retain their appearance.")
                        .clicked()
                    {
                        operation = Some(true);
                    }
                });
            });
            if dialog.pending.is_some() {
                ui.small("Processing profile…");
            }
            if let Some(error) = &dialog.error {
                ui.small(error);
            }
            cancel = ui.pc_button("Cancel").clicked();
            cancel
        });
        if !close && !cancel {
            if let Some(convert) = operation {
                let label = if convert {
                    "Convert color profile"
                } else {
                    "Assign color profile"
                };
                let input = self.doc.clone();
                let selected = dialog.selected.clone();
                let ctx = ctx.clone();
                let cancelled = Arc::new(AtomicBool::new(false));
                let (tx, rx) = mpsc::channel();
                dialog.pending = Some(ProfileWork {
                    result: rx,
                    label,
                    cancelled: Arc::clone(&cancelled),
                });
                dialog.error = None;
                std::thread::spawn(move || {
                    let target = selected.as_deref().map(read_icc_profile).transpose();
                    let result = target.and_then(|target| {
                        let target = target.as_deref().unwrap_or(&[]);
                        if convert {
                            convert_document_profile_until(&input, target, || {
                                cancelled.load(Ordering::Relaxed)
                            })
                        } else {
                            assign_document_profile(&input, target)
                        }
                    });
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
            }
            self.profile_dialog = Some(dialog);
        }
    }
}
