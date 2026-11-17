use super::*;
use pixelcraft::color::{hsv_to_rgb, rgb_to_hsv};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Wheel {
    #[default]
    Rgb,
    Ryb,
    Normal,
}

impl Wheel {
    fn label(self) -> &'static str {
        match self {
            Self::Rgb => "RGB wheel",
            Self::Ryb => "RYB wheel",
            Self::Normal => "Normal-map wheel",
        }
    }

    fn color(self, position: Vec2, value: f32, alpha: u8) -> Pixel {
        let radius = position.length().min(1.);
        if self == Self::Normal {
            let position = position / position.length().max(1.);
            let z = (1. - radius * radius).max(0.).sqrt();
            let [r, g, b] = [position.x, -position.y, z].map(|n| ((n + 1.) * 127.5).round() as u8);
            return [r, g, b, alpha];
        }
        let angle = (-position.y)
            .atan2(position.x)
            .to_degrees()
            .rem_euclid(360.);
        let hue = if self == Self::Ryb {
            if angle < 120. {
                angle / 2.
            } else if angle < 180. {
                angle - 60.
            } else if angle < 240. {
                120. + 2. * (angle - 180.)
            } else {
                angle
            }
        } else {
            angle
        };
        hsv_to_rgb([hue / 360., radius, value], alpha)
    }

    fn position(self, color: Pixel) -> Vec2 {
        if self == Self::Normal {
            let normal = Vec2::new(
                f32::from(color[0]) / 127.5 - 1.,
                1. - f32::from(color[1]) / 127.5,
            );
            return normal / normal.length().max(1.);
        }
        let [hue, saturation, _] = rgb_to_hsv(color);
        let degrees = hue * 360.;
        let angle = if self == Self::Ryb {
            if degrees < 60. {
                degrees * 2.
            } else if degrees < 120. {
                degrees + 60.
            } else if degrees < 240. {
                180. + (degrees - 120.) / 2.
            } else {
                degrees
            }
        } else {
            degrees
        }
        .to_radians();
        Vec2::new(angle.cos(), -angle.sin()) * saturation
    }
}

#[derive(Default)]
pub(super) struct ColorWheel {
    mode: Wheel,
    secondary: bool,
    texture: Option<((Wheel, u8), egui::TextureHandle)>,
}

impl ColorWheel {
    fn texture(&mut self, ctx: &egui::Context, value: f32) -> &egui::TextureHandle {
        let key = (self.mode, (value * 255.).round() as u8);
        if self.texture.as_ref().is_none_or(|(old, _)| old != &key) {
            const SIZE: usize = 128;
            let mut image = egui::ColorImage::filled([SIZE; 2], Color32::TRANSPARENT);
            let r = SIZE as f32 / 2.;
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let p = Vec2::new((x as f32 + 0.5) / r - 1., (y as f32 + 0.5) / r - 1.);
                    if p.length_sq() <= 1. {
                        let [r, g, b, _] = self.mode.color(p, value, 255);
                        image[(x, y)] = Color32::from_rgb(r, g, b);
                    }
                }
            }
            let texture = ctx.load_texture("color-wheel", image, egui::TextureOptions::LINEAR);
            self.texture = Some((key, texture));
        }
        &self.texture.as_ref().unwrap().1
    }
}

impl PixelCraft {
    pub(super) fn color_wheel_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Color wheel", |ui| {
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("wheel-mode")
                    .selected_text(self.color_wheel.mode.label())
                    .show_ui(ui, |ui| {
                        for mode in [Wheel::Rgb, Wheel::Ryb, Wheel::Normal] {
                            ui.pc_selectable_value(&mut self.color_wheel.mode, mode, mode.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.pc_selectable_value(&mut self.color_wheel.secondary, false, "Primary swatch");
                ui.pc_selectable_value(&mut self.color_wheel.secondary, true, "Secondary swatch");
            });
            let mode = self.color_wheel.mode;
            let secondary = self.color_wheel.secondary;
            let mut color = if secondary { self.bg } else { self.fg };
            let original = color;
            let [hue, saturation, mut value] = rgb_to_hsv(color);
            if mode != Wheel::Normal {
                if ui
                    .add(egui::Slider::new(&mut value, 0.0..=1.0).text("Value"))
                    .changed()
                {
                    color = hsv_to_rgb([hue, saturation, value], color[3]);
                }
            } else {
                value = 1.;
            }
            ui.add(egui::Slider::new(&mut color[3], 0..=255).text("Alpha"));
            let size = ui.available_width().min(160.);
            let texture = self.color_wheel.texture(ui.ctx(), value);
            let response = ui
                .add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(Vec2::splat(size))
                        .sense(Sense::click_and_drag()),
                )
                .on_hover_cursor(egui::CursorIcon::Crosshair);
            let radius = response.rect.width() / 2.;
            if (response.clicked() || response.dragged_by(egui::PointerButton::Primary))
                && let Some(pointer) = response.interact_pointer_pos()
            {
                color = mode.color((pointer - response.rect.center()) / radius, value, color[3]);
            }
            let position = response.rect.center() + mode.position(color) * radius;
            ui.painter()
                .circle_stroke(position, 4., Stroke::new(2., Color32::BLACK));
            ui.painter()
                .circle_stroke(position, 4., Stroke::new(1., Color32::WHITE));
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(egui::Id::new("color-wheel-disk"), response.rect)
            });
            if color != original {
                if secondary {
                    let index = self
                        .doc
                        .palette_for(self.frame)
                        .iter()
                        .position(|&swatch| swatch == color);
                    self.set_secondary_color(color, index);
                } else {
                    self.set_active_color(color);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_colors_follow_rgb_ryb_and_normal_coordinates() {
        let yellow_angle = 120_f32.to_radians();
        let point = Vec2::new(yellow_angle.cos(), -yellow_angle.sin());
        assert_eq!(Wheel::Rgb.color(point, 1., 90), [0, 255, 0, 90]);
        assert_eq!(Wheel::Ryb.color(point, 1., 90), [255, 255, 0, 90]);
        assert!((Wheel::Ryb.position([255, 255, 0, 90]) - point).length() < 0.001);
        assert_eq!(
            Wheel::Normal.color(Vec2::ZERO, 1., 255),
            [128, 128, 255, 255]
        );
        assert_eq!(
            Wheel::Normal.color(Vec2::new(0., -1.), 1., 255),
            [128, 255, 128, 255]
        );
    }
}
