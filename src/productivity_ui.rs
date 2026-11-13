use super::*;
use pixelcraft::{
    atlas::*,
    automation::{self, LiveBridge},
    productivity::*,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    recent: Vec<PathBuf>,
    onion: OnionOptions,
    preset: String,
    interface_scale: f32,
    comfortable: bool,
    checker_size: usize,
    checker_colors: Option<[[u8; 3]; 2]>,
    autosave: bool,
    sample_references: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            recent: vec![],
            onion: OnionOptions::default(),
            preset: String::new(),
            interface_scale: 1.,
            comfortable: false,
            checker_size: 8,
            checker_colors: None,
            autosave: true,
            sample_references: false,
        }
    }
}
pub(super) struct PowerTools {
    pub open: bool,
    pub mode: BrushMode,
    pub reverse: bool,
    pub stamp: Option<(usize, usize, Vec<Pixel>)>,
    pub contiguous: bool,
    pub tolerance: u8,
    pub selection_op: SelectionOp,
    preferences: Preferences,
    grid: GridImport,
    grid_source: Option<image::RgbaImage>,
    grid_texture: Option<egui::TextureHandle>,
    atlas: AtlasOptions,
    atlas_files: Vec<PathBuf>,
    pub reference_edit: Option<usize>,
    reference_textures: Vec<(Pixels, egui::TextureHandle)>,
    reference_drag: Option<(f32, f32, f32, Pos2)>,
    pub bridge: Option<LiveBridge>,
    bridge_snapshot: Option<Document>,
    pub bridge_generation: u64,
    bridge_tab: usize,
    bridge_session: String,
    bridge_work: Option<std::sync::mpsc::Receiver<PreparedLive>>,
    pub shade_seen: std::collections::HashSet<usize>,
    ramp_from: usize,
    ramp_to: usize,
    ramp_steps: usize,
    shortcut_key: String,
    shortcut_action: String,
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
            contiguous: true,
            tolerance: 0,
            selection_op: SelectionOp::Replace,
            preferences,
            grid: GridImport::default(),
            grid_source: None,
            grid_texture: None,
            atlas: AtlasOptions::default(),
            atlas_files: vec![],
            reference_edit: None,
            reference_textures: vec![],
            reference_drag: None,
            bridge: None,
            bridge_snapshot: None,
            bridge_generation: 0,
            bridge_tab: 0,
            bridge_session: format!("{}-{:?}", std::process::id(), std::time::SystemTime::now()),
            bridge_work: None,
            shade_seen: Default::default(),
            ramp_from: 0,
            ramp_to: 1,
            ramp_steps: 5,
            shortcut_key: "W".into(),
            shortcut_action: "wand".into(),
        }
    }
}

enum LiveResult {
    Read(serde_json::Value),
    Export(automation::PreparedExport),
    Save(automation::PreparedExport, Document),
    Edit(Document, serde_json::Value),
}

struct PreparedLive {
    request: automation::LiveRequest,
    revision: String,
    result: Result<LiveResult, String>,
}
impl PowerTools {
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

    pub(super) fn preferences_onion(&self) -> OnionOptions {
        self.preferences.onion.clone()
    }
}
impl PixelCraft {
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
    fn settings_ui(&mut self, ui: &mut egui::Ui) {
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
            ui.pc_checkbox(&mut p.autosave, "Enable recovery autosave");
            if before != serde_json::to_value(&*p).ok() {
                ui.ctx().set_zoom_factor(self.power.interface_scale());
                self.save_workflow_preferences();
            }
        });
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
                let mut next = self.doc.clone();
                match next.reorder_palette(self.power.ramp_from, self.power.ramp_to) {
                    Ok(()) => {
                        self.remember();
                        self.doc = next;
                    }
                    Err(e) => self.status = e,
                }
            }
            ui.add(
                egui::DragValue::new(&mut self.power.ramp_steps)
                    .range(3..=256)
                    .prefix("Ramp steps (including endpoints) "),
            );
            if ui.pc_button("Add ramp between swatches").clicked() {
                let mut next = self.doc.clone();
                match next.palette_ramp(
                    self.power.ramp_from,
                    self.power.ramp_to,
                    self.power.ramp_steps,
                ) {
                    Ok(()) => {
                        self.remember();
                        self.doc = next;
                    }
                    Err(e) => self.status = e,
                }
            }
            ui.small("Swatch numbers start at 0. Existing indexed artwork keeps its colors.");
        });
    }
    pub(super) fn recent_files(&self) -> Vec<PathBuf> {
        self.power.preferences.recent.clone()
    }
    pub(super) fn open_recent(&mut self, p: PathBuf) {
        if self.open_job.is_some() {
            self.status = "A file is already opening".into();
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.status = format!(
            "Opening {}",
            p.file_name().unwrap_or_default().to_string_lossy()
        );
        self.open_job = Some(rx);
        std::thread::spawn(move || {
            let result = formats::load(&p);
            let _ = tx.send((p, result));
        });
    }

    pub(super) fn poll_open_document(&mut self) {
        if self.start.is_some() || self.script_job.is_some() {
            return;
        }
        let Some(job) = &self.open_job else { return };
        let (p, result) = match job.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.open_job = None;
                self.status = "File loading worker stopped unexpectedly".into();
                return;
            }
        };
        self.open_job = None;
        match result {
            Ok(d) => {
                self.add_recent(p.clone());
                let editable = matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("pixel" | "ase" | "aseprite")
                );
                self.open_document(d, editable.then_some(p));
                if std::mem::take(&mut self.play_after_open) {
                    self.playing = true;
                    self.last_tick = Instant::now();
                }
                self.status = "Opened sprite".into();
            }
            Err(e) => {
                self.play_after_open = false;
                self.status = format!("Could not open: {e}");
            }
        }
    }
    pub(super) fn draw_selection_mask(&self, painter: &egui::Painter, board: Rect, view: Rect) {
        let Some(mask) = &self.studio.mask else {
            return;
        };
        let step = (1. / self.zoom).ceil().max(1.) as i32;
        let x0 = (((view.min.x - board.min.x) / self.zoom).floor() as i32)
            .clamp(0, self.doc.width as i32);
        let y0 = (((view.min.y - board.min.y) / self.zoom).floor() as i32)
            .clamp(0, self.doc.height as i32);
        let x1 = (((view.max.x - board.min.x) / self.zoom).ceil() as i32)
            .clamp(0, self.doc.width as i32);
        let y1 = (((view.max.y - board.min.y) / self.zoom).ceil() as i32)
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
                            board.min + Vec2::new(a.0 as f32, a.1 as f32) * self.zoom,
                            board.min + Vec2::new(b.0 as f32, b.1 as f32) * self.zoom,
                        ];
                        painter.line_segment(pts, Stroke::new(2., Color32::BLACK));
                        painter.line_segment(pts, Stroke::new(1., Color32::WHITE));
                    }
                }
            }
        }
    }
    fn save_workflow_preferences(&mut self) {
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
        let hits = matching(
            &self.doc.layer_pixels(self.layer, self.frame),
            self.doc.width,
            self.doc.height,
            p,
            self.power.tolerance,
            self.power.contiguous,
            self.doc.wraparound,
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
        let mask = self.selection.map(|_| {
            (0..self.doc.width * self.doc.height)
                .map(|i| self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32))
                .collect::<Vec<_>>()
        });
        let mode = if self.tool == Tool::Eraser {
            BrushMode::Normal
        } else {
            self.power.mode
        };
        let mut o = BrushOptions {
            mode,
            alternate: self.bg,
            reverse: self.power.reverse,
            stamp: self.power.stamp.clone(),
            mask,
        };
        if mode == BrushMode::Shade {
            let n = self.doc.width * self.doc.height;
            let mut allowed = o.mask.take().unwrap_or_else(|| vec![true; n]);
            for &i in &self.power.shade_seen {
                if let Some(hit) = allowed.get_mut(i) {
                    *hit = false;
                }
            }
            o.mask = Some(allowed);
            let before = self.doc.layers[self.layer].cels[self.frame].clone();
            paint_brush(&mut self.doc, self.layer, self.frame, p, color, size, &o);
            for (i, (a, b)) in before
                .iter()
                .zip(&self.doc.layers[self.layer].cels[self.frame])
                .enumerate()
            {
                if a != b {
                    self.power.shade_seen.insert(i);
                }
            }
        } else {
            paint_brush(&mut self.doc, self.layer, self.frame, p, color, size, &o);
        }
    }
    pub(super) fn power_fill(&mut self, p: (i32, i32), color: Pixel) {
        let color = self.doc.snapped_color(self.frame, color);
        let mut hits = matching(
            &self.doc.layer_pixels(self.layer, self.frame),
            self.doc.width,
            self.doc.height,
            p,
            0,
            true,
            self.doc.wraparound,
        );
        for (i, hit) in hits.iter_mut().enumerate() {
            if *hit && self.selected((i % self.doc.width) as i32, (i / self.doc.width) as i32) {
                self.doc.set_pixel(self.layer, self.frame, i, color);
            }
        }
    }
    fn capture_stamp(&mut self) {
        self.copy();
        if let Some(c) = &self.clipboard {
            self.power.stamp = Some((c.w, c.h, c.pixels.clone()));
            self.power.mode = BrushMode::Selection;
            self.selection = None;
            self.studio.mask = None;
            self.studio.polygon.clear();
            self.tool = Tool::Pencil;
            self.status = "Selection captured as a brush, including transparent pixels".into();
        }
    }
    pub(super) fn power_dialogs(&mut self, ctx: &egui::Context) {
        if !self.power.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Workflow tools").open(&mut open).default_width(530.).default_height(650.).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
 self.settings_ui(ui);
 self.palette_arrangement_ui(ui);
 ui.collapsing("Magic Wand & brushes",|ui|{if ui_icons::button(ui,"wand","Magic Wand",false,false).clicked(){self.tool=Tool::Wand;}ui.pc_checkbox(&mut self.power.contiguous,"Contiguous matching");ui.add(egui::Slider::new(&mut self.power.tolerance,0..=255).text("Color tolerance"));ui.horizontal(|ui|{for op in [SelectionOp::Replace,SelectionOp::Add,SelectionOp::Subtract]{ui.pc_selectable_value(&mut self.power.selection_op,op,format!("{op:?}"));}});ui.small("Shift-click adds; Ctrl/Cmd-click subtracts.");self.brush_mode_ui(ui);if ui.pc_button("Capture selection as stamp").clicked(){self.capture_stamp();}ui.pc_checkbox(&mut self.power.reverse,"Shade toward previous palette color");ui.small("Dither alternates active/alternate colors on the canvas grid. Shade changes each pixel once per stroke.");});
 ui.collapsing("Wraparound & palette",|ui|{let mut wrap=self.doc.wraparound;let mut snap=self.doc.palette_snap;ui.pc_checkbox(&mut wrap,"Wrap drawing & fill across canvas edges");ui.pc_checkbox(&mut snap,"Snap pencil, fill & shapes to palette in RGB mode");if wrap!=self.doc.wraparound||snap!=self.doc.palette_snap{self.remember();self.doc.wraparound=wrap;self.doc.palette_snap=snap;}ui.small("Wraparound drawing is independent of View → Seamless preview.");ui.horizontal(|ui|{if ui.pc_button("Import palette…").clicked(){self.palette_file(false);}
if ui.pc_button("Export palette…").clicked(){self.palette_file(true);}});ui.small("HEX supports RGBA. JASC PAL and GIMP GPL store RGB.");});
 ui.collapsing("Reference images",|ui|{if ui.pc_checkbox(&mut self.power.preferences.sample_references,"Sample reference colors").changed(){self.save_workflow_preferences();}
if ui.pc_button("Import PNG / JPEG overlay…").clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("Reference image",&["png","jpg","jpeg"]).pick_file(){match import_reference(&path){Ok(r)=>{let mut next=self.doc.clone();next.references.push(r);match next.validate(){Ok(())=>{self.remember();self.doc=next;},Err(e)=>self.status=e}},Err(e)=>self.status=e}}ui.small("Saved inside .pixel projects; excluded from artwork exports. Enable Move to drag; Shift-drag scales.");let mut remove=None;for i in 0..self.doc.references.len(){let mut r=self.doc.references[i].clone();ui.separator();ui.horizontal(|ui|{ui_icons::checkbox(ui,&mut r.visible,"eye",&r.name);if ui_icons::button_as(ui,"move","reference-move","Move / scale",self.power.reference_edit==Some(i),false).clicked(){self.power.reference_edit=if self.power.reference_edit==Some(i){None}else{Some(i)};}
if ui.pc_small_button("Remove").clicked(){remove=Some(i);}});ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut r.x).prefix("X ").range(-16384. ..=16384.));ui.add(egui::DragValue::new(&mut r.y).prefix("Y ").range(-16384. ..=16384.));ui.add(egui::DragValue::new(&mut r.scale).prefix("Scale ").speed(0.01).range(0.01..=1000.));});ui.add(egui::Slider::new(&mut r.opacity,0..=255).text("Opacity"));
ui.horizontal(|ui|{ui.pc_checkbox(&mut r.behind,"Behind artwork");ui.pc_checkbox(&mut r.floating,"Floating window");});
ui.horizontal(|ui|{if ui.pc_button("Fit to canvas").clicked(){r.scale=(self.doc.width as f32/r.width as f32).min(self.doc.height as f32/r.height as f32).clamp(0.01,1000.);r.x=(self.doc.width as f32-r.width as f32*r.scale)/2.;r.y=(self.doc.height as f32-r.height as f32*r.scale)/2.;}
if ui.pc_button("Copy to layer").clicked(){let mut d=self.doc.clone();match reference_to_layer(&mut d,&r,self.frame){Ok(())=>{self.remember();self.doc=d;self.layer=self.doc.layers.len()-1;},Err(e)=>self.status=e}}});if r!=self.doc.references[i]{self.remember();self.doc.references[i]=r;}}
if let Some(i)=remove{self.remember();self.doc.references.remove(i);self.power.reference_edit=None;}
if ui.pc_button("Finish moving references").clicked(){self.power.reference_edit=None;}});
 ui.collapsing("Onion skin",|ui|{ui.pc_checkbox(&mut self.onion,"Enable onion skin");let before=self.power.preferences.onion.clone();let o=&mut self.power.preferences.onion;ui.add(egui::Slider::new(&mut o.previous,0..=8).text("Previous frames"));ui.add(egui::Slider::new(&mut o.next,0..=8).text("Next frames"));ui.horizontal(|ui|{ui.label("Previous tint");ui.color_edit_button_srgb(&mut o.previous_tint);});ui.add(egui::Slider::new(&mut o.previous_opacity,0..=255).text("Previous opacity"));ui.horizontal(|ui|{ui.label("Next tint");ui.color_edit_button_srgb(&mut o.next_tint);});ui.add(egui::Slider::new(&mut o.next_opacity,0..=255).text("Next opacity"));if serde_json::to_value(before).ok()!=serde_json::to_value(&*o).ok(){self.save_workflow_preferences();}});
 ui.collapsing("Grid sprite-sheet import",|ui|{if ui.pc_button("Choose sprite sheet…").clicked()&& let Some(p)=rfd::FileDialog::new().add_filter("Image",&["png","jpg","jpeg"]).pick_file(){match read_image(&p){Ok(im)=>{self.power.grid_texture=Some(ctx.load_texture("grid-import",color_image(&im.pixels().map(|p|p.0).collect::<Vec<_>>(),im.width() as usize,im.height() as usize),egui::TextureOptions::NEAREST));self.power.grid_source=Some(im);},Err(e)=>self.status=e}}let g=&mut self.power.grid;ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut g.width).prefix("Cell W ").range(1..=16384));ui.add(egui::DragValue::new(&mut g.height).prefix("H ").range(1..=16384));});ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut g.offset_x).prefix("Offset X ").range(0..=16384));ui.add(egui::DragValue::new(&mut g.offset_y).prefix("Y ").range(0..=16384));});ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut g.spacing_x).prefix("Spacing X ").range(0..=16384));ui.add(egui::DragValue::new(&mut g.spacing_y).prefix("Y ").range(0..=16384));});ui.pc_checkbox(&mut g.layers,"Import as layers (otherwise frames)");if let(Some(im),Some(tex))=(&self.power.grid_source,&self.power.grid_texture){match grid_cells(im,g){Ok(cells)=>{ui.label(format!("{} complete cells · row-major order",cells.len()));let scale=(ui.available_width()/im.width() as f32).min(260./im.height() as f32).min(8.);let response=ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(im.width() as f32,im.height() as f32)*scale));let painter=ui.painter_at(response.rect);for(x,y)in cells.iter().take(4096){painter.rect_stroke(Rect::from_min_size(response.rect.min+Vec2::new(*x as f32,*y as f32)*scale,Vec2::new(g.width as f32,g.height as f32)*scale),0,Stroke::new(1.,Color32::from_rgb(255,200,60)),egui::StrokeKind::Inside);}
if ui.pc_button("Import cells into a new sprite").clicked(){match import_grid(im,g){Ok(d)=>{self.open_document(d,None);self.status="Imported sheet cells".into();},Err(e)=>self.status=e}}},Err(e)=>{ui.colored_label(Color32::RED,e);}}}});
 ui.collapsing("Batch export animation tags",|ui|{ui.small("One file per tag. Frame timing and forward/reverse/ping-pong order are preserved.");egui::ComboBox::from_label("Sheet engine preset").selected_text(&self.studio.sheet.preset).show_ui(ui,|ui|{for p in ["Generic","Godot","Unity"]{ui.pc_selectable_value(&mut self.studio.sheet.preset,p.into(),p);}});ui.horizontal(|ui|{for(gif,label)in [(true,"Export tag GIFs…"),(false,"Export tag sheets…")]{if ui.pc_button(label).clicked()&& let Some(dir)=rfd::FileDialog::new().pick_folder(){self.status=match batch_tags(&dir,&self.doc,gif,&self.studio.sheet){Ok(p)=>format!("Exported {} animation tags",p.len()),Err(e)=>format!("Export failed: {e}")};}}});});
 ui.collapsing("Multi-file atlas",|ui|{if ui.pc_button("Choose sprites…").clicked()&& let Some(paths)=rfd::FileDialog::new().add_filter("Sprites",ATLAS_EXTENSIONS).pick_files(){self.power.atlas_files=paths;}
if ui.pc_button("Add folder…").clicked()&& let Some(dir)=rfd::FileDialog::new().pick_folder(){match expand_atlas_inputs(&[dir]){Ok(paths)=>self.power.atlas_files.extend(paths),Err(e)=>self.status=e}}for p in &self.power.atlas_files{ui.small(p.display().to_string());}let o=&mut self.power.atlas;ui.horizontal(|ui|{ui.pc_selectable_value(&mut o.layout,AtlasLayout::Rows,"Rows");ui.pc_selectable_value(&mut o.layout,AtlasLayout::MaxRects,"MaxRects");});ui.add(egui::DragValue::new(&mut o.width).range(1..=16384).prefix("Maximum width "));ui.add(egui::DragValue::new(&mut o.padding).range(0..=256).prefix("Padding "));ui.pc_checkbox(&mut o.trim,"Trim transparent borders");ui.pc_checkbox(&mut o.deduplicate,"Remove duplicate frame pixels");ui.pc_checkbox(&mut o.power_of_two,"Power-of-two dimensions");if ui.pc_button("Pack & export PNG + JSON…").clicked()&& let Some(path)=rfd::FileDialog::new().add_filter("PNG atlas",&["png"]).set_file_name("atlas.png").save_file(){let result=(||{let mut sources=vec![];for p in &self.power.atlas_files{sources.push((p.file_name().unwrap_or_default().to_string_lossy().into_owned(),formats::load(p)?));}export_atlas(&path,&sources,&self.power.atlas)})();self.status=match result{Ok(())=>"Exported multi-file atlas".into(),Err(e)=>format!("Export failed: {e}")};}});
 ui.collapsing("Recent files & shortcuts",|ui|{let mut chosen=None;for p in &self.power.preferences.recent{if ui_icons::button(ui,"open",&p.display().to_string(),false,false).clicked(){chosen=Some(p.clone());}}
if let Some(p)=chosen{match formats::load(&p){Ok(d)=>{self.add_recent(p.clone());let editable=matches!(p.extension().and_then(|s|s.to_str()),Some("pixel"|"ase"|"aseprite"));self.open_document(d,editable.then_some(p));},Err(e)=>self.status=format!("Could not open: {e}")}}
if ui.pc_button("Clear recent files").clicked(){self.power.preferences.recent.clear();self.save_workflow_preferences();}ui.separator();ui.horizontal(|ui|{for name in ["PixelCraft","Aseprite","Photoshop"]{if ui.pc_button(name).clicked(){self.custom_shortcuts=shortcut_preset(name);self.power.preferences.preset=name.into();self.save_shortcut_bindings();}}});ui.label(format!("Preset: {} (custom bindings stay editable)",if self.power.preferences.preset.is_empty(){"PixelCraft"}else{&self.power.preferences.preset}));ui.horizontal(|ui|{ui.label("Key");ui.text_edit_singleline(&mut self.power.shortcut_key);});ui.horizontal(|ui|{ui.label("Command");egui::ComboBox::from_id_salt("shortcut-command").selected_text(&self.power.shortcut_action).show_ui(ui,|ui|{let actions:std::collections::BTreeSet<_>=shortcut_preset("PixelCraft").into_values().collect();for action in actions{ui.pc_selectable_value(&mut self.power.shortcut_action,action.clone(),action);}});if ui.pc_button("Bind").clicked(){let parts:Vec<_>=self.power.shortcut_key.split('+').collect();if parts.last().and_then(|k|Key::from_name(k)).is_some()&&parts[..parts.len()-1].iter().all(|p|["Ctrl","Cmd","Shift","Alt"].contains(p)){self.custom_shortcuts.insert(self.power.shortcut_key.clone(),self.power.shortcut_action.clone());self.save_shortcut_bindings();}else{self.status="Use a key such as W, Ctrl+S or Ctrl+Shift+Z".into();}}});let mut remove=None;for(k,a)in &self.custom_shortcuts{ui.horizontal(|ui|{ui.monospace(k);ui.label(a);if ui.pc_small_button("Remove").clicked(){remove=Some(k.clone());}});}
if let Some(k)=remove{self.custom_shortcuts.remove(&k);self.save_shortcut_bindings();}});
 ui.collapsing("Assistants & headless tools",|ui|{ui.label("The MCP server and CLI can work on files without opening the editor.");ui.monospace("pixelcraft --help\npixelcraft --mcp");let mut enabled=self.power.bridge.is_some();if ui.pc_checkbox(&mut enabled,"Allow assistants to access this live editor").changed(){if enabled{let config=std::env::temp_dir().join(format!("pixelcraft-live-{}.json",std::process::id()));let wake=ctx.clone();match LiveBridge::start_with_wake(config,move || wake.request_repaint()){Ok(b)=>{self.power.bridge=Some(b);self.power.bridge_generation+=1;self.status="Live editor access enabled. Edits are undoable.".into();},Err(e)=>self.status=format!("Could not enable bridge: {e}")}}else{self.power.bridge=None;self.power.bridge_work=None;self.status="Live editor access disabled".into();}}
if let Some(b)=&self.power.bridge{ui.label("Live access is active only while this checkbox is enabled.");ui.label("Use this MCP server command:");ui.monospace(format!("pixelcraft --mcp --live \"{}\"",b.config.display()));ui.small("The private bridge file contains an access token. Inspect returns a revision; stale edits are rejected. Ctrl+Z undoes assistant edits.");}});
 });});
        self.power.open = open;
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
                    self.remember();
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
    fn save_shortcut_bindings(&mut self) {
        let result = (|| {
            std::fs::create_dir_all(extensions::config_directory()).map_err(|e| e.to_string())?;
            std::fs::write(
                extensions::config_directory().join("shortcuts.json"),
                pixelcraft::productivity::save_shortcuts(&self.custom_shortcuts)?,
            )
            .map_err(|e| e.to_string())
        })();
        self.save_workflow_preferences();
        if let Err(e) = result {
            self.status = format!("Could not save shortcuts: {e}");
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
    }
    pub(super) fn sample_color(&self, x: i32, y: i32) -> Pixel {
        sample_canvas(
            &self.doc,
            self.frame,
            x,
            y,
            self.power.preferences.sample_references,
        )
    }
    pub(super) fn draw_references(&mut self, ui: &mut egui::Ui, board: Rect) {
        self.draw_references_pass(ui, board, false);
    }
    pub(super) fn draw_references_pass(&mut self, ui: &mut egui::Ui, board: Rect, behind: bool) {
        for i in 0..self.doc.references.len() {
            let r = self.doc.references[i].clone();
            if !r.visible
                || (if r.floating {
                    behind
                } else {
                    r.behind != behind
                })
            {
                continue;
            }
            let tex = if let Some((_, t)) = self
                .power
                .reference_textures
                .iter()
                .find(|(p, _)| p.shares_storage(&r.pixels))
            {
                t.clone()
            } else {
                let t = ui.ctx().load_texture(
                    format!("reference-{i}"),
                    color_image(&r.pixels, r.width, r.height),
                    egui::TextureOptions::LINEAR,
                );
                self.power
                    .reference_textures
                    .push((r.pixels.clone(), t.clone()));
                t
            };
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
                            let size = Vec2::new(r.width as f32, r.height as f32);
                            let scale = (ui.available_width() / size.x).min(450. / size.y);
                            let response = ui.add(
                                egui::Image::new(&tex)
                                    .fit_to_exact_size(size * scale)
                                    .sense(Sense::click()),
                            );
                            if response.clicked()
                                && self.power.preferences.sample_references
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let xy = (pos - response.rect.min) / scale;
                                let x = (xy.x as usize).min(r.width - 1);
                                let y = (xy.y as usize).min(r.height - 1);
                                self.set_drawing_color(over(
                                    CLEAR,
                                    r.pixels[y * r.width + x],
                                    r.opacity,
                                ));
                            }
                        });
                    if !open {
                        self.remember();
                        self.doc.references[i].floating = false;
                    }
                }
                continue;
            }
            let rect = Rect::from_min_size(
                board.min + Vec2::new(r.x, r.y) * self.zoom,
                Vec2::new(r.width as f32, r.height as f32) * r.scale * self.zoom,
            );
            ui.painter().image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::from_white_alpha(r.opacity),
            );
            if self.power.reference_edit == Some(i) {
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
                    self.remember();
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
                    let delta = (pointer - origin) / self.zoom;
                    let reference = &mut self.doc.references[i];
                    if ui.input(|i| i.modifiers.shift) {
                        reference.scale = (scale + delta.x / r.width as f32).clamp(0.01, 1000.);
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
                    self.finish_live_request(result);
                    ctx.request_repaint();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.power.bridge_work = None,
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
        if self.power.bridge_snapshot.as_ref() != Some(&self.doc)
            || self.power.bridge_tab != self.active_tab
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
        self.start.is_some()
            || self.script_job.is_some()
            || self.extension_session.is_some()
            || self.power.reference_drag.is_some()
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
        let error = if Instant::now() >= request.deadline {
            Some("Live request expired; no changes applied")
        } else if (editing || saving) && self.live_busy() {
            Some("Editor is busy; finish the active operation first")
        } else if saving && self.save_job.is_some() {
            Some("A save is already running")
        } else if op == "live-save" && self.path.is_none() {
            Some("The sprite is untitled. Use live-save-as with an output path.")
        } else if (editing
            || saving
            || op == "live-export"
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
        if matches!(op, "live-inspect" | "live-undo" | "live-redo") {
            if op == "live-undo" {
                self.undo();
            }
            if op == "live-redo" {
                self.redo();
            }
            self.sync_live_revision();
            let mut info = automation::inspect_revision(&self.doc, &self.live_revision());
            info["path"] = serde_json::json!(self.path);
            let _ = request.response.send(Ok(info));
            return None;
        }
        if op == "live-save" {
            request.arguments["output"] = serde_json::json!(self.path);
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
                    return automation::prepare_live_export(&doc, frame, args, &revision)
                        .map(LiveResult::Export);
                }
                "live-save" | "live-save-as" => {
                    let output = args["output"]
                        .as_str()
                        .filter(|p| !p.is_empty())
                        .ok_or("Missing output path")?;
                    let prepared = automation::prepare_live_save(
                        &doc,
                        PathBuf::from(output),
                        &revision,
                        request.operation == "live-save",
                    )?;
                    return Ok(LiveResult::Save(prepared, doc));
                }
                "live-commands" => {
                    automation::apply_command_batch_until(&doc, args, Some(request.deadline))?
                }
                "live-edit" => automation::apply_pixel_batch(&doc, args)?,
                "live-create" => {
                    let w = args["width"].as_u64().unwrap_or(0);
                    let h = args["height"].as_u64().unwrap_or(0);
                    if w == 0
                        || h == 0
                        || w > MAX_DIMENSION as u64
                        || h > MAX_DIMENSION as u64
                        || w * h > MAX_PIXELS as u64
                    {
                        return Err("Invalid canvas dimensions".into());
                    }
                    Document::new(w as usize, h as usize)
                }
                _ => return Err("Unknown live operation".into()),
            };
            let bounds = if request.operation == "live-create" {
                serde_json::json!([])
            } else {
                automation::changed_cel_bounds(&doc, &next)
            };
            Ok(LiveResult::Edit(next, bounds))
        })();
        PreparedLive {
            request,
            revision,
            result,
        }
    }

    fn finish_live_request(&mut self, prepared: PreparedLive) {
        let result = prepared.result.and_then(|result| {
            if Instant::now() >= prepared.request.deadline {
                return Err("Live request expired; no changes applied".into());
            }
            let (next, bounds) = match result {
                LiveResult::Read(value) => return Ok(value),
                LiveResult::Export(export) => return export.write(),
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
                    self.saved = snapshot;
                    self.add_recent(path.clone());
                    self.path = Some(path);
                    self.status = "Saved sprite through MCP".into();
                    return Ok(result);
                }
                LiveResult::Edit(next, bounds) => (next, bounds),
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
                self.push_undo(previous);
                self.status = "Assistant created an untitled sprite — Ctrl+Z to undo".into();
            } else if next != self.doc {
                self.remember();
                self.doc = next;
                self.clamp();
                self.status = "Assistant edited the sprite — Ctrl+Z to undo".into();
            }
            self.sync_live_revision();
            let mut value = automation::inspect_revision(&self.doc, &self.live_revision());
            value["changed_cels"] = bounds;
            Ok(value)
        });
        let _ = prepared.request.response.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let (tx, rx) = std::sync::mpsc::channel();
        let mut request = LiveRequest {
            deadline: if expired {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now() + Duration::from_secs(1)
            },
            operation: operation.into(),
            arguments,
            response: tx,
        };
        a.sync_live_revision();
        if let Some((doc, revision, frame)) = a.begin_live_request(&mut request) {
            let prepared = PixelCraft::prepare_live_request(doc, revision, frame, request);
            a.finish_live_request(prepared);
        }
        rx.recv().unwrap()
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
        let args = serde_json::json!({"revision":info["revision"],"commands":[{"op":"add-layer","name":"Ink"},{"op":"add-frame","frame":0}]});
        assert!(request(&mut a, "live-commands", args.clone(), true).is_err());
        assert_eq!(a.doc, before);
        request(&mut a, "live-commands", args.clone(), false).unwrap();
        assert_eq!(a.undo.len(), 1);
        assert_eq!(a.doc.layers.len(), 2);
        assert_eq!(a.doc.durations.len(), 2);
        assert!(request(&mut a, "live-commands", args, false).is_err());
        a.undo();
        assert_eq!(a.doc, before);
        assert_eq!(a.studio.mask, Some(vec![true, false, false, false]));
        let info = request(&mut a, "live-inspect", serde_json::json!({}), false).unwrap();
        request(
            &mut a,
            "live-create",
            serde_json::json!({"revision":info["revision"],"width":4,"height":3}),
            false,
        )
        .unwrap();
        assert_eq!((a.doc.width, a.doc.height), (4, 3));
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
            response,
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
        assert_eq!(&app.doc.layers[0].cels[0][..], &[[0; 4], [50, 60, 70, 255]]);
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
                json!({"revision":info["revision"],"commands":[
                    {"op":"blit","layer":2*pass,"frame":0,"x":pass*41,"y":0,"width":256,"height":256,"data":patch},
                    {"op":"stroke","layer":2*pass+1,"frame":0,"points":[[40,20+pass*20],[450,20+pass*20]],"color":[255,172,81,255],"size":2,"spacing":3}
                ]}),
            );
            info = metadata(&paint);
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
        for shift in [false, true] {
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
            if shift {
                assert!((r.scale - 1.125).abs() < 0.0001, "scale was {}", r.scale);
            } else {
                assert_eq!((r.x, r.y), (12.5, 7.5));
            }
        }
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
