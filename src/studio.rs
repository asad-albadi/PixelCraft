use super::*;
use pixelcraft::{
    interchange,
    workflows::{self, SheetOptions, Transform},
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

mod recovery_images;

struct RecoveryWrite {
    state: Workspace,
    reset_history: bool,
    result: Result<(), String>,
}

pub(super) struct Studio {
    pub recovery_enabled: bool,
    pub(super) collapsed_groups: std::collections::HashSet<String>,
    pub open: bool,
    pub sheet_open: bool,
    pub tiles_open: bool,
    tileset_atlas_export: Option<TilesetAtlasDialog>,
    tile_browser_set: usize,
    show_tile_indices: bool,
    pub seamless: WrapAxes,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub pixel_perfect: bool,
    pub pressure: bool,
    pub force: f32,
    pub sheet: SheetOptions,
    sheet_use_timeline_layers: bool,
    sheet_slice: Option<String>,
    pub polygon: Vec<(i32, i32)>,
    pub mask: Option<Vec<bool>>,
    pub move_mask: Option<Vec<bool>>,
    pub stroke: Vec<(i32, i32)>,
    pub selection_origin: Option<(i32, i32, i32, i32)>,
    dx: i32,
    dy: i32,
    w: usize,
    h: usize,
    ramp_steps: usize,
    outline_corners: bool,
    rotation_angle: f64,
    rotation_pivot: Option<[f64; 2]>,
    rotation_rotsprite: bool,
    rotation_preview: bool,
    rotation_texture: Option<RotationPreview>,
    canvas_resize: Option<CanvasResize>,
    tile_conversion: Option<TileConversionDialog>,
    sprite_job: Option<SpriteJob>,
    adjustment_input: Adjustment,
    recovery_root: PathBuf,
    last_save: Instant,
    last_session: Option<Workspace>,
    recovery_job: Option<std::thread::JoinHandle<RecoveryWrite>>,
    restore_job: Option<std::thread::JoinHandle<Result<Option<Workspace>, String>>>,
    restore_candidate: Option<Workspace>,
    salvage_job: Option<std::thread::JoinHandle<Result<Document, String>>>,
    recovery_decided: bool,
    reset_recovery_history: bool,
    restore_started: Instant,
    pub recovery_status: String,
}

struct CanvasResize {
    document: String,
    resample: bool,
    size: [usize; 2],
    anchor: usize,
    offset: [i32; 2],
}

#[derive(Clone, Copy, Default)]
struct TilesetAtlasDialog {
    visible: bool,
    columns: usize,
}

struct TileConversionDialog {
    layer: String,
    width: usize,
    height: usize,
}

struct SpriteJob {
    source: Document,
    title: &'static str,
    worker: std::thread::JoinHandle<Result<SpriteJobOutput, String>>,
}

struct SpriteJobOutput {
    doc: Document,
    layer: Option<usize>,
}

const CANVAS_ANCHORS: [&str; 10] = [
    "Top left",
    "Top",
    "Top right",
    "Left",
    "Center",
    "Right",
    "Bottom left",
    "Bottom",
    "Bottom right",
    "Custom offset",
];

impl CanvasResize {
    fn offset_from(&self, old: [usize; 2]) -> [i32; 2] {
        if self.anchor == 9 {
            return self.offset;
        }
        let [horizontal, vertical] = [(self.anchor % 3) as i32, (self.anchor / 3) as i32];
        let dx = self.size[0] as i32 - old[0] as i32;
        let dy = self.size[1] as i32 - old[1] as i32;
        [
            (dx * horizontal).div_euclid(2),
            (dy * vertical).div_euclid(2),
        ]
    }
}

struct RotationPreview {
    source: Document,
    layer: usize,
    frame: usize,
    bounds: [i32; 4],
    mask: Option<Vec<bool>>,
    polygon: Vec<(i32, i32)>,
    degrees: f64,
    pivot: [f64; 2],
    texture: egui::TextureHandle,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            recovery_enabled: false,
            collapsed_groups: std::collections::HashSet::new(),
            open: false,
            sheet_open: false,
            tiles_open: false,
            tileset_atlas_export: None,
            tile_browser_set: 0,
            show_tile_indices: false,
            seamless: WrapAxes::NONE,
            mirror_x: false,
            mirror_y: false,
            pixel_perfect: false,
            pressure: false,
            force: 1.,
            sheet: SheetOptions::default(),
            sheet_use_timeline_layers: false,
            sheet_slice: None,
            polygon: vec![],
            mask: None,
            move_mask: None,
            stroke: vec![],
            selection_origin: None,
            dx: 1,
            dy: 0,
            w: 16,
            h: 16,
            ramp_steps: 5,
            outline_corners: false,
            rotation_angle: 0.,
            rotation_pivot: None,
            rotation_rotsprite: false,
            rotation_preview: false,
            rotation_texture: None,
            canvas_resize: None,
            tile_conversion: None,
            sprite_job: None,
            adjustment_input: Adjustment::Levels {
                black: 0,
                white: 255,
            },
            recovery_root: extensions::config_directory(),
            last_save: Instant::now(),
            last_session: None,
            recovery_job: None,
            restore_job: None,
            restore_candidate: None,
            salvage_job: None,
            recovery_decided: false,
            reset_recovery_history: false,
            restore_started: Instant::now(),
            recovery_status: "Autosave every 15 seconds".into(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct StoredTab {
    #[serde(with = "formats::recovery_document")]
    doc: Document,
    #[serde(with = "formats::recovery_document")]
    saved: Document,
    path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    raw_recovery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live_save_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    live_save_root_identity: Option<(u64, u64)>,
    layer: usize,
    frame: usize,
    zoom: f32,
    pan: [f32; 2],
    selection: Option<(i32, i32, i32, i32)>,
    polygon: Vec<(i32, i32)>,
    #[serde(default)]
    mask: Option<Vec<bool>>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Workspace {
    version: u32,
    tabs: Vec<StoredTab>,
    active: usize,
    colors: Vec<(String, Pixel)>,
    color: usize,
}
impl Workspace {
    fn remove_tab(&mut self, index: usize) -> Option<StoredTab> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        self.active =
            (self.active - usize::from(index < self.active)).min(self.tabs.len().saturating_sub(1));
        Some(tab)
    }
}

impl PixelCraft {
    fn workspace(&self) -> Workspace {
        let tabs = self
            .tabs
            .iter()
            .map(|tab| {
                if let Some(t) = tab {
                    StoredTab {
                        doc: t.doc.clone(),
                        saved: t.saved.clone(),
                        path: t.path.clone(),
                        raw_recovery: false,
                        live_save_root: t.live_save_root.clone(),
                        live_save_root_identity: t.live_save_root_identity,
                        layer: t.layer,
                        frame: t.frame,
                        zoom: t.zoom,
                        pan: [t.pan.x, t.pan.y],
                        selection: t.selection,
                        polygon: t.polygon.clone(),
                        mask: t.mask.clone(),
                    }
                } else {
                    StoredTab {
                        doc: self.doc.clone(),
                        saved: self.saved.clone(),
                        path: self.path.clone(),
                        raw_recovery: false,
                        live_save_root: self.live_save_root.clone(),
                        live_save_root_identity: self.live_save_root_identity,
                        layer: self.layer,
                        frame: self.frame,
                        zoom: self.zoom,
                        pan: [self.pan.x, self.pan.y],
                        selection: self.selection,
                        polygon: self.studio.polygon.clone(),
                        mask: self.studio.mask.clone(),
                    }
                }
            })
            .collect();
        Workspace {
            version: 1,
            tabs,
            active: self.active_tab,
            colors: self.named_colors.clone(),
            color: self.active_color,
        }
    }
    pub(super) fn autosave(&mut self, force: bool, ctx: &egui::Context) {
        if self
            .studio
            .recovery_job
            .as_ref()
            .is_some_and(|job| force || job.is_finished())
        {
            let job = self.studio.recovery_job.take().unwrap();
            match job.join() {
                Ok(write) => self.record_recovery(write),
                Err(_) => {
                    self.studio.recovery_status = "Autosave worker stopped unexpectedly".into()
                }
            }
        }
        if self.shot.is_some() || !self.studio.recovery_enabled || !self.power.autosave_enabled() {
            return;
        }
        if self.studio.recovery_job.is_some()
            || self.studio.restore_job.is_some()
            || (self.studio.restore_candidate.is_some() && !self.studio.recovery_decided)
        {
            return;
        }
        if !force
            && !self.studio.reset_recovery_history
            && self.studio.last_save.elapsed() < Duration::from_secs(15)
        {
            return;
        }
        self.studio.last_save = Instant::now();
        let mut state = self.workspace();
        if let Some(pending) = &self.studio.restore_candidate {
            state.tabs.extend(pending.tabs.iter().cloned());
        }
        if !self.studio.reset_recovery_history && self.studio.last_session.as_ref() == Some(&state)
        {
            return;
        }
        let root = self.studio.recovery_root.clone();
        let reset_history = std::mem::take(&mut self.studio.reset_recovery_history);
        let save = move || {
            let result = write_recovery_snapshot(&root, &state, reset_history);
            RecoveryWrite {
                state,
                reset_history,
                result,
            }
        };
        if force {
            self.record_recovery(save());
        } else {
            let ctx = ctx.clone();
            self.studio.recovery_job = Some(std::thread::spawn(move || {
                let write = save();
                ctx.request_repaint();
                write
            }));
        }
    }
    fn record_recovery(&mut self, write: RecoveryWrite) {
        match write.result {
            Ok(()) => {
                self.studio.last_session = Some(write.state);
                self.studio.recovery_status = "Open tabs and unsaved work backed up".into();
            }
            Err(e) => {
                self.studio.reset_recovery_history |= write.reset_history;
                self.studio.recovery_status = format!("Autosave failed: {e}");
            }
        }
    }
    pub(super) fn restore_workspace(&mut self) {
        if self.shot.is_some()
            || !self.studio.recovery_enabled
            || !self.power.autosave_enabled()
            || self.studio.restore_job.is_some()
            || self.studio.restore_candidate.is_some()
        {
            return;
        }
        let root = self.studio.recovery_root.clone();
        self.studio.restore_started = Instant::now();
        self.studio.restore_job = Some(std::thread::spawn(move || read_recovery(&root)));
    }

    fn poll_workspace_restore(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.studio.restore_job else {
            return;
        };
        if !job.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(25));
            return;
        }
        let job = self.studio.restore_job.take().unwrap();
        match job.join() {
            Ok(Ok(candidate)) => self.studio.restore_candidate = candidate,
            Ok(Err(error)) => self.status = format!("Could not restore workspace: {error}"),
            Err(_) => self.status = "Workspace recovery worker stopped unexpectedly".into(),
        }
    }

    fn start_recovery_salvage(&mut self, index: usize, layout: recovery_images::Layout) {
        if self.studio.salvage_job.is_some() {
            return;
        }
        let Some(tab) = self
            .studio
            .restore_candidate
            .as_ref()
            .and_then(|state| state.tabs.get(index))
        else {
            return;
        };
        let doc = tab.doc.clone();
        self.studio.salvage_job = Some(std::thread::spawn(move || {
            recovery_images::salvage(&doc, layout)
        }));
        self.status = "Recovering raw images…".into();
    }

    fn poll_recovery_salvage(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.studio.salvage_job else {
            return;
        };
        if !job.is_finished() || self.script_job.is_some() || self.save_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(25));
            return;
        }
        let job = self.studio.salvage_job.take().unwrap();
        match job.join() {
            Ok(Ok(doc)) => {
                let baseline = Document::new(doc.width, doc.height);
                self.open_document(doc, None);
                self.saved = baseline;
                self.status = "Opened recovered images as a new sprite".into();
            }
            Ok(Err(error)) => self.status = format!("Could not recover images: {error}"),
            Err(_) => self.status = "Image recovery worker stopped unexpectedly".into(),
        }
    }

    pub(super) fn workspace_recovery_dialog(&mut self, ctx: &egui::Context) {
        self.poll_workspace_restore(ctx);
        self.poll_recovery_salvage(ctx);
        let Some(state) = &self.studio.restore_candidate else {
            if self.studio.restore_job.is_some()
                && self.studio.restore_started.elapsed() >= Duration::from_millis(250)
            {
                egui::Window::new("Workspace recovery")
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Reading saved workspace…");
                        });
                    });
            }
            return;
        };
        let mut restore = None;
        let mut discard = None;
        let mut salvage = None;
        egui::Window::new("Workspace recovery")
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Choose which backups to restore or discard.");
                egui::ScrollArea::vertical()
                    .max_height(280.)
                    .show(ui, |ui| {
                        for (index, tab) in state.tabs.iter().enumerate() {
                            ui.push_id(index, |ui| {
                                ui.horizontal(|ui| {
                                    let display_name = tab
                                        .path
                                        .as_ref()
                                        .and_then(|path| path.file_name())
                                        .map_or_else(
                                            || format!("Untitled {}", index + 1),
                                            |name| name.to_string_lossy().into_owned(),
                                        );
                                    let label = ui.label(display_name);
                                    if tab.raw_recovery {
                                        ui.label("Images only");
                                    }
                                    if let Some(path) = &tab.path {
                                        label.on_hover_text(path.display().to_string());
                                    }
                                    ui.add_enabled_ui(
                                        !tab.raw_recovery
                                            && self.script_job.is_none()
                                            && self.save_job.is_none(),
                                        |ui| {
                                            if ui.pc_button("Restore").clicked() {
                                                restore = Some(Some(index));
                                            }
                                        },
                                    );
                                    ui.add_enabled_ui(self.studio.salvage_job.is_none(), |ui| {
                                        ui.menu_button("Recover images", |ui| {
                                            if ui.pc_button("As layers").clicked() {
                                                salvage =
                                                    Some((index, recovery_images::Layout::Layers));
                                                ui.close();
                                            }
                                            if ui.pc_button("As frames").clicked() {
                                                salvage =
                                                    Some((index, recovery_images::Layout::Frames));
                                                ui.close();
                                            }
                                        });
                                    });
                                    if ui.pc_button("Discard").clicked() {
                                        discard = Some(Some(index));
                                    }
                                });
                            });
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(
                        state.tabs.iter().any(|tab| !tab.raw_recovery)
                            && self.script_job.is_none()
                            && self.save_job.is_none(),
                        |ui| {
                            if ui.pc_button("Restore all").clicked() {
                                restore = Some(None);
                            }
                        },
                    );
                    if ui.pc_button("Discard all").clicked() {
                        discard = Some(None);
                    }
                });
            });
        if let Some(index) = restore {
            self.apply_workspace_restore(index);
        } else if let Some(index) = discard {
            self.discard_workspace_backup(index, ctx);
        } else if let Some((index, layout)) = salvage {
            self.start_recovery_salvage(index, layout);
        }
    }

    fn discard_workspace_backup(&mut self, index: Option<usize>, ctx: &egui::Context) {
        if let Some(index) = index {
            let Some(state) = &mut self.studio.restore_candidate else {
                return;
            };
            if state.remove_tab(index).is_none() {
                return;
            }
            if state.tabs.is_empty() {
                self.studio.restore_candidate = None;
            }
            self.studio.recovery_decided = true;
            self.studio.reset_recovery_history = true;
            self.studio.last_save = Instant::now();
            self.autosave(false, ctx);
            self.status = "Removed document from recovery list".into();
            return;
        }
        if self.studio.recovery_job.is_some() {
            self.status = "Wait for autosave before discarding all backups".into();
            return;
        }
        match discard_recovery(&self.studio.recovery_root) {
            Ok(()) => {
                self.studio.restore_candidate = None;
                self.studio.reset_recovery_history = false;
                self.studio.last_session = None;
                self.status = "Workspace backup discarded".into();
            }
            Err(error) => self.status = format!("Could not discard workspace backup: {error}"),
        }
    }

    fn apply_workspace_restore(&mut self, index: Option<usize>) {
        let Some(mut state) = self.studio.restore_candidate.take() else {
            return;
        };
        if let Some(index) = index {
            let Some(tab) = state
                .tabs
                .get(index)
                .filter(|tab| !tab.raw_recovery)
                .cloned()
            else {
                self.studio.restore_candidate = Some(state);
                return;
            };
            state.remove_tab(index);
            let single = Workspace {
                tabs: vec![tab],
                active: 0,
                colors: state.colors.clone(),
                ..state
            };
            if !state.tabs.is_empty() {
                self.studio.restore_candidate = Some(state);
            }
            self.studio.recovery_decided = true;
            state = single;
        } else if state.tabs.iter().any(|tab| tab.raw_recovery) {
            let earlier_raw = state.tabs[..state.active]
                .iter()
                .filter(|tab| tab.raw_recovery)
                .count();
            let (pending, intact): (Vec<_>, Vec<_>) =
                state.tabs.into_iter().partition(|tab| tab.raw_recovery);
            state.tabs = intact;
            state.active = (state.active - earlier_raw).min(state.tabs.len().saturating_sub(1));
            self.studio.restore_candidate = Some(Workspace {
                version: state.version,
                tabs: pending,
                active: 0,
                colors: state.colors.clone(),
                color: state.color,
            });
            self.studio.recovery_decided = true;
            if state.tabs.is_empty() {
                return;
            }
        }
        let empty_workspace = self.tabs.len() == 1 && self.path.is_none() && !self.dirty();
        let restored_count = state.tabs.len();
        let open_count = self.tabs.len();
        let restored_active = open_count + state.active;
        for t in &state.tabs {
            self.tabs.push(Some(SpriteTab {
                doc: t.doc.clone(),
                saved: t.saved.clone(),
                path: t.path.clone(),
                live_save_root: t.live_save_root.clone(),
                live_save_root_identity: t.live_save_root_identity,
                layer: t.layer,
                frame: t.frame,
                selected_tile: 1,
                undo: vec![],
                redo: vec![],
                zoom: t.zoom,
                pan: Vec2::new(t.pan[0], t.pan[1]),
                split_canvas: None,
                active_canvas: CanvasPane::Primary,
                selection: t.selection,
                polygon: t.polygon.clone(),
                mask: t.mask.clone(),
                palette_index: 0,
                active_tag: None,
                timeline_anchor: None,
                selected_cels: None,
                timeline_frames: TimelineHeaders::default(),
                timeline_layers: TimelineHeaders::default(),
            }));
        }
        self.switch_tab(restored_active);
        self.initial_view = 0;
        if empty_workspace {
            self.tabs.remove(0);
            self.active_tab -= 1;
            self.named_colors.clone_from(&state.colors);
            self.active_color = state.color;
            self.fg = self.named_colors[self.active_color].1;
            if index.is_none() && !self.studio.recovery_decided {
                self.studio.last_session = Some(state);
            }
        }
        self.status = format!("Restored {restored_count} open tabs");
    }
    pub(super) fn selection_bounds(&self) -> [i32; 4] {
        let (x, y, x1, y1) =
            self.selection
                .unwrap_or((0, 0, self.doc.width as i32 - 1, self.doc.height as i32 - 1));
        [x.min(x1), y.min(y1), (x1 - x).abs() + 1, (y1 - y).abs() + 1]
    }
    pub(super) fn selected(&self, x: i32, y: i32) -> bool {
        if let Some(mask) = &self.studio.mask {
            return self
                .doc
                .index(x, y)
                .is_some_and(|i| mask.get(i) == Some(&true));
        }
        let b = self.selection_bounds();
        x >= b[0]
            && y >= b[1]
            && x < b[0] + b[2]
            && y < b[1] + b[3]
            && (self.studio.polygon.is_empty()
                || workflows::inside_polygon(&self.studio.polygon, x, y))
    }
    pub(super) fn transform_selection(&mut self, t: Transform) {
        let b = self.selection_bounds();
        let mask: Vec<bool> = (0..self.doc.width * self.doc.height)
            .map(|i| self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32))
            .collect();
        let original_bounds = b;
        let mut next = self.doc.clone();
        let result = if let Transform::RotateAround { degrees, pivot } = t
            && self.studio.rotation_rotsprite
        {
            workflows::transform_rotsprite(
                &mut next,
                self.layer,
                self.frame,
                b,
                Some(&mask),
                degrees,
                pivot,
            )
            .map(|(bounds, selection)| (bounds, Some(selection)))
        } else {
            workflows::transform(&mut next, self.layer, self.frame, b, Some(&mask), t)
                .map(|bounds| (bounds, None))
        };
        match result {
            Ok((b, rotated_selection)) => {
                self.remember_action("Rotate selection");
                self.doc = next;
                self.selection = Some((
                    b[0].max(0),
                    b[1].max(0),
                    (b[0] + b[2] - 1).min(self.doc.width as i32 - 1),
                    (b[1] + b[3] - 1).min(self.doc.height as i32 - 1),
                ));
                if b[0] >= self.doc.width as i32
                    || b[1] >= self.doc.height as i32
                    || b[0] + b[2] <= 0
                    || b[1] + b[3] <= 0
                {
                    self.selection = None
                }
                let [ox, oy, ow, oh] = original_bounds;
                for p in &mut self.studio.polygon {
                    let (x, y) = (p.0 - ox, p.1 - oy);
                    *p = match t {
                        Transform::Move(dx, dy) => (p.0 + dx, p.1 + dy),
                        Transform::RotateAround { .. } => *p,
                        Transform::Rotate => (ox + oh - y, oy + x),
                        Transform::FlipX => (ox + ow - x, p.1),
                        Transform::FlipY => (p.0, oy + oh - y),
                        Transform::Resize(w, h) => (
                            ox + (x as f32 * w as f32 / ow as f32).round() as i32,
                            oy + (y as f32 * h as f32 / oh as f32).round() as i32,
                        ),
                    };
                }
                if self.studio.mask.is_some() || matches!(t, Transform::RotateAround { .. }) {
                    let mask = rotated_selection.unwrap_or_else(|| {
                        workflows::transform_mask(&self.doc, original_bounds, &mask, t)
                    });
                    self.studio.polygon.clear();
                    self.set_selection_mask(mask);
                }
                self.status = "Transformed selection".into()
            }
            Err(e) => self.status = e,
        }
    }
    pub(super) fn studio_action(&mut self, s: &str) -> bool {
        match s {
            "studio" => self.studio.open = true,
            "sheet" => self.studio.sheet_open = true,
            "tiles" => {
                self.studio.tile_browser_set = self.doc.layers[self.layer].tileset.unwrap_or(0);
                self.studio.tiles_open = true;
            }
            "seamless" | "seamless-horizontal" | "seamless-vertical" => {
                let axes = match s {
                    "seamless-horizontal" => WrapAxes::HORIZONTAL,
                    "seamless-vertical" => WrapAxes::VERTICAL,
                    _ => WrapAxes::BOTH,
                };
                self.set_seamless_axes(if self.studio.seamless == axes {
                    WrapAxes::NONE
                } else {
                    axes
                });
            }
            "lasso" => self.tool = Tool::Lasso,
            "move" => self.tool = Tool::Move,
            "gradient" => self.tool = Tool::Gradient,
            "rotate" => self.transform_selection(Transform::Rotate),
            "selectionflipx" => self.transform_selection(Transform::FlipX),
            "selectionflipy" => self.transform_selection(Transform::FlipY),
            "resizecanvas" | "spritesize" => {
                self.studio.canvas_resize = Some(CanvasResize {
                    document: self.doc.editor_id.to_string(),
                    resample: s == "spritesize",
                    size: [self.doc.width, self.doc.height],
                    anchor: 4,
                    offset: [0, 0],
                });
            }
            "duplicatesprite" => {
                let duplicate = workflows::duplicate_sprite(&self.doc);
                let baseline = Document::new(duplicate.width, duplicate.height);
                self.open_document(duplicate, None);
                self.saved = baseline;
                self.status = "Opened a duplicate sprite".into();
            }
            "layer-duplicate" | "layer-merge" => {
                let layer = self.layer;
                let duplicate = s == "layer-duplicate";
                let title = if duplicate {
                    "Duplicate layer"
                } else {
                    "Merge down"
                };
                self.start_document_job(title, move |source| {
                    let mut doc = source.clone();
                    let layer = if duplicate {
                        doc.duplicate_layer(layer)?
                    } else {
                        doc.merge_down(layer)?
                    };
                    Ok(SpriteJobOutput {
                        doc,
                        layer: Some(layer),
                    })
                });
            }
            "flatten-layers" | "flatten-visible" => {
                let visible_only = s == "flatten-visible";
                self.start_document_job("Flatten layers", move |source| {
                    let mut doc = source.clone();
                    let layer = doc.flatten_layers(visible_only)?;
                    Ok(SpriteJobOutput {
                        doc,
                        layer: Some(layer),
                    })
                });
            }
            "layer-via-copy" | "layer-via-cut" => {
                let layer = self.layer;
                let frame = self.frame;
                let cut = s == "layer-via-cut";
                let mask: Vec<_> = (0..self.doc.width * self.doc.height)
                    .map(|index| {
                        self.selected(
                            (index % self.doc.width) as i32,
                            (index / self.doc.width) as i32,
                        )
                    })
                    .collect();
                let title = if cut {
                    "Layer via cut"
                } else {
                    "Layer via copy"
                };
                self.start_document_job(title, move |source| {
                    let mut doc = source.clone();
                    let layer = doc.layer_from_selection(layer, frame, &mask, cut)?;
                    Ok(SpriteJobOutput {
                        doc,
                        layer: Some(layer),
                    })
                });
            }
            "convert-raster" => self.start_layer_conversion(LayerConversion::Raster),
            "convert-background" => {
                self.start_layer_conversion(LayerConversion::Background(self.bg))
            }
            "convert-tilemap" => {
                self.studio.tile_conversion = Some(TileConversionDialog {
                    layer: self.doc.layers[self.layer].editor_id.to_string(),
                    width: self.tile_size,
                    height: self.tile_size,
                });
            }
            "trimsprite" => self.start_sprite_job("Auto-trim sprite", workflows::trim_sprite),
            "rotatespriteright" | "rotatespriteleft" | "rotatespritehalf" | "flipspritex"
            | "flipspritey" => {
                let orientation = match s {
                    "rotatespriteright" => workflows::Orientation::TurnRight,
                    "rotatespriteleft" => workflows::Orientation::TurnLeft,
                    "rotatespritehalf" => workflows::Orientation::TurnHalf,
                    "flipspritex" => workflows::Orientation::FlipHorizontal,
                    _ => workflows::Orientation::FlipVertical,
                };
                self.start_sprite_job("Transform sprite", move |doc| {
                    workflows::orient_sprite(doc, orientation)
                });
            }
            "crop" => {
                let mut d = self.doc.clone();
                match workflows::crop(&mut d, self.selection_bounds()) {
                    Ok(()) => {
                        self.remember_action("Crop to selection");
                        self.doc = d;
                        self.selection = None;
                        self.studio.mask = None;
                        self.studio.move_mask = None;
                        self.studio.polygon.clear();
                        self.status = "Cropped canvas".into()
                    }
                    Err(e) => self.status = e,
                }
            }
            "sequence" => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("PNG sequence", &["png"])
                    .pick_files()
                {
                    match interchange::load_sequence(&paths) {
                        Ok(d) => self.open_document(d, None),
                        Err(e) => self.status = format!("Could not import sequence: {e}"),
                    }
                }
            }
            "tileset-atlas" => {
                let layer = &self.doc.layers[self.layer];
                self.studio.tileset_atlas_export = Some(TilesetAtlasDialog {
                    visible: self.timeline_layers.selected.is_empty()
                        && !layer.group
                        && layer.tileset.is_none(),
                    ..TilesetAtlasDialog::default()
                });
            }
            "tiledexport" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Tiled JSON map", &["tmj"])
                    .set_file_name("map.tmj")
                    .save_file()
                {
                    self.status = match interchange::export_tiled(&path, &self.doc, self.frame) {
                        Ok(()) => "Exported Tiled map, tilesets and images".into(),
                        Err(e) => format!("Export failed: {e}"),
                    }
                }
            }
            "savepalette" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Named palette", &["json"])
                    .set_file_name("palette.json")
                    .save_file()
                {
                    self.status = match std::fs::write(
                        path,
                        serde_json::to_vec_pretty(&self.named_colors).unwrap(),
                    ) {
                        Ok(()) => "Saved named palette".into(),
                        Err(e) => format!("Could not save palette: {e}"),
                    }
                }
            }
            "loadpalette" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Named palette", &["json"])
                    .pick_file()
                {
                    let result: Result<Vec<(String, Pixel)>, String> = (|| {
                        let data = std::fs::read(path).map_err(|e| e.to_string())?;
                        let colors: Vec<(String, Pixel)> =
                            serde_json::from_slice(&data).map_err(|e| e.to_string())?;
                        if colors.is_empty() || colors.len() > 256 {
                            return Err("Palette must have 1–256 colors".into());
                        }
                        Ok(colors)
                    })();
                    match result {
                        Ok(colors) => {
                            self.named_colors = colors;
                            self.select_named_color(0)
                        }
                        Err(e) => self.status = format!("Could not load palette: {e}"),
                    }
                }
            }
            _ => return false,
        }
        true
    }
    pub(super) fn effects(&mut self, kind: &'static str) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            self.status = "Select an unlocked image layer".into();
            return;
        }
        if kind == "Outline" || kind == "Drop shadow" {
            let mask = (0..self.doc.width * self.doc.height)
                .map(|i| self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32))
                .collect::<Vec<_>>();
            let mut next = self.doc.clone();
            let result = if kind == "Drop shadow" {
                workflows::spatial_effect(
                    &mut next,
                    self.layer,
                    self.frame,
                    &mask,
                    self.fg,
                    Some((self.studio.dx, self.studio.dy)),
                )
            } else {
                workflows::spatial_outline(
                    &mut next,
                    self.layer,
                    self.frame,
                    &mask,
                    self.fg,
                    self.studio.outline_corners,
                )
            };
            match result {
                Ok(()) => {
                    self.remember_action(kind);
                    self.doc = next;
                    self.status = format!("Applied {kind}");
                }
                Err(e) => self.status = e,
            }
            return;
        }
        self.remember_action(kind);
        for i in 0..self.doc.width * self.doc.height {
            if !self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                continue;
            }
            let mut p = self.doc.layers[self.layer].cels[self.frame][i];
            if p[3] == 0 {
                continue;
            }
            let gray =
                ((p[0] as u32 * 2126 + p[1] as u32 * 7152 + p[2] as u32 * 722) / 10000) as u8;
            for c in &mut p[..3] {
                *c = match kind {
                    "Invert" => 255 - *c,
                    "Grayscale" => gray,
                    "Lighten" => c.saturating_add(16),
                    _ => c.saturating_sub(16),
                }
            }
            self.doc.set_pixel(self.layer, self.frame, i, p);
        }
        self.status = format!("Applied {kind}");
    }
    fn rotation_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Rotate around a pivot", |ui| {
            let [x, y, w, h] = self.selection_bounds();
            let mut pivot = self.studio.rotation_pivot.unwrap_or([
                f64::from(x) + f64::from(w) * 0.5,
                f64::from(y) + f64::from(h) * 0.5,
            ]);
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.studio.rotation_angle)
                        .range(-360. ..=360.)
                        .suffix("°"),
                );
                ui.add(egui::DragValue::new(&mut pivot[0]).prefix("Pivot X "));
                ui.add(egui::DragValue::new(&mut pivot[1]).prefix("Pivot Y "));
            });
            ui.horizontal(|ui| {
                ui.pc_selectable_value(&mut self.studio.rotation_rotsprite, false, "Fast");
                ui.pc_selectable_value(&mut self.studio.rotation_rotsprite, true, "RotSprite")
                    .on_hover_text("Smooths stepped edges without blending colors.");
            });
            ui.pc_checkbox(&mut self.studio.rotation_preview, "Preview on canvas");
            let within_budget = !self.studio.rotation_rotsprite
                || workflows::rotsprite_within_budget(
                    w as usize,
                    h as usize,
                    self.studio.rotation_angle,
                );
            if !within_budget {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "Selection is too large for RotSprite. Select a smaller area or use Fast.",
                );
            }
            let transform = Transform::RotateAround {
                degrees: self.studio.rotation_angle,
                pivot,
            };
            if ui
                .add_enabled_ui(within_budget, |ui| ui.pc_button("Apply rotation"))
                .inner
                .clicked()
            {
                self.transform_selection(transform);
                self.studio.rotation_preview = false;
            }
            self.studio.rotation_pivot = Some(pivot);
        });
    }

    pub(super) fn rotation_preview_active(&self) -> bool {
        self.studio.open && self.studio.rotation_preview
    }

    pub(super) fn rotation_preview(
        &mut self,
        ui: &mut egui::Ui,
        board: Rect,
    ) -> Option<egui::TextureHandle> {
        if !self.studio.open
            || !self.studio.rotation_preview
            || self.doc.width * self.doc.height > 4_194_304
        {
            self.studio.rotation_texture = None;
            return None;
        }
        let mut pivot = self.studio.rotation_pivot?;
        let center = board.min + Vec2::new(pivot[0] as f32, pivot[1] as f32) * self.zoom;
        let response = ui.interact(
            Rect::from_center_size(center, Vec2::splat(20.)),
            ui.id().with("rotation-pivot"),
            Sense::drag(),
        );
        let delta = response.drag_delta();
        if delta != Vec2::ZERO {
            pivot[0] += f64::from(delta.x / self.zoom);
            pivot[1] += f64::from(delta.y / self.zoom);
            self.studio.rotation_pivot = Some(pivot);
        }
        let bounds = self.selection_bounds();
        let degrees = self.studio.rotation_angle;
        if self.studio.rotation_rotsprite
            && let Some(cached) = &self.studio.rotation_texture
            && cached.degrees == degrees
            && cached.pivot == pivot
            && cached.bounds == bounds
            && cached.layer == self.layer
            && cached.frame == self.frame
            && cached.mask == self.studio.mask
            && cached.polygon == self.studio.polygon
            && cached.source == self.doc
        {
            return Some(cached.texture.clone());
        }
        self.studio.rotation_texture = None;
        if self.studio.rotation_rotsprite
            && !workflows::rotsprite_within_budget(bounds[2] as usize, bounds[3] as usize, degrees)
        {
            return None;
        }
        let mut preview = self.doc.clone();
        let mask: Vec<_> = (0..self.doc.width * self.doc.height)
            .map(|i| self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32))
            .collect();
        let transform = Transform::RotateAround { degrees, pivot };
        if self.studio.rotation_rotsprite {
            workflows::transform_rotsprite(
                &mut preview,
                self.layer,
                self.frame,
                bounds,
                Some(&mask),
                degrees,
                pivot,
            )
            .ok()?;
        } else {
            workflows::transform(
                &mut preview,
                self.layer,
                self.frame,
                bounds,
                Some(&mask),
                transform,
            )
            .ok()?;
        }
        let pixels = preview.composite(self.frame);
        let texture = ui.ctx().load_texture(
            "rotation-preview",
            color_image(&pixels, preview.width, preview.height),
            egui::TextureOptions::NEAREST,
        );
        if self.studio.rotation_rotsprite {
            self.studio.rotation_texture = Some(RotationPreview {
                source: self.doc.clone(),
                layer: self.layer,
                frame: self.frame,
                bounds,
                mask: self.studio.mask.clone(),
                polygon: self.studio.polygon.clone(),
                degrees,
                pivot,
                texture: texture.clone(),
            });
        }
        Some(texture)
    }

    pub(super) fn paint_rotation_pivot(&self, painter: &egui::Painter, board: Rect) {
        if self.rotation_preview_active()
            && let Some(pivot) = self.studio.rotation_pivot
        {
            let center = board.min + Vec2::new(pivot[0] as f32, pivot[1] as f32) * self.zoom;
            painter.circle_stroke(center, 7., Stroke::new(3., Color32::BLACK));
            painter.circle_stroke(center, 7., Stroke::new(1., Color32::WHITE));
        }
    }

    fn adjustment_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Masks and adjustments", |ui| {
            let layer = self.layer;
            if ui.pc_button("Mask from selection").clicked() {
                let mut mask = Pixels::empty(self.doc.width * self.doc.height);
                for i in 0..mask.len() {
                    if self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                        mask.set(i, [0, 0, 0, 255]);
                    }
                }
                self.edit_timeline("Set layer mask", |doc| {
                    doc.set_layer_mask(layer, Some(mask))
                });
            }
            if ui.pc_button("Remove layer mask").clicked() {
                self.edit_timeline("Remove layer mask", |doc| doc.set_layer_mask(layer, None));
            }
            let mut adjustment = self.studio.adjustment_input.clone();
            ui.horizontal(|ui| {
                if ui.pc_button("Edit current adjustment").clicked()
                    && let Some(current) = &self.doc.layers[layer].adjustment
                {
                    adjustment = current.clone();
                }
                if ui.pc_button("Levels").clicked() {
                    adjustment = Adjustment::Levels {
                        black: 0,
                        white: 255,
                    };
                }
                if ui.pc_button("Hue / saturation").clicked() {
                    adjustment = Adjustment::HueSaturation {
                        hue: 0,
                        saturation: 0,
                    };
                }
                if ui.pc_button("Palette map").clicked() {
                    adjustment = Adjustment::Palette { colors: Vec::new() };
                }
            });
            match &mut adjustment {
                Adjustment::Levels { black, white } => {
                    ui.add(
                        egui::DragValue::new(black)
                            .range(0..=white.saturating_sub(1))
                            .prefix("Black "),
                    );
                    ui.add(
                        egui::DragValue::new(white)
                            .range(black.saturating_add(1)..=255)
                            .prefix("White "),
                    );
                }
                Adjustment::HueSaturation { hue, saturation } => {
                    ui.add(egui::DragValue::new(hue).range(-180..=180).prefix("Hue "));
                    ui.add(
                        egui::DragValue::new(saturation)
                            .range(-100..=100)
                            .prefix("Saturation "),
                    );
                }
                Adjustment::Palette { colors } => {
                    if colors.len() < 256 && ui.pc_button("Map alternate to active color").clicked()
                    {
                        if let Some((_, color)) =
                            colors.iter_mut().find(|(source, _)| *source == self.bg)
                        {
                            *color = self.fg;
                        } else {
                            colors.push((self.bg, self.fg));
                        }
                    }
                    ui.label(format!("{} color mappings", colors.len()));
                }
            }
            self.studio.adjustment_input = adjustment.clone();
            if ui.pc_button("Set layer adjustment").clicked() {
                self.edit_timeline("Set adjustment", |doc| {
                    doc.set_adjustment(layer, Some(adjustment))
                });
            }
            if self.doc.layers[layer].adjustment.is_some()
                && ui.pc_button("Remove adjustment").clicked()
            {
                self.edit_timeline("Remove adjustment", |doc| doc.set_adjustment(layer, None));
            }
        });
    }

    fn apply_canvas_resize(&mut self, resize: &CanvasResize) -> Result<(), String> {
        let old = [self.doc.width, self.doc.height];
        let offset = resize.offset_from(old);
        if resize.size == old && offset == [0, 0] {
            return Ok(());
        }
        let mut resized = self.doc.clone();
        workflows::resize_canvas(&mut resized, resize.size, offset)?;
        self.apply_sprite_geometry("Resize sprite", resized);
        self.status = format!("Canvas resized to {} × {}", self.doc.width, self.doc.height);
        Ok(())
    }

    fn apply_sprite_geometry(&mut self, label: &'static str, doc: Document) {
        self.cancel_gesture();
        self.remember_action(label);
        self.doc = doc;
        self.selection = None;
        self.studio.mask = None;
        self.studio.move_mask = None;
        self.studio.polygon.clear();
    }

    fn start_sprite_job(
        &mut self,
        title: &'static str,
        operation: impl FnOnce(&Document) -> Result<Document, String> + Send + 'static,
    ) {
        self.start_document_job(title, move |source| {
            operation(source).map(|doc| SpriteJobOutput { doc, layer: None })
        });
    }

    fn start_document_job(
        &mut self,
        title: &'static str,
        operation: impl FnOnce(&Document) -> Result<SpriteJobOutput, String> + Send + 'static,
    ) {
        if self.studio.sprite_job.is_some() {
            self.status = "A sprite operation is already running".into();
            return;
        }
        self.cancel_gesture();
        let source = self.doc.clone();
        let snapshot = source.clone();
        let worker = std::thread::spawn(move || operation(&snapshot));
        self.studio.sprite_job = Some(SpriteJob {
            source,
            title,
            worker,
        });
        self.status = format!("{title} in progress…");
    }

    fn poll_sprite_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.studio.sprite_job else {
            return;
        };
        if !job.worker.is_finished()
            || self.save_job.is_some()
            || self.script_job.is_some()
            || self.start.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(25));
            return;
        }
        let job = self.studio.sprite_job.take().unwrap();
        match job.worker.join() {
            Ok(Ok(SpriteJobOutput { doc, layer })) => {
                if !self.doc.same_editor_targets(&job.source)
                    || self.doc.differs_from_saved(&job.source)
                {
                    self.status = format!(
                        "{} canceled because the original sprite changed or is no longer active",
                        job.title
                    );
                } else {
                    if self.doc.differs_from_saved(&doc) {
                        self.apply_sprite_geometry(job.title, doc);
                        if let Some(layer) = layer {
                            self.layer = layer;
                            self.timeline_anchor = None;
                            self.selected_cels = None;
                            self.timeline_frames = TimelineHeaders::default();
                            self.timeline_layers = TimelineHeaders::default();
                            self.timeline_destination = None;
                        }
                    }
                    self.status = format!("{} complete", job.title);
                }
            }
            Ok(Err(error)) => self.status = error,
            Err(_) => self.status = format!("{} worker stopped unexpectedly", job.title),
        }
    }

    fn canvas_resize_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut resize) = self.studio.canvas_resize.take() else {
            return;
        };
        if resize.document != self.doc.editor_id.to_string() {
            return;
        }
        let old = [self.doc.width, self.doc.height];
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let title = if resize.resample {
            "Sprite size"
        } else {
            "Canvas size"
        };
        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut resize.size[0]).range(1..=MAX_DIMENSION));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut resize.size[1]).range(1..=MAX_DIMENSION));
                });
                if resize.resample {
                    ui.small("Nearest-neighbor resampling scales the artwork and tilesets.");
                } else {
                    let old_anchor = resize.anchor;
                    let old_offset = resize.offset_from(old);
                    egui::ComboBox::from_label("Anchor")
                        .selected_text(CANVAS_ANCHORS[resize.anchor])
                        .show_ui(ui, |ui| {
                            for (anchor, name) in CANVAS_ANCHORS.into_iter().enumerate() {
                                ui.pc_selectable_value(&mut resize.anchor, anchor, name);
                            }
                        });
                    if resize.anchor == 9 && old_anchor != 9 {
                        resize.offset = old_offset;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Offset");
                        if resize.anchor == 9 {
                            ui.add(egui::DragValue::new(&mut resize.offset[0]).prefix("X "));
                            ui.add(egui::DragValue::new(&mut resize.offset[1]).prefix("Y "));
                        } else {
                            let [x, y] = resize.offset_from(old);
                            ui.label(format!("X {x}, Y {y}"));
                        }
                    });
                    ui.small("Pixels and whole tiles outside the new canvas are cropped.");
                }
                ui.horizontal(|ui| {
                    apply = ui.pc_button("Resize").clicked();
                    cancel = ui.pc_button("Cancel").clicked();
                });
            });
        if apply {
            if resize.resample {
                let size = resize.size;
                self.start_sprite_job("Resize sprite", move |doc| {
                    workflows::resample_sprite(doc, size)
                });
            } else if let Err(error) = self.apply_canvas_resize(&resize) {
                self.status = error;
                self.studio.canvas_resize = Some(resize);
            }
        } else if open && !cancel {
            self.studio.canvas_resize = Some(resize);
        }
    }

    fn start_layer_conversion(&mut self, conversion: LayerConversion) {
        let layer = self.layer;
        self.start_document_job("Convert layer", move |source| {
            let mut doc = source.clone();
            let layer = doc.convert_layer(layer, conversion)?;
            Ok(SpriteJobOutput {
                doc,
                layer: Some(layer),
            })
        });
    }

    fn start_tileset_atlas_export(&mut self, path: PathBuf, options: TilesetAtlasDialog) {
        if self.export_job.is_some() {
            self.status = "Wait for the current export to finish".into();
            return;
        }
        let mut args = serde_json::json!({
            "output": path, "format": "tileset-atlas", "overwrite": true,
        });
        if !options.visible {
            let layers = if self.timeline_layers.selected.is_empty() {
                vec![self.layer]
            } else {
                self.timeline_layers.selected.clone()
            };
            args["layers"] = serde_json::json!(layers);
        }
        if options.columns != 0 {
            args["columns"] = serde_json::json!(options.columns);
        }
        let doc = self.doc.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.export_job = Some(rx);
        self.status = "Exporting tileset atlas…".into();
        std::thread::spawn(move || {
            let result = pixelcraft::automation::prepare_live_export(&doc, 0, &args, "")
                .and_then(|prepared| prepared.write())
                .map(|_| ());
            let _ = tx.send((path, result));
        });
    }

    fn tileset_atlas_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut options) = self.studio.tileset_atlas_export else {
            return;
        };
        let mut open = true;
        let mut export = false;
        egui::Window::new("Export Tileset Atlas")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.pc_selectable_value(&mut options.visible, false, "Selected layers");
                    ui.pc_selectable_value(&mut options.visible, true, "Visible layers");
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut options.columns)
                            .range(0..=MAX_DIMENSION)
                            .prefix("Columns "),
                    );
                    ui.small("0 uses automatic columns");
                });
                export = ui.pc_button("Save PNG…").clicked();
            });
        if export
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG tileset atlas", &["png"])
                .set_file_name("tileset.png")
                .save_file()
        {
            self.start_tileset_atlas_export(path, options);
            open = false;
        }
        self.studio.tileset_atlas_export = open.then_some(options);
    }

    fn tileset_browser(&mut self, ctx: &egui::Context) {
        if !self.studio.tiles_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Tileset browser")
            .open(&mut open)
            .default_width(350.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.pc_button("Import PNG tilesheet…").clicked() {
                        match self.import_tileset() {
                            Ok(()) => {
                                if let Some(index) = self.doc.layers[self.layer].tileset {
                                    self.studio.tile_browser_set = index;
                                }
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    ui.add(egui::DragValue::new(&mut self.tile_size).range(1..=256).prefix("Tile size "));
                });
                if self.doc.tilesets.is_empty() {
                    ui.label("Import a tilesheet to browse and paint tiles.");
                    return;
                }
                self.studio.tile_browser_set = self.studio.tile_browser_set.min(self.doc.tilesets.len() - 1);
                egui::ComboBox::from_label("Tileset")
                    .selected_text(&self.doc.tilesets[self.studio.tile_browser_set].name)
                    .show_ui(ui, |ui| {
                        for (index, set) in self.doc.tilesets.iter().enumerate() {
                            ui.pc_selectable_value(&mut self.studio.tile_browser_set, index, &set.name);
                        }
                    });
                let tileset = self.studio.tile_browser_set;
                let set = &self.doc.tilesets[tileset];
                let count = set.tiles.len();
                let active_set = self.doc.layers[self.layer].tileset == Some(tileset);
                let selected = self.selected_tile & TILE_ID_MASK;
                let thumb_size = 32.;
                let row_height = 46.;
                let columns = (ui.available_width() / row_height).floor().max(1.) as usize;
                let mut clicked = None;
                egui::ScrollArea::vertical()
                    .id_salt(("tiles", tileset))
                    .max_height(345.)
                    .show_rows(ui, row_height, count.div_ceil(columns), |ui, rows| {
                        for row in rows {
                            ui.horizontal(|ui| {
                                for index in row * columns..((row + 1) * columns).min(count) {
                                    let id = index as u32 + 1;
                                    // Sample at thumbnail size without expanding sparse or oversized tiles.
                                    let preview_w = set.width.min(thumb_size as usize);
                                    let preview_h = set.height.min(thumb_size as usize);
                                    let placement = if active_set && selected == id { self.selected_tile } else { id };
                                    let pixels = (0..preview_h).flat_map(|row| {
                                        (0..preview_w).map(move |col| {
                                            set.placement_pixel(placement, col * set.width / preview_w, row * set.height / preview_h)
                                        })
                                    }).collect::<Vec<_>>();
                                    let texture = ctx.load_texture(
                                        format!("tile-{tileset}-{index}"),
                                        color_image(&pixels, preview_w, preview_h),
                                        egui::TextureOptions::NEAREST,
                                    );
                                    let button = egui::Button::image(egui::Image::new(&texture).fit_to_exact_size(Vec2::splat(thumb_size)))
                                        .selected(active_set && selected == id);
                                    if ui.add(button).on_hover_text(format!("Tile {id}")).clicked() {
                                        clicked = Some(id);
                                    }
                                }
                            });
                        }
                    });
                if let Some(id) = clicked {
                    if !active_set {
                        let layer = self.doc.layers.iter().position(|layer| layer.tileset == Some(tileset));
                        if let Some(layer) = layer {
                            self.timeline_select(layer, self.frame, false);
                        } else if self.edit_timeline("New tile layer", |doc| doc.add_tile_layer(tileset)) {
                            self.timeline_select(self.doc.layers.len() - 1, self.frame, false);
                        }
                    }
                    if self.doc.layers[self.layer].tileset == Some(tileset) {
                        self.tool = Tool::Pencil;
                        self.selected_tile = id;
                    }
                }
                let selected = self.selected_tile & TILE_ID_MASK;
                if self.doc.layers[self.layer].tileset == Some(tileset)
                    && selected > 0
                    && selected as usize <= count
                {
                    ui.label(format!("Tile {selected}"));
                    ui.horizontal_wrapped(|ui| {
                        for (label, transform) in [
                            ("Flip horizontally", TileTransform::FlipHorizontal),
                            ("Flip vertically", TileTransform::FlipVertical),
                            ("Rotate clockwise", TileTransform::RotateClockwise),
                        ] {
                            let transformed = transform.apply(self.selected_tile, &self.doc.tilesets[tileset]);
                            let tooltip = match &transformed {
                                Ok(_) => "Change tile brush orientation",
                                Err(reason) => reason,
                            };
                            if ui.add_enabled(transformed.is_ok(), egui::Button::new(label)).on_disabled_hover_text(tooltip)
                                .on_hover_text(tooltip).clicked()
                                && let Ok(placement) = transformed
                            {
                                self.selected_tile = placement;
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.pc_button("Duplicate tile").clicked() {
                            self.apply_tile_definition_edit(tileset, TileDefinitionEdit::Duplicate);
                        }
                        if ui.pc_button("Delete tile").on_hover_text("Clear placements of this tile from every frame and layer using the tileset").clicked() {
                            self.apply_tile_definition_edit(tileset, TileDefinitionEdit::Delete);
                        }
                        if ui.add_enabled(selected > 1, egui::Button::new("Move earlier")).clicked() {
                            self.apply_tile_definition_edit(tileset, TileDefinitionEdit::MoveTo(selected - 1));
                        }
                        if ui.add_enabled((selected as usize) < count, egui::Button::new("Move later")).clicked() {
                            self.apply_tile_definition_edit(tileset, TileDefinitionEdit::MoveTo(selected + 1));
                        }
                        ui.menu_button("Move to index…", |ui| {
                            let key = ui.id().with((self.active_tab, tileset, selected));
                            let mut position = ui.data_mut(|data| data.get_temp::<u32>(key))
                                .unwrap_or(selected).min(count as u32);
                            ui.add(egui::DragValue::new(&mut position).range(1..=count as u32).prefix("Tile "));
                            ui.data_mut(|data| data.insert_temp(key, position));
                            if ui.add_enabled(position != selected, egui::Button::new("Move tile")).clicked() {
                                self.apply_tile_definition_edit(tileset, TileDefinitionEdit::MoveTo(position));
                                ui.close();
                            }
                        });
                    });
                }
                ui.pc_checkbox(&mut self.studio.show_tile_indices, "Show tile indices on canvas");
                let mut axes = self.studio.seamless;
                let modes = [
                    (WrapAxes::NONE, "Off"),
                    (WrapAxes::HORIZONTAL, "Horizontal"),
                    (WrapAxes::VERTICAL, "Vertical"),
                    (WrapAxes::BOTH, "Both axes"),
                ];
                egui::ComboBox::from_label("Seamless canvas")
                    .selected_text(modes.iter().find(|(mode, _)| *mode == axes).map_or("Off", |(_, label)| *label))
                    .show_ui(ui, |ui| {
                        for (mode, label) in modes {
                            ui.pc_selectable_value(&mut axes, mode, label);
                        }
                    });
                if axes != self.studio.seamless {
                    self.set_seamless_axes(axes);
                }
                if ui.pc_command_button("tileset-atlas", "Export tileset atlas…").clicked() {
                    self.studio_action("tileset-atlas");
                }
                if ui.pc_command_button("tiledexport", "Export Tiled map…").clicked() {
                    self.studio_action("tiledexport");
                }
            });
        self.studio.tiles_open = open;
    }

    pub(super) fn paint_tile_indices(&self, painter: &egui::Painter, board: Rect) {
        let layer = &self.doc.layers[self.layer];
        if !self.studio.show_tile_indices || self.playing || !layer.visible {
            return;
        }
        let Some(set) = layer.tileset.and_then(|set| self.doc.tilesets.get(set)) else {
            return;
        };
        let step = Vec2::new(set.width as f32, set.height as f32) * self.zoom;
        let digits = set.tiles.len().max(1).ilog10() as f32 + 1.;
        if step.x < digits * 7. + 6. || step.y < 18. {
            return;
        }
        let geometry = layer.cel_meta[self.frame].tilemap.as_ref();
        let columns = geometry.map_or(self.doc.width.div_ceil(set.width), |g| g.columns);
        let rows = geometry.map_or(self.doc.height.div_ceil(set.height), |g| g.rows);
        let origin = board.min
            + geometry.map_or(Vec2::ZERO, |g| {
                Vec2::new(g.x as f32, g.y as f32) * self.zoom
            });
        let clip = painter.clip_rect().intersect(board);
        if !clip.is_positive() {
            return;
        }
        let first = (clip.min - origin) / step;
        let end = (clip.max - origin) / step;
        let placements = &layer.tile_cels[self.frame];
        let font = egui::FontId::monospace(11.);
        for y in (first.y.floor() as usize).min(rows)..(end.y.ceil() as usize).min(rows) {
            for x in (first.x.floor() as usize).min(columns)..(end.x.ceil() as usize).min(columns) {
                let id = placements[y * columns + x] & TILE_ID_MASK;
                if id == 0 {
                    continue;
                }
                let pos = origin + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * step;
                let galley = painter.layout_no_wrap(id.to_string(), font.clone(), Color32::WHITE);
                let label = Rect::from_center_size(pos, galley.size());
                painter.rect_filled(label.expand(2.), 2., Color32::from_black_alpha(180));
                painter.galley(label.min, galley, Color32::WHITE);
            }
        }
    }

    fn tile_conversion_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.studio.tile_conversion.take() else {
            return;
        };
        if self.doc.layers[self.layer].editor_id.to_string() != dialog.layer {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Convert layer to tilemap")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Tile width");
                    ui.add(egui::DragValue::new(&mut dialog.width).range(1..=MAX_DIMENSION));
                    ui.label("Tile height");
                    ui.add(egui::DragValue::new(&mut dialog.height).range(1..=MAX_DIMENSION));
                });
                ui.small("Identical tiles share one tileset entry across frames.");
                ui.horizontal(|ui| {
                    apply = ui.pc_button("Convert").clicked();
                    cancel = ui.pc_button("Cancel").clicked();
                });
            });
        if apply {
            self.start_layer_conversion(LayerConversion::Tilemap {
                width: dialog.width,
                height: dialog.height,
            });
        } else if open && !cancel {
            self.studio.tile_conversion = Some(dialog);
        }
    }

    pub(super) fn studio_dialogs(&mut self, ctx: &egui::Context) {
        self.poll_sprite_job(ctx);
        self.canvas_resize_dialog(ctx);
        self.tile_conversion_dialog(ctx);
        if self.studio.open {
            let mut open = true;
            egui::Window::new("Drawing & selection studio")
                .open(&mut open)
                .default_width(370.)
                .show(ctx, |ui| {
                    ui.strong("Drawing");
                    ui.horizontal(|ui| {
                        ui.pc_checkbox(&mut self.studio.mirror_x, "Mirror X");
                        ui.pc_checkbox(&mut self.studio.mirror_y, "Mirror Y");
                    });
                    ui.pc_checkbox(
                        &mut self.studio.pixel_perfect,
                        "Pixel-perfect pencil (1 px)",
                    );
                    ui.pc_checkbox(&mut self.studio.pressure, "Pressure controls brush size");
                    ui.small("Pressure is used when provided by the tablet backend.");
                    ui.horizontal(|ui| {
                        if ui.pc_command_button("lasso", "Lasso").clicked() {
                            self.tool = Tool::Lasso;
                            self.studio.open = false
                        }
                        if ui.pc_command_button("move", "Move").clicked() {
                            self.tool = Tool::Move;
                            self.studio.open = false
                        }
                        if ui.pc_command_button("gradient", "Gradient").clicked() {
                            self.tool = Tool::Gradient;
                            self.studio.open = false
                        }
                    });
                    ui.separator();
                    ui.strong("Selection transforms");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.studio.dx)
                                .range(-16384..=16384)
                                .prefix("X "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.studio.dy)
                                .range(-16384..=16384)
                                .prefix("Y "),
                        );
                        if ui_icons::button_as(ui, "move", "transform-move", "Move", false, false)
                            .clicked()
                        {
                            self.transform_selection(Transform::Move(
                                self.studio.dx,
                                self.studio.dy,
                            ))
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.studio.w)
                                .range(1..=16384)
                                .prefix("W "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.studio.h)
                                .range(1..=16384)
                                .prefix("H "),
                        );
                        if ui.pc_button("Resize").clicked() {
                            self.transform_selection(Transform::Resize(
                                self.studio.w,
                                self.studio.h,
                            ))
                        }
                    });
                    ui.horizontal(|ui| {
                        for (label, t) in [
                            ("Rotate 90°", Transform::Rotate),
                            ("Flip X", Transform::FlipX),
                            ("Flip Y", Transform::FlipY),
                        ] {
                            if ui.pc_button(label).clicked() {
                                self.transform_selection(t)
                            }
                        }
                        if ui.pc_command_button("crop", "Crop canvas").clicked() {
                            self.studio_action("crop");
                        }
                    });
                    ui.separator();
                    self.rotation_controls(ui);
                    self.adjustment_controls(ui);
                    ui.strong("Shading & effects");
                    ui.label("Gradient uses the active and alternate colors (X swaps).");
                    ui.horizontal(|ui| {
                        let mut bg = self.bg;
                        if ui.color_edit_button_srgba_unmultiplied(&mut bg).changed() {
                            self.bg = bg
                        }
                        ui.label("Alternate color");
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.studio.ramp_steps)
                                .range(2..=32)
                                .prefix("Steps "),
                        );
                        if ui.pc_button("Add shading ramp").clicked() {
                            for (i, c) in workflows::ramp(self.fg, self.bg, self.studio.ramp_steps)
                                .into_iter()
                                .enumerate()
                            {
                                self.named_colors.push((format!("Ramp {}", i + 1), c));
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        for label in ["Invert", "Grayscale", "Lighten", "Darken", "Outline", "Drop shadow"] {
                            if ui.pc_button(label).clicked() {
                                self.effects(label)
                            }
                        }
                    });
                    ui.pc_checkbox(&mut self.studio.outline_corners, "Include diagonal neighbors in outline");
                    ui.small("Outline/shadow use the active color and selection. Shadow uses the movement X/Y offsets above.");
                    ui.separator();
                    ui.small(&self.studio.recovery_status);
                });
            self.studio.open &= open;
        }
        self.sprite_sheet_dialog(ctx);
        self.tileset_browser(ctx);
        self.tileset_atlas_dialog(ctx);
    }

    fn sprite_sheet_options(&self) -> SheetOptions {
        let mut options = self.studio.sheet.clone();
        if self.studio.sheet_use_timeline_layers {
            options.layers = Some(if self.timeline_layers.selected.is_empty() {
                vec![self.layer]
            } else {
                self.timeline_layers.selected.clone()
            });
        }
        options
    }

    fn start_sprite_sheet_export(&mut self, path: PathBuf) {
        if self.export_job.is_some() {
            self.status = "Wait for the current export to finish".into();
            return;
        }
        let doc = self.doc.clone();
        let options = self.sprite_sheet_options();
        let slice = self.studio.sheet_slice.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.export_job = Some(rx);
        self.status = "Exporting sprite sheet…".into();
        std::thread::spawn(move || {
            let result =
                interchange::export_sheet_for_slice(&path, &doc, &options, slice.as_deref());
            let _ = tx.send((path, result));
        });
    }

    fn sprite_sheet_dialog(&mut self, ctx: &egui::Context) {
        if !self.studio.sheet_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Export sprite sheet")
            .open(&mut open)
            .default_width(390.)
            .show(ctx, |ui| {
                self.export_scale_ui(ui);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.studio.sheet.columns)
                            .range(1..=256)
                            .prefix("Columns "),
                    );
                });
                ui.horizontal(|ui| {
                    let options = &mut self.studio.sheet;
                    for (label, setting) in [
                        ("Border ", &mut options.border_padding),
                        ("Shape ", &mut options.shape_padding),
                    ] {
                        let mut value = setting.unwrap_or(options.padding);
                        if ui
                            .add(
                                egui::DragValue::new(&mut value)
                                    .range(0..=256)
                                    .prefix(label),
                            )
                            .changed()
                        {
                            *setting = Some(value);
                        }
                    }
                    ui.add(
                        egui::DragValue::new(&mut options.inner_padding)
                            .range(0..=256)
                            .prefix("Inner "),
                    );
                });
                ui.pc_checkbox(&mut self.studio.sheet.trim, "Trim transparent borders");
                ui.pc_checkbox(
                    &mut self.studio.sheet.trim_by_grid,
                    "Align trim to drawing grid",
                );
                let tag = &mut self.studio.sheet.tag;
                if tag.is_some_and(|index| index >= self.doc.tags.len()) {
                    *tag = None;
                }
                egui::ComboBox::from_label("Tag")
                    .selected_text(
                        tag.and_then(|i| self.doc.tags.get(i))
                            .map_or("All tags", |tag| tag.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        ui.pc_selectable_value(tag, None, "All tags");
                        for (index, value) in self.doc.tags.iter().enumerate() {
                            ui.pc_selectable_value(tag, Some(index), &value.name);
                        }
                    });

                let max = self.doc.durations.len();
                let mut limit_frames = self.studio.sheet.frame_range.is_some();
                if ui
                    .pc_checkbox(&mut limit_frames, "Limit to frame range")
                    .changed()
                {
                    self.studio.sheet.frame_range = limit_frames.then_some([0, max - 1]);
                }
                if let Some([start, end]) = &mut self.studio.sheet.frame_range {
                    let mut first = start.saturating_add(1).clamp(1, max);
                    let mut last = end.saturating_add(1).clamp(first, max);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut first)
                                .range(1..=last)
                                .prefix("First "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut last)
                                .range(first..=max)
                                .prefix("Last "),
                        );
                    });
                    *start = first - 1;
                    *end = last - 1;
                }
                ui.pc_checkbox(
                    &mut self.studio.sheet_use_timeline_layers,
                    "Only selected timeline layers and groups",
                );
                if self.studio.sheet_use_timeline_layers {
                    let count = self.timeline_layers.selected.len().max(1);
                    ui.small(format!(
                        "{count} selected · groups include visible descendants"
                    ));
                }
                egui::ComboBox::from_label("Engine preset")
                    .selected_text(&self.studio.sheet.preset)
                    .show_ui(ui, |ui| {
                        for preset in ["Generic", "Godot", "Unity"] {
                            ui.pc_selectable_value(
                                &mut self.studio.sheet.preset,
                                preset.into(),
                                preset,
                            );
                        }
                    });
                let slice = &mut self.studio.sheet_slice;
                if slice
                    .as_ref()
                    .is_some_and(|name| !self.doc.slices.iter().any(|item| item.name == *name))
                {
                    *slice = None;
                }
                if !self.doc.slices.is_empty() {
                    egui::ComboBox::from_label("Crop to slice")
                        .selected_text(slice.as_deref().unwrap_or("Whole sprite"))
                        .show_ui(ui, |ui| {
                            ui.pc_selectable_value(slice, None, "Whole sprite");
                            for item in &self.doc.slices {
                                ui.pc_selectable_value(slice, Some(item.name.clone()), &item.name);
                            }
                        });
                }
                ui.pc_checkbox(
                    &mut self.studio.sheet.split_layers,
                    "Split layers into samples",
                );
                ui.pc_checkbox(&mut self.studio.sheet.split_grid, "Split by drawing grid");
                ui.collapsing("Export names", |ui| {
                    ui.label("Frame filename template");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.studio.sheet.filename_template)
                            .hint_text("{sample}"),
                    );
                    ui.label("Animation tag template");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.studio.sheet.tag_template)
                            .hint_text("{tag}"),
                    );
                    ui.small("Leave the fields blank to use the generated names.");
                });
                let options = self.sprite_sheet_options();
                let slice = self.studio.sheet_slice.as_deref();
                let summary = if options.split_grid || options.split_layers {
                    interchange::split_sheet_sample_count(&self.doc, &options, slice)
                        .map(|count| format!("{count} samples"))
                } else {
                    interchange::pack_sheet_for_slice(&self.doc, &options, slice).map(
                        |(image, metadata)| {
                            format!(
                                "{} × {} · {} frames",
                                image.width(),
                                image.height(),
                                metadata["frames"].as_array().unwrap().len()
                            )
                        },
                    )
                };
                match summary {
                    Ok(summary) => {
                        ui.label(summary);
                        if ui.pc_button("Export PNG + metadata…").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("PNG sheet", &["png"])
                                .set_file_name("sprite.png")
                                .save_file()
                        {
                            self.start_sprite_sheet_export(path);
                        }
                    }
                    Err(error) => {
                        ui.label(error);
                    }
                }
            });
        self.studio.sheet_open = open;
    }

    pub(super) fn stroke_line(&mut self, a: (i32, i32), b: (i32, i32), color: Pixel) {
        let (mut x, mut y) = a;
        let dx = (b.0 - x).abs();
        let sx = if x < b.0 { 1 } else { -1 };
        let dy = -(b.1 - y).abs();
        let sy = if y < b.1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if self.studio.stroke.last() != Some(&(x, y)) {
                self.studio.stroke.push((x, y));
            }
            if (x, y) == b {
                break;
            }
            let e = 2 * err;
            if e >= dy {
                err += dy;
                x += sx
            }
            if e <= dx {
                err += dx;
                y += sy
            }
        }
        if self.studio.pixel_perfect && self.brush == 1 && self.tool == Tool::Pencil {
            let mut path = vec![];
            for &p in &self.studio.stroke {
                if path.len() >= 2 {
                    let c: (i32, i32) = path[path.len() - 1];
                    let a: (i32, i32) = path[path.len() - 2];
                    if (a.0 == c.0 && c.1 == p.1 || a.1 == c.1 && c.0 == p.0)
                        && (a.0 - p.0).abs() == 1
                        && (a.1 - p.1).abs() == 1
                    {
                        path.pop();
                    }
                }
                path.push(p);
            }
            if let Some(base) = &self.stroke_base {
                self.doc = base.clone()
            }
            self.power.shade_seen.clear();
            for p in path {
                self.paint_at(p, color)
            }
        } else {
            let start = self
                .studio
                .stroke
                .iter()
                .rposition(|p| *p == a)
                .unwrap_or(0);
            let points = self.studio.stroke[start..].to_vec();
            for p in points {
                self.paint_at(p, color)
            }
        }
    }
    pub(super) fn gradient(&mut self, a: (i32, i32), b: (i32, i32)) {
        let dx = (b.0 - a.0) as f32;
        let dy = (b.1 - a.1) as f32;
        let len = dx * dx + dy * dy;
        if len == 0. {
            return;
        }
        for i in 0..self.doc.width * self.doc.height {
            let (x, y) = ((i % self.doc.width) as i32, (i / self.doc.width) as i32);
            if !self.selected(x, y) {
                continue;
            }
            let t = (((x - a.0) as f32 * dx + (y - a.1) as f32 * dy) / len).clamp(0., 1.);
            let p = std::array::from_fn(|c| {
                (self.fg[c] as f32 * (1. - t) + self.bg[c] as f32 * t).round() as u8
            });
            self.doc.set_pixel(self.layer, self.frame, i, p);
        }
    }
}

const RECOVERY_FILES: [&str; 2] = ["workspace.recovery", "workspace.recovery.previous"];

fn read_recovery(root: &std::path::Path) -> Result<Option<Workspace>, String> {
    let mut failure = None;
    for name in RECOVERY_FILES {
        let file = match std::fs::File::open(root.join(name)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                failure = Some(error.to_string());
                continue;
            }
        };
        match decode_recovery(file) {
            Ok(state) => return Ok(Some(state)),
            Err(error) => {
                if let Ok(file) = std::fs::File::open(root.join(name))
                    && let Ok(state) = recovery_images::read_partial_workspace(file)
                {
                    return Ok(Some(state));
                }
                failure = Some(error);
            }
        }
    }
    failure.map_or(Ok(None), Err)
}

fn decode_recovery(file: std::fs::File) -> Result<Workspace, String> {
    let input = std::io::BufReader::with_capacity(64 * 1024, file);
    let decoded = flate2::read::ZlibDecoder::new(input).take(1_500_000_000);
    let buffered = std::io::BufReader::with_capacity(64 * 1024, decoded);
    let state: Workspace = serde_json::from_reader(buffered).map_err(|error| error.to_string())?;
    if state.version != 1
        || state.tabs.is_empty()
        || state.active >= state.tabs.len()
        || state.colors.is_empty()
        || state.color >= state.colors.len()
    {
        return Err("Invalid workspace recovery".into());
    }
    for tab in &state.tabs {
        validate_recovery_tab(tab)?;
    }
    Ok(state)
}

fn validate_recovery_tab(tab: &StoredTab) -> Result<(), String> {
    tab.doc.validate()?;
    tab.saved.validate()?;
    if tab.layer >= tab.doc.layers.len()
        || tab.frame >= tab.doc.durations.len()
        || !tab.zoom.is_finite()
        || !tab.pan.iter().all(|value| value.is_finite())
    {
        return Err("Invalid recovery tab or memory limit".into());
    }
    Ok(())
}

fn discard_recovery(root: &std::path::Path) -> Result<(), std::io::Error> {
    for name in RECOVERY_FILES.into_iter().rev() {
        match std::fs::remove_file(root.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
fn write_recovery(root: &std::path::Path, state: &Workspace) -> Result<(), String> {
    write_recovery_snapshot(root, state, false)
}

fn write_recovery_snapshot(
    root: &std::path::Path,
    state: &Workspace,
    reset_history: bool,
) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut buffered = std::io::BufWriter::with_capacity(64 * 1024, &mut z);
    serde_json::to_writer(&mut buffered, state).map_err(|e| e.to_string())?;
    buffered.flush().map_err(|e| e.to_string())?;
    drop(buffered);
    let bytes = z.finish().map_err(|e| e.to_string())?;
    let file = root.join("workspace.recovery");
    let tmp = root.join("workspace.recovery.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    let previous = root.join(RECOVERY_FILES[1]);
    if reset_history {
        match std::fs::remove_file(&previous) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    } else if file.exists() {
        std::fs::copy(&file, previous).map_err(|error| error.to_string())?;
    }
    std::fs::rename(tmp, file).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_indices_use_active_geometry_and_canvas_clip() {
        let mut editor = app();
        editor.doc = Document::new(3, 1);
        editor.doc.tilesets.push(Tileset {
            name: "Numbers".into(),
            width: 1,
            height: 1,
            tiles: vec![vec![[45, 12, 77, 255]].into(); 2],
        });
        editor.doc.add_tile_layer(0).unwrap();
        editor.layer = 1;
        editor.zoom = 32.;
        editor.studio.show_tile_indices = true;
        editor.doc.layers[1].cel_meta[0].tilemap = Some(TilemapGeometry {
            x: -1,
            y: 0,
            columns: 3,
            rows: 1,
        });
        editor.doc.layers[1].tile_cels[0] = vec![1, 2, 0];
        let ctx = egui::Context::default();
        let board = Rect::from_min_size(Pos2::ZERO, Vec2::new(96., 32.));
        let shown = |editor: &PixelCraft| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(board),
                    ..Default::default()
                },
                |ui| {
                    let painter = ui.painter().with_clip_rect(board);
                    editor.paint_tile_indices(&painter, board);
                },
            );
            output.textures_delta.clear();
            output
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&editor), ["2"]);
        editor.studio.show_tile_indices = false;
        assert!(shown(&editor).is_empty());
    }

    #[test]
    fn layer_conversion_actions_use_the_secondary_color_and_are_undoable() {
        let mut editor = app();
        let mut doc = Document::new(2, 2);
        doc.add_layer();
        doc.layers[1].name = "Paper".into();
        editor.replace(doc.clone(), None);
        editor.layer = 1;
        editor.bg = [24, 68, 93, 150];
        assert!(editor.studio_action("convert-background"));

        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while editor.studio.sprite_job.is_some() {
            assert!(Instant::now() < deadline);
            editor.poll_sprite_job(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(editor.layer, 0);
        assert!(editor.doc.layers[0].background);
        assert_eq!(editor.doc.composite(0)[3], [24, 68, 93, 255]);
        editor.undo();
        assert_eq!(editor.layer, 1);
        assert_eq!(editor.doc, doc);
        assert!(editor.studio_action("convert-tilemap"));
        let dialog = editor.studio.tile_conversion.as_ref().unwrap();
        assert_eq!(dialog.layer, editor.doc.layers[1].editor_id.to_string());
        assert_eq!([dialog.width, dialog.height], [editor.tile_size; 2]);
    }

    #[test]
    fn layer_commands_select_the_result_and_keep_each_edit_undoable() {
        let mut editor = app();
        let mut doc = Document::new(2, 2);
        doc.set_pixel(0, 0, 0, [30, 42, 74, 255]);
        editor.replace(doc.clone(), None);
        let ctx = egui::Context::default();

        for (action, layer_count, active) in [("layer-duplicate", 2, 1), ("layer-merge", 1, 0)] {
            assert!(editor.studio_action(action));
            let deadline = Instant::now() + Duration::from_secs(10);
            while editor.studio.sprite_job.is_some() {
                assert!(Instant::now() < deadline);
                editor.poll_sprite_job(&ctx);
                std::thread::yield_now();
            }
            assert_eq!(editor.doc.layers.len(), layer_count);
            assert_eq!(editor.layer, active);
        }
        assert_eq!(editor.doc.composite(0), doc.composite(0));
        editor.undo();
        assert_eq!(editor.doc.layers.len(), 2);
        assert_eq!(editor.layer, 1);
        editor.undo();
        assert_eq!(editor.doc, doc);
        assert_eq!(editor.layer, 0);
    }

    #[test]
    fn whole_sprite_actions_apply_as_one_edit_and_duplicate_in_a_new_tab() {
        let mut app = app();
        let mut source = Document::new(2, 3);
        let ink = [190, 75, 20, 255];
        source.set_pixel(0, 0, 0, ink);
        app.replace(source.clone(), None);
        assert!(app.studio_action("rotatespriteright"));
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.studio.sprite_job.is_some() {
            assert!(Instant::now() < deadline);
            app.poll_sprite_job(&ctx);
            std::thread::yield_now();
        }
        assert_eq!([app.doc.width, app.doc.height], [3, 2]);
        assert_eq!(app.doc.composite(0)[2], ink);
        app.undo();
        assert_eq!(app.doc, source);
        assert!(app.studio_action("duplicatesprite"));
        assert_eq!(app.tabs.len(), 2);
        assert!(app.dirty());
        assert!(!app.doc.same_editor_targets(&source));
        app.doc.set_pixel(0, 0, 0, CLEAR);
        app.switch_tab(0);
        assert_eq!(app.doc.composite(0)[0], ink);
    }

    #[test]
    fn background_sprite_transform_keeps_edits_made_while_it_runs() {
        let mut app = app();
        app.replace(Document::new(2, 3), None);
        assert!(app.studio_action("rotatespriteright"));
        let ink = [190, 75, 20, 255];
        app.doc.set_pixel(0, 0, 0, ink);
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.studio.sprite_job.is_some() {
            assert!(Instant::now() < deadline);
            app.poll_sprite_job(&ctx);
            std::thread::yield_now();
        }
        assert_eq!([app.doc.width, app.doc.height], [2, 3]);
        assert_eq!(app.doc.composite(0)[0], ink);
    }

    #[test]
    fn canvas_size_action_uses_an_anchor_and_undo_restores_the_sprite() {
        let mut editor = app();
        let mut source = Document::new(2, 2);
        let ink = [109, 48, 28, 255];
        source.set_pixel(0, 0, 0, ink);
        editor.replace(source.clone(), None);
        assert!(editor.studio_action("resizecanvas"));
        let mut resize = editor.studio.canvas_resize.take().unwrap();
        resize.size = [6, 4];
        resize.anchor = 4;
        editor.apply_canvas_resize(&resize).unwrap();
        assert_eq!([editor.doc.width, editor.doc.height], [6, 4]);
        assert_eq!(editor.doc.composite(0)[8], ink);
        assert_eq!(editor.doc.composite(0)[0], CLEAR);
        editor.undo();
        assert_eq!(editor.doc, source);
        editor.redo();
        assert_eq!(editor.doc.composite(0)[8], ink);
    }

    #[test]
    fn rotation_mode_applies_to_the_whole_sprite_without_a_selection() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        let mut source = Document::new(3, 3);
        let ink = [240, 100, 60, 255];
        for index in [0, 2, 3, 4, 5] {
            source.set_pixel(0, 0, index, ink);
        }
        let turn = Transform::RotateAround {
            degrees: 30.,
            pivot: [1.5, 1.5],
        };
        a.replace(source.clone(), None);
        a.transform_selection(turn);
        assert_eq!(a.doc.layers[0].cels[0][1], CLEAR);
        a.undo();
        assert!(a.doc == source);
        assert!(a.selection.is_none());

        a.studio.rotation_rotsprite = true;
        a.transform_selection(turn);
        assert_eq!(a.doc.layers[0].cels[0][1], ink);
        assert!(a.selected(1, 0));
        a.undo();
        assert!(a.doc == source);
        assert!(a.selection.is_none());
    }

    #[test]
    fn rotsprite_preview_reuses_its_texture_until_rotation_changes() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.replace(Document::new(8, 8), None);
        a.doc.set_pixel(0, 0, 23, [240, 100, 60, 255]);
        a.studio.open = true;
        a.studio.rotation_preview = true;
        a.studio.rotation_rotsprite = true;
        a.studio.rotation_pivot = Some([4., 4.]);
        a.studio.rotation_angle = 30.;
        let preview = |a: &mut PixelCraft| {
            let mut texture = None;
            let mut rendered = ctx.run_ui(egui::RawInput::default(), |ui| {
                texture = a.rotation_preview(ui, Rect::from_min_size(Pos2::ZERO, Vec2::splat(80.)));
            });
            rendered.textures_delta.clear();
            texture.unwrap()
        };
        let first = preview(&mut a);
        let reused = preview(&mut a);
        assert_eq!(first.id(), reused.id());
        a.studio.rotation_angle = 40.;
        assert_ne!(first.id(), preview(&mut a).id());
    }
    fn app() -> PixelCraft {
        PixelCraft::with_context(&egui::Context::default(), None)
    }
    #[test]
    fn periodic_recovery_does_not_block_the_editor() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-background-recovery-{}",
            std::process::id()
        ));
        let mut a = app();
        a.replace(Document::new(768, 384), None);
        for _ in 0..7 {
            a.doc.add_layer();
        }
        for frame in 0..5 {
            a.doc.add_frame(frame, true);
        }
        a.studio.recovery_enabled = true;
        a.studio.recovery_root = root.clone();
        a.studio.last_save = Instant::now() - Duration::from_secs(16);
        let start = Instant::now();
        let ctx = egui::Context::default();
        a.autosave(false, &ctx);
        let elapsed = start.elapsed();
        eprintln!("Periodic recovery UI time: {elapsed:?}");
        assert!(
            elapsed < Duration::from_millis(250),
            "recovery blocked the UI for {elapsed:?}"
        );
        a.autosave(true, &ctx);
        assert!(root.join("workspace.recovery").exists());
        std::fs::remove_file(root.join("workspace.recovery")).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
    fn wait_for_recovery(editor: &mut PixelCraft) {
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while editor.studio.restore_job.is_some() {
            assert!(
                Instant::now() < deadline,
                "Workspace recovery did not finish"
            );
            editor.poll_workspace_restore(&ctx);
            std::thread::yield_now();
        }
        assert!(
            editor.studio.restore_candidate.is_some(),
            "{}",
            editor.status
        );
    }

    #[test]
    fn startup_recovery_waits_for_a_decision_and_keeps_existing_artwork() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-startup-recovery-{}",
            std::process::id()
        ));
        let mut previous = app();
        let ink = [12, 34, 56, 255];
        previous.doc.set_pixel(1, 0, 0, ink);
        write_recovery(&root, &previous.workspace()).unwrap();

        let mut editor = app();
        editor.studio.recovery_enabled = true;
        editor.studio.recovery_root = root.clone();
        editor.open_document(Document::new(2, 3), Some(root.join("current.pixel")));
        editor.doc.set_pixel(0, 0, 0, ink);
        editor.restore_workspace();
        wait_for_recovery(&mut editor);

        assert_eq!((editor.doc.width, editor.doc.height), (2, 3));
        assert!(editor.dirty());
        assert_eq!(editor.doc.layers[0].cels[0][0], ink);
        editor.autosave(true, &egui::Context::default());
        let backup = read_recovery(&root).unwrap().unwrap();
        assert_eq!(backup.tabs.len(), 1);
        assert_eq!(backup.tabs[0].doc.layers[1].cels[0][0], ink);

        let current_tab = editor.active_tab;
        editor.apply_workspace_restore(None);
        assert_eq!(editor.tabs.len(), 3);
        assert_eq!(editor.doc.layers[1].cels[0][0], ink);
        editor.switch_tab(current_tab);
        assert_eq!((editor.doc.width, editor.doc.height), (2, 3));
        assert!(editor.dirty());
        assert_eq!(editor.doc.layers[0].cels[0][0], ink);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_salvages_healthy_rasters_when_other_backup_records_are_damaged() {
        let root = std::env::temp_dir().join(format!(
            "pixelcraft-damaged-tab-recovery-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let ink = [31, 55, 112, 255];
        let mut previous = app();
        previous.replace(Document::new(2, 2), Some(PathBuf::from("first.pixel")));
        previous.doc.set_pixel(0, 0, 0, ink);
        previous.open_document(Document::new(3, 3), Some(PathBuf::from("second.pixel")));
        let mut encoded = serde_json::to_value(previous.workspace()).unwrap();
        encoded["tabs"][0]["saved"] = serde_json::json!("damaged baseline");
        encoded["tabs"][0]["zoom"] = serde_json::json!("damaged view");
        encoded["tabs"][1]["doc"] = serde_json::json!({"document": "damaged raster record"});
        let file = std::fs::File::create(root.join(RECOVERY_FILES[0])).unwrap();
        let mut stream = flate2::write::ZlibEncoder::new(file, flate2::Compression::fast());
        serde_json::to_writer(&mut stream, &encoded).unwrap();
        stream.finish().unwrap();

        let result = read_recovery(&root);
        std::fs::remove_dir_all(&root).unwrap();
        let recovered = result.unwrap().unwrap();
        assert_eq!(recovered.tabs.len(), 1);
        assert!(recovered.tabs[0].raw_recovery);
        assert_eq!(recovered.tabs[0].doc.layers[0].cels[0][0], ink);
        assert_eq!(recovered.active, 0);
    }

    #[test]
    fn recovering_raw_images_opens_an_unsaved_copy_and_keeps_the_backup() {
        let mut previous = app();
        let path = PathBuf::from("original.pixel");
        previous.replace(Document::new(2, 2), Some(path.clone()));
        let ink = [110, 88, 55, 255];
        previous.doc.set_pixel(0, 0, 0, ink);
        let mut editor = app();
        editor.studio.restore_candidate = Some(previous.workspace());
        editor.start_recovery_salvage(0, recovery_images::Layout::Frames);
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while editor.studio.salvage_job.is_some() {
            assert!(Instant::now() < deadline);
            editor.poll_recovery_salvage(&ctx);
            std::thread::yield_now();
        }

        assert_eq!(editor.tabs.len(), 2);
        assert!(editor.path.is_none());
        assert!(editor.dirty());
        assert_eq!(editor.doc.layers[0].cels[0][0], ink);
        let backup = &editor.studio.restore_candidate.as_ref().unwrap().tabs[0];
        assert_eq!(backup.path.as_ref(), Some(&path));
        assert_eq!(backup.doc.layers[0].cels[0][0], ink);
    }

    #[test]
    fn recovery_restores_one_document_and_keeps_other_backups_pending() {
        let mut previous = app();
        previous.path = Some(PathBuf::from("first.pixel"));
        previous.open_document(Document::new(2, 2), Some(PathBuf::from("second.pixel")));
        let mut editor = app();
        editor.studio.restore_candidate = Some(previous.workspace());
        editor.apply_workspace_restore(Some(0));

        assert_eq!(
            editor.path.as_deref(),
            Some(std::path::Path::new("first.pixel"))
        );
        assert_eq!(editor.tabs.len(), 1);
        let pending = editor.studio.restore_candidate.as_ref().unwrap();
        assert_eq!(pending.tabs.len(), 1);
        assert_eq!(
            pending.tabs[0].path.as_deref(),
            Some(std::path::Path::new("second.pixel"))
        );

        editor.apply_workspace_restore(Some(0));
        assert_eq!(editor.tabs.len(), 2);
        assert_eq!(
            editor.path.as_deref(),
            Some(std::path::Path::new("second.pixel"))
        );
        assert!(editor.studio.restore_candidate.is_none());
    }

    #[test]
    fn discarding_one_recovery_keeps_remaining_backups_and_current_work_on_disk() {
        use std::path::Path;

        let root = std::env::temp_dir().join(format!(
            "pixelcraft-discard-recovery-{}",
            std::process::id()
        ));
        let removed = root.join("discarded.pixel");
        let kept = root.join("kept.pixel");
        let current = root.join("current.pixel");
        let mut previous = app();
        previous.path = Some(removed.clone());
        previous.open_document(Document::new(2, 2), Some(kept.clone()));
        let backup = previous.workspace();
        write_recovery(&root, &backup).unwrap();
        write_recovery(&root, &backup).unwrap();

        let ctx = egui::Context::default();
        let mut editor = app();
        editor.path = Some(current.clone());
        editor.studio.recovery_root = root.clone();
        editor.studio.recovery_enabled = true;
        editor.studio.restore_candidate = Some(backup);
        editor.discard_workspace_backup(Some(0), &ctx);
        editor.autosave(true, &ctx);

        let snapshot = read_recovery(&root).unwrap().unwrap();
        assert_eq!(
            snapshot
                .tabs
                .iter()
                .filter_map(|tab| tab.path.as_deref())
                .collect::<Vec<&Path>>(),
            [current.as_path(), kept.as_path()]
        );
        assert!(
            !root.join(RECOVERY_FILES[1]).exists(),
            "old generation contained the discarded document"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_restores_dirty_tabs_saved_baselines_and_active_view() {
        let root = std::env::temp_dir().join(format!("pixelcraft-recovery-{}", std::process::id()));
        let mut a = app();
        a.studio.recovery_enabled = true;
        a.studio.recovery_root = root.clone();
        a.doc.set_pixel(1, 0, 0, [11, 22, 33, 255]);
        let first_project = root.join("first");
        let second_project = root.join("second");
        a.path = Some(first_project.join("art.pixel"));
        a.live_save_root = Some(first_project.clone());
        a.open_document(Document::new(3, 2), Some(second_project.join("art.pixel")));
        a.live_save_root = Some(second_project.clone());
        a.doc.set_pixel(0, 0, 5, [99, 88, 77, 255]);
        a.named_colors.push(("Grass".into(), [1, 2, 3, 255]));
        a.zoom = 7.;
        a.pan = Vec2::new(10., 20.);
        a.autosave(true, &egui::Context::default());
        assert!(root.join("workspace.recovery").exists());
        let mut b = app();
        b.studio.recovery_enabled = true;
        b.studio.recovery_root = root.clone();
        b.restore_workspace();
        wait_for_recovery(&mut b);
        b.apply_workspace_restore(None);
        assert_eq!(b.tabs.len(), 2);
        assert_eq!(b.active_tab, 1);
        assert_eq!(b.live_save_root.as_ref(), Some(&second_project));
        assert!(b.dirty());
        assert_eq!(b.doc.layers[0].cels[0][5], [99, 88, 77, 255]);
        assert_eq!(b.zoom, 7.);
        assert_eq!(b.pan, Vec2::new(10., 20.));
        b.switch_tab(0);
        assert_eq!(b.live_save_root.as_ref(), Some(&first_project));
        assert!(b.dirty());
        assert_eq!(b.doc.layers[1].cels[0][0], [11, 22, 33, 255]);
        assert_eq!(b.named_colors[1].0, "Grass");
        a.doc.set_pixel(0, 0, 1, [1, 1, 1, 255]);
        a.autosave(true, &egui::Context::default());
        std::fs::write(root.join("workspace.recovery"), b"corrupt").unwrap();
        let fallback = read_recovery(&root).unwrap().unwrap();
        assert_eq!(fallback.active, 1);
        assert_eq!(fallback.tabs[1].doc.layers[0].cels[0][1], CLEAR);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn symmetry_and_selection_clipping_are_undoable() {
        let mut a = app();
        a.replace(Document::new(5, 5), None);
        a.studio.mirror_x = true;
        a.studio.mirror_y = true;
        a.remember();
        a.paint_at((1, 1), [255, 0, 0, 255]);
        for i in [6, 8, 16, 18] {
            assert_eq!(a.doc.layers[0].cels[0][i], [255, 0, 0, 255]);
        }
        a.undo();
        assert!(a.doc.layers[0].cels[0].iter().all(|p| *p == CLEAR));
        a.selection = Some((1, 1, 1, 1));
        a.brush = 3;
        a.paint_at((1, 1), [255, 0, 0, 255]);
        assert_eq!(
            a.doc.layers[0].cels[0].iter().filter(|p| p[3] > 0).count(),
            1
        );
    }
    #[test]
    fn pixel_perfect_removes_an_orthogonal_corner() {
        let mut a = app();
        a.replace(Document::new(4, 4), None);
        a.studio.pixel_perfect = true;
        a.stroke_base = Some(a.doc.clone());
        let c = [255, 0, 0, 255];
        a.paint_at((0, 0), c);
        a.stroke_line((0, 0), (1, 0), c);
        a.stroke_line((1, 0), (1, 1), c);
        assert_eq!(a.doc.layers[0].cels[0][1], CLEAR);
        assert_eq!(a.doc.layers[0].cels[0][5], c);
    }
    #[test]
    fn tileset_atlas_gui_exports_selected_layers_on_the_worker() {
        let mut editor = app();
        editor.replace(Document::new(1, 1), None);
        let visible = [190, 37, 31, 255];
        let hidden = [10, 106, 205, 255];
        for (index, color) in [visible, hidden].into_iter().enumerate() {
            editor.doc.tilesets.push(Tileset {
                name: index.to_string(),
                width: 1,
                height: 1,
                tiles: vec![vec![color].into()],
            });
            editor.doc.add_tile_layer(index).unwrap();
        }
        editor.doc.layers[2].visible = false;
        editor.timeline_layers.selected = vec![2];
        editor.action("tileset-atlas");
        let options = editor.studio.tileset_atlas_export.unwrap();
        assert!(!options.visible);
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-gui-tileset-atlas-{}.png",
            std::process::id()
        ));
        editor.start_tileset_atlas_export(path.clone(), options);
        let (output, result) = editor
            .export_job
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        result.unwrap();
        assert_eq!(output, path);
        let image = image::open(&path).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (1, 1));
        assert_eq!(image.get_pixel(0, 0).0, hidden);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn sprite_sheet_worker_exports_timeline_groups_and_frame_range() {
        use std::time::Duration;

        let ctx = egui::Context::default();
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.doc = Document::new(2, 1);
        let group = editor.doc.add_group("Actor").unwrap();
        editor.doc.add_layer();
        let child = editor.doc.layers.len() - 1;
        editor.doc.layers[child].parent = Some(group);
        editor.doc.add_frame(0, false);
        let background = [220, 90, 40, 255];
        let foreground = [40, 90, 220, 255];
        editor.doc.set_pixel(0, 1, 0, background);
        editor.doc.set_pixel(child, 1, 1, foreground);
        editor.studio.sheet_use_timeline_layers = true;
        editor.timeline_layers.selected = vec![group];
        editor.studio.sheet.frame_range = Some([1, 1]);
        editor.studio.sheet.padding = 0;

        let path = std::env::temp_dir().join(format!(
            "pixelcraft-gui-filtered-sheet-{}.png",
            std::process::id()
        ));
        editor.start_sprite_sheet_export(path.clone());
        let (output, result) = editor
            .export_job
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        result.unwrap();
        assert_eq!(output, path);
        let image = image::open(&path).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, CLEAR);
        assert_eq!(image.get_pixel(1, 0).0, foreground);
        assert_eq!(editor.doc.composite(1)[0], background);
        let metadata = path.with_extension("json");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(metadata).unwrap();
    }

    #[test]
    fn sprite_sheet_worker_crops_to_the_selected_named_slice() {
        use std::time::Duration;

        let ctx = egui::Context::default();
        let mut editor = PixelCraft::with_context(&ctx, None);
        editor.doc = Document::new(2, 1);
        let ink = [40, 90, 220, 255];
        editor.doc.set_pixel(0, 0, 1, ink);
        editor.doc.slices.push(SpriteSlice {
            name: "Right".into(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: [1, 0, 1, 1],
                center: None,
                pivot: None,
            }],
        });
        editor.studio.sheet_slice = Some("Right".into());
        editor.studio.sheet.padding = 0;
        editor.studio.sheet.split_layers = true;
        editor.studio.sheet.filename_template = "{slice}-{frame_id}".into();
        assert_eq!(
            interchange::split_sheet_sample_count(
                &editor.doc,
                &editor.sprite_sheet_options(),
                Some("Right")
            )
            .unwrap(),
            1
        );
        let path = std::env::temp_dir().join(format!(
            "pixelcraft-gui-named-slice-sheet-{}.png",
            std::process::id()
        ));
        editor.start_sprite_sheet_export(path.clone());
        let (_, result) = editor
            .export_job
            .take()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        result.unwrap();
        let image = image::open(&path).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (1, 1));
        assert_eq!(image.get_pixel(0, 0).0, ink);
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap()).unwrap();
        assert_eq!(meta["frames"][0]["filename"], "Right-0");
        std::fs::remove_file(path.with_extension("json")).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn advanced_dialogs_render() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.studio.open = true;
        a.studio.sheet_open = true;
        a.studio.tiles_open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| a.studio_dialogs(ui.ctx()));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
}
