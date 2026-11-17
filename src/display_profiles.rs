use super::*;
use raw_window_handle::HasWindowHandle;
#[cfg(any(windows, target_os = "linux"))]
use raw_window_handle::RawWindowHandle;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

mod platform;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum DisplayChoice {
    #[default]
    Srgb,
    Monitor,
    Custom(PathBuf),
}

#[derive(Clone, Copy)]
pub(super) enum Surface {
    #[cfg(windows)]
    Windows(isize),
    #[cfg(target_os = "linux")]
    X11(u32),
    Unsupported,
}

impl Surface {
    pub(super) fn from_frame(frame: &eframe::Frame) -> Self {
        match frame.window_handle().map(|handle| handle.as_raw()) {
            #[cfg(windows)]
            Ok(RawWindowHandle::Win32(handle)) => Self::Windows(handle.hwnd.get()),
            #[cfg(target_os = "linux")]
            Ok(RawWindowHandle::Xlib(handle)) => {
                u32::try_from(u128::from(handle.window)).map_or(Self::Unsupported, Self::X11)
            }
            #[cfg(target_os = "linux")]
            Ok(RawWindowHandle::Xcb(handle)) => Self::X11(handle.window.get()),
            _ => Self::Unsupported,
        }
    }
}

struct ProfileProbe {
    choice: DisplayChoice,
    receiver: mpsc::Receiver<Result<Vec<u8>, String>>,
}

#[derive(Default)]
pub(super) struct DisplayProfile {
    pub(super) bytes: Arc<Vec<u8>>,
    choice: DisplayChoice,
    probe: Option<ProfileProbe>,
    retry_at: Option<Instant>,
    message: Option<String>,
}

impl PixelCraft {
    pub(super) fn set_display_profile_bytes(&mut self, bytes: Vec<u8>) {
        if *self.display_profile.bytes == bytes {
            return;
        }
        self.display_profile.bytes = Arc::new(bytes);
        self.frame_textures.fill(None);
        self.frame_texture_order.clear();
        self.frame_prefetch = None;
        self.preview_prefetch = None;
        self.preview_frame = None;
        self.preview = None;
        self.texture = None;
        self.onion_texture = None;
        self.power.reference_textures.clear();
        self.clear_rotation_preview_cache();
        self.clear_filter_preview_cache();
    }

    pub(super) fn display_artwork(&self, source: &[u8], pixels: Vec<Pixel>) -> Vec<Pixel> {
        if source == self.display_profile.bytes.as_slice() {
            pixels
        } else {
            pixelcraft::color::transform_pixels(source, &self.display_profile.bytes, &pixels)
                .or_else(|_| pixelcraft::color::to_srgb(source, &pixels))
                .unwrap_or(pixels)
        }
    }

    pub(super) fn poll_display_profile(&mut self, ctx: &egui::Context, surface: Surface) {
        let wanted = self.display_choice().clone();
        let display = &mut self.display_profile;
        if wanted != display.choice {
            display.choice = wanted.clone();
            display.retry_at = None;
            display.message = None;
            if wanted == DisplayChoice::Srgb {
                self.set_display_profile_bytes(Vec::new());
            }
        }
        if let Some(probe) = &self.display_profile.probe {
            let result = match probe.receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("Display profile worker stopped".into())
                }
            };
            let probe = self.display_profile.probe.take().unwrap();
            if probe.choice == wanted {
                match result {
                    Ok(bytes) => {
                        self.display_profile.message = Some("Display profile active".into());
                        self.set_display_profile_bytes(bytes);
                    }
                    Err(error) => {
                        self.display_profile.message = Some(format!("Using sRGB display. {error}"));
                        self.set_display_profile_bytes(Vec::new());
                    }
                }
            }
        }
        if wanted == DisplayChoice::Srgb {
            return;
        }
        let now = Instant::now();
        if let Some(retry) = self.display_profile.retry_at {
            if wanted != DisplayChoice::Monitor {
                return;
            }
            if now < retry {
                ctx.request_repaint_after(retry - now);
                return;
            }
        }
        let (send, receiver) = mpsc::channel();
        self.display_profile.probe = Some(ProfileProbe {
            choice: wanted.clone(),
            receiver,
        });
        self.display_profile.retry_at = Some(now + Duration::from_secs(3));
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = match wanted {
                DisplayChoice::Monitor => platform::monitor_profile(surface),
                DisplayChoice::Custom(path) => pixelcraft::color::read_icc_profile(&path),
                DisplayChoice::Srgb => unreachable!("sRGB needs no profile probe"),
            }
            .and_then(|bytes| {
                pixelcraft::color::validate_display_profile(&bytes)?;
                Ok(bytes)
            });
            let _ = send.send(result);
            ctx.request_repaint();
        });
    }

    pub(super) fn display_profile_ui(&mut self, ui: &mut egui::Ui, choice: &mut DisplayChoice) {
        let label = match choice {
            DisplayChoice::Srgb => "sRGB",
            DisplayChoice::Monitor => "Monitor profile",
            DisplayChoice::Custom(_) => "Custom ICC profile",
        };
        egui::ComboBox::from_label("Display profile")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.pc_selectable_value(choice, DisplayChoice::Srgb, "sRGB");
                ui.pc_selectable_value(choice, DisplayChoice::Monitor, "Monitor profile");
                if ui.pc_button("Choose custom ICC…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("ICC profile", &["icc", "icm"])
                        .pick_file()
                {
                    *choice = DisplayChoice::Custom(path);
                }
            });
        if let DisplayChoice::Custom(path) = choice
            && let Some(name) = path.file_name()
        {
            ui.small(name.to_string_lossy());
        }
        if choice != &DisplayChoice::Srgb {
            ui.small(
                self.display_profile
                    .message
                    .as_deref()
                    .unwrap_or("Reading display profile…"),
            );
        }
        if self.display_choice() != choice {
            ui.ctx().request_repaint();
        }
    }
}
