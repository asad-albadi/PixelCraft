use super::*;
use pixelcraft::{
    atlas::*,
    automation::{self, LiveBridge},
    productivity::*,
    workflows::SheetOptions,
};
use serde::{Deserialize, Serialize};
mod brush_library;
use brush_library::BrushLibrary;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum ImageBrushAlignment {
    #[default]
    Stamp,
    Source,
    Stroke,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum EyedropperSource {
    #[default]
    Visible,
    Active,
    References,
}

impl EyedropperSource {
    fn label(self) -> &'static str {
        match self {
            Self::Visible => "Visible layers",
            Self::Active => "Active layer",
            Self::References => "Reference images",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum EyedropperChannel {
    #[default]
    ColorAlpha,
    Color,
    Alpha,
    Red,
    Green,
    Blue,
    Gray,
    GrayAlpha,
    PaletteIndex,
}

impl EyedropperChannel {
    fn label(self) -> &'static str {
        match self {
            Self::ColorAlpha => "Color and alpha",
            Self::Color => "Color only",
            Self::Alpha => "Alpha only",
            Self::Red => "Red only",
            Self::Green => "Green only",
            Self::Blue => "Blue only",
            Self::Gray => "Grayscale",
            Self::GrayAlpha => "Grayscale and alpha",
            Self::PaletteIndex => "Palette index",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct EyedropperOptions {
    source: EyedropperSource,
    channel: EyedropperChannel,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
enum GradientShape {
    #[default]
    Linear,
    Radial,
}

impl GradientShape {
    fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Radial => "Radial",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum GradientPattern {
    #[default]
    Smooth,
    Bayer2,
    Bayer4,
    Bayer8,
    Installed,
}

impl GradientPattern {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Bayer2 => "Bayer 2×2",
            Self::Bayer4 => "Bayer 4×4",
            Self::Bayer8 => "Bayer 8×8",
            Self::Installed => "Installed matrix",
        }
    }

    pub(super) fn matrix(
        self,
        doc: &Document,
    ) -> Option<pixelcraft::contributions::DitheringMatrix> {
        use pixelcraft::contributions::DitheringMatrix;
        match self {
            Self::Smooth => None,
            Self::Installed => doc
                .dithering
                .as_ref()
                .filter(|matrix| matrix.validate())
                .cloned(),
            Self::Bayer2 => DitheringMatrix::bayer(2),
            Self::Bayer4 => DitheringMatrix::bayer(4),
            Self::Bayer8 => DitheringMatrix::bayer(8),
        }
    }

    fn thresholds(self, doc: &Document) -> Option<(usize, usize, Vec<f32>)> {
        self.matrix(doc)
            .map(|matrix| (matrix.width, matrix.height, matrix.thresholds()))
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct GradientOptions {
    shape: GradientShape,
    pattern: GradientPattern,
}

#[derive(Default)]
struct ReferenceAvailability {
    root: PathBuf,
    files: std::collections::HashSet<PathBuf>,
}

fn linked_reference_exists(root: &Path, source: &Path) -> bool {
    if !valid_reference_source(source) {
        return false;
    }
    let mut path = root.to_path_buf();
    let mut components = source.components().peekable();
    while let Some(part) = components.next() {
        path.push(part);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            return false;
        };
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return false;
            }
        }
        if components.peek().is_none() {
            return metadata.is_file();
        }
        if !metadata.is_dir() {
            return false;
        }
    }
    false
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    recent: Vec<PathBuf>,
    onion: OnionOptions,
    rewind_on_stop: bool,
    preset: String,
    interface_scale: f32,
    comfortable: bool,
    checker_size: usize,
    checker_colors: Option<[[u8; 3]; 2]>,
    autosave: bool,
    new_sprite_width: usize,
    new_sprite_height: usize,
    default_grid: bool,
    background_updates: bool,
    sample_references: bool,
    eyedropper: EyedropperOptions,
    gradient: GradientOptions,
    tool_inks: std::collections::BTreeMap<String, PaintInk>,
    brush_dynamics: BrushDynamics,
    import_profile: pixelcraft::color::ImportProfile,
    pub(super) display_profile: crate::display_profiles::DisplayChoice,
    whole_pixel_wheel_zoom: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            recent: vec![],
            onion: OnionOptions::default(),
            rewind_on_stop: false,
            preset: String::new(),
            interface_scale: 1.,
            comfortable: false,
            checker_size: 8,
            checker_colors: None,
            autosave: true,
            new_sprite_width: 32,
            new_sprite_height: 32,
            default_grid: false,
            background_updates: false,
            sample_references: false,
            eyedropper: EyedropperOptions::default(),
            gradient: GradientOptions::default(),
            tool_inks: Default::default(),
            brush_dynamics: Default::default(),
            import_profile: pixelcraft::color::ImportProfile::default(),
            display_profile: crate::display_profiles::DisplayChoice::default(),
            whole_pixel_wheel_zoom: false,
        }
    }
}
impl Preferences {
    fn new_sprite_size(&self) -> (usize, usize) {
        let width = self.new_sprite_width.clamp(1, MAX_DIMENSION);
        let height = self
            .new_sprite_height
            .clamp(1, MAX_DIMENSION.min(MAX_PIXELS / width));
        (width, height)
    }

    fn reset_settings(&mut self) {
        let recent = std::mem::take(&mut self.recent);
        let preset = std::mem::take(&mut self.preset);
        *self = Self {
            recent,
            preset,
            ..Self::default()
        };
    }
}

fn recent_file_label(path: &std::path::Path, paths: &[PathBuf]) -> String {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let duplicated = paths
        .iter()
        .any(|other| other != path && other.file_name() == path.file_name());
    let label = if duplicated {
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let colliding_parent = paths.iter().any(|other| {
            other != path
                && other.file_name() == path.file_name()
                && other.parent().and_then(std::path::Path::file_name) == parent.file_name()
        });
        let folder = if colliding_parent {
            parent.display().to_string()
        } else {
            parent
                .file_name()
                .unwrap_or(parent.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        format!("{name} ({folder})")
    } else {
        name.into_owned()
    };
    if path.is_file() {
        label
    } else {
        format!("{label} · Missing")
    }
}

pub(super) struct PowerTools {
    pub open: bool,
    pub mode: BrushMode,
    pub reverse: bool,
    pub stamp: Option<(usize, usize, Vec<Pixel>)>,
    image_brush_alignment: ImageBrushAlignment,
    image_brush_origin: (i32, i32),
    pub contiguous: bool,
    pub tolerance: u8,
    pub(super) fill_tolerance: u8,
    pub(super) fill_global: bool,
    pub selection_op: SelectionOp,
    preferences: Preferences,
    brush_library: BrushLibrary,
    grid: GridImportOptions,
    grid_preview: Option<CachedGridPreview>,
    grid_source: Option<GridSource>,
    grid_texture: Option<egui::TextureHandle>,
    atlas: AtlasOptions,
    atlas_files: Vec<PathBuf>,
    atlas_path: String,
    atlas_preview: Option<Result<AtlasPreview, String>>,
    pub reference_edit: Option<usize>,
    pub(super) reference_textures: Vec<(Pixels, egui::TextureHandle)>,
    reference_drag: Option<(f32, f32, f32, Pos2)>,
    reference_availability: ReferenceAvailability,
    reference_check: Option<std::sync::mpsc::Receiver<ReferenceAvailability>>,
    reference_check_due: Instant,
    pub bridge: Option<LiveBridge>,
    bridge_snapshot: Option<Document>,
    pub bridge_generation: u64,
    bridge_tab: usize,
    bridge_session: String,
    bridge_work: Option<std::sync::mpsc::Receiver<PreparedLive>>,
    pub bridge_export: Option<automation::LiveExportProgress>,
    pub shade_seen: std::collections::HashSet<usize>,
    pub stroke_opacity: std::collections::HashMap<usize, u8>,
    ramp_from: usize,
    ramp_to: usize,
    ramp_steps: usize,
    ramp_interpolation: pixelcraft::workflows::RampInterpolation,
    palette_sort: PaletteSort,
    palette_picks: Option<(EditorId, Vec<usize>)>,
}
impl Default for PowerTools {
    fn default() -> Self {
        let preferences = std::fs::read(extensions::config_directory().join("workflow.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            open: false,
            mode: BrushMode::Normal,
            reverse: false,
            stamp: None,
            image_brush_alignment: ImageBrushAlignment::Stamp,
            image_brush_origin: (0, 0),
            contiguous: true,
            tolerance: 0,
            fill_tolerance: 0,
            fill_global: false,
            selection_op: SelectionOp::Replace,
            preferences,
            brush_library: BrushLibrary::read(&brush_library::path()).unwrap_or_default(),
            grid: GridImportOptions::default(),
            grid_preview: None,
            grid_source: None,
            grid_texture: None,
            atlas: AtlasOptions::default(),
            atlas_files: vec![],
            atlas_path: String::new(),
            atlas_preview: None,
            reference_edit: None,
            reference_textures: vec![],
            reference_drag: None,
            reference_availability: ReferenceAvailability::default(),
            reference_check: None,
            reference_check_due: Instant::now(),
            bridge: None,
            bridge_snapshot: None,
            bridge_generation: 0,
            bridge_tab: 0,
            bridge_session: format!("{}-{:?}", std::process::id(), std::time::SystemTime::now()),
            bridge_work: None,
            bridge_export: None,
            shade_seen: Default::default(),
            stroke_opacity: Default::default(),
            ramp_from: 0,
            ramp_to: 1,
            ramp_steps: 5,
            ramp_interpolation: pixelcraft::workflows::RampInterpolation::Rgb,
            palette_sort: PaletteSort::Hue,
            palette_picks: None,
        }
    }
}

struct CachedGridPreview {
    options: GridImportOptions,
    result: Result<GridPreview, String>,
}

struct AtlasPreview {
    inputs: Vec<PathBuf>,
    options: AtlasOptions,
    packed: (image::RgbaImage, serde_json::Value),
    texture: egui::TextureHandle,
}

fn import_preview_texture(
    ctx: &egui::Context,
    label: &str,
    image: &image::RgbaImage,
) -> egui::TextureHandle {
    // A preview never needs full-resolution GPU storage for a large sheet.
    let thumb;
    let image = if image.width().max(image.height()) > 1024 {
        let scale = 1024. / image.width().max(image.height()) as f32;
        thumb = image::imageops::resize(
            image,
            (image.width() as f32 * scale).round().max(1.) as u32,
            (image.height() as f32 * scale).round().max(1.) as u32,
            image::imageops::FilterType::Nearest,
        );
        &thumb
    } else {
        image
    };
    ctx.load_texture(
        label,
        egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ),
        egui::TextureOptions::NEAREST,
    )
}

enum LiveResult {
    Read(serde_json::Value),
    Export(automation::PreparedExport),
    Save(automation::PreparedExport, Box<Document>),
    Edit(Box<Document>, serde_json::Value),
}

struct PreparedLive {
    request: automation::LiveRequest,
    revision: String,
    result: Result<LiveResult, String>,
}
impl PowerTools {
    pub(super) fn shortcut_preset_name(&self) -> &str {
        if self.preferences.preset.is_empty() {
            "PixelCraft"
        } else {
            &self.preferences.preset
        }
    }
    pub(super) fn set_shortcut_preset(&mut self, name: &str) {
        self.preferences.preset = name.into();
    }

    pub(super) fn comfortable(&self) -> bool {
        self.preferences.comfortable
    }
    pub(super) fn apply_density(&self, ctx: &egui::Context) {
        let (height, padding, spacing) = if self.comfortable() {
            (32., Vec2::new(8., 5.), Vec2::new(6., 5.))
        } else {
            (28., Vec2::new(6., 3.), Vec2::new(4., 3.))
        };
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let style = ctx.style_of(theme);
            if style.spacing.interact_size.y != height
                || style.spacing.button_padding != padding
                || style.spacing.item_spacing != spacing
            {
                let mut style = (*style).clone();
                style.spacing.interact_size.y = height;
                style.spacing.button_padding = padding;
                style.spacing.item_spacing = spacing;
                ctx.set_style_of(theme, style);
            }
        }
    }
    pub(super) fn interface_scale(&self) -> f32 {
        self.preferences.interface_scale.clamp(0.5, 3.)
    }
    pub(super) fn autosave_enabled(&self) -> bool {
        self.preferences.autosave
    }
    pub(super) fn checker_size(&self) -> f32 {
        self.preferences.checker_size.clamp(1, 64) as f32
    }
    pub(super) fn checker_color(&self, index: usize) -> Option<Color32> {
        self.preferences
            .checker_colors
            .map(|c| Color32::from_rgb(c[index][0], c[index][1], c[index][2]))
    }

    pub(super) fn new_sprite_size(&self) -> (usize, usize) {
        self.preferences.new_sprite_size()
    }
    pub(super) fn default_grid(&self) -> bool {
        self.preferences.default_grid
    }
    pub(super) fn background_updates(&self) -> bool {
        self.preferences.background_updates
    }
    pub(super) fn whole_pixel_wheel_zoom(&self) -> bool {
        self.preferences.whole_pixel_wheel_zoom
    }

    pub(super) fn preferences_onion(&self) -> OnionOptions {
        self.preferences.onion.clone()
    }
}
impl PixelCraft {
    pub(super) fn display_choice(&self) -> &crate::display_profiles::DisplayChoice {
        &self.power.preferences.display_profile
    }
    pub(super) fn set_onion_range(&mut self, previous: bool, count: usize) -> bool {
        let onion = &mut self.power.preferences.onion;
        let setting = if previous {
            &mut onion.previous
        } else {
            &mut onion.next
        };
        let count = count.min(8);
        std::mem::replace(setting, count) != count
    }

    pub(super) fn timeline_onion_range(
        &mut self,
        ui: &mut egui::Ui,
        track: Rect,
        first_header: Rect,
    ) {
        let options = self.power.preferences_onion();
        let frame = self.frame;
        let prev = options.previous.min(8).min(frame);
        let next = options
            .next
            .min(8)
            .min(self.doc.durations.len() - 1 - frame);
        let boundary = |index: usize| first_header.left() + index as f32 * first_header.width();
        let y = track.center().y;
        let painter = ui.painter();
        painter.text(
            Pos2::new(track.left() + 8., y),
            egui::Align2::LEFT_CENTER,
            "Onion range",
            egui::FontId::proportional(11.),
            ui.visuals().text_color(),
        );
        painter.hline(
            boundary(frame)..=boundary(frame + 1),
            y,
            Stroke::new(2., ui.visuals().selection.stroke.color),
        );

        for (previous, count, origin, available, tint, label) in [
            (
                true,
                prev,
                frame,
                frame,
                options.previous_tint,
                "Previous onion frames",
            ),
            (
                false,
                next,
                frame + 1,
                self.doc.durations.len() - 1 - frame,
                options.next_tint,
                "Next onion frames",
            ),
        ] {
            let direction = if previous { -1 } else { 1 };
            let end = origin.saturating_add_signed(direction * count as isize);
            let x = boundary(end);
            let handle = Rect::from_center_size(Pos2::new(x, y), Vec2::new(12., 18.));
            let color = Color32::from_rgb(tint[0], tint[1], tint[2]);
            painter.hline(
                x.min(boundary(origin))..=x.max(boundary(origin)),
                y,
                Stroke::new(3., color),
            );

            let id = ui.id().with(("onion-range", self.active_tab, previous));
            let response = ui.interact(handle, id, egui::Sense::click_and_drag());
            let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), count as f64, label));
            let change_id = id.with("changed");
            let dragging = response.dragged_by(egui::PointerButton::Primary);
            let released = response.drag_stopped_by(egui::PointerButton::Primary);
            if dragging || response.clicked() {
                response.request_focus();
            }
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                );
            });

            if (dragging || released)
                && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
            {
                let delta =
                    ((pointer.x - boundary(origin)) / first_header.width()).round() as isize;
                let count = (delta * direction).clamp(0, available.min(8) as isize) as usize;
                if self.set_onion_range(previous, count) {
                    ui.data_mut(|data| data.insert_temp(change_id, true));
                    ui.ctx().request_repaint();
                }
            }
            let mut save = released
                && ui.data_mut(|data| data.remove_temp::<bool>(change_id).unwrap_or(false));
            if response.has_focus() {
                let key_delta = ui.input_mut(|input| {
                    isize::from(input.consume_key(egui::Modifiers::NONE, Key::ArrowRight))
                        - isize::from(input.consume_key(egui::Modifiers::NONE, Key::ArrowLeft))
                });
                if key_delta != 0 {
                    let count = count
                        .saturating_add_signed(key_delta * direction)
                        .min(available.min(8));
                    save |= self.set_onion_range(previous, count);
                }
            }
            if save && self.shot.is_none() && !cfg!(test) {
                self.save_workflow_preferences();
            }
            ui.painter()
                .rect_filled(handle.shrink2(Vec2::new(3., 2.)), 2., color);
            response.on_hover_text(format!("{label}: {count}"));
            #[cfg(test)]
            ui.data_mut(|data| {
                data.insert_temp(egui::Id::new(("onion-handle-test", previous)), handle)
            });
        }
    }

    pub(super) fn rewind_on_stop(&self) -> bool {
        self.power.preferences.rewind_on_stop
    }
    pub(super) fn toggle_rewind_on_stop(&mut self) {
        self.power.preferences.rewind_on_stop = !self.rewind_on_stop();
        if self.shot.is_none() && !cfg!(test) {
            self.save_workflow_preferences();
        }
    }
    pub(super) fn export_scale_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Export scale");
            for scale in [1, 2, 4] {
                ui.pc_selectable_value(&mut self.studio.sheet.scale, scale, format!("{scale}×"));
            }
            ui.add(
                egui::DragValue::new(&mut self.studio.sheet.scale)
                    .range(1..=16384)
                    .prefix("Custom "),
            );
        });
        ui.small("Nearest-neighbor scaling for PNG, BMP, sprite sheets, GIF and MP4.");
    }
    pub(super) fn density_options(&mut self, ui: &mut egui::Ui) {
        let before = self.power.preferences.comfortable;
        ui.selectable_value(&mut self.power.preferences.comfortable, false, "Compact");
        ui.selectable_value(&mut self.power.preferences.comfortable, true, "Comfortable");
        if before != self.power.preferences.comfortable {
            self.save_workflow_preferences();
        }
    }
    pub(super) fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        self.settings_open = !fluent::modal_dialog(ctx, "Settings", |ui| {
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 130.).max(80.))
                .show(ui, |ui| self.settings_ui(ui));
            ui.pc_button("Done").clicked()
        });
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Color management", |ui| {
            let mut display = self.power.preferences.display_profile.clone();
            self.display_profile_ui(ui, &mut display);
            if display != self.power.preferences.display_profile {
                self.power.preferences.display_profile = display;
                self.save_workflow_preferences();
            }
            use pixelcraft::color::ImportProfile;
            let policy = &mut self.power.preferences.import_profile;
            let before = *policy;
            let label = |policy| match policy {
                ImportProfile::Preserve => "Preserve embedded profile",
                ImportProfile::ConvertToSrgb => "Convert to sRGB",
                ImportProfile::AssignSrgb => "Assign sRGB",
            };
            egui::ComboBox::from_label("Image import profile")
                .selected_text(label(*policy))
                .show_ui(ui, |ui| {
                    for value in [ImportProfile::Preserve, ImportProfile::ConvertToSrgb, ImportProfile::AssignSrgb] {
                        ui.pc_selectable_value(policy, value, label(value));
                    }
                });
            ui.small("Imported layers match the destination profile. Assign sRGB treats their source values as sRGB.");
            if before != *policy {
                self.save_workflow_preferences();
            }
        });
        ui.collapsing("New sprites", |ui| {
            let p = &mut self.power.preferences;
            let before = (p.new_sprite_size(), p.default_grid);
            ui.horizontal(|ui| {
                ui.label("Default size");
                let _width = ui.add(
                    egui::DragValue::new(&mut p.new_sprite_width)
                        .range(1..=MAX_DIMENSION)
                        .suffix(" px"),
                );
                #[cfg(test)]
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new("default-sprite-width"), _width.rect);
                });
                ui.label("×");
                let height_limit =
                    MAX_DIMENSION.min(MAX_PIXELS / p.new_sprite_width.clamp(1, MAX_DIMENSION));
                ui.add(
                    egui::DragValue::new(&mut p.new_sprite_height)
                        .range(1..=height_limit)
                        .suffix(" px"),
                );
            });
            ui.pc_checkbox(&mut p.default_grid, "Show pixel grid by default");
            if before != (p.new_sprite_size(), p.default_grid) {
                self.save_workflow_preferences();
            }
        });
        ui.collapsing("Updates", |ui| {
            if ui
                .pc_checkbox(
                    &mut self.power.preferences.background_updates,
                    "Download and verify updates in the background",
                )
                .changed()
            {
                self.background_update_attempt = None;
                self.save_workflow_preferences();
            }
            ui.small("Install verified updates from Help → About.");
        });
        ui.collapsing("Onion skin defaults", |ui| {
            let before = self.power.preferences.onion.clone();
            let o = &mut self.power.preferences.onion;
            ui.add(egui::Slider::new(&mut o.previous, 0..=8).text("Previous frames"));
            ui.add(egui::Slider::new(&mut o.next, 0..=8).text("Next frames"));
            ui.horizontal(|ui| {
                ui.label("Previous tint");
                ui.color_edit_button_srgb(&mut o.previous_tint);
            });
            ui.add(egui::Slider::new(&mut o.previous_opacity, 0..=255).text("Previous opacity"));
            ui.horizontal(|ui| {
                ui.label("Next tint");
                ui.color_edit_button_srgb(&mut o.next_tint);
            });
            ui.add(egui::Slider::new(&mut o.next_opacity, 0..=255).text("Next opacity"));
            if serde_json::to_value(before).ok() != serde_json::to_value(&*o).ok() {
                self.save_workflow_preferences();
            }
        });
        ui.collapsing("Export scaling", |ui| self.export_scale_ui(ui));
        ui.collapsing("Interface & autosave", |ui| {
            let before = serde_json::to_value(&self.power.preferences).ok();
            let p = &mut self.power.preferences;
            ui.horizontal(|ui| {
                ui.label("Density");
                ui.selectable_value(&mut p.comfortable, false, "Compact");
                ui.selectable_value(&mut p.comfortable, true, "Comfortable");
            });
            ui.label("100% interface zoom follows system display scaling.");
            ui.add(egui::Slider::new(&mut p.interface_scale, 0.5..=3.).text("Interface scale"));
            ui.add(
                egui::DragValue::new(&mut p.checker_size)
                    .range(1..=64)
                    .prefix("Checker size "),
            );
            let mut custom = p.checker_colors.is_some();
            ui.pc_checkbox(&mut custom, "Custom checker colors");
            if custom {
                let colors = p
                    .checker_colors
                    .get_or_insert([[210, 202, 202], [184, 176, 182]]);
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgb(&mut colors[0]);
                    ui.color_edit_button_srgb(&mut colors[1]);
                });
            } else {
                p.checker_colors = None;
            }
            ui.pc_checkbox(&mut p.whole_pixel_wheel_zoom, "Whole-pixel wheel zoom");
            ui.pc_checkbox(&mut p.autosave, "Enable recovery autosave");
            if before != serde_json::to_value(&*p).ok() {
                ui.ctx().set_zoom_factor(self.power.interface_scale());
                self.save_workflow_preferences();
            }
        });
        if ui.pc_button("Reset settings to defaults").clicked() {
            self.power.preferences.reset_settings();
            self.grid = self.power.default_grid();
            self.studio.sheet.scale = 1;
            self.background_update_attempt = None;
            self.power.apply_density(ui.ctx());
            ui.ctx().set_zoom_factor(self.power.interface_scale());
            self.save_workflow_preferences();
        }
    }
    pub(super) fn move_palette_swatch(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        let mut next = self.doc.clone();
        if let Err(error) = next.reorder_palette(from, to) {
            self.status = error;
            return;
        }
        self.remember_action("Reorder palette");
        self.doc = next;
        for slot in
            std::iter::once(&mut self.palette_index).chain(self.secondary_palette_index.iter_mut())
        {
            *slot = match *slot {
                index if index == from => to,
                index if from < index && index <= to => index - 1,
                index if to <= index && index < from => index + 1,
                index => index,
            };
        }
    }

    pub(super) fn picked_swatches(&self) -> &[usize] {
        self.power
            .palette_picks
            .as_ref()
            .filter(|(document, slots)| {
                *document == self.doc.editor_id
                    && slots.iter().all(|&slot| slot < self.doc.palette.len())
            })
            .map_or(&[], |(_, slots)| slots)
    }

    pub(super) fn select_palette_swatch(
        &mut self,
        slot: usize,
        color: Pixel,
        modifiers: egui::Modifiers,
    ) {
        if modifiers.shift {
            let anchor = self.palette_index.min(self.doc.palette.len() - 1);
            let slots = (slot.min(anchor)..=slot.max(anchor)).collect();
            self.power.palette_picks = Some((self.doc.editor_id, slots));
        } else if modifiers.command || modifiers.ctrl {
            let mut slots = self.picked_swatches().to_vec();
            if let Some(index) = slots.iter().position(|&picked| picked == slot) {
                slots.remove(index);
            } else {
                slots.push(slot);
            }
            self.power.palette_picks = Some((self.doc.editor_id, slots));
        } else {
            self.power.palette_picks = None;
            self.fg = color;
            self.palette_index = slot;
        }
    }

    fn apply_palette_order(&mut self, reverse: bool) {
        let mut next = self.doc.clone();
        let selected = self.picked_swatches();
        let result = if reverse && selected.is_empty() {
            let (a, b) = (self.power.ramp_from, self.power.ramp_to);
            next.reverse_palette_entries(&(a.min(b)..=a.max(b)).collect::<Vec<_>>())
        } else if reverse {
            next.reverse_palette_entries(selected)
        } else {
            next.sort_palette_by_frame(selected, self.power.palette_sort, Some(self.frame))
        };
        let remap = match result {
            Ok(remap) => remap,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if next == self.doc {
            return;
        }
        self.remember_action(if reverse {
            "Reverse palette swatches"
        } else {
            "Sort palette swatches"
        });
        self.doc = next;
        for slot in
            std::iter::once(&mut self.palette_index).chain(self.secondary_palette_index.iter_mut())
        {
            if let Some(mapped) = remap.get(*slot) {
                *slot = *mapped;
            }
        }
    }

    fn palette_arrangement_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Palette order & ramps", |ui| {
            let max = self.doc.palette.len() - 1;
            ui.add(
                egui::DragValue::new(&mut self.power.ramp_from)
                    .range(0..=max)
                    .prefix("From swatch "),
            );
            ui.add(
                egui::DragValue::new(&mut self.power.ramp_to)
                    .range(0..=max)
                    .prefix("To swatch "),
            );
            if ui.pc_button("Move swatch to position").clicked() {
                self.move_palette_swatch(self.power.ramp_from, self.power.ramp_to);
            }
            let name = |sort| match sort {
                PaletteSort::Hue => "Hue",
                PaletteSort::Saturation => "Saturation",
                PaletteSort::Value => "Value",
                PaletteSort::Luminance => "Luminance",
                PaletteSort::Rgb => "RGB channels",
                PaletteSort::Alpha => "Alpha",
                PaletteSort::Name => "Swatch name",
            };
            egui::ComboBox::from_label("Sort by").selected_text(name(self.power.palette_sort)).show_ui(ui, |ui| {
                for sort in [PaletteSort::Hue, PaletteSort::Saturation, PaletteSort::Value, PaletteSort::Luminance, PaletteSort::Rgb, PaletteSort::Alpha, PaletteSort::Name] {
                    ui.pc_selectable_value(&mut self.power.palette_sort, sort, name(sort));
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.pc_button(if self.picked_swatches().is_empty() { "Sort palette" } else { "Sort selected swatches" }).clicked() {
                    self.apply_palette_order(false);
                }
                if ui.pc_button("Reverse selected swatches").clicked() {
                    self.apply_palette_order(true);
                }
            });
            let interpolation = |space| match space {
                pixelcraft::workflows::RampInterpolation::Rgb => "RGB",
                pixelcraft::workflows::RampInterpolation::Hue => "Hue",
            };
            egui::ComboBox::from_label("Ramp interpolation")
                .selected_text(interpolation(self.power.ramp_interpolation))
                .show_ui(ui, |ui| {
                    for mode in [pixelcraft::workflows::RampInterpolation::Rgb, pixelcraft::workflows::RampInterpolation::Hue] {
                        ui.pc_selectable_value(&mut self.power.ramp_interpolation, mode, interpolation(mode));
                    }
                });
            ui.add(
                egui::DragValue::new(&mut self.power.ramp_steps)
                    .range(2..=256)
                    .prefix("Ramp steps (including endpoints) "),
            );
            if ui.add_enabled_ui(self.power.ramp_steps >= 3, |ui| {
                ui.pc_button("Add ramp between swatches").clicked()
            }).inner {
                let mut next = self.doc.clone();
                match next.palette_ramp_interpolated(
                    self.power.ramp_from,
                    self.power.ramp_to,
                    self.power.ramp_steps,
                    self.power.ramp_interpolation,
                ) {
                    Ok(()) => {
                        self.remember_action("Add palette ramp");
                        self.doc = next;
                    }
                    Err(e) => self.status = e,
                }
            }
            if ui.pc_button("Replace selected span").clicked() {
                self.replace_palette_ramp();
            }
            ui.small("Swatch numbers start at 0. Replacing the span maps indexed pixels to the corresponding shade in the ramp.");
        });
    }

    fn replace_palette_ramp(&mut self) {
        let mut next = self.doc.clone();
        let remap = match next.replace_palette_span_interpolated(
            self.power.ramp_from,
            self.power.ramp_to,
            self.power.ramp_steps,
            self.power.ramp_interpolation,
        ) {
            Ok(remap) => remap,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let swatches = |doc: &Document| {
            if doc.color_mode == ColorMode::Indexed {
                doc.editing_palette(self.layer, self.frame)
            } else {
                doc.palette_for(self.frame).to_vec()
            }
        };
        let before = swatches(&self.doc);
        let after = swatches(&next);
        self.remember_action("Replace palette span with ramp");
        self.doc = next;
        let update = |slot: usize, color: &mut Pixel| {
            let mapped = remap[slot.min(before.len() - 1)];
            if before.get(slot) == Some(color) {
                *color = after[mapped];
            }
            mapped
        };
        self.palette_index = update(self.palette_index, &mut self.fg);
        if let Some(slot) = self.secondary_palette_index {
            self.secondary_palette_index = Some(update(slot, &mut self.bg));
        }
        self.power.ramp_from = remap[self.power.ramp_from];
        self.power.ramp_to = remap[self.power.ramp_to];
        self.status = "Replaced palette span with ramp".into();
    }
    pub(super) fn recent_file_entries(&self) -> Vec<(PathBuf, String)> {
        let files = &self.power.preferences.recent;
        files
            .iter()
            .map(|path| (path.clone(), recent_file_label(path, files)))
            .collect()
    }
    pub(super) fn open_recent(&mut self, p: PathBuf) {
        self.queue_file_open(p, OpenTarget::Tab);
    }

    pub(super) fn open_as_raster_layer(&mut self, p: PathBuf) {
        self.queue_file_open(
            p,
            OpenTarget::RasterLayer {
                document: self.doc.editor_id,
                frame_id: self.doc.frame_export_id(self.frame),
            },
        );
    }

    fn queue_file_open(&mut self, p: PathBuf, target: OpenTarget) {
        if self.open_job.is_some() {
            self.status = "A file is already opening".into();
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.status = format!(
            "Opening {}",
            p.file_name().unwrap_or_default().to_string_lossy()
        );
        let into_layer = matches!(target, OpenTarget::RasterLayer { .. });
        let import_policy = self.power.preferences.import_profile;
        let native = p
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ["pixel", "ase", "aseprite"]
                    .iter()
                    .any(|native| native.eq_ignore_ascii_case(ext))
            });
        self.open_job = Some(FileOpenJob {
            target,
            receiver: rx,
        });
        std::thread::spawn(move || {
            let result = if into_layer {
                read_raster_layer(&p).and_then(|source| {
                    let policy = if import_policy == pixelcraft::color::ImportProfile::AssignSrgb {
                        import_policy
                    } else {
                        pixelcraft::color::ImportProfile::Preserve
                    };
                    pixelcraft::color::apply_import_profile(source, policy)
                })
            } else if native {
                formats::load(&p)
            } else {
                formats::load(&p).and_then(|source| {
                    pixelcraft::color::apply_import_profile(source, import_policy)
                })
            };
            let _ = tx.send((p, result));
        });
    }

    pub(super) fn poll_open_document(&mut self) {
        if self.start.is_some() || self.script_job.is_some() {
            return;
        }
        let Some(job) = &self.open_job else { return };
        let (p, result) = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.open_job = None;
                self.status = "File loading worker stopped unexpectedly".into();
                return;
            }
        };
        let target = self.open_job.take().unwrap().target;
        if let OpenTarget::RasterLayer { document, frame_id } = target {
            let result = result.and_then(|source| {
                let frame = (self.doc.editor_id == document)
                    .then(|| {
                        (0..self.doc.durations.len())
                            .find(|&frame| self.doc.frame_export_id(frame) == frame_id)
                    })
                    .flatten()
                    .ok_or("The import target is no longer active")?;
                self.install_raster_layer(&p, &source, frame)
            });
            self.status = match result {
                Ok(()) => "Imported image as layer".into(),
                Err(error) => format!("Could not import layer: {error}"),
            };
            return;
        }
        match result {
            Ok(d) => {
                self.add_recent(p.clone());
                let editable = matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("pixel" | "ase" | "aseprite")
                );
                self.open_document(d, editable.then_some(p));
                if std::mem::take(&mut self.play_after_open) {
                    self.start_playback(false, false);
                }
                self.status = "Opened sprite".into();
            }
            Err(e) => {
                self.play_after_open = false;
                self.status = format!("Could not open: {e}");
            }
        }
    }

    pub(super) fn install_raster_layer(
        &mut self,
        path: &Path,
        source: &Document,
        frame: usize,
    ) -> Result<(), String> {
        let name = path
            .file_stem()
            .filter(|stem| !stem.is_empty())
            .map_or_else(
                || "Imported image".into(),
                |stem| stem.to_string_lossy().into_owned(),
            );
        let layer = prepare_raster_layer(&self.doc, source, frame, name)?;
        self.remember_action("Import as layer");
        self.doc.layers.push(layer);
        self.stop_playback(false);
        self.layer = self.doc.layers.len() - 1;
        self.frame = frame;
        self.timeline_anchor = None;
        self.selected_cels = None;
        self.timeline_frames = TimelineHeaders::default();
        self.timeline_layers = TimelineHeaders::default();
        self.timeline_destination = None;
        Ok(())
    }
    pub(super) fn draw_selection_mask(&self, painter: &egui::Painter, board: Rect, view: Rect) {
        let Some(mask) = &self.studio.mask else {
            return;
        };
        let pixel_size = self.canvas_pixel_size();
        let step = (1. / pixel_size.x.max(pixel_size.y)).ceil().max(1.) as i32;
        let x0 = (((view.min.x - board.min.x) / pixel_size.x).floor() as i32)
            .clamp(0, self.doc.width as i32);
        let y0 = (((view.min.y - board.min.y) / pixel_size.y).floor() as i32)
            .clamp(0, self.doc.height as i32);
        let x1 = (((view.max.x - board.min.x) / pixel_size.x).ceil() as i32)
            .clamp(0, self.doc.width as i32);
        let y1 = (((view.max.y - board.min.y) / pixel_size.y).ceil() as i32)
            .clamp(0, self.doc.height as i32);
        let selected = |x, y| {
            self.doc
                .index(x, y)
                .is_some_and(|i| mask.get(i) == Some(&true))
        };
        for y in (y0..y1).step_by(step as usize) {
            for x in (x0..x1).step_by(step as usize) {
                if !selected(x, y) {
                    continue;
                }
                for (a, b, neighbor) in [
                    ((x, y), (x + step, y), (x, y - step)),
                    ((x, y + step), (x + step, y + step), (x, y + step)),
                    ((x, y), (x, y + step), (x - step, y)),
                    ((x + step, y), (x + step, y + step), (x + step, y)),
                ] {
                    if !selected(neighbor.0, neighbor.1) {
                        let pts = [
                            board.min + Vec2::new(a.0 as f32, a.1 as f32) * pixel_size,
                            board.min + Vec2::new(b.0 as f32, b.1 as f32) * pixel_size,
                        ];
                        painter.line_segment(pts, Stroke::new(2., Color32::BLACK));
                        painter.line_segment(pts, Stroke::new(1., Color32::WHITE));
                    }
                }
            }
        }
    }
    pub(super) fn save_workflow_preferences(&mut self) {
        let result = (|| {
            let dir = extensions::config_directory();
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(
                dir.join("workflow.json"),
                serde_json::to_vec_pretty(&self.power.preferences).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        })();
        if let Err(e) = result {
            self.status = format!("Preferences could not be saved: {e}");
        }
    }
    pub(super) fn add_recent(&mut self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        self.power.preferences.recent.retain(|p| p != &path);
        self.power.preferences.recent.insert(0, path);
        self.power.preferences.recent.truncate(15);
        if self.shot.is_none() {
            self.save_workflow_preferences();
        }
    }
    pub(super) fn wand_at(&mut self, p: (i32, i32), op: SelectionOp) {
        let hits = matching_on_axes(
            &self.doc.layer_pixels(self.layer, self.frame),
            self.doc.width,
            self.doc.height,
            p,
            self.power.tolerance,
            self.power.contiguous,
            self.drawing_axes(),
        );
        let mut mask = if self.selection.is_some() {
            (0..hits.len())
                .map(|i| self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32))
                .collect()
        } else {
            vec![false; hits.len()]
        };
        combine_selection(&mut mask, &hits, op);
        self.studio.polygon.clear();
        self.set_selection_mask(mask);
    }
    pub(super) fn apply_marquee(&mut self, bounds: SelectionBounds, op: SelectionOp) {
        if op == SelectionOp::Replace || (op == SelectionOp::Add && self.selection.is_none()) {
            if self.selection != Some(bounds)
                || self.studio.mask.is_some()
                || !self.studio.polygon.is_empty()
            {
                self.remember_action("Marquee selection");
                self.studio.mask = None;
                self.studio.polygon.clear();
                self.selection = Some(bounds);
            }
            return;
        }
        if self.selection.is_none() {
            return;
        }

        let mut mask = self.studio.mask.clone().unwrap_or_else(|| {
            (0..self.doc.height as i32)
                .flat_map(|y| (0..self.doc.width as i32).map(move |x| (x, y)))
                .map(|(x, y)| self.selected(x, y))
                .collect()
        });
        let (left, top, right, bottom) = bounds;
        let width = self.doc.width;
        let mut changed = false;
        for (y, row) in mask.chunks_exact_mut(width).enumerate() {
            if op == SelectionOp::Intersect {
                for (x, selected) in row.iter_mut().enumerate() {
                    let inside = (top..=bottom).contains(&(y as i32))
                        && (left..=right).contains(&(x as i32));
                    changed |= *selected && !inside;
                    *selected &= inside;
                }
            } else if (top..=bottom).contains(&(y as i32)) {
                let region = &mut row[left as usize..=right as usize];
                let selected = op == SelectionOp::Add;
                changed |= region.iter().any(|&current| current != selected);
                region.fill(selected);
            }
        }
        if changed {
            self.remember_action("Marquee selection");
            self.studio.polygon.clear();
            self.set_selection_mask(mask);
        }
    }

    pub(super) fn set_selection_mask(&mut self, mask: Vec<bool>) {
        let (mut x0, mut y0, mut x1, mut y1) = (self.doc.width, self.doc.height, 0, 0);
        let mut any = false;
        for (i, hit) in mask.iter().enumerate() {
            if *hit {
                any = true;
                x0 = x0.min(i % self.doc.width);
                y0 = y0.min(i / self.doc.width);
                x1 = x1.max(i % self.doc.width);
                y1 = y1.max(i / self.doc.width);
            }
        }
        self.selection = if any {
            Some((x0 as i32, y0 as i32, x1 as i32, y1 as i32))
        } else {
            Some((0, 0, 0, 0))
        };
        self.studio.mask = Some(mask);
    }
    pub(super) fn power_paint(&mut self, p: (i32, i32), color: Pixel, size: i32) {
        self.power_paint_opacity(p, color, size, None);
    }

    pub(super) fn power_paint_opacity(
        &mut self,
        p: (i32, i32),
        color: Pixel,
        size: i32,
        opacity: Option<u8>,
    ) {
        let axes = self.drawing_axes();
        let [left, top, width, height] = self.selection_bounds();
        let stride = self.doc.width;
        let mask = self.studio.mask.as_deref();
        let polygon = &self.studio.polygon;
        let has_selection = self.selection.is_some();
        let allows = |index| {
            if !has_selection {
                return true;
            }
            if let Some(mask) = mask {
                return mask.get(index) == Some(&true);
            }
            let (x, y) = ((index % stride) as i32, (index / stride) as i32);
            (left..left + width).contains(&x)
                && (top..top + height).contains(&y)
                && (polygon.is_empty() || pixelcraft::workflows::inside_polygon(polygon, x, y))
        };
        let mode = if self.tool == Tool::Eraser {
            BrushMode::Normal
        } else {
            self.power.mode
        };
        let mut o = BrushOptions {
            mode,
            alternate: self.alternate_stroke_color(),
            reverse: self.power.reverse,
            stamp: None,
            mask: None,
        };
        if mode == BrushMode::Shade {
            o.mode = BrushMode::Normal;
            let target = (self.layer, self.frame);
            let reverse = self.power.reverse;
            let ramp = self.power.brush_library.active_ramp();
            let seen = &mut self.power.shade_seen;
            paint_brush_using(
                &mut self.doc,
                (target.0, target.1, axes),
                p,
                color,
                size,
                &o,
                |doc, index, _| {
                    if allows(index)
                        && seen.insert(index)
                        && let Some((value, slot)) =
                            shading_color(doc, target, index, reverse, ramp)
                    {
                        let previous = doc.drawing_index;
                        doc.drawing_index = slot;
                        doc.set_pixel(target.0, target.1, index, value);
                        doc.drawing_index = previous;
                    }
                },
            );
        } else {
            let ink = self.tool_ink();
            let target = (self.layer, self.frame);
            let original = self
                .stroke_base
                .as_ref()
                .map(|doc| &doc.layers[target.0].cels[target.1]);
            let eraser = self.tool == Tool::Eraser;
            let strongest = &mut self.power.stroke_opacity;
            let apply = |doc: &mut Document, index, source| {
                if !allows(index) {
                    return;
                }
                let old = original.map_or(doc.layers[target.0].cels[target.1][index], |base| {
                    base[index]
                });
                if let Some(opacity) = opacity {
                    let used = strongest.entry(index).or_default();
                    if opacity <= *used {
                        return;
                    }
                    *used = opacity;
                    if eraser {
                        let [r, g, b, alpha] = old;
                        let alpha =
                            ((u16::from(alpha) * u16::from(255 - opacity) + 127) / 255) as u8;
                        let color = if alpha == 0 { CLEAR } else { [r, g, b, alpha] };
                        doc.set_pixel(target.0, target.1, index, color);
                    } else {
                        let ink = if ink == PaintInk::Default {
                            PaintInk::AlphaCompositing
                        } else {
                            ink
                        };
                        ink.draw_with_opacity(doc, target, index, old, source, opacity);
                    }
                } else {
                    ink.draw(doc, target, index, old, source);
                }
            };
            if mode == BrushMode::Selection {
                if let Some((width, height, pixels)) = &self.power.stamp {
                    let pattern_origin = match self.power.image_brush_alignment {
                        ImageBrushAlignment::Stamp => None,
                        ImageBrushAlignment::Source => Some(self.power.image_brush_origin),
                        ImageBrushAlignment::Stroke => {
                            let (x, y) = self.start.unwrap_or(p);
                            Some((x - (*width / 2) as i32, y - (*height / 2) as i32))
                        }
                    };
                    paint_image_brush_using(
                        &mut self.doc,
                        (target.0, target.1, axes),
                        p,
                        ImageBrushDab {
                            size: (*width, *height),
                            pixels,
                            pattern_origin,
                        },
                        o.mask.as_deref(),
                        apply,
                    );
                }
            } else {
                paint_brush_using(
                    &mut self.doc,
                    (target.0, target.1, axes),
                    p,
                    color,
                    size,
                    &o,
                    apply,
                );
            }
        }
    }

    pub(super) fn brush_dynamics(&self) -> BrushDynamics {
        self.power.preferences.brush_dynamics
    }

    pub(super) fn set_brush_dynamics(&mut self, dynamics: BrushDynamics) {
        if self.power.preferences.brush_dynamics == dynamics {
            return;
        }
        self.power.preferences.brush_dynamics = dynamics;
        if self.shot.is_none() {
            self.save_workflow_preferences();
        }
    }

    pub(super) fn brush_dynamics_controls(&mut self, ui: &mut egui::Ui) {
        let mut dynamics = self.brush_dynamics();
        ui.horizontal(|ui| {
            ui.label("Stabilizer distance");
            ui.add(
                egui::DragValue::new(&mut dynamics.stabilizer)
                    .range(0..=64)
                    .suffix(" px"),
            );
        });
        ui.pc_checkbox(&mut dynamics.pressure_size, "Pressure controls size");
        ui.pc_checkbox(&mut dynamics.pressure_opacity, "Pressure controls opacity");
        self.set_brush_dynamics(dynamics);
    }

    pub(super) fn active_image_brush_size(&self) -> Option<(i32, i32)> {
        if self.tool == Tool::Pencil && self.power.mode == BrushMode::Selection {
            self.power
                .stamp
                .as_ref()
                .map(|&(width, height, _)| (width as i32, height as i32))
        } else {
            None
        }
    }

    fn tool_supports_ink(&self) -> bool {
        matches!(
            self.tool,
            Tool::Pencil
                | Tool::Fill
                | Tool::Line
                | Tool::Rect
                | Tool::Ellipse
                | Tool::Polygon
                | Tool::Contour
                | Tool::Curve
        ) && !(self.tool == Tool::Pencil && self.power.mode == BrushMode::Shade)
    }

    pub(super) fn tool_ink(&self) -> PaintInk {
        if self.tool_supports_ink() && self.doc.layers[self.layer].tileset.is_none() {
            self.power
                .preferences
                .tool_inks
                .get(self.tool.action())
                .copied()
                .unwrap_or_default()
        } else {
            PaintInk::Default
        }
    }

    pub(super) fn set_tool_ink(&mut self, ink: PaintInk) {
        let key = self.tool.action();
        if ink == PaintInk::Default {
            self.power.preferences.tool_inks.remove(key);
        } else {
            self.power.preferences.tool_inks.insert(key.into(), ink);
        }
        if self.shot.is_none() {
            self.save_workflow_preferences();
        }
    }

    pub(super) fn drawing_ink_ui(&mut self, ui: &mut egui::Ui) {
        if !self.tool_supports_ink() || self.doc.layers[self.layer].tileset.is_some() {
            return;
        }
        let original = self.tool_ink();
        let mut ink = original;
        let label = |option| match option {
            PaintInk::Default => "Default",
            PaintInk::CopyColor => "Copy Color",
            PaintInk::AlphaCompositing => "Alpha Compositing",
            PaintInk::LockAlpha => "Lock Alpha",
        };
        egui::ComboBox::from_id_salt("drawing-ink")
            .selected_text(format!("Ink · {}", label(ink)))
            .show_ui(ui, |ui| {
                for value in [
                    PaintInk::Default,
                    PaintInk::CopyColor,
                    PaintInk::AlphaCompositing,
                    PaintInk::LockAlpha,
                ] {
                    ui.pc_selectable_value(&mut ink, value, label(value));
                }
            });
        if ink != original {
            self.set_tool_ink(ink);
        }
    }
    pub(super) fn fill_controls(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Fill options", |ui| {
            if self.doc.layers[self.layer].tileset.is_none() {
                ui.add(
                    egui::Slider::new(&mut self.power.fill_tolerance, 0..=255)
                        .text("Color tolerance"),
                );
            }
            ui.pc_checkbox(&mut self.power.fill_global, "Match disconnected areas");
        });
    }

    pub(super) fn pick_tile(&mut self, p: (i32, i32)) -> bool {
        let Some((geometry, x, y)) = self.doc.tile_at_point(self.layer, self.frame, p) else {
            return false;
        };
        let tile = self.doc.layers[self.layer].tile_cels[self.frame][y * geometry.columns + x];
        let tile_id = tile & TILE_ID_MASK;
        if tile_id == 0 {
            self.status = "Empty tile cell".into();
        } else {
            self.selected_tile = tile;
            self.status = format!("Picked tile {tile_id}");
        }
        true
    }

    pub(super) fn power_fill(&mut self, p: (i32, i32), color: Pixel) {
        if let Some(tileset) = self.doc.layers[self.layer].tileset {
            if self.tile_pixel_mode.is_some() {
                self.status = "Switch to Place tiles to fill tile cells".into();
                return;
            }
            let Some((geometry, x, y)) = self.doc.tile_at_point(self.layer, self.frame, p) else {
                return;
            };
            let cells = &self.doc.layers[self.layer].tile_cels[self.frame];
            let mut matches = matching_by_on_axes(
                cells,
                geometry.columns,
                geometry.rows,
                (x as i32, y as i32),
                !self.power.fill_global,
                self.drawing_axes(),
                |a, b| a == b,
            );
            if self.selection.is_some() || self.studio.mask.is_some() {
                let tile = &self.doc.tilesets[tileset];
                let [x, y, width, height] = self.selection_bounds().map(i64::from);
                let (selected_right, selected_bottom) = (x + width, y + height);
                for (index, hit) in matches.iter_mut().enumerate().filter(|(_, hit)| **hit) {
                    let left =
                        i64::from(geometry.x) + ((index % geometry.columns) * tile.width) as i64;
                    let top =
                        i64::from(geometry.y) + ((index / geometry.columns) * tile.height) as i64;
                    let right = (left + tile.width as i64).min(selected_right);
                    let bottom = (top + tile.height as i64).min(selected_bottom);
                    *hit = (top.max(y)..bottom).any(|row| {
                        (left.max(x)..right).any(|col| self.selected(col as i32, row as i32))
                    });
                }
            }
            if let Err(error) =
                self.doc
                    .paint_matching_tiles(self.layer, self.frame, self.selected_tile, &matches)
            {
                self.status = error;
            }
            return;
        }
        let color = self.doc.snapped_color(self.frame, color);
        let ink = self.tool_ink();
        let hits = matching_on_axes(
            &self.doc.layer_pixels(self.layer, self.frame),
            self.doc.width,
            self.doc.height,
            p,
            self.power.fill_tolerance,
            !self.power.fill_global,
            self.drawing_axes(),
        );
        for (i, hit) in hits.into_iter().enumerate() {
            if hit && self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                let original = self.doc.layers[self.layer].cels[self.frame][i];
                ink.draw(&mut self.doc, (self.layer, self.frame), i, original, color);
            }
        }
    }
    fn capture_stamp(&mut self) {
        self.copy();
        if let Some(c) = &self.clipboard {
            self.power.stamp = Some((c.w, c.h, c.pixels.clone()));
            self.power.image_brush_origin = c
                .selection
                .as_ref()
                .map_or((0, 0), |mask| (mask.origin[0], mask.origin[1]));
            self.power.mode = BrushMode::Selection;
            self.selection = None;
            self.studio.mask = None;
            self.studio.polygon.clear();
            self.tool = Tool::Pencil;
            self.status = "Selection captured as a brush, including transparent pixels".into();
        }
    }

    fn reference_root(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }

    fn reference_available(&self, reference: &ReferenceImage) -> bool {
        reference.options.source.as_ref().is_none_or(|path| {
            self.reference_root() == Some(self.power.reference_availability.root.as_path())
                && self.power.reference_availability.files.contains(path)
        })
    }

    pub(super) fn poll_reference_sources(&mut self, ctx: &egui::Context) {
        if self.power.reference_edit.is_some_and(|index| {
            self.doc.references.get(index).is_none_or(|reference| {
                reference.options.locked || !reference.visible || reference.floating
            })
        }) {
            self.power.reference_edit = None;
            self.power.reference_drag = None;
        }
        if let Some(probe) = &self.power.reference_check {
            match probe.try_recv() {
                Ok(available) => {
                    self.power.reference_availability = available;
                    self.power.reference_check = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.power.reference_check = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }
            }
        }
        if !self
            .doc
            .references
            .iter()
            .any(|reference| reference.options.source.is_some())
        {
            return;
        }
        let now = Instant::now();
        let Some(root) = self.reference_root().map(Path::to_path_buf) else {
            return;
        };
        if now < self.power.reference_check_due && self.power.reference_availability.root == root {
            ctx.request_repaint_after(self.power.reference_check_due - now);
            return;
        }
        let sources: std::collections::HashSet<_> = self
            .doc
            .references
            .iter()
            .filter_map(|reference| reference.options.source.clone())
            .collect();
        let (tx, rx) = std::sync::mpsc::channel();
        self.power.reference_check = Some(rx);
        self.power.reference_check_due = now + Duration::from_secs(2);
        std::thread::spawn(move || {
            let files = sources
                .into_iter()
                .filter(|path| linked_reference_exists(&root, path))
                .collect();
            let _ = tx.send(ReferenceAvailability { root, files });
        });
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn pick_reference_file(&mut self, linked: bool, replace: Option<usize>) {
        let root = self.reference_root().map(Path::to_path_buf);
        let mut dialog =
            rfd::FileDialog::new().add_filter("Reference image", &["png", "jpg", "jpeg"]);
        if let Some(root) = &root {
            dialog = dialog.set_directory(root);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let source = if linked {
            let Some(relative) = root
                .as_ref()
                .and_then(|root| path.strip_prefix(root).ok())
                .filter(|relative| valid_reference_source(relative))
            else {
                self.status = "Choose a reference image within the project folder".into();
                return;
            };
            if !root
                .as_ref()
                .is_some_and(|root| linked_reference_exists(root, relative))
            {
                self.status = "Choose a regular image file inside the project folder".into();
                return;
            }
            let portable = relative
                .components()
                .filter_map(|part| part.as_os_str().to_str())
                .collect::<Vec<_>>()
                .join("/");
            Some(PathBuf::from(portable))
        } else {
            None
        };
        let result = import_reference(&path).and_then(|mut reference| {
            let mut next = self.doc.clone();
            if let Some(index) = replace {
                let old = &next.references[index];
                reference.x = old.x;
                reference.y = old.y;
                reference.visible = old.visible;
                reference.opacity = old.opacity;
                reference.behind = old.behind;
                reference.floating = old.floating;
                reference.options = old.options.clone();
                let [width, height] = old.display_dimensions();
                reference.set_display_width(width);
                if reference.options.scale_y.is_some() {
                    reference.set_display_height(height);
                }
                reference.options.source = source;
                next.references[index] = reference;
            } else {
                reference.options.source = source;
                next.references.push(reference);
            }
            next.validate()?;
            self.remember_action(if replace.is_some() {
                "Relink reference image"
            } else {
                "Import reference image"
            });
            self.doc = next;
            self.power.reference_check_due = Instant::now();
            Ok(())
        });
        if let Err(error) = result {
            self.status = error;
        }
    }

    fn reference_controls(&mut self, ui: &mut egui::Ui) {
        if ui
            .pc_checkbox(
                &mut self.power.preferences.sample_references,
                "Sample reference colors",
            )
            .changed()
        {
            self.save_workflow_preferences();
        }
        ui.horizontal(|ui| {
            if ui.pc_button("Import embedded image…").clicked() {
                self.pick_reference_file(false, None);
            }
            if ui
                .add_enabled_ui(self.reference_root().is_some(), |ui| {
                    ui.pc_button("Link image…")
                        .on_disabled_hover_text("Save the project before linking an image.")
                        .clicked()
                })
                .inner
            {
                self.pick_reference_file(true, None);
            }
        });
        let mut remove = None;
        for index in 0..self.doc.references.len() {
            let mut reference = self.doc.references[index].clone();
            let available = self.reference_available(&reference);
            ui.push_id(("reference-options", index), |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui_icons::checkbox(ui, &mut reference.visible, "eye", &reference.name);
                    if ui.pc_small_button("Remove").clicked() {
                        remove = Some(index);
                    }
                });
                if let Some(path) = &reference.options.source {
                    ui.small(if available {
                        format!("Linked · {}", path.display())
                    } else {
                        format!("Missing linked image · {}", path.display())
                    });
                    if ui.pc_button("Relink or refresh…").clicked() {
                        self.pick_reference_file(true, Some(index));
                        reference = self.doc.references[index].clone();
                    }
                }
                ui.horizontal(|ui| {
                    ui.pc_checkbox(&mut reference.options.locked, "Lock placement");
                    ui.pc_checkbox(&mut reference.options.sampleable, "Allow sampling");
                });
                ui.add_enabled_ui(!reference.options.locked, |ui| {
                    if ui_icons::button_as(
                        ui,
                        "move",
                        "reference-move",
                        "Move / scale",
                        self.power.reference_edit == Some(index),
                        false,
                    )
                    .clicked()
                    {
                        self.power.reference_edit =
                            (self.power.reference_edit != Some(index)).then_some(index);
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut reference.x)
                                .prefix("X ")
                                .range(-16384. ..=16384.),
                        );
                        ui.add(
                            egui::DragValue::new(&mut reference.y)
                                .prefix("Y ")
                                .range(-16384. ..=16384.),
                        );
                    });
                    let mut preserve = reference.options.scale_y.is_none();
                    if ui
                        .pc_checkbox(&mut preserve, "Preserve aspect ratio")
                        .changed()
                    {
                        reference.set_aspect_locked(preserve);
                    }
                    ui.horizontal(|ui| {
                        let [mut width, mut height] = reference.display_dimensions();
                        if ui
                            .add(
                                egui::DragValue::new(&mut width)
                                    .prefix("Width ")
                                    .suffix(" px")
                                    .range(0.01..=16_384_000.),
                            )
                            .changed()
                        {
                            reference.set_display_width(width);
                            height = reference.display_dimensions()[1];
                        }
                        if ui
                            .add(
                                egui::DragValue::new(&mut height)
                                    .prefix("Height ")
                                    .suffix(" px")
                                    .range(0.01..=16_384_000.),
                            )
                            .changed()
                        {
                            reference.set_display_height(height);
                        }
                    });
                    if ui.pc_button("Fit to canvas").clicked() {
                        let [width, height] = reference.display_dimensions();
                        let ratio =
                            (self.doc.width as f32 / width).min(self.doc.height as f32 / height);
                        reference.set_display_width(width * ratio);
                        if !preserve {
                            reference.set_display_height(height * ratio);
                        }
                        let [width, height] = reference.display_dimensions();
                        reference.x = (self.doc.width as f32 - width) / 2.;
                        reference.y = (self.doc.height as f32 - height) / 2.;
                    }
                });
                ui.add(egui::Slider::new(&mut reference.opacity, 0..=255).text("Opacity"));
                ui.horizontal(|ui| {
                    ui.pc_checkbox(&mut reference.behind, "Behind artwork");
                    ui.pc_checkbox(&mut reference.floating, "Floating window");
                });
                if ui
                    .add_enabled_ui(available, |ui| ui.pc_button("Copy to layer").clicked())
                    .inner
                {
                    let mut next = self.doc.clone();
                    match reference_to_layer(&mut next, &reference, self.frame) {
                        Ok(()) => {
                            self.remember_action("Copy reference to layer");
                            self.doc = next;
                            self.layer = self.doc.layers.len() - 1;
                        }
                        Err(error) => self.status = error,
                    }
                }
            });
            if reference != self.doc.references[index] {
                self.remember_action("Edit reference image");
                self.doc.references[index] = reference;
            }
        }
        if let Some(index) = remove {
            self.remember_action("Remove reference image");
            self.doc.references.remove(index);
            self.power.reference_edit = None;
            self.power.reference_drag = None;
        }
        if ui.pc_button("Finish moving references").clicked() {
            self.power.reference_edit = None;
            self.power.reference_drag = None;
        }
    }
    pub(super) fn power_dialogs(&mut self, ctx: &egui::Context) {
        if !self.power.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Workflow tools").open(&mut open).default_width(530.).default_height(650.).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
 self.palette_arrangement_ui(ui);
 ui.collapsing("Magic Wand & brushes",|ui|{if ui_icons::button(ui,"wand","Magic Wand",false,false).clicked(){self.tool=Tool::Wand;}ui.pc_checkbox(&mut self.power.contiguous,"Contiguous matching");ui.add(egui::Slider::new(&mut self.power.tolerance,0..=255).text("Color tolerance"));ui.horizontal(|ui|{for op in [SelectionOp::Replace,SelectionOp::Add,SelectionOp::Subtract]{ui.pc_selectable_value(&mut self.power.selection_op,op,format!("{op:?}"));}});ui.small("Shift-click adds; Ctrl/Cmd-click subtracts.");self.brush_mode_ui(ui);if ui.pc_button("Capture selection as stamp").clicked(){self.capture_stamp();}ui.pc_checkbox(&mut self.power.reverse,"Shade toward previous palette color");ui.small("Dither alternates active/alternate colors on the canvas grid. Shade changes each pixel once per stroke.");});
 ui.collapsing("Wraparound & palette",|ui|{let mut wrap=self.doc.wraparound;let mut snap=self.doc.palette_snap;ui.pc_checkbox(&mut wrap,"Wrap drawing & fill across canvas edges");ui.pc_checkbox(&mut snap,"Snap pencil, fill & shapes to palette in RGB mode");if wrap!=self.doc.wraparound||snap!=self.doc.palette_snap{self.remember_action("Change drawing options");self.doc.wraparound=wrap;self.doc.palette_snap=snap;}ui.small("The seamless canvas uses the axes selected under View.");ui.horizontal(|ui|{if ui.pc_button("Import palette…").clicked(){self.palette_file(false);}
if ui.pc_button("Export palette…").clicked(){self.palette_file(true);}});ui.small("HEX supports RGBA. JASC PAL and GIMP GPL store RGB.");});
 ui.collapsing("Reference images", |ui| self.reference_controls(ui));
 ui.collapsing("Onion skin", |ui| { ui.pc_checkbox(&mut self.onion, "Enable onion skin"); ui.small("Set frame counts, tints, and opacity in Edit → Settings."); });
 ui.collapsing("Grid sprite-sheet import", |ui| self.grid_import_ui(ui));
        ui.collapsing("Batch export animation tags", |ui| {
            ui.small("One file per tag. Frame timing and forward/reverse/ping-pong order are preserved.");
            egui::ComboBox::from_label("Sheet engine preset")
                .selected_text(&self.studio.sheet.preset)
                .show_ui(ui, |ui| {
                    for preset in SheetOptions::PRESETS {
                        ui.pc_selectable_value(&mut self.studio.sheet.preset, preset.into(), preset);
                    }
                });
            ui.horizontal(|ui| {
                for (gif, label) in [(true, "Export tag GIFs…"), (false, "Export tag sheets…")] {
                    if ui.add_enabled_ui(self.export_job.is_none(), |ui| ui.pc_button(label)).inner.clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        let kind = ExportKind::Tags { gif, options: Box::new(self.studio.sheet.clone()) };
                        self.start_export_recipe(ExportRecipe { path, kind }, false);
                    }
                }
            });
        });
 ui.collapsing("Multi-file atlas", |ui| self.atlas_export_ui(ui));
 ui.collapsing("Recent files & shortcuts",|ui|{let mut chosen=None;for (path,label) in self.recent_file_entries(){if ui.add_enabled_ui(path.is_file(),|ui|ui_icons::button(ui,"open",&label,false,false)).inner.on_hover_text(path.display().to_string()).clicked(){chosen=Some(path);}}
if let Some(p)=chosen{match formats::load(&p){Ok(d)=>{self.add_recent(p.clone());let editable=matches!(p.extension().and_then(|s|s.to_str()),Some("pixel"|"ase"|"aseprite"));self.open_document(d,editable.then_some(p));},Err(e)=>self.status=format!("Could not open: {e}")}}
if ui.pc_button("Clear recent files").clicked(){self.power.preferences.recent.clear();self.save_workflow_preferences();}ui.separator();if ui.pc_button("Keyboard shortcuts…").clicked(){self.action("shortcuts");}});
 ui.collapsing("Assistants & headless tools",|ui|{ui.label("The MCP server and CLI can work on files without opening the editor.");ui.monospace("pixelcraft --help\npixelcraft --mcp");let mut enabled=self.power.bridge.is_some();if ui.pc_checkbox(&mut enabled,"Allow assistants to access this live editor").changed(){if enabled{let config=std::env::temp_dir().join(format!("pixelcraft-live-{}.json",std::process::id()));let wake=ctx.clone();match LiveBridge::start_with_wake(config,move || wake.request_repaint()){Ok(b)=>{self.power.bridge=Some(b);self.power.bridge_generation+=1;self.status="Live editor access enabled. Edits are undoable.".into();},Err(e)=>self.status=format!("Could not enable bridge: {e}")}}else{if let Some(progress)=self.power.bridge_export.take(){progress.cancel();}self.power.bridge=None;self.power.bridge_work=None;self.status="Live editor access disabled".into();}}
if let Some(b)=&self.power.bridge{ui.label("Live access is active only while this checkbox is enabled.");ui.label("Use this MCP server command:");ui.monospace(format!("pixelcraft --mcp --live \"{}\"",b.config.display()));ui.small("The private bridge file contains an access token. Inspect returns a revision; stale edits are rejected. Ctrl+Z undoes assistant edits.");}});
 });});
        self.power.open = open;
    }
    fn grid_import_ui(&mut self, ui: &mut egui::Ui) {
        if ui.pc_button("Choose sprite sheet…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Sprite sheets", &["png", "jpg", "jpeg", "bmp", "gif"])
                .pick_file()
        {
            match GridSource::read(&path) {
                Ok(source) => {
                    let image = &source.image;
                    let grid = &mut self.power.grid.grid;
                    let default = GridImport {
                        layers: grid.layers,
                        ..GridImport::default()
                    };
                    if *grid == default
                        && let Some(size) = GridImport::suggest_strip(
                            image.width() as usize,
                            image.height() as usize,
                        )
                    {
                        grid.use_cell_size(size);
                    }
                    self.power.grid_texture =
                        Some(import_preview_texture(ui.ctx(), "grid-import", image));
                    self.power.grid_source = Some(source);
                    self.power.grid_preview = None;
                }
                Err(error) => self.status = error,
            }
        }
        let options = &mut self.power.grid;
        let mut from_counts = options.columns.is_some() || options.rows.is_some();
        if ui
            .pc_checkbox(&mut from_counts, "Size cells from row and column counts")
            .changed()
        {
            options.columns = from_counts.then_some(1);
            options.rows = from_counts.then_some(1);
        }
        let grid = &mut options.grid;
        if !from_counts
            && let Some(source) = &self.power.grid_source
            && let Some(size) = GridImport::suggest_strip(
                source.image.width() as usize,
                source.image.height() as usize,
            )
            && ui
                .pc_button(format!("Use suggested {} × {} cells", size.0, size.1))
                .clicked()
        {
            grid.use_cell_size(size);
        }
        if from_counts {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(options.columns.get_or_insert(1))
                        .prefix("Columns ")
                        .range(1..=MAX_DIMENSION),
                );
                ui.add(
                    egui::DragValue::new(options.rows.get_or_insert(1))
                        .prefix("Rows ")
                        .range(1..=MAX_DIMENSION),
                );
            });
        } else {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut grid.width)
                        .prefix("Cell W ")
                        .range(1..=MAX_DIMENSION),
                );
                ui.add(
                    egui::DragValue::new(&mut grid.height)
                        .prefix("H ")
                        .range(1..=MAX_DIMENSION),
                );
            });
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut grid.offset_x)
                    .prefix("Offset X ")
                    .range(0..=16384),
            );
            ui.add(
                egui::DragValue::new(&mut grid.offset_y)
                    .prefix("Y ")
                    .range(0..=16384),
            );
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut grid.spacing_x)
                    .prefix("Spacing X ")
                    .range(0..=16384),
            );
            ui.add(
                egui::DragValue::new(&mut grid.spacing_y)
                    .prefix("Y ")
                    .range(0..=16384),
            );
        });
        ui.pc_checkbox(&mut grid.layers, "Import as layers (otherwise frames)");
        ui.pc_checkbox(&mut options.include_partial, "Include partial edge cells");
        let order_label = match options.order {
            GridOrder::RowMajor => "Row-major",
            GridOrder::ColumnMajor => "Column-major",
        };
        egui::ComboBox::from_label("Cell order")
            .selected_text(order_label)
            .show_ui(ui, |ui| {
                ui.pc_selectable_value(&mut options.order, GridOrder::RowMajor, "Row-major");
                ui.pc_selectable_value(&mut options.order, GridOrder::ColumnMajor, "Column-major");
            });
        ui.pc_checkbox(
            &mut options.skip_transparent,
            "Skip fully transparent cells",
        );
        ui.add(
            egui::DragValue::new(&mut options.duration)
                .range(1..=u16::MAX)
                .prefix("Initial frame duration ")
                .suffix(" ms"),
        );

        let (Some(source), Some(texture)) = (&self.power.grid_source, &self.power.grid_texture)
        else {
            return;
        };
        let image = &source.image;
        if self
            .power
            .grid_preview
            .as_ref()
            .is_none_or(|preview| preview.options != *options)
        {
            self.power.grid_preview = Some(CachedGridPreview {
                options: options.clone(),
                result: preview_grid_with_options(image, options),
            });
        }
        let preview = match &self.power.grid_preview.as_ref().unwrap().result {
            Ok(preview) => preview,
            Err(error) => {
                ui.colored_label(Color32::RED, error);
                return;
            }
        };
        let kind = if options.grid.layers {
            "layers"
        } else {
            "frames"
        };
        let [width, height] = preview.cell_size;
        ui.label(format!(
            "{} {kind} · {width} × {height} px",
            preview.cells.len()
        ));
        if options.skip_transparent {
            ui.small(format!("{} transparent cells skipped", preview.skipped));
        }
        let dimensions = Vec2::new(image.width() as f32, image.height() as f32);
        let scale = (ui.available_width() / dimensions.x)
            .min(260. / dimensions.y)
            .min(8.);
        let response = ui.add(egui::Image::new(texture).fit_to_exact_size(dimensions * scale));
        let painter = ui.painter_at(response.rect);
        let cell_size = Vec2::new(width as f32, height as f32) * scale;
        let cell_rect = |x, y| {
            Rect::from_min_size(
                response.rect.min + Vec2::new(x as f32, y as f32) * scale,
                cell_size,
            )
            .intersect(response.rect)
        };
        let marker = Color32::from_rgb(255, 200, 60);
        if cell_size.x.min(cell_size.y) < 3. {
            let mut markers = egui::Mesh::default();
            for &(x, y) in &preview.cells {
                markers.add_colored_rect(cell_rect(x, y), marker.gamma_multiply(0.45));
            }
            painter.add(egui::Shape::mesh(markers));
        } else {
            for (index, &(x, y)) in preview.cells.iter().enumerate() {
                let rect = cell_rect(x, y);
                painter.rect_stroke(rect, 0, Stroke::new(1., marker), egui::StrokeKind::Inside);
                if rect.width() >= 28. && rect.height() >= 18. && preview.cells.len() <= 256 {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        index + 1,
                        egui::FontId::monospace(11.),
                        marker,
                    );
                }
            }
        }
        if ui
            .add_enabled_ui(!preview.cells.is_empty(), |ui| {
                ui.pc_button("Import cells into a new sprite")
            })
            .inner
            .clicked()
        {
            match source.import(options) {
                Ok(document) => {
                    self.open_document(document, None);
                    self.status = "Imported sheet cells".into();
                }
                Err(error) => self.status = error,
            }
        }
    }

    fn append_atlas_inputs(&mut self, paths: &[PathBuf]) -> Result<(), String> {
        let mut additions = Vec::new();
        for path in paths {
            let files = if path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        ATLAS_EXTENSIONS
                            .iter()
                            .any(|valid| valid.eq_ignore_ascii_case(ext))
                    }) {
                vec![path.clone()]
            } else {
                expand_atlas_inputs(std::slice::from_ref(path))?
            };
            if self.power.atlas_files.len() + additions.len() + files.len() > 4096 {
                return Err("Atlas input limit is 4096 files".into());
            }
            additions.extend(files);
        }
        if !additions.is_empty() {
            self.power.atlas_files.extend(additions);
            self.power.atlas_preview = None;
        }
        Ok(())
    }

    fn add_typed_atlas_input(&mut self) -> Result<(), String> {
        let text = self.power.atlas_path.trim();
        let text = text
            .strip_prefix('"')
            .and_then(|text| text.strip_suffix('"'))
            .unwrap_or(text);
        if text.is_empty() {
            return Err("Enter a sprite or folder path".into());
        }
        let path = Path::new(text);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path
                .as_deref()
                .and_then(Path::parent)
                .unwrap_or(Path::new("."))
                .join(path)
        };
        self.append_atlas_inputs(&[path])?;
        self.power.atlas_path.clear();
        Ok(())
    }

    fn remove_atlas_input(&mut self, index: usize) {
        if index < self.power.atlas_files.len() {
            self.power.atlas_files.remove(index);
            self.power.atlas_preview = None;
        }
    }

    fn atlas_export_ui(&mut self, ui: &mut egui::Ui) {
        if ui.pc_button("Add sprites…").clicked()
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter("Sprites", ATLAS_EXTENSIONS)
                .pick_files()
            && let Err(error) = self.append_atlas_inputs(&paths)
        {
            self.status = error;
        }
        if ui.pc_button("Add folder…").clicked()
            && let Some(dir) = rfd::FileDialog::new().pick_folder()
            && let Err(error) = self.append_atlas_inputs(&[dir])
        {
            self.status = error;
        }
        ui.horizontal(|ui| {
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.power.atlas_path)
                    .hint_text("Sprite or folder path")
                    .desired_width(280.),
            );
            let add = ui.pc_button("Add path").clicked()
                || (input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            if add && let Err(error) = self.add_typed_atlas_input() {
                self.status = error;
            }
        });
        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("atlas-inputs")
            .max_height(160.)
            .show_rows(
                ui,
                ui.spacing().interact_size.y,
                self.power.atlas_files.len(),
                |ui, rows| {
                    for index in rows {
                        let path = &self.power.atlas_files[index];
                        ui.push_id(index, |ui| {
                            ui.horizontal(|ui| {
                                let label = path.display().to_string();
                                let tooltip = format!("Remove {label} from atlas inputs");
                                if ui_icons::button(ui, "trash", &tooltip, false, false).clicked() {
                                    remove = Some(index);
                                }
                                ui.add(egui::Label::new(label.as_str()).truncate())
                                    .on_hover_text(label);
                            });
                        });
                    }
                },
            );
        if let Some(index) = remove {
            self.remove_atlas_input(index);
        }

        let options = &mut self.power.atlas;
        let old_options = options.clone();
        ui.horizontal(|ui| {
            ui.pc_selectable_value(&mut options.layout, AtlasLayout::Rows, "Rows");
            ui.pc_selectable_value(&mut options.layout, AtlasLayout::MaxRects, "MaxRects");
        });
        ui.add(
            egui::DragValue::new(&mut options.width)
                .range(1..=16384)
                .prefix("Maximum width "),
        );
        ui.add(
            egui::DragValue::new(&mut options.padding)
                .range(0..=256)
                .prefix("Padding "),
        );
        ui.pc_checkbox(&mut options.trim, "Trim transparent borders");
        ui.pc_checkbox(&mut options.deduplicate, "Remove duplicate frame pixels");
        ui.pc_checkbox(&mut options.power_of_two, "Power-of-two dimensions");
        if *options != old_options {
            self.power.atlas_preview = None;
        }
        ui.horizontal(|ui| {
            ui.label("Output");
            ui.pc_selectable_value(
                &mut options.metadata,
                AtlasMetadata::Array,
                "PNG + JSON array",
            );
            ui.pc_selectable_value(
                &mut options.metadata,
                AtlasMetadata::Hash,
                "PNG + JSON hash",
            );
            ui.pc_selectable_value(&mut options.metadata, AtlasMetadata::None, "PNG only");
        });

        if ui
            .add_enabled_ui(!self.power.atlas_files.is_empty(), |ui| {
                ui.pc_button("Preview atlas")
            })
            .inner
            .clicked()
        {
            let result = (|| {
                let inputs = self.power.atlas_files.clone();
                let sources = load_atlas_inputs(&inputs)?;
                let packed = pack_atlas(&sources, options)?;
                let texture = import_preview_texture(ui.ctx(), "atlas-preview", &packed.0);
                Ok(AtlasPreview {
                    inputs,
                    options: options.clone(),
                    packed,
                    texture,
                })
            })();
            self.power.atlas_preview = Some(result);
        }
        match &self.power.atlas_preview {
            Some(Ok(preview)) => {
                let (width, height) = preview.packed.0.dimensions();
                ui.label(format!("Atlas size: {width} × {height} px"));
                let dimensions = Vec2::new(width as f32, height as f32);
                let scale = (ui.available_width() / dimensions.x)
                    .min(260. / dimensions.y)
                    .min(8.);
                ui.add(egui::Image::new(&preview.texture).fit_to_exact_size(dimensions * scale));
                if ui
                    .add_enabled_ui(self.export_job.is_none(), |ui| ui.pc_button("Save atlas…"))
                    .inner
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG atlas", &["png"])
                        .set_file_name("atlas.png")
                        .save_file()
                {
                    self.status = match save_packed_atlas_with_metadata(
                        &path,
                        &preview.packed,
                        options.metadata,
                    ) {
                        Ok(()) => {
                            self.last_export = Some(ExportRecipe {
                                path,
                                kind: ExportKind::MultiAtlas {
                                    inputs: preview.inputs.clone(),
                                    options: AtlasOptions {
                                        metadata: options.metadata,
                                        ..preview.options.clone()
                                    },
                                },
                            });
                            "Exported multi-file atlas".into()
                        }
                        Err(error) => format!("Export failed: {error}"),
                    };
                }
            }
            Some(Err(error)) => {
                ui.colored_label(Color32::RED, error);
            }
            None => {
                ui.small("Preview the packed image and its dimensions before saving.");
            }
        }
    }

    fn palette_file(&mut self, export: bool) {
        let dialog = rfd::FileDialog::new()
            .add_filter("HEX palette", &["hex"])
            .add_filter("JASC palette", &["pal"])
            .add_filter("GIMP palette", &["gpl"])
            .add_filter("JSON palette", &["json"]);
        let path = if export {
            dialog.set_file_name("palette.hex").save_file()
        } else {
            dialog.pick_file()
        };
        if let Some(path) = path {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("hex");
            let result = (|| {
                if export {
                    std::fs::write(&path, write_palette(&self.doc.palette, ext)?)
                        .map_err(|e| e.to_string())?;
                } else {
                    let p = parse_palette(&std::fs::read(&path).map_err(|e| e.to_string())?, ext)?;
                    self.remember_action("Import palette");
                    self.doc.import_palette(p)?;
                    self.doc.palette_names.clear();
                }
                Ok::<_, String>(())
            })();
            self.status = match result {
                Ok(()) => "Palette operation complete".into(),
                Err(e) => format!("Could not process palette: {e}"),
            };
        }
    }
    pub(super) fn brush_mode_ui(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt(ui.id().with("brush-mode"))
            .selected_text(format!("{:?}", self.power.mode))
            .width(126.)
            .show_ui(ui, |ui| {
                for mode in [
                    BrushMode::Normal,
                    BrushMode::Shade,
                    BrushMode::Dither,
                    BrushMode::Selection,
                ] {
                    ui.pc_selectable_value(&mut self.power.mode, mode, format!("{mode:?}"));
                }
            });
        if self.active_image_brush_size().is_some() {
            let label = |alignment| match alignment {
                ImageBrushAlignment::Stamp => "Stamp",
                ImageBrushAlignment::Source => "Source aligned",
                ImageBrushAlignment::Stroke => "Stroke aligned",
            };
            egui::ComboBox::from_id_salt(ui.id().with("image-brush-alignment"))
                .selected_text(label(self.power.image_brush_alignment))
                .show_ui(ui, |ui| {
                    for alignment in [
                        ImageBrushAlignment::Stamp,
                        ImageBrushAlignment::Source,
                        ImageBrushAlignment::Stroke,
                    ] {
                        ui.pc_selectable_value(
                            &mut self.power.image_brush_alignment,
                            alignment,
                            label(alignment),
                        );
                    }
                });
        }
    }
    pub(super) fn gradient_controls(&mut self, ui: &mut egui::Ui) {
        let options = &mut self.power.preferences.gradient;
        let before = *options;
        egui::ComboBox::from_id_salt("gradient-shape")
            .selected_text(options.shape.label())
            .show_ui(ui, |ui| {
                for shape in [GradientShape::Linear, GradientShape::Radial] {
                    ui.pc_selectable_value(&mut options.shape, shape, shape.label());
                }
            });
        let has_matrix = self.doc.dithering.is_some();
        ui.label("Pattern");
        let pattern = if options.pattern == GradientPattern::Installed && !has_matrix {
            GradientPattern::Smooth
        } else {
            options.pattern
        };
        egui::ComboBox::from_id_salt("gradient-pattern")
            .selected_text(pattern.label())
            .show_ui(ui, |ui| {
                use GradientPattern::*;
                for pattern in [Smooth, Bayer2, Bayer4, Bayer8, Installed] {
                    ui.add_enabled_ui(pattern != Installed || has_matrix, |ui| {
                        ui.pc_selectable_value(&mut options.pattern, pattern, pattern.label())
                            .on_disabled_hover_text(
                                "Apply a dithering matrix from Extensions first.",
                            );
                    });
                }
            });
        if before != *options && self.shot.is_none() {
            self.save_workflow_preferences();
        }
    }

    pub(super) fn gradient(&mut self, a: (i32, i32), b: (i32, i32)) {
        let dx = (b.0 - a.0) as f32;
        let dy = (b.1 - a.1) as f32;
        let length_squared = dx * dx + dy * dy;
        if length_squared == 0.
            || !self.doc.editable(self.layer)
            || self.doc.layers[self.layer].tileset.is_some()
        {
            return;
        }
        let options = self.power.preferences.gradient;
        let thresholds = options.pattern.thresholds(&self.doc);
        let mut start = if self.gesture_button.is_some() {
            self.stroke_color
        } else {
            self.fg
        };
        let mut end = self.alternate_stroke_color();
        if thresholds.is_none() && start[3] == 0 && end[3] != 0 {
            start[..3].copy_from_slice(&end[..3]);
        } else if thresholds.is_none() && end[3] == 0 && start[3] != 0 {
            end[..3].copy_from_slice(&start[..3]);
        }
        let stops = [start, end];
        let indexed_stops: Option<[(Pixel, u8); 2]> =
            (thresholds.is_some() && self.doc.color_mode == ColorMode::Indexed).then(|| {
                let palette = self.doc.editing_palette(self.layer, self.frame);
                let mut preferred = [Some(self.palette_index), self.secondary_palette_index];
                if self.secondary_stroke() {
                    preferred.swap(0, 1);
                }
                std::array::from_fn(|stop| {
                    let color = stops[stop];
                    let slot = preferred[stop]
                        .filter(|&index| palette.get(index) == Some(&color))
                        .unwrap_or_else(|| {
                            if color[3] == 0 && !self.doc.layers[self.layer].background {
                                self.doc.transparent_index.min(palette.len() - 1)
                            } else {
                                nearest(color, &palette)
                            }
                        });
                    (palette[slot], slot as u8)
                })
            });
        let [left, top, width, height] = self.selection_bounds();
        let stride = self.doc.width;
        for y in top.max(0)..(top + height).min(self.doc.height as i32) {
            for x in left.max(0)..(left + width).min(stride as i32) {
                if !self.selected(x, y) {
                    continue;
                }
                let (ox, oy) = ((x - a.0) as f32, (y - a.1) as f32);
                let t = match options.shape {
                    GradientShape::Linear => (ox * dx + oy * dy) / length_squared,
                    GradientShape::Radial => ((ox * ox + oy * oy) / length_squared).sqrt(),
                }
                .clamp(0., 1.);
                let index = y as usize * stride + x as usize;
                let color = if let Some((width, height, pattern)) = &thresholds {
                    let threshold = pattern[(y as usize % height) * width + x as usize % width];
                    let selected_stop = usize::from(t >= threshold);
                    if let Some(indexed) = &indexed_stops {
                        let (color, slot) = indexed[selected_stop];
                        self.doc
                            .restore_pixel(self.layer, self.frame, index, color, Some(slot));
                        continue;
                    }
                    stops[selected_stop]
                } else {
                    std::array::from_fn(|c| {
                        (start[c] as f32 * (1. - t) + end[c] as f32 * t).round() as u8
                    })
                };
                self.doc.set_pixel(self.layer, self.frame, index, color);
            }
        }
    }

    pub(super) fn eyedropper_controls(&mut self, ui: &mut egui::Ui) {
        let options = &mut self.power.preferences.eyedropper;
        let before = *options;
        ui.label("Sample");
        egui::ComboBox::from_id_salt("eyedropper-source")
            .selected_text(options.source.label())
            .show_ui(ui, |ui| {
                for source in [
                    EyedropperSource::Visible,
                    EyedropperSource::Active,
                    EyedropperSource::References,
                ] {
                    ui.pc_selectable_value(&mut options.source, source, source.label());
                }
            });
        ui.label("Pick");
        egui::ComboBox::from_id_salt("eyedropper-channel")
            .height(320.)
            .selected_text(options.channel.label())
            .show_ui(ui, |ui| {
                use EyedropperChannel::*;
                for channel in [
                    ColorAlpha,
                    Color,
                    Alpha,
                    Red,
                    Green,
                    Blue,
                    Gray,
                    GrayAlpha,
                    PaletteIndex,
                ] {
                    ui.pc_selectable_value(&mut options.channel, channel, channel.label());
                }
            });
        if before != *options && self.shot.is_none() {
            self.save_workflow_preferences();
        }
    }

    pub(super) fn eyedropper_picks_tiles(&self) -> bool {
        self.power.preferences.eyedropper == EyedropperOptions::default()
    }

    pub(super) fn pick_color(&mut self, (x, y): (i32, i32), secondary: bool) {
        let (sample, index) = match self.power.preferences.eyedropper.source {
            EyedropperSource::Visible => (
                Some(sample_canvas_if(
                    &self.doc,
                    self.frame,
                    x,
                    y,
                    self.power.preferences.sample_references,
                    |reference| self.reference_available(reference),
                )),
                None,
            ),
            EyedropperSource::Active => {
                let layer = &self.doc.layers[self.layer];
                let index = if self.doc.color_mode == ColorMode::Indexed && layer.tileset.is_none()
                {
                    self.doc.index(x, y).and_then(|index| {
                        layer.cel_meta[self.frame]
                            .indices
                            .as_ref()
                            .and_then(|indices| indices.get(index))
                            .map(|index| usize::from(*index))
                    })
                } else {
                    None
                };
                (self.doc.sample_layer(self.layer, self.frame, x, y), index)
            }
            EyedropperSource::References => (
                sample_reference_image_if(&self.doc, x, y, |reference| {
                    self.reference_available(reference)
                }),
                None,
            ),
        };
        if let Some(sample) = sample {
            self.apply_eyedropper_sample(sample, index, secondary);
        }
    }

    fn apply_eyedropper_sample(
        &mut self,
        sample: Pixel,
        authored_index: Option<usize>,
        secondary: bool,
    ) {
        use EyedropperChannel::*;
        let current = if secondary { self.bg } else { self.fg };
        let [r, g, b, a] = sample;
        let mut slot = None;
        let color = match self.power.preferences.eyedropper.channel {
            ColorAlpha => {
                slot = authored_index;
                sample
            }
            Alpha => [current[0], current[1], current[2], a],
            GrayAlpha => convert(sample, ColorMode::Grayscale, &[]),
            PaletteIndex => {
                let palette = if self.doc.color_mode == ColorMode::Indexed {
                    self.doc.editing_palette(self.layer, self.frame)
                } else {
                    self.doc.palette_for(self.frame).to_vec()
                };
                let index = authored_index
                    .filter(|&index| index < palette.len())
                    .unwrap_or_else(|| nearest(sample, &palette));
                slot = Some(index);
                let Some(&color) = palette.get(index) else {
                    return;
                };
                color
            }
            _ if a == 0 => return,
            Color => [r, g, b, current[3]],
            Red => [r, current[1], current[2], current[3]],
            Green => [current[0], g, current[2], current[3]],
            Blue => [current[0], current[1], b, current[3]],
            Gray => {
                let [gray, _, _, _] = convert(sample, ColorMode::Grayscale, &[]);
                [gray, gray, gray, current[3]]
            }
        };
        if color == current && slot.is_none() {
            return;
        }
        if secondary {
            self.set_secondary_color(color, slot);
        } else {
            self.set_active_color(color);
            if let Some(index) = slot {
                self.palette_index = index;
            }
        }
    }

    pub(super) fn draw_references(&mut self, ui: &mut egui::Ui, board: Rect) {
        self.draw_references_pass(ui, board, false, true);
    }
    pub(super) fn draw_references_pass(
        &mut self,
        ui: &mut egui::Ui,
        board: Rect,
        behind: bool,
        show_floating: bool,
    ) {
        for i in 0..self.doc.references.len() {
            let r = self.doc.references[i].clone();
            if !r.visible
                || (if r.floating {
                    behind || !show_floating
                } else {
                    r.behind != behind
                })
            {
                continue;
            }
            let tex = self.reference_available(&r).then(|| {
                if let Some((_, t)) = self
                    .power
                    .reference_textures
                    .iter()
                    .find(|(p, _)| p.shares_storage(&r.pixels))
                {
                    t.clone()
                } else {
                    let pixels = self.display_artwork(&[], r.pixels.to_vec());
                    let t = ui.ctx().load_texture(
                        format!("reference-{i}"),
                        color_image(&pixels, r.width, r.height),
                        egui::TextureOptions::LINEAR,
                    );
                    self.power
                        .reference_textures
                        .push((r.pixels.clone(), t.clone()));
                    t
                }
            });
            if r.floating {
                if !behind {
                    let mut open = true;
                    egui::Window::new(format!("Reference: {}", r.name))
                        .id(egui::Id::new(("floating-reference", i)))
                        .open(&mut open)
                        .default_width(300.)
                        .show(ui.ctx(), |ui| {
                            ui.pc_checkbox(
                                &mut self.power.preferences.sample_references,
                                "Sample reference colors",
                            );
                            let Some(tex) = &tex else {
                                ui.label("Missing linked image");
                                return;
                            };
                            let [width, height] = r.display_dimensions();
                            let size = Vec2::new(width, height);
                            let scale = (ui.available_width() / size.x).min(450. / size.y);
                            let response = ui.add(
                                egui::Image::new(tex)
                                    .fit_to_exact_size(size * scale)
                                    .sense(Sense::click()),
                            );
                            let picker = self.tool == Tool::Picker;
                            let explicit_reference = picker
                                && self.power.preferences.eyedropper.source
                                    == EyedropperSource::References;
                            let secondary = picker && response.secondary_clicked();
                            if (response.clicked() || secondary)
                                && r.options.sampleable
                                && (self.power.preferences.sample_references || explicit_reference)
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let xy = (pos - response.rect.min) / response.rect.size();
                                let x = ((xy.x * r.width as f32) as usize).min(r.width - 1);
                                let y = ((xy.y * r.height as f32) as usize).min(r.height - 1);
                                if picker {
                                    self.apply_eyedropper_sample(
                                        r.sample_pixel(x, y),
                                        None,
                                        secondary,
                                    );
                                } else {
                                    self.set_drawing_color(over(
                                        CLEAR,
                                        r.pixels[y * r.width + x],
                                        r.opacity,
                                    ));
                                }
                            }
                        });
                    if !open {
                        self.remember_action("Dock reference image");
                        self.doc.references[i].floating = false;
                    }
                }
                continue;
            }
            let pixel_size = self.canvas_pixel_size();
            let [width, height] = r.display_dimensions();
            let rect = Rect::from_min_size(
                board.min + Vec2::new(r.x, r.y) * pixel_size,
                Vec2::new(width, height) * pixel_size,
            );
            if let Some(tex) = tex {
                ui.painter().image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    Color32::from_white_alpha(r.opacity),
                );
            } else {
                let painter = ui.painter();
                let visible = rect.intersect(painter.clip_rect());
                if visible.is_positive() {
                    let faded = Color32::from_rgba_unmultiplied(45, 40, 48, 180);
                    painter.rect_filled(rect, 0, faded);
                    let stroke = Stroke::new(1., Color32::LIGHT_GRAY);
                    painter.rect_stroke(rect, 0, stroke, egui::StrokeKind::Inside);
                    painter.line_segment([rect.left_top(), rect.right_bottom()], stroke);
                    painter.line_segment([rect.right_top(), rect.left_bottom()], stroke);
                    if visible.width() > 120. && visible.height() > 20. {
                        painter.text(
                            visible.center(),
                            egui::Align2::CENTER_CENTER,
                            "Missing linked image",
                            egui::FontId::proportional(12.),
                            Color32::WHITE,
                        );
                    }
                }
            }
            if !r.options.locked && self.power.reference_edit == Some(i) {
                let response = ui.interact(
                    rect.intersect(ui.clip_rect()),
                    ui.id().with(("reference-drag", i)),
                    Sense::drag(),
                );
                ui.painter().rect_stroke(
                    rect,
                    0,
                    Stroke::new(1., Color32::YELLOW),
                    egui::StrokeKind::Inside,
                );
                if response.drag_started() {
                    self.remember_action("Move reference image");
                    self.power.reference_drag = ui
                        .input(|i| i.pointer.press_origin())
                        .map(|origin| (r.x, r.y, r.scale, origin));
                }
                if response.dragged()
                    && let Some((x, y, scale, origin)) = self.power.reference_drag
                {
                    let Some(pointer) = response.interact_pointer_pos() else {
                        continue;
                    };
                    let delta = (pointer - origin) / pixel_size;
                    let reference = &mut self.doc.references[i];
                    if ui.input(|i| i.modifiers.shift) {
                        let new = (scale + delta.x / r.width as f32).clamp(0.01, 1000.);
                        if let Some(old_y) = &mut reference.options.scale_y {
                            *old_y = (*old_y * new / reference.scale).clamp(0.01, 1000.);
                        }
                        reference.scale = new;
                    } else {
                        reference.x = (x + delta.x).clamp(-16384., 16384.);
                        reference.y = (y + delta.y).clamp(-16384., 16384.);
                    }
                }
                if response.drag_stopped() {
                    self.power.reference_drag = None;
                }
            }
        }
        self.power.reference_textures.retain(|(p, _)| {
            self.doc
                .references
                .iter()
                .any(|r| r.pixels.shares_storage(p))
        });
    }
    pub(super) fn poll_bridge(&mut self, ctx: &egui::Context) {
        if self.power.bridge.is_none() {
            return;
        }
        self.sync_live_revision();
        if let Some(worker) = &self.power.bridge_work {
            match worker.try_recv() {
                Ok(result) => {
                    self.power.bridge_work = None;
                    self.power.bridge_export = None;
                    self.finish_live_request(result);
                    ctx.request_repaint();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.power.bridge_work = None;
                    self.power.bridge_export = None;
                }
            }
        }
        while let Some(mut request) = self
            .power
            .bridge
            .as_ref()
            .and_then(|b| b.requests.try_recv().ok())
        {
            if let Some((snapshot, revision, frame)) = self.begin_live_request(&mut request) {
                let (tx, rx) = std::sync::mpsc::channel();
                if request.operation == "live-export" {
                    self.power.bridge_export = Some(request.export_progress.clone());
                }
                self.power.bridge_work = Some(rx);
                let wake = ctx.clone();
                std::thread::spawn(move || {
                    let prepared = Self::prepare_live_request(snapshot, revision, frame, request);
                    let _ = tx.send(prepared);
                    wake.request_repaint();
                });
                break;
            }
        }
    }

    fn sync_live_revision(&mut self) {
        if self.power.bridge_snapshot.as_ref().is_none_or(|snapshot| {
            snapshot != &self.doc || !snapshot.same_editor_targets(&self.doc)
        }) || self.power.bridge_tab != self.active_tab
        {
            self.power.bridge_snapshot = Some(self.doc.clone());
            self.power.bridge_tab = self.active_tab;
            self.power.bridge_generation += 1;
        }
    }

    fn live_revision(&self) -> String {
        format!(
            "{}:{}",
            self.power.bridge_session, self.power.bridge_generation
        )
    }

    fn live_busy(&self) -> bool {
        self.start.is_some() || self.script_job.is_some() || self.power.reference_drag.is_some()
    }

    fn begin_live_request(
        &mut self,
        request: &mut automation::LiveRequest,
    ) -> Option<(Document, String, usize)> {
        let revision = self.live_revision();
        let op = request.operation.as_str();
        let editing = matches!(
            op,
            "live-edit" | "live-commands" | "live-create" | "live-undo" | "live-redo"
        );
        let saving = matches!(op, "live-save" | "live-save-as");
        if let Err(error) = automation::verify_document_target(&self.doc, &request.arguments) {
            let _ = request.response.send(Err(error));
            return None;
        }
        let error = if request.export_progress.is_cancelled() {
            Some("Live request cancelled; no changes applied")
        } else if Instant::now() >= request.deadline {
            Some("Live request expired; no changes applied")
        } else if (editing || saving) && self.live_busy() {
            Some("Editor is busy; finish the active operation first")
        } else if saving && self.save_job.is_some() {
            Some("A save is already running")
        } else if op == "live-save" && self.path.is_none() {
            Some("The sprite is untitled. Use live-save-as with an output path.")
        } else if (editing
            || saving
            || matches!(op, "live-export" | "live-preview-commands")
            || request.arguments.get("revision").is_some())
            && request.arguments["revision"] != revision
        {
            Some("Stale revision; inspect the open sprite again")
        } else {
            None
        };
        if let Some(error) = error {
            let _ = request.response.send(Err(error.into()));
            return None;
        }
        if op == "live-cel-targets" {
            let result =
                automation::inspect_cel_page(&self.doc, &request.arguments).map(|mut result| {
                    result["revision"] = serde_json::json!(revision);
                    result
                });
            let _ = request.response.send(result);
            return None;
        }
        if matches!(op, "live-inspect" | "live-undo" | "live-redo") {
            let destination = match op {
                "live-undo" => self.undo.last(),
                "live-redo" => self.redo.last(),
                _ => None,
            };
            let doc = destination.map_or(&self.doc, |entry| &entry.doc);
            let reply_revision = if op == "live-inspect" {
                revision
            } else {
                format!("{}:{}", self.power.bridge_session, u64::MAX)
            };
            let mut info = automation::inspect_revision(doc, &reply_revision);
            info["path"] = serde_json::json!(self.path);
            info["session_id"] = serde_json::json!(self.power.bridge_session);
            let result = automation::check_live_response_size(&info).and_then(|()| {
                if request.export_progress.is_cancelled() {
                    return Err("Live request cancelled; no changes applied".into());
                }
                if Instant::now() >= request.deadline {
                    return Err("Live request expired; no changes applied".into());
                }
                match op {
                    "live-undo" => self.undo(),
                    "live-redo" => self.redo(),
                    _ => {}
                }
                self.sync_live_revision();
                info["revision"] = serde_json::json!(self.live_revision());
                Ok(info)
            });
            let _ = request.response.send(result);
            return None;
        }
        if op == "live-save" {
            request.arguments["output"] = serde_json::json!(self.path);
            request.project_root = self.live_save_root.clone();
            request.project_identity = self.live_save_root_identity;
        }
        Some((self.doc.clone(), revision, self.frame))
    }

    fn prepare_live_request(
        doc: Document,
        revision: String,
        frame: usize,
        request: automation::LiveRequest,
    ) -> PreparedLive {
        let args = &request.arguments;
        let result = (|| {
            if Instant::now() >= request.deadline {
                return Err("Live request expired; no changes applied".into());
            }
            let frame = if let Some(value) = args.get("frame") {
                value
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("Invalid frame")?
            } else {
                frame
            };
            let next = match request.operation.as_str() {
                "live-render" => {
                    return automation::render_preview(&doc, frame, args, &revision, 4_194_304)
                        .map(LiveResult::Read);
                }
                "live-export" => {
                    return automation::prepare_live_export_under(
                        &doc,
                        frame,
                        args,
                        &revision,
                        request.project_root.as_deref(),
                        request.project_identity,
                        |done, total| {
                            if Instant::now() >= request.deadline {
                                return Err("Live request expired; no output published".into());
                            }
                            request.export_progress.update(done, total)
                        },
                    )
                    .map(LiveResult::Export);
                }
                "live-save" | "live-save-as" => {
                    let output = args["output"]
                        .as_str()
                        .filter(|p| !p.is_empty())
                        .ok_or("Missing output path")?;
                    let prepared = automation::prepare_live_save_under(
                        &doc,
                        PathBuf::from(output),
                        &revision,
                        request.operation == "live-save" || args["overwrite"] == true,
                        request.project_root.as_deref(),
                        request.project_identity,
                    )?;
                    return Ok(LiveResult::Save(prepared, Box::new(doc)));
                }
                "live-preview-commands" => {
                    return automation::preview_commands(
                        &doc,
                        frame,
                        args,
                        &revision,
                        Some(request.deadline),
                    )
                    .map(LiveResult::Read);
                }
                "live-commands" => {
                    automation::apply_command_batch_until(&doc, args, Some(request.deadline))?
                }
                "live-edit" => automation::apply_pixel_batch(&doc, args)?,
                "live-create" => automation::new_canvas(args)?,
                _ => return Err("Unknown live operation".into()),
            };
            let bounds = if request.operation == "live-create" {
                serde_json::json!([])
            } else {
                automation::changed_cel_bounds(&doc, &next)
            };
            // Reserve the maximum decimal generation length. Finish replaces the
            // revision after a successful commit, without growing the wire reply.
            let (session, _) = revision
                .rsplit_once(':')
                .expect("Live revision has a generation");
            let reply_revision = format!("{session}:{}", u64::MAX);
            let mut value = automation::inspect_revision(&next, &reply_revision);
            value["changed_cels"] = bounds;
            if matches!(request.operation.as_str(), "live-commands" | "live-edit")
                && args["return_preview"] == true
            {
                let mut preview = automation::render_preview(
                    &next,
                    frame.min(next.durations.len() - 1),
                    args,
                    &reply_revision,
                    4_194_304,
                )?;
                let fields = preview
                    .as_object_mut()
                    .expect("Render metadata is an object");
                value["mimeType"] = fields.remove("mimeType").unwrap();
                value["data"] = fields.remove("data").unwrap();
                value["preview"] = preview;
            }
            automation::check_live_response_size(&value)?;
            Ok(LiveResult::Edit(Box::new(next), value))
        })();
        PreparedLive {
            request,
            revision,
            result,
        }
    }

    fn finish_live_request(&mut self, prepared: PreparedLive) {
        let result = prepared.result.and_then(|result| {
            if prepared.request.export_progress.is_cancelled() {
                return Err("Live request cancelled; no changes applied".into());
            }
            if Instant::now() >= prepared.request.deadline {
                return Err("Live request expired; no changes applied".into());
            }
            let (next, mut value) = match result {
                LiveResult::Read(value) => {
                    if prepared.request.operation == "live-preview-commands"
                        && prepared.revision != self.live_revision()
                    {
                        return Err("Stale revision; inspect the open sprite again".into());
                    }
                    return Ok(value);
                }
                LiveResult::Export(export) => {
                    prepared.request.export_progress.begin_commit()?;
                    return export.write();
                }
                LiveResult::Save(save, snapshot) => {
                    if prepared.revision != self.live_revision()
                        || self.live_busy()
                        || self.save_job.is_some()
                    {
                        return Err(
                            "The active sprite changed while saving. Inspect it and retry.".into(),
                        );
                    }
                    let path = save.output.clone();
                    if prepared.request.operation == "live-save"
                        && self.path.as_ref() != Some(&path)
                    {
                        return Err("The current project path changed while saving".into());
                    }
                    let result = save.write()?;
                    self.saved = *snapshot;
                    self.add_recent(path.clone());
                    self.path = Some(path);
                    self.live_save_root = prepared.request.project_root.clone();
                    self.live_save_root_identity = prepared.request.project_identity;
                    self.status = "Saved sprite through MCP".into();
                    return Ok(result);
                }
                LiveResult::Edit(next, value) => (*next, value),
            };
            if prepared.revision != self.live_revision() {
                return Err("Stale revision; inspect the open sprite again".into());
            }
            if self.live_busy() {
                return Err("Editor is busy; finish the active operation first".into());
            }
            if prepared.request.operation == "live-create" {
                let previous = self.doc.clone();
                self.open_document(next, None);
                self.push_undo(previous, "MCP new sprite");
                self.status = "Assistant created an untitled sprite — Ctrl+Z to undo".into();
            } else if next != self.doc || !next.same_editor_targets(&self.doc) {
                self.remember_action("MCP edit");
                self.doc = next;
                self.clamp();
                self.status = "Assistant edited the sprite — Ctrl+Z to undo".into();
            }
            self.sync_live_revision();
            let revision = serde_json::json!(self.live_revision());
            value["revision"] = revision.clone();
            if let Some(preview) = value.get_mut("preview") {
                preview["revision"] = revision;
            }
            Ok(value)
        });
        let _ = prepared.request.response.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_inputs_append_manual_paths_and_invalidate_stale_previews() {
        let dir =
            std::env::temp_dir().join(format!("pixelcraft-atlas-inputs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hero = dir.join("Hero idle.pixel");
        let enemy = dir.join("Enemy.pixel");
        let effect = dir.join("Effect.png");
        for path in [&hero, &enemy, &effect] {
            std::fs::write(path, b"fixture").unwrap();
        }
        let mut app = PixelCraft::with_context(&egui::Context::default(), None);
        app.replace(Document::new(2, 2), Some(hero.clone()));
        app.append_atlas_inputs(std::slice::from_ref(&hero))
            .unwrap();
        app.power.atlas_preview = Some(Err("old preview".into()));
        app.append_atlas_inputs(std::slice::from_ref(&enemy))
            .unwrap();
        assert_eq!(app.power.atlas_files, [hero.clone(), enemy.clone()]);
        assert!(app.power.atlas_preview.is_none());

        app.power.atlas_path = "\"Effect.png\"".into();
        app.add_typed_atlas_input().unwrap();
        assert_eq!(app.power.atlas_files, [hero.clone(), enemy, effect.clone()]);
        assert!(app.power.atlas_path.is_empty());
        app.power.atlas_path = "missing.png".into();
        app.power.atlas_preview = Some(Err("still current".into()));
        assert!(app.add_typed_atlas_input().is_err());
        assert_eq!(app.power.atlas_path, "missing.png");
        assert!(app.power.atlas_preview.is_some());
        app.remove_atlas_input(1);
        assert_eq!(app.power.atlas_files, [hero, effect]);
        assert!(app.power.atlas_preview.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn editor_controls_whole_pixel_wheel_zoom_keeps_mouse_over_the_same_artwork() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(16, 16), None);
        app.power.preferences.whole_pixel_wheel_zoom = true;
        app.initial_view = 0;
        app.zoom = 2.4;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900., 600.))),
            ..Default::default()
        };
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(1.25);
        ctx.run_ui(raw.clone(), |ui| app.canvas(ui))
            .textures_delta
            .clear();
        let view = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new("canvas-bounds")))
            .unwrap();
        let pointer = view.center() + Vec2::new(12., -14.);
        raw.events = vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0., 1.),
                phase: egui::TouchPhase::Move,
                modifiers: Default::default(),
            },
        ];
        let before = (pointer - view.center() - app.pan) / app.zoom;
        ctx.run_ui(raw, |ui| app.canvas(ui)).textures_delta.clear();
        let after = (pointer - view.center() - app.pan) / app.zoom;
        assert!((app.zoom * ctx.pixels_per_point() - 4.).abs() < 0.01);
        assert!((before - after).length() < 0.01);
    }

    #[test]
    fn pixel_perfect_shade_keeps_earlier_stroke_pixels() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(4, 1), None);
        a.doc.palette = vec![[10, 0, 0, 255], [100, 0, 0, 255], [200, 0, 0, 255]];
        for i in 0..4 {
            a.doc.set_pixel(0, 0, i, [100, 0, 0, 255]);
        }
        a.power.mode = BrushMode::Shade;
        a.studio.pixel_perfect = true;
        a.brush = 1;
        a.stroke_base = Some(a.doc.clone());
        a.paint_at((0, 0), [255; 4]);
        a.stroke_line((0, 0), (1, 0), [255; 4]);
        assert_eq!(a.doc.composite(0)[0], [200, 0, 0, 255]);
        a.stroke_line((1, 0), (2, 0), [255; 4]);
        assert_eq!(&a.doc.composite(0)[..3], &[[200, 0, 0, 255]; 3]);
    }
    #[test]
    fn captured_stamp_can_paint_outside_its_source_selection() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(6, 2), None);
        a.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        a.selection = Some((0, 0, 1, 0));
        a.capture_stamp();
        a.doc.set_pixel(0, 0, 4, [0, 255, 0, 255]);
        a.power_paint((4, 0), [255; 4], 1);
        assert_eq!(a.doc.composite(0)[3], [255, 0, 0, 255]);
        assert_eq!(a.doc.composite(0)[4], CLEAR);
    }
    #[test]
    fn transformed_wand_selection_moves_its_mask() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(6, 2), None);
        a.doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        a.doc.set_pixel(0, 0, 2, [255, 0, 0, 255]);
        a.power.contiguous = false;
        a.wand_at((0, 0), SelectionOp::Replace);
        a.transform_selection(pixelcraft::workflows::Transform::Move(1, 1));
        assert!(a.selected(1, 1));
        assert!(a.selected(3, 1));
        assert!(!a.selected(2, 1));
        assert!(!a.selected(0, 0));
    }
    #[test]
    fn workflow_dialog_renders_all_sections() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.power.open = true;
        ctx.memory_mut(|m| m.set_everything_is_visible(true));
        let mut result = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100., 900.))),
                ..Default::default()
            },
            |ui| {
                a.power_dialogs(ui.ctx());
            },
        );
        assert!(!result.shapes.is_empty());
        result.textures_delta.clear();
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;
    use pixelcraft::automation::LiveRequest;
    fn request(
        a: &mut PixelCraft,
        operation: &str,
        arguments: serde_json::Value,
        expired: bool,
    ) -> Result<serde_json::Value, String> {
        request_under(a, operation, arguments, expired, None)
    }

    fn request_under(
        a: &mut PixelCraft,
        operation: &str,
        arguments: serde_json::Value,
        expired: bool,
        project_root: Option<PathBuf>,
    ) -> Result<serde_json::Value, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut request = LiveRequest {
            deadline: if expired {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now() + Duration::from_secs(1)
            },
            operation: operation.into(),
            arguments,
            project_root,
            project_identity: None,
            response: tx,
            export_progress: Default::default(),
        };
        a.sync_live_revision();
        if let Some((doc, revision, frame)) = a.begin_live_request(&mut request) {
            let prepared = PixelCraft::prepare_live_request(doc, revision, frame, request);
            a.finish_live_request(prepared);
        }
        rx.recv().unwrap()
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn live_save_preserves_the_restriction_of_its_saved_as_path() {
        use serde_json::json;
        use std::os::unix::fs::symlink;

        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let workspace = std::env::temp_dir().join(format!(
            "pixelcraft-live-save-redirect-{}",
            std::process::id()
        ));
        let root = workspace.join("project");
        let original = root.join("original");
        let outside = workspace.join("outside");
        let alias = root.join("alias");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink("original", &alias).unwrap();
        let info = request(&mut app, "live-inspect", json!({}), false).unwrap();
        request_under(
            &mut app,
            "live-save-as",
            json!({"revision":info["revision"],"output":alias.join("art.pixel")}),
            false,
            Some(root.canonicalize().unwrap()),
        )
        .unwrap();
        app.open_document(Document::new(1, 1), None);
        app.switch_tab(0);
        std::fs::remove_file(&alias).unwrap();
        symlink(&outside, &alias).unwrap();
        let info = request(&mut app, "live-inspect", json!({}), false).unwrap();
        let outcome = request(
            &mut app,
            "live-save",
            json!({"revision":info["revision"]}),
            false,
        );
        let outside_created = outside.join("art.pixel").exists();
        std::fs::remove_dir_all(&workspace).unwrap();
        assert!(matches!(outcome, Err(error) if error.contains("project root")));
        assert!(!outside_created);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restricted_live_sequence_rejects_redirected_parent_at_publish() {
        use std::os::unix::fs::symlink;
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let workspace =
            std::env::temp_dir().join(format!("pixelcraft-live-redirect-{}", std::process::id()));
        let root = workspace.join("project");
        let original = root.join("original");
        let outside = workspace.join("outside");
        let alias = root.join("alias");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink("original", &alias).unwrap();
        let (response, result) = std::sync::mpsc::channel();
        let mut request = LiveRequest {
            deadline: Instant::now() + Duration::from_secs(3),
            operation: "live-export".into(),
            arguments: serde_json::json!({
                "revision":app.live_revision(), "format":"png-sequence",
                "output":alias.join("frames"), "overwrite":true
            }),
            project_root: Some(root.canonicalize().unwrap()),
            project_identity: None,
            response,
            export_progress: Default::default(),
        };
        let (snapshot, revision, frame) = app.begin_live_request(&mut request).unwrap();
        let prepared = PixelCraft::prepare_live_request(snapshot, revision, frame, request);
        assert!(prepared.result.is_ok());
        std::fs::remove_file(&alias).unwrap();
        symlink(&outside, &alias).unwrap();
        app.finish_live_request(prepared);
        let outcome = result.recv().unwrap();
        let outside_created = outside.join("frames").exists();
        let staged_files = std::fs::read_dir(&original).unwrap().count();
        std::fs::remove_dir_all(&workspace).unwrap();
        assert!(matches!(outcome, Err(error) if error.contains("project root")));
        assert!(!outside_created);
        assert_eq!(staged_files, 0);
    }

    #[test]
    fn live_saves_bind_path_and_preserve_previous_file_on_collision() {
        use serde_json::json;
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 2), None);
        let folder =
            std::env::temp_dir().join(format!("pixelcraft-live-save-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let output = folder.join("saved.pixel");
        let revision = |app: &mut PixelCraft| {
            request(app, "live-inspect", json!({}), false).unwrap()["revision"].clone()
        };
        let rev = revision(&mut a);
        assert!(request(&mut a, "live-save", json!({"revision":rev}), false).is_err());
        assert!(
            request(
                &mut a,
                "live-save-as",
                json!({"revision":"stale","output":output}),
                false
            )
            .is_err()
        );
        request(
            &mut a,
            "live-save-as",
            json!({"revision":rev,"output":output}),
            false,
        )
        .unwrap();
        assert_eq!(a.path.as_ref(), Some(&output));
        assert!(!a.dirty());
        a.doc.set_pixel(0, 0, 0, [10, 20, 30, 255]);
        let rev = revision(&mut a);
        assert!(
            request(
                &mut a,
                "live-save-as",
                json!({"revision":rev,"output":output}),
                false
            )
            .is_err()
        );
        assert_eq!(formats::load(&output).unwrap().layers[0].cels[0][0], CLEAR);
        request(&mut a, "live-save", json!({"revision":rev}), false).unwrap();
        assert_eq!(
            formats::load(&output).unwrap().layers[0].cels[0][0],
            [10, 20, 30, 255]
        );
        assert!(!a.dirty());
        a.doc.set_pixel(0, 0, 0, [40, 50, 60, 255]);
        let rev = revision(&mut a);
        request(
            &mut a,
            "live-save-as",
            json!({"revision":rev,"output":output,"overwrite":true}),
            false,
        )
        .unwrap();
        assert_eq!(
            formats::load(&output).unwrap().layers[0].cels[0][0],
            [40, 50, 60, 255]
        );
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn editor_png_sequence_export_uses_the_document_snapshot_and_scale() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(2, 1);
        doc.add_frame(0, false);
        doc.set_pixel(0, 1, 0, [0, 255, 0, 255]);
        app.replace(doc, None);
        app.action("png-sequence");
        assert_eq!(app.animation_export, Some("png-sequence"));
        app.studio.sheet.scale = 4;
        let output =
            std::env::temp_dir().join(format!("pixelcraft-editor-sequence-{}", std::process::id()));
        app.start_export("png-sequence", output.clone());
        app.doc.set_pixel(0, 1, 0, [255, 0, 0, 255]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.export_job.is_some() && Instant::now() < deadline {
            app.poll_export();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.status.starts_with("Exported "), "{}", app.status);
        let png = image::open(output.join("frame-0002.png"))
            .unwrap()
            .into_rgba8();
        assert_eq!(png.dimensions(), (8, 4));
        assert_eq!(png.get_pixel(3, 3).0, [0, 255, 0, 255]);
        std::fs::remove_file(output.join("frame-0001.png")).unwrap();
        std::fs::remove_file(output.join("frame-0002.png")).unwrap();
        std::fs::remove_dir(output).unwrap();
    }

    #[test]
    fn live_png_sequence_exports_timeline_order_and_selected_layers() {
        use serde_json::json;
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(2, 1);
        doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        doc.add_frame(0, true);
        doc.set_pixel(0, 1, 0, [0, 255, 0, 255]);
        doc.add_layer();
        doc.set_pixel(1, 0, 0, [0, 0, 255, 255]);
        app.replace(doc, None);
        let revision =
            request(&mut app, "live-inspect", json!({}), false).unwrap()["revision"].clone();
        let output =
            std::env::temp_dir().join(format!("pixelcraft-live-sequence-{}", std::process::id()));
        let args = json!({"revision":revision,"output":output,"format":"png-sequence","scale":4,"layers":[0]});
        let result = request(&mut app, "live-export", args.clone(), false).unwrap();
        assert_eq!(result["frames"], 2);
        for (name, color) in [
            ("frame-0001.png", [255, 0, 0, 255]),
            ("frame-0002.png", [0, 255, 0, 255]),
        ] {
            let png = image::open(output.join(name)).unwrap().into_rgba8();
            assert_eq!(png.dimensions(), (8, 4));
            assert_eq!(png.get_pixel(3, 3).0, color);
            assert_eq!(png.get_pixel(4, 3).0, CLEAR);
        }
        assert!(request(&mut app, "live-export", args, false).is_err());
        std::fs::remove_file(output.join("frame-0001.png")).unwrap();
        std::fs::remove_file(output.join("frame-0002.png")).unwrap();
        std::fs::remove_dir(output).unwrap();
    }

    #[test]
    fn live_gif_exports_scaled_frames_with_original_timing() {
        use image::AnimationDecoder;
        use serde_json::json;
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(2, 1);
        doc.set_pixel(0, 0, 0, [255, 0, 0, 255]);
        doc.add_frame(0, true);
        doc.set_pixel(0, 1, 0, [0, 255, 0, 255]);
        doc.add_layer();
        doc.set_pixel(1, 0, 0, [0, 0, 255, 255]);
        doc.durations = vec![90, 120];
        a.replace(doc, None);
        let rev = request(&mut a, "live-inspect", json!({}), false).unwrap()["revision"].clone();
        let output =
            std::env::temp_dir().join(format!("pixelcraft-live-scaled-{}.gif", std::process::id()));
        let result = request(
            &mut a,
            "live-export",
            json!({"revision":rev,"output":output,"scale":4,"layers":[0]}),
            false,
        )
        .unwrap();
        assert_eq!(result["width"], 8);
        let file = std::fs::File::open(&output).unwrap();
        let frames = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file))
            .unwrap()
            .into_frames()
            .collect_frames()
            .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].buffer().dimensions(), (8, 4));
        assert_eq!(frames[0].delay().numer_denom_ms(), (90, 1));
        assert_eq!(frames[1].delay().numer_denom_ms(), (120, 1));
        assert_eq!(frames[0].buffer().get_pixel(3, 3).0, [255, 0, 0, 255]);
        assert_eq!(frames[1].buffer().get_pixel(3, 3).0, [0, 255, 0, 255]);
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn live_structural_commands_and_create_are_single_undo_steps() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 2), None);
        a.path = Some(PathBuf::from("existing.pixel"));
        let original_tab = a.active_tab;
        let before = a.doc.clone();
        a.selection = Some((0, 0, 0, 0));
        a.studio.mask = Some(vec![true, false, false, false]);
        let info = request(&mut a, "live-inspect", serde_json::json!({}), false).unwrap();
        let args = serde_json::json!({"revision":info["revision"],"commands":[
            {"op":"add-group","name":"Effects"},
            {"op":"add-layer","name":"Ink","parent":1},
            {"op":"set-layer","layer":2,"blend":"Overlay"},
            {"op":"fill","layer":2,"frame":0,"x":0,"y":0,"color":[90,80,70,255]},
            {"op":"add-frame","frame":0}
        ]});
        assert!(request(&mut a, "live-commands", args.clone(), true).is_err());
        assert_eq!(a.doc, before);
        let preview = request(&mut a, "live-preview-commands", args.clone(), false).unwrap();
        assert!(preview["data"].is_string());
        assert_eq!(a.doc, before);
        let result = request(&mut a, "live-commands", args.clone(), false).unwrap();
        assert_eq!(a.undo.len(), 1);
        assert_eq!(a.doc.layers.len(), 3);
        assert_eq!(a.doc.durations.len(), 2);
        assert_eq!(result["layers"][2]["blend"], "Overlay");
        assert_eq!(a.doc.layers[2].cels[0][3], [90, 80, 70, 255]);
        request(
            &mut a,
            "live-commands",
            serde_json::json!({
                "revision":result["revision"],
                "commands":[{"op":"set-layer","layer":2,"parent_id":result["layers"][1]["id"]}]
            }),
            false,
        )
        .unwrap();
        assert!(request(&mut a, "live-commands", args, false).is_err());
        a.undo();
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.studio.mask, Some(vec![true, false, false, false]));
        let info = request(&mut a, "live-inspect", serde_json::json!({}), false).unwrap();
        request(
            &mut a,
            "live-create",
            serde_json::json!({"revision":info["revision"],"width":4,"height":3,"background":[11,22,33,128]}),
            false,
        )
        .unwrap();
        assert_eq!((a.doc.width, a.doc.height), (4, 3));
        assert_eq!(a.doc.layers[0].cels[0][0], [11, 22, 33, 128]);
        assert_ne!(a.active_tab, original_tab);
        assert!(a.path.is_none());
        a.undo();
        assert_eq!(a.doc, before);
        assert!(a.path.is_none());
        a.switch_tab(original_tab);
        assert_eq!(a.path, Some(PathBuf::from("existing.pixel")));
        assert_eq!(a.doc, before);
    }
    #[test]
    fn live_edits_are_revision_checked_undoable_and_expire() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(2, 2), None);
        let revision =
            request(&mut a, "live-inspect", serde_json::json!({}), false).unwrap()["revision"]
                .clone();
        let args = serde_json::json!({"revision":revision,"edits":[{"layer":0,"frame":0,"x":1,"y":0,"color":[1,2,3,255]}]});
        let before = a.doc.clone();
        assert!(request(&mut a, "live-edit", args.clone(), true).is_err());
        assert_eq!(before, a.doc);
        assert!(request(&mut a, "live-edit", args.clone(), false).is_ok());
        assert_eq!(a.doc.composite(0)[1], [1, 2, 3, 255]);
        assert!(request(&mut a, "live-edit", args.clone(), false).is_err());
        a.undo();
        assert_eq!(before, a.doc);
        assert!(request(&mut a, "live-edit", args, false).is_err());
        assert!(a.power.bridge.is_none());
    }

    #[test]
    fn oversized_history_replies_leave_the_document_and_history_intact() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut large = Document::new(1, 1);
        large.layers[0].name = "\"".repeat(4_000_000);
        app.replace(large, None);
        app.remember();
        app.doc.layers[0].name = "Small".into();
        app.sync_live_revision();
        let revision = app.live_revision();
        let args = serde_json::json!({"revision":revision});
        let result = request(&mut app, "live-undo", args, false);
        assert!(matches!(result, Err(error) if error.contains("live response limit")));
        assert_eq!(app.live_revision(), revision);
        assert_eq!(app.doc.layers[0].name, "Small");
        assert_eq!(app.undo.len(), 1);
        assert!(app.redo.is_empty());

        app.undo();
        app.redo();
        app.remember();
        app.doc.layers[0].name = "\"".repeat(4_000_000);
        app.undo();
        app.sync_live_revision();
        let revision = app.live_revision();
        let args = serde_json::json!({"revision":revision});
        let result = request(&mut app, "live-redo", args, false);
        assert!(matches!(result, Err(error) if error.contains("live response limit")));
        assert_eq!(app.live_revision(), revision);
        assert_eq!(app.doc.layers[0].name, "Small");
        assert_eq!(app.redo.len(), 1);
    }

    #[test]
    fn oversized_edit_reply_is_rejected_before_commit() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(1, 1);
        doc.layers[0].name = "\"".repeat(3_999_800);
        app.replace(doc, None);
        app.sync_live_revision();
        let revision = app.live_revision();
        let result = request(
            &mut app,
            "live-commands",
            serde_json::json!({"revision":revision,"return_preview":true,"commands":[
                {"op":"stroke","layer":0,"frame":0,"points":[[0,0]],"color":[90,180,230,255]}
            ]}),
            false,
        );
        assert!(matches!(result, Err(error) if error.contains("live response limit")));
        assert_eq!(app.live_revision(), revision);
        assert_eq!(app.doc.composite(0), vec![[0; 4]]);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn prepared_paint_never_overwrites_a_newer_manual_edit() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(2, 1), None);
        let info = request(&mut app, "live-inspect", serde_json::json!({}), false).unwrap();
        let (response, receiver) = std::sync::mpsc::channel();
        let mut edit = LiveRequest {
            operation: "live-commands".into(),
            arguments: serde_json::json!({"revision":info["revision"],"commands":[
                {"op":"blit","layer":0,"frame":0,"x":0,"y":0,"width":1,"height":1,"data":"/////w=="}
            ]}),
            deadline: Instant::now() + Duration::from_secs(2),
            project_root: None,
            project_identity: None,
            response,
            export_progress: Default::default(),
        };
        let (snapshot, revision, frame) = app.begin_live_request(&mut edit).unwrap();
        let prepared = PixelCraft::prepare_live_request(snapshot, revision, frame, edit);
        assert!(prepared.result.is_ok());
        app.remember();
        app.doc.set_pixel(0, 0, 1, [50, 60, 70, 255]);
        app.sync_live_revision();
        app.finish_live_request(prepared);
        assert!(
            receiver
                .recv()
                .unwrap()
                .unwrap_err()
                .contains("Stale revision")
        );
        assert_eq!(
            &app.doc.layers[0].cels[0].to_vec()[..],
            &[[0; 4], [50, 60, 70, 255]]
        );
        assert_eq!(app.undo.len(), 1);
    }

    #[test]
    #[ignore = "Requires a local TCP listener"]
    fn layered_paint_workflow_over_the_live_mcp_bridge() {
        use base64::Engine;
        use serde_json::{Value, json};
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let root = std::env::temp_dir().join(format!("pixelcraft-layered-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let config = root.join("bridge.json");
        app.power.bridge = Some(LiveBridge::start(config.clone()).unwrap());
        let mut call = |name: &str, args: Value| {
            let config = config.clone();
            let request =
                json!({"id":1,"method":"tools/call","params":{"name":name,"arguments":args}});
            let start = Instant::now();
            let client =
                std::thread::spawn(move || automation::rpc(&request, Some(&config)).unwrap());
            while !client.is_finished() {
                assert!(start.elapsed() < Duration::from_secs(12));
                app.poll_bridge(&ctx);
                std::thread::sleep(Duration::from_millis(1));
            }
            let response = client.join().unwrap();
            assert_ne!(response["result"]["isError"], true, "{response}");
            (response["result"]["content"].clone(), start.elapsed())
        };
        fn metadata(content: &Value) -> Value {
            serde_json::from_str(
                content.as_array().unwrap().last().unwrap()["text"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap()
        }
        let (info, _) = call("live-inspect", json!({}));
        let (created, _) = call(
            "live-create",
            json!({"revision":metadata(&info)["revision"],"width":480,"height":270}),
        );
        let names = [
            "Sky",
            "Mountains",
            "Lake",
            "Shore",
            "Meadow",
            "Temple",
            "Trunks",
            "Crimson Leaves",
            "Gold Leaves",
            "Reflections",
            "Sunlight",
            "Flowers",
        ];
        let mut commands = vec![json!({"op":"set-layer","layer":0,"name":names[0]})];
        commands.extend(
            names[1..]
                .iter()
                .map(|name| json!({"op":"add-layer","name":name})),
        );
        let (setup, _) = call(
            "live-commands",
            json!({"revision":metadata(&created)["revision"],"commands":commands}),
        );
        let mut info = metadata(&setup);
        assert_eq!(info["layers"].as_array().unwrap().len(), 12);
        let mut paint_times = Vec::new();
        let mut render_times = Vec::new();
        for pass in 0..6 {
            let color = [20 + pass as u8 * 30, 72, 91, 255];
            let patch = base64::engine::general_purpose::STANDARD.encode(color.repeat(256 * 256));
            let (paint, elapsed) = call(
                "live-commands",
                json!({"revision":info["revision"],"return_preview":true,
                    "region":{"x":0,"y":0,"width":128,"height":128},"commands":[
                    {"op":"blit","layer":2*pass,"frame":0,"x":pass*41,"y":0,"width":256,"height":256,"data":patch},
                    {"op":"stroke","layer":2*pass+1,"frame":0,"points":[[40,20+pass*20],[450,20+pass*20]],"color":[255,172,81,255],"size":2,"spacing":3}
                ]}),
            );
            assert_eq!(paint[0]["type"], "image");
            info = metadata(&paint);
            assert_eq!(info["preview"]["width"], 128);
            assert_eq!(info["preview"]["height"], 128);
            assert_eq!(info["preview"]["revision"], info["revision"]);
            assert_eq!(info["changed_cels"].as_array().unwrap().len(), 2);
            paint_times.push(elapsed.as_millis());
            let (preview, elapsed) = call(
                "live-render",
                json!({"revision":info["revision"],"frame":0}),
            );
            assert_eq!(metadata(&preview)["revision"], info["revision"]);
            assert_eq!(preview[0]["type"], "image");
            render_times.push(elapsed.as_millis());
        }
        let (undo, _) = call("live-undo", json!({"revision":info["revision"]}));
        let (redo, _) = call("live-redo", json!({"revision":metadata(&undo)["revision"]}));
        let revision = metadata(&redo)["revision"].clone();
        let (preview, _) = call("live-render", json!({"revision":revision}));
        let native = base64::engine::general_purpose::STANDARD
            .decode(preview[0]["data"].as_str().unwrap())
            .unwrap();
        let native = image::load_from_memory(&native).unwrap().into_rgba8();
        let output = root.join("scene.png");
        call(
            "live-export",
            json!({"revision":revision,"output":output,"scale":4}),
        );
        let export = image::open(&output).unwrap().into_rgba8();
        assert_eq!(export.dimensions(), (1920, 1080));
        assert!(
            export
                .enumerate_pixels()
                .all(|(x, y, p)| p == native.get_pixel(x / 4, y / 4))
        );
        let sequence = root.join("sequence");
        let (result, _) = call(
            "live-export",
            json!({"revision":revision,"output":sequence,"format":"png-sequence","scale":4}),
        );
        assert_eq!(metadata(&result)["frames"], 1);
        let first = sequence.join("frame-0001.png");
        assert_eq!(image::open(&first).unwrap().into_rgba8(), export);
        std::fs::remove_file(first).unwrap();
        std::fs::remove_dir(sequence).unwrap();
        let project = root.join("scene.pixel");
        call(
            "live-save-as",
            json!({"revision":revision,"output":project}),
        );
        call("live-save", json!({"revision":revision}));
        let reopened = formats::load(&project).unwrap();
        assert_eq!(reopened.layers.len(), 12);
        assert_eq!(
            reopened.composite(0).as_slice().as_flattened(),
            native.as_raw()
        );
        eprintln!(
            "TCP MCP paint round trips (ms): {paint_times:?}; preview round trips (ms): {render_times:?}"
        );
        drop(app);
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(project).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)]
mod reference_drag_regressions {
    use super::*;
    #[test]
    fn regression_reference_drag_uses_total_movement_for_move_and_scale() {
        for (shift, locked) in [(false, false), (true, false), (false, true)] {
            let ctx = egui::Context::default();
            let mut a = PixelCraft::with_context(&ctx, None);
            a.doc.references.push(ReferenceImage {
                behind: false,
                floating: false,
                name: "guide".into(),
                width: 100,
                height: 100,
                pixels: vec![[255; 4]; 10000].into(),
                x: 0.,
                y: 0.,
                scale: 1.,
                opacity: 255,
                visible: true,
                options: ReferenceOptions {
                    locked,
                    ..Default::default()
                },
            });
            a.zoom = 2.;
            a.power.reference_edit = Some(0);
            let modifiers = egui::Modifiers {
                shift,
                ..Default::default()
            };
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.))),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        a.draw_references(ui, Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.)));
                    },
                );
                output.textures_delta.clear();
            };
            frame(vec![egui::Event::ModifiersChanged(modifiers)]);
            frame(vec![
                egui::Event::PointerMoved(Pos2::new(30., 30.)),
                egui::Event::PointerButton {
                    pos: Pos2::new(30., 30.),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                },
            ]);
            frame(vec![egui::Event::PointerMoved(Pos2::new(40., 35.))]);
            frame(vec![egui::Event::PointerMoved(Pos2::new(55., 45.))]);
            let r = &a.doc.references[0];
            if locked {
                assert_eq!((r.x, r.y, r.scale), (0., 0., 1.));
                assert!(a.undo.is_empty());
            } else if shift {
                assert!((r.scale - 1.125).abs() < 0.0001, "scale was {}", r.scale);
            } else {
                assert_eq!((r.x, r.y), (12.5, 7.5));
            }
        }
    }

    #[test]
    fn linked_reference_missing_placeholder_excludes_snapshot_from_picker() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-reference-link-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("guide.png");
        image::RgbaImage::from_pixel(12, 12, image::Rgba([80, 90, 110, 255]))
            .save(&path)
            .unwrap();
        let mut reference = import_reference(&path).unwrap();
        reference.opacity = 255;
        reference.options.source = Some(PathBuf::from("guide.png"));
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        let mut doc = Document::new(12, 12);
        doc.references.push(reference);
        let project = root.join("drawing.pixel");
        formats::save(&project, &doc).unwrap();
        let reopened = formats::load(&project).unwrap();
        app.replace(reopened, Some(project.clone()));
        app.power.preferences.eyedropper.source = EyedropperSource::References;
        let probe = |app: &mut PixelCraft| {
            app.power.reference_check_due = Instant::now();
            app.poll_reference_sources(&ctx);
            app.power.reference_availability = app
                .power
                .reference_check
                .take()
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
        };
        probe(&mut app);
        app.pick_color((0, 0), false);
        assert_eq!(app.fg, [80, 90, 110, 255]);

        std::fs::remove_file(&path).unwrap();
        probe(&mut app);
        app.fg = [1, 2, 3, 255];
        app.pick_color((0, 0), false);
        assert_eq!(app.fg, [1, 2, 3, 255]);
        assert!(!app.dirty());
        app.zoom = 16.;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.))),
                ..Default::default()
            },
            |ui| {
                app.draw_references(ui, Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.)));
            },
        );
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Missing linked image")));
        assert!(!linked_reference_exists(&root, Path::new("../guide.png")));
        assert!(!linked_reference_exists(
            &root,
            Path::new("drawing.pixel:stream")
        ));
        std::fs::remove_file(project).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)]
mod density_tests {
    use super::*;
    #[test]
    fn density_preferences_migrate_and_comfortable_keeps_readable_type() {
        let old: Preferences = serde_json::from_str(r#"{"interface_scale":1.0}"#).unwrap();
        assert!(!old.comfortable);
        assert_eq!(old.interface_scale, 1.);
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.power.preferences.comfortable = true;
        let bytes = serde_json::to_vec(&app.power.preferences).unwrap();
        assert!(
            serde_json::from_slice::<Preferences>(&bytes)
                .unwrap()
                .comfortable
        );
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.render(ui);
            assert_eq!(ui.spacing().interact_size.y, 32.);
            assert_eq!(ui.style().text_styles[&egui::TextStyle::Body].size, 14.);
        });
        out.textures_delta.clear();
    }
}

#[cfg(test)]
mod application_preferences_tests {
    use super::*;

    #[test]
    fn legacy_preferences_get_new_document_defaults_and_disable_background_downloads() {
        let prefs: Preferences = serde_json::from_str(r#"{"interface_scale":1.25}"#).unwrap();
        assert_eq!(prefs.new_sprite_size(), (32, 32));
        assert!(!prefs.default_grid);
        assert!(!prefs.background_updates);
        assert_eq!(prefs.interface_scale, 1.25);
    }

    #[cfg(all(
        target_arch = "x86_64",
        any(target_os = "linux", target_os = "windows")
    ))]
    #[test]
    fn background_updates_are_opt_in_and_try_each_detected_release_once() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.power.preferences.background_updates = false;
        app.latest_release = Some(Release::parse(
            &serde_json::json!({
                "tag_name": "v99.0.0",
                "assets": [
                    {"name": "PixelCraft-v99.0.0-x86_64.AppImage", "browser_download_url": "app"},
                    {"name": "PixelCraft-v99.0.0-x86_64-Setup.exe", "browser_download_url": "exe"}
                ],
            }).to_string(),
        ).unwrap());
        assert!(app.update_to_download(true).is_none());
        assert!(app.update_to_download(false).is_some());
        app.power.preferences.background_updates = true;
        assert!(app.update_to_download(true).is_some());
        app.background_update_attempt = Some("v99.0.0".into());
        assert!(app.update_to_download(true).is_none());
        assert!(app.update_to_download(false).is_some());
    }

    #[test]
    fn settings_reset_keeps_recent_projects_and_restores_new_sprite_defaults() {
        let paths = vec![PathBuf::from("art/sprite.pixel")];
        let prefs = Preferences {
            new_sprite_width: 60,
            new_sprite_height: 48,
            default_grid: true,
            background_updates: true,
            recent: paths.clone(),
            ..Default::default()
        };
        let mut prefs: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        assert_eq!(prefs.new_sprite_size(), (60, 48));
        assert!(prefs.default_grid && prefs.background_updates);
        prefs.reset_settings();
        assert_eq!(prefs.new_sprite_size(), (32, 32));
        assert!(!prefs.default_grid && !prefs.background_updates);
        assert_eq!(prefs.recent, paths);
    }

    #[test]
    fn recent_paths_use_folder_hints_only_when_names_match() {
        let paths = [
            PathBuf::from("art/heroes/idle.pixel"),
            PathBuf::from("art/items/idle.pixel"),
            PathBuf::from("art/items/run.pixel"),
        ];
        assert_eq!(
            recent_file_label(&paths[0], &paths),
            "idle.pixel (heroes) · Missing"
        );
        assert_eq!(
            recent_file_label(&paths[1], &paths),
            "idle.pixel (items) · Missing"
        );
        assert_eq!(recent_file_label(&paths[2], &paths), "run.pixel · Missing");
    }
}

#[cfg(test)]
mod smarter_fill_tests {
    use super::*;

    #[test]
    fn fill_tolerance_and_global_mode_respect_the_selection_without_changing_wand_settings() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(5, 1), None);
        let near = [21, 30, 40, 255];
        let exact = [20, 30, 40, 255];
        let other = [80, 30, 40, 255];
        let fill = [250, 0, 0, 255];
        app.doc.layers[0].cels[0] = vec![exact, near, other, near, exact].into();
        app.power.fill_tolerance = 2;
        app.power.fill_global = true;
        app.selection = Some((0, 0, 3, 0));
        app.remember();
        app.power_fill((0, 0), fill);
        assert_eq!(app.doc.composite(0), vec![fill, fill, other, fill, exact]);
        app.undo();
        assert_eq!(app.doc.composite(0), vec![exact, near, other, near, exact]);
        assert_eq!(app.power.tolerance, 0);
        assert!(app.power.contiguous);
    }

    #[test]
    fn tile_picker_and_fill_use_cell_geometry_and_preserve_tileset_artwork() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.replace(Document::new(8, 2), None);
        app.doc.tilesets.push(Tileset {
            name: "Tiles".into(),
            width: 2,
            height: 2,
            tiles: vec![
                vec![[10, 20, 30, 255]; 4].into(),
                vec![[40, 50, 60, 255]; 4].into(),
            ],
        });
        app.doc.add_tile_layer(0).unwrap();
        app.layer = 1;
        let flipped_tile = 2 | 0x8000_0000;
        app.doc.layers[1].tile_cels[0] = vec![1, 1, flipped_tile, 1];
        assert!(app.pick_tile((4, 0)));
        assert_eq!(app.selected_tile, flipped_tile);
        assert_eq!(app.status, "Picked tile 2");
        app.tool = Tool::Pencil;
        app.remember();
        app.paint_at((6, 0), CLEAR);
        assert_eq!(app.doc.layers[1].tile_cels[0][3], flipped_tile);
        app.undo();
        app.remember();
        app.power_fill((0, 0), CLEAR);
        assert_eq!(
            app.doc.layers[1].tile_cels[0],
            vec![flipped_tile, flipped_tile, flipped_tile, 1]
        );
        app.undo();
        app.power.fill_global = true;
        app.tile_pixel_mode = Some(TilePixelEdit::Shared);
        app.power_fill((0, 0), CLEAR);
        assert_eq!(app.doc.layers[1].tile_cels[0], vec![1, 1, flipped_tile, 1]);
        app.tile_pixel_mode = None;
        app.selection = Some((1, 0, 1, 0));
        app.power_fill((1, 0), CLEAR);
        assert_eq!(
            app.doc.layers[1].tile_cels[0],
            vec![flipped_tile, 1, flipped_tile, 1]
        );
        app.selection = None;
        app.power_fill((2, 0), CLEAR);
        assert_eq!(app.doc.layers[1].tile_cels[0], vec![flipped_tile; 4]);
        assert_eq!(app.doc.tilesets[0].tiles[0][0], [10, 20, 30, 255]);
        assert_eq!(app.doc.tilesets[0].tiles[1][0], [40, 50, 60, 255]);
    }
}

#[cfg(test)]
mod brush_library_tests {
    use super::*;

    #[test]
    fn image_brush_phase_follows_stamp_source_or_stroke() {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-image-brush.png")));
        let [a, b, c] = [[232, 45, 18, 255], [30, 140, 81, 255], [44, 110, 237, 255]];
        for (alignment, expected) in [
            (ImageBrushAlignment::Stamp, [a, a, b, c]),
            (ImageBrushAlignment::Source, [b, c, a, b]),
            (ImageBrushAlignment::Stroke, [a, b, c, a]),
        ] {
            app.replace(Document::new(9, 3), None);
            app.power.mode = BrushMode::Selection;
            app.power.stamp = Some((3, 1, vec![a, b, c]));
            app.power.image_brush_alignment = alignment;
            app.power.image_brush_origin = (0, 1);
            app.start = Some((5, 1));
            app.power_paint((5, 1), CLEAR, 1);
            app.power_paint((6, 1), CLEAR, 1);
            assert_eq!(
                app.doc.layers[0].cels[0]
                    .iter()
                    .skip(13)
                    .take(4)
                    .copied()
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn brush_presets_and_named_ramps_round_trip_and_restore_painting() {
        let ctx = egui::Context::default();
        let new_app =
            || PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-brush-presets.png")));
        let (low, mid, high, unrelated) = (
            [32, 40, 50, 255],
            [65, 88, 95, 255],
            [146, 175, 180, 255],
            [250, 15, 40, 255],
        );
        let mut app = new_app();
        app.replace(Document::new(8, 3), None);
        app.brush = 7;
        app.doc.brush_shape = BrushShape::Diamond;
        app.power.mode = BrushMode::Selection;
        app.power.stamp = Some((2, 1, vec![high, CLEAR]));
        app.power.image_brush_alignment = ImageBrushAlignment::Source;
        app.power.image_brush_origin = (0, 0);
        let dynamics = BrushDynamics {
            stabilizer: 7,
            pressure_size: true,
            pressure_opacity: true,
        };
        app.set_brush_dynamics(dynamics);
        app.set_tool_ink(PaintInk::CopyColor);
        app.capture_brush_preset("Leaf").unwrap();
        app.doc.palette = vec![unrelated, low, mid, high];
        app.capture_shading_ramp("Water", 1, 3).unwrap();

        let directory =
            std::env::temp_dir().join(format!("pixelcraft-brush-library-{}", std::process::id()));
        let path = directory.join("brush-library.json");
        app.power.brush_library.write(&path).unwrap();
        let mut next = new_app();
        next.replace(Document::new(8, 3), None);
        next.power.brush_library = BrushLibrary::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&directory).unwrap();

        next.apply_brush_preset("Leaf").unwrap();
        assert_eq!(next.brush, 7);
        assert_eq!(next.brush_dynamics(), dynamics);
        assert_eq!(next.doc.brush_shape, BrushShape::Diamond);
        assert_eq!(
            next.power.image_brush_alignment,
            ImageBrushAlignment::Source
        );
        assert_eq!(next.tool_ink(), PaintInk::CopyColor);
        next.doc.set_pixel(0, 0, 1, low);
        next.power_paint((1, 0), unrelated, next.brush);
        assert_eq!(next.doc.layers[0].cels[0][0], high);
        assert_eq!(next.doc.layers[0].cels[0][1], CLEAR);

        next.doc.palette = vec![high, unrelated, low, mid];
        next.select_shading_ramp(Some("Water")).unwrap();
        next.doc.set_pixel(0, 0, 2, low);
        next.doc.set_pixel(0, 0, 3, unrelated);
        next.power_paint((2, 0), unrelated, 1);
        next.power_paint((2, 0), unrelated, 1);
        next.power_paint((3, 0), unrelated, 1);
        assert_eq!(next.doc.layers[0].cels[0][2], mid);
        assert_eq!(next.doc.layers[0].cels[0][3], unrelated);
        next.power.shade_seen.clear();
        next.power.reverse = true;
        next.power_paint((2, 0), unrelated, 1);
        assert_eq!(next.doc.layers[0].cels[0][2], low);
    }
}
