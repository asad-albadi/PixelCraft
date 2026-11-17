#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod canvas_transform;
mod color_profiles;
mod color_wheel;
mod curve_tool;
mod display_profiles;
mod editor_filters;
mod export_actions;
mod file_path_dialog;
mod fluent;
mod freehand_stroke;
use file_path_dialog::{FilePathDialog, FilePathPurpose};
use freehand_stroke::{BrushDynamics, StrokeStabilizer};
mod ui_icons;
use ui_icons::UiIcons;
mod palette_conversion;
mod productivity_ui;
mod retouch_tool;

mod script_ui;
mod selection_tools;
mod shortcut_editor;
mod spray_tool;
mod sprite_text;
use shortcut_editor::{ShortcutChord, ShortcutEdit};
mod studio;
mod subtag_playback;
mod system_appearance;
mod system_clipboard;
use eframe::egui::{self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use export_actions::{ExportJob, ExportKind, ExportRecipe, export_layer_ids, load_atlas_inputs};
#[cfg(test)]
use pixelcraft::extensions::Session;
use pixelcraft::{
    extensions::{self, Manager, PluginCommand},
    formats,
    model::*,
    updates::{self, AboutInfo, INSTALLED_VERSION, Release},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
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
    Spray,
    BlurBrush,
    JumbleBrush,
    Eraser,
    Fill,
    Picker,
    Line,
    Curve,
    Text,
    Rect,
    Ellipse,
    Polygon,
    Contour,
    Select,
    SelectEllipse,
    Hand,
    Lasso,
    PolygonLasso,
    Move,
    Gradient,
    Wand,
}
impl Tool {
    fn action(self) -> &'static str {
        match self {
            Self::Picker => "picker",
            Self::Rect => "rectangle",
            Self::Select => "selection",
            Self::SelectEllipse => "elliptical-marquee",
            Self::Pencil => "pencil",
            Self::Spray => "spray",
            Self::BlurBrush => "blur-brush",
            Self::JumbleBrush => "jumble-brush",
            Self::Eraser => "eraser",
            Self::Fill => "fill",
            Self::Line => "line",
            Self::Curve => "curve",
            Self::Text => "text",
            Self::Ellipse => "ellipse",
            Self::Polygon => "polygon",
            Self::Contour => "contour",
            Self::Hand => "hand",
            Self::Lasso => "lasso",
            Self::PolygonLasso => "polygonal-lasso",
            Self::Move => "move",
            Self::Gradient => "gradient",
            Self::Wand => "wand",
        }
    }

    fn script_name(self, filled: bool) -> Option<&'static str> {
        Some(match self {
            Self::Pencil => "pencil",
            Self::Eraser => "eraser",
            Self::Fill => "paint_bucket",
            Self::Picker => "eyedropper",
            Self::Select => "rectangular_marquee",
            Self::SelectEllipse => "elliptical_marquee",
            Self::Lasso => "lasso",
            Self::PolygonLasso => "polygonal_lasso",
            Self::Wand => "magic_wand",
            Self::Line => "line",
            Self::Curve => "curve",
            Self::BlurBrush => "blur",
            Self::JumbleBrush => "jumble",
            Self::Rect if filled => "filled_rectangle",
            Self::Rect => "rectangle",
            Self::Ellipse if filled => "filled_ellipse",
            Self::Ellipse => "ellipse",
            Self::Polygon if filled => "filled_polygon",
            Self::Polygon => "polygon",
            Self::Contour => "filled_contour",
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Spray => "Spray",
            Self::BlurBrush => "Blur brush",
            Self::JumbleBrush => "Jumble brush",
            Self::Eraser => "Eraser",
            Self::Fill => "Paint bucket",
            Self::Picker => "Eyedropper",
            Self::Line => "Line",
            Self::Curve => "Bézier curve",
            Self::Text => "Text",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Polygon => "Polygon",
            Self::Contour => "Filled contour",
            Self::Select => "Selection",
            Self::SelectEllipse => "Elliptical marquee",
            Self::Hand => "Hand",
            Self::Lasso => "Lasso",
            Self::PolygonLasso => "Polygonal lasso",
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

enum OpenTarget {
    Tab,
    RasterLayer { document: EditorId, frame_id: u32 },
}

struct FileOpenJob {
    target: OpenTarget,
    receiver: std::sync::mpsc::Receiver<(PathBuf, Result<Document, String>)>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PasteMode {
    InPlace,
    Sprite,
    Layer,
    Reference,
}

#[derive(PartialEq)]
struct ClipboardTarget {
    mode: PasteMode,
    tab: usize,
    generation: u64,
    layer: usize,
    frame: usize,
    selection: Option<(i32, i32, i32, i32)>,
    mask: Option<Vec<bool>>,
}
#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct ClipboardMask {
    origin: [i32; 2],
    selected: Vec<bool>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct ClipboardTileset {
    set: Tileset,
    flags: u32,
    base_index: i16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    user_data: Vec<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    external: Vec<Vec<u8>>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct Clipboard {
    w: usize,
    h: usize,
    pixels: Vec<Pixel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection: Option<ClipboardMask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    palette: Option<Vec<Pixel>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tileset: Option<ClipboardTileset>,
}

impl Clipboard {
    fn raster(w: usize, h: usize, pixels: Vec<Pixel>) -> Self {
        Self {
            w,
            h,
            pixels,
            selection: None,
            palette: None,
            tileset: None,
        }
    }

    fn visible_pixel(&self, index: usize) -> Pixel {
        if self
            .selection
            .as_ref()
            .is_none_or(|mask| mask.selected[index])
        {
            self.pixels[index]
        } else {
            CLEAR
        }
    }

    fn native_pixels(&self) -> Vec<Pixel> {
        (0..self.pixels.len())
            .map(|index| self.visible_pixel(index))
            .collect()
    }

    fn matches_native(&self, w: usize, h: usize, pixels: &[Pixel]) -> bool {
        (w, h) == (self.w, self.h)
            && pixels.len() == self.pixels.len()
            && pixels
                .iter()
                .enumerate()
                .all(|(i, pixel)| *pixel == self.visible_pixel(i))
    }
}

#[derive(Clone, Default)]
struct TimelineHeaders {
    anchor: Option<usize>,
    selected: Vec<usize>,
}

impl TimelineHeaders {
    fn toggle(&mut self, index: usize) {
        match self.selected.binary_search(&index) {
            Ok(position) => {
                self.selected.remove(position);
            }
            Err(position) => self.selected.insert(position, index),
        }
        self.anchor = Some(index);
    }
}

struct LayerHeaderDrag {
    layers: Vec<usize>,
    grabbed: usize,
    tab: usize,
    generation: u64,
}

#[derive(Clone, Copy)]
struct FrameHeaderDrag {
    frame: usize,
    tab: usize,
}

#[derive(Clone)]
struct CelDrag {
    selection: CelSelection,
    bounds: CelRange,
    grabbed: (usize, usize),
    tab: usize,
    generation: u64,
}

impl CelDrag {
    fn destination(&self, target: (usize, usize)) -> Option<(usize, usize)> {
        Some((
            target
                .0
                .checked_sub(self.grabbed.0 - self.bounds.first_layer)?,
            target
                .1
                .checked_sub(self.grabbed.1 - self.bounds.first_frame)?,
        ))
    }
}

#[derive(Clone)]
struct HistoryEntry {
    label: &'static str,
    doc: Document,
    selection: Option<(i32, i32, i32, i32)>,
    mask: Option<Vec<bool>>,
    polygon: Vec<(i32, i32)>,
    layer: usize,
    frame: usize,
    selected_tile: u32,
    timeline_anchor: Option<(usize, usize)>,
    selected_cels: Option<CelSelection>,
    timeline_frames: TimelineHeaders,
    timeline_layers: TimelineHeaders,
}
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
enum CanvasPane {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Copy)]
struct CanvasView {
    zoom: f32,
    pan: Vec2,
    size: Option<Vec2>,
    initial_view: u8,
    wheel_zoom_steps: f32,
}

#[derive(Clone, Copy)]
struct SplitCanvas {
    view: CanvasView,
    fraction: f32,
}

struct SpriteTab {
    doc: Document,
    last_export: Option<ExportRecipe>,
    saved: Document,
    path: Option<PathBuf>,
    live_save_root: Option<PathBuf>,
    live_save_root_identity: Option<(u64, u64)>,
    layer: usize,
    frame: usize,
    selected_tile: u32,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    zoom: f32,
    pan: Vec2,
    split_canvas: Option<SplitCanvas>,
    active_canvas: CanvasPane,
    selection: Option<(i32, i32, i32, i32)>,
    palette_index: usize,
    secondary_palette_index: Option<usize>,
    active_tag: Option<usize>,
    timeline_anchor: Option<(usize, usize)>,
    selected_cels: Option<CelSelection>,
    timeline_frames: TimelineHeaders,
    timeline_layers: TimelineHeaders,
    polygon: Vec<(i32, i32)>,
    mask: Option<Vec<bool>>,
    symmetry_axes: [Option<i32>; 2],
    previous_selection: Option<selection_tools::RememberedMask>,
}
struct ScriptPreview {
    initial_tab: usize,
    tab_count: usize,
    opened: std::collections::HashMap<u32, usize>,
    layer_keys: std::collections::HashMap<u32, ScriptTargets>,
    visible: ScriptTabPreview,
}

#[derive(Default)]
struct ScriptTargets {
    targets: std::collections::HashMap<String, EditorId>,
    cel_targets: std::collections::HashMap<u64, EditorId>,
}

impl ScriptTargets {
    fn native_key(target: EditorId) -> String {
        format!("editor:{target}")
    }

    fn expose_layers(&mut self, document: &Document) -> Vec<String> {
        let known: std::collections::HashMap<_, _> = self
            .targets
            .iter()
            .map(|(key, id)| (Self::native_key(*id), key.clone()))
            .collect();
        document
            .layers
            .iter()
            .map(|layer| {
                let native = Self::native_key(layer.editor_id);
                if let Some(key) = known.get(&native) {
                    key.clone()
                } else {
                    self.targets.insert(native.clone(), layer.editor_id);
                    native
                }
            })
            .collect()
    }

    fn apply_layers(&mut self, document: &mut Document, keys: &[String]) {
        if keys.len() != document.layers.len() {
            return;
        }
        let mut seen = std::collections::HashSet::new();
        for (key, layer) in keys.iter().zip(&mut document.layers) {
            if !seen.insert(key) {
                continue;
            }
            if let Some(target) = self.targets.get(key) {
                layer.editor_id = *target;
            } else if key
                .strip_prefix("script:")
                .is_some_and(|id| id.parse::<u64>().is_ok())
            {
                self.targets.insert(key.to_owned(), layer.editor_id);
            }
        }
    }
    // Only publish identities requested by Lua, even for large timelines.
    fn expose_cels(&self, document: &Document) -> Vec<extensions::ScriptCelTarget> {
        if self.cel_targets.is_empty() {
            return Vec::new();
        }
        let watched: std::collections::HashMap<_, _> = self
            .cel_targets
            .iter()
            .map(|(&key, id)| (id.runtime_key(), key))
            .collect();
        document
            .layers
            .iter()
            .enumerate()
            .flat_map(|(layer, source)| {
                let watched = &watched;
                source
                    .cel_meta
                    .iter()
                    .enumerate()
                    .filter_map(move |(frame, meta)| {
                        Some(extensions::ScriptCelTarget {
                            key: *watched.get(&meta.editor_id.runtime_key())?,
                            layer: layer + 1,
                            frame: frame + 1,
                        })
                    })
            })
            .collect()
    }

    fn apply_cels(
        &mut self,
        previous: &Document,
        document: &mut Document,
        keys: &[extensions::ScriptCelTarget],
    ) -> bool {
        if keys.is_empty() {
            return false;
        }
        let previous_layers: std::collections::HashMap<_, _> = previous
            .layers
            .iter()
            .map(|layer| (layer.editor_id.runtime_key(), layer))
            .collect();
        let previous_frames: std::collections::HashMap<_, _> = (0..previous.durations.len())
            .map(|frame| (previous.frame_export_id(frame), frame))
            .collect();
        let mut seen = std::collections::HashSet::new();
        let mut claimed = std::collections::HashSet::new();
        let mut changed = false;
        for target in keys {
            let Some(layer) = target.layer.checked_sub(1) else {
                continue;
            };
            let Some(frame) = target.frame.checked_sub(1) else {
                continue;
            };
            if frame >= document.durations.len() {
                continue;
            }
            let frame_id = document.frame_export_id(frame);
            let Some(layer) = document.layers.get_mut(layer) else {
                continue;
            };
            let Some(meta) = layer.cel_meta.get_mut(frame) else {
                continue;
            };
            if !seen.insert(target.key) {
                continue;
            }
            let prior = previous_layers
                .get(&layer.editor_id.runtime_key())
                .zip(previous_frames.get(&frame_id))
                .map(|(layer, &frame)| layer.cel_meta[frame].editor_id);
            let id = self
                .cel_targets
                .get(&target.key)
                .copied()
                .or(prior)
                .unwrap_or(meta.editor_id);
            if claimed.insert(id.runtime_key()) {
                changed |= prior.is_some_and(|previous| previous.runtime_key() != id.runtime_key());
                meta.editor_id = id;
                self.cel_targets.insert(target.key, id);
            }
        }
        changed
    }
}

type SelectionBounds = (i32, i32, i32, i32);

#[derive(Default)]
struct ScriptSelectionState {
    bounds: Option<SelectionBounds>,
    mask: Option<Vec<bool>>,
    polygon: Vec<(i32, i32)>,
}

struct ScriptTabPreview {
    id: u32,
    original: HistoryEntry,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    drawn: Document,
    generation: u64,
    site: (usize, usize),
    selection: ScriptSelectionState,
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
struct SaveJob {
    tab: usize,
    finished: std::sync::mpsc::Receiver<(PathBuf, Document, Result<(), String>)>,
}

struct FramePrefetch {
    frame: usize,
    snapshot: Document,
    result: std::sync::mpsc::Receiver<(egui::ColorImage, Option<String>)>,
}

impl FramePrefetch {
    fn start(ctx: &egui::Context, doc: &Document, frame: usize, display: Arc<Vec<u8>>) -> Self {
        let snapshot = doc.clone();
        let worker_doc = snapshot.clone();
        let wake = ctx.clone();
        let (send, result) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let image = PixelCraft::compose_frame_image(&worker_doc, frame, &display);
            if send.send(image).is_ok() {
                wake.request_repaint();
            }
        });
        Self {
            frame,
            snapshot,
            result,
        }
    }
}

#[derive(Clone, Copy)]
struct ReplaceColor {
    source: Pixel,
    replacement: Pixel,
    all_layers: bool,
    all_frames: bool,
    selection_only: bool,
}

fn playback_seconds(duration: u16, percent: u16) -> f64 {
    f64::from(duration.max(1)) / (10. * f64::from(percent))
}

struct PlaybackRun {
    origin: usize,
    frames: Option<Vec<usize>>,
    subtags: Option<SubtagRun>,
    once: bool,
}

struct SubtagRun {
    cursor: subtag_playback::SubtagFrames,
    following: Option<usize>,
    frame_count: usize,
}

impl SubtagRun {
    fn prepare_next(&mut self, once: bool) {
        self.following = self.cursor.next().or_else(|| {
            if once {
                None
            } else {
                self.cursor.rewind();
                self.cursor.next()
            }
        });
    }
}

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
    live_save_root: Option<PathBuf>,
    live_save_root_identity: Option<(u64, u64)>,
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
    spray_radius: i32,
    spray_rate: u16,
    spray_stroke: Option<spray_tool::SprayStroke>,
    retouch_strength: u8,
    retouch_gesture: Option<retouch_tool::RetouchGesture>,
    zoom: f32,
    wheel_zoom_steps: f32,
    pan: Vec2,
    grid: bool,
    show_drawing_grid: bool,
    snap_to_grid: bool,
    grid_settings: Option<DrawingGrid>,
    onion: bool,
    filled: bool,
    playing: bool,
    last_tick: Instant,
    playback_speed: u16,
    playback_waiting_for_frame: bool,
    playback_run: Option<PlaybackRun>,
    undo: Vec<HistoryEntry>,
    history_open: bool,
    redo: Vec<HistoryEntry>,
    selection: Option<(i32, i32, i32, i32)>,
    replace_color: Option<ReplaceColor>,
    editor_filter: Option<editor_filters::FilterDialog>,
    canvas_transform: Option<canvas_transform::CanvasTransform>,
    curve_draft: Option<curve_tool::CurveDraft>,
    text_draft: Option<sprite_text::TextDraft>,
    palette_conversion: Option<palette_conversion::PaletteConversion>,
    profile_dialog: Option<color_profiles::ProfileDialog>,
    selection_tools: selection_tools::SelectionTools,
    display_profile: display_profiles::DisplayProfile,
    color_wheel: color_wheel::ColorWheel,
    clipboard: Option<Clipboard>,
    system_clipboard: system_clipboard::SystemClipboard,
    clipboard_target: Option<ClipboardTarget>,
    script_clipboard_text: Option<String>,
    script_clipboard_reply: Option<std::sync::mpsc::Sender<serde_json::Value>>,
    native_paste_handled: bool,
    start: Option<(i32, i32)>,
    last: Option<(i32, i32)>,
    stroke_base: Option<Document>,
    stroke_stabilizer: Option<StrokeStabilizer>,
    stroke_color: Pixel,
    texture: Option<egui::TextureHandle>,
    preview: Option<egui::TextureHandle>,
    preview_frame: Option<usize>,
    preview_prefetch: Option<FramePrefetch>,
    preview_settings: fluent::PreviewSettings,
    preview_pan: Vec2,
    preview_playback: Option<fluent::PreviewPlayback>,
    canvas_view_size: Option<Vec2>,
    split_canvas: Option<SplitCanvas>,
    active_canvas: CanvasPane,
    gesture_canvas: Option<CanvasPane>,
    gesture_button: Option<egui::PointerButton>,
    preview_settings_path: Option<PathBuf>,
    render_snapshot: Option<Document>,
    frame_textures: Vec<Option<egui::TextureHandle>>,
    frame_prefetch: Option<FramePrefetch>,
    timeline_anchor: Option<(usize, usize)>,
    selected_cels: Option<CelSelection>,
    timeline_frames: TimelineHeaders,
    timeline_layers: TimelineHeaders,
    timeline_duration: u16,
    timeline_destination: Option<(usize, usize)>,
    cel_occupied: Vec<Vec<bool>>,
    open_job: Option<FileOpenJob>,
    play_after_open: bool,
    export_job: Option<ExportJob>,
    last_export: Option<ExportRecipe>,
    animation_export: Option<&'static str>,
    file_path: Option<FilePathDialog>,
    sequence_name: String,
    save_job: Option<SaveJob>,
    frame_texture_order: std::collections::VecDeque<usize>,
    onion_texture: Option<egui::TextureHandle>,
    properties_open: bool,
    palette_index: usize,
    secondary_palette_index: Option<usize>,
    tile_size: usize,
    selected_tile: u32,
    tile_pixel_mode: Option<TilePixelEdit>,
    active_tag: Option<usize>,
    tag_name: String,
    playback_reverse: bool,
    custom_shortcuts: std::collections::BTreeMap<String, String>,
    shortcut_action: String,
    shortcut_edit: Option<ShortcutEdit>,
    shortcut_settings_open: bool,
    shortcuts_path: Option<PathBuf>,
    translations: std::collections::BTreeMap<String, String>,
    status: String,
    hover: Option<(i32, i32)>,
    pending: Option<Pending>,
    new_dialog: bool,
    settings_open: bool,
    new_w: usize,
    new_h: usize,
    rename: Option<String>,
    rename_frame: Option<(EditorId, u32, String)>,
    help: bool,
    about: bool,
    github_texture: egui::TextureHandle,
    about_info: AboutInfo,
    update_job: Option<std::sync::mpsc::Receiver<Result<Release, String>>>,
    latest_release: Option<Release>,
    download_job: Option<(
        String,
        std::sync::mpsc::Receiver<Result<updates::VerifiedDownload, String>>,
    )>,
    downloaded_update: Option<(String, updates::VerifiedDownload)>,
    background_update_attempt: Option<String>,
    update_status: String,
    next_update_check: Option<Instant>,
    extensions_open: bool,
    extension_manager: Option<Manager>,
    plugin_commands: Vec<PluginCommand>,
    extension_error: String,
    #[cfg(test)]
    extension_session: Option<Session>,
    script_ui: script_ui::ScriptUi,
    script_preview: Option<ScriptPreview>,
    script_tab_closes: Vec<String>,
    script_close_destination: Option<String>,
    script_job: Option<std::sync::mpsc::Receiver<Result<extensions::ScriptWorkspace, String>>>,
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
        let shortcuts_path = extensions::config_directory().join("shortcuts.json");
        if let Ok(bytes) = std::fs::read(&shortcuts_path) {
            app.custom_shortcuts = pixelcraft::productivity::load_shortcuts(&bytes)
                .unwrap_or_else(|_| pixelcraft::productivity::shortcut_preset("PixelCraft"));
        }
        app.shortcuts_path = Some(shortcuts_path);
        if let Ok(bytes) = std::fs::read(extensions::config_directory().join("language.ini")) {
            app.translations = pixelcraft::contributions::language(&bytes).unwrap_or_default();
        }
        app.grid = app.power.default_grid();
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
            live_save_root: None,
            live_save_root_identity: None,
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
            spray_radius: 8,
            spray_rate: 60,
            spray_stroke: None,
            retouch_strength: 100,
            retouch_gesture: None,
            zoom: 10.,
            wheel_zoom_steps: 0.,
            pan: Vec2::ZERO,
            grid: false,
            show_drawing_grid: false,
            snap_to_grid: false,
            grid_settings: None,
            onion: false,
            filled: false,
            playing: false,
            last_tick: Instant::now(),
            playback_speed: 100,
            playback_waiting_for_frame: false,
            playback_run: None,
            undo: vec![],
            history_open: false,
            redo: vec![],
            selection: None,
            replace_color: None,
            editor_filter: None,
            canvas_transform: None,
            curve_draft: None,
            text_draft: None,
            palette_conversion: None,
            profile_dialog: None,
            selection_tools: selection_tools::SelectionTools::default(),
            display_profile: display_profiles::DisplayProfile::default(),
            color_wheel: color_wheel::ColorWheel::default(),
            clipboard: None,
            system_clipboard: system_clipboard::SystemClipboard::default(),
            clipboard_target: None,
            script_clipboard_text: None,
            script_clipboard_reply: None,
            native_paste_handled: false,
            start: None,
            last: None,
            stroke_base: None,
            stroke_stabilizer: None,
            stroke_color: CLEAR,
            texture: None,
            preview: None,
            preview_frame: None,
            preview_prefetch: None,
            preview_settings: fluent::PreviewSettings::default(),
            preview_pan: Vec2::ZERO,
            preview_playback: None,
            canvas_view_size: None,
            split_canvas: None,
            active_canvas: CanvasPane::Primary,
            gesture_canvas: None,
            gesture_button: None,
            preview_settings_path: None,
            render_snapshot: None,
            frame_textures: vec![],
            frame_prefetch: None,
            timeline_anchor: None,
            selected_cels: None,
            timeline_frames: TimelineHeaders::default(),
            timeline_layers: TimelineHeaders::default(),
            timeline_duration: DEFAULT_DURATION,
            timeline_destination: None,
            cel_occupied: vec![],
            open_job: None,
            play_after_open: false,
            export_job: None,
            last_export: None,
            animation_export: None,
            file_path: None,
            sequence_name: String::new(),
            save_job: None,
            frame_texture_order: Default::default(),
            onion_texture: None,
            properties_open: false,
            palette_index: 0,
            secondary_palette_index: None,
            tile_size: 16,
            selected_tile: 1,
            tile_pixel_mode: None,
            active_tag: None,
            tag_name: "Animation".into(),
            playback_reverse: false,
            custom_shortcuts: pixelcraft::productivity::shortcut_preset("PixelCraft"),
            shortcut_action: "wand".into(),
            shortcut_edit: None,
            shortcut_settings_open: false,
            shortcuts_path: None,
            translations: std::collections::BTreeMap::new(),
            status: "Ready. Draw on the canvas, or create a new sprite.".into(),
            hover: None,
            pending: None,
            new_dialog: false,
            settings_open: false,
            new_w: 32,
            new_h: 32,
            rename: None,
            rename_frame: None,
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
            background_update_attempt: None,
            update_status: "Not checked yet.".into(),
            next_update_check: None,
            extensions_open: false,
            extension_manager: Manager::new().ok(),
            plugin_commands: vec![],
            extension_error: String::new(),
            #[cfg(test)]
            extension_session: None,
            script_job: None,
            script_ui: script_ui::ScriptUi::default(),
            script_preview: None,
            script_tab_closes: Vec::new(),
            script_close_destination: None,
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
        self.doc.differs_from_saved(&self.saved)
    }
    fn frame_texture(&mut self, ctx: &egui::Context, f: usize) -> egui::TextureHandle {
        self.refresh_frame_cache(ctx);
        if self.frame_textures[f].is_none() {
            let (image, error) =
                Self::compose_frame_image(&self.doc, f, &self.display_profile.bytes);
            self.store_frame_texture(ctx, f, image, error);
        }
        let texture = self.frame_textures[f].as_ref().unwrap().clone();
        if self.playing
            && self.frame_prefetch.is_none()
            && let Some((frame, _)) = self.next_playback_frame()
            && self.frame_textures[frame].is_none()
        {
            self.frame_prefetch = Some(FramePrefetch::start(
                ctx,
                &self.doc,
                frame,
                Arc::clone(&self.display_profile.bytes),
            ));
        }
        texture
    }

    fn update_preview_texture(&mut self, ctx: &egui::Context, frame: usize) {
        self.refresh_frame_cache(ctx);
        let ready = self.poll_preview_prefetch();
        if frame == self.frame {
            self.preview_frame = Some(frame);
            self.preview = Some(self.frame_texture(ctx, frame));
        } else if let Some(texture) = &self.frame_textures[frame] {
            self.preview_frame = Some(frame);
            self.preview = Some(texture.clone());
        } else if self.preview_frame != Some(frame) {
            // Retain one detached display texture outside the canvas cache, and
            // compose large misses in a worker so repainting can continue.
            const ASYNC_PREVIEW_PIXELS: usize = 2_000_000;
            if self.doc.width * self.doc.height <= ASYNC_PREVIEW_PIXELS {
                let (image, error) =
                    Self::compose_frame_image(&self.doc, frame, &self.display_profile.bytes);
                self.preview_frame = Some(frame);
                self.preview = Some(self.load_frame_texture(ctx, frame, image, error));
                return;
            }
            if self.preview_prefetch.is_none() && ready.as_ref().is_none_or(|(f, _, _)| *f != frame)
            {
                self.preview_prefetch = Some(FramePrefetch::start(
                    ctx,
                    &self.doc,
                    frame,
                    Arc::clone(&self.display_profile.bytes),
                ));
            }
            if let Some((completed, image, error)) = ready {
                self.preview_frame = Some(completed);
                self.preview = Some(self.load_frame_texture(ctx, completed, image, error));
            } else if self.preview.is_none() {
                self.preview_frame = Some(self.frame);
                self.preview = Some(self.frame_texture(ctx, self.frame));
            }
        }
    }

    fn poll_preview_prefetch(&mut self) -> Option<(usize, egui::ColorImage, Option<String>)> {
        let result = match self.preview_prefetch.as_ref()?.result.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            result => result,
        };
        let job = self.preview_prefetch.take().unwrap();
        if let Ok((image, error)) = result
            && job.frame < self.doc.durations.len()
            && self.doc.same_frame_pixels(&job.snapshot, job.frame)
        {
            return Some((job.frame, image, error));
        }
        None
    }

    fn refresh_frame_cache(&mut self, ctx: &egui::Context) {
        if self.render_snapshot.as_ref() != Some(&self.doc) {
            if self.preview_frame.is_some_and(|frame| {
                frame >= self.doc.durations.len()
                    || self
                        .render_snapshot
                        .as_ref()
                        .is_none_or(|previous| !self.doc.same_frame_pixels(previous, frame))
            }) {
                self.preview_frame = None;
                self.preview = None;
            }
            self.frame_textures.resize(self.doc.durations.len(), None);
            for (frame, texture) in self.frame_textures.iter_mut().enumerate() {
                if texture.is_some()
                    && self
                        .render_snapshot
                        .as_ref()
                        .is_none_or(|previous| !self.doc.same_frame_pixels(previous, frame))
                {
                    *texture = None;
                }
            }
            self.frame_texture_order
                .retain(|&frame| self.frame_textures.get(frame).is_some_and(Option::is_some));
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
                                    .entry(l.cels[f].backing_id())
                                    .or_insert_with(|| l.cels[f].occupied())
                        })
                        .collect()
                })
                .collect();
            self.render_snapshot = Some(self.doc.clone());
        }
        self.poll_frame_prefetch(ctx);
    }

    fn compose_frame_image(
        doc: &Document,
        frame: usize,
        display: &[u8],
    ) -> (egui::ColorImage, Option<String>) {
        let pixels = doc.composite_view(frame);
        let (pixels, error) = if doc.color_profile == display {
            (pixels, None)
        } else {
            match pixelcraft::color::transform_pixels(&doc.color_profile, display, &pixels) {
                Ok(converted) => (converted, None),
                Err(error) => {
                    let fallback =
                        pixelcraft::color::to_srgb(&doc.color_profile, &pixels).unwrap_or(pixels);
                    (fallback, Some(error))
                }
            }
        };
        (color_image(&pixels, doc.width, doc.height), error)
    }

    fn poll_frame_prefetch(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.frame_prefetch else {
            return;
        };
        let result = job.result.try_recv();
        if matches!(result, Err(std::sync::mpsc::TryRecvError::Empty)) {
            return;
        }
        let job = self.frame_prefetch.take().unwrap();
        if let Ok((image, error)) = result
            && self.playing
            && self
                .frame_textures
                .get(job.frame)
                .is_some_and(Option::is_none)
            && self.doc.same_frame_pixels(&job.snapshot, job.frame)
        {
            self.store_frame_texture(ctx, job.frame, image, error);
        }
    }

    fn store_frame_texture(
        &mut self,
        ctx: &egui::Context,
        frame: usize,
        image: egui::ColorImage,
        error: Option<String>,
    ) {
        let frame_bytes = self.doc.width * self.doc.height * 4;
        let capacity = (64 * 1024 * 1024 / frame_bytes).max(if self.playing { 2 } else { 1 });
        while self.frame_texture_order.len() >= capacity {
            let oldest = self
                .frame_texture_order
                .iter()
                .position(|&cached| !self.playing || cached != self.frame)
                .expect("Playback reserves room for the current and upcoming frames");
            let old = self.frame_texture_order.remove(oldest).unwrap();
            self.frame_textures[old] = None;
        }
        self.frame_textures[frame] = Some(self.load_frame_texture(ctx, frame, image, error));
        self.frame_texture_order.push_back(frame);
    }

    fn load_frame_texture(
        &mut self,
        ctx: &egui::Context,
        frame: usize,
        image: egui::ColorImage,
        error: Option<String>,
    ) -> egui::TextureHandle {
        if let Some(error) = error {
            self.status = format!("Color profile: {error}; showing unconverted values");
        }
        ctx.load_texture(
            format!("frame-{frame}"),
            image,
            egui::TextureOptions::NEAREST,
        )
    }
    fn drawing_axes(&self) -> WrapAxes {
        if self.studio.seamless == WrapAxes::NONE {
            self.doc.wraparound.into()
        } else {
            self.studio.seamless
        }
    }

    fn set_seamless_axes(&mut self, axes: WrapAxes) {
        self.cancel_gesture();
        self.studio.seamless = axes;
        if axes != WrapAxes::NONE {
            self.zoom = self.zoom.min(self.canvas_fit_scale(self.canvas_view_size));
            self.pan = Vec2::ZERO;
            if let Some(split) = self.split_canvas {
                let zoom = split.view.zoom.min(self.canvas_fit_scale(split.view.size));
                if let Some(split) = &mut self.split_canvas {
                    split.view.zoom = zoom;
                    split.view.pan = Vec2::ZERO;
                }
            }
        }
    }

    fn canvas_fit_scale(&self, viewport: Option<Vec2>) -> f32 {
        let view = viewport.unwrap_or(Vec2::splat(512.)) - Vec2::splat(32.);
        let size = Vec2::new(
            self.doc.width as f32
                * if self.studio.seamless.horizontal {
                    3.
                } else {
                    1.
                },
            self.doc.height as f32
                * if self.studio.seamless.vertical {
                    3.
                } else {
                    1.
                },
        ) * self.pixel_aspect_scale();
        let zoom = (view.x / size.x).min(view.y / size.y);
        (if zoom >= 1. { zoom.floor() } else { zoom }).clamp(self.minimum_canvas_zoom(), 32.)
    }

    fn pixel_aspect_scale(&self) -> Vec2 {
        let [width, height] = self.doc.pixel_aspect();
        Vec2::new(f32::from(width), f32::from(height))
    }

    fn canvas_pixel_size(&self) -> Vec2 {
        self.pixel_aspect_scale() * self.zoom
    }

    fn minimum_canvas_zoom(&self) -> f32 {
        let aspect = self.pixel_aspect_scale();
        0.02 / aspect.x.max(aspect.y)
    }

    fn sprite_display_size(&self) -> Vec2 {
        Vec2::new(self.doc.width as f32, self.doc.height as f32) * self.pixel_aspect_scale()
    }

    fn paint_at(&mut self, p: (i32, i32), color: Pixel) {
        self.paint_at_force(p, color, self.studio.force);
    }

    fn paint_at_force(&mut self, p: (i32, i32), color: Pixel, force: f32) {
        self.doc.drawing_index = self.drawing_palette_index(color);
        if let Some(ti) = self.doc.layers[self.layer].tileset {
            let p = self
                .studio
                .seamless
                .position((self.doc.width, self.doc.height), p)
                .unwrap_or(p);
            let t = &self.doc.tilesets[ti];
            let Some((geometry, x, y)) = self.doc.tile_at_point(self.layer, self.frame, p) else {
                self.status = "Point is outside this tilemap cel".into();
                self.doc.drawing_index = None;
                return;
            };
            if let Some(mut mode) = self.tile_pixel_mode {
                let pixel = (
                    (i64::from(p.0) - i64::from(geometry.x)) as usize % t.width,
                    (i64::from(p.1) - i64::from(geometry.y)) as usize % t.height,
                );
                if mode == TilePixelEdit::CreateVariant
                    && let Some(before_stroke) = self
                        .stroke_base
                        .as_ref()
                        .and_then(|doc| doc.tilesets.get(ti))
                {
                    // This stroke owns the fresh IDs it appended. Keep editing
                    // the placement's new tile until the pointer is released.
                    let cel = &self.doc.layers[self.layer].tile_cels[self.frame];
                    let id = (cel[y * geometry.columns + x] & TILE_ID_MASK) as usize;
                    if id > before_stroke.tiles.len() {
                        mode = TilePixelEdit::Shared;
                    }
                }
                let color = if self.tool == Tool::Eraser {
                    CLEAR
                } else {
                    color
                };
                if let Err(error) =
                    self.doc
                        .paint_tile_pixel(self.layer, self.frame, (x, y), pixel, color, mode)
                {
                    self.status = error;
                }
                self.doc.drawing_index = None;
                return;
            }
            let id = if self.tool == Tool::Eraser {
                0
            } else {
                self.selected_tile
            };
            if let Err(e) = self.doc.paint_tile(self.layer, self.frame, x, y, id) {
                self.status = e;
            }
        } else {
            let dynamics = self.brush_dynamics();
            let size = if dynamics.pressure_size {
                (self.brush as f32 * force).round().max(1.) as i32
            } else {
                self.brush
            };
            let mirror_x = self.symmetry_axis(0) - 1 - p.0;
            let mirror_y = self.symmetry_axis(1) - 1 - p.1;
            let xs = [p.0, mirror_x];
            let ys = [p.1, mirror_y];
            let nx = 1 + usize::from(self.studio.mirror_x && mirror_x != p.0);
            let ny = 1 + usize::from(self.studio.mirror_y && mirror_y != p.1);
            for &y in &ys[..ny] {
                for &x in &xs[..nx] {
                    if dynamics.pressure_opacity && self.studio.force_available {
                        self.power_paint_opacity(
                            (x, y),
                            color,
                            size,
                            Some((force * 255.).round() as u8),
                        );
                    } else {
                        self.power_paint((x, y), color, size);
                    }
                }
            }
        }
        self.doc.drawing_index = None;
    }
    fn update_playback_clock(&mut self, now: Instant) -> Duration {
        let duration = |app: &Self| {
            Duration::from_secs_f64(playback_seconds(
                app.doc.durations[app.frame],
                app.playback_speed,
            ))
        };
        // Repaint delivery varies by a few milliseconds around a refresh. For
        // short cels, a small margin avoids a hold followed by a skipped cel.
        const FAST_FRAME: Duration = Duration::from_millis(20);
        const REPAINT_JITTER: Duration = Duration::from_millis(3);

        if now.saturating_duration_since(self.last_tick + duration(self)) > Duration::from_secs(1)
            && !self.playback_waiting_for_frame
        {
            self.last_tick = now;
            return duration(self);
        }
        let mut previous_duration = None;
        loop {
            let step = duration(self);
            let deadline = self.last_tick + step;
            let fast = self.playing && step <= FAST_FRAME;
            // Sample a nearby deadline, but keep the authored clock phase.
            let early_margin =
                if previous_duration.is_none() && fast && !self.playback_waiting_for_frame {
                    REPAINT_JITTER.min(step / 4)
                } else {
                    Duration::ZERO
                };
            if deadline.saturating_duration_since(now) > early_margin {
                return deadline - now;
            }
            if fast
                && previous_duration.is_some_and(|previous| previous <= FAST_FRAME)
                && now.saturating_duration_since(deadline) <= REPAINT_JITTER
            {
                // The current redraw already has a new cel to show. Give it
                // this refresh when its successor is only marginally overdue.
                return Duration::ZERO;
            }
            let Some((next, _)) = self.next_playback_frame() else {
                if now < deadline {
                    return deadline - now;
                }
                self.stop_playback(true);
                return Duration::ZERO;
            };
            if self.playing && self.frame_textures.get(next).is_none_or(Option::is_none) {
                if now < deadline {
                    return deadline - now;
                }
                self.playback_waiting_for_frame = true;
                // The prefetch worker wakes the editor as soon as its result is ready.
                return Duration::from_millis(16);
            }
            self.advance_playback();
            if std::mem::take(&mut self.playback_waiting_for_frame) {
                self.last_tick = now;
                return duration(self);
            }
            self.last_tick = deadline;
            previous_duration = Some(step);
        }
    }
    fn advance_playback(&mut self) {
        let Some(next) = self.next_playback_frame() else {
            self.stop_playback(true);
            return;
        };
        (self.frame, self.playback_reverse) = next;
        if let Some(run) = &mut self.playback_run
            && let Some(subtags) = &mut run.subtags
        {
            subtags.prepare_next(run.once);
        }
        self.timeline_anchor = None;
        self.selected_cels = None;
        self.timeline_frames = TimelineHeaders::default();
        self.timeline_layers = TimelineHeaders::default();
        self.timeline_destination = None;
    }

    fn start_playback(&mut self, selected: bool, once: bool) {
        let origin = self.frame;
        let frames = selected.then(|| {
            if !self.timeline_frames.selected.is_empty() {
                return self.timeline_frames.selected.clone();
            }
            if let Some(CelSelection::Individual(cels)) = &self.selected_cels {
                let mut frames: Vec<_> = cels.iter().map(|&(_, frame)| frame).collect();
                frames.sort_unstable();
                frames.dedup();
                return frames;
            }
            let bounds = self.timeline_range();
            (bounds.first_frame..=bounds.last_frame).collect()
        });
        if let Some(first) = frames.as_ref().and_then(|frames| frames.first()) {
            self.frame = *first;
        } else if let Some(tag) = self.active_tag.and_then(|i| self.doc.tags.get(i)) {
            if once || !(tag.from..=tag.to).contains(&self.frame) {
                self.frame = if matches!(tag.direction, 1 | 3) {
                    tag.to
                } else {
                    tag.from
                };
            }
            self.playback_reverse = matches!(tag.direction, 1 | 3);
        } else if once {
            self.frame = 0;
        }
        self.begin_playback_run(PlaybackRun {
            origin,
            frames,
            subtags: None,
            once,
        });
    }

    fn start_subtag_playback(&mut self, once: bool) {
        let origin = self.frame;
        let frame_count = self.doc.durations.len();
        let mut cursor =
            subtag_playback::SubtagFrames::new(frame_count, &self.doc.tags, self.active_tag);
        let Some(first) = cursor.next() else { return };
        let mut run = SubtagRun {
            cursor,
            following: None,
            frame_count,
        };
        run.prepare_next(once);
        self.frame = first;
        self.playback_reverse = false;
        self.begin_playback_run(PlaybackRun {
            origin,
            frames: None,
            subtags: Some(run),
            once,
        });
    }

    fn begin_playback_run(&mut self, run: PlaybackRun) {
        self.playback_run = Some(run);
        self.playing = true;
        self.playback_waiting_for_frame = false;
        self.last_tick = Instant::now();
    }

    fn stop_playback(&mut self, allow_rewind: bool) {
        if let Some(run) = self.playback_run.take()
            && self.playing
            && allow_rewind
            && self.rewind_on_stop()
        {
            self.frame = run.origin.min(self.doc.durations.len() - 1);
        }
        self.playing = false;
        self.playback_waiting_for_frame = false;
    }

    fn next_playback_frame(&self) -> Option<(usize, bool)> {
        if let Some(subtags) = self
            .playback_run
            .as_ref()
            .and_then(|run| run.subtags.as_ref())
        {
            return subtags
                .following
                .filter(|_| subtags.frame_count == self.doc.durations.len())
                .map(|frame| (frame, false));
        }
        let once = self.playback_run.as_ref().is_some_and(|run| run.once);
        if let Some(frames) = self
            .playback_run
            .as_ref()
            .and_then(|run| run.frames.as_deref())
        {
            let count = self.doc.durations.len();
            let frames = &frames[..frames.partition_point(|&frame| frame < count)];
            let next_index = frames.partition_point(|&frame| frame <= self.frame);
            return frames
                .get(next_index)
                .or_else(|| if once { None } else { frames.first() })
                .map(|&frame| (frame, false));
        }
        let Some(tag) = self.active_tag.and_then(|i| self.doc.tags.get(i)) else {
            return (!once || self.frame + 1 < self.doc.durations.len()).then(|| {
                (
                    (self.frame + 1) % self.doc.durations.len(),
                    self.playback_reverse,
                )
            });
        };
        let reached_end = match tag.direction {
            1 => self.frame <= tag.from,
            2 => self.frame <= tag.from && self.playback_reverse,
            3 => self.frame >= tag.to && !self.playback_reverse,
            _ => self.frame >= tag.to,
        };
        if once && (tag.from == tag.to || reached_end) {
            return None;
        }
        Some(self.step_frame_tag(tag))
    }

    fn step_frame_tag(&self, tag: &FrameTag) -> (usize, bool) {
        let reverse = match tag.direction {
            1 => true,
            2 | 3 => self.playback_reverse,
            _ => false,
        };
        if reverse {
            if self.frame > tag.from {
                (self.frame - 1, self.playback_reverse)
            } else if tag.direction >= 2 {
                ((tag.from + 1).min(tag.to), false)
            } else {
                (tag.to, self.playback_reverse)
            }
        } else if self.frame < tag.to {
            (self.frame + 1, self.playback_reverse)
        } else if tag.direction >= 2 {
            (tag.to.saturating_sub(1).max(tag.from), true)
        } else {
            (tag.from, self.playback_reverse)
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
                self.remember_action("Install palette");
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
                let validated = shortcuts
                    .into_iter()
                    .map(|(binding, action)| {
                        let chord = ShortcutChord::parse(&binding)
                            .ok_or_else(|| format!("Unknown shortcut key {binding}"))?;
                        if !chord.allowed_for(&action) {
                            return Err(format!("{binding} is reserved"));
                        }
                        Ok((chord, binding, action))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                for (chord, binding, action) in validated {
                    self.custom_shortcuts
                        .retain(|key, _| ShortcutChord::parse(key) != Some(chord));
                    self.custom_shortcuts.insert(binding, action);
                }
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
                self.remember_action("Install dithering pattern");
                self.doc.dithering = Some(matrix);
                self.status =
                    "Dithering matrix selected; use Indexed color mode to apply it".into();
            }
            _ => return Err("Unsupported asset kind".into()),
        }
        self.status = format!("Applied {kind} from {id}");
        Ok(())
    }
    fn apply_tile_definition_edit(&mut self, tileset: usize, edit: TileDefinitionEdit) {
        let id = self.selected_tile & TILE_ID_MASK;
        let mut next = self.doc.clone();
        let result = next.edit_tile_definition(tileset, id, edit);
        match result {
            Ok(next_id) => {
                let selected = if matches!(edit, TileDefinitionEdit::Delete) {
                    next_id
                } else {
                    (self.selected_tile & !TILE_ID_MASK) | next_id
                };
                let label = match edit {
                    TileDefinitionEdit::Duplicate => "Duplicate tile",
                    TileDefinitionEdit::Delete => "Delete tile",
                    TileDefinitionEdit::MoveTo(_) => "Reorder tiles",
                };
                self.apply_document_edit(label, Ok(Some(next)));
                self.selected_tile = selected;
            }
            Err(error) => self.status = error,
        }
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
        self.remember_action("Import tileset");
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
    fn slice_controls(&self, ui: &mut egui::Ui, doc: &mut Document) {
        ui.collapsing("Slices and pivots", |ui| {
            if ui.pc_button("Add slice from selection").clicked() {
                let bounds = self.selection_bounds();
                doc.slices.push(SpriteSlice {
                    name: format!("Slice {}", doc.slices.len() + 1),
                    keys: vec![SliceKey {
                        frame: self.frame,
                        bounds,
                        center: None,
                        pivot: None,
                    }],
                });
            }
            let mut remove = None;
            for (index, slice) in doc.slices.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut slice.name);
                        if ui.pc_button("Remove slice").clicked() {
                            remove = Some(index);
                        }
                    });
                    let position = slice
                        .keys
                        .binary_search_by_key(&self.frame, |key| key.frame);
                    if let Ok(position) = position {
                        let key = &mut slice.keys[position];
                        ui.horizontal(|ui| {
                            for (axis, value) in key.bounds.iter_mut().enumerate() {
                                let minimum = if axis < 2 { -16384 } else { 0 };
                                ui.add(
                                    egui::DragValue::new(value)
                                        .range(minimum..=16384)
                                        .prefix(["X ", "Y ", "W ", "H "][axis]),
                                );
                            }
                        });
                        let mut pivot_enabled = key.pivot.is_some();
                        if ui.pc_checkbox(&mut pivot_enabled, "Pivot").changed() {
                            key.pivot = pivot_enabled.then_some([0, 0]);
                        }
                        if let Some(pivot) = &mut key.pivot {
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut pivot[0]).prefix("Pivot X "));
                                ui.add(egui::DragValue::new(&mut pivot[1]).prefix("Pivot Y "));
                            });
                        }
                        if ui.pc_button("Remove current slice key").clicked() {
                            slice.keys.remove(position);
                        }
                    } else if let Err(position) = position
                        && ui.pc_button("Add slice key at this frame").clicked()
                    {
                        let mut key = slice.key_at(self.frame).cloned().unwrap_or(SliceKey {
                            frame: self.frame,
                            bounds: self.selection_bounds(),
                            center: None,
                            pivot: None,
                        });
                        key.frame = self.frame;
                        slice.keys.insert(position, key);
                    }
                });
            }
            if let Some(index) = remove {
                doc.slices.remove(index);
                doc.metadata
                    .retain(|chunk| chunk.owner != format!("slice:{index}"));
                for chunk in &mut doc.metadata {
                    if let Some(id) = chunk
                        .owner
                        .strip_prefix("slice:")
                        .and_then(|id| id.parse::<usize>().ok())
                        && id > index
                    {
                        chunk.owner = format!("slice:{}", id - 1);
                    }
                }
            }
        });
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
        let mut indexed_options = false;
        let mut remove_tag = None;
        egui::Window::new("Sprite properties").open(&mut open).default_width(550.).max_height(660.).vscroll(true).show(ctx,|ui|{
            ui.heading("Color & project");let mut mode=edited.color_mode;
            egui::ComboBox::from_label("Color mode").selected_text(format!("{:?}",mode)).show_ui(ui,|ui|{for value in [ColorMode::Rgba,ColorMode::Indexed,ColorMode::Grayscale]{ui.pc_selectable_value(&mut mode,value,format!("{:?}",value));}});if mode!=edited.color_mode{if mode==ColorMode::Indexed{indexed_options=true;}else{edited.set_color_mode(mode);}}
            if ui.pc_button("Palette and Indexed Color…").clicked(){indexed_options=true;}
            let mut palette_size=edited.palette.len();ui.add(egui::DragValue::new(&mut palette_size).range(1..=256).prefix("Palette entries "));if palette_size!=edited.palette.len(){edited.palette.resize(palette_size,[0,0,0,255]);edited.palette_names.resize(palette_size,String::new());for p in &mut edited.frame_palettes{p.resize(palette_size,[0,0,0,255]);}let mode=edited.color_mode;edited.set_color_mode(mode);self.palette_index=self.palette_index.min(palette_size-1);}
            ui.small(format!("Canvas: {} × {} · Limit: 16384 per side, 128 million pixels across layers and frames",edited.width,edited.height));
            let mut aspect = edited.pixel_aspect();
            ui.horizontal(|ui| {
                ui.label("Pixel aspect ratio");
                ui.add(egui::DragValue::new(&mut aspect[0]).range(1..=255).prefix("Width "));
                ui.add(egui::DragValue::new(&mut aspect[1]).range(1..=255).prefix("Height "));
            });
            if aspect != edited.pixel_aspect() {
                edited.pixel_aspect = Some(aspect);
            }
            ui.collapsing("Drawing grid", |ui| {
                let mut grid = edited.drawing_grid();
                Self::drawing_grid_controls(ui, &mut grid);
                if grid != edited.drawing_grid() {
                    edited.drawing_grid = Some(grid);
                }
            });
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
            if !edited.tilesets.is_empty(){ui.horizontal(|ui|{if ui.pc_button("+ Tile layer from last tileset").clicked(){add_tile=true;}let mut tile_id = self.selected_tile & TILE_ID_MASK;
                    let tile_count = edited.layers[self.layer].tileset
                        .and_then(|index| edited.tilesets.get(index))
                        .or_else(|| edited.tilesets.last())
                        .map_or(1, |tileset| tileset.tiles.len().max(1));
                    if ui.add(egui::DragValue::new(&mut tile_id).range(1..=tile_count as u32).prefix("Paint tile ")).changed() {
                        self.selected_tile = tile_id;
                    }});}
            ui.horizontal_wrapped(|ui| {
                ui.pc_selectable_value(&mut self.tile_pixel_mode, None, "Place tiles");
                ui.pc_selectable_value(&mut self.tile_pixel_mode, Some(TilePixelEdit::Shared), "Edit shared tile pixels");
                ui.pc_selectable_value(&mut self.tile_pixel_mode, Some(TilePixelEdit::ReuseVariant), "Reuse placement variant");
                ui.pc_selectable_value(&mut self.tile_pixel_mode, Some(TilePixelEdit::CreateVariant), "Always create variant")
                    .on_hover_text("Create one independent tile for each changed placement in a stroke");
            });
            self.slice_controls(ui, &mut edited);
            ui.separator();ui.heading("Named animation ranges");ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.tag_name);if ui.pc_button("+ Range").clicked(){edited.tags.push(FrameTag{name:self.tag_name.clone(),from:self.frame,to:self.frame,direction:0, repeat:0,color:[110,170,220]});}});
            for(i,t)in edited.tags.iter_mut().enumerate(){ui.horizontal(|ui|{ui.text_edit_singleline(&mut t.name);let mut from=t.from+1;let mut to=t.to+1;ui.add(egui::DragValue::new(&mut from).range(1..=edited.durations.len()).prefix("From "));ui.add(egui::DragValue::new(&mut to).range(from..=edited.durations.len()).prefix("To "));t.from=from-1;t.to=to.max(from)-1;if ui_icons::button(ui, "trash", "Remove range", false, false).clicked(){remove_tag=Some(i);}});ui.horizontal(|ui|{egui::ComboBox::from_id_salt(("direction",i)).selected_text(["Forward","Reverse","Ping-pong","Ping-pong reverse"][t.direction as usize]).show_ui(ui,|ui|{for (d,name)in ["Forward","Reverse","Ping-pong","Ping-pong reverse"].iter().enumerate(){ui.pc_selectable_value(&mut t.direction,d as u8,*name);}});if ui_icons::button(ui,"tag","Use for playback",self.active_tag==Some(i),false).clicked(){self.active_tag=Some(i);self.frame=if t.direction==1||t.direction==3{t.to}else{t.from};self.playback_reverse=t.direction==3;}});}
            if ui.pc_button("Use all frames for playback").clicked(){self.active_tag=None;}ui.small("Range names, endpoints and playback direction are saved in the project.");
        });
        self.properties_open = open;
        if let Some(i) = remove_tag {
            edited.remove_tag(i);
            self.active_tag = None;
        }
        if edited != self.doc {
            match edited.validate() {
                Ok(()) => {
                    self.remember_action("Edit sprite properties");
                    self.doc = edited;
                }
                Err(e) => self.status = e,
            }
        }
        if indexed_options {
            self.properties_open = false;
            self.open_palette_conversion();
        }
        if add_group {
            self.remember_action("New group");
            match self.doc.add_group("Group") {
                Ok(g) => self.layer = g,
                Err(e) => self.status = e,
            }
        }
        if add_tile {
            self.remember_action("New tile layer");
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
            self.remember_action("Capture selection as tile");
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
    #[cfg(test)]
    fn remember(&mut self) {
        self.remember_action("Edit");
    }

    fn remember_action(&mut self, label: &'static str) {
        self.power.bridge_generation += 1;
        self.redo.clear();
        self.push_undo(self.doc.clone(), label);
    }

    fn push_undo(&mut self, d: Document, label: &'static str) {
        self.undo.push(self.history_entry(d, label));
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
                    if p.backing_id().is_some_and(|id| seen.insert(id)) {
                        bytes = bytes.saturating_add(p.allocation_pixels() * 4);
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
                    if p.backing_id().is_some_and(|id| seen.insert(id)) {
                        bytes = bytes.saturating_add(p.allocation_pixels() * 4);
                    }
                }
            }
            for r in &d.references {
                if r.pixels.backing_id().is_some_and(|id| seen.insert(id)) {
                    bytes = bytes.saturating_add(r.pixels.allocation_pixels() * 4);
                }
            }
            bytes = bytes.saturating_add(d.metadata.iter().map(|m| m.data.len()).sum::<usize>());
        }
        bytes
    }

    fn dialog_open(&self) -> bool {
        self.pending.is_some()
            || self.new_dialog
            || self.settings_open
            || self.rename.is_some()
            || self.replace_color.is_some()
            || self.editor_filter.is_some()
            || self.text_draft.is_some()
            || self.palette_conversion.is_some()
            || self.profile_dialog.is_some()
            || self.selection_tools.open()
            || self.rename_frame.is_some()
            || self.help
            || self.properties_open
            || self.grid_settings.is_some()
            || self.animation_export.is_some()
            || self.file_path.is_some()
            || self.shortcut_edit.is_some()
            || self.shortcut_settings_open
            || self.studio.open
            || self.studio.sheet_open
            || self.studio.tiles_open
            || self.extensions_open
            || (self.script_job.is_some() && !self.script_ui.modeless)
            || self.status.starts_with("Could not")
            || self.status.starts_with("Export failed")
    }
    fn cancel_gesture(&mut self) {
        self.selection_tools.cancel_polygon();
        self.curve_draft = None;
        self.text_draft = None;
        self.spray_stroke = None;
        self.retouch_gesture = None;
        self.studio.marquee_operation = None;
        self.gesture_canvas = None;
        self.gesture_button = None;
        self.start = None;
        self.last = None;
        self.stroke_base = None;
        self.stroke_stabilizer = None;
        self.power.stroke_opacity.clear();
    }

    fn clamp(&mut self) {
        if self.active_tag.is_some_and(|i| i >= self.doc.tags.len()) {
            self.active_tag = None;
        }
        self.layer = self.layer.min(self.doc.layers.len() - 1);
        self.frame = self.frame.min(self.doc.durations.len() - 1);
        self.timeline_anchor = None;
        self.selected_cels = None;
        self.timeline_frames = TimelineHeaders::default();
        self.timeline_layers = TimelineHeaders::default();
        self.timeline_destination = None;
        self.selection = None;
        self.studio.mask = None;
        self.power.reference_edit = None;
        self.studio.polygon.clear();
    }
    fn history_entry(&self, doc: Document, label: &'static str) -> HistoryEntry {
        HistoryEntry {
            label,
            doc,
            selection: self.selection,
            mask: self.studio.mask.clone(),
            polygon: self.studio.polygon.clone(),
            layer: self.layer,
            frame: self.frame,
            selected_tile: self.selected_tile,
            timeline_anchor: self.timeline_anchor,
            selected_cels: self.selected_cels.clone(),
            timeline_frames: self.timeline_frames.clone(),
            timeline_layers: self.timeline_layers.clone(),
        }
    }
    fn restore_history(&mut self, mut entry: HistoryEntry) -> HistoryEntry {
        self.cancel_gesture();
        entry.doc.retain_frame_export_id_counter(&self.doc);
        let previous = std::mem::replace(&mut self.doc, entry.doc);
        let current = self.history_entry(previous, entry.label);
        self.clamp();
        self.layer = entry.layer.min(self.doc.layers.len() - 1);
        self.frame = entry.frame.min(self.doc.durations.len() - 1);
        self.selected_tile = entry.selected_tile;
        self.timeline_anchor = entry.timeline_anchor;
        self.selected_cels = entry.selected_cels;
        self.timeline_frames = entry.timeline_frames;
        self.timeline_layers = entry.timeline_layers;
        self.selection = entry.selection;
        self.studio.mask = entry.mask;
        self.studio.polygon = entry.polygon;
        current
    }

    fn history_label(&self, position: usize) -> Option<&'static str> {
        if position == 0 {
            Some("Oldest retained state")
        } else if position <= self.undo.len() {
            self.undo.get(position - 1).map(|entry| entry.label)
        } else {
            let index = (self.undo.len() + self.redo.len()).checked_sub(position)?;
            self.redo.get(index).map(|entry| entry.label)
        }
    }

    fn jump_history(&mut self, position: usize) -> bool {
        if position == self.undo.len() || position > self.undo.len() + self.redo.len() {
            return false;
        }
        self.power.bridge_generation += 1;
        self.stop_playback(false);
        while self.undo.len() > position {
            let entry = self.undo.pop().expect("history target is valid");
            let previous = self.restore_history(entry);
            self.redo.push(previous);
        }
        while self.undo.len() < position {
            let entry = self.redo.pop().expect("history target is valid");
            let previous = self.restore_history(entry);
            self.undo.push(previous);
        }
        true
    }

    fn undo(&mut self) {
        if self.jump_history(self.undo.len().saturating_sub(1)) {
            self.status = "Undo".into();
        } else {
            self.power.bridge_generation += 1;
        }
    }

    fn redo(&mut self) {
        if self.jump_history(self.undo.len() + 1) {
            self.status = "Redo".into();
        } else {
            self.power.bridge_generation += 1;
        }
    }

    fn history_panel(&mut self, ctx: &egui::Context) {
        if !self.history_open {
            return;
        }
        let mut open = true;
        let mut jump = None;
        let can_jump = !self.dialog_open() && self.script_job.is_none();
        let current = self.undo.len();
        let total = current + self.redo.len();
        egui::Window::new("Undo History")
            .open(&mut open)
            .default_width(275.)
            .show(ctx, |ui| {
                ui.set_min_width(220.);
                let scroll_key = ui.id().with("current-history-entry");
                let view = (self.active_tab, current, total);
                let scroll_to_current = ui.data_mut(|data| {
                    let changed = data.get_temp::<(usize, usize, usize)>(scroll_key) != Some(view);
                    data.insert_temp(scroll_key, view);
                    changed
                });
                egui::ScrollArea::vertical()
                    .max_height(360.)
                    .show(ui, |ui| {
                        for position in 0..=total {
                            let label = self
                                .history_label(position)
                                .expect("history position is valid");
                            let response = ui.add_enabled(
                                can_jump,
                                egui::Button::selectable(position == current, label),
                            );
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new(("history-row", position)),
                                    response.rect,
                                )
                            });
                            if position == current && scroll_to_current {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() && position != current {
                                jump = Some((position, label));
                            }
                        }
                    });
            });
        self.history_open = open;
        if let Some((position, label)) = jump
            && self.jump_history(position)
        {
            self.status = format!("Restored history entry · {label}");
        }
    }

    fn replace(&mut self, d: Document, path: Option<PathBuf>) {
        self.power.bridge_generation += 1;
        self.cancel_gesture();
        self.split_canvas = None;
        self.active_canvas = CanvasPane::Primary;
        #[cfg(test)]
        {
            self.extension_session = None;
        }
        self.active_tag = None;
        self.preview_playback = None;
        self.palette_index = 0;
        self.render_snapshot = None;
        self.last_export = None;
        self.doc = d;
        self.saved = self.doc.clone();
        self.path = path;
        self.live_save_root = None;
        self.live_save_root_identity = None;
        self.layer = 0;
        self.frame = 0;
        self.selected_tile = 1;
        self.timeline_anchor = None;
        self.selected_cels = None;
        self.timeline_frames = TimelineHeaders::default();
        self.timeline_layers = TimelineHeaders::default();
        self.timeline_destination = None;
        self.undo.clear();
        self.redo.clear();
        self.selection = None;
        self.studio.mask = None;
        self.studio.symmetry_axes = [None; 2];
        self.selection_tools.previous = None;
        self.power.reference_edit = None;
        self.studio.polygon.clear();
        self.pan = Vec2::ZERO;
        let display = self.sprite_display_size();
        self.zoom = (480. / display.x.max(display.y)).floor().clamp(1., 16.);
        self.stop_playback(false);
    }
    fn open_document(&mut self, doc: Document, path: Option<PathBuf>) {
        if self.script_job.is_some() {
            self.status = "Wait for the current script before opening another tab".into();
            return;
        }
        self.insert_document_tab(doc, path);
    }

    fn insert_document_tab(&mut self, doc: Document, path: Option<PathBuf>) {
        self.cancel_gesture();
        let tab = SpriteTab {
            doc: self.doc.clone(),
            last_export: self.last_export.take(),
            saved: self.saved.clone(),
            path: self.path.clone(),
            live_save_root: self.live_save_root.clone(),
            live_save_root_identity: self.live_save_root_identity,
            layer: self.layer,
            frame: self.frame,
            selected_tile: self.selected_tile,
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            zoom: self.zoom,
            pan: self.pan,
            split_canvas: self.split_canvas,
            active_canvas: self.active_canvas,
            selection: self.selection,
            palette_index: self.palette_index,
            secondary_palette_index: self.secondary_palette_index,
            active_tag: self.active_tag,
            timeline_anchor: self.timeline_anchor,
            selected_cels: self.selected_cels.clone(),
            timeline_frames: self.timeline_frames.clone(),
            timeline_layers: self.timeline_layers.clone(),
            polygon: self.studio.polygon.clone(),
            mask: self.studio.mask.clone(),
            symmetry_axes: self.studio.symmetry_axes,
            previous_selection: self.selection_tools.previous.take(),
        };
        self.tabs[self.active_tab] = Some(tab);
        self.active_tab = self.tabs.len();
        self.tabs.push(None);
        self.replace(doc, path);
    }
    fn switch_tab(&mut self, index: usize) {
        if self.script_job.is_none() {
            self.select_document_tab(index);
        }
    }

    fn select_document_tab(&mut self, index: usize) {
        if index == self.active_tab || index >= self.tabs.len() {
            return;
        }
        self.cancel_gesture();
        self.power.bridge_generation += 1;
        let mut tab = self.tabs[index].take().unwrap();
        std::mem::swap(&mut self.doc, &mut tab.doc);
        std::mem::swap(&mut self.last_export, &mut tab.last_export);
        std::mem::swap(&mut self.saved, &mut tab.saved);
        std::mem::swap(&mut self.path, &mut tab.path);
        std::mem::swap(&mut self.live_save_root, &mut tab.live_save_root);
        std::mem::swap(
            &mut self.live_save_root_identity,
            &mut tab.live_save_root_identity,
        );
        std::mem::swap(&mut self.layer, &mut tab.layer);
        std::mem::swap(&mut self.frame, &mut tab.frame);
        std::mem::swap(&mut self.selected_tile, &mut tab.selected_tile);
        std::mem::swap(&mut self.undo, &mut tab.undo);
        std::mem::swap(&mut self.redo, &mut tab.redo);
        std::mem::swap(&mut self.zoom, &mut tab.zoom);
        std::mem::swap(&mut self.pan, &mut tab.pan);
        std::mem::swap(&mut self.split_canvas, &mut tab.split_canvas);
        std::mem::swap(&mut self.active_canvas, &mut tab.active_canvas);
        std::mem::swap(&mut self.selection, &mut tab.selection);
        std::mem::swap(&mut self.studio.polygon, &mut tab.polygon);
        std::mem::swap(&mut self.studio.mask, &mut tab.mask);
        std::mem::swap(&mut self.studio.symmetry_axes, &mut tab.symmetry_axes);
        std::mem::swap(
            &mut self.selection_tools.previous,
            &mut tab.previous_selection,
        );
        self.power.reference_edit = None;
        std::mem::swap(&mut self.palette_index, &mut tab.palette_index);
        std::mem::swap(
            &mut self.secondary_palette_index,
            &mut tab.secondary_palette_index,
        );
        std::mem::swap(&mut self.active_tag, &mut tab.active_tag);
        std::mem::swap(&mut self.timeline_anchor, &mut tab.timeline_anchor);
        std::mem::swap(&mut self.selected_cels, &mut tab.selected_cels);
        std::mem::swap(&mut self.timeline_frames, &mut tab.timeline_frames);
        std::mem::swap(&mut self.timeline_layers, &mut tab.timeline_layers);
        self.timeline_destination = None;
        self.tabs[self.active_tab] = Some(tab);
        self.active_tab = index;
        self.preview_playback = None;
        #[cfg(test)]
        {
            self.extension_session = None;
        }
        self.render_snapshot = None;
        self.stop_playback(false);
        self.hover = None;
        self.status = "Switched sprite".into();
    }
    fn request(&mut self, p: Pending) {
        if self.script_job.is_some() && matches!(p, Pending::CloseTab) {
            self.status = "Wait for the current script before closing a tab".into();
            return;
        }
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
            Pending::New => {
                (self.new_w, self.new_h) = self.power.new_sprite_size();
                self.new_dialog = true;
            }
            Pending::Demo => {
                self.open_document(Document::demo(), None);
                self.layer = 1;
            }
            Pending::Open => self.begin_file_path(FilePathPurpose::Open, String::new()),
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
            if let Some(index) = self.tabs.iter().position(|tab| {
                tab.as_ref()
                    .is_some_and(|tab| tab.doc.differs_from_saved(&tab.saved))
            }) {
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
        if self.script_job.is_some() {
            self.status = "Wait for the current script before saving the canvas".into();
            return false;
        }
        if self.save_job.is_some() {
            self.status = "A save is already running".into();
            return false;
        }
        if !as_new && let Some(path) = self.path.clone() {
            return self.save_to_path(path);
        }
        let suggested = self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled.pixel".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.begin_file_path(FilePathPurpose::Save, suggested);
        false
    }

    fn save_to_path(&mut self, mut path: PathBuf) -> bool {
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
        if !saving.has_frame_names()
            && !saving.references.is_empty()
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
        self.save_job = Some(SaveJob {
            tab: self.active_tab,
            finished: rx,
        });
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
        let Some(job) = &self.save_job else {
            return;
        };
        let tab = job.tab;
        let result = job.finished.try_recv();
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
                    if self.path.as_ref() != Some(&path) {
                        self.live_save_root = None;
                        self.live_save_root_identity = None;
                    }
                    self.path = Some(path);
                    self.saved = saving;
                } else if let Some(Some(tab)) = self.tabs.get_mut(tab) {
                    if tab.path.as_ref() != Some(&path) {
                        tab.live_save_root = None;
                        tab.live_save_root_identity = None;
                    }
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
                    self.remember_action("Apply save format conversion");
                    self.doc = saving;
                }
                self.status = format!(
                    "Saved {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                self.add_recent(path.clone());
                if self.path.as_ref() != Some(&path) {
                    self.live_save_root = None;
                    self.live_save_root_identity = None;
                }
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
            "baked-ase" => "aseprite",
            "gif" => "gif",
            "mp4" => "mp4",
            "fli" => "fli",
            "flc" => "flc",
            "bmp" => "bmp",
            "jpeg" => "jpg",
            "ico" => "ico",
            "pcx" => "pcx",
            "qoi" => "qoi",
            "tga" => "tga",
            "webp" => "webp",
            "svg" => "svg",
            "css" => "css",
            _ => "png",
        };
        let suggested = match kind {
            "png-sequence" => String::new(),
            "baked-ase" => "Baked artwork.aseprite".into(),
            _ => format!("sprite.{ext}"),
        };
        self.begin_file_path(
            FilePathPurpose::Export {
                kind: kind.into(),
                extension: ext,
            },
            suggested,
        );
    }
    fn start_export(&mut self, kind: &str, path: PathBuf) {
        let scale = self.studio.sheet.scale;
        let kind = match kind {
            "png-sequence" => ExportKind::Sequence { scale },
            "gif" => ExportKind::Animation {
                format: "gif",
                scale,
            },
            "mp4" => ExportKind::Animation {
                format: "mp4",
                scale,
            },
            "fli" => ExportKind::Animation {
                format: "fli",
                scale,
            },
            "flc" => ExportKind::Animation {
                format: "flc",
                scale,
            },
            "baked-ase" => ExportKind::BakedAse,
            _ => ExportKind::Raster {
                frame_id: self.doc.frame_export_id(self.frame),
                scale,
                sheet: kind == "sheet",
            },
        };
        self.start_export_recipe(ExportRecipe { path, kind }, false);
    }
    fn copy(&mut self) {
        self.copy_pixels(false);
    }

    fn selection_clipboard(&self, merged: bool) -> Option<Clipboard> {
        let (x0, y0, x1, y1) =
            self.selection
                .unwrap_or((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1));
        let (x0, y0, x1, y1) = (
            x0.min(x1).max(0),
            y0.min(y1).max(0),
            x0.max(x1).min(self.doc.width as i32 - 1),
            y0.max(y1).min(self.doc.height as i32 - 1),
        );
        if x0 > x1 || y0 > y1 {
            return None;
        }
        let composite = merged.then(|| self.doc.composite(self.frame));
        let w = (x1 - x0 + 1) as usize;
        let h = (y1 - y0 + 1) as usize;
        let mut pixels = Vec::with_capacity(w * h);
        let mut selected_pixels =
            Vec::with_capacity(if self.selection.is_some() { w * h } else { 0 });
        for y in y0..=y1 {
            for x in x0..=x1 {
                let selected = self.selected(x, y);
                if self.selection.is_some() {
                    selected_pixels.push(selected);
                }
                pixels.push(if !selected {
                    CLEAR
                } else if let Some(composite) = &composite {
                    composite[y as usize * self.doc.width + x as usize]
                } else {
                    self.doc
                        .sample_layer(self.layer, self.frame, x, y)
                        .unwrap_or(CLEAR)
                });
            }
        }
        if self.selection.is_some() && !selected_pixels.contains(&true) {
            return None;
        }
        Some(Clipboard {
            w,
            h,
            pixels,
            selection: self.selection.map(|_| ClipboardMask {
                origin: [x0, y0],
                selected: selected_pixels,
            }),
            palette: Some(self.doc.palette_for(self.frame).to_vec()),
            tileset: None,
        })
    }

    fn copy_pixels(&mut self, merged: bool) {
        let Some(clip) = self.selection_clipboard(merged) else {
            self.status = "No pixels selected".into();
            return;
        };
        self.clipboard_target = None;
        self.script_clipboard_text = None;
        self.status = if merged {
            "Copied visible artwork"
        } else {
            "Copied pixels from the active layer"
        }
        .into();
        if self.shot.is_none() && !cfg!(test) {
            let result = self
                .system_clipboard
                .copy(clip.w, clip.h, clip.pixels.clone());
            if let Err(e) = result {
                self.status = format!("Copied internally; system clipboard: {e}");
            }
        }
        self.clipboard = Some(clip);
    }

    fn paste(&mut self) {
        self.paste_as(PasteMode::InPlace);
    }

    fn paste_as(&mut self, mode: PasteMode) {
        if mode == PasteMode::InPlace
            && (!self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some())
        {
            return;
        }
        if self.shot.is_none() && !cfg!(test) {
            self.clipboard_target = None;
            match self.system_clipboard.paste() {
                Ok(()) => {
                    self.clipboard_target = Some(self.clipboard_destination(mode));
                    self.status = "Reading system clipboard…".into();
                }
                Err(e) => {
                    self.paste_internal(mode);
                    self.status = format!("{e}; using internal clipboard");
                }
            }
            return;
        }
        self.paste_internal(mode);
    }
    fn clipboard_destination(&self, mode: PasteMode) -> ClipboardTarget {
        ClipboardTarget {
            mode,
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
            if let Some(reply) = self.script_clipboard_reply.take() {
                let response = match result {
                    system_clipboard::Completion::Copy(result) => {
                        if result.is_err() {
                            self.script_ui.use_system_clipboard = Some(false);
                        }
                        serde_json::json!({})
                    }
                    system_clipboard::Completion::Paste(Ok((w, h, pixels))) => {
                        self.remember_native_image(w, h, pixels);
                        self.read_script_clipboard(false)
                    }
                    system_clipboard::Completion::Paste(Err(error)) => {
                        self.script_clipboard_fallback(error, false)
                    }
                    system_clipboard::Completion::Text(Ok(text)) => {
                        self.script_clipboard_text = Some(text);
                        self.read_script_clipboard(true)
                    }
                    system_clipboard::Completion::Text(Err(error)) => {
                        self.script_clipboard_fallback(error, true)
                    }
                };
                let _ = reply.send(response);
            } else {
                self.apply_native_clipboard(result);
            }
        }
        if self.system_clipboard.busy() {
            ctx.request_repaint_after(Duration::from_millis(25));
        }
    }

    fn apply_native_clipboard(&mut self, result: system_clipboard::Completion) {
        match result {
            system_clipboard::Completion::Copy(Err(e)) => {
                self.status = format!("Copied internally; {e}")
            }
            system_clipboard::Completion::Copy(Ok(())) | system_clipboard::Completion::Text(_) => {}
            system_clipboard::Completion::Paste(result) => {
                if let Some(target) = self.clipboard_target.take()
                    && target == self.clipboard_destination(target.mode)
                {
                    match result {
                        Ok((w, h, pixels)) => {
                            self.script_clipboard_text = None;
                            self.remember_native_image(w, h, pixels);
                            self.paste_internal(target.mode);
                        }
                        Err(e) => {
                            if !e.contains("timed out") && !e.contains("exceeds limits") {
                                self.paste_internal(target.mode);
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
    fn remember_native_image(&mut self, w: usize, h: usize, pixels: Vec<Pixel>) {
        if !self
            .clipboard
            .as_ref()
            .is_some_and(|previous| previous.matches_native(w, h, &pixels))
        {
            self.clipboard = Some(Clipboard::raster(w, h, pixels));
        }
    }

    fn script_clipboard_fallback(&self, error: String, text: bool) -> serde_json::Value {
        if error.contains("timed out") || error.contains("exceeds limits") {
            serde_json::json!({"error":error})
        } else {
            self.read_script_clipboard(text)
        }
    }

    fn read_script_clipboard(&self, text: bool) -> serde_json::Value {
        if text {
            return serde_json::json!({"text":self.script_clipboard_text});
        }
        match self.clipboard.as_ref() {
            Some(image) if image.w.saturating_mul(image.h) > 1_000_000 => {
                serde_json::json!({"error":"Clipboard image exceeds the Lua pixel budget"})
            }
            image => serde_json::json!({"image":image}),
        }
    }

    fn handle_script_clipboard(&mut self, request: script_ui::Request) {
        let native = self.script_ui.use_system_clipboard == Some(true);
        let format = request.value["format"].as_str().unwrap_or("image");
        let write = request.value["op"] == "clipboardWrite";
        let text = format == "text";
        let result = if write {
            self.write_script_clipboard(&request.value, format, native)
        } else if native {
            let job = if text {
                self.system_clipboard.read_text()
            } else {
                self.system_clipboard.paste()
            };
            Ok(job.is_ok())
        } else {
            Ok(false)
        };
        if write && result.is_ok() {
            self.clipboard_target = None;
        }
        match result {
            Ok(true) => {
                self.clipboard_target = None;
                self.script_clipboard_reply = Some(request.reply);
            }
            Ok(false) => {
                if write && native {
                    self.script_ui.use_system_clipboard = Some(false);
                }
                let response = if write {
                    serde_json::json!({})
                } else {
                    self.read_script_clipboard(text)
                };
                let _ = request.reply.send(response);
            }
            Err(error) => {
                let _ = request.reply.send(serde_json::json!({"error":error}));
            }
        }
    }

    fn write_script_clipboard(
        &mut self,
        value: &serde_json::Value,
        format: &str,
        native: bool,
    ) -> Result<bool, String> {
        match format {
            "image" => {
                let image = Self::decode_script_clipboard(&value["image"])?;
                let started = native
                    && self
                        .system_clipboard
                        .copy(image.w, image.h, image.native_pixels())
                        .is_ok();
                self.clipboard = Some(image);
                self.script_clipboard_text = None;
                Ok(started)
            }
            "text" => {
                let text = value["text"].as_str().ok_or("Expected clipboard text")?;
                if text.len() > system_clipboard::MAX_SCRIPT_TEXT_BYTES || text.contains('\0') {
                    return Err(
                        "Clipboard text contains a null byte or exceeds the Lua limit".into(),
                    );
                }
                let started = native && self.system_clipboard.write_text(text.to_owned()).is_ok();
                self.script_clipboard_text = Some(text.to_owned());
                self.clipboard = None;
                Ok(started)
            }
            "clear" => {
                let started = native && self.system_clipboard.clear().is_ok();
                self.clipboard = None;
                self.script_clipboard_text = None;
                Ok(started)
            }
            _ => Err("Unsupported clipboard format".into()),
        }
    }

    fn decode_script_clipboard(value: &serde_json::Value) -> Result<Clipboard, String> {
        let image: Clipboard = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if image.w == 0
            || image.h == 0
            || image.w > MAX_DIMENSION
            || image.h > MAX_DIMENSION
            || image.w.saturating_mul(image.h) != image.pixels.len()
            || image.pixels.len() > 1_000_000
        {
            return Err("Invalid clipboard image or Lua clipboard pixel budget exceeded".into());
        }
        if image.selection.as_ref().is_some_and(|mask| {
            mask.selected.len() != image.pixels.len()
                || mask.origin[0].checked_add(image.w as i32 - 1).is_none()
                || mask.origin[1].checked_add(image.h as i32 - 1).is_none()
        }) || image
            .palette
            .as_ref()
            .is_some_and(|palette| palette.is_empty() || palette.len() > 256)
        {
            return Err("Invalid clipboard selection or palette".into());
        }
        if image.tileset.as_ref().is_some_and(|tileset| {
            let set = &tileset.set;
            let area = set.width.saturating_mul(set.height);
            set.width == 0
                || set.height == 0
                || set.width > MAX_DIMENSION
                || set.height > MAX_DIMENSION
                || set.name.len() > u16::MAX as usize
                || tileset.flags & 1 != 0
                || tileset.user_data.len() > set.tiles.len() + 2
                || tileset.user_data.iter().any(|chunk| chunk.len() < 4)
                || tileset.external.iter().any(|chunk| chunk.len() < 12)
                || tileset
                    .user_data
                    .iter()
                    .chain(&tileset.external)
                    .map(Vec::len)
                    .sum::<usize>()
                    > extensions::MAX_TILESET_METADATA_BYTES
                || area
                    .saturating_mul(set.tiles.len())
                    .saturating_add(image.pixels.len())
                    > 1_000_000
                || set.tiles.iter().any(|pixels| pixels.len() != area)
        }) {
            return Err("Invalid clipboard tileset or Lua clipboard pixel budget exceeded".into());
        }
        Ok(image)
    }

    fn paste_raster(
        doc: &mut Document,
        layer: usize,
        frame: usize,
        origin: (i32, i32),
        clip: &Clipboard,
    ) {
        for cy in 0..clip.h {
            for cx in 0..clip.w {
                if let Some(i) = doc.index(
                    origin.0.saturating_add(cx as i32),
                    origin.1.saturating_add(cy as i32),
                ) {
                    let p = clip.visible_pixel(cy * clip.w + cx);
                    if p[3] == 0 {
                        continue;
                    }
                    let color = over(doc.layers[layer].cels[frame][i], p, 255);
                    doc.set_pixel(layer, frame, i, color);
                }
            }
        }
    }

    fn apply_script_paste(request: &serde_json::Value) -> Result<serde_json::Value, String> {
        let mut doc: Document =
            serde_json::from_value(request["document"].clone()).map_err(|e| e.to_string())?;
        doc.validate()?;
        let clip = Self::decode_script_clipboard(&request["image"])?;
        let (layer, frame): (usize, usize) =
            serde_json::from_value(request["site"].clone()).map_err(|e| e.to_string())?;
        let origin =
            serde_json::from_value(request["origin"].clone()).map_err(|e| e.to_string())?;
        if frame >= doc.durations.len()
            || !doc.editable(layer)
            || doc.layers[layer].tileset.is_some()
        {
            return Err("Choose an editable image layer and frame".into());
        }
        Self::paste_raster(&mut doc, layer, frame, origin, &clip);
        Ok(serde_json::json!({"document":extensions::script_document_value(&doc)?}))
    }

    fn paste_internal(&mut self, mode: PasteMode) {
        let Some(clip) = self.clipboard.take() else {
            self.status = "No image in the clipboard".into();
            return;
        };
        self.paste_image(mode, &clip);
        self.clipboard = Some(clip);
    }

    fn paste_image(&mut self, mode: PasteMode, clip: &Clipboard) {
        let (x, y, _, _) = self.selection.unwrap_or((0, 0, 0, 0));
        match mode {
            PasteMode::InPlace => {
                if self.doc.editable(self.layer) && self.doc.layers[self.layer].tileset.is_none() {
                    self.remember_action("Paste");
                    Self::paste_raster(&mut self.doc, self.layer, self.frame, (x, y), clip);
                    self.status = "Pasted pixels".into();
                }
            }
            PasteMode::Sprite => {
                let mut doc = Document::new(clip.w, clip.h);
                if let Some(palette) = &clip.palette {
                    doc.palette.clone_from(palette);
                }
                doc.layers[0].cels[0] = clip.native_pixels().into();
                self.open_derived_sprite(doc, "Pasted as a new sprite");
            }
            PasteMode::Layer => {
                if !self.doc.can_add_layer() {
                    self.status = "Layer limit reached for this canvas size".into();
                    return;
                }
                let mut next = self.doc.clone();
                next.add_layer();
                let layer = next.layers.len() - 1;
                next.layers[layer].name = "Pasted layer".into();
                for index in 0..clip.pixels.len() {
                    let pixel = clip.visible_pixel(index);
                    if pixel[3] == 0 {
                        continue;
                    }
                    let px = x.saturating_add((index % clip.w) as i32);
                    let py = y.saturating_add((index / clip.w) as i32);
                    if let Some(i) = next.index(px, py) {
                        next.set_pixel(layer, self.frame, i, pixel);
                    }
                }
                if let Err(error) = next.validate() {
                    self.status = error;
                    return;
                }
                self.remember_action("Paste as new layer");
                self.doc = next;
                self.layer = layer;
                self.status = "Pasted as a new layer".into();
            }
            PasteMode::Reference => {
                let mut next = self.doc.clone();
                next.references
                    .push(pixelcraft::productivity::ReferenceImage {
                        behind: false,
                        floating: false,
                        name: "Pasted reference".into(),
                        width: clip.w,
                        height: clip.h,
                        pixels: clip.native_pixels().into(),
                        x: x as f32,
                        y: y as f32,
                        scale: 1.,
                        opacity: 255,
                        visible: true,
                        options: Default::default(),
                    });
                match next.validate() {
                    Ok(()) => {
                        self.remember_action("Paste as reference");
                        self.doc = next;
                        self.status = "Pasted as a reference image".into();
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }

    fn open_derived_sprite(&mut self, doc: Document, status: &str) {
        if self.script_job.is_some() {
            self.status = "Wait for the current script before opening another tab".into();
            return;
        }
        let blank = Document::new(doc.width, doc.height);
        self.open_document(doc, None);
        self.saved = blank;
        self.status = status.into();
    }

    fn new_sprite_from_selection(&mut self) {
        let Some(clip) = self.selection.and_then(|_| self.selection_clipboard(false)) else {
            self.status = "Select pixels to create a sprite".into();
            return;
        };
        let mut doc = Document::new(clip.w, clip.h);
        doc.palette = clip.palette.unwrap_or_else(default_palette);
        doc.layers[0].cels[0] = clip.pixels.into();
        doc.color_profile.clone_from(&self.doc.color_profile);
        doc.pixel_aspect = self.doc.pixel_aspect;
        let source = &self.doc.layers[self.layer];
        if self.doc.color_mode == ColorMode::Indexed && !source.background {
            doc.transparent_index = self.doc.transparent_index;
            if let Some(indices) = &source.cel_meta[self.frame].indices {
                let mask = clip.selection.unwrap();
                let [x, y] = mask.origin;
                doc.layers[0].cel_meta[0].indices = Some(
                    mask.selected
                        .iter()
                        .enumerate()
                        .map(|(i, selected)| {
                            if *selected {
                                indices[(y as usize + i / clip.w) * self.doc.width
                                    + x as usize
                                    + i % clip.w]
                            } else {
                                doc.transparent_index as u8
                            }
                        })
                        .collect::<Vec<_>>()
                        .into(),
                );
            }
            doc.set_color_mode(ColorMode::Indexed);
        } else if self.doc.color_mode == ColorMode::Grayscale {
            doc.color_mode = ColorMode::Grayscale;
        }
        self.open_derived_sprite(doc, "Created a sprite from the selection");
    }
    fn clear(&mut self) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            return;
        }
        self.remember_action("Clear pixels");
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
        self.canvas_transform = None;
        self.cancel_gesture();
        if self.studio_action(s) {
            return;
        }
        match s {
            "canvas-transform" => self.start_canvas_transform(),
            "shortcuts" => self.shortcut_settings_open = true,
            "new" => self.request(Pending::New),
            "closetab" => self.request(Pending::CloseTab),
            "open" => self.request(Pending::Open),
            "import-layer" => self.begin_file_path(FilePathPurpose::ImportLayer, String::new()),
            "save" => {
                self.save(false);
            }
            "export-baked-ase" => self.export("baked-ase"),
            "repeat-export" => self.repeat_export(),
            "saveas" => {
                self.save(true);
            }
            "png" | "bmp" | "jpeg" | "ico" | "pcx" | "qoi" | "tga" | "webp" | "svg" | "css"
            | "sheet" => self.export(s),
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
            "fli" => self.animation_export = Some("fli"),
            "flc" => self.animation_export = Some("flc"),
            "undo" => self.undo(),
            "undo-history" => self.history_open = !self.history_open,
            "redo" => self.redo(),
            "copy" => self.copy(),
            "copy-merged" => self.copy_pixels(true),
            "paste" => self.paste(),
            "paste-as-sprite" => self.paste_as(PasteMode::Sprite),
            "paste-as-layer" => self.paste_as(PasteMode::Layer),
            "paste-as-reference" => self.paste_as(PasteMode::Reference),
            "new-sprite-from-selection" => self.new_sprite_from_selection(),
            "cut" => {
                self.copy();
                self.clear()
            }
            "clear" => self.clear(),
            "filters" => self.open_filters(),
            "palette-conversion" => self.open_palette_conversion(),
            "color-profile" => self.open_profile_dialog(),
            "replace-color" => {
                self.stop_playback(false);
                self.replace_color = Some(ReplaceColor {
                    source: self.fg,
                    replacement: self.bg,
                    all_layers: false,
                    all_frames: false,
                    selection_only: self.selection.is_some(),
                });
            }
            "selectall" => {
                self.studio.mask = None;
                self.studio.polygon.clear();
                self.selection = Some((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1))
            }
            "deselect" => {
                self.deselect_and_remember();
            }
            "reselect" => self.reselect_previous(),
            "invert-selection" => self.invert_canvas_selection(),
            "modify-selection" => self.open_selection_tools(false),
            "color-range" => self.open_selection_tools(true),
            "import-selection-mask" => self.import_selection_mask(),
            "export-selection-mask" => self.export_selection_mask(),
            "layer" => {
                if self.doc.can_add_layer() {
                    self.remember_action("New layer");
                    self.doc.add_layer();
                    self.layer = self.doc.layers.len() - 1;
                    self.timeline_anchor = None;
                    self.selected_cels = None;
                    self.timeline_frames = TimelineHeaders::default();
                    self.timeline_layers = TimelineHeaders::default();
                    self.timeline_destination = None;
                } else {
                    self.status = "Layer limit reached for this canvas size".into()
                }
            }
            "deletelayer" if self.doc.layers.len() > 1 => {
                self.remember_action("Delete layer");
                self.doc.remove_layer(self.layer);
                self.clamp();
            }
            "deletelayer" => {}
            "up" | "down" => {
                let to = if s == "up" {
                    (self.layer + 1).min(self.doc.layers.len() - 1)
                } else {
                    self.layer.saturating_sub(1)
                };
                if to != self.layer {
                    self.remember_action("Move layer");
                    self.doc.swap_layers(self.layer, to);
                    self.layer = to;
                    self.timeline_layers = TimelineHeaders::default();
                    self.selected_cels = None;
                    self.timeline_anchor = None;
                    self.timeline_destination = None;
                }
            }
            "frame" | "blank" => {
                if self.doc.can_add_frame(self.frame, s == "frame") {
                    self.remember_action(if s == "frame" {
                        "Duplicate frame"
                    } else {
                        "New empty frame"
                    });
                    self.doc.add_frame(self.frame, s == "frame");
                    self.frame += 1;
                    self.timeline_anchor = None;
                    self.selected_cels = None;
                    self.timeline_frames = TimelineHeaders::default();
                    self.timeline_layers = TimelineHeaders::default();
                    self.timeline_destination = None;
                } else {
                    self.status = "Frame limit reached for this canvas size".into()
                }
            }
            "deleteframe" if self.doc.durations.len() > 1 => {
                self.remember_action("Delete frame");
                self.doc.remove_frame(self.frame);
                self.clamp();
            }
            "deleteframe" => {}
            "reverse-frames" => self.reverse_timeline_frames(),
            "play" if self.playing => self.stop_playback(true),
            "play" => self.start_playback(false, false),
            "play-once" => self.start_playback(false, true),
            "play-selection" => self.start_playback(true, false),
            "play-selection-once" => self.start_playback(true, true),
            "play-subtags" => self.start_subtag_playback(false),
            "play-subtags-once" => self.start_subtag_playback(true),
            "rewind-on-stop" => self.toggle_rewind_on_stop(),
            "prev" | "next" => {
                let count = self.doc.durations.len();
                let offset = if s == "next" { 1 } else { count - 1 };
                self.frame = (self.frame + offset) % count;
                self.timeline_frames = TimelineHeaders::default();
                self.timeline_layers = TimelineHeaders::default();
                self.selected_cels = None;
            }
            "flipx" | "flipy" => {
                self.remember_action(if s == "flipx" {
                    "Flip horizontally"
                } else {
                    "Flip vertically"
                });
                self.doc.flip(self.layer, self.frame, s == "flipx")
            }
            "settings" => self.settings_open = true,
            "grid" => self.grid = !self.grid,
            "drawing-grid" => self.show_drawing_grid = !self.show_drawing_grid,
            "snap-to-grid" => self.snap_to_grid = !self.snap_to_grid,
            "drawing-grid-settings" => self.grid_settings = Some(self.doc.drawing_grid()),
            "grid-from-selection" => {
                if self.selection.is_some()
                    && self
                        .studio
                        .mask
                        .as_ref()
                        .is_none_or(|mask| mask.iter().any(|&pixel| pixel))
                {
                    let [x, y, w, h] = self.selection_bounds();
                    self.set_drawing_grid(DrawingGrid {
                        origin: [x, y],
                        size: [w as u32, h as u32],
                    });
                } else {
                    self.status = "Select an area to set the drawing grid".into();
                }
            }
            "onion" => self.onion = !self.onion,
            "split-canvas" => self.toggle_split_canvas(),
            "fit" => {
                let view = self.active_canvas_view();
                let zoom = if self.split_canvas.is_none() && self.studio.seamless == WrapAxes::NONE
                {
                    let display = self.sprite_display_size();
                    let fit = 480. / display.x.max(display.y);
                    (if fit >= 1. { fit.floor() } else { fit })
                        .clamp(self.minimum_canvas_zoom(), 32.)
                } else {
                    self.canvas_fit_scale(view.size)
                };
                self.set_active_canvas_zoom(zoom);
                self.set_active_canvas_pan(Vec2::ZERO);
            }
            "properties" => self.properties_open = true,
            "pencil" => self.tool = Tool::Pencil,
            "spray" => self.tool = Tool::Spray,
            "blur-brush" => self.tool = Tool::BlurBrush,
            "jumble-brush" => self.tool = Tool::JumbleBrush,
            "eraser" => self.tool = Tool::Eraser,
            "fill" => self.tool = Tool::Fill,
            "picker" => self.tool = Tool::Picker,
            "line" => self.tool = Tool::Line,
            "rectangle" | "rect" => self.tool = Tool::Rect,
            "ellipse" => self.tool = Tool::Ellipse,
            "selection" | "select" => self.tool = Tool::Select,
            "elliptical-marquee" => self.tool = Tool::SelectEllipse,
            "polygonal-lasso" => self.tool = Tool::PolygonLasso,
            "polygon" => self.tool = Tool::Polygon,
            "curve" => self.tool = Tool::Curve,
            "text" => self.tool = Tool::Text,
            "contour" => self.tool = Tool::Contour,
            "wand" => self.tool = Tool::Wand,
            "workflow" => self.power.open = true,
            "swap" => {
                std::mem::swap(&mut self.fg, &mut self.bg);
                self.set_active_color(self.fg);
            }
            "resetcolors" => {
                self.set_secondary_color(DEFAULT_BG, None);
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
    #[cfg(test)]
    fn import_extension_result(&mut self) -> Result<(), String> {
        let session = self
            .extension_session
            .as_ref()
            .ok_or("No script result is available")?;
        let doc = session.load_result()?;
        if doc != self.doc {
            self.remember_action("Run extension");
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
    fn script_selection(
        doc: &Document,
        selection: Option<SelectionBounds>,
        mask: Option<&[bool]>,
        polygon: &[(i32, i32)],
    ) -> extensions::ScriptSelection {
        let [x, y, width, height] = selection.map_or([0; 4], |(x0, y0, x1, y1)| {
            [
                x0.min(x1),
                y0.min(y1),
                (x1 - x0).abs() + 1,
                (y1 - y0).abs() + 1,
            ]
        });
        let points =
            if (mask.is_some() || !polygon.is_empty()) && doc.width * doc.height <= 1_000_000 {
                Some(
                    (0..doc.width * doc.height)
                        .filter_map(|i| {
                            let (px, py) = ((i % doc.width) as i32, (i / doc.width) as i32);
                            mask.map_or_else(
                                || pixelcraft::workflows::inside_polygon(polygon, px, py),
                                |mask| mask.get(i).copied().unwrap_or(false),
                            )
                            .then_some((px, py))
                        })
                        .collect(),
                )
            } else {
                None
            };
        extensions::ScriptSelection {
            x,
            y,
            width,
            height,
            points,
        }
    }

    fn current_script_selection(&self) -> extensions::ScriptSelection {
        Self::script_selection(
            &self.doc,
            self.selection,
            self.studio.mask.as_deref(),
            &self.studio.polygon,
        )
    }

    fn script_selection_state(&self) -> ScriptSelectionState {
        ScriptSelectionState {
            bounds: self.selection,
            mask: self.studio.mask.clone(),
            polygon: self.studio.polygon.clone(),
        }
    }

    fn decode_script_selection(
        doc: &Document,
        wire: extensions::ScriptSelection,
    ) -> ScriptSelectionState {
        if let Some(points) = wire.points {
            let mut state = ScriptSelectionState::default();
            let mut mask = vec![false; doc.width * doc.height];
            for (x, y) in points {
                if let Some(i) = doc.index(x, y) {
                    mask[i] = true;
                    state.bounds = Some(state.bounds.map_or(
                        (x, y, x, y),
                        |(left, top, right, bottom)| {
                            (left.min(x), top.min(y), right.max(x), bottom.max(y))
                        },
                    ));
                }
            }
            if state.bounds.is_some() {
                state.mask = Some(mask);
            }
            return state;
        }
        ScriptSelectionState {
            bounds: (wire.width > 0 && wire.height > 0).then(|| {
                (
                    wire.x,
                    wire.y,
                    wire.x.saturating_add(wire.width - 1),
                    wire.y.saturating_add(wire.height - 1),
                )
            }),
            ..Default::default()
        }
    }

    fn set_script_selection(&mut self, state: ScriptSelectionState) {
        self.selection = state.bounds;
        self.studio.mask = state.mask;
        self.studio.polygon = state.polygon;
    }

    fn start_script(&mut self, script: PathBuf, command: Option<String>) -> Result<(), String> {
        if self.script_job.is_some() || self.pending.is_some() || !self.script_tab_closes.is_empty()
        {
            return Err("Finish the current script or tab close first".into());
        }
        let tab_ids: Vec<_> = (0..self.tabs.len())
            .map(|index| {
                if index == self.active_tab {
                    1
                } else {
                    index + 2
                }
            })
            .collect();
        // Document clones share cel pixel storage. Encode a background document only
        // when Lua accesses its handle, so untouched tabs use none of the Lua budget.
        let background_tabs: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| {
                tab.as_ref().map(|tab| {
                    let active = (tab.layer, tab.frame);
                    let range = CelRange::between(tab.timeline_anchor.unwrap_or(active), active);
                    let options = serde_json::json!({
                        "path": tab.path, "frame": tab.frame + 1, "layer": tab.layer + 1,
                        "selection": Self::script_selection(&tab.doc, tab.selection, tab.mask.as_deref(), &tab.polygon), "zoom": tab.zoom,
                        "timelineRange": range,
                        "layerKeys": ScriptTargets::default().expose_layers(&tab.doc),
                    });
                    (tab.doc.clone(), options)
                })
            })
            .collect();
        let (tx, rx) = std::sync::mpsc::channel();
        let doc = self.doc.clone();
        let (frame, layer) = (self.frame, self.layer);
        let colors = [self.fg, self.bg];
        let (ui_tx, ui_rx) = std::sync::mpsc::channel::<script_ui::Request>();
        self.script_ui
            .connect(ui_rx, self.shot.is_none() && !cfg!(test));
        self.script_preview = Some(ScriptPreview {
            initial_tab: self.active_tab,
            tab_count: self.tabs.len(),
            opened: Default::default(),
            layer_keys: Default::default(),
            visible: self.begin_script_tab_preview(1),
        });
        let preferences_path = extensions::config_directory().join("lua-preferences.json");
        let saved_preferences: serde_json::Value = std::fs::read(&preferences_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(serde_json::json!({}));
        let selection = self.current_script_selection();
        let tilemap_mode = u8::from(self.tile_pixel_mode.is_none());
        let tileset_mode = match self.tile_pixel_mode {
            Some(TilePixelEdit::ReuseVariant) => 1,
            Some(TilePixelEdit::CreateVariant) => 2,
            _ => 0,
        };
        let selection_mode = match self.power.selection_op {
            pixelcraft::productivity::SelectionOp::Replace => 0,
            pixelcraft::productivity::SelectionOp::Add => 1,
            pixelcraft::productivity::SelectionOp::Subtract => 2,
            pixelcraft::productivity::SelectionOp::Intersect => 3,
        };
        let options = serde_json::json!({"selection":selection,"editorTabs":tab_ids,"layerKeys":ScriptTargets::default().expose_layers(&doc),"tool":self.tool.script_name(self.filled),"brushSize":self.brush,"pixelPerfect":self.studio.pixel_perfect,"fillTolerance":self.power.fill_tolerance,"fillContiguous":!self.power.fill_global,"wandTolerance":self.power.tolerance,"wandContiguous":self.power.contiguous,"selectionMode":selection_mode,"fgTile":self.selected_tile,"tilemapMode":tilemap_mode,"tilesetMode":tileset_mode,"timelineRange":self.timeline_range(),"filename":self.path.as_ref().map(|p|p.to_string_lossy()),"zoom":self.zoom,"preferences":saved_preferences["preferences"],"pluginPreferences":saved_preferences[script.to_string_lossy().as_ref()],"configPath":extensions::config_directory(),"projectPath":script.parent()});
        std::thread::spawn(move || {
            let result = std::fs::read_to_string(&script)
                .map_err(|e| e.to_string())
                .and_then(|source| {
                    let base = script
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf();
                    let mut preferences = saved_preferences;
                    extensions::run_workspace_with_host(
                        &doc,
                        &source,
                        frame,
                        layer,
                        command.as_deref(),
                        colors,
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
                            if op == "clipboardApply" {
                                return Self::apply_script_paste(&request);
                            }
                            if op == "editorDocument" {
                                let index = request["sprite"]
                                    .as_u64()
                                    .and_then(|id| id.checked_sub(2))
                                    .and_then(|index| usize::try_from(index).ok());
                                let (doc, options) = index
                                    .and_then(|index| background_tabs.get(index))
                                    .and_then(Option::as_ref)
                                    .ok_or("Unknown editor tab")?;
                                let mut response = options.clone();
                                response["document"] = extensions::script_document_value(doc)?;
                                return Ok(response);
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
                });
            let _ = tx.send(result);
        });
        self.script_job = Some(rx);
        self.stop_playback(false);
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
                let layer_keys = self
                    .script_preview
                    .as_mut()
                    .map(|preview| std::mem::take(&mut preview.layer_keys))
                    .unwrap_or_default();
                self.finish_script_preview();
                let result =
                    result.and_then(|workspace| self.apply_script_workspace(workspace, layer_keys));
                if let Err(e) = result {
                    self.status = format!("Extension failed: {e}");
                }
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.script_job = None;
                self.script_ui.disconnect();
                self.finish_script_preview();
                self.status =
                    "Extension failed: the Lua worker stopped. Your sprite is unchanged.".into();
            }
            _ => {}
        }
    }

    fn begin_script_tab_preview(&mut self, id: u32) -> ScriptTabPreview {
        ScriptTabPreview {
            id,
            original: self.history_entry(self.doc.clone(), "Run extension"),
            undo: std::mem::take(&mut self.undo),
            redo: std::mem::take(&mut self.redo),
            drawn: self.doc.clone(),
            generation: self.power.bridge_generation,
            site: (self.frame, self.layer),
            selection: self.script_selection_state(),
        }
    }

    fn restore_script_tab_preview(&mut self, preview: ScriptTabPreview) {
        let original = preview.original.doc.clone();
        self.restore_history(preview.original);
        self.doc = original;
        self.undo = preview.undo;
        self.redo = preview.redo;
        self.render_snapshot = None;
    }

    fn finish_script_preview(&mut self) {
        if let Some(preview) = self.script_preview.take() {
            self.restore_script_tab_preview(preview.visible);
            self.select_document_tab(preview.initial_tab);
            self.tabs.truncate(preview.tab_count);
        }
    }

    fn script_existing_tab(id: u32, initial: usize, count: usize) -> Option<usize> {
        if id == 1 {
            Some(initial)
        } else {
            let index = id.checked_sub(2)? as usize;
            (index < count && index != initial).then_some(index)
        }
    }

    fn show_script_ui(&mut self, ctx: &egui::Context) {
        if self.script_ui.modeless
            && let Some(tab) = self.script_preview.as_ref().map(|p| &p.visible)
            && (tab.drawn != self.doc
                || !tab.drawn.same_editor_layer_targets(&self.doc)
                || (tab.generation != self.power.bridge_generation
                    && !tab.drawn.same_editor_targets(&self.doc))
                || tab.site != (self.frame, self.layer)
                || tab.selection.bounds != self.selection
                || tab.selection.mask != self.studio.mask
                || tab.selection.polygon != self.studio.polygon)
        {
            let id = tab.id;
            let targets = self
                .script_preview
                .as_mut()
                .unwrap()
                .layer_keys
                .entry(id)
                .or_default();
            let layer_keys = targets.expose_layers(&self.doc);
            let cel_keys = targets.expose_cels(&self.doc);
            self.script_ui.editor_update = Some(serde_json::json!({
                "sprite":id,"document":self.doc,"frame":self.frame+1,"layer":self.layer+1,
                "selection":self.current_script_selection(),"layerKeys":layer_keys,"celKeys":cel_keys,
            }));
            let selection = self.script_selection_state();
            let tab = &mut self.script_preview.as_mut().unwrap().visible;
            tab.drawn = self.doc.clone();
            tab.generation = self.power.bridge_generation;
            tab.site = (self.frame, self.layer);
            tab.selection = selection;
        }
        let updated_sprite = self
            .script_ui
            .editor_update
            .as_ref()
            .and_then(|update| update["sprite"].as_u64());
        self.script_ui.show(ctx);
        if let Some(request) = self.script_ui.clipboard_request.take() {
            self.handle_script_clipboard(request);
        }
        let Some(request) = self.script_ui.preview.take() else {
            return;
        };
        let Some(id) = request["sprite"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
        else {
            return;
        };
        if request["op"] == "closePreview" {
            self.remove_script_preview_tab(id);
        } else if updated_sprite != Some(u64::from(id))
            && let Ok(document) = serde_json::from_value::<Document>(request["document"].clone())
            && document.validate().is_ok()
        {
            self.show_script_tab(id, document, &request);
        }
    }

    fn remove_script_preview_tab(&mut self, id: u32) {
        let Some(mut preview) = self.script_preview.take() else {
            return;
        };
        if let Some(index) = preview.opened.remove(&id) {
            if index == self.active_tab {
                self.restore_script_tab_preview(preview.visible);
                self.select_document_tab(preview.initial_tab);
                preview.visible = self.begin_script_tab_preview(1);
            }
            self.tabs.remove(index);
            if self.active_tab > index {
                self.active_tab -= 1;
            }
            for other in preview.opened.values_mut().filter(|other| **other > index) {
                *other -= 1;
            }
        }
        self.script_preview = Some(preview);
    }

    fn show_script_tab(&mut self, id: u32, mut document: Document, request: &serde_json::Value) {
        let Some(mut preview) = self.script_preview.take() else {
            return;
        };
        if preview.visible.id != id {
            self.restore_script_tab_preview(preview.visible);
            let existing = Self::script_existing_tab(id, preview.initial_tab, preview.tab_count);
            if let Some(index) = existing.or_else(|| preview.opened.get(&id).copied()) {
                self.select_document_tab(index);
            } else {
                let path = request["path"]
                    .as_str()
                    .and_then(|name| Self::script_native_path(Path::new(name)));
                self.insert_document_tab(document.clone(), path);
                preview.opened.insert(id, self.active_tab);
            }
            preview.visible = self.begin_script_tab_preview(id);
        }
        document.editor_id = self.doc.editor_id;
        if let Ok(keys) = serde_json::from_value::<Vec<String>>(request["layerKeys"].clone()) {
            let targets = preview.layer_keys.entry(id).or_insert_with(|| {
                let mut targets = ScriptTargets::default();
                targets.expose_layers(&self.doc);
                targets
            });
            targets.apply_layers(&mut document, &keys);
            if let Ok(cels) = serde_json::from_value::<Vec<extensions::ScriptCelTarget>>(
                request["celKeys"].clone(),
            ) {
                targets.apply_cels(&self.doc, &mut document, &cels);
            }
        }
        let changed_layout = document.width != self.doc.width
            || document.height != self.doc.height
            || document.layers.len() != self.doc.layers.len()
            || document.durations.len() != self.doc.durations.len();
        self.doc = document;
        if changed_layout {
            self.clamp();
        }
        self.frame =
            Self::script_site_index(&request["frame"], self.frame, self.doc.durations.len());
        self.layer = Self::script_site_index(&request["layer"], self.layer, self.doc.layers.len());
        preview.visible.drawn = self.doc.clone();
        preview.visible.generation = self.power.bridge_generation;
        preview.visible.site = (self.frame, self.layer);
        if let Ok(wire) =
            serde_json::from_value::<extensions::ScriptSelection>(request["selection"].clone())
        {
            self.set_script_selection(Self::decode_script_selection(&self.doc, wire));
        }
        preview.visible.selection = self.script_selection_state();
        self.script_preview = Some(preview);
    }

    fn script_site_index(value: &serde_json::Value, current: usize, count: usize) -> usize {
        value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map_or(current, |index| index.saturating_sub(1))
            .min(count - 1)
    }

    fn script_native_path(path: &Path) -> Option<PathBuf> {
        let ext = path.extension()?.to_str()?;
        ["pixel", "ase", "aseprite"]
            .iter()
            .any(|item| item.eq_ignore_ascii_case(ext))
            .then(|| path.to_path_buf())
    }

    fn apply_script_workspace(
        &mut self,
        workspace: extensions::ScriptWorkspace,
        mut layer_keys: std::collections::HashMap<u32, ScriptTargets>,
    ) -> Result<(), String> {
        let initial_tab = self.active_tab;
        let mut active_tab = None;
        let existing_tabs = self.tabs.len();
        let existing_index = |id| Self::script_existing_tab(id, initial_tab, existing_tabs);
        for sprite in workspace.sprites {
            let path = Self::script_native_path(&sprite.filename);
            let existing = existing_index(sprite.id);
            if let Some(index) = existing {
                self.switch_tab(index);
            } else {
                let baseline = sprite.baseline.ok_or("Missing opened sprite baseline")?;
                self.open_document(baseline, path.clone());
            }
            let mut document = sprite.document;
            document.editor_id = self.doc.editor_id;
            let targets = layer_keys.entry(sprite.id).or_insert_with(|| {
                let mut targets = ScriptTargets::default();
                targets.expose_layers(&self.doc);
                targets
            });
            targets.apply_layers(&mut document, &sprite.layer_keys);
            let cel_targets_changed =
                targets.apply_cels(&self.doc, &mut document, &sprite.cel_keys);
            let changed = self.doc != document
                || (existing.is_some()
                    && (cel_targets_changed || !self.doc.same_editor_layer_targets(&document)));
            let selection = Self::decode_script_selection(&document, sprite.selection);
            let selection_changed = selection.bounds != self.selection
                || selection.mask != self.studio.mask
                || !self.studio.polygon.is_empty();
            if changed || selection_changed {
                self.remember_action("Run extension");
            }
            if changed {
                self.doc = document;
            } else {
                self.doc.retain_frame_export_id_counter(&document);
            }
            if let Some(path) = path {
                self.path = Some(path);
                if let Some(saved) = sprite.saved {
                    self.saved = saved;
                }
            }
            let frame = sprite.frame.saturating_sub(1);
            let layer = sprite.layer.saturating_sub(1);
            if changed || (frame, layer) != (self.frame, self.layer) {
                self.frame = frame;
                self.layer = layer;
                self.clamp();
                self.set_script_selection(selection);
            } else if selection_changed {
                self.set_script_selection(selection);
            }
            if Some(sprite.id) == workspace.active {
                active_tab = Some(self.active_tab);
            }
        }
        if let Some(index) = active_tab {
            self.switch_tab(index);
        }
        [self.fg, self.bg] = workspace.colors;
        self.set_active_color(self.fg);
        self.status = "Applied extension changes".into();
        if !workspace.closed.is_empty() {
            self.script_close_destination = active_tab.map(|_| self.doc.editor_id.to_string());
            self.script_tab_closes = workspace
                .closed
                .into_iter()
                .rev()
                .filter_map(existing_index)
                .map(|index| {
                    self.tabs[index]
                        .as_ref()
                        .map_or(&self.doc, |tab| &tab.doc)
                        .editor_id
                        .to_string()
                })
                .collect();
            self.continue_script_tab_closes();
        }
        Ok(())
    }

    fn continue_script_tab_closes(&mut self) {
        if self.save_job.is_some() || self.pending.is_some() {
            return;
        }
        while let Some(id) = self.script_tab_closes.pop() {
            if let Some(index) = self.script_tab_index(&id) {
                self.switch_tab(index);
                self.request(Pending::CloseTab);
                if self.pending.is_some() {
                    return;
                }
            }
        }
        if let Some(id) = self.script_close_destination.take()
            && let Some(index) = self.script_tab_index(&id)
        {
            self.switch_tab(index);
        }
    }

    fn script_tab_index(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|tab| {
            tab.as_ref()
                .map_or(&self.doc, |tab| &tab.doc)
                .editor_id
                .to_string()
                == id
        })
    }
    #[cfg(test)]
    fn action_tooltip(&self, action: &str, label: &str) -> String {
        ui_icons::tooltip(&self.custom_shortcuts, action, label)
    }
    fn live_export_status(&mut self, ui: &mut egui::Ui) {
        let Some(progress) = &self.power.bridge_export else {
            return;
        };
        let (done, total) = progress.counts();
        let fraction = done as f32 / total.max(1) as f32;
        ui.add(egui::ProgressBar::new(fraction).text(format!("MCP export {done}/{total}")));
        if ui.pc_button("Cancel export").clicked() {
            progress.cancel();
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.curve_draft.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.cancel_curve();
            return;
        }
        if self.canvas_transform.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.canvas_transform = None;
            self.status = "Canvas transform cancelled".into();
            return;
        }
        if ctx.egui_wants_keyboard_input() || self.polygon_shortcuts(ctx) {
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
        let custom = ctx.input(|input| {
            self.custom_shortcuts.iter().find_map(|(binding, action)| {
                let chord = ShortcutChord::parse(binding)?;
                (chord.allowed_for(action) && chord.matches(input)).then(|| action.clone())
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
            let has_update = self.downloaded_update.is_some()
                || self
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
                        ("Import as Layer…", "import-layer"),
                        ("Close sprite   Ctrl+W", "closetab"),
                        ("Import PNG sequence…", "sequence"),
                        ("Grid import, tag batch & atlas…", "workflow"),
                        ("Save   Ctrl+S", "save"),
                        ("Save as…", "saveas"),
                        ("Repeat Last Export", "repeat-export"),
                        ("Export baked ASE copy…", "export-baked-ase"),
                        ("Export PNG…", "png"),
                        ("Export sprite sheet…", "sheet"),
                        ("Export PNG sequence…", "png-sequence"),
                        ("Export animation GIF…", "gif"),
                        ("Export video MP4…", "mp4"),
                        ("Export FLI…", "fli"),
                        ("Export FLC…", "flc"),
                        ("Export tileset atlas…", "tileset-atlas"),
                        ("Export Tiled map…", "tiledexport"),
                        ("Export BMP…", "bmp"),
                        ("Export JPEG…", "jpeg"),
                        ("Export WebP…", "webp"),
                        ("Export icon…", "ico"),
                        ("Export PCX…", "pcx"),
                        ("Export QOI…", "qoi"),
                        ("Export SVG…", "svg"),
                        ("Export CSS…", "css"),
                        ("Export TGA…", "tga"),
                        ("Open sample garden", "demo"),
                    ],
                ),
                (
                    "Edit",
                    vec![
                        ("Undo   Ctrl+Z", "undo"),
                        ("Redo   Ctrl+Y", "redo"),
                        ("Undo History…", "undo-history"),
                        ("Copy   Ctrl+C", "copy"),
                        ("Copy Merged", "copy-merged"),
                        ("Cut   Ctrl+X", "cut"),
                        ("Paste   Ctrl+V", "paste"),
                        ("Paste as New Sprite", "paste-as-sprite"),
                        ("Paste as New Layer", "paste-as-layer"),
                        ("Paste as Reference", "paste-as-reference"),
                        ("Clear   Delete", "clear"),
                        ("Replace Color…", "replace-color"),
                        ("Filters…", "filters"),
                        ("Select all   Ctrl+A", "selectall"),
                        ("Deselect   Esc", "deselect"),
                        ("Settings…", "settings"),
                        ("Keyboard shortcuts…", "shortcuts"),
                        ("Drawing & selection studio…", "studio"),
                        ("Magic Wand", "wand"),
                        ("Brushes, references & workflow tools…", "workflow"),
                        ("Lasso selection", "lasso"),
                        ("Move selection", "move"),
                        ("Scale or skew cels on canvas…", "canvas-transform"),
                        ("Rotate selection 90°", "rotate"),
                        ("Flip selection horizontally", "selectionflipx"),
                        ("Flip selection vertically", "selectionflipy"),
                        ("Crop to selection", "crop"),
                        ("New Sprite from Selection", "new-sprite-from-selection"),
                    ],
                ),
                (
                    "Sprite",
                    vec![
                        ("Properties, layers, tiles & ranges…", "properties"),
                        ("Palette and Indexed Color…", "palette-conversion"),
                        ("Color Profile…", "color-profile"),
                        ("Canvas size…", "resizecanvas"),
                        ("Sprite size…", "spritesize"),
                        ("Duplicate sprite", "duplicatesprite"),
                        ("Auto-trim sprite", "trimsprite"),
                        ("Rotate sprite 90° clockwise", "rotatespriteright"),
                        ("Rotate sprite 90° counterclockwise", "rotatespriteleft"),
                        ("Rotate sprite 180°", "rotatespritehalf"),
                        ("Flip sprite horizontally", "flipspritex"),
                        ("Flip sprite vertically", "flipspritey"),
                        ("Tileset browser…", "tiles"),
                        ("Flip active cel horizontally", "flipx"),
                        ("Flip active cel vertically", "flipy"),
                    ],
                ),
                (
                    "Layer",
                    vec![
                        ("New layer", "layer"),
                        ("Duplicate layer or group", "layer-duplicate"),
                        ("Merge down", "layer-merge"),
                        ("Flatten visible layers", "flatten-visible"),
                        ("Flatten all layers", "flatten-layers"),
                        ("Layer via copy", "layer-via-copy"),
                        ("Layer via cut", "layer-via-cut"),
                        ("Convert to raster layer", "convert-raster"),
                        ("Convert to background", "convert-background"),
                        ("Convert to tilemap…", "convert-tilemap"),
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
                        ("Reverse frames", "reverse-frames"),
                        ("Play / stop   Space", "play"),
                        ("Play once", "play-once"),
                        ("Play selection", "play-selection"),
                        ("Play selection once", "play-selection-once"),
                        ("Play subtags", "play-subtags"),
                        ("Play subtags once", "play-subtags-once"),
                        ("Rewind on stop", "rewind-on-stop"),
                        ("Previous frame   ,", "prev"),
                        ("Next frame   .", "next"),
                    ],
                ),
                (
                    "View",
                    vec![
                        ("Toggle pixel grid", "grid"),
                        ("Show drawing grid", "drawing-grid"),
                        ("Snap to drawing grid", "snap-to-grid"),
                        ("Drawing grid settings…", "drawing-grid-settings"),
                        ("Set drawing grid from selection", "grid-from-selection"),
                        ("Toggle onion skin", "onion"),
                        ("Fit canvas", "fit"),
                        ("Split canvas", "split-canvas"),
                        ("Seamless canvas · both axes", "seamless"),
                        ("Seamless canvas · horizontal", "seamless-horizontal"),
                        ("Seamless canvas · vertical", "seamless-vertical"),
                    ],
                ),
                (
                    "Select",
                    vec![
                        ("Select all", "selectall"),
                        ("Elliptical marquee", "elliptical-marquee"),
                        ("Polygonal lasso", "polygonal-lasso"),
                        ("Deselect", "deselect"),
                        ("Reselect", "reselect"),
                        ("Invert selection", "invert-selection"),
                        ("Color Range…", "color-range"),
                        ("Grow, Shrink & Border…", "modify-selection"),
                        ("Import mask PNG…", "import-selection-mask"),
                        ("Export mask PNG…", "export-selection-mask"),
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
                            let entries = self.recent_file_entries();
                            if entries.is_empty() {
                                ui.label("No recent files yet");
                            }
                            for (path, label) in entries {
                                let response = ui
                                    .add_enabled_ui(path.is_file(), |ui| {
                                        ui_icons::button(ui, "open", &label, false, true)
                                    })
                                    .inner
                                    .on_hover_text(path.display().to_string());
                                if response.clicked() {
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
                        if ui
                            .button(if self.preview_settings.visible {
                                "Hide Preview"
                            } else {
                                "Show Preview"
                            })
                            .clicked()
                        {
                            self.toggle_preview_visibility();
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
                            "split-canvas" => self.split_canvas.is_some(),
                            "drawing-grid" => self.show_drawing_grid,
                            "snap-to-grid" => self.snap_to_grid,
                            "onion" => self.onion,
                            "rewind-on-stop" => self.rewind_on_stop(),
                            "play" => self.playing,
                            "seamless" => self.studio.seamless == WrapAxes::BOTH,
                            "seamless-horizontal" => self.studio.seamless == WrapAxes::HORIZONTAL,
                            "seamless-vertical" => self.studio.seamless == WrapAxes::VERTICAL,
                            _ => false,
                        };
                        let icon = if a == "play" && self.playing {
                            "stop"
                        } else {
                            a
                        };
                        let enabled = match a {
                            "repeat-export" => self.can_repeat_export(),
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
                if has_update {
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
                            if self.downloaded_update.is_some() {
                                "Update ready"
                            } else {
                                "New version found"
                            },
                            egui::FontId::proportional(14.),
                            Color32::from_rgb(241, 190, 93),
                        );
                        if ui
                            .interact(notice, egui::Id::new("update_notice"), Sense::click())
                            .on_hover_text("Open update details")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.about = true;
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
    fn set_secondary_color(&mut self, color: Pixel, slot: Option<usize>) {
        self.bg = color;
        self.secondary_palette_index = slot;
    }

    fn secondary_stroke(&self) -> bool {
        self.gesture_button == Some(egui::PointerButton::Secondary)
    }

    fn alternate_stroke_color(&self) -> Pixel {
        if self.secondary_stroke() {
            self.fg
        } else {
            self.bg
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
                        .entry(cel.backing_id())
                        .and_modify(|entry: &mut (&Pixels, u64)| entry.1 += 1)
                        .or_insert((cel, 1));
                }
            }
        }
        for (_, (cel, uses)) in unique {
            cel.visit_nonclear(|_, pixel| {
                if pixel[3] != 0 {
                    *counts.entry(pixel).or_default() += uses;
                }
            });
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
        let preferred = if self.secondary_stroke() {
            self.secondary_palette_index
        } else {
            Some(self.palette_index)
        };
        preferred
            .filter(|&index| palette.get(index) == Some(&color))
            .or_else(|| palette.iter().position(|p| *p == color))
    }
    fn tools(&mut self, ui: &mut egui::Ui) {
        for t in [
            Tool::Pencil,
            Tool::Spray,
            Tool::BlurBrush,
            Tool::JumbleBrush,
            Tool::Eraser,
            Tool::Fill,
            Tool::Picker,
            Tool::Line,
            Tool::Curve,
            Tool::Text,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Polygon,
            Tool::Contour,
            Tool::Select,
            Tool::SelectEllipse,
            Tool::Wand,
            Tool::Lasso,
            Tool::PolygonLasso,
            Tool::Move,
            Tool::Gradient,
            Tool::Hand,
        ] {
            let action = t.action();
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
    fn timeline_range(&self) -> CelRange {
        let active = (self.layer, self.frame);
        CelRange::between(self.timeline_anchor.unwrap_or(active), active)
    }

    fn timeline_select(&mut self, layer: usize, frame: usize, extend: bool) {
        self.timeline_frames = TimelineHeaders::default();
        self.timeline_layers = TimelineHeaders::default();
        self.selected_cels = None;
        if extend {
            self.timeline_anchor.get_or_insert((self.layer, self.frame));
        } else {
            self.timeline_anchor = None;
            self.timeline_duration = self.doc.durations[frame];
        }
        self.layer = layer;
        self.frame = frame;
        self.timeline_destination = None;
        self.stop_playback(false);
    }

    fn timeline_cel_selection(&self) -> CelSelection {
        self.selected_cels
            .clone()
            .unwrap_or_else(|| CelSelection::Rectangle(self.timeline_range()))
    }

    fn toggle_cel_selection(&mut self, layer: usize, frame: usize) {
        let mut cells = match self.selected_cels.take() {
            Some(CelSelection::Individual(cells)) => cells,
            _ => Arc::new(
                CelSelection::Rectangle(self.timeline_range())
                    .cells()
                    .collect(),
            ),
        };
        let position = (layer, frame);
        let selected = Arc::make_mut(&mut cells);
        let active = if selected.remove(&position) {
            selected.last().copied().unwrap_or(position)
        } else {
            selected.insert(position);
            position
        };
        self.timeline_select(active.0, active.1, false);
        if !cells.is_empty() {
            self.selected_cels = Some(CelSelection::Individual(cells));
        }
    }

    fn place_timeline_cels(
        &mut self,
        selection: &CelSelection,
        target: (usize, usize),
        duplicate: bool,
    ) {
        let Some(bounds) = selection.bounds() else {
            return;
        };
        let next = self
            .doc
            .with_moved_cels(selection, target.0, target.1, duplicate);
        if self.apply_document_edit(
            if duplicate {
                "Duplicate cels"
            } else {
                "Move cels"
            },
            next,
        ) {
            let active = if selection.contains(self.layer, self.frame) {
                (self.layer, self.frame)
            } else {
                (bounds.first_layer, bounds.first_frame)
            };
            let translate = |(layer, frame)| {
                (
                    target.0 + layer - bounds.first_layer,
                    target.1 + frame - bounds.first_frame,
                )
            };
            let anchor = self.timeline_anchor.unwrap_or(active);
            (self.layer, self.frame) = translate(active);
            match selection {
                CelSelection::Rectangle(_) => {
                    self.timeline_anchor = selection
                        .contains(anchor.0, anchor.1)
                        .then(|| translate(anchor));
                    self.selected_cels = None;
                }
                CelSelection::Individual(cells) => {
                    self.timeline_anchor = None;
                    self.selected_cels = Some(CelSelection::Individual(Arc::new(
                        cells.iter().copied().map(translate).collect(),
                    )));
                }
            }
            self.timeline_duration = self.doc.durations[self.frame];
            self.timeline_frames = TimelineHeaders::default();
            self.timeline_layers = TimelineHeaders::default();
            self.timeline_destination = None;
            self.stop_playback(false);
        }
    }

    fn select_layer_header(&mut self, layer: usize, modifiers: egui::Modifiers, visible: &[usize]) {
        let mut layers = std::mem::take(&mut self.timeline_layers);
        if modifiers.shift {
            let anchor = *layers.anchor.get_or_insert(self.layer);
            let target = visible
                .iter()
                .position(|&index| index == layer)
                .unwrap_or(0);
            let start = visible
                .iter()
                .position(|&index| index == anchor)
                .unwrap_or(target);
            layers.selected = visible[start.min(target)..=start.max(target)].to_vec();
            layers.selected.sort_unstable();
        } else if modifiers.command {
            layers.toggle(layer);
        } else {
            layers.selected = vec![layer];
            layers.anchor = Some(layer);
        }
        let active = if layers.selected.binary_search(&layer).is_ok() {
            layer
        } else {
            layers.selected.last().copied().unwrap_or(layer)
        };
        self.timeline_select(active, self.frame, false);
        self.timeline_layers = layers;
    }

    fn drop_layers(&mut self, drag: &LayerHeaderDrag, drop: LayerDrop) {
        match self.doc.with_layers_restacked(&drag.layers, drop) {
            Ok(Some((next, positions))) => {
                if self.apply_document_edit("Restack layers", Ok(Some(next))) {
                    let active = if drag.layers.binary_search(&self.layer).is_ok() {
                        self.layer
                    } else {
                        drag.grabbed
                    };
                    self.timeline_select(positions[active], self.frame, false);
                    self.timeline_layers.anchor = Some(self.layer);
                    self.timeline_layers.selected =
                        drag.layers.iter().map(|&layer| positions[layer]).collect();
                    self.timeline_layers.selected.sort_unstable();
                    if let LayerDrop::Into(target) = drop {
                        self.studio
                            .collapsed_groups
                            .remove(&self.doc.layers[positions[target]].editor_id.to_string());
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.status = error,
        }
    }

    fn select_frame_header(&mut self, frame: usize, modifiers: egui::Modifiers) {
        let selected = &mut self.timeline_frames.selected;
        if modifiers.shift {
            let anchor = *self.timeline_frames.anchor.get_or_insert(self.frame);
            *selected = (anchor.min(frame)..=anchor.max(frame)).collect();
        } else if modifiers.command {
            self.timeline_frames.toggle(frame);
        } else {
            *selected = vec![frame];
            self.timeline_frames.anchor = Some(frame);
        }
        self.frame = frame;
        self.timeline_layers = TimelineHeaders::default();
        self.timeline_duration = self.doc.durations[frame];
        self.timeline_anchor = None;
        self.selected_cels = None;
        self.timeline_destination = None;
        self.stop_playback(false);
    }

    fn reorder_timeline_frames(&mut self, from: usize, before: usize) {
        let frames = &self.timeline_frames.selected;
        let selected = if frames.binary_search(&from).is_ok() {
            frames.clone()
        } else {
            vec![from]
        };
        if self.edit_timeline("Move frames", |doc| {
            doc.move_frames(&selected, before).map(|_| ())
        }) {
            let insertion = before - selected.partition_point(|&frame| frame < before);
            let active = selected.binary_search(&self.frame).unwrap_or(0);
            self.frame = insertion + active;
            self.timeline_frames.selected = (insertion..insertion + selected.len()).collect();
            self.timeline_frames.anchor = Some(self.frame);
            self.timeline_layers = TimelineHeaders::default();
            self.timeline_anchor = None;
            self.selected_cels = None;
            self.timeline_destination = None;
            self.timeline_duration = self.doc.durations[self.frame];
            self.stop_playback(false);
        }
    }

    fn reverse_timeline_frames(&mut self) {
        let selected = if self.timeline_frames.selected.is_empty() {
            (0..self.doc.durations.len()).collect()
        } else {
            self.timeline_frames.selected.clone()
        };
        if self.edit_timeline("Reverse frames", |doc| {
            doc.reverse_frames(&selected).map(|_| ())
        }) {
            self.timeline_duration = self.doc.durations[self.frame];
            self.stop_playback(false);
        }
    }

    fn edit_timeline(
        &mut self,
        label: &'static str,
        edit: impl FnOnce(&mut Document) -> Result<(), String>,
    ) -> bool {
        let mut next = self.doc.clone();
        let result = edit(&mut next)
            .and_then(|()| next.validate())
            .map(|()| Some(next));
        self.apply_document_edit(label, result)
    }

    fn apply_document_edit(
        &mut self,
        label: &'static str,
        next: Result<Option<Document>, String>,
    ) -> bool {
        match next {
            Ok(Some(next)) if next != self.doc || !next.same_editor_targets(&self.doc) => {
                self.power.bridge_generation += 1;
                let previous = std::mem::replace(&mut self.doc, next);
                self.redo.clear();
                self.push_undo(previous, label);
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn set_tag_repeat(&mut self, tag: usize, repeat: u16) {
        if self.doc.tags[tag].repeat != repeat {
            self.remember_action("Change tag repeats");
            self.doc.tags[tag].repeat = repeat;
        }
    }

    fn timeline_tag_controls(&mut self, ui: &mut egui::Ui) {
        if self.doc.tags.is_empty() {
            return;
        }
        let mut changed = None;
        ui.menu_button("Tags", |ui| {
            ui.small("0 repeats loops indefinitely in exports.");
            for (index, tag) in self.doc.tags.iter().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} · {}–{}", tag.name, tag.from + 1, tag.to + 1));
                        let mut repeat = tag.repeat;
                        if ui
                            .add(
                                egui::DragValue::new(&mut repeat)
                                    .range(0..=u16::MAX)
                                    .prefix("Repeat "),
                            )
                            .changed()
                        {
                            changed = Some((index, repeat));
                        }
                    });
                });
            }
        });
        if let Some((index, repeat)) = changed {
            self.set_tag_repeat(index, repeat);
        }
    }

    fn start_rename_frame(&mut self, frame: usize) {
        self.rename_frame = Some((
            self.doc.editor_id,
            self.doc.frame_export_id(frame),
            self.doc.frame_name(frame).unwrap_or_default().into(),
        ));
    }

    fn collapsed_parent(&self, mut layer: usize) -> Option<usize> {
        let mut collapsed = None;
        if self.studio.collapsed_groups.is_empty() {
            return collapsed;
        }
        while let Some(parent) = self.doc.layers[layer].parent {
            if self
                .studio
                .collapsed_groups
                .contains(&self.doc.layers[parent].editor_id.to_string())
            {
                collapsed = Some(parent);
            }
            layer = parent;
        }
        collapsed
    }

    fn toggle_timeline_group(&mut self, layer: usize) {
        let id = self.doc.layers[layer].editor_id.to_string();
        if !self.studio.collapsed_groups.remove(&id) {
            self.studio.collapsed_groups.insert(id);
            if let Some(visible) = self.collapsed_parent(self.layer) {
                self.timeline_select(visible, self.frame, false);
                self.timeline_destination = None;
            }
        }
    }

    fn timeline(&mut self, ui: &mut egui::Ui) {
        let visible_rows: Vec<_> = (0..self.doc.layers.len())
            .rev()
            .filter(|&layer| self.collapsed_parent(layer).is_none())
            .collect();
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
            ui.menu_button("Playback", |ui| {
                for (label, command) in [
                    ("Play once", "play-once"),
                    ("Play selection", "play-selection"),
                    ("Play selection once", "play-selection-once"),
                    ("Play subtags", "play-subtags"),
                    ("Play subtags once", "play-subtags-once"),
                ] {
                    if ui.pc_command_button(command, label).clicked() {
                        a = Some(command);
                        ui.close();
                    }
                }
                let mut rewind = self.rewind_on_stop();
                if ui.pc_checkbox(&mut rewind, "Rewind on stop").changed() {
                    self.toggle_rewind_on_stop();
                }
            });
            let speed = self.playback_speed;
            egui::ComboBox::from_id_salt("playback_speed")
                .selected_text(format!("{speed}%"))
                .width(60.)
                .show_ui(ui, |ui| {
                    for percent in [25, 50, 100, 150, 200, 400] {
                        ui.pc_selectable_value(
                            &mut self.playback_speed,
                            percent,
                            format!("{percent}%"),
                        );
                    }
                })
                .response
                .on_hover_text("Playback speed for the timeline and preview");
            if self.playback_speed != speed {
                self.last_tick = Instant::now();
                self.playback_waiting_for_frame = false;
                if let Some(preview) = &mut self.preview_playback {
                    preview.set_speed(ui.input(|input| input.time), self.playback_speed);
                }
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
            if let Some(name) = self.doc.frame_name(self.frame) {
                ui.add(egui::Label::new(name).truncate())
                    .on_hover_text(name);
            }
            let mut ms = self.doc.durations[self.frame];
            if !narrow
                && ui
                    .add(egui::DragValue::new(&mut ms).range(1..=65535).suffix(" ms"))
                    .changed()
            {
                self.remember_action("Change frame duration");
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
            self.timeline_tag_controls(ui);
        });
        ui.separator();
        let mut toggle = None;
        let mut collapse = None;
        let mut lock = None;
        let mut rename = None;
        let mut selection = None;
        let mut toggle_cel = None;
        let mut header_selection = None;
        let mut frame_drop = None;
        let mut layer_header_selection = None;
        let mut layer_drop = None;
        let mut cel_drop = None;
        let mut cel_drag_preview = None;
        let mut visible_cel_rects = Vec::new();
        let mut reverse_header_selection = false;
        let cel_selection = self.timeline_cel_selection();
        let menu_bounds = self
            .timeline_destination
            .and_then(|_| cel_selection.bounds());
        let destination = self.timeline_destination.and_then(|(layer, frame)| {
            self.doc
                .cel_selection_destination(&cel_selection, layer, frame)
                .ok()
        });
        egui::ScrollArea::both()
            .id_salt("timeline_scroll")
            .max_height((ui.available_height() - 36.).max(0.))
            .show(ui, |ui| {
                let onion_track =
                    (self.onion && !self.playing).then(|| ui.allocate_space(Vec2::new(0., 20.)).1);
                let mut first_header = None;
                egui::Grid::new("cels")
                    .spacing(Vec2::ZERO)
                    .min_row_height(row_height)
                    .show(ui, |ui| {
                        ui.allocate_ui(Vec2::new(label_width, header_height), |ui| {
                            ui.label("");
                        });
                        for f in 0..self.doc.durations.len() {
                            let button = if let Some(Some(texture)) = self.frame_textures.get(f) {
                                egui::Button::image_and_text(
                                    egui::Image::new((texture.id(), Vec2::splat(20.))),
                                    (f + 1).to_string(),
                                )
                            } else {
                                egui::Button::new((f + 1).to_string())
                            };
                            let selected = f == self.frame
                                || self.timeline_frames.selected.binary_search(&f).is_ok();
                            let response = ui.add_sized(
                                [cell_width, header_height],
                                button
                                    .selected(selected)
                                    .sense(egui::Sense::click_and_drag()),
                            );
                            if f == 0 {
                                first_header = Some(response.rect);
                            }
                            if response.clicked() || response.double_clicked() {
                                header_selection = Some((f, ui.input(|input| input.modifiers)));
                            }
                            #[cfg(test)]
                            ui.ctx().data_mut(|data| {
                                data.insert_temp(egui::Id::new(("frame-header", f)), response.rect);
                            });
                            if response.drag_started_by(egui::PointerButton::Primary) {
                                response.dnd_set_drag_payload(FrameHeaderDrag {
                                    frame: f,
                                    tab: self.active_tab,
                                });
                            }
                            if let Some(drag) = response.dnd_hover_payload::<FrameHeaderDrag>()
                                && drag.tab == self.active_tab
                                && let Some(pointer) =
                                    ui.input(|input| input.pointer.interact_pos())
                            {
                                let after = pointer.x >= response.rect.center().x;
                                let x = if after {
                                    response.rect.right()
                                } else {
                                    response.rect.left()
                                };
                                ui.painter().vline(
                                    x,
                                    response.rect.y_range(),
                                    Stroke::new(2., ui.visuals().selection.stroke.color),
                                );
                                if response.dnd_release_payload::<FrameHeaderDrag>().is_some() {
                                    frame_drop = Some((drag.frame, f + usize::from(after)));
                                }
                            }
                            if response.double_clicked() {
                                self.start_rename_frame(f);
                            }
                            response.context_menu(|ui| {
                                if ui.pc_button("Rename frame…").clicked() {
                                    self.start_rename_frame(f);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        self.timeline_frames.selected.len() > 1
                                            && self
                                                .timeline_frames
                                                .selected
                                                .binary_search(&f)
                                                .is_ok(),
                                        egui::Button::new("Reverse selected frames"),
                                    )
                                    .clicked()
                                {
                                    reverse_header_selection = true;
                                    ui.close();
                                }
                            });
                            let tags: Vec<_> = self
                                .doc
                                .tags
                                .iter()
                                .filter(|tag| (tag.from..=tag.to).contains(&f))
                                .collect();
                            let mut description = match self.doc.frame_name(f) {
                                Some(name) => format!("Frame {}: {name}", f + 1),
                                None => format!("Select frame {}", f + 1),
                            };
                            if !tags.is_empty() {
                                use std::fmt::Write;
                                let marker_width = (response.rect.width() - 4.) / tags.len() as f32;
                                description.push_str(" · Tags");
                                for (index, tag) in tags.iter().enumerate() {
                                    let [r, g, b] = tag.color;
                                    let origin = response.rect.left_bottom()
                                        + Vec2::new(2. + marker_width * index as f32, -4.);
                                    ui.painter().rect_filled(
                                        Rect::from_min_size(origin, Vec2::new(marker_width, 3.)),
                                        0,
                                        Color32::from_rgb(r, g, b),
                                    );
                                    let _ = write!(description, " · {}", tag.name);
                                }
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::SelectableLabel,
                                    response.enabled(),
                                    selected,
                                    &description,
                                )
                            });
                            response.on_hover_text(description);
                        }
                        ui.end_row();
                        let cel_drag_active =
                            egui::DragAndDrop::has_payload_of_type::<CelDrag>(ui.ctx());
                        for (row, &li) in visible_rows.iter().enumerate() {
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
                                if l.group {
                                    let collapsed = self
                                        .studio
                                        .collapsed_groups
                                        .contains(&l.editor_id.to_string());
                                    let (icon, action) = if collapsed {
                                        ("chevron-right", "Expand")
                                    } else {
                                        ("chevron-down", "Collapse")
                                    };
                                    if ui_icons::local_icon_button(
                                        ui,
                                        icon,
                                        &format!("{action} group {}", l.name),
                                        false,
                                    )
                                    .clicked()
                                    {
                                        collapse = Some(li);
                                    }
                                }
                                let response = ui.add_sized(
                                    [label_width - if l.group { 118. } else { 90. }, row_height],
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
                                    .selected(
                                        li == self.layer
                                            || self
                                                .timeline_layers
                                                .selected
                                                .binary_search(&li)
                                                .is_ok(),
                                    )
                                    .sense(egui::Sense::click_and_drag())
                                    .truncate(),
                                );
                                #[cfg(test)]
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(
                                        egui::Id::new(("layer-header", li)),
                                        response.rect,
                                    );
                                });
                                if response.clicked() || response.double_clicked() {
                                    layer_header_selection =
                                        Some((li, ui.input(|input| input.modifiers)));
                                }
                                if response.double_clicked() {
                                    rename = Some(l.name.clone());
                                }
                                if response.drag_started_by(egui::PointerButton::Primary) {
                                    let layers =
                                        if self.timeline_layers.selected.binary_search(&li).is_ok()
                                        {
                                            self.timeline_layers.selected.clone()
                                        } else {
                                            vec![li]
                                        };
                                    response.dnd_set_drag_payload(LayerHeaderDrag {
                                        layers,
                                        grabbed: li,
                                        tab: self.active_tab,
                                        generation: self.power.bridge_generation,
                                    });
                                }
                                if let Some(drag) = response.dnd_hover_payload::<LayerHeaderDrag>()
                                    && drag.tab == self.active_tab
                                    && drag.generation == self.power.bridge_generation
                                    && let Some(pointer) =
                                        ui.input(|input| input.pointer.interact_pos())
                                {
                                    let y =
                                        (pointer.y - response.rect.top()) / response.rect.height();
                                    let target = if l.group && (0.3..=0.7).contains(&y) {
                                        LayerDrop::Into(li)
                                    } else if y < 0.5 {
                                        LayerDrop::Above(li)
                                    } else {
                                        LayerDrop::Below(li)
                                    };
                                    if self.doc.can_restack_layers(&drag.layers, target) {
                                        let stroke =
                                            Stroke::new(2., ui.visuals().selection.stroke.color);
                                        match target {
                                            LayerDrop::Into(_) => ui.painter().rect_stroke(
                                                response.rect.shrink(1.),
                                                3.,
                                                stroke,
                                                egui::StrokeKind::Inside,
                                            ),
                                            LayerDrop::Above(_) => ui.painter().hline(
                                                response.rect.x_range(),
                                                response.rect.top(),
                                                stroke,
                                            ),
                                            LayerDrop::Below(_) => ui.painter().hline(
                                                response.rect.x_range(),
                                                response.rect.bottom(),
                                                stroke,
                                            ),
                                        };
                                        if response
                                            .dnd_release_payload::<LayerHeaderDrag>()
                                            .is_some()
                                        {
                                            layer_drop = Some((drag, target));
                                        }
                                    }
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
                                    .selected(cel_selection.contains(li, f))
                                    .sense(egui::Sense::click_and_drag()),
                                );
                                if response.drag_started_by(egui::PointerButton::Primary) {
                                    let grabbed = (li, f);
                                    let selection = if cel_selection.contains(li, f) {
                                        cel_selection.clone()
                                    } else {
                                        CelSelection::Rectangle(CelRange::between(grabbed, grabbed))
                                    };
                                    if let Some(bounds) = selection.bounds() {
                                        response.dnd_set_drag_payload(CelDrag {
                                            selection,
                                            bounds,
                                            grabbed,
                                            tab: self.active_tab,
                                            generation: self.power.bridge_generation,
                                        });
                                    }
                                }
                                if cel_drag_active && ui.is_rect_visible(response.rect) {
                                    visible_cel_rects.push((li, f, response.rect));
                                    if let Some(drag) = response.dnd_hover_payload::<CelDrag>()
                                        && drag.tab == self.active_tab
                                        && drag.generation == self.power.bridge_generation
                                        && let Some(target) = drag.destination((li, f))
                                        && let Ok(preview) = self.doc.cel_selection_destination(
                                            &drag.selection,
                                            target.0,
                                            target.1,
                                        )
                                    {
                                        if response.dnd_release_payload::<CelDrag>().is_some() {
                                            cel_drop = Some((drag.clone(), target));
                                        }
                                        cel_drag_preview = Some((drag, preview));
                                    }
                                }
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        cel_selection.contains(li, f),
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
                                if let (Some(target), Some(bounds)) = (destination, menu_bounds)
                                    && target.contains(li, f)
                                    && cel_selection.contains(
                                        bounds.first_layer + li - target.first_layer,
                                        bounds.first_frame + f - target.first_frame,
                                    )
                                {
                                    ui.painter().rect_stroke(
                                        response.rect.shrink(1.),
                                        3.,
                                        Stroke::new(1.5, ui.visuals().selection.stroke.color),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if response.has_focus() {
                                    let navigation = ui.input_mut(|input| {
                                        let shift = egui::Modifiers::SHIFT;
                                        let move_layer =
                                            i32::from(input.consume_key(shift, Key::ArrowUp))
                                                - i32::from(
                                                    input.consume_key(shift, Key::ArrowDown),
                                                );
                                        let move_frame =
                                            i32::from(input.consume_key(shift, Key::ArrowRight))
                                                - i32::from(
                                                    input.consume_key(shift, Key::ArrowLeft),
                                                );
                                        (move_layer, move_frame)
                                    });
                                    if navigation != (0, 0) {
                                        let target_row = row
                                            .saturating_add_signed(-(navigation.0 as isize))
                                            .min(visible_rows.len() - 1);
                                        let layer = visible_rows[target_row];
                                        let frame = self
                                            .frame
                                            .saturating_add_signed(navigation.1 as isize)
                                            .min(self.doc.durations.len() - 1);
                                        selection = Some((layer, frame, true));
                                    }
                                }
                                if response.on_hover_text(&description).clicked() {
                                    let modifiers = ui.input(|input| input.modifiers);
                                    if modifiers.command && !modifiers.shift {
                                        toggle_cel = Some((li, f));
                                    } else {
                                        selection = Some((li, f, modifiers.shift));
                                    }
                                }
                            }
                            ui.end_row();
                        }
                        if let Some((drag, preview)) = cel_drag_preview {
                            for (layer, frame, rect) in visible_cel_rects {
                                if preview.contains(layer, frame)
                                    && drag.selection.contains(
                                        drag.bounds.first_layer + layer - preview.first_layer,
                                        drag.bounds.first_frame + frame - preview.first_frame,
                                    )
                                {
                                    ui.painter().rect_stroke(
                                        rect.shrink(1.),
                                        3.,
                                        Stroke::new(1.5, ui.visuals().selection.stroke.color),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                            }
                        }
                    });
                if let (Some(track), Some(header)) = (onion_track, first_header) {
                    self.timeline_onion_range(ui, track, header);
                }
            });
        if let Some((layer, frame, extend)) = selection {
            self.timeline_select(layer, frame, extend);
        }
        if let Some((layer, frame)) = toggle_cel {
            self.toggle_cel_selection(layer, frame);
        }
        if let Some((layer, modifiers)) = layer_header_selection {
            self.select_layer_header(layer, modifiers, &visible_rows);
        }
        if let Some((drag, target)) = layer_drop {
            self.drop_layers(&drag, target);
        }
        if let Some((frame, modifiers)) = header_selection {
            self.select_frame_header(frame, modifiers);
        }
        if let Some((drag, target)) = cel_drop {
            self.place_timeline_cels(&drag.selection, target, false);
        }
        if let Some((source, before)) = frame_drop {
            self.reorder_timeline_frames(source, before);
        }
        if reverse_header_selection {
            self.reverse_timeline_frames();
        }
        if let Some(layer) = collapse {
            self.toggle_timeline_group(layer);
        }
        ui.menu_button("Layer & frame actions", |ui| {
            for (label, command) in [
                ("Duplicate layer or group", "layer-duplicate"),
                ("Merge down", "layer-merge"),
            ] {
                if ui.pc_command_button(command, label).clicked() {
                    self.action(command);
                    ui.close();
                }
            }
            if ui.pc_button("Rename current frame…").clicked() {
                self.start_rename_frame(self.frame);
                ui.close();
            }
            let selection = self.timeline_cel_selection();
            ui.menu_button("Selected cels", |ui| {
                let Some(range) = selection.bounds() else {
                    return;
                };
                if let CelSelection::Individual(cells) = &selection {
                    ui.label(format!("{} selected cels", cells.len()));
                }
                ui.label(format!(
                    "Layers {}–{}, frames {}–{}",
                    range.first_layer + 1,
                    range.last_layer + 1,
                    range.first_frame + 1,
                    range.last_frame + 1
                ));
                if ui.pc_button("Clear cels").clicked() {
                    self.edit_timeline("Clear cels", |doc| doc.clear_selected_cels(&selection));
                    ui.close();
                }
                for (name, linked) in [
                    ("Link selected cels", true),
                    ("Unlink selected cels", false),
                ] {
                    if ui.pc_button(name).clicked() {
                        self.edit_timeline(
                            if linked { "Link cels" } else { "Unlink cels" },
                            |doc| doc.set_selected_cels_linked(&selection, linked),
                        );
                        ui.close();
                    }
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.timeline_duration)
                            .range(1..=65535)
                            .suffix(" ms"),
                    );
                    if ui.pc_button("Set duration").clicked() {
                        let duration = self.timeline_duration;
                        self.edit_timeline("Change frame durations", |doc| {
                            doc.set_selected_cel_duration(&selection, duration)
                        });
                        ui.close();
                    }
                });
                let mut target = self
                    .timeline_destination
                    .unwrap_or((range.first_layer, range.first_frame));
                let layer_span = range.last_layer - range.first_layer;
                let frame_span = range.last_frame - range.first_frame;
                let max_layer = self.doc.layers.len() - layer_span;
                let max_frame = self.doc.durations.len() - frame_span;
                let (mut first_layer, mut first_frame) = (target.0 + 1, target.1 + 1);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut first_layer)
                            .range(1..=max_layer)
                            .prefix("To layer "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut first_frame)
                            .range(1..=max_frame)
                            .prefix("To frame "),
                    );
                });
                target = (first_layer - 1, first_frame - 1);
                self.timeline_destination = Some(target);
                ui.label(format!(
                    "Destination ends at layer {}, frame {}",
                    first_layer + layer_span,
                    first_frame + frame_span
                ));
                for (name, duplicate) in [("Move cels", false), ("Duplicate cels", true)] {
                    if ui.pc_button(name).clicked() {
                        self.place_timeline_cels(&selection, target, duplicate);
                        self.timeline_destination = None;
                        ui.close();
                    }
                }
                if ui.pc_button("Cancel placement").clicked() {
                    self.timeline_destination = None;
                    ui.close();
                }
            });
            ui.menu_button("Position and opacity keyframe", |ui| {
                let (layer, frame) = (self.layer, self.frame);
                let mut key = self.doc.sampled_cel_key(layer, frame);
                let mut changed = false;
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut key.x)
                                .range(-16384..=16384)
                                .prefix("X "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut key.y)
                                .range(-16384..=16384)
                                .prefix("Y "),
                        )
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut key.opacity).prefix("Opacity "))
                        .changed();
                });
                changed |= ui
                    .pc_checkbox(&mut key.hold, "Hold until next keyframe")
                    .changed();
                if ui.pc_button("Set keyframe at current frame").clicked() || changed {
                    self.edit_timeline("Set cel keyframe", |doc| {
                        doc.set_cel_keyframe(layer, frame, Some(key))
                    });
                }
                if self.doc.layers[layer].cel_meta[frame].keyframe.is_some()
                    && ui.pc_button("Remove keyframe").clicked()
                {
                    self.edit_timeline("Remove cel keyframe", |doc| {
                        doc.set_cel_keyframe(layer, frame, None)
                    });
                }
            });
            let mut onion_included = !self.doc.layers[self.layer].exclude_from_onion;
            if ui
                .pc_checkbox(&mut onion_included, "Include layer in onion skin")
                .changed()
            {
                self.remember_action("Change onion-skin layer visibility");
                self.doc.layers[self.layer].exclude_from_onion = !onion_included;
            }

            if narrow {
                let mut ms = self.doc.durations[self.frame];
                if ui
                    .add(egui::DragValue::new(&mut ms).range(1..=65535).suffix(" ms"))
                    .changed()
                {
                    self.remember_action("Change frame duration");
                    self.doc.durations[self.frame] = ms;
                }
                for (command, label) in [("frame", "+ Frame"), ("layer", "+ Layer")] {
                    if ui.pc_command_button(command, label).clicked() {
                        a = Some(command);
                        ui.close();
                    }
                }
            }
            let mut link_duplicates = self.doc.layers[self.layer].link_duplicates;
            if ui
                .pc_checkbox(&mut link_duplicates, "Link duplicated cels")
                .changed()
            {
                self.remember_action("Change cel linking preference");
                self.doc.layers[self.layer].link_duplicates = link_duplicates;
            }
            if self.doc.editable(self.layer) {
                ui.menu_button("Link cel to frame", |ui| {
                    for frame in 0..self.doc.durations.len() {
                        if frame != self.frame
                            && ui.pc_button(format!("Frame {}", frame + 1)).clicked()
                        {
                            self.remember_action("Link cel");
                            if let Err(error) = self.doc.link_cel(self.layer, self.frame, frame) {
                                self.status = error;
                            }
                            ui.close();
                        }
                    }
                });
                if self.doc.layers[self.layer].cel_meta[self.frame]
                    .artwork_link
                    .is_some()
                    && ui.pc_button("Unlink cel").clicked()
                {
                    self.remember_action("Unlink cel");
                    if let Err(error) = self.doc.unlink_cel(self.layer, self.frame) {
                        self.status = error;
                    }
                    ui.close();
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
            self.remember_action("Change layer visibility");
            self.doc.layers[li].visible = !self.doc.layers[li].visible
        }
        if let Some(li) = lock {
            self.remember_action("Change layer lock");
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
        let ink = self.tool_ink();
        let before = self
            .selection
            .filter(|_| !ink.blends())
            .map(|_| self.doc.layers[self.layer].cels[self.frame].clone());
        let old_indices = self.doc.layers[self.layer].cel_meta[self.frame]
            .indices
            .clone();
        self.doc.drawing_index = self.drawing_palette_index(c);
        let l = self.layer;
        let f = self.frame;
        let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        let axes = self.drawing_axes();
        let mut coverage = ink.blends().then(|| {
            let mut mask = Document::new(self.doc.width, self.doc.height);
            mask.brush_shape = self.doc.brush_shape;
            mask
        });
        let (target, cel, color) = if let Some(mask) = &mut coverage {
            (mask, (0, 0), [255; 4])
        } else {
            (&mut self.doc, (l, f), c)
        };
        match self.tool {
            Tool::Line => target.line_on_axes(cel, a, b, color, self.brush, axes),
            Tool::Rect => {
                if self.filled {
                    for y in y0..=y1 {
                        target.line_on_axes(cel, (x0, y), (x1, y), color, 1, axes)
                    }
                } else {
                    for (a, b) in [
                        ((x0, y0), (x1, y0)),
                        ((x1, y0), (x1, y1)),
                        ((x1, y1), (x0, y1)),
                        ((x0, y1), (x0, y0)),
                    ] {
                        target.line_on_axes(cel, a, b, color, self.brush, axes)
                    }
                }
            }
            Tool::Ellipse => {
                target.ellipse_on_axes(cel, (a, b), color, self.filled, self.brush, axes)
            }
            _ => {}
        }
        if let Some(mask) = coverage {
            let original = self.doc.layers[l].cels[f].clone();
            let color = self.doc.snapped_color(f, c);
            mask.layers[0].cels[0].visit_nonclear(|i, _| {
                if self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                    ink.draw(&mut self.doc, (l, f), i, original[i], color);
                }
            });
        }
        if let Some(before) = before {
            let implicit_slot = (self.doc.color_mode == ColorMode::Indexed
                && !self.doc.layers[l].background)
                .then_some(self.doc.transparent_index as u8);
            for i in 0..before.len() {
                let old_slot = old_indices.as_ref().map(|old| old[i]);
                let current_slot = self.doc.layers[l].cel_meta[f]
                    .indices
                    .as_ref()
                    .map(|p| p[i]);
                if !self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32)
                    && (self.doc.layers[l].cels[f][i] != before[i]
                        || current_slot.or(implicit_slot) != old_slot.or(implicit_slot))
                {
                    self.doc.restore_pixel(l, f, i, before[i], old_slot);
                }
            }
        }
        self.doc.drawing_index = None;
    }
    fn whole_pixel_wheel_scale(&mut self, ctx: &egui::Context) -> f32 {
        // A point-based wheel gesture (for example, a trackpad swipe) has
        // fractional motion. Collect it before taking a single discrete step.
        self.wheel_zoom_steps += ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, .. } => {
                        Some(if *unit == egui::MouseWheelUnit::Point {
                            delta.y / 24.
                        } else {
                            delta.y
                        })
                    }
                    _ => None,
                })
                .sum::<f32>()
        });
        let steps = self.wheel_zoom_steps.trunc();
        self.wheel_zoom_steps -= steps;
        if steps == 0. {
            return self.zoom;
        }
        let points = ctx.pixels_per_point();
        let mut scale = self.zoom * points;
        if (scale - scale.round()).abs() < 0.0001 {
            scale = scale.round();
        }
        for _ in 0..(steps.abs() as usize).min(40) {
            scale = if steps > 0. {
                if scale < 1. {
                    (scale * 1.2).min(1.)
                } else {
                    scale.floor() + 1.
                }
            } else if scale > 1. {
                scale.ceil() - 1.
            } else {
                scale / 1.2
            };
        }
        scale.clamp(
            self.minimum_canvas_zoom() * points,
            (40. * points).floor().max(1.),
        ) / points
    }

    fn set_drawing_grid(&mut self, grid: DrawingGrid) {
        if self.doc.drawing_grid() != grid && grid.valid() {
            self.remember_action("Set drawing grid");
            self.doc.drawing_grid = Some(grid);
        }
        self.show_drawing_grid = true;
    }

    fn drawing_grid_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut grid) = self.grid_settings else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Drawing Grid")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                Self::drawing_grid_controls(ui, &mut grid);
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(grid.valid(), egui::Button::new("Apply grid"))
                        .clicked();
                    cancel = ui.pc_button("Cancel").clicked();
                });
            });
        if apply {
            self.set_drawing_grid(grid);
        }
        self.grid_settings = (open && !apply && !cancel).then_some(grid);
    }

    fn drawing_grid_controls(ui: &mut egui::Ui, grid: &mut DrawingGrid) {
        for (axis, name) in ["X", "Y"].into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut grid.origin[axis])
                        .range(i32::from(i16::MIN)..=i32::from(i16::MAX))
                        .prefix(format!("{name} origin ")),
                );
                ui.add(
                    egui::DragValue::new(&mut grid.size[axis])
                        .range(1..=u32::from(u16::MAX))
                        .prefix(format!("{name} spacing ")),
                );
            });
        }
    }

    fn snapped_canvas_point(&self, point: (i32, i32), repeats: WrapAxes) -> (i32, i32) {
        if !self.snap_to_grid || self.doc.layers[self.layer].tileset.is_some() {
            return point;
        }
        let w = self.doc.width as i32;
        let h = self.doc.height as i32;
        let offset = (
            if repeats.horizontal {
                point.0.div_euclid(w) * w
            } else {
                0
            },
            if repeats.vertical {
                point.1.div_euclid(h) * h
            } else {
                0
            },
        );
        let local = (point.0 - offset.0, point.1 - offset.1);
        let snapped = self.doc.drawing_grid().snap(local, (0, w - 1, 0, h - 1));
        (snapped.0 + offset.0, snapped.1 + offset.1)
    }

    fn paint_drawing_grid(&self, painter: &egui::Painter, board: Rect, view: Rect) {
        let grid = self.doc.drawing_grid();
        let visible = view.intersect(board);
        let size = self.canvas_pixel_size();
        for (axis, from, to, base, pixel_extent) in [
            (0, visible.left(), visible.right(), board.left(), size.x),
            (1, visible.top(), visible.bottom(), board.top(), size.y),
        ] {
            let step = grid.size[axis] as f32 * pixel_extent;
            if step < 5. {
                continue;
            }
            let phase = base + grid.origin[axis] as f32 * pixel_extent;
            let first = ((from - phase) / step).ceil() as i32;
            let last = ((to - phase) / step).floor() as i32;
            for index in first..=last {
                let line = phase + index as f32 * step;
                let stroke = Stroke::new(0.75, self.colors().accent.linear_multiply(0.65));
                if axis == 0 {
                    painter.vline(line, visible.y_range(), stroke);
                } else {
                    painter.hline(visible.x_range(), line, stroke);
                }
            }
        }
    }

    fn marquee_bounds(&self, end: (i32, i32)) -> Option<SelectionBounds> {
        let (x, y) = self.start?;
        Some((x.min(end.0), y.min(end.1), x.max(end.0), y.max(end.1)))
    }

    fn paint_canvas_checker(&self, painter: &egui::Painter, board: Rect, r: Rect) {
        // Draw only visible checker squares, including at high zoom and while panning.
        let check = self.power.checker_size();
        let check_size = self.canvas_pixel_size() * check;
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
        for y in (((r.min.y - board.min.y) / check_size.y).floor() as i32).clamp(0, rows)
            ..(((r.max.y - board.min.y) / check_size.y).ceil() as i32).clamp(0, rows)
        {
            for x in (((r.min.x - board.min.x) / check_size.x).floor() as i32).clamp(0, cols)
                ..(((r.max.x - board.min.x) / check_size.x).ceil() as i32).clamp(0, cols)
            {
                if (x + y) % 2 == 0 {
                    let q = Rect::from_min_size(
                        board.min + Vec2::new(x as f32, y as f32) * check_size,
                        check_size,
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
    }

    fn toggle_split_canvas(&mut self) {
        self.cancel_gesture();
        if self.split_canvas.take().is_some() {
            self.active_canvas = CanvasPane::Primary;
        } else {
            self.split_canvas = Some(SplitCanvas {
                view: CanvasView {
                    zoom: self.zoom,
                    pan: Vec2::ZERO,
                    size: None,
                    initial_view: 1,
                    wheel_zoom_steps: 0.,
                },
                fraction: 0.5,
            });
        }
    }

    fn active_canvas_view(&self) -> CanvasView {
        if self.active_canvas == CanvasPane::Secondary
            && let Some(split) = &self.split_canvas
        {
            split.view
        } else {
            CanvasView {
                zoom: self.zoom,
                pan: self.pan,
                size: self.canvas_view_size,
                initial_view: self.initial_view,
                wheel_zoom_steps: self.wheel_zoom_steps,
            }
        }
    }

    fn set_active_canvas_zoom(&mut self, zoom: f32) {
        if self.active_canvas == CanvasPane::Secondary
            && let Some(split) = &mut self.split_canvas
        {
            split.view.zoom = zoom;
            split.view.initial_view = 0;
        } else {
            self.zoom = zoom;
        }
    }

    fn set_active_canvas_pan(&mut self, pan: Vec2) {
        if self.active_canvas == CanvasPane::Secondary
            && let Some(split) = &mut self.split_canvas
        {
            split.view.pan = pan;
        } else {
            self.pan = pan;
        }
    }

    fn swap_canvas_view(&mut self, alternate: &mut CanvasView) {
        std::mem::swap(&mut self.zoom, &mut alternate.zoom);
        std::mem::swap(&mut self.pan, &mut alternate.pan);
        std::mem::swap(&mut self.canvas_view_size, &mut alternate.size);
        std::mem::swap(&mut self.initial_view, &mut alternate.initial_view);
        std::mem::swap(&mut self.wheel_zoom_steps, &mut alternate.wheel_zoom_steps);
    }

    fn canvas_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(mut split) = self.split_canvas.take() else {
            self.canvas(ui);
            return;
        };
        if ui.available_width() <= 40. || ui.available_height() <= 0. {
            self.active_canvas = CanvasPane::Primary;
            if self.gesture_canvas == Some(CanvasPane::Secondary) {
                self.cancel_gesture();
            }
            self.canvas(ui);
            self.split_canvas = Some(split);
            return;
        }
        self.hover = None;
        let (area, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let divider_width = 6.;
        let divider_x = area.left() + split.fraction * area.width();
        let divider = Rect::from_center_size(
            Pos2::new(divider_x, area.center().y),
            Vec2::new(divider_width, area.height()),
        );
        let response = ui
            .interact(
                divider,
                ui.id().with("canvas-splitter"),
                egui::Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let minimum = (100. / area.width()).clamp(0.15, 0.45);
            split.fraction =
                ((pointer.x - area.left()) / area.width()).clamp(minimum, 1. - minimum);
            ui.ctx().request_repaint();
        }
        let primary = Rect::from_min_max(area.min, Pos2::new(divider.left(), area.bottom()));
        let secondary = Rect::from_min_max(Pos2::new(divider.right(), area.top()), area.max);
        for (pane, rect) in [
            (CanvasPane::Primary, primary),
            (CanvasPane::Secondary, secondary),
        ] {
            let mut child = ui.new_child(egui::UiBuilder::new().id_salt(pane).max_rect(rect));
            child.set_clip_rect(ui.clip_rect().intersect(rect));
            if pane == CanvasPane::Secondary {
                // Render the second camera through the same tools and frame cache.
                self.swap_canvas_view(&mut split.view);
                self.canvas_pane(&mut child, pane);
                self.swap_canvas_view(&mut split.view);
            } else {
                self.canvas_pane(&mut child, pane);
            }
        }
        let active = if self.active_canvas == CanvasPane::Primary {
            primary
        } else {
            secondary
        };
        ui.painter().rect_stroke(
            active.shrink(0.5),
            0.,
            Stroke::new(1., self.colors().accent),
            egui::StrokeKind::Inside,
        );
        let divider_fill = if response.hovered() || response.dragged() {
            self.colors().accent
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        ui.painter().rect_filled(divider, 0., divider_fill);
        self.split_canvas = Some(split);
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        self.hover = None;
        self.canvas_pane(ui, CanvasPane::Primary);
    }

    fn symmetry_axis(&self, index: usize) -> i32 {
        let extent = [self.doc.width, self.doc.height][index] as i32;
        self.studio.symmetry_axes[index]
            .unwrap_or(extent)
            .clamp(0, 2 * extent)
    }

    fn draw_symmetry_guides(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        board: Rect,
        pane: CanvasPane,
        can_edit: bool,
    ) -> bool {
        let visible = board.intersect(painter.clip_rect());
        if !visible.is_positive() {
            return false;
        }
        let step = self.canvas_pixel_size();
        let mut captured = false;
        for (index, enabled) in [self.studio.mirror_x, self.studio.mirror_y]
            .into_iter()
            .enumerate()
        {
            if !enabled {
                continue;
            }
            let x_axis = index == 0;
            let screen_axis =
                board.min[index] + self.symmetry_axis(index) as f32 * step[index] / 2.;
            let (start, end, center, cursor, hint) = if x_axis {
                (
                    Pos2::new(screen_axis, visible.top()),
                    Pos2::new(screen_axis, visible.bottom()),
                    Pos2::new(screen_axis, visible.top() + 7.),
                    egui::CursorIcon::ResizeHorizontal,
                    "Drag to move the left-right mirror axis",
                )
            } else {
                (
                    Pos2::new(visible.left(), screen_axis),
                    Pos2::new(visible.right(), screen_axis),
                    Pos2::new(visible.left() + 7., screen_axis),
                    egui::CursorIcon::ResizeVertical,
                    "Drag to move the top-bottom mirror axis",
                )
            };
            let handle = Rect::from_center_size(center, Vec2::splat(12.));
            let interactive = can_edit && self.gesture_canvas.is_none() && self.tool != Tool::Hand;
            let mut active = false;
            if interactive && handle.intersects(painter.clip_rect()) {
                let response = ui.interact(
                    handle.intersect(painter.clip_rect()),
                    ui.id().with(("symmetry-axis", pane, index)),
                    Sense::drag(),
                );
                captured |= response.contains_pointer() || response.is_pointer_button_down_on();
                active = response.hovered() || response.dragged();
                if (response.dragged_by(egui::PointerButton::Primary)
                    || response.drag_stopped_by(egui::PointerButton::Primary))
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let extent = [self.doc.width, self.doc.height][index] as i32;
                    let offset = ((pointer[index] - board.min[index]) * 2. / step[index]).round();
                    self.studio.symmetry_axes[index] = Some((offset as i32).clamp(0, 2 * extent));
                    ui.ctx().request_repaint();
                }
                response.on_hover_cursor(cursor).on_hover_text(hint);
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    let name = if x_axis {
                        "symmetry-x-handle"
                    } else {
                        "symmetry-y-handle"
                    };
                    if pane == CanvasPane::Primary {
                        data.insert_temp(egui::Id::new(name), handle);
                    }
                });
            }
            let accent = self.colors().accent;
            painter.line_segment(
                [start, end],
                Stroke::new(3., Color32::from_black_alpha(115)),
            );
            painter.line_segment(
                [start, end],
                Stroke::new(1., Color32::from_rgb(117, 222, 222)),
            );
            painter.rect_filled(
                handle.shrink(1.),
                2.,
                if active { self.colors().ink } else { accent },
            );
            painter.rect_stroke(
                handle.shrink(1.),
                2.,
                Stroke::new(1., Color32::from_rgb(117, 222, 222)),
                egui::StrokeKind::Inside,
            );
        }
        captured
    }

    fn canvas_pane(&mut self, ui: &mut egui::Ui, pane: CanvasPane) {
        if pane == CanvasPane::Primary {
            self.poll_canvas_transform(ui.ctx());
        }
        let (r, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            if pane == CanvasPane::Primary {
                d.insert_temp(egui::Id::new("canvas-bounds"), r);
            }
            d.insert_temp(egui::Id::new(("canvas-pane", pane)), r);
        });
        let view_changed = self.canvas_view_size.replace(r.size()) != Some(r.size());
        let previous_pan = self.pan;
        let previous_zoom = self.zoom;
        let painter = ui.painter_at(r);
        painter.rect_filled(r, 0, self.colors().canvas);
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let can_edit =
            ui.is_enabled() && !self.dialog_open() && self.power.reference_edit.is_none();
        if can_edit && response.hovered() && ctx.input(|i| i.pointer.any_pressed()) {
            self.active_canvas = pane;
        }
        if let Some(force) = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| {
                if let egui::Event::Touch {
                    phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                    force: Some(force),
                    ..
                } = e
                {
                    force.is_finite().then_some(*force)
                } else {
                    None
                }
            })
        }) {
            self.studio.force = force.clamp(0., 1.);
            self.studio.force_available = true;
        } else if self.gesture_canvas.is_none() {
            self.studio.force = 1.;
            self.studio.force_available = false;
        }
        if can_edit && response.hovered() {
            ctx.set_cursor_icon(if self.tool == Tool::Hand {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
            let old = self.zoom;
            self.zoom = if self.power.whole_pixel_wheel_zoom() {
                self.whole_pixel_wheel_scale(&ctx)
            } else {
                self.wheel_zoom_steps = 0.;
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.01 {
                    (old * (scroll * 0.003).exp()).clamp(self.minimum_canvas_zoom(), 40.)
                } else {
                    old
                }
            };
            if self.zoom != old
                && let Some(p) = pointer
            {
                let offset = p - r.center() - self.pan;
                self.pan -= offset * (self.zoom / old - 1.);
            }
        } else {
            self.wheel_zoom_steps = 0.;
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
                let limit = if pane == CanvasPane::Secondary {
                    self.canvas_fit_scale(Some(r.size()))
                } else {
                    let display = self.sprite_display_size();
                    ((r.width() - 16.) / display.x)
                        .min((r.height() - 16.) / display.y)
                        .floor()
                        .max(1.)
                };
                self.zoom = self.zoom.min(limit);
            }
        }
        let pixel_size = self.canvas_pixel_size();
        let board = Rect::from_center_size(
            r.center() + self.pan,
            self.sprite_display_size() * self.zoom,
        );
        if view_changed || self.pan != previous_pan || self.zoom != previous_zoom {
            ctx.request_repaint();
        }
        let repeats = self.studio.seamless;
        let rx = i32::from(repeats.horizontal);
        let ry = i32::from(repeats.vertical);
        let mut copies = [board; 9];
        let mut count = 0;
        for y in -ry..=ry {
            for x in -rx..=rx {
                let copy = board.translate(Vec2::new(
                    x as f32 * board.width(),
                    y as f32 * board.height(),
                ));
                if copy.intersects(r) {
                    self.paint_canvas_checker(&painter, copy, r);
                    copies[count] = copy;
                    count += 1;
                }
            }
        }
        let copies = &copies[..count];
        self.draw_references_pass(ui, board, true, false);
        if self.onion && !self.playing && pane == CanvasPane::Primary {
            let pixels = pixelcraft::productivity::onion_pixels(
                &self.doc,
                self.frame,
                &self.power.preferences_onion(),
            );
            let pixels = self.display_artwork(&self.doc.color_profile, pixels);
            let im = color_image(&pixels, self.doc.width, self.doc.height);
            if let Some(t) = &mut self.onion_texture {
                t.set(im, egui::TextureOptions::NEAREST);
            } else {
                self.onion_texture =
                    Some(ctx.load_texture("onion", im, egui::TextureOptions::NEAREST));
            }
            self.texture = self.onion_texture.clone();
        } else if !self.onion || self.playing {
            self.texture = Some(self.frame_texture(&ctx, self.frame));
        }
        let rotation_preview = self.rotation_preview(ui, board);
        if let Some(t) = self
            .filter_texture()
            .or(rotation_preview.as_ref())
            .or(self.canvas_transform_texture())
            .or(self.texture.as_ref())
        {
            for &copy in copies {
                painter.image(
                    t.id(),
                    copy,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    Color32::WHITE,
                );
            }
        }
        self.draw_sprite_text(&painter, board);
        if pane == CanvasPane::Primary {
            self.draw_references(ui, board);
        } else {
            self.draw_references_pass(ui, board, false, false);
        }
        if self.show_drawing_grid {
            for &copy in copies {
                self.paint_drawing_grid(&painter, copy, r);
            }
        }
        self.paint_tile_indices(&painter, board);
        self.paint_rotation_pivot(&painter, board);
        painter.rect_stroke(
            board.expand(1.),
            0,
            Stroke::new(1., self.colors().ink),
            egui::StrokeKind::Outside,
        );
        if self.grid && pixel_size.x.min(pixel_size.y) >= 5. {
            for x in 1..self.doc.width {
                let px = board.min.x + x as f32 * pixel_size.x;
                painter.line_segment(
                    [Pos2::new(px, board.min.y), Pos2::new(px, board.max.y)],
                    Stroke::new(0.5, Color32::from_black_alpha(45)),
                );
            }
            for y in 1..self.doc.height {
                let py = board.min.y + y as f32 * pixel_size.y;
                painter.line_segment(
                    [Pos2::new(board.min.x, py), Pos2::new(board.max.x, py)],
                    Stroke::new(0.5, Color32::from_black_alpha(45)),
                );
            }
        }
        let transforming = self.canvas_transform.is_some();
        let over_symmetry_handle =
            self.draw_symmetry_guides(ui, &painter, board, pane, can_edit && !transforming);
        let canvas_point = |p: Pos2| {
            r.contains(p).then(|| {
                (
                    ((p.x - board.min.x) / pixel_size.x).floor() as i32,
                    ((p.y - board.min.y) / pixel_size.y).floor() as i32,
                )
            })
        };
        let pos = pointer.and_then(canvas_point);
        let w = self.doc.width as i32;
        let h = self.doc.height as i32;
        let limits = (-rx * w, (rx + 1) * w - 1, -ry * h, (ry + 1) * h - 1);
        let on_canvas = pos.filter(|&(x, y)| {
            (limits.0..=limits.1).contains(&x) && (limits.2..=limits.3).contains(&y)
        });
        let hover = on_canvas.and_then(|p| repeats.position((self.doc.width, self.doc.height), p));
        if pointer.is_some_and(|p| r.contains(p)) {
            self.hover = hover;
        }
        let draws = matches!(
            self.tool,
            Tool::Pencil
                | Tool::Spray
                | Tool::Eraser
                | Tool::BlurBrush
                | Tool::JumbleBrush
                | Tool::Line
                | Tool::Rect
                | Tool::Ellipse
        );
        let accepts_copies = draws
            || matches!(self.tool, Tool::Fill | Tool::Wand | Tool::Picker)
            || ctx.input(|i| i.modifiers.alt);
        let pressed = ctx.input(|i| {
            [egui::PointerButton::Primary, egui::PointerButton::Secondary]
                .into_iter()
                .find(|&button| i.pointer.button_pressed(button))
        });
        if can_edit
            && !transforming
            && let Some(button) = pressed
            && self.gesture_canvas.is_none()
            && response.hovered()
            && !over_symmetry_handle
            && let Some(raw) = on_canvas
            && (accepts_copies || self.doc.index(raw.0, raw.1).is_some())
            && self.tool != Tool::Hand
        {
            self.stop_playback(false);
            let p = if draws {
                self.snapped_canvas_point(raw, repeats)
            } else {
                hover.unwrap()
            };
            let secondary = button == egui::PointerButton::Secondary;
            self.stroke_color = if secondary { self.bg } else { self.fg };
            let sample_color = ctx.input(|i| i.modifiers.alt);
            if sample_color || self.tool == Tool::Picker {
                let p = hover.unwrap();
                if sample_color
                    || self.tile_pixel_mode.is_some()
                    || !self.eyedropper_picks_tiles()
                    || !self.pick_tile(p)
                {
                    self.pick_color(p, secondary);
                }
            } else if self.tool == Tool::Spray {
                self.begin_spray(p, pane, button, &ctx);
            } else if matches!(self.tool, Tool::BlurBrush | Tool::JumbleBrush) {
                self.begin_retouch(pane, button, p);
            } else if self.tool == Tool::Curve {
                if secondary {
                    self.cancel_curve();
                } else {
                    self.curve_click(pane, self.snapped_canvas_point(raw, repeats));
                }
            } else if self.tool == Tool::Text {
                if !secondary {
                    self.begin_sprite_text(p);
                }
            } else if matches!(self.tool, Tool::PolygonLasso | Tool::Polygon) {
                if secondary {
                    self.finish_polygon();
                } else {
                    let operation = selection_tools::marquee_operation(ctx.input(|i| i.modifiers));
                    self.polygon_vertex(p, pane, operation);
                }
            } else if self.tool == Tool::Contour {
                if self.begin_painted_polygon(p, pane) {
                    self.gesture_canvas = Some(pane);
                    self.gesture_button = Some(button);
                    self.start = Some(p);
                }
            } else if self.tool == Tool::Wand {
                let op = ctx.input(|i| {
                    if i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl {
                        selection_tools::marquee_operation(i.modifiers)
                    } else {
                        self.power.selection_op
                    }
                });
                self.wand_at(p, op);
            } else if matches!(self.tool, Tool::Select | Tool::SelectEllipse) {
                let operation = selection_tools::marquee_operation(ctx.input(|i| i.modifiers));
                self.studio.marquee_operation = Some(operation);
                self.gesture_canvas = Some(pane);
                self.gesture_button = Some(button);
                self.start = Some(p);
                self.last = Some(p);
            } else if self.tool == Tool::Lasso {
                self.studio.mask = None;
                self.gesture_canvas = Some(pane);
                self.gesture_button = Some(button);
                self.start = Some(p);
                self.studio.polygon.clear();
                self.studio.polygon.push(p);
                self.selection = Some((p.0, p.1, p.0, p.1));
            } else if !self.doc.editable(self.layer) {
                self.status = "This layer is locked. Unlock it in the timeline to draw.".into();
            } else {
                self.remember_action(match self.tool {
                    Tool::Pencil => "Pencil stroke",
                    Tool::Eraser => "Eraser stroke",
                    Tool::Fill => "Fill",
                    Tool::Move => "Move selection",
                    Tool::Gradient => "Gradient",
                    Tool::Line => "Line",
                    Tool::Rect => "Rectangle",
                    Tool::Ellipse => "Ellipse",
                    _ => "Paint",
                });
                self.gesture_canvas = Some(pane);
                self.gesture_button = Some(button);
                self.start = Some(p);
                self.last = Some(p);
                self.stroke_base = Some(self.doc.clone());
                self.studio.stroke.clear();
                self.studio.stroke_force = None;
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
                    self.cancel_gesture();
                } else if matches!(self.tool, Tool::Pencil | Tool::Eraser) {
                    let color = if self.tool == Tool::Eraser {
                        CLEAR
                    } else {
                        self.stroke_color
                    };
                    if self.doc.layers[self.layer].tileset.is_none() {
                        self.stroke_stabilizer =
                            Some(StrokeStabilizer::new(p, self.brush_dynamics().stabilizer));
                        self.studio.stroke.push((p, self.studio.force));
                        self.studio.stroke_force = Some(self.studio.force);
                    }
                    self.paint_at(p, color);
                }
            }
        }
        let owns_gesture = self.gesture_canvas == Some(pane);
        let down = self
            .gesture_button
            .is_some_and(|button| ctx.input(|i| i.pointer.button_down(button)));
        if owns_gesture && can_edit && down && self.tool == Tool::Spray {
            let point = on_canvas.map(|point| self.snapped_canvas_point(point, repeats));
            self.spray_tick(point, &ctx);
        }
        let gradient_release = self.tool == Tool::Gradient
            && self
                .gesture_button
                .is_some_and(|button| ctx.input(|i| i.pointer.button_released(button)));
        if owns_gesture
            && can_edit
            && self.tool == Tool::Gradient
            && (down || gradient_release)
            && let (Some(a), Some((x, y))) = (self.start, pos)
        {
            let b = (x.clamp(0, w - 1), y.clamp(0, h - 1));
            if self.last != Some(b) {
                if let Some(base) = &self.stroke_base {
                    self.doc = base.clone();
                }
                self.gradient(a, b);
                self.last = Some(b);
            }
        }
        if owns_gesture && can_edit && down {
            if let (Some(a), Some(mut b)) = (self.start, pos) {
                if draws && repeats != WrapAxes::NONE {
                    b.0 = b.0.clamp(limits.0, limits.1);
                    b.1 = b.1.clamp(limits.2, limits.3);
                } else if !draws || !self.doc.wraparound {
                    b.0 = b.0.clamp(0, w - 1);
                    b.1 = b.1.clamp(0, h - 1);
                }
                if draws {
                    b = self.snapped_canvas_point(b, repeats);
                }
                match self.tool {
                    Tool::BlurBrush | Tool::JumbleBrush => self.extend_retouch(b),
                    Tool::Contour => self.contour_point(b),
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
                    Tool::Select | Tool::SelectEllipse => self.last = Some(b),
                    Tool::Pencil | Tool::Eraser => {
                        let color = if self.tool == Tool::Eraser {
                            CLEAR
                        } else {
                            self.stroke_color
                        };
                        if self.stroke_stabilizer.is_some() {
                            self.extend_freehand(Some(b), false, color);
                        } else if self.doc.layers[self.layer].tileset.is_some()
                            && self.tile_pixel_mode != Some(TilePixelEdit::CreateVariant)
                        {
                            self.paint_at(b, color);
                            self.last = Some(b);
                        } else {
                            self.stroke_line(self.last.unwrap_or(a), b, color);
                            self.last = Some(b);
                        }
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
        } else if owns_gesture {
            if self.stroke_stabilizer.is_some()
                && can_edit
                && self
                    .gesture_button
                    .is_some_and(|button| ctx.input(|i| i.pointer.button_released(button)))
            {
                let color = if self.tool == Tool::Eraser {
                    CLEAR
                } else {
                    self.stroke_color
                };
                let release_position = ctx.input(|input| {
                    input.events.iter().rev().find_map(|event| match event {
                        egui::Event::PointerButton {
                            pos,
                            button,
                            pressed: false,
                            ..
                        } if Some(*button) == self.gesture_button => Some(*pos),
                        _ => None,
                    })
                });
                let point = release_position
                    .and_then(canvas_point)
                    .or(pos)
                    .map(|(x, y)| {
                        let (x, y) = (x.clamp(limits.0, limits.1), y.clamp(limits.2, limits.3));
                        self.snapped_canvas_point((x, y), repeats)
                    });
                self.extend_freehand(point, true, color);
            }
            if matches!(self.tool, Tool::BlurBrush | Tool::JumbleBrush)
                && can_edit
                && self
                    .gesture_button
                    .is_some_and(|button| ctx.input(|i| i.pointer.button_released(button)))
                && let Some(raw) = on_canvas
            {
                self.extend_retouch(self.snapped_canvas_point(raw, repeats));
            }
            if self.tool == Tool::Contour
                && can_edit
                && self
                    .gesture_button
                    .is_some_and(|button| ctx.input(|i| i.pointer.button_released(button)))
            {
                if let Some((x, y)) = pos {
                    self.contour_point((x.clamp(0, w - 1), y.clamp(0, h - 1)));
                }
                self.finish_polygon();
            }
            if matches!(self.tool, Tool::Select | Tool::SelectEllipse)
                && can_edit
                && let Some(operation) = self.studio.marquee_operation
                && self
                    .gesture_button
                    .is_some_and(|button| ctx.input(|i| i.pointer.button_released(button)))
                && let Some(end) = pos
                    .map(|(x, y)| (x.clamp(0, w - 1), y.clamp(0, h - 1)))
                    .or(self.last)
                && let Some(bounds) = self.marquee_bounds(end)
            {
                if self.tool == Tool::SelectEllipse {
                    self.apply_ellipse_marquee(bounds, operation);
                } else {
                    self.apply_marquee(bounds, operation);
                }
            }
            if self.tool == Tool::Move
                && let (Some(a), Some(b)) = (self.start, self.last)
            {
                for p in &mut self.studio.polygon {
                    p.0 += b.0 - a.0;
                    p.1 += b.1 - a.1;
                }
            }
            self.cancel_gesture();
        }
        if !transforming && self.studio.polygon.len() > 1 {
            let points: Vec<_> = self
                .studio
                .polygon
                .iter()
                .chain(self.studio.polygon.first())
                .map(|p| board.min + Vec2::new(p.0 as f32 + 0.5, p.1 as f32 + 0.5) * pixel_size)
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(1., Color32::WHITE)));
        }
        if !transforming {
            self.draw_selection_mask(&painter, board, r);
        }
        if !transforming && self.tool == Tool::Curve {
            let preview = on_canvas
                .filter(|&(x, y)| self.doc.index(x, y).is_some())
                .map(|point| self.snapped_canvas_point(point, repeats));
            self.draw_curve_draft(&painter, board, pane, preview);
        }
        if matches!(
            self.tool,
            Tool::PolygonLasso | Tool::Polygon | Tool::Contour
        ) && !transforming
        {
            if self.tool != Tool::Contour && can_edit && response.double_clicked() {
                self.finish_polygon();
            }
            self.draw_polygon_draft(
                &painter,
                board,
                pane,
                pointer.filter(|position| r.contains(*position)),
            );
        }
        let candidate = if owns_gesture && self.studio.marquee_operation.is_some() {
            self.last.and_then(|end| self.marquee_bounds(end))
        } else {
            None
        };
        let rectangular_selection = self
            .selection
            .filter(|_| !transforming && self.studio.mask.is_none());
        for (index, bounds) in [rectangular_selection, candidate].into_iter().enumerate() {
            let Some((x0, y0, x1, y1)) = bounds else {
                continue;
            };
            let sr = Rect::from_min_max(
                board.min + Vec2::new(x0 as f32, y0 as f32) * pixel_size,
                board.min + Vec2::new((x1 + 1) as f32, (y1 + 1) as f32) * pixel_size,
            );
            if index == 1 && self.tool == Tool::SelectEllipse {
                painter.add(egui::Shape::ellipse_stroke(
                    sr.center(),
                    sr.size() / 2.,
                    Stroke::new(3., self.colors().ink),
                ));
                painter.add(egui::Shape::ellipse_stroke(
                    sr.center(),
                    sr.size() / 2.,
                    Stroke::new(1., Color32::WHITE),
                ));
                continue;
            }
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
        if can_edit {
            self.draw_canvas_transform(ui, &painter, board, pane);
        }
        if !transforming
            && self.tool == Tool::Spray
            && let Some(raw) = on_canvas
        {
            let (x, y) = self.snapped_canvas_point(raw, repeats);
            painter.add(egui::Shape::ellipse_stroke(
                board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * pixel_size,
                pixel_size * (self.spray_radius as f32 + 0.5),
                Stroke::new(1., Color32::WHITE),
            ));
        }
        if !transforming
            && let Some((x, y)) = on_canvas
            && matches!(
                self.tool,
                Tool::Pencil | Tool::Eraser | Tool::BlurBrush | Tool::JumbleBrush
            )
        {
            let raw = self.snapped_canvas_point((x, y), repeats);
            let (x, y) = if owns_gesture && self.stroke_stabilizer.is_some() {
                self.last.unwrap_or(raw)
            } else {
                raw
            };
            if (x, y) != raw {
                let center = |(x, y): (i32, i32)| {
                    board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * pixel_size
                };
                painter.line_segment(
                    [center((x, y)), center(raw)],
                    Stroke::new(1., self.colors().accent),
                );
            }
            let size = if owns_gesture && self.brush_dynamics().pressure_size {
                (self.brush as f32 * self.studio.force).round().max(1.) as i32
            } else {
                self.brush
            };
            let (width, height) = self.active_image_brush_size().unwrap_or((size, size));
            let size = pixel_size * Vec2::new(width as f32, height as f32);
            let p =
                board.min + Vec2::new((x - width / 2) as f32, (y - height / 2) as f32) * pixel_size;
            painter.rect_stroke(
                Rect::from_min_size(p, size),
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
    fn apply_replace_color(&mut self, request: ReplaceColor) {
        if request.source == request.replacement
            || (request.selection_only && self.selection.is_none())
        {
            return;
        }
        let layers = if request.all_layers {
            0..self.doc.layers.len()
        } else {
            self.layer..self.layer + 1
        };
        let frames = if request.all_frames {
            0..self.doc.durations.len()
        } else {
            self.frame..self.frame + 1
        };
        let width = self.doc.width;
        let height = self.doc.height;
        let region = if request.selection_only {
            let [x, y, w, h] = self.selection_bounds();
            [
                x.clamp(0, width as i32) as usize,
                y.clamp(0, height as i32) as usize,
                x.saturating_add(w).clamp(0, width as i32) as usize,
                y.saturating_add(h).clamp(0, height as i32) as usize,
            ]
        } else {
            [0, 0, width, height]
        };
        let [left, top, right, bottom] = region;
        let area = (right - left) * (bottom - top);
        let targets = layers
            .filter(|&layer| self.doc.editable(layer) && self.doc.layers[layer].tileset.is_none())
            .flat_map(|layer| frames.clone().map(move |frame| (layer, frame)));

        // Bound both the scan and the copy-on-write backing kept alive by Undo.
        const REPLACEMENT_BUDGET: usize = MAX_PIXELS / 4;
        let mut work = 0;
        let mut allocation = 0;
        for (layer, frame) in targets.clone() {
            let (cel_work, cel_allocation) = self.doc.color_replacement_cost(
                layer,
                frame,
                request.source,
                request.replacement,
                area,
            );
            work += cel_work;
            allocation += cel_allocation;
            if work > REPLACEMENT_BUDGET || allocation > REPLACEMENT_BUDGET {
                self.status =
                    "Color replacement is too large. Reduce the scope or selection.".into();
                return;
            }
        }

        let mut next = self.doc.clone();
        let mut count = 0;
        for (layer, frame) in targets {
            count += next.replace_color_in_cel(
                layer,
                frame,
                request.source,
                request.replacement,
                region,
                |index| {
                    !request.selection_only
                        || self.selected((index % width) as i32, (index / width) as i32)
                },
            );
        }
        if count == 0 {
            self.status = "No matching pixels changed".into();
        } else if let Err(error) = next.validate() {
            self.status = error;
        } else {
            self.remember_action("Replace color");
            self.doc = next;
            self.status = format!("Replaced color in {count} pixels");
        }
    }

    fn replace_color_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut request) = self.replace_color else {
            return;
        };
        let mut apply = false;
        let close = fluent::modal_dialog(ctx, "Replace Color", |ui| {
            for (label, color) in [
                ("Source color", &mut request.source),
                ("Replacement color", &mut request.replacement),
            ] {
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgba_unmultiplied(color)
                        .on_hover_text(label);
                    ui.label(label);
                });
            }
            ui.label("Scope");
            ui.pc_checkbox(&mut request.all_layers, "All unlocked image layers");
            ui.pc_checkbox(&mut request.all_frames, "All frames");
            ui.add_enabled_ui(self.selection.is_some(), |ui| {
                ui.pc_checkbox(&mut request.selection_only, "Limit to selection");
            });
            let editable =
                self.doc.editable(self.layer) && self.doc.layers[self.layer].tileset.is_none();
            if !request.all_layers && !editable {
                ui.small("Choose an unlocked image layer or change the scope.");
            }
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(
                        request.source != request.replacement && (request.all_layers || editable),
                        egui::Button::new("Replace"),
                    )
                    .clicked();
                apply || ui.pc_button("Cancel").clicked()
            })
            .inner
        });
        self.replace_color = (!close).then_some(request);
        if apply {
            self.apply_replace_color(request);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.shortcut_edit.is_some() {
            self.shortcut_edit_dialog(ctx);
            return;
        }
        if self.shortcut_settings_open {
            self.shortcut_settings_dialog(ctx);
            return;
        }
        self.replace_color_dialog(ctx);
        self.filters_dialog(ctx);
        self.sprite_text_dialog(ctx);
        self.palette_conversion_dialog(ctx);
        self.document_profile_dialog(ctx);
        self.selection_tools_dialog(ctx);
        if let Some(kind) = self.animation_export {
            let mut accept = false;
            let mut cancel = false;
            let title = match kind {
                "png-sequence" => "Export PNG sequence",
                "gif" => "Export animated GIF",
                "fli" => "Export FLI animation",
                "flc" => "Export FLC animation",
                _ => "Export MP4 video",
            };
            let dismiss = fluent::modal_dialog(ctx, title, |ui| {
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
                    cancel = ui.pc_button("Cancel").clicked();
                    cancel
                }
            });
            if dismiss || accept || cancel {
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
        if self.file_path.is_some() {
            self.file_path_dialog(ctx);
            return;
        }
        self.continue_script_tab_closes();
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
                            self.grid = self.power.default_grid();
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
                    self.remember_action("Rename layer");
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
        if let Some((document, frame_id, mut name)) = self.rename_frame.take() {
            let Some(frame) = (self.doc.editor_id == document)
                .then(|| {
                    (0..self.doc.durations.len()).find(|&f| self.doc.frame_export_id(f) == frame_id)
                })
                .flatten()
            else {
                return;
            };
            let mut done = false;
            let mut cancel = false;
            let dismissed = fluent::modal_dialog(ctx, "Rename frame", |ui| {
                ui.label(format!("Frame {}", frame + 1));
                ui.add(egui::TextEdit::singleline(&mut name).char_limit(MAX_FRAME_NAME_CHARS));
                ui.small("Leave blank to clear the name. Names are saved in .pixel projects and sheet/atlas metadata.");
                ui.horizontal(|ui| {
                    done = ui.pc_button("Save name").clicked();
                    cancel = ui.pc_button("Cancel").clicked();
                });
                done || cancel
            });
            if done {
                self.edit_timeline("Rename frame", |doc| doc.set_frame_name(frame, &name));
            } else if !cancel && !dismissed {
                self.rename_frame = Some((document, frame_id, name));
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
        self.poll_reference_sources(&ctx);
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
                    if self
                        .downloaded_update
                        .as_ref()
                        .is_some_and(|(tag, _)| tag != &release.tag)
                    {
                        self.downloaded_update = None;
                    }
                    if self.downloaded_update.is_none()
                        && self
                            .download_job
                            .as_ref()
                            .is_none_or(|(tag, _)| tag != &release.tag)
                    {
                        self.update_status = if release.is_newer_than(INSTALLED_VERSION) {
                            "A new version is available.".into()
                        } else {
                            "You are up to date.".into()
                        };
                    }
                    self.latest_release = Some(release);
                }
                Err(error) => {
                    if self.downloaded_update.is_none() && self.download_job.is_none() {
                        self.update_status = error;
                    }
                }
            }
            self.update_job = None;
        }
        if let Some((tag, job)) = &self.download_job
            && let Ok(result) = job.try_recv()
        {
            if self
                .latest_release
                .as_ref()
                .is_none_or(|release| &release.tag == tag)
            {
                match result {
                    Ok(download) => {
                        self.downloaded_update = Some((tag.clone(), download));
                        self.update_status =
                            "Download verified. Ready to install when you confirm.".into();
                    }
                    Err(e) => self.update_status = e,
                }
            }
            self.download_job = None;
        }
        self.start_update_download(&ctx, true);
        self.about_dialog(&ctx);
        self.settings_dialog(&ctx);
        self.show_script_ui(&ctx);
        self.poll_script();
        if self.script_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.script_job.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.status = "Finish the current extension before closing PixelCraft".into();
        } else if self.save_job.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.status = "Wait for the current save before closing PixelCraft".into();
        } else if ctx.input(|i| i.viewport().close_requested())
            && !self.dirty()
            && let Some(i) = self.tabs.iter().position(|t| {
                t.as_ref()
                    .is_some_and(|t| t.doc.differs_from_saved(&t.saved))
            })
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
            self.refresh_frame_cache(&ctx);
            ctx.request_repaint_after(self.update_playback_clock(Instant::now()));
        }
        if self.dialog_open() && (!self.rotation_preview_active() || self.pending.is_some()) {
            ui.disable();
        }
        egui::Panel::top("menu")
            .exact_size(if comfortable { 36. } else { 32. })
            .show(ui, |ui| self.menu(ui));
        if self.power.bridge_export.is_some() {
            egui::Panel::top("live-export-progress").show(ui, |ui| {
                ui.horizontal(|ui| self.live_export_status(ui));
            });
        }
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
                                (&tab.path, tab.doc.differs_from_saved(&tab.saved))
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
        #[cfg(test)]
        ctx.data_mut(|data| {
            data.remove::<egui::Rect>(egui::Id::new("pinned-preview-bounds"));
            data.remove::<egui::Rect>(egui::Id::new("inspector-bounds"));
        });
        if self.inspector_open || self.preview_settings.visible {
            let panel = egui::Panel::left("palette_panel")
                .default_size(264.)
                .size_range(180.0..=(ctx.content_rect().width() * 0.4).clamp(180., 420.))
                .resizable(true)
                .show(ui, |ui| {
                    if self.inspector_open {
                        self.inspector_contents(ui);
                    } else {
                        self.inspector_toggle(ui);
                        self.docked_preview(ui);
                    }
                });
            #[cfg(test)]
            if self.inspector_open {
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new("inspector-bounds"), panel.response.rect)
                });
            }
            let _ = panel;
        } else {
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
            .show(ui, |ui| self.canvas_workspace(ui));
        self.dialogs(&ctx);
        self.history_panel(&ctx);
        self.drawing_grid_dialog(&ctx);
        self.workspace_recovery_dialog(&ctx);
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
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.poll_display_profile(ui.ctx(), display_profiles::Surface::from_frame(frame));
        self.render(ui)
    }
}
fn color_image(p: &[Pixel], w: usize, h: usize) -> egui::ColorImage {
    let bytes: Vec<u8> = p.iter().flatten().copied().collect();
    egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)
}

fn standalone_check(root: PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    #[cfg(any(windows, target_os = "macos"))]
    {
        let webp = root.join("webp-animation-check.webp");
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
            return Err("WebP animation import mismatch".into());
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
            if headless_args
                .get(1)
                .is_some_and(|operation| operation == "lua-console")
            {
                pixelcraft::automation::run_cli_console(&headless_args[2..])
            } else {
                pixelcraft::automation::run_cli(&headless_args[1..]).map(|v| println!("{v}"))
            }
        } else {
            let mut live = None;
            let mut root = None;
            let (args, remainder) = headless_args[1..].as_chunks::<2>();
            for pair in args {
                let path = Some(std::path::Path::new(&pair[1]));
                match pair[0].as_str() {
                    "--live" if live.is_none() => live = path,
                    "--project-root" if root.is_none() => root = path,
                    _ => {
                        eprintln!("Use --mcp [--live BRIDGE_CONFIG] [--project-root DIRECTORY]");
                        std::process::exit(2);
                    }
                }
            }
            if !remainder.is_empty() {
                eprintln!("Use --mcp [--live BRIDGE_CONFIG] [--project-root DIRECTORY]");
                std::process::exit(2);
            }
            pixelcraft::automation::serve_mcp_with_root(live, root)
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
    fn replace_color_can_update_all_frames_of_one_layer() {
        let mut a = app();
        a.doc = Document::new(1, 1);
        a.doc.add_frame(0, false);
        a.doc.add_layer();
        let source = [12, 34, 56, 255];
        for layer in 0..2 {
            for frame in 0..2 {
                a.doc.set_pixel(layer, frame, 0, source);
            }
        }
        a.apply_replace_color(ReplaceColor {
            source,
            replacement: CLEAR,
            all_layers: false,
            all_frames: true,
            selection_only: false,
        });
        assert!(a.doc.layers[0].cels.iter().all(|cel| cel[0] == CLEAR));
        assert!(a.doc.layers[1].cels.iter().all(|cel| cel[0] == source));
    }

    #[test]
    fn replace_color_limits_large_transparent_regions_but_can_update_a_small_selection() {
        let mut a = app();
        a.doc = Document::new(5000, 5000);
        let color = [12, 34, 56, 255];
        let mut request = ReplaceColor {
            source: CLEAR,
            replacement: color,
            all_layers: false,
            all_frames: false,
            selection_only: false,
        };
        a.apply_replace_color(request);
        assert!(a.undo.is_empty());
        assert_eq!(a.doc.layers[0].cels[0].allocation_pixels(), 0);
        a.selection = Some((1, 2, 1, 2));
        request.selection_only = true;
        a.apply_replace_color(request);
        assert_eq!(a.doc.layers[0].cels[0][10001], color);
        assert_eq!(a.doc.layers[0].cels[0][10000], CLEAR);
    }

    #[test]
    fn replace_color_selection_across_cels_is_one_undo_and_respects_locked_layers() {
        let mut a = app();
        a.doc = Document::new(3, 1);
        a.doc.add_frame(0, false);
        a.doc.add_layer();
        a.doc.add_layer();
        let source = [12, 34, 56, 255];
        let replacement = [89, 1, 23, 128];
        for layer in 0..3 {
            for frame in 0..2 {
                for x in 0..3 {
                    a.doc.set_pixel(layer, frame, x, source);
                }
            }
        }
        a.doc.layers[2].locked = true;
        a.selection = Some((0, 0, 2, 0));
        a.studio.mask = Some(vec![true, false, true]);
        let before = a.doc.clone();
        a.apply_replace_color(ReplaceColor {
            source,
            replacement,
            all_layers: true,
            all_frames: true,
            selection_only: true,
        });
        for layer in &a.doc.layers[..2] {
            for cel in &layer.cels[..2] {
                assert_eq!(cel.to_vec(), vec![replacement, source, replacement]);
            }
        }
        assert_eq!(a.doc.layers[2], before.layers[2]);
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert_eq!(a.doc, before);
    }

    #[test]
    fn replace_color_active_cel_leaves_linked_other_frames_and_other_layers_intact() {
        let mut a = app();
        a.doc = Document::new(2, 1);
        a.doc.add_frame(0, false);
        a.doc.add_layer();
        let source = [12, 34, 56, 255];
        a.doc.set_pixel(0, 0, 0, source);
        a.doc.set_pixel(1, 0, 0, source);
        a.doc.link_cel(0, 1, 0).unwrap();
        a.apply_replace_color(ReplaceColor {
            source,
            replacement: CLEAR,
            all_layers: false,
            all_frames: false,
            selection_only: false,
        });
        assert_eq!(a.doc.layers[0].cels[0][0], CLEAR);
        assert_eq!(a.doc.layers[0].cels[1][0], source);
        assert_eq!(a.doc.layers[1].cels[0][0], source);
        a.doc.validate().unwrap();
        a.undo();
        assert!(a.doc.layers[0].cel_meta[0].artwork_link.is_some());
    }

    #[test]
    fn undo_history_panel_clicks_restore_only_the_active_tab() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 4), None);
        a.action("layer");
        let draw = |a: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| a.history_panel(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        a.action("undo-history");
        draw(&mut a, vec![]);
        draw(&mut a, vec![]);
        let row = |index| {
            ctx.data_mut(|data| data.get_temp::<Rect>(egui::Id::new(("history-row", index))))
                .unwrap()
                .center()
        };
        let click = |a: &mut PixelCraft, pos| {
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            draw(a, vec![egui::Event::PointerMoved(pos), button(true)]);
            draw(a, vec![button(false)]);
        };
        click(&mut a, row(0usize));
        assert_eq!(a.doc.layers.len(), 1);
        assert_eq!(a.redo.len(), 1);

        a.open_document(Document::new(9, 2), None);
        draw(&mut a, vec![]);
        click(&mut a, row(0usize));
        assert_eq!(a.doc.width, 9);
        assert!(a.redo.is_empty());
        a.switch_tab(0);
        draw(&mut a, vec![]);
        click(&mut a, row(1usize));
        assert_eq!(a.doc.width, 4);
        assert_eq!(a.doc.layers.len(), 2);

        a.switch_tab(1);
        assert_eq!(a.doc.width, 9);
        assert_eq!(a.doc.layers.len(), 1);
    }

    #[test]
    fn labeled_history_jumps_both_ways_and_branches_at_the_chosen_entry() {
        let mut a = app();
        let red = [180, 30, 50, 255];
        a.remember_action("Pencil stroke");
        a.doc.set_pixel(0, 0, 0, red);
        a.action("frame");
        a.selection = Some((0, 0, 2, 2));
        a.remember_action("Clear selection");
        a.selection = None;
        assert_eq!(a.history_label(1), Some("Pencil stroke"));
        assert_eq!(a.history_label(2), Some("Duplicate frame"));
        assert_eq!(a.history_label(3), Some("Clear selection"));

        let revision = a.power.bridge_generation;
        assert!(a.jump_history(0));
        assert_eq!(a.power.bridge_generation, revision + 1);
        assert_eq!(a.doc.layers[0].cels[0][0], CLEAR);
        assert_eq!(a.history_label(2), Some("Duplicate frame"));
        assert!(a.jump_history(2));
        assert_eq!(a.doc.layers[0].cels[0][0], red);
        assert_eq!(a.frame, 1);
        assert_eq!(a.selection, Some((0, 0, 2, 2)));
        assert_eq!(a.history_label(3), Some("Clear selection"));

        a.action("layer");
        assert!(a.redo.is_empty());
        assert_eq!(a.history_label(3), Some("New layer"));
        assert!(!a.jump_history(4));
        assert!(a.jump_history(1));
        assert_eq!(a.doc.durations.len(), 1);
        assert!(a.jump_history(3));
        assert_eq!(a.doc.layers.len(), 2);
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
        assert!(a.dirty(), "Undo keeps the added frame's export ID reserved");
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
    fn clipboard_commands_create_distinct_destinations_and_keep_selection_holes() {
        let mut a = app();
        a.replace(Document::new(3, 2), None);
        let base = [160, 40, 30, 255];
        let top = [31, 111, 201, 128];
        a.doc.layers[0].cels[0] = vec![base; 6].into();
        a.doc.add_layer();
        a.layer = 1;
        a.doc.layers[1].cels[0] = vec![top; 6].into();
        a.set_selection_mask(vec![false, true, false, false, true, true]);

        let merged = a.doc.composite(0)[1];
        a.action("copy-merged");
        assert_eq!(
            a.clipboard.as_ref().unwrap().pixels,
            [merged, CLEAR, merged, merged]
        );
        a.doc.layers[1].locked = true;
        a.action("paste-as-layer");
        assert_eq!(a.doc.layers.len(), 3);
        assert_eq!(
            &a.doc.layers[2].cels[0].to_vec(),
            &[CLEAR, merged, CLEAR, CLEAR, merged, merged]
        );
        a.undo();
        assert_eq!(a.doc.layers.len(), 2);

        a.action("paste-as-reference");
        let reference = &a.doc.references[0];
        assert_eq!(
            (reference.x, reference.y, reference.width, reference.height),
            (1., 0., 2, 2)
        );
        assert_eq!(reference.pixels.to_vec(), [merged, CLEAR, merged, merged]);
        assert_eq!(a.doc.composite(0)[1], merged);
        a.undo();
        assert!(a.doc.references.is_empty());

        a.action("new-sprite-from-selection");
        assert_eq!((a.doc.width, a.doc.height), (2, 2));
        assert_eq!(a.doc.layers[0].cels[0].to_vec(), [top, CLEAR, top, top]);
        assert!(a.dirty());
        assert_eq!(a.clipboard.as_ref().unwrap().pixels[0], merged);
        a.action("paste-as-sprite");
        assert_eq!(a.tabs.len(), 3);
        assert_eq!(
            a.doc.layers[0].cels[0].to_vec(),
            [merged, CLEAR, merged, merged]
        );
        assert!(a.dirty());
    }

    #[test]
    fn clipboard_destinations_keep_native_requests_bound_to_their_original_site() {
        let mut a = app();
        let color = [31, 111, 201, 128];
        a.selection = Some((2, 3, 2, 3));
        a.clipboard_target = Some(a.clipboard_destination(PasteMode::Layer));
        a.apply_native_clipboard(system_clipboard::Completion::Paste(Ok((1, 1, vec![color]))));
        assert_eq!(a.doc.layers.len(), 2);
        assert_eq!(a.doc.layers[1].cels[0][26], color);

        a.clipboard_target = Some(a.clipboard_destination(PasteMode::Reference));
        a.open_document(Document::new(8, 8), None);
        a.apply_native_clipboard(system_clipboard::Completion::Paste(Ok((1, 1, vec![color]))));
        assert!(a.doc.references.is_empty());
        a.switch_tab(0);
        assert!(a.doc.references.is_empty());

        a.clipboard_target = Some(a.clipboard_destination(PasteMode::Reference));
        a.apply_native_clipboard(system_clipboard::Completion::Paste(Err(
            "No native bitmap".into()
        )));
        assert_eq!(a.doc.references[0].pixels[0], color);
    }

    #[test]
    fn new_sprite_from_selection_preserves_duplicate_indexed_swatches() {
        let mut a = app();
        let color = [51, 91, 151, 255];
        a.doc.import_palette(vec![CLEAR, color, color]).unwrap();
        a.doc.set_color_mode(ColorMode::Indexed);
        a.doc.drawing_index = Some(2);
        a.doc.set_pixel(0, 0, 9, color);
        a.doc.drawing_index = Some(1);
        a.doc.set_pixel(0, 0, 10, color);
        a.doc.drawing_index = None;
        a.selection = Some((1, 1, 2, 1));
        a.action("new-sprite-from-selection");
        assert_eq!(a.doc.color_mode, ColorMode::Indexed);
        assert_eq!(
            a.doc.layers[0].cel_meta[0]
                .indices
                .as_deref()
                .unwrap()
                .as_slice(),
            [2, 1]
        );
        a.doc.validate().unwrap();
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
    fn split_canvas_shares_artwork_and_undo_but_keeps_independent_views() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a.zoom = 10.;
        a.action("split-canvas");
        a.split_canvas.as_mut().unwrap().view.zoom = 20.;
        let draw = |a: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(950., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| a.canvas_workspace(ui));
                },
            );
            output.textures_delta.clear();
            output.shapes
        };
        for _ in 0..4 {
            draw(&mut a, vec![]);
        }
        let bounds = |pane| {
            ctx.data_mut(|d| d.get_temp::<Rect>(egui::Id::new(("canvas-pane", pane))))
                .unwrap()
        };
        let left = bounds(CanvasPane::Primary);
        let right = bounds(CanvasPane::Secondary);
        assert!(left.right() < right.left());
        assert_eq!(a.zoom, 10.);
        assert_eq!(a.split_canvas.as_ref().unwrap().view.zoom, 20.);
        let p = right.center() + Vec2::splat(10.);
        let q = left.center() + Vec2::splat(5.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let color = [50, 160, 102, 255];
        a.fg = color;
        draw(&mut a, vec![egui::Event::PointerMoved(p), button(p, true)]);
        draw(&mut a, vec![egui::Event::PointerMoved(q)]);
        draw(&mut a, vec![button(q, false)]);
        let cels = &a.doc.layers[0].cels[0];
        assert_eq!(cels[36], color);
        assert_eq!(cels.iter().filter(|&&p| p == color).count(), 1);
        assert_eq!(a.active_canvas, CanvasPane::Secondary);
        assert_eq!(a.undo.len(), 1);
        let shapes = draw(&mut a, vec![]);
        let texture = a.texture.as_ref().unwrap().id();
        let mesh_count = shapes
            .iter()
            .filter(|shape| {
                matches!(
                    &shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == texture
                )
            })
            .count();
        assert_eq!(mesh_count, 2);

        let divider = Pos2::new((left.right() + right.left()) / 2., left.center().y);
        let moved = divider + Vec2::new(100., 0.);
        draw(
            &mut a,
            vec![egui::Event::PointerMoved(divider), button(divider, true)],
        );
        draw(&mut a, vec![egui::Event::PointerMoved(moved)]);
        draw(&mut a, vec![button(moved, false)]);
        assert!(bounds(CanvasPane::Primary).right() > left.right() + 95.);
        assert_eq!(a.undo.len(), 1);

        a.set_active_canvas_zoom(12.);
        a.set_active_canvas_pan(Vec2::new(40., 24.));
        assert_eq!(a.zoom, 10.);
        assert_eq!(a.pan, Vec2::ZERO);
        assert_eq!(a.split_canvas.as_ref().unwrap().view.zoom, 12.);
        assert_eq!(
            a.split_canvas.as_ref().unwrap().view.pan,
            Vec2::new(40., 24.)
        );
        a.open_document(Document::new(2, 2), None);
        assert!(a.split_canvas.is_none());
        a.switch_tab(0);
        assert_eq!(a.active_canvas, CanvasPane::Secondary);
        assert_eq!(
            a.split_canvas.as_ref().unwrap().view.pan,
            Vec2::new(40., 24.)
        );
        a.undo();
        assert_eq!(a.doc.layers[0].cels[0][36], CLEAR);
    }

    #[test]
    fn selection_sets_the_grid_and_pointer_starts_on_a_grid_intersection() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a.zoom = 10.;
        a.selection = Some((1, 2, 3, 4));
        a.action("grid-from-selection");
        assert_eq!(a.doc.drawing_grid().origin, [1, 2]);
        assert_eq!(a.doc.drawing_grid().size, [3, 3]);
        a.action("snap-to-grid");
        a.selection = None;
        let draw = |a: &mut PixelCraft, events| {
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
        };
        draw(&mut a, vec![]);
        let p = Pos2::new(315., 295.); // Canvas pixel (5, 3), near grid intersection (4, 2).
        let button = |pressed| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        a.fg = [44, 50, 150, 255];
        draw(&mut a, vec![egui::Event::PointerMoved(p), button(true)]);
        draw(&mut a, vec![button(false)]);
        assert_eq!(a.doc.layers[0].cels[0][20], a.fg);
        assert_eq!(a.doc.layers[0].cels[0][29], CLEAR);
        assert_eq!(
            a.snapped_canvas_point((13, 3), WrapAxes::HORIZONTAL),
            (12, 2)
        );
        assert_eq!(
            a.snapped_canvas_point((-3, 3), WrapAxes::HORIZONTAL),
            (-4, 2)
        );
        a.undo();
        a.undo();
        assert_eq!(a.doc.drawing_grid(), DrawingGrid::default());
    }

    #[test]
    fn lua_clipboard_shares_editor_images_and_pastes_through_native_raster_editing() {
        let ctx = egui::Context::default();
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.replace(Document::new(4, 1), None);
        editor.open_document(Document::new(1, 1), None);
        editor.switch_tab(0);
        editor.doc.layers[0].cels[0].set(0, [3, 60, 90, 128]);
        editor.doc.layers[0].cels[0].set(1, [210, 60, 100, 255]);
        editor.doc.layers[0].cels[0].set(2, [10, 20, 30, 255]);
        editor.selection = Some((0, 0, 2, 0));
        editor.studio.mask = Some(vec![true, false, true, false]);
        editor.copy();
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-lua-clipboard-{}.lua",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"
 local sprite=app.activeSprite
 local fromEditor=app.clipboard.content
 assert(sprite.selection:contains(0,0) and not sprite.selection:contains(1,0))
 assert(app.clipboard.hasImage and fromEditor.image.width==3)
 assert(fromEditor.selection:contains(0,0) and fromEditor.selection:contains(2,0))
 assert(not fromEditor.selection:contains(1,0) and #fromEditor.palette>1)
 assert(app.clipboard.image:getPixel(0,0)==app.pixelColor.rgba(3,60,90,128))
 local text='café 🌿'..string.char(10)..'second line'
 app.clipboard.text=text
 assert(app.clipboard.hasText and not app.clipboard.hasImage)
 assert(app.clipboard.content.text==text)
 app.clipboard:clear()
 assert(not app.clipboard.hasImage and not app.clipboard.hasText)
 sprite.selection=Selection(Rectangle(1,0,2,1))
 sprite.selection:subtract(Rectangle(2,0,1,1))
 app.command.Copy()
 assert(app.clipboard.image.width==1)
 local im=Image(2,1,ColorMode.RGB)
 im:drawPixel(0,0,Color{r=80,g=110,b=140,a=128})
 im:drawPixel(1,0,Color('#ff0000'))
 local tiles=sprite:newTileset(Rectangle(0,0,2,2),2)
 tiles.name='Clipboard tiles'
 tiles.baseIndex=19
 tiles.properties.kind='terrain'
 tiles.properties('shared/namespace').grid=Point(2,2)
 tiles:tile(1).properties('source/only').density=0.75
 tiles:tile(1).image:drawPixel(1,0,Color('#112233'))
 app.clipboard.content={image=im,selection=Selection(Rectangle(10,20,1,1)),
  palette=Palette(2),tileset=tiles}
 sprite:deleteTileset(tiles)
 local saved=app.clipboard.content
 assert(saved.selection:contains(10,20) and saved.tileset.baseIndex==19)
 assert(saved.tileset.properties.kind=='terrain')
 assert(saved.tileset.properties('shared/namespace').grid==Point(2,2))
 saved.tileset:tile(1).properties('source/only').density=0.5
 assert(app.clipboard.content.tileset:tile(1).properties('source/only').density==0.75)
 app.command.Paste{position=Point(2,0)}
 local destination=app.sprites[2]
 app.activeSprite=destination
 destination.properties('destination/only').label='keep'
 destination.properties('shared/namespace').label='other'
 local imported=destination:newTileset(saved.tileset)
 assert(imported.properties('shared/namespace').grid==Point(2,2))
 assert(imported:tile(1).properties('source/only').density==0.5)
 assert(imported:tile(1).properties('destination/only').density==nil)
 assert(destination.properties('destination/only').label=='keep')
 assert(destination.properties('shared/namespace').label=='other')
 assert(imported:tile(1).image:getPixel(1,0)==app.pixelColor.rgba(17,34,51,255))
 assert(imported:tile(1).image:getPixel(0,1)==0)
 local map=destination:newTilemap(imported)
 app.useTool{layer=map,tool='pencil',points={Point(0,0)}}
 app.activeSprite=sprite
 "#,
        )
        .unwrap();
        editor.start_script(path.clone(), None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while editor.script_job.is_some() && Instant::now() < deadline {
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
                editor.show_script_ui(&ctx);
                editor.poll_clipboard(&ctx);
            });
            output.textures_delta.clear();
            editor.poll_script();
            std::thread::yield_now();
        }
        assert!(editor.script_job.is_none());
        assert_eq!(editor.status, "Applied extension changes");
        assert_eq!(
            editor.doc.layers[0].cels[0][2],
            over([10, 20, 30, 255], [80, 110, 140, 128], 255)
        );
        assert_eq!(editor.doc.layers[0].cels[0][3], CLEAR);
        assert_eq!(
            editor.clipboard.as_ref().unwrap().pixels[0],
            [80, 110, 140, 128]
        );
        assert_eq!(editor.undo.len(), 1);
        assert!(editor.doc.tilesets.is_empty());
        let imported = &editor.tabs[1].as_ref().unwrap().doc;
        assert_eq!(imported.tilesets[0].name, "Clipboard tiles");
        assert_eq!(imported.tilesets[0].tiles[0][1], [17, 34, 51, 255]);
        assert_eq!(imported.ase_tilesets[0].base_index, 19);
        assert_eq!(imported.layers[1].tile_cels[0], vec![1]);
        let ase = formats::encode_ase(imported).unwrap();
        let reopened = formats::decode_ase(&ase).unwrap();
        extensions::run_source(
            &reopened,
            r#"
 local sprite=app.activeSprite
 assert(sprite.properties('destination/only').label=='keep')
 assert(sprite.tilesets[1].properties.kind=='terrain')
 assert(sprite.tilesets[1].properties('shared/namespace').grid==Point(2,2))
 assert(sprite.tilesets[1]:tile(1).properties('source/only').density==0.5)
 "#,
            0,
            0,
            None,
        )
        .unwrap();
        let native = editor.clipboard.as_ref().unwrap().native_pixels();
        assert_eq!(native, vec![[80, 110, 140, 128], CLEAR]);
        editor.remember_native_image(2, 1, native);
        assert!(editor.clipboard.as_ref().unwrap().selection.is_some());
        assert!(editor.clipboard.as_ref().unwrap().tileset.is_some());
        assert_eq!(
            editor
                .clipboard
                .as_ref()
                .unwrap()
                .palette
                .as_ref()
                .unwrap()
                .len(),
            2
        );
        editor.remember_native_image(2, 1, vec![CLEAR; 2]);
        assert!(editor.clipboard.as_ref().unwrap().selection.is_none());
        assert!(editor.clipboard.as_ref().unwrap().tileset.is_none());
        assert!(editor.clipboard.as_ref().unwrap().palette.is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn lua_workspace_presents_tabs_before_completion_and_restores_failed_previews() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(3, 1), None);
        app.open_document(Document::new(2, 1), None);
        let original = app.doc.clone();
        let background = app.tabs[0].as_ref().unwrap().doc.clone();
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-lua-preview-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("preview.lua");
        formats::save(&directory.join("new.pixel"), &Document::new(4, 1)).unwrap();
        std::fs::write(
            &path,
            r#"
 app.activeSprite=app.sprites[1]
 app.useTool{tool='rectangular_marquee',points={Point(0,0),Point(2,0)}}
 app.useTool{tool='rectangular_marquee',points={Point(1,0)},selection=SelectionMode.SUBTRACT}
 app.useTool{tool='pencil',color=Color('#ff9966'),points={Point(2,0)}}
 app.refresh()
 local transient=app.open('new.pixel')
 app.activeImage:drawPixel(1,0,Color('#6699ff'))
 app.refresh()
 transient:close()
 error('rollback visible preview')
 "#,
        )
        .unwrap();
        app.start_script(path.clone(), None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut showed_background = false;
        let mut showed_mask = false;
        let mut showed_new_tab = false;
        let mut closed_new_tab = false;
        while app.script_job.is_some() && Instant::now() < deadline {
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| app.show_script_ui(&ctx));
            output.textures_delta.clear();
            if app.active_tab == 0 && app.doc.composite(0)[2] == [255, 153, 102, 255] {
                showed_background = true;
                showed_mask |= app.studio.mask.as_deref() == Some(&[true, false, true]);
            }
            if app.tabs.len() == 3
                && app.active_tab == 2
                && app.doc.composite(0)[1] == [102, 153, 255, 255]
            {
                showed_new_tab = true;
            }
            if showed_new_tab && app.tabs.len() == 2 && app.active_tab == 1 {
                closed_new_tab = true;
            }
            app.poll_script();
            std::thread::yield_now();
        }
        assert!(app.script_job.is_none(), "Lua script timed out");
        assert!(showed_background, "background preview never appeared");
        assert!(showed_mask, "script selection mask never appeared");
        assert!(showed_new_tab, "new tab preview never appeared");
        assert!(
            closed_new_tab,
            "closed preview stayed visible until completion"
        );
        assert!(
            app.status.contains("rollback visible preview"),
            "{}",
            app.status
        );
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active_tab, 1);
        assert_eq!(app.doc, original);
        assert_eq!(app.tabs[0].as_ref().unwrap().doc, background);
        assert!(app.tabs[0].as_ref().unwrap().mask.is_none());
        std::fs::write(
            &path,
            r#"
 local initial=app.activeSprite
 local target=app.sprites[1]
 app.activeSprite=target
 local listener
 listener=target.events:on('change',function()
  assert(app.activeImage:getPixel(1,0)==app.pixelColor.rgba(41,66,91,255))
  assert(target.selection:contains(0,0) and target.selection:contains(2,0))
  assert(not target.selection:contains(1,0))
  target.events:off(listener)
  app.activeSprite=initial
 end)
 "#,
        )
        .unwrap();
        app.start_script(path, None).unwrap();
        let mut painted = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.script_job.is_some() && Instant::now() < deadline {
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| app.show_script_ui(&ctx));
            output.textures_delta.clear();
            if !painted && app.script_ui.modeless && app.active_tab == 0 {
                app.remember_action("Paint while extension is open");
                app.doc.layers[0].cels[0].set(1, [41, 66, 91, 255]);
                app.set_selection_mask(vec![true, false, true]);
                painted = true;
            }
            app.poll_script();
            std::thread::yield_now();
        }
        assert!(app.script_job.is_none(), "Lua script timed out");
        assert!(painted);
        assert_eq!(app.status, "Applied extension changes");
        assert_eq!(app.active_tab, 1);
        assert_eq!(app.doc, original);
        let edited = app.tabs[0].as_ref().unwrap();
        assert_eq!(edited.doc.composite(0)[1], [41, 66, 91, 255]);
        assert_eq!(edited.undo.len(), 1);
        assert_eq!(edited.mask.as_deref(), Some(&[true, false, true][..]));
        assert_eq!(edited.undo[0].doc, background);
        assert_eq!(edited.undo[0].label, "Run extension");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn modeless_lua_tracks_layers_frames_and_cels_through_editor_timeline_edits() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(3, 1);
        doc.add_layer();
        doc.layers[1].name = "Keep".into();
        doc.add_frame(0, false);
        doc.layers[1].cels[1].set(0, [70, 80, 90, 255]);
        let original_cel = doc.layers[1].cel_meta[1].editor_id.to_string();
        let original_layer = doc.layers[1].editor_id.to_string();
        app.replace(doc, None);
        let directory = std::env::temp_dir().join(format!(
            "pixelcraft-lua-modeless-identity-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("identity.lua");
        std::fs::write(
            &path,
            r#"
 local sprite=app.activeSprite
 local original=sprite.layers[2]
 local frame=sprite.frames[2]
 local cel=original:cel(frame)
 local image=cel.image
 local created=sprite:newLayer()
 created.name='Script layer'
 local replaced=created:cel(1)
 local dialog=Dialog{title='Timeline identity'}
 dialog:show{wait=false}
 local listener,vacated,inserted
 local phase=0
 listener=sprite.events:on('change',function()
  phase=phase+1
  if phase==1 then
   assert(original==sprite.layers[1] and original.name=='Keep')
   assert(created==sprite.layers[2] and created.name=='Script layer')
   assert(frame==sprite.frames[3] and frame.frameNumber==3)
   inserted=sprite.frames[2]
   assert(cel==created:cel(1) and cel.layer==created and cel.frameNumber==1)
   vacated=original:cel(frame)
   assert(vacated~=cel)
   assert(not pcall(function()return replaced.opacity end))
   app.activeFrame=frame
  elseif phase==2 then
   assert(original==sprite.layers[2] and created==sprite.layers[3])
   assert(frame==sprite.frames[2] and cel==original:cel(frame))
   assert(replaced==created:cel(1) and replaced.opacity==255)
   assert(not pcall(function()return vacated.opacity end))
   assert(not pcall(function()return inserted.frameNumber end))
   app.activeFrame=frame
  else
   assert(phase==3 and frame==sprite.frames[3] and inserted==sprite.frames[2])
   assert(cel==created:cel(1) and vacated==original:cel(frame))
   assert(not pcall(function()return replaced.opacity end))
   image:drawPixel(2,0,Color('#0b2137'))
   sprite.events:off(listener)
   dialog:close()
  end
 end)
 "#,
        )
        .unwrap();
        app.start_script(path, None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut phase = 0;
        while app.script_job.is_some() && Instant::now() < deadline {
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| app.show_script_ui(&ctx));
            output.textures_delta.clear();
            if phase == 0 && app.script_ui.modeless && app.doc.layers.len() == 3 {
                app.remember_action("Edit timeline while extension is open");
                app.doc.swap_layers(0, 1);
                app.doc.swap_layers(1, 2);
                app.doc.add_frame(0, false);
                app.doc
                    .move_cel_range(CelRange::between((0, 2), (0, 2)), 1, 0, false)
                    .unwrap();
                phase = 1;
            } else if phase == 1 && app.frame == 2 {
                app.undo();
                phase = 2;
            } else if phase == 2 && app.frame == 1 {
                app.redo();
                phase = 3;
            }
            app.poll_script();
            std::thread::yield_now();
        }
        assert!(app.script_job.is_none(), "Lua script timed out");
        assert_eq!(
            phase, 3,
            "The modeless edits did not reach redo: {}",
            app.status
        );
        assert_eq!(app.status, "Applied extension changes");
        assert_eq!(app.doc.layers[0].name, "Keep");
        assert_eq!(app.doc.layers[0].editor_id.to_string(), original_layer);
        assert_eq!(app.doc.layers[1].name, "Script layer");
        assert_eq!(
            app.doc.layers[1].cel_meta[0].editor_id.to_string(),
            original_cel
        );
        assert_eq!(app.doc.layers[1].cels[0][0], [70, 80, 90, 255]);
        assert_eq!(app.doc.layers[1].cels[0][2], [11, 33, 55, 255]);
        assert_eq!(app.doc.layers[0].cels[2][0], CLEAR);
        assert_eq!(app.doc.layers[0].cels[2][2], CLEAR);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lua_file_workspace_commits_separate_editor_tabs_and_rolls_back_errors() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(3, 1), None);
        let original_id = a.doc.editor_id.to_string();
        let dir = std::env::temp_dir().join(format!("pixelcraft-lua-tabs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("second.pixel");
        let script = dir.join("script.lua");
        formats::save(&source, &Document::new(2, 2)).unwrap();
        let complete = |app: &mut PixelCraft| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.script_job.is_some() && Instant::now() < deadline {
                let mut output =
                    ctx.run_ui(egui::RawInput::default(), |_| app.script_ui.show(&ctx));
                output.textures_delta.clear();
                app.poll_script();
                std::thread::yield_now();
            }
            assert!(app.script_job.is_none(), "Lua script timed out");
        };
        std::fs::write(
            &script,
            r#"
 app.activeImage:drawPixel(2,0,Color('#110022'))
 app.activeSprite.selection=Selection(Rectangle(1,0,2,1))
 app.open('second.pixel')
 app.activeImage:drawPixel(1,0,Color('#004499'))
 app.activeSprite.selection=Selection(Rectangle(1,0,1,2))
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        complete(&mut a);
        assert_eq!(a.tabs.len(), 2, "{}", a.status);
        assert_eq!(a.path.as_deref(), Some(source.as_path()));
        assert_eq!(a.doc.composite(0)[1], [0, 68, 153, 255]);
        assert!(a.dirty());
        assert_eq!(a.selection, Some((1, 0, 1, 1)));
        assert_eq!(a.tabs[0].as_ref().unwrap().selection, Some((1, 0, 2, 0)));
        assert_eq!(
            a.tabs[0].as_ref().unwrap().doc.editor_id.to_string(),
            original_id
        );
        assert_eq!(
            a.tabs[0].as_ref().unwrap().doc.composite(0)[2],
            [17, 0, 34, 255]
        );
        let prior = a.doc.clone();
        std::fs::write(
            &script,
            r#"
 app.activeImage:drawPixel(1,0,Color('#ffffff'))
 app.open('second.pixel')
 error('test rollback')
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        complete(&mut a);
        assert_eq!(a.tabs.len(), 2);
        assert_eq!(a.doc, prior);
        assert!(a.status.contains("test rollback"));

        std::fs::write(
            &script,
            r#"
 local tabs=app.sprites
 assert(#tabs==2 and tabs[2]==app.activeSprite)
 local other=tabs[1]
 assert(other.width==3 and other.selection:contains(2,0))
 other.cels[1].image:drawPixel(0,0,Color('#449922'))
 assert(app.activeSprite==tabs[2])
 app.activeSprite=other
 assert(app.activeSprite==other)
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        complete(&mut a);
        assert_eq!(a.active_tab, 0, "{}", a.status);
        assert_eq!(a.doc.editor_id.to_string(), original_id);
        assert_eq!(a.doc.composite(0)[0], [68, 153, 34, 255]);
        assert_eq!(a.tabs[1].as_ref().unwrap().doc, prior);
        let other_before_failure = a.doc.clone();
        a.switch_tab(1);
        std::fs::write(
            &script,
            r#"
 app.sprites[1].cels[1].image:drawPixel(0,0,Color('#ffffff'))
 app.activeSprite=app.sprites[1]
 error('inactive tab rollback')
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        complete(&mut a);
        assert_eq!(a.active_tab, 1);
        assert_eq!(a.doc, prior);
        assert_eq!(a.tabs[0].as_ref().unwrap().doc, other_before_failure);
        assert!(a.status.contains("inactive tab rollback"));

        a.saved = a.doc.clone();
        std::fs::write(
            &script,
            r#"
 local original=app.activeSprite
 local transient=app.open('second.pixel')
 transient:close()
 original:close()
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        complete(&mut a);
        assert_eq!(a.tabs.len(), 1, "{}", a.status);
        assert_eq!(a.doc.editor_id.to_string(), original_id);
        assert_eq!(a.doc.composite(0)[2], [17, 0, 34, 255]);
        a.saved = a.doc.clone();
        a.open_document(Document::new(1024, 1024), None);
        a.open_document(Document::new(3, 1), None);
        a.doc.layers[0].cels[0].set(1, [4, 57, 6, 255]);
        a.open_document(Document::new(3, 1), None);
        std::fs::write(
            &script,
            r#"
 local a,large,b,current=table.unpack(app.sprites)
 assert(app.activeSprite==current and #app.sprites==4)
 local ok,errorMessage=pcall(function() return large.width end)
 assert(not ok and errorMessage:find('one million pixels',1,true))
 assert(app.activeSprite==current)
 large:close()
 b:close()
 current.cels[1].image:drawPixel(1,0,Color('#119944'))
 current:close()
 app.activeSprite=a
 "#,
        )
        .unwrap();
        a.start_script(script.clone(), None).unwrap();
        a.request(Pending::CloseTab);
        assert_eq!(a.tabs.len(), 4, "editor tabs must stay stable during Lua");
        complete(&mut a);
        assert_eq!(a.tabs.len(), 3, "{}", a.status);
        assert_eq!(a.doc.composite(0)[1], [4, 57, 6, 255]);
        assert!(matches!(a.pending, Some(Pending::CloseTab)));

        // Cancelling a close leaves that tab intact and advances to the next.
        a.pending = None;
        a.continue_script_tab_closes();
        assert_eq!(a.doc.composite(0)[1], [17, 153, 68, 255]);
        assert!(matches!(a.pending, Some(Pending::CloseTab)));
        let pending = a.pending.take().unwrap();
        a.doc = a.saved.clone();
        a.perform(pending);
        a.continue_script_tab_closes();
        assert_eq!(a.tabs.len(), 2);
        assert_eq!(a.active_tab, 0);
        assert_eq!(a.doc, other_before_failure);
        assert_eq!(
            a.tabs[1].as_ref().unwrap().doc.composite(0)[1],
            [4, 57, 6, 255]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raster_layer_import_crops_and_pads_only_the_target_frame_and_undoes() {
        let mut a = app();
        let mut target = Document::new(3, 2);
        target.add_frame(0, true);
        target.layers[0].cels[0].set(0, [80, 40, 10, 255]);
        a.replace(target, None);
        a.frame = 1;
        let mut source = Document::new(4, 1);
        source.layers[0].cels[0] = vec![
            [10, 0, 0, 255],
            [20, 0, 0, 128],
            [30, 0, 0, 255],
            [40, 0, 0, 255],
        ]
        .into();
        let before = a.doc.clone();
        a.install_raster_layer(Path::new("Overlay.png"), &source, 1)
            .unwrap();
        assert_eq!((a.doc.width, a.doc.height, a.frame, a.layer), (3, 2, 1, 1));
        assert_eq!(a.doc.layers[1].name, "Overlay");
        assert_eq!(
            a.doc.layers[1].cels[0].iter().copied().collect::<Vec<_>>(),
            [CLEAR; 6]
        );
        assert_eq!(
            a.doc.layers[1].cels[1].iter().copied().collect::<Vec<_>>(),
            [
                [10, 0, 0, 255],
                [20, 0, 0, 128],
                [30, 0, 0, 255],
                CLEAR,
                CLEAR,
                CLEAR,
            ]
        );
        assert_eq!(a.history_label(1), Some("Import as layer"));
        let after = a.doc.clone();
        a.undo();
        assert_eq!(a.doc, before);
        a.redo();
        assert_eq!(a.doc, after);
        assert!(
            a.install_raster_layer(Path::new("Other.png"), &source, 20)
                .is_err()
        );
        assert_eq!(a.doc, after);
    }

    #[test]
    fn queued_raster_layer_import_follows_the_original_frame_identity() {
        let dir =
            std::env::temp_dir().join(format!("pixelcraft-layer-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Stamp.png");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([70, 40, 30, 255]))
            .save(&path)
            .unwrap();
        let mut a = app();
        let mut target = Document::new(2, 1);
        target.add_frame(0, true);
        a.replace(target, None);
        a.frame = 1;
        a.open_as_raster_layer(path.clone());
        a.doc.add_frame(0, false);

        let complete = |app: &mut PixelCraft| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.open_job.is_some() && Instant::now() < deadline {
                app.poll_open_document();
                std::thread::yield_now();
            }
            assert!(app.open_job.is_none(), "Raster import timed out");
        };
        complete(&mut a);
        assert_eq!(a.status, "Imported image as layer");
        assert_eq!(a.frame, 2);
        assert_eq!(a.doc.layers[1].cels[2][0], [70, 40, 30, 255]);
        a.open_as_raster_layer(path);
        a.replace(Document::new(2, 1), None);
        complete(&mut a);
        assert!(a.status.contains("target is no longer active"));
        assert_eq!(a.doc.layers.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_pixel_aspect_maps_pointer_and_grid_in_sprite_coordinates() {
        let mut bytes = formats::encode_ase(&Document::new(4, 4)).unwrap();
        bytes[34..36].copy_from_slice(&[2, 1]);
        bytes[36..44].copy_from_slice(&[1, 0, 1, 0, 2, 0, 2, 0]);
        let doc = formats::decode_ase(&bytes).unwrap();
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(doc, None);
        a.zoom = 10.;
        a.action("snap-to-grid");
        assert_eq!(
            a.doc.drawing_grid(),
            DrawingGrid {
                origin: [1, 1],
                size: [2, 2]
            },
        );
        let draw = |a: &mut PixelCraft, events| {
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
        };
        draw(&mut a, vec![]);
        let pos = Pos2::new(329., 316.);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        a.fg = [20, 30, 90, 255];
        draw(&mut a, vec![egui::Event::PointerMoved(pos), button(true)]);
        draw(&mut a, vec![button(false)]);
        assert_eq!(a.doc.layers[0].cels[0][15], a.fg);
        assert_eq!(
            a.doc.layers[0].cels[0]
                .iter()
                .filter(|&&pixel| pixel != CLEAR)
                .count(),
            1
        );
    }

    #[test]
    fn seamless_fill_and_brush_footprints_honor_the_view_axes() {
        let mut a = app();
        a.replace(Document::new(3, 3), None);
        let barrier = [29, 80, 114, 255];
        let paint = [220, 50, 80, 255];
        for index in [1, 3, 4, 5, 7, 8] {
            a.doc.set_pixel(0, 0, index, barrier);
        }
        a.doc.wraparound = true;
        a.action("seamless-horizontal");
        assert_eq!(a.studio.seamless, WrapAxes::HORIZONTAL);
        a.power_fill((0, 0), paint);
        assert_eq!(a.doc.layers[0].cels[0][2], paint);
        assert_eq!(a.doc.layers[0].cels[0][6], CLEAR);
        a.action("seamless-vertical");
        assert_eq!(a.studio.seamless, WrapAxes::VERTICAL);
        a.doc.set_pixel(0, 0, 0, CLEAR);
        a.power_fill((0, 0), paint);
        assert_eq!(a.doc.layers[0].cels[0][6], paint);
        assert!(a.doc.wraparound);

        a.doc = Document::new(4, 4);
        a.studio.seamless = WrapAxes::HORIZONTAL;
        a.doc.brush_shape = BrushShape::Square;
        a.brush = 3;
        a.paint_at((0, 0), paint);
        let pixels = &a.doc.layers[0].cels[0];
        assert_eq!(pixels[3], paint);
        assert_eq!(pixels[12], CLEAR);
        a.action("seamless");
        a.paint_at((0, 0), paint);
        assert_eq!(a.doc.layers[0].cels[0][15], paint);
        a.action("seamless");
        assert_eq!(a.studio.seamless, WrapAxes::NONE);
    }

    #[test]
    fn seamless_pointer_strokes_cross_only_selected_axes_and_undo_together() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 4), None);
        a.zoom = 10.;
        a.studio.seamless = WrapAxes::HORIZONTAL;
        let paint = [180, 25, 63, 255];
        a.fg = paint;
        let draw = |a: &mut PixelCraft, events| {
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
        let drag = |a: &mut PixelCraft, from, to| {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            draw(a, vec![egui::Event::PointerMoved(from), button(from, true)]);
            draw(a, vec![egui::Event::PointerMoved(to)]);
            draw(a, vec![button(to, false)]);
        };
        // Begin in the left copy and end in the center sprite.
        drag(&mut a, Pos2::new(275., 295.), Pos2::new(285., 295.));
        let canvas = &a.doc.layers[0].cels[0];
        assert_eq!(&canvas.to_vec()[4..8], &[paint, CLEAR, CLEAR, paint]);
        assert_eq!(a.undo.len(), 1);
        // The horizontal view has no canvas above the sprite.
        drag(&mut a, Pos2::new(295., 275.), Pos2::new(295., 275.));
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert!(a.doc.composite(0).iter().all(|&p| p == CLEAR));
        a.studio.seamless = WrapAxes::VERTICAL;
        drag(&mut a, Pos2::new(295., 275.), Pos2::new(295., 285.));
        let canvas = &a.doc.layers[0].cels[0];
        assert_eq!(canvas[1], paint);
        assert_eq!(canvas[13], paint);
        assert_eq!(a.undo.len(), 1);
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
    fn undo_does_not_reissue_a_frame_export_identity() {
        let mut a = app();
        a.remember();
        a.doc.add_frame(0, false);
        let issued = a.doc.frame_export_id(1);
        a.undo();
        assert!(a.dirty(), "The persistent export ID counter must be saved");
        assert_eq!(a.doc.durations.len(), 1);
        let mut reopened =
            formats::decode_project(&formats::encode_project(&a.doc).unwrap()).unwrap();
        reopened.add_frame(0, false);
        assert_ne!(reopened.frame_export_id(1), issued);
        a.redo();
        assert_eq!(a.doc.frame_export_id(1), issued);
        a.undo();
        a.remember();
        a.doc.add_frame(0, false);
        assert_ne!(a.doc.frame_export_id(1), issued);
    }

    #[test]
    fn frame_header_selection_reorders_and_reverses_with_undo() {
        let mut a = app();
        a.doc = Document::new(2, 1);
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        a.select_frame_header(1, egui::Modifiers::NONE);
        a.select_frame_header(3, egui::Modifiers::COMMAND);
        assert_eq!(a.timeline_frames.selected, vec![1, 3]);
        a.reorder_timeline_frames(3, 5);
        assert_eq!(a.timeline_frames.selected, vec![3, 4]);
        assert_eq!(a.frame, 4);
        assert_eq!(a.doc.frame_export_id(3), 1);
        assert_eq!(a.doc.frame_export_id(4), 3);
        a.action("reverse-frames");
        assert_eq!(a.doc.frame_export_id(3), 3);
        assert_eq!(a.doc.frame_export_id(4), 1);
        a.undo();
        a.undo();
        assert_eq!(a.timeline_frames.selected, vec![1, 3]);
        assert_eq!(a.frame, 3);
        assert_eq!(a.doc.frame_export_id(1), 1);
        assert_eq!(a.doc.frame_export_id(3), 3);
        a.select_frame_header(4, egui::Modifiers::SHIFT);
        assert_eq!(a.timeline_frames.selected, vec![3, 4]);
    }

    #[test]
    fn layer_headers_select_nonadjacent_rows_and_drop_them_into_a_group() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 1), None);
        a.doc.layers[0].name = "Grass".into();
        a.doc.add_layer();
        a.doc.layers[1].name = "Path".into();
        a.doc.add_layer();
        a.doc.layers[2].name = "Flowers".into();
        let group = a.doc.add_group("Meadow").unwrap();
        let draw = |a: &mut PixelCraft, events| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| a.timeline(ui));
            });
            output.textures_delta.clear();
        };
        draw(&mut a, vec![]);
        let point = |layer: usize| {
            ctx.data_mut(|data| data.get_temp::<Rect>(egui::Id::new(("layer-header", layer))))
                .unwrap()
                .center()
        };
        let button = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let click = |a: &mut PixelCraft, layer, modifiers| {
            let pos = point(layer);
            draw(
                a,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerMoved(pos),
                    button(pos, true, modifiers),
                ],
            );
            draw(a, vec![button(pos, false, modifiers)]);
        };

        click(&mut a, 0, egui::Modifiers::NONE);
        click(&mut a, 2, egui::Modifiers::SHIFT);
        assert_eq!(a.timeline_layers.selected, [0, 1, 2]);
        click(&mut a, 1, egui::Modifiers::COMMAND);
        assert_eq!(a.timeline_layers.selected, [0, 2]);
        let from = point(2);
        let to = point(group);
        let none = egui::Modifiers::NONE;
        draw(
            &mut a,
            vec![
                egui::Event::ModifiersChanged(none),
                egui::Event::PointerMoved(from),
                button(from, true, none),
            ],
        );
        draw(&mut a, vec![egui::Event::PointerMoved(to)]);
        draw(&mut a, vec![button(to, false, none)]);
        assert_eq!(a.doc.layers[0].name, "Path");
        assert_eq!(a.doc.layers[1].parent, Some(group));
        assert_eq!(a.doc.layers[2].parent, Some(group));
        assert_eq!(a.timeline_layers.selected, [1, 2]);
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert_eq!(a.doc.layers[0].name, "Grass");
        assert_eq!(a.doc.layers[0].parent, None);
        assert_eq!(a.timeline_layers.selected, [0, 2]);
    }

    #[test]
    fn pointer_drag_of_frame_headers_uses_the_left_and_right_insertion_gaps() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 1), None);
        for frame in 0..3 {
            a.doc.add_frame(frame, false);
        }
        let draw = |a: &mut PixelCraft, events| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| a.timeline(ui));
            });
            output.textures_delta.clear();
        };
        draw(&mut a, vec![]);
        let header = |frame: usize| {
            ctx.data_mut(|data| data.get_temp::<Rect>(egui::Id::new(("frame-header", frame))))
                .unwrap()
        };
        let drag = |a: &mut PixelCraft, from, to| {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            draw(a, vec![egui::Event::PointerMoved(from), button(from, true)]);
            draw(a, vec![egui::Event::PointerMoved(to)]);
            draw(a, vec![button(to, false)]);
        };
        let after_last = Pos2::new(header(3).right() - 4., header(3).center().y);
        drag(&mut a, header(0).center(), after_last);
        assert_eq!(a.doc.frame_export_id(3), 0);
        let before_first = Pos2::new(header(0).left() + 4., header(0).center().y);
        drag(&mut a, header(3).center(), before_first);
        assert_eq!(a.doc.frame_export_id(0), 0);
        assert_eq!(a.undo.len(), 2);
    }

    #[test]
    fn pointer_drag_translates_overlapping_cels_and_restores_selection_on_undo() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 1), None);
        for frame in 0..3 {
            a.doc.add_frame(frame, false);
        }
        let red = [200, 32, 49, 255];
        let blue = [29, 119, 209, 255];
        a.doc.set_pixel(0, 0, 0, red);
        a.doc.set_pixel(0, 1, 0, blue);
        a.timeline_select(0, 0, false);
        a.timeline_select(0, 1, true);
        let previous_selection = a.timeline_range();
        let before = a.doc.clone();
        let draw = |a: &mut PixelCraft, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| a.timeline(ui));
            });
            output.textures_delta.clear();
        };
        draw(&mut a, vec![]);
        let point = |frame| {
            ctx.data_mut(|data| {
                data.get_temp::<(egui::Id, Rect, Rect, String)>(egui::Id::new((
                    "timeline-cell",
                    0usize,
                    frame,
                )))
            })
            .unwrap()
            .1
            .center()
        };
        let from = point(1usize);
        let to = point(2usize);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw(
            &mut a,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        draw(&mut a, vec![egui::Event::PointerMoved(to)]);
        draw(&mut a, vec![button(to, false)]);
        assert_eq!(a.doc.layers[0].cels[0][0], CLEAR);
        assert_eq!(a.doc.layers[0].cels[1][0], red);
        assert_eq!(a.doc.layers[0].cels[2][0], blue);
        assert_eq!(a.timeline_range(), CelRange::between((0, 1), (0, 2)));
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.timeline_range(), previous_selection);

        let click = |a: &mut PixelCraft, frame, modifiers| {
            let pos = point(frame);
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            };
            draw(
                a,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerMoved(pos),
                    button(true),
                ],
            );
            draw(a, vec![button(false)]);
        };
        click(&mut a, 0, egui::Modifiers::NONE);
        click(&mut a, 2, egui::Modifiers::COMMAND);
        let selected = a.timeline_cel_selection();
        assert!(selected.contains(0, 0) && selected.contains(0, 2));
        assert!(!selected.contains(0, 1));
        let from = point(2);
        let to = point(3);
        draw(
            &mut a,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        draw(&mut a, vec![egui::Event::PointerMoved(to)]);
        draw(&mut a, vec![button(to, false)]);
        assert_eq!(a.doc.layers[0].cels[1][0], red);
        assert_eq!(
            a.timeline_cel_selection().cells().collect::<Vec<_>>(),
            [(0, 1), (0, 3)]
        );
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.timeline_cel_selection(), selected);
    }

    #[test]
    fn nonadjacent_cels_keep_gaps_during_moves_and_history() {
        let mut a = app();
        a.doc = Document::new(2, 1);
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        let colors = [[12, 64, 120, 255], [80, 76, 22, 255], [91, 32, 51, 255]];
        for (frame, color) in [0, 2, 3].into_iter().zip(colors) {
            a.doc.set_pixel(0, frame, 0, color);
        }
        a.timeline_select(0, 0, false);
        a.toggle_cel_selection(0, 3);
        let selection = a.timeline_cel_selection();
        assert!(selection.contains(0, 0));
        assert!(!selection.contains(0, 1));
        assert!(selection.contains(0, 3));
        let before = a.doc.clone();
        a.place_timeline_cels(&selection, (0, 1), false);
        assert_eq!(a.doc.layers[0].cels[0][0], CLEAR);
        assert_eq!(a.doc.layers[0].cels[1][0], colors[0]);
        assert_eq!(a.doc.layers[0].cels[2][0], colors[1]);
        assert_eq!(a.doc.layers[0].cels[3][0], CLEAR);
        assert_eq!(a.doc.layers[0].cels[4][0], colors[2]);
        let moved = a.timeline_cel_selection();
        assert!(moved.contains(0, 1));
        assert!(!moved.contains(0, 2));
        assert!(moved.contains(0, 4));
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.timeline_cel_selection(), selection);
    }

    #[test]
    fn timeline_rectangle_matches_mcp_and_undo_restores_selection() {
        let mut a = app();
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        a.doc.add_layer();
        a.doc.set_pixel(0, 0, 0, [80, 120, 190, 255]);
        a.doc.set_pixel(1, 1, 1, [49, 220, 150, 255]);
        a.doc.tags.push(FrameTag {
            name: "Loop".into(),
            from: 0,
            to: 4,
            direction: 0,
            repeat: 0,
            color: [130, 150, 250],
        });

        a.timeline_select(0, 0, false);
        a.timeline_select(1, 1, true);
        let selection = a.timeline_range();
        let before = a.doc.clone();
        let duplicate = serde_json::json!({
            "op": "duplicate-cel-range", "range": selection,
            "to_layer": 0, "to_frame": 2
        });
        let moved = serde_json::json!({
            "op": "move-cel-range", "range": selection,
            "to_layer": 0, "to_frame": 1
        });
        let expected = pixelcraft::automation::apply_command_batch(
            &before,
            &serde_json::json!({"commands": [duplicate, moved]}),
        )
        .unwrap();

        assert!(a.edit_timeline("Duplicate cels", |doc| {
            doc.move_cel_range(selection, 0, 2, true)
        }));
        assert!(a.edit_timeline("Move cels", |doc| {
            doc.move_cel_range(selection, 0, 1, false)
        }));
        assert_eq!(a.doc, expected);
        assert_eq!(a.doc.tags, before.tags);
        a.undo();
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.timeline_range(), selection);
    }

    fn wait_for_export(editor: &mut PixelCraft) {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(5);
        while editor.export_job.is_some() && Instant::now() < deadline {
            editor.poll_export();
            if editor.export_job.is_some() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(editor.export_job.is_none());
    }

    #[test]
    fn repeat_last_export_uses_the_last_successful_settings_for_each_tab() {
        let ctx = egui::Context::default();
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.replace(Document::new(2, 1), None);
        editor.doc.add_frame(0, false);
        editor.doc.set_pixel(0, 0, 0, [120, 20, 40, 255]);
        editor.doc.set_pixel(0, 1, 0, [10, 40, 210, 255]);
        editor.frame = 1;
        editor.studio.sheet.scale = 2;
        let directory =
            std::env::temp_dir().join(format!("pixelcraft-repeat-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let png = directory.join("first.png");

        editor.start_export("png", png.clone());
        wait_for_export(&mut editor);
        assert!(editor.status.starts_with("Exported "), "{}", editor.status);
        editor.start_export("png", directory.join("missing/fail.png"));
        wait_for_export(&mut editor);
        assert!(
            editor.status.starts_with("Export failed"),
            "{}",
            editor.status
        );

        let edited = [110, 230, 60, 255];
        editor.doc.set_pixel(0, 1, 0, edited);
        editor.doc.move_frames(&[1], 0).unwrap();
        editor.frame = 1;
        editor.studio.sheet.scale = 1;
        editor.open_document(Document::new(1, 1), None);
        editor.action("repeat-export");
        assert!(editor.export_job.is_none(), "a new tab inherited an export");
        editor.switch_tab(0);
        editor.action("repeat-export");
        assert!(
            editor.export_job.is_some(),
            "repeat action did not start the worker"
        );
        wait_for_export(&mut editor);
        let rendered = image::open(&png).unwrap().into_rgba8();
        assert_eq!(rendered.dimensions(), (4, 2));
        assert_eq!(rendered.get_pixel(0, 0).0, edited);

        let second = directory.join("sequence");
        editor.start_export("png-sequence", second.clone());
        wait_for_export(&mut editor);
        assert!(editor.status.starts_with("Exported "), "{}", editor.status);
        editor.doc.set_pixel(0, 0, 0, edited);
        editor.action("repeat-export");
        wait_for_export(&mut editor);
        assert!(editor.status.starts_with("Exported "), "{}", editor.status);
        let repeated = image::open(second.join("frame-0001.png"))
            .unwrap()
            .into_rgba8();
        assert_eq!(repeated.get_pixel(0, 0).0, edited);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn repeat_export_routes_completed_sheet_to_its_original_tab() {
        use pixelcraft::workflows::SheetLayout;

        let ctx = egui::Context::default();
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.replace(Document::new(2, 1), None);
        editor.doc.add_frame(0, false);
        editor.doc.set_pixel(0, 0, 1, [220, 70, 12, 255]);
        let options = pixelcraft::workflows::SheetOptions {
            scale: 2,
            padding: 0,
            layout: SheetLayout::Vertical,
            width: Some(3),
            filename_template: "fixed_{frame_id}".into(),
            ..Default::default()
        };
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-repeat-sheet-{}.png",
            std::process::id()
        ));
        let recipe = ExportRecipe {
            path: path.clone(),
            kind: ExportKind::Sheet {
                options: Box::new(options),
                slice: None,
                layers: None,
                tag: None,
            },
        };
        editor.start_export_recipe(recipe, false);
        editor.open_document(Document::new(1, 1), None);
        wait_for_export(&mut editor);
        assert!(!editor.can_repeat_export());
        editor.switch_tab(0);
        assert!(editor.can_repeat_export());
        let changed = [120, 210, 10, 255];
        editor.doc.set_pixel(0, 0, 1, changed);
        editor.studio.sheet.scale = 4;
        editor.action("repeat-export");
        wait_for_export(&mut editor);
        assert!(editor.status.starts_with("Exported "), "{}", editor.status);
        let exported = image::open(&path).unwrap().into_rgba8();
        assert_eq!(exported.dimensions(), (6, 4));
        assert_eq!(exported.get_pixel(2, 0).0, changed);
        let metadata = path.with_extension("json");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
        assert_eq!(json["frames"][0]["filename"], "fixed_0");
        std::fs::remove_file(metadata).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn editing_tag_repeat_updates_gif_and_godot_batch_exports() {
        use image::AnimationDecoder;
        let mut a = app();
        a.doc = Document::new(2, 2);
        a.doc.add_frame(0, false);
        a.doc.tags.push(FrameTag {
            name: "Walk".into(),
            from: 0,
            to: 1,
            direction: 0,
            repeat: 0,
            color: [63, 123, 181],
        });
        a.set_tag_repeat(0, 3);
        let root =
            std::env::temp_dir().join(format!("pixelcraft-tag-repeat-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let options = pixelcraft::workflows::SheetOptions {
            preset: "Godot".into(),
            ..Default::default()
        };
        let gifs = pixelcraft::productivity::batch_tags(&root, &a.doc, true, &options).unwrap();
        let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            std::fs::File::open(&gifs[0]).unwrap(),
        ))
        .unwrap();
        assert!(
            matches!(decoder.loop_count(), image::metadata::LoopCount::Finite(n) if n.get() == 3)
        );
        let sheets = pixelcraft::productivity::batch_tags(&root, &a.doc, false, &options).unwrap();
        let resource = std::fs::read_to_string(sheets[0].with_extension("tres")).unwrap();
        assert!(resource.contains(r#""loop": false"#));
        assert_eq!(resource.matches(r#""texture": SubResource"#).count(), 6);
        a.undo();
        assert_eq!(a.doc.tags[0].repeat, 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn timeline_onion_handles_drag_both_ranges_without_editing_the_document() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 1), None);
        for frame in 0..6 {
            a.doc.add_frame(frame, false);
        }
        a.frame = 3;
        a.doc.set_pixel(0, 1, 0, [250, 10, 20, 255]);
        a.doc.set_pixel(0, 5, 1, [10, 200, 20, 255]);
        let before = a.doc.clone();
        a.onion = true;
        a.set_onion_range(true, 1);
        a.set_onion_range(false, 0);
        let draw = |a: &mut PixelCraft, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(950., 650.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| a.timeline(ui));
                },
            );
            output.textures_delta.clear();
        };
        draw(&mut a, vec![]);
        let handle = |previous| {
            ctx.data_mut(|data| {
                data.get_temp::<Rect>(egui::Id::new(("onion-handle-test", previous)))
            })
            .unwrap()
            .center()
        };
        let width = ctx
            .data_mut(|data| data.get_temp::<Rect>(egui::Id::new(("frame-header", 2usize))))
            .unwrap()
            .width();
        let drag = |a: &mut PixelCraft, from, to| {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            draw(a, vec![egui::Event::PointerMoved(from), button(from, true)]);
            draw(a, vec![egui::Event::PointerMoved(to)]);
            draw(a, vec![button(to, false)]);
        };
        drag(&mut a, handle(true), handle(true) - Vec2::new(width, 0.));
        drag(
            &mut a,
            handle(false),
            handle(false) + Vec2::new(2. * width, 0.),
        );
        let options = a.power.preferences_onion();
        assert_eq!((options.previous, options.next), (2, 2));
        let pixels = pixelcraft::productivity::onion_pixels(&a.doc, a.frame, &options);
        assert!(pixels[0][3] > 0 && pixels[1][3] > 0);
        draw(
            &mut a,
            vec![egui::Event::Key {
                key: Key::ArrowLeft,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(a.power.preferences_onion().next, 1);
        assert_eq!(a.frame, 3);
        assert_eq!(a.doc, before);
        assert!(a.undo.is_empty());
    }

    #[test]
    fn subtag_playback_clock_prefetches_nested_order_and_loops_only_when_requested() {
        let mut a = app();
        if a.rewind_on_stop() {
            a.action("rewind-on-stop");
        }
        a.replace(Document::new(1, 1), None);
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        for (from, to, direction, repeat) in [(1, 4, 0, 2), (1, 3, 1, 2)] {
            a.doc.tags.push(FrameTag {
                name: "Animation".into(),
                from,
                to,
                direction,
                repeat,
                color: [0; 3],
            });
        }
        a.action("play-subtags-once");
        assert!(a.playing);
        assert_eq!(a.frame, 0);
        let ctx = egui::Context::default();
        for frame in 0..5 {
            let _ = a.frame_texture(&ctx, frame);
        }
        let start = a.last_tick;
        let expected = [3, 2, 1, 3, 2, 1, 4, 3, 2, 1, 3, 2, 1, 4];
        for (tick, frame) in expected.into_iter().enumerate() {
            assert_eq!(a.next_playback_frame().map(|step| step.0), Some(frame));
            a.update_playback_clock(start + Duration::from_millis(100 * (tick as u64 + 1)));
            assert_eq!(a.frame, frame);
            assert!(a.playing);
        }
        a.update_playback_clock(start + Duration::from_millis(1500));
        assert!(!a.playing);
        assert_eq!(a.frame, 4);
        a.action("play-subtags");
        for _ in 0..15 {
            a.advance_playback();
        }
        assert!(a.playing);
        assert_eq!(a.frame, 0);
    }

    #[test]
    fn play_selected_frames_once_honors_gaps_and_the_final_frame_duration() {
        let mut a = app();
        if a.rewind_on_stop() {
            a.action("rewind-on-stop");
        }
        a.replace(Document::new(1, 1), None);
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        a.frame = 2;
        a.timeline_frames.selected = vec![1, 4];
        a.action("play-selection-once");
        assert!(a.playing);
        assert_eq!(a.frame, 1);
        // Advance via the production playback clock with the next frame cached.
        let ctx = egui::Context::default();
        let _ = a.frame_texture(&ctx, 4);
        let start = a.last_tick;
        a.doc.durations[4] = 200;
        a.update_playback_clock(start + Duration::from_millis(100));
        assert_eq!(a.frame, 4);
        assert!(a.playing);
        a.update_playback_clock(start + Duration::from_millis(299));
        assert!(a.playing);
        a.update_playback_clock(start + Duration::from_millis(300));
        assert!(!a.playing);
        assert_eq!(a.frame, 4);

        a.doc.durations[4] = 16;
        a.timeline_frames.selected = vec![4];
        a.action("play-selection-once");
        let start = a.last_tick;
        assert_eq!(
            a.update_playback_clock(start + Duration::from_millis(14)),
            Duration::from_millis(2)
        );
        assert!(a.playing);
        a.update_playback_clock(start + Duration::from_millis(16));
        assert!(!a.playing);
    }

    #[test]
    fn play_once_traverses_reverse_ping_pong_and_rewind_restores_entry_frame() {
        let mut a = app();
        if a.rewind_on_stop() {
            a.action("rewind-on-stop");
        }
        a.replace(Document::new(1, 1), None);
        for frame in 0..4 {
            a.doc.add_frame(frame, false);
        }
        a.doc.tags.push(FrameTag {
            name: "Reverse walk".into(),
            from: 1,
            to: 3,
            direction: 3,
            repeat: 0,
            color: [0, 0, 0],
        });
        a.active_tag = Some(0);
        a.frame = 4;
        a.action("play-once");
        assert_eq!(a.frame, 3);
        for frame in [2, 1, 2, 3] {
            a.advance_playback();
            assert_eq!(a.frame, frame);
            assert!(a.playing);
        }
        a.advance_playback();
        assert!(!a.playing);
        assert_eq!(a.frame, 3);

        a.action("rewind-on-stop");
        a.frame = 2;
        a.action("play-once");
        a.advance_playback();
        a.advance_playback();
        assert_eq!(a.frame, 1);
        a.action("play");
        assert!(!a.playing);
        assert_eq!(a.frame, 2);
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
    fn detached_large_preview_does_not_evict_the_canvas_texture() {
        let ctx = egui::Context::default();
        let mut app = app();
        app.replace(Document::new(4096, 4096), None);
        app.doc.add_frame(0, false);
        app.refresh_frame_cache(&ctx);
        let small_image = || egui::ColorImage::filled([1, 1], Color32::WHITE);
        app.store_frame_texture(&ctx, 0, small_image(), None);
        let canvas = app.frame_texture(&ctx, 0);

        let (send, result) = std::sync::mpsc::channel();
        app.preview_prefetch = Some(FramePrefetch {
            frame: 1,
            snapshot: app.doc.clone(),
            result,
        });
        app.update_preview_texture(&ctx, 1);
        assert_eq!(app.preview.as_ref().unwrap().id(), canvas.id());
        send.send((small_image(), None)).unwrap();
        app.update_preview_texture(&ctx, 1);
        let detached = app.preview.as_ref().unwrap().id();
        assert_ne!(detached, canvas.id());
        assert_eq!(app.frame_texture(&ctx, 0).id(), canvas.id());
        app.update_preview_texture(&ctx, 1);
        assert_eq!(app.preview.as_ref().unwrap().id(), detached);
        assert_eq!(app.frame_texture_order.len(), 1);
    }
    #[test]
    fn always_create_tile_brush_uses_one_variant_per_placement_in_a_stroke() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 1), None);
        a.zoom = 32.;
        let old = [10, 20, 30, 255];
        let paint = [40, 50, 60, 255];
        a.doc.tilesets.push(Tileset {
            name: "Variants".into(),
            width: 2,
            height: 1,
            tiles: vec![vec![old, old].into(), vec![paint, paint].into()],
        });
        a.doc.add_tile_layer(0).unwrap();
        a.layer = 1;
        a.doc.layers[1].tile_cels[0] = vec![1, 1];
        a.tile_pixel_mode = Some(TilePixelEdit::CreateVariant);
        a.fg = paint;
        let draw = |a: &mut PixelCraft, events| {
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
        let from = Pos2::new(252., 300.);
        let to = Pos2::new(348., 300.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw(
            &mut a,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        draw(&mut a, vec![egui::Event::PointerMoved(to)]);
        draw(&mut a, vec![egui::Event::PointerMoved(from)]);
        draw(&mut a, vec![button(from, false)]);
        assert_eq!(a.doc.layers[1].tile_cels[0], [3, 4]);
        assert_eq!(a.doc.tilesets[0].tiles.len(), 4);
        assert_eq!(a.doc.tilesets[0].tiles[2], a.doc.tilesets[0].tiles[1]);
        assert_eq!(a.doc.tilesets[0].tiles[3], a.doc.tilesets[0].tiles[1]);
        assert_eq!(a.undo.len(), 1);
        a.undo();
        assert_eq!(a.doc.layers[1].tile_cels[0], [1, 1]);
        assert_eq!(a.doc.tilesets[0].tiles.len(), 2);
    }

    #[test]
    fn tileset_editor_reorders_duplicates_and_deletes_with_brush_history() {
        let mut editor = app();
        editor.doc = Document::new(2, 1);
        editor.doc.tilesets.push(Tileset {
            name: "Stone".into(),
            width: 1,
            height: 1,
            tiles: vec![
                vec![[20, 60, 110, 255]].into(),
                vec![[80, 90, 100, 255]].into(),
            ],
        });
        editor.doc.add_tile_layer(0).unwrap();
        editor.layer = 1;
        editor.doc.layers[1].tile_cels[0] = vec![0x8000_0001, 2];
        editor.selected_tile = 0x8000_0001;
        editor.apply_tile_definition_edit(0, TileDefinitionEdit::MoveTo(2));
        assert_eq!(editor.selected_tile, 0x8000_0002);
        assert_eq!(editor.doc.layers[1].tile_cels[0], [0x8000_0002, 1]);
        editor.apply_tile_definition_edit(0, TileDefinitionEdit::Duplicate);
        assert_eq!(editor.selected_tile, 0x8000_0003);
        editor.apply_tile_definition_edit(0, TileDefinitionEdit::Delete);
        assert_eq!(editor.selected_tile, 2);
        assert_eq!(editor.undo.len(), 3);
        editor.undo();
        assert_eq!(editor.selected_tile, 0x8000_0003);
        assert_eq!(editor.doc.tilesets[0].tiles.len(), 3);
        editor.undo();
        editor.undo();
        assert_eq!(editor.selected_tile, 0x8000_0001);
        assert_eq!(editor.doc.layers[1].tile_cels[0], [0x8000_0001, 2]);
        editor.redo();
        assert_eq!(editor.selected_tile, 0x8000_0002);
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
        if self.download_job.is_none() && self.downloaded_update.is_none() {
            self.background_update_attempt = None;
        }
        if let Some(slug) = updates::repository_slug(&self.about_info.repository) {
            self.next_update_check = Some(Instant::now() + Duration::from_secs(3600));
            if self.download_job.is_none() && self.downloaded_update.is_none() {
                self.update_status = "Checking GitHub releases…".into();
            }
            self.update_job = Some(updates::check(&slug, ctx.clone()));
        } else {
            self.latest_release = None;
            self.next_update_check = None;
            self.update_job = None;
            self.update_status =
                "This build needs PixelCraft’s GitHub repository to check for updates.".into();
        }
    }

    fn update_to_download(&self, background: bool) -> Option<&Release> {
        if self.update_job.is_some()
            || self.download_job.is_some()
            || self.downloaded_update.is_some()
        {
            return None;
        }
        self.latest_release.as_ref().filter(|release| {
            release.is_newer_than(INSTALLED_VERSION)
                && release.download_available()
                && (!background
                    || (self.power.background_updates()
                        && self.background_update_attempt.as_deref() != Some(&release.tag)))
        })
    }

    fn start_update_download(&mut self, ctx: &egui::Context, background: bool) {
        let Some(release) = self.update_to_download(background).cloned() else {
            return;
        };
        self.background_update_attempt = Some(release.tag.clone());
        self.update_status = "Downloading update and verifying SHA-256…".into();
        self.download_job = Some((
            release.tag.clone(),
            updates::download(
                release,
                self.about_info.repository.clone(),
                extensions::config_directory().join("updates"),
                ctx.clone(),
            ),
        ));
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
                if self.update_to_download(false).is_some()
                    && ui.pc_button("Download update in PixelCraft").clicked(){
                        self.start_update_download(ctx, false);
                    }
                if let Some((_, download))=&self.downloaded_update {
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

    #[test]
    fn a_superseded_download_cannot_hide_the_new_release_status() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("test.png")));
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let (download_sender, download_receiver) = std::sync::mpsc::channel();
        app.latest_release = Some(Release::parse(r#"{"tag_name":"v99.0.0"}"#).unwrap());
        app.update_job = Some(release_receiver);
        app.download_job = Some(("v99.0.0".into(), download_receiver));
        release_sender
            .send(Ok(Release::parse(r#"{"tag_name":"v100.0.0"}"#).unwrap()))
            .unwrap();
        download_sender
            .send(Err("Old download failed".into()))
            .unwrap();

        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.render(ui));
        output.textures_delta.clear();

        assert_eq!(app.update_status, "A new version is available.");
        assert_eq!(app.latest_release.unwrap().tag, "v100.0.0");
        assert!(app.download_job.is_none());
    }

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
    fn slow_playback_advances_a_cel_whose_deadline_is_longer_than_a_second() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(1, 1), None);
        app.doc.add_frame(0, false);
        app.doc.durations = vec![300, 400];
        app.playback_speed = 25;
        let start = Instant::now();
        app.last_tick = start;
        assert_eq!(
            app.update_playback_clock(start + Duration::from_millis(1200)),
            Duration::from_millis(1600)
        );
        assert_eq!(app.frame, 1);
        assert_eq!(app.doc.durations, vec![300, 400]);
    }

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
                options: Default::default(),
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
