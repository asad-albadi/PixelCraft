use super::*;
use egui::epaint::text::{FontDefinitions, Fonts, TextOptions};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Typeface {
    Regular,
    Bold,
    Monospace,
}

impl Typeface {
    fn name(self) -> &'static str {
        match self {
            Self::Regular => "Noto Sans",
            Self::Bold => "Noto Sans Bold",
            Self::Monospace => "Monospace",
        }
    }

    fn fonts(self) -> (Fonts, egui::FontFamily) {
        let mut definitions = FontDefinitions::default();
        let family = match self {
            Self::Monospace => egui::FontFamily::Monospace,
            Self::Regular | Self::Bold => {
                let bytes = if self == Self::Regular {
                    include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf").as_slice()
                } else {
                    include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf").as_slice()
                };
                let key = "sprite-text".to_owned();
                definitions
                    .font_data
                    .insert(key.clone(), egui::FontData::from_static(bytes).into());
                definitions
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, key);
                egui::FontFamily::Proportional
            }
        };
        (
            Fonts::new(
                TextOptions {
                    max_texture_side: 4096,
                    subpixel_binning: false,
                    color_transfer_function: egui::epaint::FontColorTransferFunction::Off,
                    ..Default::default()
                },
                definitions,
            ),
            family,
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Settings {
    contents: String,
    face: Typeface,
    size: u16,
    smooth: bool,
    position: [i32; 2],
}

struct TextRaster {
    size: [usize; 2],
    pixels: Vec<Pixel>,
}

fn rasterize(
    settings: &Settings,
    available: [usize; 2],
    color: Pixel,
) -> Result<TextRaster, String> {
    let (mut fonts, family) = settings.face.fonts();
    let galley = fonts.with_pixels_per_point(1.).layout_no_wrap(
        settings.contents.clone(),
        egui::FontId::new(f32::from(settings.size), family),
        Color32::WHITE,
    );
    if !galley.mesh_bounds.is_positive() {
        return Err("Enter text with visible characters".into());
    }
    let atlas = fonts.texture_atlas().image();
    let minimum = galley.mesh_bounds.min.min(Pos2::ZERO);
    let origin = [minimum.x.floor() as i32, minimum.y.floor() as i32];
    let size = [
        (galley.mesh_bounds.max.x.ceil() as i32 - origin[0]).max(0) as usize,
        (galley.mesh_bounds.max.y.ceil() as i32 - origin[1]).max(0) as usize,
    ];
    let size = [size[0].min(available[0]), size[1].min(available[1])];
    if size.contains(&0) || size[0].saturating_mul(size[1]) > 4_000_000 {
        return Err("Text is outside the canvas or exceeds the preview size".into());
    }
    let mut mask = vec![0u8; size[0] * size[1]];
    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv = glyph.uv_rect;
            if uv.is_nothing() {
                continue;
            }
            let left_top = row.pos + (glyph.pos.to_vec2() + uv.offset);
            let left = left_top.x.round() as i32 - origin[0];
            let top = left_top.y.round() as i32 - origin[1];
            let [u0, v0] = uv.min.map(usize::from);
            let [u1, v1] = uv.max.map(usize::from);
            for (dy, v) in (v0..v1).enumerate() {
                for (dx, u) in (u0..u1).enumerate() {
                    let (x, y) = (left + dx as i32, top + dy as i32);
                    if x >= 0 && y >= 0 && x < size[0] as i32 && y < size[1] as i32 {
                        let alpha = atlas[(u, v)].a();
                        let pixel = &mut mask[y as usize * size[0] + x as usize];
                        *pixel = (*pixel).max(alpha);
                    }
                }
            }
        }
    }
    let [r, g, b, ink] = color;
    let pixels = mask
        .into_iter()
        .map(|alpha| {
            let alpha = if settings.smooth {
                alpha
            } else if alpha >= 128 {
                255
            } else {
                0
            };
            let alpha = ((u16::from(alpha) * u16::from(ink) + 127) / 255) as u8;
            if alpha == 0 { CLEAR } else { [r, g, b, alpha] }
        })
        .collect();
    Ok(TextRaster { size, pixels })
}

struct Preview {
    raster: TextRaster,
    tiles: Vec<PreviewTile>,
}

struct PreviewTile {
    origin: [usize; 2],
    texture: egui::TextureHandle,
}

impl Preview {
    fn paint(&self, painter: &egui::Painter, origin: Pos2, step: Vec2) {
        for tile in &self.tiles {
            let [x, y] = tile.origin;
            let rect = Rect::from_min_size(
                origin + Vec2::new(x as f32, y as f32) * step,
                tile.texture.size_vec2() * step,
            );
            painter.image(
                tile.texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
    }
}

pub(super) struct TextDraft {
    tab: usize,
    generation: u64,
    cel: (usize, usize),
    settings: Settings,
    preview: Option<Preview>,
    color: Pixel,
    focus: bool,
    error: Option<String>,
}

impl PixelCraft {
    pub(super) fn begin_sprite_text(&mut self, point: (i32, i32)) {
        if !self.session.doc.editable(self.session.layer)
            || self.session.doc.layers[self.session.layer]
                .tileset
                .is_some()
            || self.session.doc.layers[self.session.layer]
                .adjustment
                .is_some()
        {
            self.status = "Choose an unlocked image layer for text".into();
            return;
        }
        self.text_draft = Some(TextDraft {
            tab: self.active_tab,
            generation: self.power.bridge_generation,
            cel: (self.session.layer, self.session.frame),
            color: self.fg,
            settings: Settings {
                contents: String::new(),
                face: Typeface::Regular,
                size: 16,
                smooth: false,
                position: [point.0, point.1],
            },
            preview: None,
            focus: true,
            error: None,
        });
    }

    fn refresh_sprite_text(&self, ctx: &egui::Context, state: &mut TextDraft) {
        state.error = None;
        state.preview = None;
        if state.settings.contents.trim().is_empty() {
            return;
        }
        let [x, y] = state.settings.position;
        let available = [
            self.session.doc.width.saturating_sub(x as usize),
            self.session.doc.height.saturating_sub(y as usize),
        ];
        match rasterize(&state.settings, available, state.color) {
            Err(error) => state.error = Some(error),
            Ok(mut raster) => {
                let stride = raster.size[0];
                for (i, pixel) in raster.pixels.iter_mut().enumerate() {
                    if !self.selected(x + (i % stride) as i32, y + (i / stride) as i32) {
                        *pixel = CLEAR;
                    }
                }
                if raster.pixels.iter().all(|pixel| *pixel == CLEAR) {
                    state.error = Some("Text has no visible pixels in the selection".into());
                    return;
                }
                let pixels =
                    self.display_artwork(&self.session.doc.color_profile, raster.pixels.clone());
                let max_side = ctx.input(|i| i.max_texture_side).max(1);
                let mut tiles = Vec::new();
                for top in (0..raster.size[1]).step_by(max_side) {
                    for left in (0..stride).step_by(max_side) {
                        let width = (stride - left).min(max_side);
                        let height = (raster.size[1] - top).min(max_side);
                        let data: Vec<_> = (top..top + height)
                            .flat_map(|y| &pixels[y * stride + left..y * stride + left + width])
                            .copied()
                            .collect();
                        let image = color_image(&data, width, height);
                        let texture = ctx.load_texture(
                            "sprite-text-preview",
                            image,
                            egui::TextureOptions::NEAREST,
                        );
                        tiles.push(PreviewTile {
                            origin: [left, top],
                            texture,
                        });
                    }
                }
                state.preview = Some(Preview { raster, tiles });
            }
        }
    }

    pub(super) fn draw_sprite_text(&self, painter: &egui::Painter, board: Rect) {
        let Some(state) = self.text_draft.as_ref().filter(|state| {
            (state.tab, state.generation, state.cel)
                == (
                    self.active_tab,
                    self.power.bridge_generation,
                    (self.session.layer, self.session.frame),
                )
        }) else {
            return;
        };
        let Some(preview) = &state.preview else {
            return;
        };
        let [x, y] = state.settings.position;
        let step = self.canvas_pixel_size();
        preview.paint(
            painter,
            board.min + Vec2::new(x as f32, y as f32) * step,
            step,
        );
    }

    fn place_sprite_text(&mut self, state: TextDraft) {
        let Some(preview) = state.preview else {
            return;
        };
        let [left, top] = state.settings.position.map(|value| value as usize);
        let stride = preview.raster.size[0];
        let (layer, frame) = state.cel;
        let mut next = self.session.doc.clone();
        next.drawing_index = self.drawing_palette_index(state.color);
        for (i, pixel) in preview.raster.pixels.into_iter().enumerate() {
            if pixel[3] != 0 {
                let index = (top + i / stride) * next.width + left + i % stride;
                let before = next.layers[layer].cels[frame][index];
                next.set_pixel(layer, frame, index, over(before, pixel, 255));
            }
        }
        next.drawing_index = None;
        if next != self.session.doc {
            self.remember_action("Place text");
            self.session.doc = next;
        }
        self.status = "Placed text".into();
    }

    pub(super) fn sprite_text_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.text_draft.take() else {
            return;
        };
        if (state.tab, state.generation, state.cel)
            != (
                self.active_tab,
                self.power.bridge_generation,
                (self.session.layer, self.session.frame),
            )
        {
            self.status = "Text placement cancelled after the target cel changed".into();
            return;
        }
        let mut apply = false;
        let mut cancel = false;
        let dismiss = fluent::modal_dialog(ctx, "Place text", |ui| {
            let previous = state.settings.clone();
            let response = egui::ScrollArea::vertical()
                .max_height(96.)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.settings.contents)
                            .hint_text("Text")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3)
                            .char_limit(512),
                    )
                })
                .inner;
            if std::mem::take(&mut state.focus) {
                response.request_focus();
            }
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("sprite-text-face")
                    .selected_text(state.settings.face.name())
                    .show_ui(ui, |ui| {
                        for face in [Typeface::Regular, Typeface::Bold, Typeface::Monospace] {
                            ui.pc_selectable_value(&mut state.settings.face, face, face.name());
                        }
                    });
                ui.add(
                    egui::DragValue::new(&mut state.settings.size)
                        .range(5..=96)
                        .suffix(" px"),
                );
                if self.session.doc.color_mode != ColorMode::Indexed {
                    ui.pc_checkbox(&mut state.settings.smooth, "Antialiasing");
                }
            });
            ui.horizontal(|ui| {
                for (i, (label, limit)) in [
                    ("X", self.session.doc.width),
                    ("Y", self.session.doc.height),
                ]
                .into_iter()
                .enumerate()
                {
                    ui.label(label);
                    ui.add(
                        egui::DragValue::new(&mut state.settings.position[i])
                            .range(0..=limit as i32 - 1),
                    );
                }
            });
            if previous != state.settings {
                self.refresh_sprite_text(ctx, &mut state);
                ctx.request_repaint();
            }
            if let Some(preview) = &state.preview {
                let [width, height] = preview.raster.size;
                let size = Vec2::new(width as f32, height as f32);
                let scale = (ui.available_width() / size.x).min(108. / size.y).min(3.);
                let (rect, _) = ui.allocate_exact_size(size * scale, Sense::hover());
                preview.paint(&ui.painter_at(rect), rect.min, Vec2::splat(scale));
            }
            if let Some(error) = &state.error {
                ui.label(error);
            }
            ui.horizontal(|ui| {
                let button = ui
                    .add_enabled_ui(state.preview.is_some(), |ui| ui.pc_button("Place"))
                    .inner;
                #[cfg(test)]
                ctx.data_mut(|data| {
                    data.insert_temp(egui::Id::new("sprite-text-apply"), button.rect)
                });
                apply = button.clicked();
                cancel = ui.pc_button("Cancel").clicked();
            });
            apply || cancel
        });
        if apply {
            self.place_sprite_text(state);
        } else if !dismiss && !cancel {
            self.text_draft = Some(state);
        }
    }

    #[cfg(test)]
    pub(super) fn text_preview_texture(&self) -> Option<&egui::TextureHandle> {
        self.text_draft
            .as_ref()?
            .preview
            .as_ref()?
            .tiles
            .first()
            .map(|tile| &tile.texture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_raster_supports_binary_and_antialiased_coverage() {
        let mut settings = Settings {
            contents: "Qä\nHi".into(),
            face: Typeface::Regular,
            size: 19,
            smooth: false,
            position: [0, 0],
        };
        let color = [222, 130, 44, 204];
        let crisp = rasterize(&settings, [100, 100], color).unwrap();
        assert!(crisp.pixels.contains(&color));
        assert!(crisp.pixels.iter().all(|&p| p == CLEAR || p == color));
        settings.smooth = true;
        let smooth = rasterize(&settings, [100, 100], color).unwrap();
        assert_eq!(smooth.size, crisp.size);
        assert!(smooth.pixels.iter().any(|p| (1..204).contains(&p[3])));
        let clipped = rasterize(&settings, [9, 13], color).unwrap();
        assert_eq!(clipped.size, [9, 13]);
        for y in 0..13 {
            assert_eq!(
                &clipped.pixels[9 * y..9 * (y + 1)],
                &smooth.pixels[y * smooth.size[0]..y * smooth.size[0] + 9]
            );
        }
    }
}
