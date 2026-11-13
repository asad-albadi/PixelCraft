#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod fluent;
mod ui_icons;
use ui_icons::UiIcons;
mod productivity_ui;
mod script_ui;
mod studio;
mod system_appearance;
mod system_clipboard;
use eframe::egui::{self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use pixelcraft::{
    extensions::{self, Manager, PluginCommand, Session},
    formats,
    model::*,
    updates::{self, AboutInfo, INSTALLED_VERSION, Release},
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
}
impl ThemeColors {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                face: Color32::from_rgb(32, 34, 39),
                ink: Color32::from_rgb(237, 238, 242),
                canvas: Color32::from_rgb(26, 28, 33),
                accent: Color32::from_rgb(0, 133, 133),
                hover: Color32::from_rgb(50, 57, 63),
            }
        } else {
            Self {
                face: Color32::from_rgb(247, 248, 252),
                ink: Color32::from_rgb(24, 26, 31),
                canvas: Color32::from_rgb(224, 227, 235),
                accent: Color32::from_rgb(0, 133, 133),
                hover: Color32::from_rgb(225, 241, 242),
            }
        }
    }
}
fn theme_path() -> Option<PathBuf> {
    Some(extensions::config_directory().join("theme.json"))
}
// Older versions stored a boolean; preserve those explicit overrides.
fn read_theme_preference(bytes: &[u8]) -> egui::ThemePreference {
    match serde_json::from_slice::<serde_json::Value>(bytes).ok() {
        Some(serde_json::Value::Bool(true)) => egui::ThemePreference::Dark,
        Some(serde_json::Value::Bool(false)) => egui::ThemePreference::Light,
        Some(serde_json::Value::String(value)) if value == "dark" => egui::ThemePreference::Dark,
        Some(serde_json::Value::String(value)) if value == "light" => egui::ThemePreference::Light,
        _ => egui::ThemePreference::System,
    }
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
    Lasso,
    Move,
    Gradient,
    Wand,
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
            Self::Lasso => "Lasso",
            Self::Move => "Move selection",
            Self::Gradient => "Gradient",
            Self::Wand => "Magic Wand",
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
#[derive(PartialEq)]
struct ClipboardTarget {
    tab: usize,
    generation: u64,
    layer: usize,
    frame: usize,
    selection: Option<(i32, i32, i32, i32)>,
    mask: Option<Vec<bool>>,
}
#[derive(Clone)]
struct Clipboard {
    w: usize,
    h: usize,
    pixels: Vec<Pixel>,
}
#[derive(Clone)]
struct HistoryEntry {
    doc: Document,
    selection: Option<(i32, i32, i32, i32)>,
    mask: Option<Vec<bool>>,
    polygon: Vec<(i32, i32)>,
    layer: usize,
    frame: usize,
}
struct SpriteTab {
    doc: Document,
    saved: Document,
    path: Option<PathBuf>,
    layer: usize,
    frame: usize,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    zoom: f32,
    pan: Vec2,
    selection: Option<(i32, i32, i32, i32)>,
    palette_index: usize,
    active_tag: Option<usize>,
    polygon: Vec<(i32, i32)>,
    mask: Option<Vec<bool>>,
}
// Only compare inputs used by color extraction. Editing names, timing,
// selections, drawing colors or other editor metadata leaves this cache intact.
fn same_color_artwork(a: &Document, b: &Document) -> bool {
    a.width == b.width
        && a.height == b.height
        && a.layers.len() == b.layers.len()
        && a.layers.iter().zip(&b.layers).all(|(a, b)| {
            a.group == b.group
                && a.cels == b.cels
                && a.tileset == b.tileset
                && a.tile_cels == b.tile_cels
                && a.cel_meta
                    .iter()
                    .map(|c| &c.tilemap)
                    .eq(b.cel_meta.iter().map(|c| &c.tilemap))
        })
        && a.tilesets.len() == b.tilesets.len()
        && a.tilesets
            .iter()
            .zip(&b.tilesets)
            .all(|(a, b)| a.width == b.width && a.height == b.height && a.tiles == b.tiles)
}
// GlobalPref::ColorBar 0x366d20: factory colors, shared across documents.
const DEFAULT_FG: Pixel = [255, 255, 255, 255];
const DEFAULT_BG: Pixel = [0, 0, 0, 255];
struct PixelCraft {
    studio: studio::Studio,
    power: productivity_ui::PowerTools,
    tabs: Vec<Option<SpriteTab>>,
    active_tab: usize,
    dark_mode: bool,
    theme_preference: egui::ThemePreference,
    theme_path: Option<PathBuf>,
    system_theme_rx: Option<std::sync::mpsc::Receiver<Option<egui::Theme>>>,
    system_theme: Option<Option<egui::Theme>>,
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
    used_colors_job: Option<std::sync::mpsc::Receiver<(Document, Vec<Pixel>)>>,
    used_colors: Vec<Pixel>,
    brush: i32,
    zoom: f32,
    pan: Vec2,
    grid: bool,
    onion: bool,
    filled: bool,
    playing: bool,
    last_tick: Instant,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    selection: Option<(i32, i32, i32, i32)>,
    clipboard: Option<Clipboard>,
    system_clipboard: system_clipboard::SystemClipboard,
    clipboard_target: Option<ClipboardTarget>,
    native_paste_handled: bool,
    start: Option<(i32, i32)>,
    last: Option<(i32, i32)>,
    stroke_base: Option<Document>,
    stroke_color: Pixel,
    texture: Option<egui::TextureHandle>,
    preview: Option<egui::TextureHandle>,
    preview_settings: fluent::PreviewSettings,
    preview_pan: Vec2,
    preview_settings_path: Option<PathBuf>,
    render_snapshot: Option<Document>,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    cel_occupied: Vec<Vec<bool>>,
    open_job: Option<std::sync::mpsc::Receiver<(PathBuf, Result<Document, String>)>>,
    play_after_open: bool,
    export_job: Option<std::sync::mpsc::Receiver<(PathBuf, Result<(), String>)>>,
    animation_export: Option<&'static str>,
    sequence_name: String,
    save_job: Option<(
        usize,
        std::sync::mpsc::Receiver<(PathBuf, Document, Result<(), String>)>,
    )>,
    frame_texture_order: std::collections::VecDeque<usize>,
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
    about: bool,
    github_texture: egui::TextureHandle,
    about_info: AboutInfo,
    update_job: Option<std::sync::mpsc::Receiver<Result<Release, String>>>,
    latest_release: Option<Release>,
    download_job: Option<std::sync::mpsc::Receiver<Result<updates::VerifiedDownload, String>>>,
    downloaded_update: Option<updates::VerifiedDownload>,
    update_status: String,
    next_update_check: Option<Instant>,
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
    initial_view: u8,
    inspector_open: bool,
}
impl PixelCraft {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, shot);
        app.theme_path = theme_path();
        app.restore_preview_settings(extensions::config_directory().join("preview.json"));
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("shortcuts.json")) {
            app.custom_shortcuts = pixelcraft::productivity::load_shortcuts(&bytes)
                .unwrap_or_else(|_| pixelcraft::productivity::shortcut_preset("PixelCraft"));
        }
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("language.ini")) {
            app.translations = pixelcraft::contributions::language(&bytes).unwrap_or_default();
        }
        app.restore_appearance(&cc.egui_ctx, &extensions::config_directory());
        app.system_theme_rx = system_appearance::watch(cc.egui_ctx.clone());
        app.about_info = AboutInfo::load();
        app.check_updates(&cc.egui_ctx);
        app.refresh_extensions();
        cc.egui_ctx.set_zoom_factor(app.power.interface_scale());
        app.studio.recovery_enabled = true;
        app.restore_workspace();
        app
    }
    fn restore_appearance(&mut self, ctx: &egui::Context, directory: &std::path::Path) {
        let custom = std::fs::read(directory.join("custom-theme.json"))
            .ok()
            .and_then(|bytes| pixelcraft::contributions::theme(&bytes).ok());
        // Resolve the base first: a saved choice takes priority over the asset's mode.
        if let Some(bytes) = self.theme_path.as_ref().and_then(|p| std::fs::read(p).ok()) {
            self.set_theme_preference(ctx, read_theme_preference(&bytes));
        } else if let Some(v) = &custom {
            self.set_dark_mode(ctx, v.get("dark").and_then(|v| v.as_bool()).unwrap_or(true));
        }
        if let Some(v) = &custom {
            Self::apply_theme_colors(ctx, v);
        }
    }
    fn with_context(ctx: &egui::Context, shot: Option<PathBuf>) -> Self {
        fluent::install_fonts(ctx);
        Self::apply_theme(ctx, true);
        Self::apply_theme(ctx, false);
        ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
        ctx.set_theme(egui::ThemePreference::System);
        let doc = Document::demo();
        Self {
            studio: studio::Studio::default(),
            power: productivity_ui::PowerTools::default(),
            tabs: vec![None],
            active_tab: 0,
            dark_mode: ctx.theme() == egui::Theme::Dark,
            theme_preference: egui::ThemePreference::System,
            theme_path: None,
            system_theme_rx: None,
            system_theme: None,
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
            used_colors_job: None,
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
            system_clipboard: system_clipboard::SystemClipboard::default(),
            clipboard_target: None,
            native_paste_handled: false,
            start: None,
            last: None,
            stroke_base: None,
            stroke_color: CLEAR,
            texture: None,
            preview: None,
            preview_settings: fluent::PreviewSettings::default(),
            preview_pan: Vec2::ZERO,
            preview_settings_path: None,
            render_snapshot: None,
            frame_textures: vec![],
            cel_occupied: vec![],
            open_job: None,
            play_after_open: false,
            export_job: None,
            animation_export: None,
            sequence_name: String::new(),
            save_job: None,
            frame_texture_order: Default::default(),
            onion_texture: None,
            properties_open: false,
            palette_index: 0,
            tile_size: 16,
            selected_tile: 1,
            active_tag: None,
            tag_name: "Animation".into(),
            playback_reverse: false,
            custom_shortcuts: pixelcraft::productivity::shortcut_preset("PixelCraft"),
            translations: std::collections::BTreeMap::new(),
            status: "Ready. Draw on the canvas, or create a new sprite.".into(),
            hover: None,
            pending: None,
            new_dialog: false,
            new_w: 32,
            new_h: 32,
            rename: None,
            help: false,
            about: false,
            github_texture: {
                let icon = image::load_from_memory(include_bytes!("../assets/github-mark.png"))
                    .expect("bundled GitHub icon")
                    .to_rgba8();
                ctx.load_texture(
                    "github-mark",
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], icon.as_raw()),
                    egui::TextureOptions::LINEAR,
                )
            },
            about_info: AboutInfo::default(),
            update_job: None,
            latest_release: None,
            download_job: None,
            downloaded_update: None,
            update_status: "Not checked yet.".into(),
            next_update_check: None,
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
            initial_view: 3,
            inspector_open: true,
        }
    }
    fn apply_theme(ctx: &egui::Context, dark: bool) {
        let colors = ThemeColors::new(dark);
        let theme = if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        style.visuals.panel_fill = colors.face;
        style.visuals.window_fill = if dark {
            Color32::from_rgb(37, 39, 45)
        } else {
            Color32::from_rgb(250, 251, 254)
        };
        style.visuals.override_text_color = Some(colors.ink);
        style.visuals.selection.bg_fill = if dark {
            Color32::from_rgb(25, 90, 94)
        } else {
            Color32::from_rgb(206, 239, 240)
        };
        style.visuals.selection.stroke = Stroke::new(1., colors.accent);
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.corner_radius = egui::CornerRadius::same(5);
            w.fg_stroke = Stroke::new(1., colors.ink);
        }
        style.visuals.widgets.inactive.bg_fill = if dark {
            Color32::from_rgb(43, 45, 51)
        } else {
            Color32::from_rgb(251, 252, 255)
        };
        style.visuals.widgets.inactive.weak_bg_fill = style.visuals.widgets.inactive.bg_fill;
        let border = if dark {
            Color32::from_rgb(65, 68, 77)
        } else {
            Color32::from_rgb(211, 215, 226)
        };
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., border);
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., border);
        style.visuals.widgets.hovered.bg_fill = colors.hover;
        style.visuals.widgets.hovered.weak_bg_fill = colors.hover;
        style.visuals.widgets.active.weak_bg_fill = colors.hover;
        style.visuals.window_corner_radius = egui::CornerRadius::same(8);
        style.visuals.window_stroke = Stroke::new(1., border);
        style.visuals.widgets.active.bg_fill = colors.accent;
        style.spacing.item_spacing = Vec2::new(4., 3.);
        style.spacing.button_padding = Vec2::new(6., 3.);
        style.spacing.interact_size.y = 28.;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(16.));
        ctx.set_style_of(theme, style);
    }
    fn colors(&self) -> ThemeColors {
        ThemeColors::new(self.dark_mode)
    }
    fn set_dark_mode(&mut self, ctx: &egui::Context, dark: bool) {
        self.set_theme_preference(
            ctx,
            if dark {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            },
        );
    }
    fn set_theme_preference(&mut self, ctx: &egui::Context, preference: egui::ThemePreference) {
        self.theme_preference = preference;
        ctx.set_theme(preference);
        self.dark_mode = ctx.theme() == egui::Theme::Dark;
        Self::apply_theme(ctx, self.dark_mode);
        ctx.request_repaint();
    }
    fn save_theme(&mut self) {
        if let Some(path) = &self.theme_path {
            let result = std::fs::create_dir_all(path.parent().unwrap()).and_then(|()| {
                std::fs::write(
                    path,
                    match self.theme_preference {
                        egui::ThemePreference::System => "\"system\"",
                        egui::ThemePreference::Light => "\"light\"",
                        egui::ThemePreference::Dark => "\"dark\"",
                    },
                )
            });
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
            self.frame_texture_order.clear();
            let mut occupied_pixels = std::collections::HashMap::new();
            self.cel_occupied = self
                .doc
                .layers
                .iter()
                .map(|l| {
                    (0..self.doc.durations.len())
                        .map(|f| {
                            l.group
                                || l.tile_cels[f].iter().any(|v| *v != 0)
                                || *occupied_pixels
                                    .entry(std::sync::Arc::as_ptr(&l.cels[f].0))
                                    .or_insert_with(|| l.cels[f].iter().any(|p| p[3] > 0))
                        })
                        .collect()
                })
                .collect();
            self.render_snapshot = Some(self.doc.clone());
        }
        if self.frame_textures[f].is_none() {
            let frame_bytes = self.doc.width * self.doc.height * 4;
            let capacity = (64 * 1024 * 1024 / frame_bytes).max(1);
            if self.frame_texture_order.len() >= capacity
                && let Some(old) = self.frame_texture_order.pop_front()
            {
                self.frame_textures[old] = None;
            }
            let pixels = pixelcraft::color::display_pixels(&self.doc, f).unwrap_or_else(|error| {
                self.status = format!("Color profile: {error}; showing unconverted values");
                self.doc.composite(f)
            });
            self.frame_textures[f] = Some(ctx.load_texture(
                format!("frame-{f}"),
                color_image(&pixels, self.doc.width, self.doc.height),
                egui::TextureOptions::NEAREST,
            ));
            self.frame_texture_order.push_back(f);
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
            let size = if self.studio.pressure {
                (self.brush as f32 * self.studio.force).round().max(1.) as i32
            } else {
                self.brush
            };
            let mut points = vec![p];
            if self.studio.mirror_x {
                points.push((self.doc.width as i32 - 1 - p.0, p.1));
            }
            if self.studio.mirror_y {
                let mirrored: Vec<_> = points
                    .iter()
                    .map(|p| (p.0, self.doc.height as i32 - 1 - p.1))
                    .collect();
                points.extend(mirrored);
            }
            for p in points {
                self.power_paint(p, color, size);
            }
        }
        self.doc.drawing_index = None;
    }
    fn update_playback_clock(&mut self, now: Instant) -> Duration {
        let duration =
            |app: &Self| Duration::from_millis(u64::from(app.doc.durations[app.frame].max(1)));
        let mut elapsed = now.saturating_duration_since(self.last_tick);
        // A long suspension should not make the editor replay a backlog.
        if elapsed > Duration::from_secs(1) {
            self.last_tick = now;
            return duration(self);
        }
        while elapsed >= duration(self) {
            let step = duration(self);
            self.last_tick += step;
            elapsed -= step;
            self.advance_playback();
        }
        duration(self) - elapsed
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
        Self::apply_theme_colors(ctx, &v);
        Ok(())
    }
    fn apply_theme_colors(ctx: &egui::Context, v: &serde_json::Value) {
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
        if let Some(c) = color(v, "panel") {
            style.visuals.panel_fill = c;
            style.visuals.window_fill = c;
        }
        if let Some(c) = color(v, "text") {
            style.visuals.override_text_color = Some(c);
            style.visuals.selection.stroke.color = c;
            for w in [
                &mut style.visuals.widgets.inactive,
                &mut style.visuals.widgets.hovered,
                &mut style.visuals.widgets.active,
                &mut style.visuals.widgets.open,
            ] {
                w.fg_stroke.color = c;
            }
        }
        if let Some(c) = color(v, "accent") {
            style.visuals.selection.bg_fill = c;
            style.visuals.widgets.active.bg_fill = c;
        }
        ctx.set_global_style(style);
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
                self.doc.import_palette(palette)?;
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
                self.custom_shortcuts.extend(shortcuts);
                std::fs::create_dir_all(extensions::config_directory())
                    .map_err(|e| e.to_string())?;
                std::fs::write(
                    extensions::config_directory().join("shortcuts.json"),
                    pixelcraft::productivity::save_shortcuts(&self.custom_shortcuts)?,
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
            egui::ComboBox::from_label("Color mode").selected_text(format!("{:?}",mode)).show_ui(ui,|ui|{for value in [ColorMode::Rgba,ColorMode::Indexed,ColorMode::Grayscale]{ui.pc_selectable_value(&mut mode,value,format!("{:?}",value));}});if mode!=edited.color_mode{edited.set_color_mode(mode);}
            let mut palette_size=edited.palette.len();ui.add(egui::DragValue::new(&mut palette_size).range(1..=256).prefix("Palette entries "));if palette_size!=edited.palette.len(){edited.palette.resize(palette_size,[0,0,0,255]);edited.palette_names.resize(palette_size,String::new());for p in &mut edited.frame_palettes{p.resize(palette_size,[0,0,0,255]);}let mode=edited.color_mode;edited.set_color_mode(mode);self.palette_index=self.palette_index.min(palette_size-1);}
            ui.small(format!("Canvas: {} × {} · Limit: 16384 per side, 64 million stored pixels",edited.width,edited.height));
            ui.small(format!("Preserved: {} metadata chunks · {} profile bytes · {} off-canvas pixels",edited.metadata.len(),edited.color_profile.len(),edited.layers.iter().flat_map(|l|&l.cel_meta).map(|m|m.outside.len()).sum::<usize>()));
            ui.separator();ui.heading("Active layer & cel");ui.label(&edited.layers[self.layer].name);
            let mut blend=edited.layers[self.layer].blend;
            egui::ComboBox::from_label("Blend mode").selected_text(format!("{:?}",blend)).show_ui(ui,|ui|{for v in BlendMode::ALL{ui.pc_selectable_value(&mut blend,v,format!("{:?}",v));}});edited.layers[self.layer].blend=blend;
            let groups:Vec<_>=edited.layers.iter().enumerate().filter(|(i,l)|*i!=self.layer&&l.group).map(|(i,l)|(i,l.name.clone())).collect();
            let mut parent=edited.layers[self.layer].parent;
            egui::ComboBox::from_label("Group").selected_text(parent.and_then(|i|edited.layers.get(i)).map(|l|l.name.as_str()).unwrap_or("Root")).show_ui(ui,|ui|{ui.pc_selectable_value(&mut parent,None,"Root");for(i,name)in groups{ui.pc_selectable_value(&mut parent,Some(i),name);}});
            edited.layers[self.layer].parent=parent;
            ui.add(egui::Slider::new(&mut edited.layers[self.layer].cel_meta[self.frame].opacity,0..=255).text("Cel opacity"));
            ui.add(egui::DragValue::new(&mut edited.layers[self.layer].cel_meta[self.frame].z_index).range(-32768..=32767).prefix("Cel order "));
            if ui.pc_button("+ New group").clicked(){add_group=true;}
            ui.separator();ui.heading("Tilesets & tile layers");
            ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.tile_size).range(1..=256).prefix("Tile size "));if ui.pc_button("Import PNG tilesheet…").clicked(){import=true;}
if ui.pc_button("Capture selection as tile").clicked(){capture=true;}});
            if !edited.tilesets.is_empty(){ui.horizontal(|ui|{if ui.pc_button("+ Tile layer from last tileset").clicked(){add_tile=true;}ui.add(egui::DragValue::new(&mut self.selected_tile).range(1..=edited.tilesets.last().map(|t|t.tiles.len()).unwrap_or(1).max(1) as u32).prefix("Paint tile "));});}
            ui.small("Select a tile layer, then paint with Pencil; Eraser clears a tile. Tile 0 is empty.");
            ui.separator();ui.heading("Named animation ranges");ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.tag_name);if ui.pc_button("+ Range").clicked(){edited.tags.push(FrameTag{name:self.tag_name.clone(),from:self.frame,to:self.frame,direction:0, repeat:0,color:[110,170,220]});}});
            for(i,t)in edited.tags.iter_mut().enumerate(){ui.horizontal(|ui|{ui.text_edit_singleline(&mut t.name);let mut from=t.from+1;let mut to=t.to+1;ui.add(egui::DragValue::new(&mut from).range(1..=edited.durations.len()).prefix("From "));ui.add(egui::DragValue::new(&mut to).range(from..=edited.durations.len()).prefix("To "));t.from=from-1;t.to=to.max(from)-1;if ui_icons::button(ui, "trash", "Remove range", false, false).clicked(){remove_tag=Some(i);}});ui.horizontal(|ui|{egui::ComboBox::from_id_salt(("direction",i)).selected_text(["Forward","Reverse","Ping-pong","Ping-pong reverse"][t.direction as usize]).show_ui(ui,|ui|{for (d,name)in ["Forward","Reverse","Ping-pong","Ping-pong reverse"].iter().enumerate(){ui.pc_selectable_value(&mut t.direction,d as u8,*name);}});if ui_icons::button(ui,"tag","Use for playback",self.active_tag==Some(i),false).clicked(){self.active_tag=Some(i);self.frame=if t.direction==1||t.direction==3{t.to}else{t.from};self.playback_reverse=t.direction==3;}});}
            if ui.pc_button("Use all frames for playback").clicked(){self.active_tag=None;}ui.small("Range names, endpoints and playback direction are saved in the project.");
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
        if import && let Err(e) = self.import_tileset() {
            self.status = format!("Could not import tileset: {e}");
        }
    }
    fn remember(&mut self) {
        self.power.bridge_generation += 1;
        self.push_undo(self.doc.clone());
        self.redo.clear();
    }
    fn push_undo(&mut self, d: Document) {
        self.undo.push(self.history_entry(d));
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
            .map(|entry| &entry.doc)
            .chain(self.redo.iter().map(|entry| &entry.doc))
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
                    if let Some(p) = &m.indices
                        && seen.insert(std::sync::Arc::as_ptr(p) as usize)
                    {
                        bytes = bytes.saturating_add(p.len());
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
            for r in &d.references {
                if seen.insert(std::sync::Arc::as_ptr(&r.pixels.0) as usize) {
                    bytes = bytes.saturating_add(r.pixels.len() * 4);
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
            || self.animation_export.is_some()
            || self.studio.open
            || self.studio.sheet_open
            || self.studio.tiles_open
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
        self.studio.mask = None;
        self.power.reference_edit = None;
        self.studio.polygon.clear();
    }
    fn history_entry(&self, doc: Document) -> HistoryEntry {
        HistoryEntry {
            doc,
            selection: self.selection,
            mask: self.studio.mask.clone(),
            polygon: self.studio.polygon.clone(),
            layer: self.layer,
            frame: self.frame,
        }
    }
    fn restore_history(&mut self, entry: HistoryEntry) {
        self.cancel_gesture();
        self.doc = entry.doc;
        self.clamp();
        self.layer = entry.layer.min(self.doc.layers.len() - 1);
        self.frame = entry.frame.min(self.doc.durations.len() - 1);
        self.selection = entry.selection;
        self.studio.mask = entry.mask;
        self.studio.polygon = entry.polygon;
    }
    fn undo(&mut self) {
        self.power.bridge_generation += 1;
        if let Some(d) = self.undo.pop() {
            self.redo.push(self.history_entry(self.doc.clone()));
            self.restore_history(d);
            self.status = "Undo".into();
        }
    }
    fn redo(&mut self) {
        self.power.bridge_generation += 1;
        if let Some(d) = self.redo.pop() {
            self.push_undo(self.doc.clone());
            self.restore_history(d);
            self.status = "Redo".into();
        }
    }
    fn replace(&mut self, d: Document, path: Option<PathBuf>) {
        self.power.bridge_generation += 1;
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
        self.studio.mask = None;
        self.power.reference_edit = None;
        self.studio.polygon.clear();
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
            layer: self.layer,
            frame: self.frame,
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            zoom: self.zoom,
            pan: self.pan,
            selection: self.selection,
            palette_index: self.palette_index,
            active_tag: self.active_tag,
            polygon: self.studio.polygon.clone(),
            mask: self.studio.mask.clone(),
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
        self.power.bridge_generation += 1;
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
        std::mem::swap(&mut self.studio.polygon, &mut tab.polygon);
        std::mem::swap(&mut self.studio.mask, &mut tab.mask);
        self.power.reference_edit = None;
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
        if self.save_job.is_some() && matches!(p, Pending::CloseTab) {
            self.status = "Wait for the current save before closing a tab".into();
            return;
        }
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
                        &[
                            "pixel", "aseprite", "ase", "png", "gif", "webp", "apng", "ora",
                            "json", "tmj", "tsj",
                        ],
                    )
                    .set_directory(
                        extensions::projects_directory().unwrap_or_else(|_| PathBuf::from(".")),
                    )
                    .pick_file()
                {
                    self.open_recent(path);
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
                self.autosave(true, ctx);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            self.perform(pending);
            self.autosave(false, ctx);
        }
    }
    fn save(&mut self, as_new: bool) -> bool {
        if self.save_job.is_some() {
            self.status = "A save is already running".into();
            return false;
        }
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
        let mut saving = self.doc.clone();
        if !saving.references.is_empty()
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ase" | "aseprite")
            )
        {
            let choice = rfd::MessageDialog::new()
                .set_title("Save reference images")
                .set_description("ASE files cannot store reference images. Save as .pixel to keep them, or explicitly discard them.")
                .set_buttons(rfd::MessageButtons::YesNoCancelCustom("Save as .pixel".into(), "Discard references".into(), "Cancel".into()))
                .show();
            match choice {
                rfd::MessageDialogResult::Custom(label) if label == "Save as .pixel" => {
                    let Some(pixel_path) = rfd::FileDialog::new()
                        .add_filter("PixelCraft project", &["pixel"])
                        .set_file_name(
                            path.with_extension("pixel")
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                        )
                        .save_file()
                    else {
                        return false;
                    };
                    path = pixel_path;
                    path.set_extension("pixel");
                }
                rfd::MessageDialogResult::Custom(label) if label == "Discard references" => {
                    saving.references.clear()
                }
                _ => return false,
            }
        }
        if self.pending.is_some() || saving != self.doc {
            return self.finish_save(path, saving);
        }
        self.queue_save(path, saving);
        false
    }
    fn queue_save(&mut self, path: PathBuf, saving: Document) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.save_job = Some((self.active_tab, rx));
        self.status = format!(
            "Saving {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        std::thread::spawn(move || {
            let result = formats::save(&path, &saving);
            let _ = tx.send((path, saving, result));
        });
    }
    fn poll_save(&mut self) {
        let Some((tab, job)) = &self.save_job else {
            return;
        };
        let tab = *tab;
        let result = job.try_recv();
        if matches!(result, Err(std::sync::mpsc::TryRecvError::Empty)) {
            return;
        }
        self.save_job = None;
        match result {
            Ok((path, saving, Ok(()))) => {
                self.add_recent(path.clone());
                self.status = format!(
                    "Saved {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                if tab == self.active_tab {
                    self.path = Some(path);
                    self.saved = saving;
                } else if let Some(Some(tab)) = self.tabs.get_mut(tab) {
                    tab.path = Some(path);
                    tab.saved = saving;
                }
            }
            Ok((_, _, Err(e))) => self.status = format!("Could not save: {e}"),
            Err(_) => self.status = "Save worker stopped unexpectedly".into(),
        }
    }
    fn finish_save(&mut self, path: PathBuf, saving: Document) -> bool {
        match formats::save(&path, &saving) {
            Ok(()) => {
                if self.doc != saving {
                    self.remember();
                    self.doc = saving;
                }
                self.status = format!(
                    "Saved {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                self.add_recent(path.clone());
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
        if self.export_job.is_some() {
            self.status = "An export is already running".into();
            return;
        }
        let ext = match kind {
            "gif" => "gif",
            "mp4" => "mp4",
            "bmp" => "bmp",
            _ => "png",
        };
        let sequence = kind == "png-sequence";
        let dialog = rfd::FileDialog::new();
        let chosen = if sequence {
            dialog
                .pick_folder()
                .map(|folder| folder.join(self.sequence_name.trim()))
        } else {
            dialog
                .add_filter("Export", &[ext])
                .set_file_name(format!("sprite.{ext}"))
                .save_file()
        };
        if let Some(path) = chosen {
            if matches!(kind, "gif" | "mp4")
                && !path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            {
                self.status = format!("Export failed: choose a .{ext} filename");
                return;
            }
            self.start_export(kind, path);
        }
    }
    fn start_export(&mut self, kind: &str, path: PathBuf) {
        let sequence = kind == "png-sequence";
        let (tx, rx) = std::sync::mpsc::channel();
        let doc = self.doc.clone();
        let frame = self.frame;
        let scale = self.studio.sheet.scale;
        let gif = kind == "gif";
        let video = kind == "mp4";
        let sheet = kind == "sheet";
        self.status = format!(
            "Exporting {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        self.export_job = Some(rx);
        std::thread::spawn(move || {
            let result = if sequence {
                pixelcraft::automation::prepare_png_sequence(
                    &doc,
                    path.clone(),
                    &serde_json::json!({"scale":scale}),
                    "",
                )
                .and_then(|export| export.write())
                .map(|_| ())
            } else if gif || video {
                pixelcraft::automation::write_animation_export(
                    if gif { "gif" } else { "mp4" },
                    &path,
                    &doc,
                    scale,
                )
            } else {
                formats::export_raster_scaled(&path, &doc, frame, sheet, scale)
            };
            let _ = tx.send((path, result));
        });
    }
    fn poll_export(&mut self) {
        let Some(job) = &self.export_job else { return };
        let result = job.try_recv();
        if matches!(result, Err(std::sync::mpsc::TryRecvError::Empty)) {
            return;
        }
        self.export_job = None;
        self.status = match result {
            Ok((path, result)) => match result {
                Ok(()) => format!(
                    "Exported {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
                Err(e) => format!("Export failed: {e}"),
            },
            Err(_) => "Export worker stopped unexpectedly".into(),
        };
    }
    fn copy(&mut self) {
        self.clipboard_target = None;
        let (x0, y0, x1, y1) =
            self.selection
                .unwrap_or((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1));
        let (x0, y0, x1, y1) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let w = (x1 - x0 + 1) as usize;
        let h = (y1 - y0 + 1) as usize;
        let mut pixels = vec![];
        for y in y0..=y1 {
            for x in x0..=x1 {
                pixels.push(if self.selected(x, y) {
                    self.doc
                        .index(x, y)
                        .map_or(CLEAR, |i| self.doc.layers[self.layer].cels[self.frame][i])
                } else {
                    CLEAR
                });
            }
        }
        self.clipboard = Some(Clipboard {
            w,
            h,
            pixels: pixels.clone(),
        });
        self.status = "Copied pixels from the active layer".into();
        if self.shot.is_none() && !cfg!(test) {
            let result = self.system_clipboard.copy(w, h, pixels);
            if let Err(e) = result {
                self.status = format!("Copied internally; system clipboard: {e}");
            }
        }
    }
    fn paste(&mut self) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            return;
        }
        if self.shot.is_none() && !cfg!(test) {
            match self.system_clipboard.paste() {
                Ok(()) => {
                    self.clipboard_target = Some(self.clipboard_destination());
                    self.status = "Reading system clipboard…".into();
                }
                Err(e) => {
                    self.paste_internal();
                    self.status = format!("{e}; using internal clipboard");
                }
            }
            return;
        }
        self.paste_internal();
    }
    fn clipboard_destination(&self) -> ClipboardTarget {
        ClipboardTarget {
            tab: self.active_tab,
            generation: self.power.bridge_generation,
            layer: self.layer,
            frame: self.frame,
            selection: self.selection,
            mask: self.studio.mask.clone(),
        }
    }
    fn poll_clipboard(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.system_clipboard.poll() {
            match result {
                system_clipboard::Completion::Copy(Err(e)) => {
                    self.status = format!("Copied internally; {e}")
                }
                system_clipboard::Completion::Copy(Ok(())) => {}
                system_clipboard::Completion::Paste(result) => {
                    if self.clipboard_target.take() == Some(self.clipboard_destination()) {
                        match result {
                            Ok((w, h, pixels)) => {
                                self.clipboard = Some(Clipboard { w, h, pixels });
                                self.paste_internal();
                            }
                            Err(e) => {
                                if !e.contains("timed out") && !e.contains("exceeds limits") {
                                    self.paste_internal();
                                }
                                self.status = format!("{e}; internal clipboard retained");
                            }
                        }
                    } else {
                        self.status = "Paste cancelled because the document changed".into();
                    }
                }
            }
        }
        if self.system_clipboard.busy() {
            ctx.request_repaint_after(Duration::from_millis(25));
        }
    }
    fn paste_internal(&mut self) {
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
                if self.selected(x, y)
                    && let Some(i) = self.doc.index(x, y)
                {
                    self.doc.set_pixel(self.layer, self.frame, i, CLEAR);
                }
            }
        }
    }
    fn action(&mut self, s: &str) {
        self.cancel_gesture();
        if self.studio_action(s) {
            return;
        }
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
            "png" | "bmp" | "sheet" => self.export(s),
            "png-sequence" => {
                self.sequence_name = self
                    .path
                    .as_deref()
                    .and_then(|p| p.file_stem())
                    .map_or_else(
                        || "sprite-frames".into(),
                        |stem| format!("{}-frames", stem.to_string_lossy()),
                    );
                self.animation_export = Some("png-sequence");
            }
            "gif" => self.animation_export = Some("gif"),
            "mp4" => self.animation_export = Some("mp4"),
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
                self.studio.mask = None;
                self.studio.polygon.clear();
                self.selection = Some((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1))
            }
            "deselect" => {
                self.studio.mask = None;
                self.selection = None;
                self.studio.polygon.clear();
            }
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
            "rectangle" | "rect" => self.tool = Tool::Rect,
            "ellipse" => self.tool = Tool::Ellipse,
            "selection" | "select" => self.tool = Tool::Select,
            "wand" => self.tool = Tool::Wand,
            "workflow" => self.power.open = true,
            "swap" => {
                std::mem::swap(&mut self.fg, &mut self.bg);
                self.set_active_color(self.fg);
            }
            "resetcolors" => {
                self.bg = DEFAULT_BG;
                self.set_active_color(DEFAULT_FG);
            }
            "hand" => self.tool = Tool::Hand,
            "help" => self.help = true,
            "about" => self.about = true,
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
                if let Some(session) = &self.extension_session
                    && let Ok(original) = formats::load(&session.input_path())
                {
                    self.doc = original;
                }
                let result = result.and_then(|()| self.import_extension_result());
                if let Err(e) = result {
                    self.status = format!("Extension failed: {e}");
                }
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.script_job = None;
                self.script_ui.disconnect();
                if let Some(session) = &self.extension_session
                    && let Ok(original) = formats::load(&session.input_path())
                {
                    self.doc = original;
                }
                self.status =
                    "Extension failed: the Lua worker stopped. Your sprite is unchanged.".into();
            }
            _ => {}
        }
    }
    #[cfg(test)]
    fn action_tooltip(&self, action: &str, label: &str) -> String {
        ui_icons::tooltip(&self.custom_shortcuts, action, label)
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let clipboard_action = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Copy => Some("copy"),
                egui::Event::Cut => Some("cut"),
                egui::Event::Paste(_) => Some("paste"),
                _ => None,
            })
        });
        if let Some(action) = clipboard_action {
            self.action(action);
            return;
        }
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
        }
    }
    fn menu(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.;
            ui.add_space(2.);
            let bar_rect = ui.max_rect();
            let has_update = self
                .latest_release
                .as_ref()
                .is_some_and(|r| r.is_newer_than(INSTALLED_VERSION));
            let compact = has_update && bar_rect.width() < 1000.;
            if compact {
                ui.spacing_mut().item_spacing.x = 2.;
                ui.spacing_mut().button_padding.x = 3.;
            }
            for (title, items) in [
                (
                    "File",
                    vec![
                        ("New sprite…   Ctrl+N", "new"),
                        ("Open…   Ctrl+O", "open"),
                        ("Close sprite   Ctrl+W", "closetab"),
                        ("Import PNG sequence…", "sequence"),
                        ("Grid import, tag batch & atlas…", "workflow"),
                        ("Save   Ctrl+S", "save"),
                        ("Save as…", "saveas"),
                        ("Export PNG…", "png"),
                        ("Export sprite sheet…", "sheet"),
                        ("Export PNG sequence…", "png-sequence"),
                        ("Export animation GIF…", "gif"),
                        ("Export video MP4…", "mp4"),
                        ("Export Tiled map…", "tiledexport"),
                        ("Export BMP…", "bmp"),
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
                        ("Drawing & selection studio…", "studio"),
                        ("Magic Wand", "wand"),
                        ("Brushes, references & workflow tools…", "workflow"),
                        ("Lasso selection", "lasso"),
                        ("Move selection", "move"),
                        ("Rotate selection 90°", "rotate"),
                        ("Flip selection horizontally", "selectionflipx"),
                        ("Flip selection vertically", "selectionflipy"),
                        ("Crop to selection", "crop"),
                    ],
                ),
                (
                    "Sprite",
                    vec![
                        ("Properties, layers, tiles & ranges…", "properties"),
                        ("Tileset browser…", "tiles"),
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
                        ("Seamless preview", "seamless"),
                    ],
                ),
                ("Extensions", vec![("Extensions & scripts…", "extensions")]),
                (
                    "Help",
                    vec![("Tools and file support", "help"), ("About", "about")],
                ),
            ] {
                let translated = self
                    .translations
                    .get(&format!("menu.{}", title.to_lowercase()))
                    .cloned()
                    .unwrap_or_else(|| title.to_string());
                let title_text = if compact {
                    RichText::new(translated).size(12.)
                } else {
                    RichText::new(translated)
                };
                ui.menu_button(title_text, |ui| {
                    if title == "File" {
                        ui_icons::submenu(ui, "history", "Recent files", |ui| {
                            let paths = self.recent_files();
                            if paths.is_empty() {
                                ui.label("No recent files yet");
                            }
                            for path in paths {
                                if ui_icons::button(
                                    ui,
                                    "open",
                                    &path.display().to_string(),
                                    false,
                                    true,
                                )
                                .clicked()
                                {
                                    self.open_recent(path);
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                    }
                    if title == "View" {
                        let open = self.inspector_open;
                        if ui
                            .button(if open {
                                "Hide inspector"
                            } else {
                                "Show inspector"
                            })
                            .clicked()
                        {
                            self.inspector_open = !open;
                            ui.close();
                        }
                        ui.menu_button("Density", |ui| {
                            self.density_options(ui);
                        });
                        ui_icons::submenu(ui, "theme", "Theme", |ui| {
                            for (label, preference) in [
                                ("System", egui::ThemePreference::System),
                                ("Light", egui::ThemePreference::Light),
                                ("Dark", egui::ThemePreference::Dark),
                            ] {
                                if ui_icons::button(
                                    ui,
                                    "theme",
                                    label,
                                    self.theme_preference == preference,
                                    false,
                                )
                                .clicked()
                                {
                                    self.set_theme_preference(ui.ctx(), preference);
                                    self.save_theme();
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                    }
                    for (label, a) in items {
                        if ["closetab", "deletelayer", "deleteframe", "clear"].contains(&a) {
                            ui.separator();
                        }
                        let label = label.split("   ").next().unwrap_or(label);
                        let selected = match a {
                            "grid" => self.grid,
                            "onion" => self.onion,
                            "play" => self.playing,
                            "seamless" => self.studio.seamless,
                            _ => false,
                        };
                        let icon = if a == "play" && self.playing {
                            "stop"
                        } else {
                            a
                        };
                        let enabled = match a {
                            "undo" => !self.undo.is_empty(),
                            "redo" => !self.redo.is_empty(),
                            _ => true,
                        };
                        let response = ui
                            .add_enabled_ui(enabled, |ui| {
                                ui_icons::button_as(ui, icon, a, label, selected, true)
                            })
                            .inner;
                        if response.clicked() {
                            action = Some(a);
                            ui.close();
                        }
                    }
                });
            }
            let menus_right = ui.cursor().left();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (github_rect, github) =
                    ui.allocate_exact_size(Vec2::splat(22.), Sense::click());
                ui.painter().image(
                    self.github_texture.id(),
                    github_rect.shrink(2.),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    ui.visuals().text_color(),
                );
                let github = github
                    .on_hover_text("PixelCraft on GitHub")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if github.clicked() {
                    if let Some(url) = self.about_info.github_url() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    } else {
                        self.about = true;
                    }
                }
                let rect = github_rect;
                if self
                    .latest_release
                    .as_ref()
                    .is_some_and(|r| r.is_newer_than(INSTALLED_VERSION))
                {
                    let mut notice =
                        Rect::from_center_size(bar_rect.center(), Vec2::new(148., 22.));
                    if notice.left() <= menus_right + 8. {
                        notice = Rect::from_min_size(
                            Pos2::new(rect.left() - 164., notice.top()),
                            Vec2::new(148., 22.),
                        );
                    }
                    // Prefer the center; use the free trailing area in narrow windows.
                    if notice.left() > menus_right + 8. && notice.right() < rect.left() - 8. {
                        ui.painter().text(
                            notice.center(),
                            egui::Align2::CENTER_CENTER,
                            "New version found",
                            egui::FontId::proportional(14.),
                            Color32::from_rgb(241, 190, 93),
                        );
                        if ui
                            .interact(notice, egui::Id::new("update_notice"), Sense::click())
                            .on_hover_text("Download the latest PixelCraft release")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                            && let Some(url) = self.about_info.releases_url()
                        {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
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
        self.named_colors[self.active_color].1 = color;
        self.set_drawing_color(color);
    }
    fn set_drawing_color(&mut self, color: Pixel) {
        self.fg = color;
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
        if let Some(job) = &self.used_colors_job {
            match job.try_recv() {
                Ok((snapshot, colors)) => {
                    self.used_colors_job = None;
                    if same_color_artwork(&snapshot, &self.doc) {
                        self.used_colors_snapshot = Some(snapshot);
                        self.used_colors = colors;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return self.used_colors.clone(),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.used_colors_job = None,
            }
        }
        if !self
            .used_colors_snapshot
            .as_ref()
            .is_some_and(|old| same_color_artwork(old, &self.doc))
        {
            if self.doc.stored_pixels() > 1_000_000 {
                let snapshot = self.doc.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                self.used_colors_job = Some(rx);
                std::thread::spawn(move || {
                    let colors = Self::compute_sprite_colors(&snapshot);
                    let _ = tx.send((snapshot, colors));
                });
                return self.used_colors.clone();
            }
            self.used_colors = Self::compute_sprite_colors(&self.doc);
            self.used_colors_snapshot = Some(self.doc.clone());
        }
        self.used_colors.clone()
    }
    fn compute_sprite_colors(doc: &Document) -> Vec<Pixel> {
        let mut counts = std::collections::BTreeMap::<Pixel, u64>::new();
        let mut unique = std::collections::HashMap::new();
        for (index, layer) in doc.layers.iter().enumerate() {
            if layer.group {
                continue;
            }
            for frame in 0..layer.cels.len() {
                if layer.tileset.is_some() {
                    for pixel in doc.layer_pixels(index, frame) {
                        if pixel[3] != 0 {
                            *counts.entry(pixel).or_default() += 1;
                        }
                    }
                } else {
                    let cel = &layer.cels[frame];
                    unique
                        .entry(std::sync::Arc::as_ptr(&cel.0))
                        .and_modify(|entry: &mut (&Pixels, u64)| entry.1 += 1)
                        .or_insert((cel, 1));
                }
            }
        }
        for (_, (cel, uses)) in unique {
            for &pixel in cel {
                if pixel[3] != 0 {
                    *counts.entry(pixel).or_default() += uses;
                }
            }
        }
        let mut colors: Vec<_> = counts.into_iter().collect();
        colors.sort_by_key(|&(pixel, count)| (std::cmp::Reverse(count), pixel));
        let mut groups: Vec<(Pixel, u64)> = Vec::new();
        for (pixel, count) in colors {
            let nearest = groups
                .iter()
                .enumerate()
                .map(|(i, (representative, _))| {
                    let distance: i32 = pixel
                        .iter()
                        .zip(representative)
                        .map(|(&a, &b)| (a as i32 - b as i32).pow(2))
                        .sum();
                    (distance, i)
                })
                .min();
            // Keep real artwork colors as representatives. Merge nearby shades,
            // and assign remaining shades to their nearest group once ten exist.
            if let Some((_, index)) = nearest.filter(|&(d, _)| d <= 48 * 48 || groups.len() == 10) {
                groups[index].1 += count;
            } else {
                groups.push((pixel, count));
            }
        }
        groups.sort_by_key(|&(pixel, count)| (std::cmp::Reverse(count), pixel));
        groups.into_iter().map(|(pixel, _)| pixel).collect()
    }
    fn drawing_palette_index(&self, color: Pixel) -> Option<usize> {
        let palette = self.doc.palette_for(self.frame);
        if palette.get(self.palette_index) == Some(&color) {
            Some(self.palette_index)
        } else {
            palette.iter().position(|p| *p == color)
        }
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
            Tool::Wand,
            Tool::Lasso,
            Tool::Move,
            Tool::Gradient,
            Tool::Hand,
        ] {
            let action = match t {
                Tool::Picker => "picker",
                Tool::Rect => "rectangle",
                Tool::Select => "selection",
                Tool::Pencil => "pencil",
                Tool::Eraser => "eraser",
                Tool::Fill => "fill",
                Tool::Line => "line",
                Tool::Ellipse => "ellipse",
                Tool::Hand => "hand",
                Tool::Lasso => "lasso",
                Tool::Move => "move",
                Tool::Gradient => "gradient",
                Tool::Wand => "wand",
            };
            let response = ui
                .scope(|ui| {
                    ui.spacing_mut().interact_size =
                        Vec2::new(36., if self.power.comfortable() { 36. } else { 30. });
                    ui.spacing_mut().item_spacing.y = 2.;
                    ui.spacing_mut().button_padding = Vec2::new(6., 4.);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                    ui_icons::button(ui, action, "", self.tool == t, false)
                })
                .inner;
            if response.clicked() {
                self.cancel_gesture();
                self.tool = t
            }
        }
    }
    fn timeline(&mut self, ui: &mut egui::Ui) {
        // The timeline must not composite every frame when a single cel changes.
        // Show thumbnails for frames the canvas or playback has already rendered.
        let _ = self.frame_texture(ui.ctx(), self.frame);
        let mut a = None;
        let wide = ui.available_width() > 1100.;
        let narrow = ui.available_width() < 700.;
        let short = !self.power.comfortable();
        let row_height = if short { 28. } else { 32. };
        let header_height = if short { 32. } else { 36. };
        ui.spacing_mut().interact_size.y = row_height;
        let label_width = 200.;
        let cell_width = if short { 56. } else { 64. };
        ui.horizontal_wrapped(|ui| {
            if ui
                .scope(|ui| {
                    ui.spacing_mut().interact_size.x = if short { 60. } else { 116. };
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = self.colors().accent;
                    ui.visuals_mut().widgets.inactive.fg_stroke.color = Color32::WHITE;
                    ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                    ui_icons::button_as(
                        ui,
                        if self.playing { "stop" } else { "play" },
                        "play",
                        if self.playing { "Stop" } else { "Play" },
                        self.playing,
                        false,
                    )
                })
                .inner
                .clicked()
            {
                a = Some("play");
            }
            if ui_icons::button(ui, "prev", "", false, false).clicked() {
                a = Some("prev")
            }
            if ui_icons::button(ui, "next", "", false, false).clicked() {
                a = Some("next")
            }
            ui.separator();
            ui.label(format!(
                "Frame {} / {}",
                self.frame + 1,
                self.doc.durations.len()
            ));
            let mut ms = self.doc.durations[self.frame];
            if !narrow
                && ui
                    .add(egui::DragValue::new(&mut ms).range(1..=65535).suffix(" ms"))
                    .changed()
            {
                self.remember();
                self.doc.durations[self.frame] = ms;
            }
            ui.separator();
            if !narrow && ui.pc_command_button("frame", "+ Frame").clicked() {
                a = Some("frame")
            }
            if !narrow && ui.pc_command_button("layer", "+ Layer").clicked() {
                a = Some("layer")
            }
            if wide {
                fluent::toggle(ui, &mut self.onion, "Onion skin");
            }
        });
        ui.separator();
        let mut toggle = None;
        let mut lock = None;
        let mut rename = None;
        egui::ScrollArea::both()
            .id_salt("timeline_scroll")
            .max_height((ui.available_height() - 36.).max(0.))
            .show(ui, |ui| {
                egui::Grid::new("cels")
                    .spacing(Vec2::ZERO)
                    .min_row_height(row_height)
                    .show(ui, |ui| {
                        ui.allocate_ui(Vec2::new(label_width, header_height), |ui| {
                            ui.label("");
                        });
                        for f in 0..self.doc.durations.len() {
                            if ui
                                .add_sized(
                                    [cell_width, header_height],
                                    if let Some(Some(t)) = self.frame_textures.get(f) {
                                        egui::Button::image_and_text(
                                            egui::Image::new((t.id(), Vec2::new(20., 20.))),
                                            (f + 1).to_string(),
                                        )
                                        .selected(f == self.frame)
                                    } else {
                                        egui::Button::new((f + 1).to_string())
                                            .selected(f == self.frame)
                                    },
                                )
                                .on_hover_text(format!("Select frame {}", f + 1))
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
                                if ui_icons::local_icon_button(
                                    ui,
                                    if l.visible { "eye" } else { "eye-off" },
                                    &format!(
                                        "{} layer {}",
                                        if l.visible { "Hide" } else { "Show" },
                                        l.name
                                    ),
                                    false,
                                )
                                .clicked()
                                {
                                    toggle = Some(li)
                                }
                                if ui_icons::local_icon_button(
                                    ui,
                                    if l.locked { "lock" } else { "unlock" },
                                    &format!(
                                        "{} layer {}",
                                        if l.locked { "Unlock" } else { "Lock" },
                                        l.name
                                    ),
                                    l.locked,
                                )
                                .clicked()
                                {
                                    lock = Some(li)
                                }
                                let response = ui.add_sized(
                                    [label_width - 90., row_height],
                                    egui::Button::image_and_text(
                                        egui::Image::new((
                                            ui_icons::texture(
                                                ui.ctx(),
                                                if l.group {
                                                    "group"
                                                } else if l.tileset.is_some() {
                                                    "tilemap"
                                                } else {
                                                    "layers"
                                                },
                                                16.,
                                            )
                                            .id(),
                                            Vec2::splat(16.),
                                        )),
                                        format!(
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
                                        ),
                                    )
                                    .image_tint_follows_text_color(true)
                                    .selected(li == self.layer)
                                    .truncate(),
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
                                let description = format!(
                                    "Layer {}, frame {}: {}",
                                    l.name,
                                    f + 1,
                                    if occupied { "occupied" } else { "empty" }
                                );
                                let response = ui.add_sized(
                                    [cell_width, row_height],
                                    egui::Button::image(egui::Image::new((
                                        ui_icons::texture(
                                            ui.ctx(),
                                            if occupied { "cel" } else { "plus" },
                                            16.,
                                        )
                                        .id(),
                                        Vec2::splat(16.),
                                    )))
                                    .image_tint_follows_text_color(true)
                                    .selected(li == self.layer && f == self.frame),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        li == self.layer && f == self.frame,
                                        &description,
                                    )
                                });
                                #[cfg(test)]
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        egui::Id::new(("timeline-cell", li, f)),
                                        (
                                            response.id,
                                            response.rect,
                                            ui.clip_rect(),
                                            description.clone(),
                                        ),
                                    )
                                });
                                if response.on_hover_text(&description).clicked() {
                                    self.layer = li;
                                    self.frame = f;
                                    self.playing = false;
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.menu_button("Layer & frame actions", |ui| {
            if narrow {
                let mut ms = self.doc.durations[self.frame];
                if ui
                    .add(egui::DragValue::new(&mut ms).range(1..=65535).suffix(" ms"))
                    .changed()
                {
                    self.remember();
                    self.doc.durations[self.frame] = ms;
                }
                for (command, label) in [("frame", "+ Frame"), ("layer", "+ Layer")] {
                    if ui.pc_command_button(command, label).clicked() {
                        a = Some(command);
                        ui.close();
                    }
                }
            }
            ui.pc_checkbox(&mut self.onion, "Onion skin");
            if ui.pc_button("Properties & ranges…").clicked() {
                self.properties_open = true;
                ui.close();
            }
            if ui.pc_small_button("Rename").clicked() {
                rename = Some(self.doc.layers[self.layer].name.clone());
                ui.close();
            }
            for (command, label) in [
                ("up", "Move layer up"),
                ("down", "Move layer down"),
                ("deletelayer", "Delete layer"),
                ("deleteframe", "Delete frame"),
            ] {
                if ui.pc_command_button(command, label).clicked() {
                    a = Some(command);
                    ui.close();
                }
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
        if let Some(s) = rename {
            self.rename = Some(s)
        }
        if let Some(a) = a {
            self.action(a)
        }
    }
    fn shape(&mut self, a: (i32, i32), b: (i32, i32), c: Pixel) {
        let before = self
            .selection
            .map(|_| self.doc.layers[self.layer].cels[self.frame].clone());
        let old_indices = self.doc.layers[self.layer].cel_meta[self.frame]
            .indices
            .clone();
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
        if let Some(before) = before {
            for i in 0..before.len() {
                if !self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                    self.doc.layers[l].cels[f][i] = before[i];
                    if let (Some(old), Some(new)) =
                        (&old_indices, &mut self.doc.layers[l].cel_meta[f].indices)
                    {
                        std::sync::Arc::make_mut(new)[i] = old[i];
                    }
                }
            }
        }
        self.doc.drawing_index = None;
    }
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (r, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("canvas-bounds"), r));
        let painter = ui.painter_at(r);
        painter.rect_filled(r, 0, self.colors().canvas);
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let can_edit =
            ui.is_enabled() && !self.dialog_open() && self.power.reference_edit.is_none();
        if let Some(force) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| {
                if let egui::Event::Touch { force, .. } = e {
                    *force
                } else {
                    None
                }
            })
        }) {
            self.studio.force = force.clamp(0., 1.);
        } else if !ctx.input(|i| i.pointer.any_down()) {
            self.studio.force = 1.;
        }
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
        if self.initial_view > 0 && self.start.is_none() && !ctx.input(|i| i.pointer.any_down()) {
            self.initial_view -= 1;
            if self.initial_view == 0 {
                self.zoom = self.zoom.min(
                    ((r.width() - 16.) / self.doc.width as f32)
                        .min((r.height() - 16.) / self.doc.height as f32)
                        .floor()
                        .max(1.),
                );
            }
        }
        let board = Rect::from_center_size(
            r.center() + self.pan,
            Vec2::new(
                self.doc.width as f32 * self.zoom,
                self.doc.height as f32 * self.zoom,
            ),
        );
        // Draw only visible checker squares, including at high zoom and while panning.
        let check = self.power.checker_size();
        let check_size = check * self.zoom;
        let cols = (self.doc.width as f32 / check).ceil() as i32;
        let rows = (self.doc.height as f32 / check).ceil() as i32;
        painter.rect_filled(
            board,
            0,
            self.power.checker_color(0).unwrap_or_else(|| {
                if self.dark_mode {
                    Color32::from_rgb(82, 83, 94)
                } else {
                    Color32::from_rgb(80, 81, 93)
                }
            }),
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
                        self.power.checker_color(1).unwrap_or_else(|| {
                            if self.dark_mode {
                                Color32::from_rgb(64, 65, 76)
                            } else {
                                Color32::from_rgb(63, 64, 76)
                            }
                        }),
                    );
                }
            }
        }
        self.draw_references_pass(ui, board, true);
        if self.onion && !self.playing {
            let pixels = pixelcraft::productivity::onion_pixels(
                &self.doc,
                self.frame,
                &self.power.preferences_onion(),
            );
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
        self.draw_references(ui, board);
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
                self.set_active_color(self.sample_color(p.0, p.1));
            } else if self.tool == Tool::Wand {
                let op = ctx.input(|i| {
                    if i.modifiers.command {
                        pixelcraft::productivity::SelectionOp::Subtract
                    } else if i.modifiers.shift {
                        pixelcraft::productivity::SelectionOp::Add
                    } else {
                        self.power.selection_op
                    }
                });
                self.wand_at(p, op);
            } else if matches!(self.tool, Tool::Select | Tool::Lasso) {
                self.studio.mask = None;
                self.start = Some(p);
                self.studio.polygon.clear();
                if self.tool == Tool::Lasso {
                    self.studio.polygon.push(p);
                }
                self.selection = Some((p.0, p.1, p.0, p.1));
            } else if !self.doc.editable(self.layer) {
                self.status = "This layer is locked. Unlock it in the timeline to draw.".into();
            } else {
                self.remember();
                self.start = Some(p);
                self.last = Some(p);
                self.stroke_base = Some(self.doc.clone());
                self.studio.stroke.clear();
                self.power.shade_seen.clear();
                self.studio.selection_origin = self.selection;
                self.studio.move_mask = if self.tool == Tool::Move {
                    Some(
                        (0..self.doc.width * self.doc.height)
                            .map(|i| {
                                self.selected(
                                    (i % self.doc.width) as i32,
                                    (i / self.doc.width) as i32,
                                )
                            })
                            .collect(),
                    )
                } else {
                    None
                };
                if self.tool == Tool::Fill {
                    self.doc.drawing_index = self.drawing_palette_index(self.stroke_color);
                    self.power_fill(p, self.stroke_color);
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
                if !self.doc.wraparound
                    || !matches!(
                        self.tool,
                        Tool::Pencil | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse
                    )
                {
                    b.0 = b.0.clamp(0, self.doc.width as i32 - 1);
                    b.1 = b.1.clamp(0, self.doc.height as i32 - 1);
                }
                match self.tool {
                    Tool::Lasso => {
                        if self.studio.polygon.last() != Some(&b) {
                            self.studio.polygon.push(b);
                        }
                        let points = &self.studio.polygon;
                        self.selection = Some((
                            points.iter().map(|p| p.0).min().unwrap(),
                            points.iter().map(|p| p.1).min().unwrap(),
                            points.iter().map(|p| p.0).max().unwrap(),
                            points.iter().map(|p| p.1).max().unwrap(),
                        ));
                    }
                    Tool::Move => {
                        if let Some(base) = &self.stroke_base {
                            self.doc = base.clone();
                        }
                        self.last = Some(b);
                        self.selection = self.studio.selection_origin;
                        let bounds = self.selection_bounds();
                        let mask = self
                            .studio
                            .move_mask
                            .clone()
                            .unwrap_or_else(|| vec![true; self.doc.width * self.doc.height]);
                        if let Ok(r) = pixelcraft::workflows::transform(
                            &mut self.doc,
                            self.layer,
                            self.frame,
                            bounds,
                            Some(&mask),
                            pixelcraft::workflows::Transform::Move(b.0 - a.0, b.1 - a.1),
                        ) {
                            let moved = pixelcraft::workflows::transform_mask(
                                &self.doc,
                                bounds,
                                &mask,
                                pixelcraft::workflows::Transform::Move(b.0 - a.0, b.1 - a.1),
                            );
                            self.studio.polygon.clear();
                            self.set_selection_mask(moved);
                            let _ = r;
                        }
                    }
                    Tool::Gradient => {
                        if let Some(base) = &self.stroke_base {
                            self.doc = base.clone();
                        }
                        self.gradient(a, b);
                    }
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
                            self.stroke_line(self.last.unwrap_or(a), b, color);
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
            if self.tool == Tool::Move
                && let (Some(a), Some(b)) = (self.start, self.last)
            {
                for p in &mut self.studio.polygon {
                    p.0 += b.0 - a.0;
                    p.1 += b.1 - a.1;
                }
            }
            self.start = None;
            self.last = None;
            self.stroke_base = None;
        }
        if self.studio.polygon.len() > 1 {
            let points: Vec<_> = self
                .studio
                .polygon
                .iter()
                .chain(self.studio.polygon.first())
                .map(|p| board.min + Vec2::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5) * self.zoom)
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(1., Color32::WHITE)));
        }
        self.draw_selection_mask(&painter, board, r);
        if let Some((x0, y0, x1, y1)) = self.selection.filter(|_| self.studio.mask.is_none()) {
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
        if let Some((x, y)) = self.hover
            && matches!(self.tool, Tool::Pencil | Tool::Eraser)
        {
            let size = self.brush as f32 * self.zoom;
            let p = board.min
                + Vec2::new((x - self.brush / 2) as f32, (y - self.brush / 2) as f32) * self.zoom;
            painter.rect_stroke(
                Rect::from_min_size(p, Vec2::splat(size)),
                0,
                Stroke::new(1., Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }
        painter.text(
            r.min + Vec2::new(14., 12.),
            egui::Align2::LEFT_TOP,
            format!("{} × {} px   /   RGBA", self.doc.width, self.doc.height),
            egui::FontId::proportional(12.),
            self.colors().ink,
        );
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(kind) = self.animation_export {
            let mut accept = false;
            let title = match kind {
                "png-sequence" => "Export PNG sequence",
                "gif" => "Export animated GIF",
                _ => "Export MP4 video",
            };
            if fluent::modal_dialog(ctx, title, |ui| {
                self.export_scale_ui(ui);
                let scale = self.studio.sheet.scale;
                let width = self.doc.width.saturating_mul(scale);
                let height = self.doc.height.saturating_mul(scale);
                let dimensions = if kind == "mp4" {
                    ui.small("60 fps. Transparency is composited onto black. Requires FFmpeg with libx264 on PATH.");
                    (width.next_multiple_of(2), height.next_multiple_of(2))
                } else {
                    (width, height)
                };
                ui.label(format!(
                    "{} × {} · {} animation frames",
                    dimensions.0,
                    dimensions.1,
                    self.doc.durations.len()
                ));
                if ui.pc_button("Choose export location…").clicked() {
                    accept = true;
                    true
                } else {
                    ui.pc_button("Cancel").clicked()
                }
            }) {
                self.animation_export = None;
                if accept {
                    self.export(kind);
                }
            }
        }
        if self.status.starts_with("Could not") || self.status.starts_with("Export failed") {
            if fluent::modal_dialog(ctx, "File operation failed", |ui| {
                ui.label(&self.status);
                if ui.pc_button("Close").clicked() {
                    self.status = "Ready".into();
                    true
                } else {
                    false
                }
            }) {
                self.status = "Ready".into();
            }
            return;
        }
        if self.pending.is_some() {
            if fluent::modal_dialog(ctx, "Save your changes?", |ui| {
                ui_icons::feedback(ui, "warning", "Save the current sprite before continuing.");
                ui.horizontal_wrapped(|ui| {
                    if ui.pc_button("Save").clicked() && self.save(false) {
                        let p = self.pending.take().unwrap();
                        self.finish_pending(ctx, p);
                    }
                    if ui.pc_button("Discard changes").clicked() {
                        let p = self.pending.take().unwrap();
                        self.doc = self.saved.clone();
                        self.finish_pending(ctx, p);
                    }
                    if ui.pc_button("Cancel").clicked() {
                        self.pending = None;
                    }
                });
                self.pending.is_none()
            }) {
                self.pending = None;
            }
            return;
        }
        if self.new_dialog {
            if fluent::modal_dialog(ctx, "New sprite", |ui| {
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
                    if ui.pc_button("Create sprite").clicked() {
                        if self.new_w.saturating_mul(self.new_h) <= MAX_PIXELS {
                            self.open_document(Document::new(self.new_w, self.new_h), None);
                        } else {
                            self.status =
                                "Could not create: canvas exceeds 64 million pixels".into();
                        }
                        self.new_dialog = false;
                        self.status = "New transparent sprite".into();
                    }
                    if ui.pc_button("Cancel").clicked() {
                        self.new_dialog = false;
                    }
                });
                !self.new_dialog
            }) {
                self.new_dialog = false;
            }
            return;
        }
        if let Some(mut name) = self.rename.clone() {
            let mut done = false;
            let mut cancel = false;
            let dismissed = fluent::modal_dialog(ctx, "Rename layer", |ui| {
                ui.add(egui::TextEdit::singleline(&mut name).char_limit(80));
                ui.horizontal(|ui| {
                    done = ui.pc_button("Rename").clicked();
                    cancel = ui.pc_button("Cancel").clicked();
                });
                done || cancel
            });
            if done {
                if !name.trim().is_empty() {
                    self.remember();
                    self.doc.layers[self.layer].name = name;
                }
                self.rename = None;
            } else if cancel || dismissed {
                self.rename = None
            } else {
                self.rename = Some(name)
            }
            return;
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
                ui.horizontal_wrapped(|ui|{for(label,id)in [("Run .lua…","run-lua"),("Install package…","install-extension"),("Install folder…","install-folder")]{if ui.add_enabled_ui(self.script_job.is_none(),|ui| ui.pc_button(label)).inner.clicked(){action=Some(id);}}});
                ui.separator();
                if let Some(manager)=&self.extension_manager {
                    ui.small(format!("Storage: {}",manager.root.display()));
                    match manager.list(){Ok(packages)=>{if packages.is_empty(){ui.label("No extensions installed yet.");}for p in packages{ui.horizontal(|ui|{let mut enabled=p.enabled;if ui_icons::checkbox(ui,&mut enabled,"extensions",if p.manifest.display_name.is_empty(){&p.id}else{&p.manifest.display_name}).changed(){change=Some((p.id.clone(),Some(enabled)));}
if ui.pc_small_button("Uninstall").clicked(){change=Some((p.id.clone(),None));}});if p.enabled{for a in &p.manifest.contributes.palettes{if ui.pc_button(format!("Use palette: {}",a.path)).clicked(){asset=Some(("palette",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.themes{if ui.pc_button(format!("Use theme: {}",a.path)).clicked(){asset=Some(("theme",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.languages{if ui.pc_button(format!("Use language: {}",a.id)).clicked(){asset=Some(("language",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.dithering_matrices{if ui.pc_button(format!("Use dithering: {}",a.id)).clicked(){asset=Some(("dither",p.id.clone(),a.clone()));}}for a in &p.manifest.contributes.shortcuts{if ui.pc_button(format!("Use shortcuts: {}",a.path)).clicked(){asset=Some(("shortcuts",p.id.clone(),a.clone()));}}}}},Err(e)=>{ui.label(e);}}
                }else{ui.label("Extension storage could not be created.");}
                ui.separator();ui.strong("Plugin commands");
                for c in &self.plugin_commands{if ui.add_enabled_ui(self.script_job.is_none(),|ui| ui_icons::local_button(ui,"terminal",&format!("{} · {}",c.package,c.title))).inner.clicked(){run=Some(c.clone());}}
                if !self.extension_error.is_empty(){ui_icons::feedback(ui,"error",&self.extension_error);}
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
            if let Some(c) = run
                && let Err(e) =
                    self.start_script(c.script, if c.id.is_empty() { None } else { Some(c.id) })
            {
                self.status = e;
            }
            if let Some(action) = action {
                self.cancel_gesture();
                if let Err(e) = self.extension_action(action) {
                    self.status = format!("Extension failed: {e}");
                }
            }
        }
        if self.help {
            let mut close = false;
            let dismissed = fluent::modal_dialog(ctx, "Help", |ui| {
                let max_height = (ctx.content_rect().height() - 112.).max(80.);
                egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
                    ui.strong("Draw, layer, animate");
                    ui.label("B Pencil · E Eraser · G Fill · I Eyedropper\nL Line · U Rectangle · O Ellipse · M Selection · H Hand\nX Swap colors · Alt-click Pick color\nScroll Zoom · Middle drag Pan · Space Play\nCtrl+Z Undo · Ctrl+Y Redo · Ctrl+S Save");
                    ui.separator();
                    ui.label("Selections support copy, cut, paste and delete on the active layer. Paste starts at the selection's top-left, or the canvas origin.");
                    ui.separator();
                    ui.strong("File support");
                    ui.label("Open PixelCraft projects, PNG and ASE sprites. Save .pixel projects or ASE sprites. Export PNG, sprite sheets, animated GIF and MP4.");
                    ui.label("Properties in the Sprite menu opens color modes, groups, blend modes, cel opacity, tile editing and named animation ranges. ASE profiles, metadata and off-canvas image pixels are retained. Embedded ICC profiles are converted to sRGB for display; source pixels are retained.");
                    ui.label("PixelCraft is standalone. Extensions install into its own data directory. Lua, JSON themes, palettes and shortcuts run here through the documented supported native API.");
                });
                close = ui.pc_button("Close").clicked();
                close
            });
            if close || dismissed {
                self.help = false;
            }
        }
    }
}
impl PixelCraft {
    fn render(&mut self, ui: &mut egui::Ui) {
        ui_icons::set_bindings(ui.ctx(), &self.custom_shortcuts);
        self.dark_mode = ui.ctx().theme() == egui::Theme::Dark;
        let ctx = ui.ctx().clone();
        let comfortable = self.power.comfortable();
        self.power.apply_density(&ctx);
        ui.spacing_mut().button_padding = Vec2::new(
            if comfortable { 8. } else { 6. },
            if comfortable { 5. } else { 3. },
        );
        ui.spacing_mut().interact_size.y = if comfortable { 32. } else { 28. };
        ui.spacing_mut().item_spacing = if comfortable {
            Vec2::new(6., 5.)
        } else {
            Vec2::new(4., 3.)
        };
        if self.script_job.is_none() && self.start.is_none() {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            for file in dropped {
                self.open_recent(file.path().to_path_buf());
            }
        }
        self.autosave(false, &ctx);
        self.poll_open_document();
        self.poll_export();
        self.poll_save();
        if self.open_job.is_some()
            || self.export_job.is_some()
            || self.save_job.is_some()
            || self.used_colors_job.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(25));
        }
        ctx.request_repaint_after(Duration::from_secs(15));
        if let Some(next_check) = self.next_update_check {
            if Instant::now() >= next_check && self.update_job.is_none() {
                self.check_updates(&ctx);
            } else {
                ctx.request_repaint_after(next_check.saturating_duration_since(Instant::now()));
            }
        }
        if let Some(job) = &self.update_job
            && let Ok(result) = job.try_recv()
        {
            match result {
                Ok(release) => {
                    self.update_status = if release.is_newer_than(INSTALLED_VERSION) {
                        "A new version is available.".into()
                    } else {
                        "You are up to date.".into()
                    };
                    self.latest_release = Some(release);
                }
                Err(error) => {
                    self.latest_release = None;
                    self.update_status = error;
                }
            }
            self.update_job = None;
        }
        if let Some(job) = &self.download_job
            && let Ok(result) = job.try_recv()
        {
            match result {
                Ok(download) => {
                    self.downloaded_update = Some(download);
                    self.update_status =
                        "Download verified. Ready to install when you confirm.".into();
                }
                Err(e) => self.update_status = e,
            }
            self.download_job = None;
        }
        self.about_dialog(&ctx);
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
        if let Some(request) = self.script_ui.preview.take()
            && let Ok(document) = serde_json::from_value::<Document>(request["document"].clone())
            && document.validate().is_ok()
            && self.script_ui.editor_update.is_none()
        {
            self.doc = document;
            self.script_preview_doc = Some(self.doc.clone());
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
        if self.save_job.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.status = "Wait for the current save before closing PixelCraft".into();
        } else if ctx.input(|i| i.viewport().close_requested())
            && !self.dirty()
            && let Some(i) = self
                .tabs
                .iter()
                .position(|t| t.as_ref().is_some_and(|t| t.doc != t.saved))
        {
            self.switch_tab(i);
        }
        if self.save_job.is_none() && ctx.input(|i| i.viewport().close_requested()) && self.dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if !self.dialog_open() {
            self.shortcuts(&ctx)
        }
        if self.playing {
            ctx.request_repaint_after(self.update_playback_clock(Instant::now()));
        }
        if self.dialog_open() {
            ui.disable();
        }
        egui::Panel::top("menu")
            .exact_size(if comfortable { 36. } else { 32. })
            .show(ui, |ui| self.menu(ui));
        egui::Panel::top("document_tab")
            .exact_size(if comfortable { 44. } else { 40. })
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
                            let active = i == self.active_tab;
                            let visuals = ui.visuals();
                            let fill = if active {
                                visuals.selection.bg_fill
                            } else {
                                visuals.widgets.inactive.bg_fill
                            };
                            let stroke = if active {
                                visuals.selection.stroke
                            } else {
                                visuals.widgets.inactive.bg_stroke
                            };
                            egui::Frame::new()
                                .fill(fill)
                                .stroke(stroke)
                                .corner_radius(3)
                                .inner_margin(egui::Margin::symmetric(6, 0))
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.x = 2.;
                                    ui.horizontal(|ui| {
                                        let width = ui
                                            .painter()
                                            .layout_no_wrap(
                                                name.clone(),
                                                egui::FontId::proportional(14.),
                                                ui.visuals().text_color(),
                                            )
                                            .size()
                                            .x
                                            .clamp(80., 150.)
                                            + 12.;
                                        if ui
                                            .add_sized(
                                                [width, 28.],
                                                egui::Button::image_and_text(
                                                    egui::Image::new((
                                                        ui_icons::texture(ui.ctx(), "image", 18.)
                                                            .id(),
                                                        Vec2::splat(18.),
                                                    )),
                                                    &name,
                                                )
                                                .image_tint_follows_text_color(true)
                                                .selected(active)
                                                .frame(false)
                                                .truncate(),
                                            )
                                            .on_hover_text(&name)
                                            .clicked()
                                        {
                                            selected = Some(i);
                                        }
                                        if dirty {
                                            let t = ui_icons::texture(ui.ctx(), "unsaved", 16.);
                                            ui.add(
                                                egui::Image::new((t.id(), Vec2::splat(16.)))
                                                    .tint(ui.visuals().text_color()),
                                            )
                                            .on_hover_text("Unsaved changes");
                                        }
                                        if ui_icons::button(ui, "closetab", "", false, false)
                                            .clicked()
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
                        if ui.pc_command_button("new", "+ New").clicked() {
                            self.request(Pending::New);
                        }
                        if ui.pc_command_button("open", "Open…").clicked() {
                            self.request(Pending::Open);
                        }
                    });
                });
            });
        self.fluent_context(ui);
        egui::Panel::bottom("status")
            .exact_size(24.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(14.))
                        .on_hover_text(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some((x, y)) = self.hover {
                            ui.monospace(format!("{x}, {y}"));
                        }
                        ui.small(format!("{} × {}", self.doc.width, self.doc.height));
                    });
                });
            });
        // All dimensions are logical points. The native pixels-per-point stays
        // under the window system's control; only available space drives reflow.
        if self.inspector_open {
            let panel = egui::Panel::left("palette_panel")
                .default_size(264.)
                .size_range(180.0..=(ctx.content_rect().width() * 0.4).clamp(180., 420.))
                .resizable(true)
                .show(ui, |ui| self.inspector_contents(ui));
            #[cfg(test)]
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("inspector-bounds"), panel.response.rect));
            let _ = panel;
        }
        if !self.inspector_open {
            egui::Panel::left("inspector_restore")
                .exact_size(32.)
                .resizable(false)
                .show(ui, |ui| self.inspector_toggle(ui));
        }
        egui::Panel::right("tool_panel")
            .exact_size(if comfortable { 56. } else { 52. })
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.tools(ui));
            });
        let max_timeline = (ui.available_height() - 64.).max(120.);
        let timeline_panel = egui::Panel::bottom("timeline")
            .default_size(200.0_f32.min(max_timeline))
            .size_range(120.0..=max_timeline)
            .resizable(true)
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height());
                self.timeline(ui);
            });
        #[cfg(test)]
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("timeline-bounds"),
                timeline_panel.response.rect,
            )
        });
        let _ = timeline_panel;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.colors().canvas))
            .show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
        self.studio_dialogs(&ctx);
        self.power_dialogs(&ctx);
        self.poll_clipboard(&ctx);
        self.poll_bridge(&ctx);
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
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        // egui-winit consumes Ctrl+V even when an image clipboard has no text.
        // Its key release still arrives; synthesize the missing paste event once.
        if input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Paste(_)))
        {
            self.native_paste_handled = true;
        }
        if input.events.iter().any(|e|matches!(e,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command)) {
            if !self.native_paste_handled {input.events.push(egui::Event::Paste(String::new()));}
            self.native_paste_handled=false;
        }

        if let Some(rx) = &self.system_theme_rx {
            for theme in rx.try_iter() {
                self.system_theme = Some(theme);
            }
        }
        if let Some(theme) = self.system_theme {
            input.system_theme = theme;
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui)
    }
}
fn color_image(p: &[Pixel], w: usize, h: usize) -> egui::ColorImage {
    let bytes: Vec<u8> = p.iter().flatten().copied().collect();
    egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)
}

fn standalone_check(root: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let webp = root.join("webp-runtime-check.webp");
        std::fs::write(
            &webp,
            include_bytes!("../tests/fixtures/workflows/animation.webp"),
        )
        .map_err(|e| e.to_string())?;
        let imported = formats::load(&webp)?;
        if (imported.width, imported.height) != (3, 2)
            || imported.durations != [70, 230]
            || imported.composite(0)[0] != [255, 0, 0, 255]
            || imported.composite(1)[5] != [0, 255, 0, 255]
            || imported.composite(1)[0] != CLEAR
        {
            return Err("Installed WebP animation import mismatch".into());
        }
    }
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
        || app.custom_shortcuts.get("Ctrl+O").map(String::as_str) != Some("open")
        || app.custom_shortcuts.get("Ctrl+V").map(String::as_str) != Some("paste")
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
    let headless_args: Vec<String> = std::env::args().skip(1).collect();
    if headless_args
        .first()
        .is_some_and(|a| a == "--help" || a == "-h")
    {
        println!("{}", pixelcraft::automation::CLI_HELP);
        return Ok(());
    }
    if headless_args
        .first()
        .is_some_and(|a| a == "--cli" || a == "--mcp")
    {
        let result = if headless_args[0] == "--cli" {
            pixelcraft::automation::run_cli(&headless_args[1..]).map(|v| println!("{v}"))
        } else {
            let live = if headless_args.len() == 3 && headless_args[1] == "--live" {
                Some(std::path::Path::new(&headless_args[2]))
            } else if headless_args.len() == 1 {
                None
            } else {
                eprintln!("Use --mcp [--live BRIDGE_CONFIG]");
                std::process::exit(2);
            };
            pixelcraft::automation::serve_mcp(live)
        };
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut shot = None;
    let mut opening = None;
    let mut play = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--open" {
            opening = Some(PathBuf::from(args.next().ok_or_else(|| {
                eframe::Error::AppCreation("--open requires a project path".into())
            })?));
            continue;
        }
        if a == "--play" {
            play = true;
            continue;
        }
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
            .with_decorations(true)
            .with_inner_size(if shot.is_some() {
                [1920., 1080.]
            } else {
                [1180., 840.]
            })
            .with_min_inner_size([600., 360.]),
        renderer: if std::env::var_os("PIXELCRAFT_OPENGL").is_some() {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        ..Default::default()
    };
    let app_created = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let created = app_created.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "PixelCraft",
            options,
            Box::new(move |cc| {
                let mut app = PixelCraft::new(cc, shot);
                if let Some(path) = opening {
                    app.open_recent(path);
                    app.play_after_open = play;
                }
                created.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(Box::new(app))
            }),
        )
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        failed
            if std::env::var_os("PIXELCRAFT_OPENGL").is_none()
                && !app_created.load(std::sync::atomic::Ordering::Relaxed) =>
        {
            eprintln!("wgpu unavailable; restarting with OpenGL");
            let _ = failed;
            let status = std::process::Command::new(
                std::env::current_exe().map_err(|e| eframe::Error::AppCreation(e.into()))?,
            )
            .args(std::env::args_os().skip(1))
            .env("PIXELCRAFT_OPENGL", "1")
            .status()
            .map_err(|e| eframe::Error::AppCreation(e.into()))?;
            if status.success() {
                Ok(())
            } else {
                Err(eframe::Error::AppCreation("OpenGL fallback failed".into()))
            }
        }
        Ok(Err(e)) => Err(e),
        Err(panic) => std::panic::resume_unwind(panic),
    }
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
    fn icon_tooltips_follow_rebinding_and_removal() {
        let mut a = app();
        assert!(a.action_tooltip("pencil", "Pencil").contains("B"));
        a.custom_shortcuts.remove("B");
        a.custom_shortcuts.insert("P".into(), "pencil".into());
        assert_eq!(a.action_tooltip("pencil", "Pencil"), "Pencil (P)");
        a.custom_shortcuts.remove("P");
        assert_eq!(a.action_tooltip("pencil", "Pencil"), "Pencil");
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
    fn dominant_colors_group_shades_rank_frequency_and_ignore_transparency() {
        let mut a = app();
        let red = [230, 40, 80, 255];
        let near_red = [235, 43, 83, 255];
        let blue = [20, 40, 230, 128];
        for i in 0..20 {
            a.doc.set_pixel(0, 0, i, red);
        }
        for i in 20..30 {
            a.doc.set_pixel(0, 0, i, near_red);
        }
        a.doc.set_pixel(0, 0, 30, blue);
        a.doc.set_pixel(0, 0, 31, [1, 2, 3, 0]);
        assert_eq!(a.sprite_colors(), vec![red, blue]);
    }
    #[test]
    fn dominant_colors_empty_and_single_color_need_no_padding() {
        let mut a = app();
        assert!(a.sprite_colors().is_empty());
        a.doc.set_pixel(0, 0, 0, [10, 20, 30, 255]);
        assert_eq!(a.sprite_colors(), vec![[10, 20, 30, 255]]);
    }
    #[test]
    fn dominant_colors_cache_ignores_editor_and_metadata_changes() {
        let mut a = app();
        a.doc.set_pixel(0, 0, 0, [10, 20, 30, 255]);
        let colors = a.sprite_colors();
        a.doc.layers[0].name = "Renamed".into();
        a.doc.durations[0] = 250;
        a.doc.brush_shape = BrushShape::Square;
        a.fg = [255, 0, 0, 255];
        assert_eq!(a.sprite_colors(), colors);
        assert_ne!(
            a.used_colors_snapshot.as_ref().unwrap().layers[0].name,
            "Renamed"
        );
        a.doc.set_pixel(0, 0, 0, [0, 200, 0, 255]);
        assert_eq!(a.sprite_colors(), vec![[0, 200, 0, 255]]);
    }
    #[test]
    fn dominant_swatch_selects_drawing_color_without_replacing_named_colors() {
        let mut a = app();
        let saved = a.named_colors.clone();
        a.set_drawing_color([200, 40, 80, 255]);
        assert_eq!(a.named_colors, saved);
        a.paint_at((2, 3), a.fg);
        assert_eq!(a.doc.composite(0)[26], [200, 40, 80, 255]);
        a.select_named_color(0);
        assert_eq!(a.fg, DEFAULT_FG);
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
    fn system_without_reported_theme_opens_light() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        for preference in [
            egui::ThemePreference::System,
            egui::ThemePreference::Dark,
            egui::ThemePreference::Light,
            egui::ThemePreference::System,
        ] {
            a.set_theme_preference(&ctx, preference);
            let dark = preference == egui::ThemePreference::Dark;
            assert_eq!(ctx.global_style().visuals.dark_mode, dark);
            assert_eq!(a.dark_mode, dark);
        }
    }
    #[test]
    fn saved_appearance_preserves_custom_colors_on_restart() {
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-custom-restart-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("custom-theme.json"),
            br#"{"dark":true,"panel":[71,82,93],"text":[210,220,230],"accent":[110,120,130]}"#,
        )
        .unwrap();
        for (saved, system, dark) in [
            ("dark", None, true),
            ("light", None, false),
            ("system", None, false),
            ("system", Some(egui::Theme::Dark), true),
            ("system", Some(egui::Theme::Light), false),
        ] {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    system_theme: system,
                    ..Default::default()
                },
                |_| {},
            );
            output.textures_delta.clear();
            let mut a = PixelCraft::with_context(&ctx, None);
            a.theme_path = Some(directory.join("theme.json"));
            std::fs::write(
                a.theme_path.as_ref().unwrap(),
                serde_json::to_vec(saved).unwrap(),
            )
            .unwrap();
            a.restore_appearance(&ctx, &directory);
            assert_eq!(
                a.theme_preference,
                read_theme_preference(&serde_json::to_vec(saved).unwrap())
            );
            assert_eq!(ctx.global_style().visuals.dark_mode, dark);
            assert_eq!(
                ctx.global_style().visuals.panel_fill,
                Color32::from_rgb(71, 82, 93)
            );
            assert_eq!(
                ctx.global_style().visuals.override_text_color,
                Some(Color32::from_rgb(210, 220, 230))
            );
            assert_eq!(
                ctx.global_style().visuals.selection.bg_fill,
                Color32::from_rgb(110, 120, 130)
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn editor_follows_system_light_without_changing_artwork() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_| {},
        );
        output.textures_delta.clear();
        let a = PixelCraft::with_context(&ctx, None);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        assert!(!a.dark_mode);
        assert_eq!(ctx.global_style().visuals.panel_fill, a.colors().face);
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
                read_theme_preference(&std::fs::read(&path).unwrap()),
                if dark {
                    egui::ThemePreference::Dark
                } else {
                    egui::ThemePreference::Light
                }
            );
            assert_eq!(a.doc, edited);
            assert!(a.dirty());
            assert_eq!(a.undo.len(), 1);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn desktop_theme_updates_reach_frames_without_window_theme_events() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let (tx, rx) = std::sync::mpsc::channel();
        a.system_theme_rx = Some(rx);
        for (preference, desktop, expected) in [
            (egui::ThemePreference::System, Some(egui::Theme::Dark), true),
            (
                egui::ThemePreference::System,
                Some(egui::Theme::Light),
                false,
            ),
            (egui::ThemePreference::Light, Some(egui::Theme::Dark), false),
            (egui::ThemePreference::System, Some(egui::Theme::Dark), true),
            (egui::ThemePreference::Dark, Some(egui::Theme::Light), true),
            (egui::ThemePreference::System, None, false),
        ] {
            a.set_theme_preference(&ctx, preference);
            tx.send(desktop).unwrap();
            // Linux supplies no window theme. Include a second frame without a signal.
            for _ in 0..2 {
                let mut input = egui::RawInput::default();
                eframe::App::raw_input_hook(&mut a, &ctx, &mut input);
                let mut output = ctx.run_ui(input, |ui| a.render(ui));
                output.textures_delta.clear();
                assert_eq!(a.dark_mode, expected);
                assert_eq!(ctx.global_style().visuals.dark_mode, expected);
                assert_eq!(ctx.global_style().visuals.panel_fill, a.colors().face);
                assert!(!a.dirty());
            }
        }
    }
    #[test]
    fn system_changes_update_editor_colors_but_explicit_overrides_stay_fixed() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        for (preference, system, expected) in [
            (egui::ThemePreference::System, egui::Theme::Light, false),
            (egui::ThemePreference::System, egui::Theme::Dark, true),
            (egui::ThemePreference::Light, egui::Theme::Dark, false),
            (egui::ThemePreference::Dark, egui::Theme::Light, true),
            (egui::ThemePreference::System, egui::Theme::Light, false),
        ] {
            a.set_theme_preference(&ctx, preference);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    system_theme: Some(system),
                    ..Default::default()
                },
                |ui| a.render(ui),
            );
            output.textures_delta.clear();
            assert_eq!(a.dark_mode, expected);
            assert_eq!(ctx.global_style().visuals.dark_mode, expected);
            assert_eq!(ctx.global_style().visuals.panel_fill, a.colors().face);
            assert!(!a.dirty());
        }
    }
    #[test]
    fn saved_theme_choices_support_legacy_overrides_and_system() {
        for (bytes, expected) in [
            (b"true".as_slice(), egui::ThemePreference::Dark),
            (b"false".as_slice(), egui::ThemePreference::Light),
            (br#""dark""#.as_slice(), egui::ThemePreference::Dark),
            (br#""light""#.as_slice(), egui::ThemePreference::Light),
            (br#""system""#.as_slice(), egui::ThemePreference::System),
            (b"invalid".as_slice(), egui::ThemePreference::System),
        ] {
            assert_eq!(read_theme_preference(bytes), expected);
        }
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-system-theme-{}", std::process::id()));
        let path = directory.join("theme.json");
        a.theme_path = Some(path.clone());
        a.set_dark_mode(&ctx, true);
        a.save_theme();
        a.set_theme_preference(&ctx, egui::ThemePreference::System);
        a.save_theme();
        assert_eq!(std::fs::read(&path).unwrap(), br#""system""#);
        assert_eq!(
            read_theme_preference(&std::fs::read(&path).unwrap()),
            egui::ThemePreference::System
        );
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
        a.apply_extension_asset(
            &ctx,
            "shortcuts",
            "ui-assets",
            &extensions::Contribution {
                path: "keys.json".into(),
                id: String::new(),
            },
        )
        .unwrap();
        assert_eq!(
            a.custom_shortcuts.get("Ctrl+O").map(String::as_str),
            Some("open")
        );
        assert_eq!(
            a.custom_shortcuts.get("Ctrl+V").map(String::as_str),
            Some("paste")
        );
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

impl PixelCraft {
    fn check_updates(&mut self, ctx: &egui::Context) {
        self.latest_release = None;
        if let Some(slug) = updates::repository_slug(&self.about_info.repository) {
            self.next_update_check = Some(Instant::now() + Duration::from_secs(3600));
            self.update_status = "Checking GitHub releases…".into();
            self.update_job = Some(updates::check(&slug, ctx.clone()));
        } else {
            self.next_update_check = None;
            self.update_job = None;
            self.update_status =
                "This build needs PixelCraft’s GitHub repository to check for updates.".into();
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.about {
            return;
        }
        let mut open = self.about;
        egui::Window::new("About PixelCraft")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(430.)
            .show(ctx, |ui| {
                ui.heading("PixelCraft");
                ui.label("Small pixels. Endless possibilities.");
                ui.separator();
                ui.label(format!("Developed by {}", self.about_info.developer));
                ui.horizontal(|ui| {
                    ui.label("Made with");
                    ui.pc_hyperlink_to(
                        "Craft Anything",
                        "https://github.com/storytold/craft-anything",
                    );
                    ui.label("by");
                    ui.pc_hyperlink_to("ArtCraft", "https://getartcraft.com/");
                });
                if let Some(url) = self.about_info.github_url() {
                    ui.pc_hyperlink_to("PixelCraft on GitHub", url);
                }
                ui.separator();
                ui.label(format!("Installed version: {INSTALLED_VERSION}"));
                ui.label(format!(
                    "Latest GitHub release: {}",
                    self.latest_release
                        .as_ref()
                        .map(|r| r.tag.as_str())
                        .unwrap_or("Not available")
                ));
                ui.label(&self.update_status);
                if self
                    .latest_release
                    .as_ref()
                    .is_some_and(|r| r.is_newer_than(INSTALLED_VERSION))
                    && let Some(url) = self.about_info.releases_url() {
                        ui.pc_hyperlink_to("Download the latest update", url);
                    }
                if self.latest_release.as_ref().is_some_and(|r|r.is_newer_than(INSTALLED_VERSION)&&r.download_available()) && self.download_job.is_none() && self.downloaded_update.is_none()
                    && ui.pc_button("Download update in PixelCraft").clicked(){
                        self.update_status="Downloading update and verifying SHA-256…".into();
                        self.download_job=Some(updates::download(self.latest_release.as_ref().unwrap().clone(),self.about_info.repository.clone(),extensions::config_directory().join("updates"),ctx.clone()));
                    }
                if let Some(download)=&self.downloaded_update {
                    ui.small(format!("Verified: {}",download.name));
                    if ui.pc_button("Install verified update…").clicked(){
                        let target=if cfg!(target_os="linux") {std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(||rfd::FileDialog::new().add_filter("AppImage",&["AppImage"]).set_file_name(&download.name).save_file())}else{None};
                        if (!cfg!(target_os="linux")||target.is_some()) && rfd::MessageDialog::new().set_title("Install PixelCraft update?").set_description("Install the verified update now? Your current editor stays open. Reopen PixelCraft after installation to use the new version.").set_buttons(rfd::MessageButtons::YesNo).show()==rfd::MessageDialogResult::Yes {
                            self.update_status=match updates::install(download,target.as_deref()){Ok(())=>"Update installed or installer started. Reopen PixelCraft when ready.".into(),Err(e)=>format!("Could not install update: {e}")};
                        }
                    }
                }
                if ui
                    .add_enabled_ui(self.update_job.is_none(), |ui| {
                        ui.pc_button("Check for updates")
                    })
                    .inner
                    .clicked()
                {
                    self.check_updates(ctx);
                }
                ui.separator();
                ui.collapsing("Developer and repository", |ui| {
                    ui.label(format!("Developer: {}", self.about_info.developer));
                    if let Some(url) = self.about_info.github_url() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Repository:");
                            ui.pc_hyperlink_to(&url, &url);
                        });
                    } else {
                        ui.label("Repository: Not configured in this build");
                    }
                });
                ui.separator();
                ui.small("MIT License · © 2026 PixelCraft contributors");
            });
        self.about = open;
    }
}

#[cfg(test)]
mod update_ui_tests {
    use super::*;

    fn has_notice(shape: &egui::Shape) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text == "New version found",
            egui::Shape::Vec(shapes) => shapes.iter().any(has_notice),
            _ => false,
        }
    }

    #[test]
    fn top_bar_shows_update_only_for_a_newer_release() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let mut newer = current.clone();
        newer.minor += 1;
        newer.patch = 0;
        let current_tag = format!("v{current}");
        let newer_tag = format!("v{newer}");
        for width in [800., 938., 1180.] {
            for (tag, expected) in [
                (None, false),
                (Some("v0.0.0"), false),
                (Some(current_tag.as_str()), false),
                (Some(newer_tag.as_str()), true),
            ] {
                app.latest_release =
                    tag.map(|tag| Release::parse(&format!(r#"{{"tag_name":"{tag}"}}"#)).unwrap());
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 1005.))),
                        ..Default::default()
                    },
                    |ui| {
                        egui::Panel::top("menu")
                            .exact_size(34.)
                            .show(ui, |ui| app.menu(ui));
                    },
                );
                output.textures_delta.clear();
                assert_eq!(
                    output.shapes.iter().any(|s| has_notice(&s.shape)),
                    expected,
                    "{tag:?} at {width}"
                );
            }
        }
    }
}

#[cfg(test)]
mod merge_regressions {
    use super::*;
    #[test]
    fn regression_crop_wand_mask_allows_copy_delete_and_fill() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 2), None);
        a.doc.layers[0].cels[0] = vec![[50, 60, 70, 255]; 8].into();
        a.set_selection_mask(vec![false, false, true, true, false, false, true, true]);
        a.studio_action("crop");
        assert_eq!((a.doc.width, a.doc.height), (2, 2));
        assert!(a.studio.mask.is_none());
        a.copy();
        assert_eq!(
            a.clipboard.as_ref().unwrap().pixels,
            vec![[50, 60, 70, 255]; 4]
        );
        a.clear();
        assert_eq!(a.doc.composite(0), vec![CLEAR; 4]);
        a.power_fill((0, 0), [80, 90, 100, 255]);
        assert_eq!(a.doc.composite(0), vec![[80, 90, 100, 255]; 4]);
    }
    #[test]
    fn regression_reference_save_updates_baseline_only_after_success() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(1, 1), None);
        a.doc
            .references
            .push(pixelcraft::productivity::ReferenceImage {
                behind: false,
                floating: false,
                name: "guide".into(),
                width: 1,
                height: 1,
                pixels: vec![[255; 4]].into(),
                x: 0.,
                y: 0.,
                scale: 1.,
                opacity: 255,
                visible: true,
            });
        let original = a.doc.clone();
        let baseline = a.saved.clone();
        let root = std::env::temp_dir().join(format!("save-baseline-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut discarded = original.clone();
        discarded.references.clear();
        assert!(!a.finish_save(root.join("missing").join("out.ase"), discarded.clone()));
        assert_eq!(a.doc, original);
        assert_eq!(a.saved, baseline);
        assert!(a.finish_save(root.join("keep.pixel"), original.clone()));
        assert_eq!(
            formats::load(&root.join("keep.pixel")).unwrap().references,
            original.references
        );
        assert_eq!(a.saved, original);
        assert!(a.finish_save(root.join("discard.aseprite"), discarded.clone()));
        assert_eq!(a.doc, discarded);
        assert_eq!(a.saved, discarded);
        a.undo();
        assert_eq!(a.doc.references, original.references);
        assert_ne!(a.doc, a.saved);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn regression_editor_white_first_palette_retains_alpha() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let root = std::env::temp_dir().join(format!("editor-palette-{}", std::process::id()));
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("package.json"),
            r#"{"name":"alpha-palette","contributes":{"palettes":[{"path":"colors.json"}]}}"#,
        )
        .unwrap();
        std::fs::write(
            source.join("colors.json"),
            "[[255,255,255,255],[0,0,0,255]]",
        )
        .unwrap();
        let manager = Manager::at(root.join("data")).unwrap();
        manager.install(&source).unwrap();
        a.extension_manager = Some(manager);
        a.replace(Document::new(2, 1), None);
        a.doc.palette = vec![CLEAR, [255; 4]];
        a.doc.set_pixel(0, 0, 1, [255; 4]);
        a.doc.set_color_mode(ColorMode::Indexed);
        a.apply_extension_asset(
            &ctx,
            "palette",
            "alpha-palette",
            &extensions::Contribution {
                path: "colors.json".into(),
                id: String::new(),
            },
        )
        .unwrap();
        assert_eq!(a.doc.composite(0), vec![CLEAR, [255; 4]]);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod ui_icon_tests;

#[cfg(test)]
mod selection_history_expansion {
    use super::*;
    #[test]
    fn undo_redo_restores_rectangle_mask_and_active_frame() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 2), None);
        let mask = vec![true, false, true, false, false, false, false, false];
        a.selection = Some((0, 0, 2, 0));
        a.studio.mask = Some(mask.clone());
        a.remember();
        a.doc.add_frame(0, false);
        a.frame = 1;
        a.selection = Some((1, 1, 2, 1));
        a.studio.mask = None;
        a.undo();
        assert_eq!(a.selection, Some((0, 0, 2, 0)));
        assert_eq!(a.studio.mask, Some(mask));
        assert_eq!(a.frame, 0);
        a.redo();
        assert_eq!(a.selection, Some((1, 1, 2, 1)));
        assert_eq!(a.studio.mask, None);
        assert_eq!(a.frame, 1);
    }
}
#[cfg(test)]
mod native_paste_expansion {
    use super::*;
    fn release() -> egui::Event {
        egui::Event::Key {
            key: Key::V,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }
    }
    #[test]
    fn image_only_paste_shortcut_generates_one_paste_event() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let mut input = egui::RawInput {
            events: vec![release()],
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut a, &ctx, &mut input);
        assert_eq!(
            input
                .events
                .iter()
                .filter(|e| matches!(e, egui::Event::Paste(_)))
                .count(),
            1
        );
        let mut input = egui::RawInput {
            events: vec![egui::Event::Paste("text".into())],
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut a, &ctx, &mut input);
        let mut input = egui::RawInput {
            events: vec![release()],
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut a, &ctx, &mut input);
        assert!(
            !input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Paste(_)))
        );
    }
}
