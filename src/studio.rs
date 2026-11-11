use super::*;
use pixelcraft::{
    interchange,
    workflows::{self, SheetOptions, Transform},
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub(super) struct Studio {
    pub recovery_enabled: bool,
    pub open: bool,
    pub sheet_open: bool,
    pub tiles_open: bool,
    pub seamless: bool,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub pixel_perfect: bool,
    pub pressure: bool,
    pub force: f32,
    pub sheet: SheetOptions,
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
    recovery_root: PathBuf,
    last_save: Instant,
    last_session: Option<Workspace>,
    pub recovery_status: String,
}
impl Default for Studio {
    fn default() -> Self {
        Self {
            recovery_enabled: false,
            open: false,
            sheet_open: false,
            tiles_open: false,
            seamless: false,
            mirror_x: false,
            mirror_y: false,
            pixel_perfect: false,
            pressure: false,
            force: 1.,
            sheet: SheetOptions::default(),
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
            recovery_root: extensions::config_directory(),
            last_save: Instant::now(),
            last_session: None,
            recovery_status: "Autosave every 15 seconds".into(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct StoredTab {
    doc: Document,
    saved: Document,
    path: Option<PathBuf>,
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
    pub(super) fn autosave(&mut self, force: bool) {
        if self.shot.is_some() || !self.studio.recovery_enabled {
            return;
        }
        if !force && self.studio.last_save.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.studio.last_save = Instant::now();
        let state = self.workspace();
        if self.studio.last_session.as_ref() == Some(&state) {
            return;
        }
        let result = (|| {
            let root = self.studio.recovery_root.clone();
            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            serde_json::to_writer(&mut z, &state).map_err(|e| e.to_string())?;
            z.flush().map_err(|e| e.to_string())?;
            let bytes = z.finish().map_err(|e| e.to_string())?;
            let file = root.join("workspace.recovery");
            let tmp = root.join("workspace.recovery.tmp");
            std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            if file.exists() {
                std::fs::copy(&file, root.join("workspace.recovery.previous"))
                    .map_err(|e| e.to_string())?;
            }
            std::fs::rename(tmp, file).map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => {
                self.studio.last_session = Some(state);
                self.studio.recovery_status = "Open tabs and unsaved work backed up".into()
            }
            Err(e) => self.studio.recovery_status = format!("Autosave failed: {e}"),
        }
    }
    pub(super) fn restore_workspace(&mut self) {
        if self.shot.is_some() || !self.studio.recovery_enabled {
            return;
        }
        let root = self.studio.recovery_root.clone();
        let mut error: Option<String> = None;
        for name in ["workspace.recovery", "workspace.recovery.previous"] {
            let path = root.join(name);
            if !path.exists() {
                continue;
            }
            let result = (|| {
                let data = std::fs::read(path).map_err(|e| e.to_string())?;
                let mut bytes = vec![];
                flate2::read::ZlibDecoder::new(&data[..])
                    .take(1_500_000_001)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 1_500_000_000 {
                    return Err("Recovery exceeds memory limit".into());
                }
                let state: Workspace = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if state.version != 1
                    || state.tabs.is_empty()
                    || state.active >= state.tabs.len()
                    || state.colors.is_empty()
                    || state.color >= state.colors.len()
                {
                    return Err("Invalid workspace recovery".into());
                }
                for t in &state.tabs {
                    t.doc.validate()?;
                    t.saved.validate()?;
                    if t.layer >= t.doc.layers.len()
                        || t.frame >= t.doc.durations.len()
                        || !t.zoom.is_finite()
                        || !t.pan.iter().all(|v| v.is_finite())
                    {
                        return Err("Invalid recovery tab or memory limit".into());
                    }
                }
                Ok(state)
            })();
            match result {
                Ok(state) => {
                    let active = state.active;
                    self.tabs.clear();
                    for t in &state.tabs {
                        self.tabs.push(Some(SpriteTab {
                            doc: t.doc.clone(),
                            saved: t.saved.clone(),
                            path: t.path.clone(),
                            layer: t.layer,
                            frame: t.frame,
                            undo: vec![],
                            redo: vec![],
                            zoom: t.zoom,
                            pan: Vec2::new(t.pan[0], t.pan[1]),
                            selection: t.selection,
                            polygon: t.polygon.clone(),
                            mask: t.mask.clone(),
                            palette_index: 0,
                            active_tag: None,
                        }));
                    }
                    let t = self.tabs[active].take().unwrap();
                    self.replace(t.doc, t.path);
                    self.saved = t.saved;
                    self.layer = t.layer;
                    self.frame = t.frame;
                    self.zoom = t.zoom;
                    self.pan = t.pan;
                    self.selection = t.selection;
                    self.studio.mask = t.mask;
                    self.studio.polygon = t.polygon;
                    self.active_tab = active;
                    self.named_colors = state.colors.clone();
                    self.active_color = state.color;
                    self.fg = self.named_colors[self.active_color].1;
                    self.status = format!(
                        "Restored {} open tabs, including unsaved work",
                        state.tabs.len()
                    );
                    self.studio.last_session = Some(state);
                    return;
                }
                Err(e) => error = Some(e),
            }
        }
        if let Some(e) = error {
            self.status = format!("Could not restore workspace: {e}");
        }
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
        match workflows::transform(&mut next, self.layer, self.frame, b, Some(&mask), t) {
            Ok(b) => {
                self.remember();
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
                        Transform::Rotate => (ox + oh - y, oy + x),
                        Transform::FlipX => (ox + ow - x, p.1),
                        Transform::FlipY => (p.0, oy + oh - y),
                        Transform::Resize(w, h) => (
                            ox + (x as f32 * w as f32 / ow as f32).round() as i32,
                            oy + (y as f32 * h as f32 / oh as f32).round() as i32,
                        ),
                    };
                }
                if self.studio.mask.is_some() {
                    let mask = workflows::transform_mask(&self.doc, original_bounds, &mask, t);
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
            "tiles" => self.studio.tiles_open = true,
            "seamless" => self.studio.seamless = !self.studio.seamless,
            "lasso" => self.tool = Tool::Lasso,
            "move" => self.tool = Tool::Move,
            "gradient" => self.tool = Tool::Gradient,
            "rotate" => self.transform_selection(Transform::Rotate),
            "selectionflipx" => self.transform_selection(Transform::FlipX),
            "selectionflipy" => self.transform_selection(Transform::FlipY),
            "crop" => {
                let mut d = self.doc.clone();
                match workflows::crop(&mut d, self.selection_bounds()) {
                    Ok(()) => {
                        self.remember();
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
    pub(super) fn effects(&mut self, kind: &str) {
        if !self.doc.editable(self.layer) || self.doc.layers[self.layer].tileset.is_some() {
            self.status = "Select an unlocked image layer".into();
            return;
        }
        self.remember();
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
    pub(super) fn studio_dialogs(&mut self, ctx: &egui::Context) {
        if self.studio.open {
            let mut open = true;
            egui::Window::new("Drawing & selection studio")
                .open(&mut open)
                .default_width(370.)
                .show(ctx, |ui| {
                    ui.strong("Drawing");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.studio.mirror_x, "Mirror X");
                        ui.checkbox(&mut self.studio.mirror_y, "Mirror Y");
                    });
                    ui.checkbox(
                        &mut self.studio.pixel_perfect,
                        "Pixel-perfect pencil (1 px)",
                    );
                    ui.checkbox(&mut self.studio.pressure, "Pressure controls brush size");
                    ui.small("Pressure is used when provided by the tablet backend.");
                    ui.horizontal(|ui| {
                        if ui.button("Lasso").clicked() {
                            self.tool = Tool::Lasso;
                            self.studio.open = false
                        }
                        if ui.button("Move").clicked() {
                            self.tool = Tool::Move;
                            self.studio.open = false
                        }
                        if ui.button("Gradient").clicked() {
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
                        if ui.button("Move").clicked() {
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
                        if ui.button("Resize").clicked() {
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
                            if ui.button(label).clicked() {
                                self.transform_selection(t)
                            }
                        }
                        if ui.button("Crop canvas").clicked() {
                            self.studio_action("crop");
                        }
                    });
                    ui.separator();
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
                        if ui.button("Add shading ramp").clicked() {
                            for (i, c) in workflows::ramp(self.fg, self.bg, self.studio.ramp_steps)
                                .into_iter()
                                .enumerate()
                            {
                                self.named_colors.push((format!("Ramp {}", i + 1), c));
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        for label in ["Invert", "Grayscale", "Lighten", "Darken"] {
                            if ui.button(label).clicked() {
                                self.effects(label)
                            }
                        }
                    });
                    ui.separator();
                    ui.small(&self.studio.recovery_status);
                });
            self.studio.open &= open;
        }
        if self.studio.sheet_open {
            let mut open = true;
            egui::Window::new("Export sprite sheet").open(&mut open).default_width(390.).show(ctx,|ui|{ui.label("Pack frames into rows with transparent padding.");ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.studio.sheet.columns).range(1..=256).prefix("Columns "));ui.add(egui::DragValue::new(&mut self.studio.sheet.padding).range(0..=256).prefix("Padding "));});ui.checkbox(&mut self.studio.sheet.trim,"Trim transparent borders (compact packing)");egui::ComboBox::from_label("Frames").selected_text(self.studio.sheet.tag.and_then(|i|self.doc.tags.get(i)).map_or("All frames",|t|t.name.as_str())).show_ui(ui,|ui|{ui.selectable_value(&mut self.studio.sheet.tag,None,"All frames");for(i,t)in self.doc.tags.iter().enumerate(){ui.selectable_value(&mut self.studio.sheet.tag,Some(i),&t.name);}});egui::ComboBox::from_label("Engine preset").selected_text(&self.studio.sheet.preset).show_ui(ui,|ui|{for p in ["Generic","Godot","Unity"]{ui.selectable_value(&mut self.studio.sheet.preset,p.into(),p);}});ui.small("PNG + JSON with frame positions, durations and tags. Godot adds SpriteFrames (.tres); Unity adds multiple-sprite importer (.png.meta). Place the Godot files together at res://.");if let Ok((im,meta))=workflows::pack_sheet(&self.doc,&self.studio.sheet){ui.label(format!("{} × {} · {} frames",im.width(),im.height(),meta["frames"].as_array().unwrap().len()));}if ui.button("Export PNG + metadata…").clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("PNG sheet",&["png"]).set_file_name("sprite.png").save_file(){self.status=match interchange::export_sheet(&path,&self.doc,&self.studio.sheet){Ok(())=>"Exported sprite sheet and metadata".into(),Err(e)=>format!("Export failed: {e}")};}}ui.small("To import an atlas, open its JSON metadata. Use File → Import PNG sequence for numbered frames.");});
            self.studio.sheet_open = open;
        }
        if self.studio.tiles_open {
            let mut open = true;
            egui::Window::new("Tileset browser").open(&mut open).default_width(330.).show(ctx,|ui|{if ui.button("Import PNG tilesheet…").clicked(){if let Err(e)=self.import_tileset(){self.status=e}}ui.add(egui::DragValue::new(&mut self.tile_size).range(1..=256).prefix("Tile size "));if self.doc.tilesets.is_empty(){ui.label("Import a tilesheet to browse and paint tiles.");}let mut choose=None;egui::ScrollArea::vertical().max_height(450.).show(ui,|ui|{for(ti,t)in self.doc.tilesets.iter().enumerate(){ui.strong(&t.name);ui.horizontal_wrapped(|ui|{for(i,tile)in t.tiles.iter().enumerate(){let tex=ctx.load_texture(format!("tile-{ti}-{i}"),color_image(tile,t.width,t.height),egui::TextureOptions::NEAREST);let selected=self.doc.layers[self.layer].tileset==Some(ti)&&self.selected_tile==i as u32+1;let button=egui::Button::image(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(32.))).selected(selected);if ui.add(button).on_hover_text(format!("Tile {}",i+1)).clicked(){choose=Some((ti,i as u32+1));}}});}});if let Some((ti,id))=choose{if self.doc.layers[self.layer].tileset!=Some(ti){if let Some(l)=self.doc.layers.iter().position(|l|l.tileset==Some(ti)){self.layer=l}else{self.remember();if self.doc.add_tile_layer(ti).is_ok(){self.layer=self.doc.layers.len()-1}}}self.selected_tile=id;self.tool=Tool::Pencil;}ui.checkbox(&mut self.studio.seamless,"Seamless preview");if ui.button("Export Tiled map…").clicked(){self.studio_action("tiledexport");}ui.small("Open .tmj / .tsj for finite orthogonal Tiled JSON maps and image tilesets.");});
            self.studio.tiles_open = open;
        }
        if self.studio.seamless {
            egui::Window::new("Seamless preview")
                .default_width(350.)
                .show(ctx, |ui| {
                    let tex = self.frame_texture(ctx, self.frame);
                    let side = (ui.available_width() / 3.).min(180.);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for _ in 0..3 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            for _ in 0..3 {
                                ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::new(
                                    side,
                                    side * self.doc.height as f32 / self.doc.width as f32,
                                )));
                            }
                        });
                    }
                    if ui.button("Close preview").clicked() {
                        self.studio.seamless = false;
                    }
                });
        }
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
            let points = self.studio.stroke.clone();
            let start = points.iter().rposition(|p| *p == a).unwrap_or(0);
            for &p in &points[start..] {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> PixelCraft {
        PixelCraft::with_context(&egui::Context::default(), None)
    }
    #[test]
    fn recovery_restores_dirty_tabs_saved_baselines_and_active_view() {
        let root = std::env::temp_dir().join(format!("pixelcraft-recovery-{}", std::process::id()));
        let mut a = app();
        a.studio.recovery_enabled = true;
        a.studio.recovery_root = root.clone();
        a.doc.set_pixel(1, 0, 0, [11, 22, 33, 255]);
        a.open_document(Document::new(3, 2), None);
        a.doc.set_pixel(0, 0, 5, [99, 88, 77, 255]);
        a.named_colors.push(("Grass".into(), [1, 2, 3, 255]));
        a.zoom = 7.;
        a.pan = Vec2::new(10., 20.);
        a.autosave(true);
        assert!(root.join("workspace.recovery").exists());
        let mut b = app();
        b.studio.recovery_enabled = true;
        b.studio.recovery_root = root.clone();
        b.restore_workspace();
        assert_eq!(b.tabs.len(), 2);
        assert_eq!(b.active_tab, 1);
        assert!(b.dirty());
        assert_eq!(b.doc.layers[0].cels[0][5], [99, 88, 77, 255]);
        assert_eq!(b.zoom, 7.);
        assert_eq!(b.pan, Vec2::new(10., 20.));
        b.switch_tab(0);
        assert!(b.dirty());
        assert_eq!(b.doc.layers[1].cels[0][0], [11, 22, 33, 255]);
        assert_eq!(b.named_colors[1].0, "Grass");
        a.doc.set_pixel(0, 0, 1, [1, 1, 1, 255]);
        a.autosave(true);
        std::fs::write(root.join("workspace.recovery"), b"corrupt").unwrap();
        b.restore_workspace();
        assert_eq!(b.active_tab, 1);
        assert_eq!(b.doc.layers[0].cels[0][1], CLEAR);
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
    fn advanced_dialogs_render() {
        let ctx = egui::Context::default();
        let mut a = PixelCraft::with_context(&ctx, None);
        a.studio.open = true;
        a.studio.sheet_open = true;
        a.studio.tiles_open = true;
        a.studio.seamless = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| a.studio_dialogs(ui.ctx()));
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
}
