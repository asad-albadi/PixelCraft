//! Shared, offline UI artwork. Logical sizes and hit targets are independent of DPI.
use eframe::egui::{self, Vec2};
use std::collections::BTreeMap;
pub type Bindings = BTreeMap<String, String>;
const BINDINGS: &str = "pixelcraft-ui-shortcuts";
pub fn set_bindings(ctx: &egui::Context, bindings: &Bindings) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(BINDINGS), bindings.clone()));
}
pub fn shortcuts(bindings: &Bindings, action: &str) -> String {
    if action.is_empty() {
        return String::new();
    }
    bindings
        .iter()
        .filter(|(_, a)| a.as_str() == action)
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
pub fn tooltip(bindings: &Bindings, action: &str, label: &str) -> String {
    let keys = shortcuts(bindings, action);
    if keys.is_empty() {
        label.into()
    } else {
        format!("{label} ({keys})")
    }
}
fn bindings(ctx: &egui::Context) -> Bindings {
    ctx.data(|d| d.get_temp(egui::Id::new(BINDINGS)))
        .unwrap_or_default()
}
pub fn icon_name(action: &str) -> &str {
    match action {
        "new" => "file-new",
        "about" => "info",
        "transform-move" => "move",
        "savepalette" => "save",
        "loadpalette" => "folder-open",
        "open" => "folder-open",
        "closetab" => "close",
        "saveas" => "save-as",
        "picker" => "eyedropper",
        "selection" => "select",
        "selectall" => "select-all",
        "layer" => "layer-add",
        "deletelayer" => "layer-delete",
        "frame" => "frame-duplicate",
        "blank" => "frame-add",
        "deleteframe" => "frame-delete",
        "prev" => "frame-previous",
        "next" => "frame-next",
        "up" => "arrow-up",
        "down" => "arrow-down",
        "onion" => "onion-skin",
        "seamless" => "wrap",
        "rotate" => "rotate-cw",
        "flipx" | "selectionflipx" => "flip-horizontal",
        "flipy" | "selectionflipy" => "flip-vertical",
        "png" | "demo" => "image",
        "mp4" => "gif",
        "png-sequence" => "export",
        "sequence" => "import",
        "tiles" | "tiledexport" => "tilemap",
        "workflow" => "settings",

        "resetcolors" => "swatches",
        "move-selection" => "move",
        other => other,
    }
}
pub fn action_label(action: &str) -> &str {
    match action {
        "closetab" => "Close sprite",
        "prev" => "Previous frame",
        "next" => "Next frame",
        "pencil" => "Pencil",
        "new" => "New sprite",
        "picker" => "Eyedropper",
        "selection" => "Rectangular selection",
        "eraser" => "Eraser",
        "fill" => "Paint bucket",
        "eyedropper" => "Eyedropper",
        "line" => "Line",
        "rectangle" => "Rectangle",
        "ellipse" => "Ellipse",
        "select" => "Rectangular selection",
        "wand" => "Magic Wand",
        "lasso" => "Lasso selection",
        "move" => "Move",
        "gradient" => "Gradient",
        "hand" => "Hand",
        "brush" => "Brush",
        "shade" => "Shade brush",
        "dither" => "Dither brush",
        "stamp" => "Selection stamp",
        "brush-round" => "Round brush",
        "brush-square" => "Square brush",
        "select-add" => "Add to selection",
        "select-subtract" => "Subtract from selection",
        "select-intersect" => "Intersect selections",
        "select-all" => "Select all",
        "deselect" => "Deselect",
        "crop" => "Crop",
        "rotate-cw" => "Rotate clockwise",
        "flip-horizontal" => "Flip horizontally",
        "flip-vertical" => "Flip vertically",
        "file-new" => "New sprite",
        "folder-open" => "Open",
        "document" => "Document",
        "file-close" => "Close sprite",
        "save" => "Save",
        "save-as" => "Save as",
        "import" => "Import",
        "export" => "Export",
        "history" => "Recent files",
        "image" => "Image",
        "sheet" => "Sprite sheet",
        "gif" => "Animated image",
        "mp4" => "MP4 video",
        "tilemap" => "Tile map",
        "undo" => "Undo",
        "redo" => "Redo",
        "cut" => "Cut",
        "copy" => "Copy",
        "paste" => "Paste",
        "clear" => "Clear pixels",
        "zoom-in" => "Zoom in",
        "zoom-out" => "Zoom out",
        "fit" => "Fit canvas",
        "grid" => "Pixel grid",
        "preview" => "Preview",
        "wrap" => "Wraparound",
        "onion-skin" => "Onion skin",
        "layers" => "Layers",
        "layer-add" => "Add layer",
        "layer-duplicate" => "Duplicate layer",
        "layer-delete" => "Delete layer",
        "eye" => "Visible",
        "eye-off" => "Hidden",
        "lock" => "Locked",
        "unlock" => "Unlocked",
        "group" => "Layer group",
        "arrow-up" => "Move up",
        "arrow-down" => "Move down",
        "link" => "Linked cel",
        "cel" => "Cel",
        "opacity" => "Opacity",
        "frame" => "Duplicate active frame",
        "frame-add" => "Add frame",
        "frame-duplicate" => "Duplicate frame",
        "frame-delete" => "Delete frame",
        "play" => "Play",
        "pause" => "Pause",
        "stop" => "Stop",
        "frame-previous" => "Previous frame",
        "frame-next" => "Next frame",
        "frame-first" => "First frame",
        "frame-last" => "Last frame",
        "repeat" => "Loop playback",
        "ping-pong" => "Ping-pong playback",
        "tag" => "Animation tag",
        "clock" => "Frame duration",
        "palette" => "Palette",
        "color-add" => "Add color",
        "color-remove" => "Remove color",
        "swatches" => "Used colors",
        "swap" => "Swap colors",
        "alpha" => "Transparency",
        "color-mode" => "Color mode",
        "reference" => "Reference image",
        "scale" => "Scale reference",
        "sheet-import" => "Grid sheet import",
        "atlas" => "Pack atlas",
        "batch-export" => "Batch export",
        "studio" => "Drawing studio",
        "settings" => "Settings",
        "properties" => "Properties",
        "keyboard" => "Keyboard shortcuts",
        "theme" => "Theme",
        "extensions" => "Extensions",
        "install" => "Install extension",
        "uninstall" => "Uninstall extension",
        "assistant" => "Assistant tools",
        "plug" => "Connected",
        "unplug" => "Disconnected",
        "terminal" => "Headless commands",
        "shield" => "Access permission",
        "live-on" => "Live access enabled",
        "live-off" => "Live access disabled",
        "plus" => "Add",
        "minus" => "Remove",
        "close" => "Close",
        "check" => "Confirm",
        "info" => "Information",
        "help" => "Help",
        "warning" => "Warning",
        "error" => "Error",
        "chevron-right" => "Expand submenu",
        "chevron-left" => "Previous",
        "chevron-down" => "Expand",
        "chevron-up" => "Collapse",
        "external-link" => "Open external link",
        "download" => "Download update",
        "upload" => "Upload",
        "refresh" => "Refresh",
        "trash" => "Delete",
        "rename" => "Rename",
        "unsaved" => "Unsaved changes",
        other => other,
    }
}
pub const ICON_NAMES: &[&str] = &[
    "pencil",
    "eraser",
    "fill",
    "eyedropper",
    "line",
    "rectangle",
    "ellipse",
    "select",
    "wand",
    "lasso",
    "move",
    "gradient",
    "hand",
    "brush",
    "shade",
    "dither",
    "stamp",
    "brush-round",
    "brush-square",
    "select-add",
    "select-subtract",
    "select-intersect",
    "select-all",
    "deselect",
    "crop",
    "rotate-cw",
    "flip-horizontal",
    "flip-vertical",
    "file-new",
    "folder-open",
    "document",
    "file-close",
    "save",
    "save-as",
    "import",
    "export",
    "history",
    "image",
    "sheet",
    "gif",
    "tilemap",
    "undo",
    "redo",
    "cut",
    "copy",
    "paste",
    "clear",
    "zoom-in",
    "zoom-out",
    "fit",
    "grid",
    "preview",
    "wrap",
    "onion-skin",
    "layers",
    "layer-add",
    "layer-duplicate",
    "layer-delete",
    "eye",
    "eye-off",
    "lock",
    "unlock",
    "group",
    "arrow-up",
    "arrow-down",
    "link",
    "cel",
    "opacity",
    "frame",
    "frame-add",
    "frame-duplicate",
    "frame-delete",
    "play",
    "pause",
    "stop",
    "frame-previous",
    "frame-next",
    "frame-first",
    "frame-last",
    "repeat",
    "ping-pong",
    "tag",
    "clock",
    "palette",
    "color-add",
    "color-remove",
    "swatches",
    "swap",
    "alpha",
    "color-mode",
    "reference",
    "scale",
    "sheet-import",
    "atlas",
    "batch-export",
    "studio",
    "settings",
    "properties",
    "keyboard",
    "theme",
    "extensions",
    "install",
    "uninstall",
    "assistant",
    "plug",
    "unplug",
    "terminal",
    "shield",
    "live-on",
    "live-off",
    "plus",
    "minus",
    "close",
    "check",
    "info",
    "help",
    "warning",
    "error",
    "chevron-right",
    "chevron-left",
    "chevron-down",
    "chevron-up",
    "external-link",
    "download",
    "upload",
    "refresh",
    "trash",
    "rename",
    "unsaved",
];
macro_rules! density_bytes {
    ($name:literal, $density:expr) => {
        match $density {
            16 => include_bytes!(concat!("../assets/icons/ui/png/16/", $name, ".png"))
                as &'static [u8],
            20 => include_bytes!(concat!("../assets/icons/ui/png/20/", $name, ".png"))
                as &'static [u8],
            24 => include_bytes!(concat!("../assets/icons/ui/png/24/", $name, ".png"))
                as &'static [u8],
            32 => include_bytes!(concat!("../assets/icons/ui/png/32/", $name, ".png"))
                as &'static [u8],
            40 => include_bytes!(concat!("../assets/icons/ui/png/40/", $name, ".png"))
                as &'static [u8],
            48 => include_bytes!(concat!("../assets/icons/ui/png/48/", $name, ".png"))
                as &'static [u8],
            _ => include_bytes!(concat!("../assets/icons/ui/png/48/", $name, ".png"))
                as &'static [u8],
        }
    };
}
fn bytes(name: &str, density: u32) -> &'static [u8] {
    match name {
        "pencil" => density_bytes!("pencil", density),
        "eraser" => density_bytes!("eraser", density),
        "fill" => density_bytes!("fill", density),
        "eyedropper" => density_bytes!("eyedropper", density),
        "line" => density_bytes!("line", density),
        "rectangle" => density_bytes!("rectangle", density),
        "ellipse" => density_bytes!("ellipse", density),
        "select" => density_bytes!("select", density),
        "wand" => density_bytes!("wand", density),
        "lasso" => density_bytes!("lasso", density),
        "move" => density_bytes!("move", density),
        "gradient" => density_bytes!("gradient", density),
        "hand" => density_bytes!("hand", density),
        "brush" => density_bytes!("brush", density),
        "shade" => density_bytes!("shade", density),
        "dither" => density_bytes!("dither", density),
        "stamp" => density_bytes!("stamp", density),
        "brush-round" => density_bytes!("brush-round", density),
        "brush-square" => density_bytes!("brush-square", density),
        "select-add" => density_bytes!("select-add", density),
        "select-subtract" => density_bytes!("select-subtract", density),
        "select-intersect" => density_bytes!("select-intersect", density),
        "select-all" => density_bytes!("select-all", density),
        "deselect" => density_bytes!("deselect", density),
        "crop" => density_bytes!("crop", density),
        "rotate-cw" => density_bytes!("rotate-cw", density),
        "flip-horizontal" => density_bytes!("flip-horizontal", density),
        "flip-vertical" => density_bytes!("flip-vertical", density),
        "file-new" => density_bytes!("file-new", density),
        "folder-open" => density_bytes!("folder-open", density),
        "document" => density_bytes!("document", density),
        "file-close" => density_bytes!("file-close", density),
        "save" => density_bytes!("save", density),
        "save-as" => density_bytes!("save-as", density),
        "import" => density_bytes!("import", density),
        "export" => density_bytes!("export", density),
        "history" => density_bytes!("history", density),
        "image" => density_bytes!("image", density),
        "sheet" => density_bytes!("sheet", density),
        "gif" => density_bytes!("gif", density),
        "tilemap" => density_bytes!("tilemap", density),
        "undo" => density_bytes!("undo", density),
        "redo" => density_bytes!("redo", density),
        "cut" => density_bytes!("cut", density),
        "copy" => density_bytes!("copy", density),
        "paste" => density_bytes!("paste", density),
        "clear" => density_bytes!("clear", density),
        "zoom-in" => density_bytes!("zoom-in", density),
        "zoom-out" => density_bytes!("zoom-out", density),
        "fit" => density_bytes!("fit", density),
        "grid" => density_bytes!("grid", density),
        "preview" => density_bytes!("preview", density),
        "wrap" => density_bytes!("wrap", density),
        "onion-skin" => density_bytes!("onion-skin", density),
        "layers" => density_bytes!("layers", density),
        "layer-add" => density_bytes!("layer-add", density),
        "layer-duplicate" => density_bytes!("layer-duplicate", density),
        "layer-delete" => density_bytes!("layer-delete", density),
        "eye" => density_bytes!("eye", density),
        "eye-off" => density_bytes!("eye-off", density),
        "lock" => density_bytes!("lock", density),
        "unlock" => density_bytes!("unlock", density),
        "group" => density_bytes!("group", density),
        "arrow-up" => density_bytes!("arrow-up", density),
        "arrow-down" => density_bytes!("arrow-down", density),
        "link" => density_bytes!("link", density),
        "cel" => density_bytes!("cel", density),
        "opacity" => density_bytes!("opacity", density),
        "frame" => density_bytes!("frame", density),
        "frame-add" => density_bytes!("frame-add", density),
        "frame-duplicate" => density_bytes!("frame-duplicate", density),
        "frame-delete" => density_bytes!("frame-delete", density),
        "play" => density_bytes!("play", density),
        "pause" => density_bytes!("pause", density),
        "stop" => density_bytes!("stop", density),
        "frame-previous" => density_bytes!("frame-previous", density),
        "frame-next" => density_bytes!("frame-next", density),
        "frame-first" => density_bytes!("frame-first", density),
        "frame-last" => density_bytes!("frame-last", density),
        "repeat" => density_bytes!("repeat", density),
        "ping-pong" => density_bytes!("ping-pong", density),
        "tag" => density_bytes!("tag", density),
        "clock" => density_bytes!("clock", density),
        "palette" => density_bytes!("palette", density),
        "color-add" => density_bytes!("color-add", density),
        "color-remove" => density_bytes!("color-remove", density),
        "swatches" => density_bytes!("swatches", density),
        "swap" => density_bytes!("swap", density),
        "alpha" => density_bytes!("alpha", density),
        "color-mode" => density_bytes!("color-mode", density),
        "reference" => density_bytes!("reference", density),
        "scale" => density_bytes!("scale", density),
        "sheet-import" => density_bytes!("sheet-import", density),
        "atlas" => density_bytes!("atlas", density),
        "batch-export" => density_bytes!("batch-export", density),
        "studio" => density_bytes!("studio", density),
        "settings" => density_bytes!("settings", density),
        "properties" => density_bytes!("properties", density),
        "keyboard" => density_bytes!("keyboard", density),
        "theme" => density_bytes!("theme", density),
        "extensions" => density_bytes!("extensions", density),
        "install" => density_bytes!("install", density),
        "uninstall" => density_bytes!("uninstall", density),
        "assistant" => density_bytes!("assistant", density),
        "plug" => density_bytes!("plug", density),
        "unplug" => density_bytes!("unplug", density),
        "terminal" => density_bytes!("terminal", density),
        "shield" => density_bytes!("shield", density),
        "live-on" => density_bytes!("live-on", density),
        "live-off" => density_bytes!("live-off", density),
        "plus" => density_bytes!("plus", density),
        "minus" => density_bytes!("minus", density),
        "close" => density_bytes!("close", density),
        "check" => density_bytes!("check", density),
        "info" => density_bytes!("info", density),
        "help" => density_bytes!("help", density),
        "warning" => density_bytes!("warning", density),
        "error" => density_bytes!("error", density),
        "chevron-right" => density_bytes!("chevron-right", density),
        "chevron-left" => density_bytes!("chevron-left", density),
        "chevron-down" => density_bytes!("chevron-down", density),
        "chevron-up" => density_bytes!("chevron-up", density),
        "external-link" => density_bytes!("external-link", density),
        "download" => density_bytes!("download", density),
        "upload" => density_bytes!("upload", density),
        "refresh" => density_bytes!("refresh", density),
        "trash" => density_bytes!("trash", density),
        "rename" => density_bytes!("rename", density),
        "unsaved" => density_bytes!("unsaved", density),
        _ => density_bytes!("settings", density),
    }
}
pub fn texture(ctx: &egui::Context, action: &str, size: f32) -> egui::TextureHandle {
    let physical = size * ctx.pixels_per_point();
    let density = [16, 20, 24, 32, 40, 48]
        .into_iter()
        .find(|s| *s as f32 >= physical)
        .unwrap_or(48);
    let name = icon_name(action);
    let id = egui::Id::new(("ui-icon", name, density));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let rgba = image::load_from_memory(bytes(name, density))
        .expect("bundled UI icon")
        .into_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    );
    let t = ctx.load_texture(
        format!("ui-{name}-{density}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}
pub fn button(
    ui: &mut egui::Ui,
    action: &str,
    label: &str,
    selected: bool,
    menu: bool,
) -> egui::Response {
    button_as(ui, action, action, label, selected, menu)
}
pub fn button_as(
    ui: &mut egui::Ui,
    icon: &str,
    action: &str,
    label: &str,
    selected: bool,
    menu: bool,
) -> egui::Response {
    button_named(ui, icon, action, label, selected, menu, None)
}
fn button_named(
    ui: &mut egui::Ui,
    icon: &str,
    action: &str,
    label: &str,
    selected: bool,
    menu: bool,
    accessible_name: Option<&str>,
) -> egui::Response {
    let t = texture(ui.ctx(), icon, if label.is_empty() { 20. } else { 18. });
    let image = egui::Image::new((
        t.id(),
        Vec2::splat(if label.is_empty() { 20. } else { 18. }),
    ));
    let mut b = if menu {
        let check = texture(ui.ctx(), "check", 12.);
        let indicator: egui::Atom = if selected {
            egui::Image::new((check.id(), Vec2::splat(12.))).into()
        } else {
            egui::Atom::custom(egui::Id::new("menu-check-space"), Vec2::splat(12.))
        };
        egui::Button::new((indicator, image, label, egui::Atom::grow()))
    } else if label.is_empty() {
        egui::Button::new(image)
    } else {
        egui::Button::new((image, label))
    }
    .image_tint_follows_text_color(true)
    .min_size(Vec2::new(
        ui.spacing().interact_size.x.max(28.),
        ui.spacing().interact_size.y.max(28.),
    ))
    .corner_radius(3)
    .frame_when_inactive(action != "closetab" || menu);
    let toggle = selected
        || matches!(
            action,
            "pencil"
                | "eraser"
                | "fill"
                | "picker"
                | "line"
                | "rectangle"
                | "ellipse"
                | "selection"
                | "wand"
                | "lasso"
                | "move"
                | "gradient"
                | "hand"
                | "grid"
                | "onion"
                | "seamless"
                | "play"
                | "lock"
                | "theme"
        );
    if toggle {
        b = b.selected(selected);
    }
    let bindings = bindings(ui.ctx());
    if menu {
        b = b.shortcut_text(shortcuts(&bindings, action));
    }
    let response = ui.add(b);
    let accessible = if let Some(name) = accessible_name {
        name.to_owned()
    } else if label.is_empty() {
        action_label(action).to_owned()
    } else {
        label.into()
    };
    response.widget_info(|| {
        if toggle {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                selected,
                &accessible,
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &accessible)
        }
    });
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new(("ui-icon-test-target", action)),
            response.rect,
        )
    });
    response.on_hover_text(tooltip(&bindings, action, &accessible))
}
/// Local actions share artwork with commands, but have no editor key binding.
pub fn local_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    button_as(ui, icon, "", label, false, false)
}

pub fn local_icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    name: &str,
    selected: bool,
) -> egui::Response {
    button_named(ui, icon, "", "", selected, false, Some(name))
}

/// Explicit label inventory for existing dialog and workflow actions.
pub fn label_action(label: &str) -> &'static str {
    match label {
        "+ New" => "new",
        "Create sprite" => "file-new",
        "Open…" | "Browse…" | "Choose sprites…" => "open",
        "Save" => "save",
        "Save as…" => "saveas",
        "Close" | "Close preview" | "Cancel" | "×" => "close",
        "Close sprite" => "closetab",
        "OK" | "Bind" | "Finish moving references" => "check",
        "Discard changes" => "trash",
        "+ Add Color" => "color-add",
        "Delete color" => "color-remove",
        "Save palette…" => "savepalette",
        "Load…" => "loadpalette",
        "Properties & color mode…" | "Properties & ranges…" => "properties",
        "Rename" => "rename",
        "Up" => "up",
        "Down" => "down",
        "Del layer" => "deletelayer",
        "Del frame" => "deleteframe",
        "+ Layer" => "layer",
        "+ Frame" => "frame",
        "+ New group" => "group",
        "Import PNG tilesheet…" | "Import palette…" | "Import PNG / JPEG overlay…" => {
            "import"
        }
        "Export palette…" | "Export PNG + metadata…" => "export",
        "Export Tiled map…" => "tiledexport",
        "Capture selection as tile" | "Capture selection as stamp" => "stamp",
        "+ Tile layer from last tileset" => "layer-add",
        "+ Range" => "tag",
        "Use all frames for playback" => "repeat",
        "Magic Wand (W)" | "Magic Wand" => "wand",
        "Lasso" => "lasso",
        "Move" => "move",
        "Gradient" => "gradient",
        "Resize" => "scale",
        "Crop canvas" => "crop",
        "Add shading ramp" => "shade",
        "Choose sprite sheet…" | "Import cells" | "Import cells into a new sprite" => {
            "sheet-import"
        }
        "Export tag GIFs…" | "Export tag sheets…" => "batch-export",
        "Pack & export PNG + JSON…" => "atlas",
        "Clear recent files" => "history",
        "PixelCraft" | "Aseprite" | "Photoshop" => "keyboard",
        "Install…" | "Install ZIP…" | "Install extension…" => "install",
        "Uninstall" => "uninstall",
        "Tools…" => "settings",
        "Fit" => "fit",
        "Stop extension" => "stop",
        "Remove" => "trash",
        "Run .lua…" => "terminal",
        "Install package…" | "Install folder…" => "install",
        "Check for updates" => "refresh",
        "Normal" => "brush",
        "Shade" => "shade",
        "Dither" => "dither",
        "Selection" => "stamp",
        "Round" => "brush-round",
        "Square" | "Diamond" | "Horizontal" | "Vertical" => "brush-square",
        "Replace" => "select",
        "Add" => "select-add",
        "Subtract" => "select-subtract",
        "Intersect" => "select-intersect",
        "Forward" => "play",
        "Reverse" => "frame-previous",
        "Ping-pong" | "Ping-pong reverse" => "ping-pong",
        "Rotate 90°" => "rotate",
        "Flip X" => "selectionflipx",
        "Flip Y" => "selectionflipy",
        "Invert" | "Grayscale" | "Lighten" | "Darken" => "color-mode",
        "System" | "Dark" | "Light" => "theme",
        "Recent files" => "history",
        "Theme" => "theme",
        label if label.starts_with("Use palette:") => "palette",
        label if label.starts_with("Use theme:") => "theme",
        label if label.starts_with("Use shortcuts:") => "keyboard",
        label if label.starts_with("Use dithering:") => "dither",
        label if label.contains("onion skin") || label == "Onion skin" => "onion",
        label if label.contains("Pixel grid") => "grid",
        label if label.contains("Wrap") || label.contains("Seamless") => "wrap",
        label if label.contains("palette") => "palette",
        label if label.contains("pressure") || label.contains("Pressure") => "brush",
        label if label.contains("Mirror") || label.contains("Symmetry") => "flip-horizontal",
        label if label.contains("live editor") => "live-off",
        _ => "settings",
    }
}
pub fn feedback(ui: &mut egui::Ui, icon: &str, text: &str) {
    let t = texture(ui.ctx(), icon, 16.);
    let color = if icon == "error" {
        ui.visuals().error_fg_color
    } else if icon == "warning" {
        ui.visuals().warn_fg_color
    } else {
        ui.visuals().text_color()
    };
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Image::new((t.id(), Vec2::splat(16.))).tint(color));
        ui.colored_label(color, text);
    });
}
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, action: &str, label: &str) -> egui::Response {
    let icon = match action {
        "eye" if !*value => "eye-off",
        "lock" if !*value => "unlock",
        "live-off" | "live-on" => {
            if *value {
                "live-on"
            } else {
                "live-off"
            }
        }
        _ => action,
    };
    let t = texture(ui.ctx(), icon, 16.);
    let tint = if ui.is_enabled() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let image = egui::Image::new((t.id(), Vec2::splat(16.))).tint(tint);
    let old = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = old.max(28.);
    let response = ui
        .add(egui::Checkbox::new(value, (image, label)))
        .on_hover_text(tooltip(&bindings(ui.ctx()), action, label));
    ui.spacing_mut().interact_size.y = old;
    response
}
pub fn submenu<R>(
    ui: &mut egui::Ui,
    action: &str,
    label: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) {
    let t = texture(ui.ctx(), action, 16.);
    let arrow = texture(ui.ctx(), "chevron-right", 16.);
    let gutter = egui::Atom::custom(egui::Id::new("menu-check-space"), Vec2::splat(12.));
    let b = egui::Button::new((
        gutter,
        egui::Image::new((t.id(), Vec2::splat(16.))),
        label,
        egui::Atom::grow(),
        egui::Image::new((arrow.id(), Vec2::splat(16.))),
    ))
    .image_tint_follows_text_color(true)
    .min_size(Vec2::splat(28.));
    egui::containers::menu::SubMenuButton::from_button(b).ui(ui, content);
}
pub trait UiIcons {
    fn pc_hyperlink_to(&mut self, label: impl AsRef<str>, url: impl ToString) -> egui::Response;
    fn pc_checkbox(&mut self, value: &mut bool, label: impl AsRef<str>) -> egui::Response;
    fn pc_selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        label: impl AsRef<str>,
    ) -> egui::Response;
    fn pc_button(&mut self, label: impl AsRef<str>) -> egui::Response;
    fn pc_command_button(&mut self, command: &str, label: impl AsRef<str>) -> egui::Response;
    fn pc_small_button(&mut self, label: impl AsRef<str>) -> egui::Response;
}
impl UiIcons for egui::Ui {
    fn pc_hyperlink_to(&mut self, label: impl AsRef<str>, url: impl ToString) -> egui::Response {
        let label = label.as_ref();
        let icon = texture(
            self.ctx(),
            if label.contains("update") {
                "download"
            } else {
                "external-link"
            },
            16.,
        );
        self.horizontal(|ui| {
            ui.add(
                egui::Image::new((icon.id(), Vec2::splat(16.))).tint(ui.visuals().hyperlink_color),
            );
            ui.hyperlink_to(label, url).on_hover_text(label)
        })
        .inner
    }
    fn pc_checkbox(&mut self, value: &mut bool, label: impl AsRef<str>) -> egui::Response {
        let label = label.as_ref();
        checkbox(self, value, label_action(label), label)
    }
    fn pc_selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        label: impl AsRef<str>,
    ) -> egui::Response {
        let label = label.as_ref();
        let mapped = label_action(label);
        let action = if ICON_NAMES.contains(&icon_name(label)) {
            label
        } else if mapped == "settings" {
            "check"
        } else {
            mapped
        };
        // A value choice is not a command, even when its displayed name matches one.
        let mut response = button_as(
            self,
            action,
            "value-choice",
            label,
            *current == value,
            false,
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                self.is_enabled(),
                *current == value,
                label,
            )
        });
        if response.clicked() && *current != value {
            *current = value;
            response.mark_changed();
        }
        response
    }
    fn pc_button(&mut self, label: impl AsRef<str>) -> egui::Response {
        let label = label.as_ref();
        local_button(self, label_action(label), label.trim_start_matches("+ "))
    }
    fn pc_command_button(&mut self, command: &str, label: impl AsRef<str>) -> egui::Response {
        button(
            self,
            command,
            label.as_ref().trim_start_matches("+ "),
            false,
            false,
        )
    }
    fn pc_small_button(&mut self, label: impl AsRef<str>) -> egui::Response {
        self.pc_button(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_bundled_icon_has_transparency_and_all_density_variants() {
        for name in ICON_NAMES {
            for density in [16, 20, 24, 32, 40, 48] {
                let rgba = image::load_from_memory(bytes(name, density))
                    .unwrap()
                    .into_rgba8();
                assert_eq!(rgba.dimensions(), (density, density), "{name}");
                assert!(rgba.pixels().any(|p| p[3] == 0), "{name}");
                assert!(rgba.pixels().any(|p| p[3] > 128), "{name}");
                assert!(
                    rgba.pixels()
                        .all(|p| p[3] == 0 || p.0[0..3] == [255, 255, 255]),
                    "white mask: {name}"
                );
            }
        }
    }
    #[test]
    fn shortcut_commands_all_have_semantic_artwork() {
        for action in pixelcraft::productivity::shortcut_preset("PixelCraft").values() {
            assert!(ICON_NAMES.contains(&icon_name(action)), "{action}");
        }
    }
}
