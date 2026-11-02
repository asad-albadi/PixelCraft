mod script_ui;
use eframe::egui::{self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use pixelcraft::{
    extensions::{self, Manager, PluginCommand, Session},
    formats,
    model::*,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
// A stepped P and a detached gold pixel form PixelCraft's emblem.
const BRAND_MARK: [&str; 7] = [
    "......g", ".tttt..", ".t..tt.", ".tttt..", ".t.....", ".t.....", ".......",
];
fn app_icon() -> egui::IconData {
    let mut rgba = vec![0; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let cell = BRAND_MARK
                .get(y / 8)
                .and_then(|row| row.as_bytes().get(x / 8))
                .copied()
                .unwrap_or(b'.');
            let color = match cell {
                b'g' => [241, 190, 93, 255],
                b't' => [86, 201, 184, 255],
                _ => [27, 43, 49, 255],
            };
            rgba[(y * 64 + x) * 4..(y * 64 + x + 1) * 4].copy_from_slice(&color);
        }
    }
    egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}
#[derive(Clone, Copy)]
struct ThemeColors {
    face: Color32,
    ink: Color32,
    canvas: Color32,
    accent: Color32,
    hover: Color32,
    border: Color32,
}
impl ThemeColors {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                face: Color32::from_rgb(27, 43, 49),
                ink: Color32::from_rgb(229, 241, 238),
                canvas: Color32::from_rgb(16, 28, 34),
                accent: Color32::from_rgb(38, 111, 113),
                hover: Color32::from_rgb(45, 76, 82),
                border: Color32::from_rgb(76, 111, 118),
            }
        } else {
            Self {
                face: Color32::from_rgb(222, 237, 233),
                ink: Color32::from_rgb(27, 53, 60),
                canvas: Color32::from_rgb(125, 153, 158),
                accent: Color32::from_rgb(113, 187, 178),
                hover: Color32::from_rgb(197, 225, 215),
                border: Color32::from_rgb(120, 157, 156),
            }
        }
    }
}
fn theme_path() -> Option<PathBuf> {
    Some(extensions::config_directory().join("theme.json"))
}
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pencil,
    Eraser,
    Fill,
    Picker,
    Line,
    Rect,
    Ellipse,
    Select,
    Hand,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Fill => "Paint bucket",
            Self::Picker => "Eyedropper",
            Self::Line => "Line",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Select => "Selection",
            Self::Hand => "Hand",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Pencil => "B",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Picker => "I",
            Self::Line => "L",
            Self::Rect => "U",
            Self::Ellipse => "O",
            Self::Select => "M",
            Self::Hand => "H",
        }
    }
}
#[derive(Clone, Copy)]
enum Pending {
    New,
    Open,
    Close,
    CloseTab,
    Demo,
}
#[derive(Clone)]
struct Clipboard {
    w: usize,
    h: usize,
    pixels: Vec<Pixel>,
}
struct SpriteTab {
    doc: Document,
    saved: Document,
    path: Option<PathBuf>,
    layer: usize,
    frame: usize,
    undo: Vec<Document>,
    redo: Vec<Document>,
    zoom: f32,
    pan: Vec2,
    selection: Option<(i32, i32, i32, i32)>,
    palette_index: usize,
    active_tag: Option<usize>,
}
// GlobalPref::ColorBar 0x366d20: factory colors, shared across documents.
const DEFAULT_FG: Pixel = [255, 255, 255, 255];
const DEFAULT_BG: Pixel = [0, 0, 0, 255];
struct PixelCraft {
    tabs: Vec<Option<SpriteTab>>,
    active_tab: usize,
    dark_mode: bool,
    theme_path: Option<PathBuf>,
    doc: Document,
    saved: Document,
    path: Option<PathBuf>,
    layer: usize,
    frame: usize,
    tool: Tool,
    fg: Pixel,
    bg: Pixel,
    named_colors: Vec<(String, Pixel)>,
    active_color: usize,
    used_colors_snapshot: Option<Document>,
    used_colors: Vec<Pixel>,
    brush: i32,
    zoom: f32,
    pan: Vec2,
    grid: bool,
    onion: bool,
    filled: bool,
    playing: bool,
    last_tick: Instant,
    undo: Vec<Document>,
    redo: Vec<Document>,
    selection: Option<(i32, i32, i32, i32)>,
    clipboard: Option<Clipboard>,
    start: Option<(i32, i32)>,
    last: Option<(i32, i32)>,
    stroke_base: Option<Document>,
    stroke_color: Pixel,
    texture: Option<egui::TextureHandle>,
    preview: Option<egui::TextureHandle>,
    render_snapshot: Option<Document>,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    cel_occupied: Vec<Vec<bool>>,
    onion_texture: Option<egui::TextureHandle>,
    properties_open: bool,
    palette_index: usize,
    tile_size: usize,
    selected_tile: u32,
    active_tag: Option<usize>,
    tag_name: String,
    playback_reverse: bool,
    custom_shortcuts: std::collections::BTreeMap<String, String>,
    translations: std::collections::BTreeMap<String, String>,
    status: String,
    hover: Option<(i32, i32)>,
    pending: Option<Pending>,
    new_dialog: bool,
    new_w: usize,
    new_h: usize,
    rename: Option<String>,
    help: bool,
    extensions_open: bool,
    extension_manager: Option<Manager>,
    plugin_commands: Vec<PluginCommand>,
    extension_error: String,
    extension_session: Option<Session>,
    script_ui: script_ui::ScriptUi,
    script_preview_doc: Option<Document>,
    script_job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    shot: Option<PathBuf>,
    shot_frames: u32,
    shot_requested: bool,
}
impl PixelCraft {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, shot);
        app.theme_path = theme_path();
        if let Some(dark) = app
            .theme_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<bool>(&bytes).ok())
        {
            app.set_dark_mode(&cc.egui_ctx, dark);
        }
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("shortcuts.json")) {
            app.custom_shortcuts = serde_json::from_slice(&bytes).unwrap_or_default();
        }
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("language.ini")) {
            app.translations = pixelcraft::contributions::language(&bytes).unwrap_or_default();
        }
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("custom-theme.json")) {
            let _ = app.apply_theme_asset(&cc.egui_ctx, &bytes);
        }
        app.refresh_extensions();
        app
    }
    fn with_context(ctx: &egui::Context, shot: Option<PathBuf>) -> Self {
        Self::apply_theme(ctx, true);
        let doc = Document::demo();
        Self {
            tabs: vec![None],
            active_tab: 0,
            dark_mode: true,
            theme_path: None,
            saved: doc.clone(),
            doc,
            path: None,
            layer: 1,
            frame: 0,
            tool: Tool::Pencil,
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            named_colors: vec![("Primary".into(), DEFAULT_FG)],
            active_color: 0,
            used_colors_snapshot: None,
            used_colors: vec![],
            brush: 1,
            zoom: 10.,
            pan: Vec2::ZERO,
            grid: false,
            onion: false,
            filled: false,
            playing: false,
            last_tick: Instant::now(),
            undo: vec![],
            redo: vec![],
            selection: None,
            clipboard: None,
            start: None,
            last: None,
            stroke_base: None,
            stroke_color: CLEAR,
            texture: None,
            preview: None,
            render_snapshot: None,
            frame_textures: vec![],
            cel_occupied: vec![],
            onion_texture: None,
            properties_open: false,
            palette_index: 0,
            tile_size: 16,
            selected_tile: 1,
            active_tag: None,
            tag_name: "Animation".into(),
            playback_reverse: false,
            custom_shortcuts: std::collections::BTreeMap::new(),
            translations: std::collections::BTreeMap::new(),
            status: "Ready. Draw on the canvas, or create a new sprite.".into(),
            hover: None,
            pending: None,
            new_dialog: false,
            new_w: 32,
            new_h: 32,
            rename: None,
            help: false,
            extensions_open: false,
            extension_manager: Manager::new().ok(),
            plugin_commands: vec![],
            extension_error: String::new(),
            extension_session: None,
            script_job: None,
            script_ui: script_ui::ScriptUi::default(),
            script_preview_doc: None,
            shot,
            shot_frames: 0,
            shot_requested: false,
        }
    }
    fn apply_theme(ctx: &egui::Context, dark: bool) {
        let colors = ThemeColors::new(dark);
        let theme = if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.panel_fill = colors.face;
        style.visuals.window_fill = if dark {
            Color32::from_rgb(32, 52, 59)
        } else {
            Color32::from_rgb(237, 246, 241)
        };
        style.visuals.override_text_color = Some(colors.ink);
        style.visuals.selection.bg_fill = colors.accent;
        style.visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.corner_radius = egui::CornerRadius::same(2);
            w.fg_stroke = Stroke::new(1., colors.ink);
        }
        style.visuals.widgets.inactive.bg_fill = if dark {
            Color32::from_rgb(36, 61, 68)
        } else {
            Color32::from_rgb(225, 217, 204)
        };
        style.visuals.widgets.hovered.bg_fill = colors.hover;
        style.visuals.widgets.active.bg_fill = colors.accent;
        style.spacing.item_spacing = Vec2::new(7., 5.);
        style.spacing.button_padding = Vec2::new(8., 5.);
        style.spacing.interact_size.y = 26.;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
        ctx.set_global_style(style);
    }
    fn colors(&self) -> ThemeColors {
        ThemeColors::new(self.dark_mode)
    }
    fn set_dark_mode(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark_mode = dark;
        Self::apply_theme(ctx, dark);
        ctx.request_repaint();
    }
    fn save_theme(&mut self) {
        if let Some(path) = &self.theme_path {
            let result = std::fs::create_dir_all(path.parent().unwrap())
                .and_then(|()| std::fs::write(path, if self.dark_mode { "true" } else { "false" }));
            if let Err(error) = result {
                self.status = format!("Theme changed, but preference could not be saved: {error}");
            }
        }
    }
    fn dirty(&self) -> bool {
        self.doc != self.saved
    }
    fn frame_texture(&mut self, ctx: &egui::Context, f: usize) -> egui::TextureHandle {
        if self.render_snapshot.as_ref() != Some(&self.doc) {
            self.frame_textures = vec![None; self.doc.durations.len()];
            self.cel_occupied = self
                .doc
                .layers
                .iter()
                .map(|l| {
                    (0..self.doc.durations.len())
                        .map(|f| {
                            l.group
                                || l.tile_cels[f].iter().any(|v| *v != 0)
                                || l.cels[f].iter().any(|p| p[3] > 0)
                        })
                        .collect()
                })
                .collect();
            self.render_snapshot = Some(self.doc.clone());
        }
        if self.frame_textures[f].is_none() {
            let pixels = pixelcraft::color::display_pixels(&self.doc, f).unwrap_or_else(|error| {
                self.status = format!("Color profile: {error}; showing unconverted values");
                self.doc.composite(f)
            });
            self.frame_textures[f] = Some(ctx.load_texture(
                format!("frame-{f}"),
                color_image(&pixels, self.doc.width, self.doc.height),
                egui::TextureOptions::NEAREST,
            ));
        }
        self.frame_textures[f].as_ref().unwrap().clone()
    }
    fn paint_at(&mut self, p: (i32, i32), color: Pixel) {
        self.doc.drawing_index = self.drawing_palette_index(color);
        if let Some(ti) = self.doc.layers[self.layer].tileset {
            let t = &self.doc.tilesets[ti];
            let (ox, oy) = self.doc.layers[self.layer].cel_meta[self.frame]
                .tilemap
                .as_ref()
                .map_or((0, 0), |g| (g.x, g.y));
            if p.0 < ox || p.1 < oy {
                self.status = "Point is outside this tilemap cel".into();
                return;
            }
            let (x, y) = (
                (p.0 - ox) as usize / t.width,
                (p.1 - oy) as usize / t.height,
            );
            let id = if self.tool == Tool::Eraser {
                0
            } else {
                self.selected_tile
            };
            if let Err(e) = self.doc.paint_tile(self.layer, self.frame, x, y, id) {
                self.status = e;
            }
        } else {
            self.doc
                .stamp(self.layer, self.frame, p.0, p.1, color, self.brush);
        }
        self.doc.drawing_index = None;
    }
    fn advance_playback(&mut self) {
        if let Some(t) = self.active_tag.and_then(|i| self.doc.tags.get(i)) {
            let reverse = match t.direction {
                1 => true,
                2 | 3 => self.playback_reverse,
                _ => false,
            };
            if reverse {
                if self.frame <= t.from {
                    if t.direction >= 2 {
                        self.playback_reverse = false;
                        self.frame = (t.from + 1).min(t.to);
                    } else {
                        self.frame = t.to;
                    }
                } else {
                    self.frame -= 1;
                }
            } else if self.frame >= t.to {
                if t.direction >= 2 {
                    self.playback_reverse = true;
                    self.frame = t.to.saturating_sub(1).max(t.from);
                } else {
                    self.frame = t.from;
                }
            } else {
                self.frame += 1;
            }
        } else {
            self.frame = (self.frame + 1) % self.doc.durations.len();
        }
    }
    fn apply_theme_asset(&mut self, ctx: &egui::Context, bytes: &[u8]) -> Result<(), String> {
        let v = pixelcraft::contributions::theme(bytes)?;
        let dark = v.get("dark").and_then(|v| v.as_bool()).unwrap_or(true);
        self.set_dark_mode(ctx, dark);
        fn color(v: &serde_json::Value, k: &str) -> Option<Color32> {
            let a = v.get(k)?.as_array()?;
            if a.len() < 3 {
                return None;
            }
            Some(Color32::from_rgb(
                a[0].as_u64()? as u8,
                a[1].as_u64()? as u8,
                a[2].as_u64()? as u8,
            ))
        }
        let mut style = (*ctx.style_of(ctx.theme())).clone();
        if let Some(c) = color(&v, "panel") {
            style.visuals.panel_fill = c;
            style.visuals.window_fill = c;
        }
        if let Some(c) = color(&v, "text") {
            style.visuals.override_text_color = Some(c);
        }
        if let Some(c) = color(&v, "accent") {
            style.visuals.selection.bg_fill = c;
            style.visuals.widgets.active.bg_fill = c;
        }
        ctx.set_global_style(style);
        Ok(())
    }
    fn apply_extension_asset(
        &mut self,
        ctx: &egui::Context,
        kind: &str,
        id: &str,
        contribution: &extensions::Contribution,
    ) -> Result<(), String> {
        let path = self
            .extension_manager
            .as_ref()
            .ok_or("Storage unavailable")?
            .asset(id, contribution)?;
        let path = if path.is_dir() && kind == "theme" {
            path.join("theme.xml")
        } else {
            path
        };
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        match kind {
            "palette" => {
                let palette: Vec<Pixel> =
                    if path.extension().and_then(|x| x.to_str()) == Some("json") {
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
                    } else if path.extension().and_then(|x| x.to_str()) == Some("gpl") {
                        String::from_utf8(bytes.clone())
                            .map_err(|e| e.to_string())?
                            .lines()
                            .filter_map(|line| {
                                let mut values = line.split_whitespace();
                                Some([
                                    values.next()?.parse().ok()?,
                                    values.next()?.parse().ok()?,
                                    values.next()?.parse().ok()?,
                                    255,
                                ])
                            })
                            .collect()
                    } else {
                        formats::load(&path)?.palette
                    };
                if palette.is_empty() || palette.len() > 256 {
                    return Err("Palette must contain 1–256 colors".into());
                }
                self.remember();
                let mode = self.doc.color_mode;
                self.doc.palette = palette;
                self.doc.set_color_mode(mode);
                self.palette_index = 0;
            }
            "theme" => {
                self.apply_theme_asset(ctx, &bytes)?;
                std::fs::create_dir_all(extensions::config_directory())
                    .map_err(|e| e.to_string())?;
                std::fs::write(
                    extensions::config_directory().join("custom-theme.json"),
                    &bytes,
                )
                .map_err(|e| e.to_string())?;
            }
            "shortcuts" => {
                let parsed = pixelcraft::contributions::shortcuts(&bytes)?;
                if !parsed.unsupported.is_empty() {
                    self.extension_error = format!(
                        "{} shortcut entries use unavailable commands or parameters",
                        parsed.unsupported.len()
                    );
                }
                let shortcuts = parsed.bindings;
                for binding in shortcuts.keys() {
                    if Key::from_name(binding.split('+').next_back().unwrap_or("")).is_none() {
                        return Err(format!("Unknown shortcut key: {binding}"));
                    }
                }
                self.custom_shortcuts = shortcuts;
                std::fs::create_dir_all(extensions::config_directory())
                    .map_err(|e| e.to_string())?;
                std::fs::write(
                    extensions::config_directory().join("shortcuts.json"),
                    serde_json::to_vec(&self.custom_shortcuts).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            }
            "language" => {
                self.translations = pixelcraft::contributions::language(&bytes)?;
                std::fs::create_dir_all(extensions::config_directory())
                    .map_err(|e| e.to_string())?;
                std::fs::write(extensions::config_directory().join("language.ini"), &bytes)
                    .map_err(|e| e.to_string())?;
            }
            "dither" => {
                let matrix = pixelcraft::contributions::DitheringMatrix::load(&path)?;
                self.remember();
                self.doc.dithering = Some(matrix);
                self.status =
                    "Dithering matrix selected; use Indexed color mode to apply it".into();
            }
            _ => return Err("Unsupported asset kind".into()),
        }
        self.status = format!("Applied {kind} from {id}");
        Ok(())
    }
    fn import_tileset(&mut self) -> Result<(), String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG tilesheet", &["png"])
            .pick_file()
        else {
            return Ok(());
        };
        let im = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
        let size = self.tile_size;
        let (w, h) = (im.width() as usize, im.height() as usize);
        if w == 0 || h == 0 || w % size != 0 || h % size != 0 || w * h > MAX_PIXELS {
            return Err(
                "Tilesheet dimensions must be multiples of the tile size and fit the pixel budget"
                    .into(),
            );
        }
        let mut tiles = vec![];
        for ty in (0..h).step_by(size) {
            for tx in (0..w).step_by(size) {
                let mut p = vec![];
                for y in 0..size {
                    for x in 0..size {
                        p.push(convert(
                            im.get_pixel((tx + x) as u32, (ty + y) as u32).0,
                            self.doc.color_mode,
                            &self.doc.palette,
                        ));
                    }
                }
                tiles.push(p.into());
            }
        }
        self.remember();
        self.doc.tilesets.push(Tileset {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            width: size,
            height: size,
            tiles,
        });
        self.doc.add_tile_layer(self.doc.tilesets.len() - 1)?;
        self.layer = self.doc.layers.len() - 1;
        self.selected_tile = 1;
        Ok(())
    }
    fn properties_dialog(&mut self, ctx: &egui::Context) {
        if !self.properties_open {
            return;
        }
        let mut open = true;
        let mut edited = self.doc.clone();
        let mut import = false;
        let mut capture = false;
        let mut add_group = false;
        let mut add_tile = false;
        let mut remove_tag = None;
        egui::Window::new("Sprite properties").open(&mut open).default_width(550.).max_height(660.).vscroll(true).show(ctx,|ui|{
            ui.heading("Color & project");let mut mode=edited.color_mode;
            egui::ComboBox::from_label("Color mode").selected_text(format!("{:?}",mode)).show_ui(ui,|ui|{for value in [ColorMode::Rgba,ColorMode::Indexed,ColorMode::Grayscale]{ui.selectable_value(&mut mode,value,format!("{:?}",value));}});if mode!=edited.color_mode{edited.set_color_mode(mode);}
            let mut palette_size=edited.palette.len();ui.add(egui::DragValue::new(&mut palette_size).range(1..=256).prefix("Palette entries "));if palette_size!=edited.palette.len(){edited.palette.resize(palette_size,[0,0,0,255]);edited.palette_names.resize(palette_size,String::new());for p in &mut edited.frame_palettes{p.resize(palette_size,[0,0,0,255]);}let mode=edited.color_mode;edited.set_color_mode(mode);self.palette_index=self.palette_index.min(palette_size-1);}
            ui.small(format!("Canvas: {} × {} · Limit: 16384 per side, 64 million stored pixels",edited.width,edited.height));
            ui.small(format!("Preserved: {} metadata chunks · {} profile bytes · {} off-canvas pixels",edited.metadata.len(),edited.color_profile.len(),edited.layers.iter().flat_map(|l|&l.cel_meta).map(|m|m.outside.len()).sum::<usize>()));
            ui.separator();ui.heading("Active layer & cel");ui.label(&edited.layers[self.layer].name);
            let mut blend=edited.layers[self.layer].blend;
            egui::ComboBox::from_label("Blend mode").selected_text(format!("{:?}",blend)).show_ui(ui,|ui|{for v in BlendMode::ALL{ui.selectable_value(&mut blend,v,format!("{:?}",v));}});edited.layers[self.layer].blend=blend;
            let groups:Vec<_>=edited.layers.iter().enumerate().filter(|(i,l)|*i!=self.layer&&l.group).map(|(i,l)|(i,l.name.clone())).collect();
            let mut parent=edited.layers[self.layer].parent;
            egui::ComboBox::from_label("Group").selected_text(parent.and_then(|i|edited.layers.get(i)).map(|l|l.name.as_str()).unwrap_or("Root")).show_ui(ui,|ui|{ui.selectable_value(&mut parent,None,"Root");for(i,name)in groups{ui.selectable_value(&mut parent,Some(i),name);}});
            edited.layers[self.layer].parent=parent;
            ui.add(egui::Slider::new(&mut edited.layers[self.layer].cel_meta[self.frame].opacity,0..=255).text("Cel opacity"));
            ui.add(egui::DragValue::new(&mut edited.layers[self.layer].cel_meta[self.frame].z_index).range(-32768..=32767).prefix("Cel order "));
            if ui.button("+ New group").clicked(){add_group=true;}
            ui.separator();ui.heading("Tilesets & tile layers");
            ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.tile_size).range(1..=256).prefix("Tile size "));if ui.button("Import PNG tilesheet…").clicked(){import=true;}if ui.button("Capture selection as tile").clicked(){capture=true;}});
            if !edited.tilesets.is_empty(){ui.horizontal(|ui|{if ui.button("+ Tile layer from last tileset").clicked(){add_tile=true;}ui.add(egui::DragValue::new(&mut self.selected_tile).range(1..=edited.tilesets.last().map(|t|t.tiles.len()).unwrap_or(1).max(1) as u32).prefix("Paint tile "));});}
            ui.small("Select a tile layer, then paint with Pencil; Eraser clears a tile. Tile 0 is empty.");
            ui.separator();ui.heading("Named animation ranges");ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.tag_name);if ui.button("+ Range").clicked(){edited.tags.push(FrameTag{name:self.tag_name.clone(),from:self.frame,to:self.frame,direction:0, repeat:0,color:[110,170,220]});}});
            for(i,t)in edited.tags.iter_mut().enumerate(){ui.horizontal(|ui|{ui.text_edit_singleline(&mut t.name);let mut from=t.from+1;let mut to=t.to+1;ui.add(egui::DragValue::new(&mut from).range(1..=edited.durations.len()).prefix("From "));ui.add(egui::DragValue::new(&mut to).range(from..=edited.durations.len()).prefix("To "));t.from=from-1;t.to=to.max(from)-1;if ui.small_button("×").clicked(){remove_tag=Some(i);}});ui.horizontal(|ui|{egui::ComboBox::from_id_salt(("direction",i)).selected_text(["Forward","Reverse","Ping-pong","Ping-pong reverse"][t.direction as usize]).show_ui(ui,|ui|{for (d,name)in ["Forward","Reverse","Ping-pong","Ping-pong reverse"].iter().enumerate(){ui.selectable_value(&mut t.direction,d as u8,*name);}});if ui.selectable_label(self.active_tag==Some(i),"Use for playback").clicked(){self.active_tag=Some(i);self.frame=if t.direction==1||t.direction==3{t.to}else{t.from};self.playback_reverse=t.direction==3;}});}
            if ui.button("Use all frames for playback").clicked(){self.active_tag=None;}ui.small("Range names, endpoints and playback direction are saved in the project.");
        });
        self.properties_open = open;
        if let Some(i) = remove_tag {
            edited.tags.remove(i);
            self.active_tag = None;
        }
        if edited != self.doc {
            match edited.validate() {
                Ok(()) => {
                    self.remember();
                    self.doc = edited;
                }
                Err(e) => self.status = e,
            }
        }
        if add_group {
            self.remember();
            match self.doc.add_group("Group") {
                Ok(g) => self.layer = g,
                Err(e) => self.status = e,
            }
        }
        if add_tile {
            self.remember();
            if let Err(e) = self.doc.add_tile_layer(self.doc.tilesets.len() - 1) {
                self.status = e;
            } else {
                self.layer = self.doc.layers.len() - 1;
            }
        }
        if capture {
            let (x0, y0, x1, y1) = self.selection.unwrap_or((
                0,
                0,
                self.tile_size.min(self.doc.width) as i32 - 1,
                self.tile_size.min(self.doc.height) as i32 - 1,
            ));
            let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
            let pixels = self.doc.layer_pixels(self.layer, self.frame);
            let mut p = vec![];
            for y in y0..=y1 {
                for x in x0..=x1 {
                    p.push(pixels[y as usize * self.doc.width + x as usize]);
                }
            }
            self.remember();
            self.doc.tilesets.push(Tileset {
                name: "Captured tile".into(),
                width: w,
                height: h,
                tiles: vec![p.into()],
            });
            self.selected_tile = 1;
        }
        if import {
            if let Err(e) = self.import_tileset() {
                self.status = format!("Could not import tileset: {e}");
            }
        }
    }
    fn remember(&mut self) {
        self.push_undo(self.doc.clone());
        self.redo.clear();
    }
    fn push_undo(&mut self, d: Document) {
        self.undo.push(d);
        while self.undo.len() > 1 && (self.undo.len() > 64 || self.history_bytes() > 192_000_000) {
            self.undo.remove(0);
        }
    }
    fn history_bytes(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut bytes = 0usize;
        for d in self
            .undo
            .iter()
            .chain(std::iter::once(&self.doc))
            .chain(std::iter::once(&self.saved))
        {
            for l in &d.layers {
                for p in &l.cels {
                    if seen.insert(std::sync::Arc::as_ptr(&p.0) as usize) {
                        bytes = bytes.saturating_add(p.len() * 4);
                    }
                }
                for m in &l.cel_meta {
                    if let Some(p) = &m.indices {
                        if seen.insert(std::sync::Arc::as_ptr(p) as usize) {
                            bytes = bytes.saturating_add(p.len());
                        }
                    }
                    bytes =
                        bytes.saturating_add(m.outside.len() * std::mem::size_of::<OutsidePixel>());
                }
                bytes =
                    bytes.saturating_add(l.tile_cels.iter().map(|v| v.len() * 4).sum::<usize>());
            }
            for t in &d.tilesets {
                for p in &t.tiles {
                    if seen.insert(std::sync::Arc::as_ptr(&p.0) as usize) {
                        bytes = bytes.saturating_add(p.len() * 4);
                    }
                }
            }
            bytes = bytes.saturating_add(d.metadata.iter().map(|m| m.data.len()).sum::<usize>());
        }
        bytes
    }

    fn dialog_open(&self) -> bool {
        self.pending.is_some()
            || self.new_dialog
            || self.rename.is_some()
            || self.help
            || self.properties_open
            || self.extensions_open
            || (self.script_job.is_some() && !self.script_ui.modeless)
            || self.status.starts_with("Could not")
            || self.status.starts_with("Export failed")
    }
    fn cancel_gesture(&mut self) {
        self.start = None;
        self.last = None;
        self.stroke_base = None;
    }

    fn clamp(&mut self) {
        if self.active_tag.is_some_and(|i| i >= self.doc.tags.len()) {
            self.active_tag = None;
        }
        self.layer = self.layer.min(self.doc.layers.len() - 1);
        self.frame = self.frame.min(self.doc.durations.len() - 1);
        self.selection = None;
    }
    fn undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.clamp();
            self.status = "Undo".into();
        }
    }
    fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            let old = std::mem::replace(&mut self.doc, d);
            self.push_undo(old);
            self.clamp();
            self.status = "Redo".into();
        }
    }
    fn replace(&mut self, d: Document, path: Option<PathBuf>) {
        self.extension_session = None;
        self.active_tag = None;
        self.palette_index = 0;
        self.render_snapshot = None;
        self.doc = d;
        self.saved = self.doc.clone();
        self.path = path;
        self.layer = 0;
        self.frame = 0;
        self.undo.clear();
        self.redo.clear();
        self.selection = None;
        self.pan = Vec2::ZERO;
        self.zoom = (480. / self.doc.width.max(self.doc.height) as f32)
            .floor()
            .clamp(1., 16.);
        self.playing = false;
    }
    fn open_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.cancel_gesture();
        let tab = SpriteTab {
            doc: self.doc.clone(),
            saved: self.saved.clone(),
            path: self.path.clone(),
            layer: self.layer.clone(),
            frame: self.frame.clone(),
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            zoom: self.zoom.clone(),
            pan: self.pan.clone(),
            selection: self.selection.clone(),
            palette_index: self.palette_index.clone(),
            active_tag: self.active_tag.clone(),
        };
        self.tabs[self.active_tab] = Some(tab);
        self.active_tab = self.tabs.len();
        self.tabs.push(None);
        self.replace(doc, path);
    }
    fn switch_tab(&mut self, index: usize) {
        if index == self.active_tab || index >= self.tabs.len() || self.script_job.is_some() {
            return;
        }
        self.cancel_gesture();
        let mut tab = self.tabs[index].take().unwrap();
        std::mem::swap(&mut self.doc, &mut tab.doc);
        std::mem::swap(&mut self.saved, &mut tab.saved);
        std::mem::swap(&mut self.path, &mut tab.path);
        std::mem::swap(&mut self.layer, &mut tab.layer);
        std::mem::swap(&mut self.frame, &mut tab.frame);
        std::mem::swap(&mut self.undo, &mut tab.undo);
        std::mem::swap(&mut self.redo, &mut tab.redo);
        std::mem::swap(&mut self.zoom, &mut tab.zoom);
        std::mem::swap(&mut self.pan, &mut tab.pan);
        std::mem::swap(&mut self.selection, &mut tab.selection);
        std::mem::swap(&mut self.palette_index, &mut tab.palette_index);
        std::mem::swap(&mut self.active_tag, &mut tab.active_tag);
        self.tabs[self.active_tab] = Some(tab);
        self.active_tab = index;
        self.extension_session = None;
        self.render_snapshot = None;
        self.playing = false;
        self.hover = None;
        self.status = "Switched sprite".into();
    }
    fn request(&mut self, p: Pending) {
        if self.dirty() && matches!(p, Pending::Close | Pending::CloseTab) {
            self.pending = Some(p)
        } else {
            self.perform(p)
        }
    }
    fn perform(&mut self, p: Pending) {
        match p {
            Pending::New => self.new_dialog = true,
            Pending::Demo => {
                self.open_document(Document::demo(), None);
                self.layer = 1;
            }
            Pending::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Project, sprite or image",
                        &["pixel", "aseprite", "ase", "png"],
                    )
                    .set_directory(
                        extensions::projects_directory().unwrap_or_else(|_| PathBuf::from(".")),
                    )
                    .pick_file()
                {
                    match formats::load(&path) {
                        Ok(d) => {
                            let ase = matches!(
                                path.extension().and_then(|e| e.to_str()),
                                Some("pixel" | "ase" | "aseprite")
                            );
                            self.open_document(d, if ase { Some(path) } else { None });
                            self.status = "Opened sprite".into();
                        }
                        Err(e) => self.status = format!("Could not open: {e}"),
                    }
                }
            }
            Pending::CloseTab => {
                if self.tabs.len() == 1 {
                    self.replace(Document::new(32, 32), None);
                } else {
                    let closed = self.active_tab;
                    let next = if closed == 0 { 1 } else { closed - 1 };
                    self.switch_tab(next);
                    self.tabs.remove(closed);
                    if self.active_tab > closed {
                        self.active_tab -= 1;
                    }
                }
            }
            Pending::Close => {}
        }
    }
    fn finish_pending(&mut self, ctx: &egui::Context, pending: Pending) {
        if matches!(pending, Pending::Close) {
            if let Some(index) = self
                .tabs
                .iter()
                .position(|tab| tab.as_ref().is_some_and(|tab| tab.doc != tab.saved))
            {
                self.switch_tab(index);
                self.pending = Some(Pending::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            self.perform(pending);
        }
    }
    fn save(&mut self, as_new: bool) -> bool {
        let path = if !as_new { self.path.clone() } else { None }.or_else(|| {
            rfd::FileDialog::new()
                .add_filter("PixelCraft project", &["pixel"])
                .add_filter("ASE sprite", &["aseprite"])
                .set_directory(
                    extensions::projects_directory().unwrap_or_else(|_| PathBuf::from(".")),
                )
                .set_file_name(
                    self.path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|p| p.to_str())
                        .unwrap_or("Untitled.pixel"),
                )
                .save_file()
        });
        let Some(mut path) = path else { return false };
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("pixel" | "ase" | "aseprite")
        ) {
            if path.extension().is_none() {
                path.set_extension("pixel");
            } else {
                self.status = "Could not save: choose a .pixel, .aseprite or .ase filename.".into();
                return false;
            }
        }
        match formats::save(&path, &self.doc) {
            Ok(()) => {
                self.status = format!(
                    "Saved {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                self.path = Some(path);
                self.saved = self.doc.clone();
                true
            }
            Err(e) => {
                self.status = format!("Could not save: {e}");
                false
            }
        }
    }
    fn export(&mut self, kind: &str) {
        let ext = if kind == "gif" { "gif" } else { "png" };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Export", &[ext])
            .set_file_name(format!("sprite.{ext}"))
            .save_file()
        {
            let result = if kind == "gif" {
                formats::export_gif(&path, &self.doc)
            } else {
                formats::export_png(&path, &self.doc, self.frame, kind == "sheet")
            };
            self.status = match result {
                Ok(()) => format!(
                    "Exported {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
                Err(e) => format!("Export failed: {e}"),
            };
        }
    }
    fn copy(&mut self) {
        let (x0, y0, x1, y1) =
            self.selection
                .unwrap_or((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1));
        let w = (x1 - x0 + 1) as usize;
        let h = (y1 - y0 + 1) as usize;
        let mut pixels = vec![];
        for y in y0..=y1 {
            for x in x0..=x1 {
                pixels.push(
                    self.doc.layers[self.layer].cels[self.frame]
                        [y as usize * self.doc.width + x as usize],
                );
            }
        }
        self.clipboard = Some(Clipboard { w, h, pixels });
        self.status = "Copied pixels from the active layer".into();
    }
    fn paste(&mut self) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            return;
        }
        if let Some(c) = self.clipboard.clone() {
            self.remember();
            let (x, y, _, _) = self.selection.unwrap_or((0, 0, 0, 0));
            for cy in 0..c.h {
                for cx in 0..c.w {
                    if let Some(i) = self.doc.index(x + cx as i32, y + cy as i32) {
                        let p = c.pixels[cy * c.w + cx];
                        let color = over(self.doc.layers[self.layer].cels[self.frame][i], p, 255);
                        self.doc.set_pixel(self.layer, self.frame, i, color);
                    }
                }
            }
            self.status = "Pasted pixels".into();
        }
    }
    fn clear(&mut self) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            return;
        }
        self.remember();
        let (x0, y0, x1, y1) =
            self.selection
                .unwrap_or((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(i) = self.doc.index(x, y) {
                    self.doc.set_pixel(self.layer, self.frame, i, CLEAR);
                }
            }
        }
    }
    fn action(&mut self, s: &str) {
        self.cancel_gesture();
        match s {
            "new" => self.request(Pending::New),
            "closetab" => self.request(Pending::CloseTab),
            "open" => self.request(Pending::Open),
            "save" => {
                self.save(false);
            }
            "saveas" => {
                self.save(true);
            }
            "png" | "sheet" | "gif" => self.export(s),
            "undo" => self.undo(),
            "redo" => self.redo(),
            "copy" => self.copy(),
            "paste" => self.paste(),
            "cut" => {
                self.copy();
                self.clear()
            }
            "clear" => self.clear(),
            "selectall" => {
                self.selection = Some((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1))
            }
            "deselect" => self.selection = None,
            "layer" => {
                if self.doc.layers.len() < u16::MAX as usize
                    && self
                        .doc
                        .can_grow(self.doc.width * self.doc.height * self.doc.durations.len())
                {
                    self.remember();
                    self.doc.add_layer();
                    self.layer = self.doc.layers.len() - 1;
                } else {
                    self.status = "Layer limit reached for this canvas size".into()
                }
            }
            "deletelayer" => {
                if self.doc.layers.len() > 1 {
                    self.remember();
                    self.doc.remove_layer(self.layer);
                    self.clamp();
                }
            }
            "up" | "down" => {
                let to = if s == "up" {
                    (self.layer + 1).min(self.doc.layers.len() - 1)
                } else {
                    self.layer.saturating_sub(1)
                };
                if to != self.layer {
                    self.remember();
                    self.doc.swap_layers(self.layer, to);
                    self.layer = to
                }
            }
            "frame" | "blank" => {
                if self.doc.durations.len() < u16::MAX as usize
                    && self
                        .doc
                        .can_grow(self.doc.width * self.doc.height * self.doc.layers.len())
                {
                    self.remember();
                    self.doc.add_frame(self.frame, s == "frame");
                    self.frame += 1;
                } else {
                    self.status = "Frame limit reached for this canvas size".into()
                }
            }
            "deleteframe" => {
                if self.doc.durations.len() > 1 {
                    self.remember();
                    self.doc.remove_frame(self.frame);
                    self.clamp();
                }
            }
            "play" => {
                self.playing = !self.playing;
                self.last_tick = Instant::now();
            }
            "prev" => {
                self.frame = (self.frame + self.doc.durations.len() - 1) % self.doc.durations.len()
            }
            "next" => self.frame = (self.frame + 1) % self.doc.durations.len(),
            "flipx" | "flipy" => {
                self.remember();
                self.doc.flip(self.layer, self.frame, s == "flipx")
            }
            "grid" => self.grid = !self.grid,
            "onion" => self.onion = !self.onion,
            "fit" => {
                self.zoom = (480. / self.doc.width.max(self.doc.height) as f32)
                    .floor()
                    .clamp(0.02, 32.);
                self.pan = Vec2::ZERO;
            }
            "properties" => self.properties_open = true,
            "pencil" => self.tool = Tool::Pencil,
            "eraser" => self.tool = Tool::Eraser,
            "fill" => self.tool = Tool::Fill,
            "picker" => self.tool = Tool::Picker,
            "line" => self.tool = Tool::Line,
            "rectangle" => self.tool = Tool::Rect,
            "ellipse" => self.tool = Tool::Ellipse,
            "selection" => self.tool = Tool::Select,
            "hand" => self.tool = Tool::Hand,
            "help" => self.help = true,
            "extensions" => {
                self.refresh_extensions();
                self.extensions_open = true;
            }
            "demo" => self.request(Pending::Demo),
            _ => {}
        }
    }
    fn import_extension_result(&mut self) -> Result<(), String> {
        let session = self
            .extension_session
            .as_ref()
            .ok_or("No script result is available")?;
        let doc = session.load_result()?;
        if doc != self.doc {
            self.remember();
            self.doc = doc;
            self.clamp();
        }
        self.status = "Imported extension changes. Undo restores your previous sprite.".into();
        Ok(())
    }
    fn refresh_extensions(&mut self) {
        if let Some(manager) = &self.extension_manager {
            match manager.commands() {
                Ok(commands) => {
                    self.plugin_commands = commands;
                    self.extension_error.clear();
                }
                Err(e) => self.extension_error = e,
            }
        }
    }
    fn start_script(&mut self, script: PathBuf, command: Option<String>) -> Result<(), String> {
        let session = Session::new(&self.doc)?;
        let (tx, rx) = std::sync::mpsc::channel();
        let work = session.clone();
        let doc = self.doc.clone();
        let (frame, layer) = (self.frame, self.layer);
        let colors = [self.fg, self.bg];
        let (ui_tx, ui_rx) = std::sync::mpsc::channel::<script_ui::Request>();
        self.script_ui.connect(ui_rx);
        self.script_preview_doc = Some(self.doc.clone());
        let preferences_path = extensions::config_directory().join("lua-preferences.json");
        let saved_preferences: serde_json::Value = std::fs::read(&preferences_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(serde_json::json!({}));
        let selection=self.selection.map(|(x0,y0,x1,y1)|serde_json::json!({"x":x0.min(x1),"y":y0.min(y1),"width":(x1-x0).abs()+1,"height":(y1-y0).abs()+1}));
        let options = serde_json::json!({"selection":selection,"filename":self.path.as_ref().map(|p|p.to_string_lossy()),"zoom":self.zoom,"preferences":saved_preferences["preferences"],"pluginPreferences":saved_preferences[script.to_string_lossy().as_ref()],"configPath":extensions::config_directory(),"projectPath":script.parent()});
        std::thread::spawn(move || {
            let result = std::fs::read_to_string(&script)
                .map_err(|e| e.to_string())
                .and_then(|source| {
                    let base = script
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf();
                    let mut preferences = saved_preferences;
                    extensions::run_source_with_host(
                        &doc,
                        &source,
                        frame,
                        layer,
                        command.as_deref(),
                        Some(colors),
                        options,
                        Box::new(move |request| {
                            let op = request["op"].as_str().unwrap_or("");
                            if matches!(
                                op,
                                "open"
                                    | "save"
                                    | "isFile"
                                    | "isDirectory"
                                    | "listFiles"
                                    | "makeDirectory"
                                    | "close"
                                    | "readScript"
                            ) {
                                return extensions::script_file_request(&base, &request);
                            }
                            if op == "preferences" {
                                preferences["preferences"] = request["preferences"].clone();
                                preferences[script.to_string_lossy().as_ref()] =
                                    request["pluginPreferences"].clone();
                                std::fs::create_dir_all(preferences_path.parent().unwrap())
                                    .map_err(|e| e.to_string())?;
                                std::fs::write(
                                    &preferences_path,
                                    serde_json::to_vec(&preferences).map_err(|e| e.to_string())?,
                                )
                                .map_err(|e| e.to_string())?;
                            }
                            let (reply, rx) = std::sync::mpsc::channel();
                            ui_tx
                                .send(script_ui::Request {
                                    value: request,
                                    reply,
                                })
                                .map_err(|_| "Editor closed".to_string())?;
                            rx.recv().map_err(|_| "Editor closed".to_string())
                        }),
                    )
                })
                .and_then(|d| formats::save(&work.result_path(), &d));
            let _ = tx.send(result);
        });
        self.extension_session = Some(session);
        self.script_job = Some(rx);
        self.playing = false;
        self.status = "Running embedded Lua…".into();
        Ok(())
    }
    fn extension_action(&mut self, action: &str) -> Result<(), String> {
        match action {
            "run-lua" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Lua script", &["lua"])
                    .pick_file()
                {
                    self.start_script(path, None)?;
                }
            }
            "install-extension" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Extension package or Lua",
                        &["aseprite-extension", "zip", "lua"],
                    )
                    .pick_file()
                {
                    let id = self
                        .extension_manager
                        .as_ref()
                        .ok_or("Extension storage is unavailable")?
                        .install(&path)?;
                    self.status = format!("Installed {id}");
                    self.refresh_extensions();
                }
            }
            "install-folder" => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose extension folder containing package.json")
                    .pick_folder()
                {
                    let id = self
                        .extension_manager
                        .as_ref()
                        .ok_or("Extension storage is unavailable")?
                        .install(&path)?;
                    self.status = format!("Installed {id}");
                    self.refresh_extensions();
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn poll_script(&mut self) {
        let result = self.script_job.as_ref().map(|rx| rx.try_recv());
        match result {
            Some(Ok(result)) => {
                self.script_job = None;
                self.script_ui.disconnect();
                if let Some(session) = &self.extension_session {
                    if let Ok(original) = formats::load(&session.input_path()) {
                        self.doc = original;
                    }
                }
                let result = result.and_then(|()| self.import_extension_result());
                if let Err(e) = result {
                    self.status = format!("Extension failed: {e}");
                }
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.script_job = None;
                self.script_ui.disconnect();
                if let Some(session) = &self.extension_session {
                    if let Ok(original) = formats::load(&session.input_path()) {
                        self.doc = original;
                    }
                }
                self.status =
                    "Extension failed: the Lua worker stopped. Your sprite is unchanged.".into();
            }
            _ => {}
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut action = None;
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            for (k, a) in [
                (Key::N, "new"),
                (Key::O, "open"),
                (Key::W, "closetab"),
                (Key::S, if shift { "saveas" } else { "save" }),
                (Key::Z, if shift { "redo" } else { "undo" }),
                (Key::Y, "redo"),
                (Key::C, "copy"),
                (Key::V, "paste"),
                (Key::X, "cut"),
                (Key::A, "selectall"),
            ] {
                if cmd && i.key_pressed(k) {
                    action = Some(a);
                }
            }
            if !cmd {
                for (k, t) in [
                    (Key::B, Tool::Pencil),
                    (Key::E, Tool::Eraser),
                    (Key::G, Tool::Fill),
                    (Key::I, Tool::Picker),
                    (Key::L, Tool::Line),
                    (Key::U, Tool::Rect),
                    (Key::O, Tool::Ellipse),
                    (Key::M, Tool::Select),
                    (Key::H, Tool::Hand),
                ] {
                    if i.key_pressed(k) {
                        self.cancel_gesture();
                        self.tool = t
                    }
                }
                if i.key_pressed(Key::Space) {
                    action = Some("play")
                }
                if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                    action = Some("clear")
                }
                if i.key_pressed(Key::Escape) {
                    action = Some("deselect")
                }
                if i.key_pressed(Key::Comma) {
                    action = Some("prev")
                }
                if i.key_pressed(Key::Period) {
                    action = Some("next")
                }
                if i.key_pressed(Key::X) {
                    std::mem::swap(&mut self.fg, &mut self.bg);
                    self.set_active_color(self.fg);
                }
            }
        });
        let custom = self.custom_shortcuts.iter().find_map(|(binding, action)| {
            let parts: Vec<_> = binding.split('+').collect();
            let key = Key::from_name(parts.last().copied().unwrap_or(""))?;
            let cmd = parts
                .iter()
                .any(|s| s.eq_ignore_ascii_case("Ctrl") || s.eq_ignore_ascii_case("Cmd"));
            let shift = parts.iter().any(|s| s.eq_ignore_ascii_case("Shift"));
            let alt = parts.iter().any(|s| s.eq_ignore_ascii_case("Alt"));
            ctx.input(|i| {
                (i.key_pressed(key)
                    && i.modifiers.command == cmd
                    && i.modifiers.shift == shift
                    && i.modifiers.alt == alt)
                    .then(|| action.clone())
            })
        });
        if let Some(custom) = custom {
            self.action(&custom);
        } else if let Some(a) = action {
            self.action(a);
        }
    }
    fn menu(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        egui::MenuBar::new().ui(ui, |ui| {
            for (title, items) in [
                (
                    "File",
                    vec![
                        ("New sprite…   Ctrl+N", "new"),
                        ("Open…   Ctrl+O", "open"),
                        ("Close sprite   Ctrl+W", "closetab"),
                        ("Save   Ctrl+S", "save"),
                        ("Save as…", "saveas"),
                        ("Export PNG…", "png"),
                        ("Export sprite sheet…", "sheet"),
                        ("Export animation GIF…", "gif"),
                        ("Open sample garden", "demo"),
                    ],
                ),
                (
                    "Edit",
                    vec![
                        ("Undo   Ctrl+Z", "undo"),
                        ("Redo   Ctrl+Y", "redo"),
                        ("Copy   Ctrl+C", "copy"),
                        ("Cut   Ctrl+X", "cut"),
                        ("Paste   Ctrl+V", "paste"),
                        ("Clear   Delete", "clear"),
                        ("Select all   Ctrl+A", "selectall"),
                        ("Deselect   Esc", "deselect"),
                    ],
                ),
                (
                    "Sprite",
                    vec![
                        ("Properties, layers, tiles & ranges…", "properties"),
                        ("Flip active cel horizontally", "flipx"),
                        ("Flip active cel vertically", "flipy"),
                    ],
                ),
                (
                    "Layer",
                    vec![
                        ("New layer", "layer"),
                        ("Delete layer", "deletelayer"),
                        ("Move layer up", "up"),
                        ("Move layer down", "down"),
                    ],
                ),
                (
                    "Frame",
                    vec![
                        ("Duplicate frame", "frame"),
                        ("New empty frame", "blank"),
                        ("Delete frame", "deleteframe"),
                        ("Play / stop   Space", "play"),
                        ("Previous frame   ,", "prev"),
                        ("Next frame   .", "next"),
                    ],
                ),
                (
                    "View",
                    vec![
                        ("Toggle pixel grid", "grid"),
                        ("Toggle onion skin", "onion"),
                        ("Fit canvas", "fit"),
                    ],
                ),
                ("Extensions", vec![("Extensions & scripts…", "extensions")]),
                ("Help", vec![("Tools and file support", "help")]),
            ] {
                let translated = self
                    .translations
                    .get(&format!("menu.{}", title.to_lowercase()))
                    .cloned()
                    .unwrap_or_else(|| title.to_string());
                ui.menu_button(translated, |ui| {
                    if title == "View" {
                        let mut dark = self.dark_mode;
                        if ui.checkbox(&mut dark, "Dark mode").changed() {
                            self.set_dark_mode(ui.ctx(), dark);
                            self.save_theme();
                        }
                        ui.separator();
                    }
                    for (label, a) in items {
                        if ui.button(label).clicked() {
                            action = Some(a);
                            ui.close();
                        }
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("PixelCraft").strong());
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.), Sense::hover());
                for (y, row) in BRAND_MARK.iter().enumerate() {
                    for (x, cell) in row.bytes().enumerate() {
                        if cell != b'.' {
                            let color = if cell == b'g' {
                                Color32::from_rgb(241, 190, 93)
                            } else {
                                Color32::from_rgb(86, 201, 184)
                            };
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    rect.min + Vec2::new(x as f32 * 3., y as f32 * 3.),
                                    Vec2::splat(3.),
                                ),
                                0.,
                                color,
                            );
                        }
                    }
                }
            });
        });
        if let Some(a) = action {
            self.action(a)
        }
    }
    fn set_active_color(&mut self, color: Pixel) {
        self.fg = color;
        self.named_colors[self.active_color].1 = color;
        if let Some(index) = self
            .doc
            .palette_for(self.frame)
            .iter()
            .position(|p| *p == color)
        {
            self.palette_index = index;
        }
    }
    fn select_named_color(&mut self, index: usize) {
        self.active_color = index;
        self.set_active_color(self.named_colors[index].1);
    }
    fn add_named_color(&mut self) {
        self.named_colors
            .push((format!("Color {}", self.named_colors.len() + 1), self.fg));
        self.active_color = self.named_colors.len() - 1;
    }
    fn sprite_colors(&mut self) -> Vec<Pixel> {
        if self.used_colors_snapshot.as_ref() != Some(&self.doc) {
            let mut colors = std::collections::BTreeSet::new();
            for layer in &self.doc.layers {
                for cel in &layer.cels {
                    colors.extend(cel.iter().copied().filter(|p| p[3] != 0));
                }
                if layer.tileset.is_some() {
                    for frame in 0..self.doc.durations.len() {
                        colors.extend(self.doc.composite(frame).into_iter().filter(|p| p[3] != 0));
                    }
                }
            }
            self.used_colors = colors.into_iter().collect();
            self.used_colors_snapshot = Some(self.doc.clone());
        }
        self.used_colors.clone()
    }
    fn drawing_palette_index(&self, color: Pixel) -> Option<usize> {
        let palette = self.doc.palette_for(self.frame);
        if palette.get(self.palette_index) == Some(&color) {
            Some(self.palette_index)
        } else {
            palette.iter().position(|p| *p == color)
        }
    }
    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.);
        ui.strong("Colors");
        ui.add_space(4.);
        let mut selected = self.active_color;
        egui::ComboBox::from_id_salt("named_colors")
            .width(150.)
            .selected_text(&self.named_colors[selected].0)
            .show_ui(ui, |ui| {
                for (index, (name, _)) in self.named_colors.iter().enumerate() {
                    ui.selectable_value(&mut selected, index, name);
                }
            });
        if selected != self.active_color {
            self.select_named_color(selected);
        }
        ui.horizontal(|ui| {
            if ui
                .color_edit_button_srgba_unmultiplied(&mut self.fg)
                .changed()
            {
                self.set_active_color(self.fg);
            }
            ui.monospace(format!(
                "#{:02X}{:02X}{:02X}",
                self.fg[0], self.fg[1], self.fg[2]
            ));
        });
        if ui.button("+ Add Color").clicked() {
            self.add_named_color();
        }
        ui.separator();
        ui.label("Colors used in this sprite");
        let colors = self.sprite_colors();
        if colors.is_empty() {
            ui.small("Draw to add colors here.");
        }
        egui::ScrollArea::vertical()
            .id_salt("sprite_used_colors")
            .max_height(180.)
            .show(ui, |ui| {
                egui::Grid::new("sprite_colors")
                    .min_col_width(0.)
                    .spacing(Vec2::splat(3.))
                    .show(ui, |ui| {
                        for (i, p) in colors.iter().enumerate() {
                            let (r, response) =
                                ui.allocate_exact_size(Vec2::splat(27.), Sense::click());
                            ui.painter().rect_filled(
                                r,
                                0,
                                Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]),
                            );
                            if *p == self.fg {
                                ui.painter().rect_stroke(
                                    r.shrink(1.),
                                    0,
                                    Stroke::new(2., self.colors().ink),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if response.clicked() {
                                self.set_active_color(*p);
                            }
                            response.on_hover_text(format!(
                                "Use #{:02X}{:02X}{:02X} (alpha {})",
                                p[0], p[1], p[2], p[3]
                            ));
                            if i % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
            });
        ui.small(format!("{} used colors", colors.len()));
        if ui.button("Properties & color mode…").clicked() {
            self.properties_open = true;
        }
    }
    fn sprite_preview(&mut self, ui: &mut egui::Ui) {
        ui.strong("Preview");
        self.preview = Some(self.frame_texture(ui.ctx(), self.frame));
        let (r, _) = ui.allocate_exact_size(Vec2::new(120., 120.), Sense::hover());
        ui.painter().rect_filled(r, 0, self.colors().canvas);
        let s = (100. / self.doc.width.max(self.doc.height) as f32).min(3.);
        let ir = Rect::from_center_size(
            r.center(),
            Vec2::new(self.doc.width as f32 * s, self.doc.height as f32 * s),
        );
        if let Some(t) = &self.preview {
            ui.painter().image(
                t.id(),
                ir,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        ui.add_space(8.);
        ui.label(
            RichText::new("Left click: active color\nScroll: zoom · Middle drag: pan").size(11.),
        );
    }
    fn tools(&mut self, ui: &mut egui::Ui) {
        for t in [
            Tool::Pencil,
            Tool::Eraser,
            Tool::Fill,
            Tool::Picker,
            Tool::Line,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Select,
            Tool::Hand,
        ] {
            let (r, response) = ui.allocate_exact_size(Vec2::splat(34.), Sense::click());
            let fill = if self.tool == t {
                self.colors().accent
            } else if response.hovered() {
                self.colors().hover
            } else {
                self.colors().face
            };
            ui.painter().rect_filled(r, 2, fill);
            ui.painter().rect_stroke(
                r,
                2,
                Stroke::new(1., self.colors().border),
                egui::StrokeKind::Inside,
            );
            tool_icon(ui.painter(), r.shrink(8.), t, self.colors());
            if response.clicked() {
                self.cancel_gesture();
                self.tool = t
            }
            response.on_hover_text(format!("{} ({})", t.name(), t.key()));
        }
    }
    fn timeline(&mut self, ui: &mut egui::Ui) {
        let textures: Vec<_> = (0..self.doc.durations.len().min(256))
            .map(|f| self.frame_texture(ui.ctx(), f))
            .collect();
        let mut a = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if self.playing { "■ Stop" } else { "▶ Play" })
                .clicked()
            {
                a = Some("play")
            }
            if ui.small_button("◀").clicked() {
                a = Some("prev")
            }
            if ui.small_button("▶").clicked() {
                a = Some("next")
            }
            ui.separator();
            ui.label(format!(
                "Frame {} / {}",
                self.frame + 1,
                self.doc.durations.len()
            ));
            let mut ms = self.doc.durations[self.frame];
            if ui
                .add(egui::DragValue::new(&mut ms).range(1..=65535).suffix(" ms"))
                .changed()
            {
                self.remember();
                self.doc.durations[self.frame] = ms;
            }
            ui.separator();
            if ui
                .button("+ Frame")
                .on_hover_text("Duplicate active frame")
                .clicked()
            {
                a = Some("frame")
            }
            if ui.button("+ Layer").clicked() {
                a = Some("layer")
            }
            ui.checkbox(&mut self.onion, "Onion skin");
            if ui.button("Properties & ranges…").clicked() {
                self.properties_open = true;
            }
        });
        ui.separator();
        let mut toggle = None;
        let mut lock = None;
        let mut opacity = None;
        let mut rename = None;
        egui::ScrollArea::both()
            .id_salt("timeline_scroll")
            .max_height(120.)
            .show(ui, |ui| {
                egui::Grid::new("cels")
                    .spacing(Vec2::new(3., 3.))
                    .min_row_height(25.)
                    .show(ui, |ui| {
                        ui.allocate_ui(Vec2::new(180., 24.), |ui| {
                            ui.strong("Layers");
                        });
                        for f in 0..self.doc.durations.len() {
                            if ui
                                .add_sized(
                                    [54., 28.],
                                    if let Some(t) = textures.get(f) {
                                        egui::Button::image_and_text(
                                            egui::Image::new((t.id(), Vec2::new(24., 20.))),
                                            (f + 1).to_string(),
                                        )
                                        .selected(f == self.frame)
                                    } else {
                                        egui::Button::new((f + 1).to_string())
                                            .selected(f == self.frame)
                                    },
                                )
                                .clicked()
                            {
                                self.frame = f;
                                self.playing = false;
                            }
                        }
                        ui.end_row();
                        for li in (0..self.doc.layers.len()).rev() {
                            let l = &self.doc.layers[li];
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button(if l.visible { "V" } else { "-" })
                                    .on_hover_text("Show / hide layer")
                                    .clicked()
                                {
                                    toggle = Some(li)
                                }
                                if ui
                                    .small_button(if l.locked { "L" } else { "+" })
                                    .on_hover_text("Lock / unlock layer")
                                    .clicked()
                                {
                                    lock = Some(li)
                                }
                                let response = ui.add_sized(
                                    [118., 24.],
                                    egui::Button::new(format!(
                                        "{}{}{}",
                                        if l.parent.is_some() { "  " } else { "" },
                                        if l.group {
                                            "[Group] "
                                        } else if l.tileset.is_some() {
                                            "[Tiles] "
                                        } else {
                                            ""
                                        },
                                        l.name
                                    ))
                                    .selected(li == self.layer),
                                );
                                if response.clicked() {
                                    self.layer = li
                                }
                                if response.double_clicked() {
                                    rename = Some(l.name.clone());
                                    self.layer = li;
                                }
                            });
                            for f in 0..self.doc.durations.len() {
                                let occupied = self.cel_occupied[li][f];
                                if ui
                                    .add_sized(
                                        [54., 28.],
                                        egui::Button::new(if occupied { "o" } else { "." })
                                            .selected(li == self.layer && f == self.frame),
                                    )
                                    .clicked()
                                {
                                    self.layer = li;
                                    self.frame = f;
                                    self.playing = false;
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().slider_width = 75.;
            ui.small("Layer opacity");
            let mut value = self.doc.layers[self.layer].opacity;
            if ui
                .add(egui::Slider::new(&mut value, 0..=255).show_value(false))
                .changed()
            {
                opacity = Some(value)
            }
            ui.small(format!("{}%", value as u32 * 100 / 255));
            if ui.small_button("Rename").clicked() {
                rename = Some(self.doc.layers[self.layer].name.clone())
            }
            if ui.small_button("Up").clicked() {
                a = Some("up")
            }
            if ui.small_button("Down").clicked() {
                a = Some("down")
            }
            if ui.small_button("Del layer").clicked() {
                a = Some("deletelayer")
            }
            if ui.small_button("Del frame").clicked() {
                a = Some("deleteframe")
            }
        });
        if let Some(li) = toggle {
            self.remember();
            self.doc.layers[li].visible = !self.doc.layers[li].visible
        }
        if let Some(li) = lock {
            self.remember();
            self.doc.layers[li].locked = !self.doc.layers[li].locked
        }
        if let Some(v) = opacity {
            self.remember();
            self.doc.layers[self.layer].opacity = v
        }
        if let Some(s) = rename {
            self.rename = Some(s)
        }
        if let Some(a) = a {
            self.action(a)
        }
    }
    fn shape(&mut self, a: (i32, i32), b: (i32, i32), c: Pixel) {
        self.doc.drawing_index = self.drawing_palette_index(c);
        let l = self.layer;
        let f = self.frame;
        let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        match self.tool {
            Tool::Line => self.doc.line(l, f, a, b, c, self.brush),
            Tool::Rect => {
                if self.filled {
                    for y in y0..=y1 {
                        self.doc.line(l, f, (x0, y), (x1, y), c, 1)
                    }
                } else {
                    for (a, b) in [
                        ((x0, y0), (x1, y0)),
                        ((x1, y0), (x1, y1)),
                        ((x1, y1), (x0, y1)),
                        ((x0, y1), (x0, y0)),
                    ] {
                        self.doc.line(l, f, a, b, c, self.brush)
                    }
                }
            }
            Tool::Ellipse => self.doc.ellipse(l, f, a, b, c, self.filled, self.brush),
            _ => {}
        }
        self.doc.drawing_index = None;
    }
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (r, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(r);
        painter.rect_filled(r, 0, self.colors().canvas);
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let can_edit = ui.is_enabled() && !self.dialog_open();
        if can_edit && response.hovered() {
            ctx.set_cursor_icon(if self.tool == Tool::Hand {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                let old = self.zoom;
                self.zoom = (self.zoom * (scroll * 0.003).exp()).clamp(0.02, 40.);
                if let Some(p) = pointer {
                    let offset = p - r.center() - self.pan;
                    self.pan -= offset * (self.zoom / old - 1.);
                }
            }
        }
        if can_edit
            && (response.dragged_by(egui::PointerButton::Middle)
                || (self.tool == Tool::Hand && response.dragged()))
        {
            self.pan += ctx.input(|i| i.pointer.delta());
        }
        let board = Rect::from_center_size(
            r.center() + self.pan,
            Vec2::new(
                self.doc.width as f32 * self.zoom,
                self.doc.height as f32 * self.zoom,
            ),
        );
        // Draw only visible checker squares, including at high zoom and while panning.
        let check = 8.;
        let check_size = check * self.zoom;
        let cols = (self.doc.width as f32 / check).ceil() as i32;
        let rows = (self.doc.height as f32 / check).ceil() as i32;
        painter.rect_filled(
            board,
            0,
            if self.dark_mode {
                Color32::from_rgb(82, 83, 94)
            } else {
                Color32::from_rgb(210, 202, 202)
            },
        );
        for y in (((r.min.y - board.min.y) / check_size).floor() as i32).clamp(0, rows)
            ..(((r.max.y - board.min.y) / check_size).ceil() as i32).clamp(0, rows)
        {
            for x in (((r.min.x - board.min.x) / check_size).floor() as i32).clamp(0, cols)
                ..(((r.max.x - board.min.x) / check_size).ceil() as i32).clamp(0, cols)
            {
                if (x + y) % 2 == 0 {
                    let q = Rect::from_min_size(
                        board.min + Vec2::new(x as f32 * check_size, y as f32 * check_size),
                        Vec2::splat(check_size),
                    )
                    .intersect(board);
                    painter.rect_filled(
                        q,
                        0,
                        if self.dark_mode {
                            Color32::from_rgb(64, 65, 76)
                        } else {
                            Color32::from_rgb(184, 176, 182)
                        },
                    );
                }
            }
        }
        if self.onion && self.frame > 0 && !self.playing {
            let mut pixels = self.doc.composite(self.frame);
            for (p, s) in pixels.iter_mut().zip(self.doc.composite(self.frame - 1)) {
                *p = over([150, 185, 220, s[3] / 4], *p, 255);
            }
            let pixels =
                pixelcraft::color::to_srgb(&self.doc.color_profile, &pixels).unwrap_or(pixels);
            let im = color_image(&pixels, self.doc.width, self.doc.height);
            if let Some(t) = &mut self.onion_texture {
                t.set(im, egui::TextureOptions::NEAREST);
            } else {
                self.onion_texture =
                    Some(ctx.load_texture("onion", im, egui::TextureOptions::NEAREST));
            }
            self.texture = self.onion_texture.clone();
        } else {
            self.texture = Some(self.frame_texture(&ctx, self.frame));
        }
        if let Some(t) = &self.texture {
            painter.image(
                t.id(),
                board,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        painter.rect_stroke(
            board.expand(1.),
            0,
            Stroke::new(1., self.colors().ink),
            egui::StrokeKind::Outside,
        );
        if self.grid && self.zoom >= 5. {
            for x in 1..self.doc.width {
                let px = board.min.x + x as f32 * self.zoom;
                painter.line_segment(
                    [Pos2::new(px, board.min.y), Pos2::new(px, board.max.y)],
                    Stroke::new(0.5, Color32::from_black_alpha(45)),
                );
            }
            for y in 1..self.doc.height {
                let py = board.min.y + y as f32 * self.zoom;
                painter.line_segment(
                    [Pos2::new(board.min.x, py), Pos2::new(board.max.x, py)],
                    Stroke::new(0.5, Color32::from_black_alpha(45)),
                );
            }
        }
        let pos = pointer.filter(|p| r.contains(*p)).map(|p| {
            (
                ((p.x - board.min.x) / self.zoom).floor() as i32,
                ((p.y - board.min.y) / self.zoom).floor() as i32,
            )
        });
        self.hover = pos.filter(|(x, y)| self.doc.index(*x, *y).is_some());
        let pressed = ctx.input(|i| {
            i.pointer.button_pressed(egui::PointerButton::Primary)
                || i.pointer.button_pressed(egui::PointerButton::Secondary)
        });
        let down = ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
        if can_edit
            && pressed
            && response.hovered()
            && self.hover.is_some()
            && self.tool != Tool::Hand
        {
            self.playing = false;
            let p = self.hover.unwrap();
            self.stroke_color = self.fg;
            if ctx.input(|i| i.modifiers.alt) || self.tool == Tool::Picker {
                self.set_active_color(
                    self.doc.composite(self.frame)[self.doc.index(p.0, p.1).unwrap()],
                );
            } else if self.tool == Tool::Select {
                self.start = Some(p);
                self.selection = Some((p.0, p.1, p.0, p.1));
            } else if !self.doc.editable(self.layer) {
                self.status = "This layer is locked. Unlock it in the timeline to draw.".into();
            } else {
                self.remember();
                self.start = Some(p);
                self.last = Some(p);
                self.stroke_base = Some(self.doc.clone());
                if self.tool == Tool::Fill {
                    self.doc.drawing_index = self.drawing_palette_index(self.stroke_color);
                    self.doc
                        .fill(self.layer, self.frame, p.0, p.1, self.stroke_color);
                    self.doc.drawing_index = None;
                    self.start = None;
                    self.stroke_base = None;
                } else if matches!(self.tool, Tool::Pencil | Tool::Eraser) {
                    let color = if self.tool == Tool::Eraser {
                        CLEAR
                    } else {
                        self.stroke_color
                    };
                    self.paint_at(p, color);
                }
            }
        }
        if can_edit && down {
            if let (Some(a), Some(mut b)) = (self.start, pos) {
                b.0 = b.0.clamp(0, self.doc.width as i32 - 1);
                b.1 = b.1.clamp(0, self.doc.height as i32 - 1);
                match self.tool {
                    Tool::Select => {
                        self.selection =
                            Some((a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)))
                    }
                    Tool::Pencil | Tool::Eraser => {
                        let color = if self.tool == Tool::Eraser {
                            CLEAR
                        } else {
                            self.stroke_color
                        };
                        if self.doc.layers[self.layer].tileset.is_some() {
                            self.paint_at(b, color);
                        } else {
                            self.doc.drawing_index = self.drawing_palette_index(color);
                            self.doc.line(
                                self.layer,
                                self.frame,
                                self.last.unwrap_or(a),
                                b,
                                color,
                                self.brush,
                            );
                            self.doc.drawing_index = None;
                        }
                        self.last = Some(b);
                    }
                    Tool::Line | Tool::Rect | Tool::Ellipse => {
                        if let Some(base) = &self.stroke_base {
                            self.doc = base.clone();
                        }
                        self.shape(a, b, self.stroke_color)
                    }
                    _ => {}
                }
            }
        } else {
            self.start = None;
            self.last = None;
            self.stroke_base = None;
        }
        if let Some((x0, y0, x1, y1)) = self.selection {
            let sr = Rect::from_min_max(
                board.min + Vec2::new(x0 as f32, y0 as f32) * self.zoom,
                board.min + Vec2::new((x1 + 1) as f32, (y1 + 1) as f32) * self.zoom,
            );
            painter.rect_stroke(
                sr,
                0,
                Stroke::new(2., Color32::WHITE),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                sr.expand(1.),
                0,
                Stroke::new(1., self.colors().ink),
                egui::StrokeKind::Outside,
            );
        }
        if let Some((x, y)) = self.hover {
            if matches!(self.tool, Tool::Pencil | Tool::Eraser) {
                let size = self.brush as f32 * self.zoom;
                let p = board.min
                    + Vec2::new((x - self.brush / 2) as f32, (y - self.brush / 2) as f32)
                        * self.zoom;
                painter.rect_stroke(
                    Rect::from_min_size(p, Vec2::splat(size)),
                    0,
                    Stroke::new(1., Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
            }
        }
        painter.text(
            r.min + Vec2::new(14., 12.),
            egui::Align2::LEFT_TOP,
            format!("{} × {} px   /   RGBA", self.doc.width, self.doc.height),
            egui::FontId::proportional(12.),
            Color32::from_rgb(220, 207, 216),
        );
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.status.starts_with("Could not") || self.status.starts_with("Export failed") {
            egui::Modal::new(egui::Id::new("File operation failed")).show(ctx, |ui| {
                ui.set_max_width(460.);
                ui.strong("File operation failed");
                ui.separator();
                ui.label(&self.status);
                if ui.button("Close").clicked() {
                    self.status = "Ready".into();
                }
            });
        }

        if self.pending.is_some() {
            egui::Modal::new(egui::Id::new("Save your changes?")).show(ctx, |ui| {
                ui.set_max_width(460.);
                ui.strong("Save your changes?");
                ui.separator();
                ui.label("Save the current sprite before continuing.");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() && self.save(false) {
                        let p = self.pending.take().unwrap();
                        self.finish_pending(ctx, p);
                    }
                    if ui.button("Discard changes").clicked() {
                        let p = self.pending.take().unwrap();
                        self.saved = self.doc.clone();
                        self.finish_pending(ctx, p);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending = None;
                    }
                });
            });
        }
        if self.new_dialog {
            egui::Modal::new(egui::Id::new("New sprite")).show(ctx, |ui| {
                ui.set_max_width(460.);
                ui.strong("New sprite");
                ui.separator();
                ui.label("Start with a transparent canvas.");
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.new_w)
                            .range(1..=MAX_DIMENSION)
                            .suffix(" px"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut self.new_h)
                            .range(1..=MAX_DIMENSION)
                            .suffix(" px"),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Create sprite").clicked() {
                        if self.new_w.saturating_mul(self.new_h) <= MAX_PIXELS {
                            self.open_document(Document::new(self.new_w, self.new_h), None);
                        } else {
                            self.status =
                                "Could not create: canvas exceeds 64 million pixels".into();
                        }
                        self.new_dialog = false;
                        self.status = "New transparent sprite".into();
                    }
                    if ui.button("Cancel").clicked() {
                        self.new_dialog = false;
                    }
                });
            });
        }
        if let Some(mut name) = self.rename.clone() {
            let mut done = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("Rename layer")).show(ctx, |ui| {
                ui.set_max_width(460.);
                ui.strong("Rename layer");
                ui.separator();
                ui.add(egui::TextEdit::singleline(&mut name).char_limit(80));
                ui.horizontal(|ui| {
                    done = ui.button("Rename").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
            if done {
                if !name.trim().is_empty() {
                    self.remember();
                    self.doc.layers[self.layer].name = name;
                }
                self.rename = None;
            } else if cancel {
                self.rename = None
            } else {
                self.rename = Some(name)
            }
        }
        self.properties_dialog(ctx);
        if self.extensions_open {
            let mut open = true;
            let mut action = None;
            let mut run = None;
            let mut change = None;
            let mut asset = None;
            egui::Window::new("PixelCraft extensions").open(&mut open).default_width(560.).show(ctx,|ui|{
                ui.label("Install plugins, themes, palettes and shortcuts for PixelCraft.");
                ui.collapsing("Aseprite compatibility",|ui|{egui::ScrollArea::vertical().max_height(260.).show(ui,|ui|{ui.label(include_str!("../COMPATIBILITY.md"));});});
                ui.small("Lua runs inside PixelCraft. Supported API: app, Sprite, Layer, Cel, Image, Color, palettes, tags and plugin commands. Dialogs, callbacks, events and timers run in the editor. See COMPATIBILITY.md for supported APIs and limitations.");
                ui.horizontal_wrapped(|ui|{for(label,id)in [("Run .lua…","run-lua"),("Install package…","install-extension"),("Install folder…","install-folder")]{if ui.add_enabled(self.script_job.is_none(),egui::Button::new(label)).clicked(){action=Some(id);}}});
                ui.separator();
                if let Some(manager)=&self.extension_manager {
                    ui.small(format!("Storage: {}",manager.root.display()));
                    match manager.list(){Ok(packages)=>{if packages.is_empty(){ui.label("No extensions installed yet.");}for p in packages{ui.horizontal(|ui|{let mut enabled=p.enabled;if ui.checkbox(&mut enabled,if p.manifest.display_name.is_empty(){&p.id}else{&p.manifest.display_name}).changed(){change=Some((p.id.clone(),Some(enabled)));}if ui.small_button("Uninstall").clicked(){change=Some((p.id.clone(),None));}});if p.enabled{for a in &p.manifest.contributes.palettes{if ui.button(format!("Use palette: {}",a.path)).clicked(){asset=Some(("palette",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.themes{if ui.button(format!("Use theme: {}",a.path)).clicked(){asset=Some(("theme",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.languages{if ui.button(format!("Use language: {}",a.id)).clicked(){asset=Some(("language",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.dithering_matrices{if ui.button(format!("Use dithering: {}",a.id)).clicked(){asset=Some(("dither",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.shortcuts{if ui.button(format!("Use shortcuts: {}",a.path)).clicked(){asset=Some(("shortcuts",p.id.clone(),a.clone()));}}}}},Err(e)=>{ui.label(e);}}
                }else{ui.label("Extension storage could not be created.");}
                ui.separator();ui.strong("Plugin commands");
                for c in &self.plugin_commands{if ui.add_enabled(self.script_job.is_none(),egui::Button::new(format!("{} · {}",c.package,c.title))).clicked(){run=Some(c.clone());}}
                if !self.extension_error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&self.extension_error);}
                if self.script_job.is_some(){ui.spinner();ctx.request_repaint_after(Duration::from_millis(100));}ui.label(&self.status);
            });
            self.extensions_open = open;
            if let Some((id, enabled)) = change {
                let result = if let Some(manager) = &self.extension_manager {
                    if let Some(enabled) = enabled {
                        manager.set_enabled(&id, enabled)
                    } else {
                        manager.uninstall(&id)
                    }
                } else {
                    Err("Storage unavailable".into())
                };
                if let Err(e) = result {
                    self.status = e;
                }
                self.refresh_extensions();
            }
            if let Some((kind, id, contribution)) = asset {
                let result = self.apply_extension_asset(ctx, kind, &id, &contribution);
                if let Err(e) = result {
                    self.status = format!("Extension failed: {e}");
                }
            }
            if let Some(c) = run {
                if let Err(e) =
                    self.start_script(c.script, if c.id.is_empty() { None } else { Some(c.id) })
                {
                    self.status = e;
                }
            }
            if let Some(action) = action {
                self.cancel_gesture();
                if let Err(e) = self.extension_action(action) {
                    self.status = format!("Extension failed: {e}");
                }
            }
        }
        if self.help {
            egui::Window::new("PixelCraft — help").open(&mut self.help).default_width(440.).show(ctx,|ui|{ui.heading("PixelCraft");ui.label("Small pixels. Endless possibilities.");ui.separator();ui.strong("Draw, layer, animate");ui.label("B Pencil · E Eraser · G Fill · I Eyedropper\nL Line · U Rectangle · O Ellipse · M Selection · H Hand\nX Swap colors · Alt-click Pick color\nScroll Zoom · Middle drag Pan · Space Play\nCtrl+Z Undo · Ctrl+Y Redo · Ctrl+S Save");ui.separator();ui.label("Selections support copy, cut, paste and delete on the active layer. Paste starts at the selection's top-left, or the canvas origin.");ui.separator();ui.strong("File support");ui.label("Open PixelCraft projects, PNG and ASE sprites. Save .pixel projects or ASE sprites. Export PNG, sprite sheets and GIF animations.");ui.label("Sprite → Properties opens color modes, groups, blend modes, cel opacity, tile editing and named animation ranges. ASE profiles, metadata and off-canvas image pixels are retained. Embedded ICC profiles are converted to sRGB for display; source pixels are retained.");ui.label("PixelCraft is standalone. Extensions install into its own data directory. Lua, JSON themes, palettes and shortcuts run here through the documented supported native API.");});
        }
    }
}
impl PixelCraft {
    fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.script_ui.modeless
            && self
                .script_preview_doc
                .as_ref()
                .is_some_and(|d| d != &self.doc)
        {
            self.script_ui.editor_update = Some(
                serde_json::json!({"document":self.doc,"frame":self.frame+1,"layer":self.layer+1}),
            );
            self.script_preview_doc = Some(self.doc.clone());
        }
        self.script_ui.show(&ctx);
        if let Some(request) = self.script_ui.preview.take() {
            if let Ok(document) = serde_json::from_value::<Document>(request["document"].clone()) {
                if document.validate().is_ok() && self.script_ui.editor_update.is_none() {
                    self.doc = document;
                    self.script_preview_doc = Some(self.doc.clone());
                }
            }
        }
        if let Some(context) = self.script_ui.final_context.take() {
            let s = &context["selection"];
            self.selection = match (
                s["x"].as_i64(),
                s["y"].as_i64(),
                s["width"].as_i64(),
                s["height"].as_i64(),
            ) {
                (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => {
                    Some((x as i32, y as i32, (x + w - 1) as i32, (y + h - 1) as i32))
                }
                _ => None,
            };
            if let Ok(colors) = serde_json::from_value::<[Pixel; 2]>(context["colors"].clone()) {
                self.fg = colors[0];
                self.bg = colors[1];
                self.set_active_color(self.fg);
            }
            self.frame = context["frame"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
            self.layer = context["layer"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
        }
        self.poll_script();
        if self.script_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.dirty() {
            if let Some(i) = self
                .tabs
                .iter()
                .position(|t| t.as_ref().is_some_and(|t| t.doc != t.saved))
            {
                self.switch_tab(i);
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if !self.dialog_open() {
            self.shortcuts(&ctx)
        }
        if self.playing {
            let ms = self.doc.durations[self.frame] as u64;
            if self.last_tick.elapsed() >= Duration::from_millis(ms) {
                self.advance_playback();
                self.last_tick = Instant::now();
            }
            ctx.request_repaint_after(Duration::from_millis(10));
        }
        if self.dialog_open() {
            ui.disable();
        }
        egui::Panel::top("menu")
            .exact_size(34.)
            .show(ui, |ui| self.menu(ui));
        egui::Panel::top("document_tab")
            .exact_size(35.)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut selected = None;
                        let mut close = None;
                        for i in 0..self.tabs.len() {
                            let (path, dirty) = if i == self.active_tab {
                                (&self.path, self.dirty())
                            } else {
                                let tab = self.tabs[i].as_ref().unwrap();
                                (&tab.path, tab.doc != tab.saved)
                            };
                            let name = path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .and_then(|p| p.to_str())
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("Untitled {}", i + 1));
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(
                                            i == self.active_tab,
                                            format!("{name}{}", if dirty { " *" } else { "" }),
                                        )
                                        .clicked()
                                    {
                                        selected = Some(i);
                                    }
                                    if ui.small_button("×").on_hover_text("Close sprite").clicked()
                                    {
                                        close = Some(i);
                                    }
                                });
                            });
                        }
                        if let Some(i) = selected {
                            self.switch_tab(i);
                        }
                        if let Some(i) = close {
                            self.switch_tab(i);
                            self.request(Pending::CloseTab);
                        }
                        if ui.button("+ New").clicked() {
                            self.request(Pending::New);
                        }
                        if ui.button("Open…").clicked() {
                            self.request(Pending::Open);
                        }
                    });
                });
            });
        egui::Panel::top("context").exact_size(38.).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(self.tool.name());
                ui.separator();
                ui.label("Brush");
                ui.add(
                    egui::DragValue::new(&mut self.brush)
                        .range(MIN_BRUSH_SIZE..=MAX_BRUSH_SIZE)
                        .suffix(" px"),
                );
                let mut shape = self.doc.brush_shape;
                egui::ComboBox::from_id_salt("brush_shape")
                    .width(84.)
                    .selected_text(format!("{:?}", shape))
                    .show_ui(ui, |ui| {
                        for v in [
                            BrushShape::Round,
                            BrushShape::Square,
                            BrushShape::Diamond,
                            BrushShape::Horizontal,
                            BrushShape::Vertical,
                        ] {
                            ui.selectable_value(&mut shape, v, format!("{:?}", v));
                        }
                    });
                if shape != self.doc.brush_shape {
                    self.remember();
                    self.doc.brush_shape = shape;
                }
                if matches!(self.tool, Tool::Rect | Tool::Ellipse) {
                    ui.checkbox(&mut self.filled, "Filled");
                }
                ui.checkbox(&mut self.grid, "Pixel grid");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Fit").clicked() {
                        self.action("fit")
                    }
                    ui.add(
                        egui::Slider::new(&mut self.zoom, 0.02..=32.0)
                            .show_value(false)
                            .logarithmic(true),
                    );
                    ui.label(format!("{:.0}%", self.zoom * 100.));
                });
            });
        });
        egui::Panel::bottom("status")
            .exact_size(28.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(12.))
                        .on_hover_text(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some((x, y)) = self.hover {
                            ui.monospace(format!("{x}, {y}"));
                        }
                        ui.small(format!("{} × {}", self.doc.width, self.doc.height));
                    });
                });
            });
        egui::Panel::left("palette_panel")
            .exact_size(174.)
            .resizable(false)
            .show(ui, |ui| {
                egui::Panel::bottom("sidebar_preview")
                    .exact_size(200.)
                    .resizable(false)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.sprite_preview(ui));
                egui::ScrollArea::vertical().show(ui, |ui| self.palette(ui));
            });
        egui::Panel::right("tool_panel")
            .exact_size(48.)
            .resizable(false)
            .show(ui, |ui| self.tools(ui));
        egui::Panel::bottom("timeline")
            .default_size(216.)
            .size_range(160.0..=380.0)
            .resizable(true)
            .show(ui, |ui| self.timeline(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.colors().canvas))
            .show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
        if let Some(path) = &self.shot {
            self.shot_frames += 1;
            ctx.request_repaint();
            if self.shot_frames >= 120 && !self.shot_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.shot_requested = true;
            }
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(im) = screenshot {
                let bytes: Vec<u8> = im.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match image::save_buffer(
                    path,
                    &bytes,
                    im.width() as u32,
                    im.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    Ok(()) => {
                        println!("Screenshot saved: {}", path.display());
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                    }
                    Err(e) => {
                        eprintln!("Screenshot failed: {e}");
                        std::process::exit(1)
                    }
                }
            }
        }
    }
}
impl eframe::App for PixelCraft {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui)
    }
}
fn color_image(p: &[Pixel], w: usize, h: usize) -> egui::ColorImage {
    let bytes: Vec<u8> = p.iter().flatten().copied().collect();
    egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)
}
fn tool_icon(p: &egui::Painter, r: Rect, t: Tool, colors: ThemeColors) {
    let s = Stroke::new(1.8, colors.ink);
    let a = r.left_bottom();
    let b = r.right_top();
    let c = r.center();
    match t {
        Tool::Pencil => {
            p.line_segment([a, b], Stroke::new(5., colors.ink));
            p.line_segment(
                [a + Vec2::new(1., -2.), b + Vec2::new(-2., 1.)],
                Stroke::new(2., colors.face),
            );
        }
        Tool::Eraser => {
            p.rect_stroke(r.shrink(2.), 2, s, egui::StrokeKind::Inside);
            p.line_segment([Pos2::new(r.min.x, c.y), Pos2::new(r.max.x, c.y)], s);
        }
        Tool::Fill => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    r.left_center(),
                    r.center_top(),
                    r.right_center(),
                    r.center_bottom(),
                ],
                Color32::TRANSPARENT,
                s,
            ));
            p.line_segment([r.center_bottom(), r.right_bottom()], s);
        }
        Tool::Picker => {
            p.line_segment([a, b], Stroke::new(3., colors.ink));
            p.line_segment([b + Vec2::new(-4., -2.), b + Vec2::new(2., 4.)], s);
        }
        Tool::Line => {
            p.line_segment([a, b], s);
            p.circle_filled(a, 2., colors.ink);
            p.circle_filled(b, 2., colors.ink);
        }
        Tool::Rect => {
            p.rect_stroke(r, 0, s, egui::StrokeKind::Inside);
        }
        Tool::Ellipse => {
            p.circle_stroke(c, r.width() / 2., s);
        }
        Tool::Select => {
            for i in 0..4 {
                let x = r.min.x + i as f32 * r.width() / 4.;
                let y = r.min.y + i as f32 * r.height() / 4.;
                p.line_segment([Pos2::new(x, r.min.y), Pos2::new(x + 2., r.min.y)], s);
                p.line_segment([Pos2::new(x, r.max.y), Pos2::new(x + 2., r.max.y)], s);
                p.line_segment([Pos2::new(r.min.x, y), Pos2::new(r.min.x, y + 2.)], s);
                p.line_segment([Pos2::new(r.max.x, y), Pos2::new(r.max.x, y + 2.)], s);
            }
        }
        Tool::Hand => {
            p.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0., 3.), Vec2::new(12., 11.)),
                3,
                s,
                egui::StrokeKind::Inside,
            );
            for i in 0..4 {
                let x = r.min.x + 4. + i as f32 * 3.;
                p.line_segment(
                    [
                        Pos2::new(x, c.y),
                        Pos2::new(x, r.min.y + if i % 3 == 0 { 4. } else { 1. }),
                    ],
                    s,
                );
            }
        }
    }
}
fn standalone_check(root: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.replace(Document::new(8, 8), None);
    app.doc.stamp(0, 0, 2, 2, [200, 40, 60, 255], 3);
    app.doc.add_frame(0, true);
    app.doc.tags.push(FrameTag {
        name: "Idle".into(),
        from: 0,
        to: 1,
        direction: 0,
        repeat: 0,
        color: [80, 160, 220],
    });
    let project = root.join("standalone.pixel");
    formats::save(&project, &app.doc)?;
    if formats::load(&project)? != app.doc {
        return Err("Native save mismatch".into());
    }
    formats::export_png(&root.join("frame.png"), &app.doc, 0, false)?;
    formats::export_png(&root.join("sheet.png"), &app.doc, 0, true)?;
    formats::export_gif(&root.join("animation.gif"), &app.doc)?;
    if formats::load(&root.join("frame.png"))?.composite(0) != app.doc.composite(0) {
        return Err("PNG export mismatch".into());
    }
    let source = root.join("example-plugin");
    std::fs::create_dir_all(&source).map_err(|e| e.to_string())?;
    std::fs::write(
        source.join("package.json"),
        include_str!("../examples/pixelcraft-starter/package.json"),
    )
    .map_err(|e| e.to_string())?;
    for (name, bytes) in [
        (
            "main.lua",
            include_bytes!("../examples/pixelcraft-starter/main.lua").as_slice(),
        ),
        (
            "palette.json",
            include_bytes!("../examples/pixelcraft-starter/palette.json").as_slice(),
        ),
        (
            "theme.json",
            include_bytes!("../examples/pixelcraft-starter/theme.json").as_slice(),
        ),
        (
            "shortcuts.json",
            include_bytes!("../examples/pixelcraft-starter/shortcuts.json").as_slice(),
        ),
    ] {
        std::fs::write(source.join(name), bytes).map_err(|e| e.to_string())?;
    }
    let manager = Manager::at(root.join("pixelcraft"))?;
    let id = manager.install(&source)?;
    let command = manager
        .commands()?
        .into_iter()
        .find(|c| c.id == "paint-mark")
        .ok_or("Plugin command not registered")?;
    let edited = extensions::run_lua(&app.doc, &command.script, 0, 0, Some(&command.id))?;
    if edited.layers[0].cels[0][0] != [12, 34, 56, 255] {
        return Err("Plugin drawing mismatch".into());
    }
    app.doc = edited;
    app.extension_manager = Some(manager.clone());
    for (kind, path) in [
        ("palette", "palette.json"),
        ("theme", "theme.json"),
        ("shortcuts", "shortcuts.json"),
    ] {
        app.apply_extension_asset(
            &ctx,
            kind,
            &id,
            &extensions::Contribution {
                path: path.into(),
                id: String::new(),
            },
        )?;
    }
    if app.doc.palette[1] != [12, 34, 56, 255]
        || app.custom_shortcuts.get("P").map(String::as_str) != Some("pencil")
        || ctx.style_of(ctx.theme()).visuals.panel_fill != Color32::from_rgb(32, 36, 44)
    {
        return Err("Extension asset mismatch".into());
    }
    manager.set_enabled(&id, false)?;
    if !manager.commands()?.is_empty() {
        return Err("Disabled plugin is still active".into());
    }
    manager.uninstall(&id)?;
    println!(
        "Standalone check passed: drawing, native save/open, PNG/sheet/GIF exports, embedded Lua plugin, theme, palette, shortcuts, disable and uninstall."
    );
    Ok(())
}
fn main() -> eframe::Result {
    let mut shot = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--standalone-check" {
            let root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| extensions::data_directory().join("standalone-check"));
            match standalone_check(root) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!("Standalone check failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        if a == "--screenshot" {
            shot = args.next().map(PathBuf::from);
            if shot.is_none() {
                eprintln!("--screenshot requires a PNG path");
                std::process::exit(2);
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(app_icon())
            .with_app_id("pixelcraft")
            .with_inner_size([1180., 840.])
            .with_min_inner_size([800., 620.]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "PixelCraft",
        options,
        Box::new(move |cc| Ok(Box::new(PixelCraft::new(cc, shot)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> PixelCraft {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a
    }
    #[test]
    fn named_colors_select_and_draw_independently() {
        let mut a = app();
        assert_eq!(a.named_colors, vec![("Primary".into(), DEFAULT_FG)]);
        a.add_named_color();
        a.set_active_color([230, 40, 80, 255]);
        a.select_named_color(0);
        assert_eq!(a.fg, DEFAULT_FG);
        a.select_named_color(1);
        a.paint_at((2, 3), a.fg);
        assert_eq!(a.doc.composite(0)[26], [230, 40, 80, 255]);
    }
    #[test]
    fn used_colors_follow_artwork_across_frames_and_undo() {
        let mut a = app();
        let red = [230, 40, 80, 255];
        let blue = [40, 80, 230, 128];
        a.doc.set_pixel(0, 0, 0, red);
        a.doc.add_frame(0, false);
        a.doc.set_pixel(0, 1, 1, blue);
        assert_eq!(a.sprite_colors(), vec![blue, red]);
        a.remember();
        a.doc.set_pixel(0, 1, 1, CLEAR);
        assert_eq!(a.sprite_colors(), vec![red]);
        a.undo();
        assert_eq!(a.sprite_colors(), vec![blue, red]);
    }
    #[test]
    fn native_color_defaults() {
        let a = app();
        assert_eq!(a.fg, [255, 255, 255, 255]);
        assert_eq!(a.bg, [0, 0, 0, 255]);
    }
    #[test]
    fn drawing_colors_are_shared_between_sprite_tabs() {
        let mut a = app();
        a.fg = [10, 20, 30, 40];
        a.bg = [50, 60, 70, 80];
        a.open_document(Document::new(3, 4), None);
        a.fg = [100, 110, 120, 130];
        a.bg = [140, 150, 160, 170];
        a.switch_tab(0);
        assert_eq!(a.fg, [100, 110, 120, 130]);
        assert_eq!(a.bg, [140, 150, 160, 170]);
        a.switch_tab(1);
        assert_eq!(a.fg, [100, 110, 120, 130]);
        assert_eq!(a.bg, [140, 150, 160, 170]);
    }
    #[test]
    fn tabs_preserve_unsaved_artwork_history_and_view() {
        let mut a = app();
        a.remember();
        a.paint_at((1, 1), [255, 0, 0, 255]);
        let edited = a.doc.clone();
        a.zoom = 7.;
        a.open_document(Document::new(3, 4), Some(PathBuf::from("second.pixel")));
        assert_eq!(a.doc.width, 3);
        assert!(!a.dirty());
        a.switch_tab(0);
        assert_eq!(a.doc, edited);
        assert!(a.dirty());
        assert_eq!(a.zoom, 7.);
        a.undo();
        assert!(!a.dirty());
        a.switch_tab(1);
        assert_eq!(a.path, Some(PathBuf::from("second.pixel")));
        assert_eq!(a.doc.height, 4);
    }
    #[test]
    fn closing_dirty_tab_requires_confirmation() {
        let mut a = app();
        a.open_document(Document::new(3, 3), None);
        a.remember();
        a.paint_at((0, 0), [1, 2, 3, 255]);
        a.request(Pending::CloseTab);
        assert!(a.pending.is_some());
        assert_eq!(a.tabs.len(), 2);
        a.pending = None;
        a.perform(Pending::CloseTab);
        assert_eq!(a.tabs.len(), 1);
        assert_eq!(a.doc.width, 8);
    }
    #[test]
    fn opening_another_document_detaches_the_previous_extension_session() {
        let mut a = app();
        let session = Session::new(&a.doc).unwrap();
        formats::save(&session.result_path(), &a.doc).unwrap();
        a.extension_session = Some(session.clone());
        a.replace(Document::new(3, 3), Some(PathBuf::from("other.aseprite")));
        assert!(a.import_extension_result().is_err());
        assert_eq!(a.doc.width, 3);
        assert!(session.result_path().is_file());
        std::fs::remove_dir_all(session.directory).unwrap();
    }
    #[test]
    fn extension_import_is_undoable_and_keeps_save_target() {
        let mut a = app();
        let before = a.doc.clone();
        a.path = Some(PathBuf::from("original.aseprite"));
        let session = Session::new(&a.doc).unwrap();
        let mut changed = a.doc.clone();
        changed.layers[0].cels[0][0] = [12, 34, 56, 255];
        formats::save(&session.result_path(), &changed).unwrap();
        a.extension_session = Some(session.clone());
        a.import_extension_result().unwrap();
        assert_eq!(a.doc, changed);
        assert!(a.dirty());
        assert_eq!(a.path, Some(PathBuf::from("original.aseprite")));
        a.undo();
        assert_eq!(a.doc, before);
        std::fs::remove_dir_all(session.directory).unwrap();
    }
    #[test]
    fn failed_extension_import_preserves_document_and_history() {
        let mut a = app();
        let before = a.doc.clone();
        let session = Session::new(&a.doc).unwrap();
        std::fs::write(session.result_path(), b"bad result").unwrap();
        a.extension_session = Some(session.clone());
        assert!(a.import_extension_result().is_err());
        assert_eq!(a.doc, before);
        assert!(a.undo.is_empty());
        assert!(session.result_path().is_file());
        std::fs::remove_dir_all(session.directory).unwrap();
    }
    #[test]
    fn editor_starts_dark_without_changing_artwork() {
        let ctx = egui::Context::default();
        let a = PixelCraft::with_context(&ctx, None);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(a.doc, Document::demo());
        assert!(!a.dirty());
    }
    #[test]
    fn switching_themes_preserves_edits_and_saves_preference() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.remember();
        a.doc.stamp(0, 0, 1, 1, [255, 0, 0, 255], 1);
        let edited = a.doc.clone();
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-theme-{}", std::process::id()));
        let path = directory.join("theme.json");
        a.theme_path = Some(path.clone());
        for dark in [false, true] {
            a.set_dark_mode(&ctx, dark);
            a.save_theme();
            assert_eq!(
                ctx.theme(),
                if dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                }
            );
            assert_eq!(ctx.global_style().visuals.dark_mode, dark);
            assert_eq!(ctx.global_style().visuals.panel_fill, a.colors().face);
            assert_eq!(
                serde_json::from_slice::<bool>(&std::fs::read(&path).unwrap()).unwrap(),
                dark
            );
            assert_eq!(a.doc, edited);
            assert!(a.dirty());
            assert_eq!(a.undo.len(), 1);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn undo_redo_restores_pixels_and_structural_edits() {
        let mut a = app();
        a.remember();
        a.doc.stamp(0, 0, 3, 4, [200, 30, 60, 255], 1);
        a.action("frame");
        a.action("layer");
        assert_eq!(a.doc.durations.len(), 2);
        assert_eq!(a.doc.layers.len(), 2);
        a.undo();
        a.undo();
        a.undo();
        assert_eq!(a.doc, Document::new(8, 8));
        assert!(!a.dirty());
        a.redo();
        assert_eq!(a.doc.layers[0].cels[0][35], [200, 30, 60, 255]);
        a.action("layer");
        assert!(a.redo.is_empty());
    }
    #[test]
    fn selection_copy_paste_clear_and_lock_are_consistent() {
        let mut a = app();
        a.doc.stamp(0, 0, 1, 1, [10, 20, 30, 255], 1);
        a.selection = Some((1, 1, 2, 2));
        a.copy();
        a.selection = Some((5, 4, 6, 5));
        a.paste();
        assert_eq!(a.doc.layers[0].cels[0][37], [10, 20, 30, 255]);
        a.clear();
        assert_eq!(a.doc.layers[0].cels[0][37], CLEAR);
        a.doc.layers[0].locked = true;
        let before = a.doc.clone();
        a.paste();
        a.clear();
        assert_eq!(a.doc, before);
    }
    #[test]
    fn drawing_is_blocked_while_new_sprite_dialog_is_open() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a.zoom = 10.;
        a.new_dialog = true;
        let p = Pos2::new(275., 275.);
        for events in [
            vec![],
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| a.canvas(ui));
                },
            );
            output.textures_delta.clear();
        }
        assert!(!a.dirty());
        assert!(a.undo.is_empty());
    }
    #[test]
    fn actual_pointer_stroke_changes_canvas_and_is_one_undo_step() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a.zoom = 10.;
        let draw = |a: &mut PixelCraft, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| a.canvas(ui));
            });
            output.textures_delta.clear();
        };
        draw(&mut a, vec![]);
        let p = Pos2::new(275., 275.);
        draw(
            &mut a,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let q = Pos2::new(315., 275.);
        draw(&mut a, vec![egui::Event::PointerMoved(q)]);
        draw(
            &mut a,
            vec![egui::Event::PointerButton {
                pos: q,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(
            a.doc.layers[0].cels[0].iter().filter(|p| p[3] > 0).count(),
            5
        );
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert!(a.doc.composite(0).iter().all(|p| *p == CLEAR));
    }
    #[test]
    fn native_assets_apply_inside_workshop_and_shortcuts_remain_local() {
        let ctx = egui::Context::default();
        let mut a = app();
        let root = std::env::temp_dir().join(format!("pw-ui-assets-{}", std::process::id()));
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("package.json"),r#"{"name":"ui-assets","contributes":{"palettes":[{"path":"colors.json"}],"themes":[{"path":"theme.json"}],"shortcuts":[{"path":"keys.json"}]}}"#).unwrap();
        std::fs::write(source.join("colors.json"), "[[0,0,0,0],[12,34,56,255]]").unwrap();
        std::fs::write(
            source.join("theme.json"),
            r#"{"dark":true,"panel":[21,31,41],"accent":[100,150,200]}"#,
        )
        .unwrap();
        std::fs::write(
            source.join("keys.json"),
            r#"{"P":"pencil","Ctrl+S":"save"}"#,
        )
        .unwrap();
        let manager = Manager::at(root.join("data")).unwrap();
        manager.install(&source).unwrap();
        a.extension_manager = Some(manager);
        // Applying document assets is undoable; theme/shortcut parsers are exercised
        // directly to avoid writing the test runner's real preferences.
        a.apply_extension_asset(
            &ctx,
            "palette",
            "ui-assets",
            &extensions::Contribution {
                path: "colors.json".into(),
                id: String::new(),
            },
        )
        .unwrap();
        assert_eq!(a.doc.palette[1], [12, 34, 56, 255]);
        a.apply_theme_asset(&ctx, &std::fs::read(source.join("theme.json")).unwrap())
            .unwrap();
        assert_eq!(
            ctx.style_of(ctx.theme()).visuals.panel_fill,
            Color32::from_rgb(21, 31, 41)
        );
        a.custom_shortcuts =
            serde_json::from_slice(&std::fs::read(source.join("keys.json")).unwrap()).unwrap();
        a.tool = Tool::Eraser;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::P,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| a.shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(a.tool.name(), "Pencil");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn range_playback_stays_inside_named_range_and_pingpongs() {
        let mut a = app();
        for _ in 0..3 {
            a.doc.add_frame(0, true);
        }
        a.doc.tags.push(FrameTag {
            name: "Walk".into(),
            from: 1,
            to: 2,
            direction: 2,
            repeat: 0,
            color: [0, 0, 0],
        });
        a.active_tag = Some(0);
        a.frame = 1;
        let mut frames = vec![];
        for _ in 0..5 {
            a.advance_playback();
            frames.push(a.frame);
        }
        assert_eq!(frames, vec![2, 1, 2, 1, 2]);
    }
    #[test]
    fn texture_cache_reuses_unchanged_frame_and_invalidates_after_drawing() {
        let ctx = egui::Context::default();
        let mut a = app();
        let t = a.frame_texture(&ctx, 0);
        let same = a.frame_texture(&ctx, 0);
        assert_eq!(t.id(), same.id());
        a.doc.stamp(0, 0, 0, 0, [1, 2, 3, 255], 1);
        let changed = a.frame_texture(&ctx, 0);
        assert_ne!(t.id(), changed.id());
    }
    #[test]
    fn tile_painting_changes_ids_and_eraser_clears_whole_tile() {
        let mut a = app();
        a.doc.tilesets.push(Tileset {
            name: "Test".into(),
            width: 2,
            height: 2,
            tiles: vec![vec![[10, 20, 30, 255]; 4].into()],
        });
        a.doc.add_tile_layer(0).unwrap();
        a.layer = 1;
        a.selected_tile = 1;
        a.paint_at((3, 3), [255, 0, 0, 255]);
        assert_eq!(a.doc.layers[1].tile_cels[0][5], 1);
        assert_eq!(a.doc.composite(0)[27], [10, 20, 30, 255]);
        a.tool = Tool::Eraser;
        a.paint_at((3, 3), CLEAR);
        assert_eq!(a.doc.layers[1].tile_cels[0][5], 0);
    }
    #[test]
    fn selected_duplicate_palette_slot_is_painted_as_its_own_index() {
        let mut a = app();
        a.doc.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255]];
        a.doc.set_color_mode(ColorMode::Indexed);
        a.palette_index = 2;
        a.fg = a.doc.palette[2];
        a.paint_at((0, 0), a.fg);
        assert_eq!(a.doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[0], 2);
        a.doc.set_palette_color(2, [0, 255, 0, 255]);
        assert_eq!(a.doc.composite(0)[0], [0, 255, 0, 255]);
    }
}
