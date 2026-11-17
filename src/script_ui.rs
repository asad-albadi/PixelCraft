//! The Lua worker blocks on replies; egui remains responsive and owns the widgets.
mod canvas;

use crate::ui_icons::UiIcons;
use eframe::egui::{self, Color32};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};
pub struct Request {
    pub value: Value,
    pub reply: mpsc::Sender<Value>,
}
#[derive(Default)]
pub struct ScriptUi {
    receiver: Option<mpsc::Receiver<Request>>,
    pending: Option<Request>,
    received: Option<Instant>,
    started: Option<Instant>,
    pub preview: Option<Value>,
    pub clipboard_request: Option<Request>,
    pub use_system_clipboard: Option<bool>,
    pub modeless: bool,
    pub editor_update: Option<Value>,
    pub history_generation: Option<u64>,
    applied_dialog_bounds: HashMap<u64, u64>,
    canvases: canvas::Canvases,
}
impl ScriptUi {
    pub fn connect(&mut self, rx: mpsc::Receiver<Request>, native_clipboard: bool) {
        *self = Self {
            receiver: Some(rx),
            use_system_clipboard: (!native_clipboard).then_some(false),
            started: Some(Instant::now()),
            ..Self::default()
        };
    }
    pub fn disconnect(&mut self) {
        self.receiver = None;
        self.pending = None;
        self.modeless = false;
        self.canvases = canvas::Canvases::default();
    }

    #[cfg(test)]
    pub fn approve_fixture_file_request(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|request| request.value["op"] == "filePermission")
        {
            let _ = self
                .pending
                .take()
                .unwrap()
                .reply
                .send(json!({"allowed":true}));
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.pending.is_none()
            && let Some(Ok(request)) = self.receiver.as_ref().map(|rx| rx.try_recv())
        {
            let op = request.value["op"].as_str().unwrap_or("");
            self.modeless = op == "pump";
            if matches!(op, "activate" | "closePreview" | "refresh" | "pump") {
                self.preview = Some(request.value.clone());
            }
            if matches!(op, "activate" | "closePreview" | "refresh" | "preferences") {
                let _ = request.reply.send(json!({}));
            } else if matches!(op, "clipboardRead" | "clipboardWrite")
                && self.use_system_clipboard.is_some()
            {
                self.clipboard_request = Some(request);
            } else {
                self.pending = Some(request);
                self.received = Some(Instant::now());
            }
        }
        let Some(request) = self.pending.as_mut() else {
            return;
        };
        let op = request.value["op"].as_str().unwrap_or("").to_string();
        let mut response = None;
        let mut dialog_bounds = Vec::new();
        if op == "filePermission" {
            let mut decision = None;
            let dismissed = crate::fluent::modal_dialog(ctx, "Extension file access", |ui| {
                ui.label(request.value["purpose"].as_str().unwrap_or("Access a file"));
                ui.monospace(request.value["path"].as_str().unwrap_or(""));
                ui.small(request.value["script"].as_str().unwrap_or(""));
                ui.horizontal(|ui| {
                    if ui.pc_button("Deny").clicked() {
                        decision = Some(false);
                    }
                    if ui.pc_button("Allow this request").clicked() {
                        decision = Some(true);
                    }
                });
                decision.is_some()
            });
            if dismissed || decision.is_some() {
                response = Some(json!({"allowed":decision.unwrap_or(false)}));
            }
        } else if matches!(op.as_str(), "clipboardRead" | "clipboardWrite") {
            let dismissed = crate::fluent::modal_dialog(ctx, "Extension clipboard access", |ui| {
                ui.label("Allow this extension to read and write images and text on the system clipboard?");
                ui.horizontal(|ui| {
                    if ui.pc_button("Allow for this run").clicked() {
                        self.use_system_clipboard = Some(true);
                    }
                    if ui.pc_button("Use PixelCraft clipboard").clicked() {
                        self.use_system_clipboard = Some(false);
                    }
                });
                self.use_system_clipboard.is_some()
            });
            if dismissed || self.use_system_clipboard.is_some() {
                self.use_system_clipboard.get_or_insert(false);
                self.clipboard_request = self.pending.take();
                return;
            }
        } else if op == "alert" {
            let value = &request.value["value"];
            let title = value["title"].as_str().unwrap_or("Extension message");
            egui::Window::new(title).collapsible(false).show(ctx, |ui| {
                if let Some(lines) = value["text"].as_array() {
                    for line in lines {
                        ui.label(line.as_str().unwrap_or(""));
                    }
                } else {
                    ui.label(value["text"].as_str().or(value.as_str()).unwrap_or(""));
                }
                if let Some(buttons) = value["buttons"].as_array() {
                    for (i, b) in buttons.iter().enumerate() {
                        if ui.pc_button(b.as_str().unwrap_or("OK")).clicked() {
                            response = Some(json!({"button":i+1}));
                        }
                    }
                } else if ui.pc_button("OK").clicked() {
                    response = Some(json!({"button":1}));
                }
            });
        } else {
            let mut dialogs = if op == "dialog" {
                vec![request.value["dialog"].clone()]
            } else {
                request.value["dialogs"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            };
            if op == "pump" {
                self.canvases.retain_visible(&dialogs);
            }
            for dialog in &mut dialogs {
                if let Some(value) = draw_dialog(
                    ctx,
                    dialog,
                    &mut dialog_bounds,
                    &mut self.applied_dialog_bounds,
                    &mut self.canvases,
                ) {
                    response = Some(value);
                }
            }
            if op == "dialog" {
                request.value["dialog"] = dialogs.first().cloned().unwrap_or(json!({}));
            } else {
                request.value["dialogs"] = json!(dialogs);
            }
            if op == "pump"
                && response.is_none()
                && self
                    .received
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(16))
            {
                response = Some(json!({}));
            }
        }
        egui::Area::new(egui::Id::new("cancel-script"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12., -12.])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if ui.pc_button("Stop extension").clicked() {
                    response = Some(
                        if matches!(op.as_str(), "clipboardRead" | "clipboardWrite") {
                            json!({"error":"Extension stopped"})
                        } else {
                            json!({"cancelled":true,"closed":true})
                        },
                    );
                }
            });
        if let Some(mut value) = response {
            if !dialog_bounds.is_empty() {
                value["dialogBounds"] = json!(dialog_bounds);
            }
            if op == "pump"
                && let Some(editor) = self.editor_update.take()
            {
                value["editor"] = editor;
            }
            value["time"] = json!(self.started.map_or(0., |t| t.elapsed().as_secs_f64()));
            if let Some(request) = self.pending.take() {
                let _ = request.reply.send(value);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
fn draw_dialog(
    ctx: &egui::Context,
    dialog: &mut Value,
    bounds: &mut Vec<Value>,
    applied_bounds: &mut HashMap<u64, u64>,
    canvases: &mut canvas::Canvases,
) -> Option<Value> {
    let title = dialog["title"].as_str().unwrap_or("Extension").to_string();
    let dialog_id = dialog["id"].as_u64().unwrap_or(0);
    let mut widgets = dialog["widgets"].take();
    if let Some(widgets) = widgets.as_array_mut() {
        for widget in widgets
            .iter_mut()
            .filter(|widget| widget["kind"] == "canvas")
        {
            canvases.receive(ctx, dialog_id, widget);
        }
    }
    let revision = dialog["boundsRevision"].as_u64().unwrap_or(0);
    let window_id = egui::Id::new(("lua-dialog", dialog_id));
    let mut open = true;
    let mut window = egui::Window::new(title)
        .id(window_id)
        .open(&mut open)
        .resizable(dialog["resizable"] != false)
        .default_width(330.);
    let requested_bounds = serde_json::from_value::<[i32; 4]>(dialog["bounds"].clone())
        .ok()
        .filter(|bounds| bounds[2] > 0 && bounds[3] > 0);
    if let Some([x, y, width, height]) = requested_bounds {
        let viewport = ctx.content_rect();
        let size = egui::vec2(width as f32, height as f32).min(viewport.size());
        let pos = egui::pos2(x as f32, y as f32);
        window = window
            .default_rect(egui::Rect::from_min_size(pos, size))
            .vscroll(true);
        if applied_bounds.insert(dialog_id, revision) != Some(revision) {
            // Apply the requested geometry once so later frames support native dragging.
            window = window.fixed_pos(pos).min_size(size).max_size(size);
        }
    }
    let data = dialog.get_mut("data").unwrap();
    let mut response = None;
    let mut active_tabs = HashMap::new();
    let mut draw_widgets = |ui: &mut egui::Ui| {
        for (n, w) in widgets.as_array().into_iter().flatten().enumerate() {
            if w["visible"] == false {
                continue;
            }
            if w["kind"] == "tabs" {
                draw_tabs(ui, w, data, dialog_id, &mut active_tabs, &mut response);
                continue;
            }
            if let Some(group) = w["tabGroup"].as_str()
                && active_tabs.get(group).copied() != w["tabId"].as_str()
            {
                continue;
            }
            if w["kind"] == "canvas" {
                ui.push_id(n, |ui| {
                    ui.add_enabled_ui(w["enabled"] != false, |ui| {
                        if let Some(event) = canvases.draw(ui, dialog_id, w) {
                            response = Some(event);
                        }
                    });
                });
                continue;
            }
            ui.push_id(n, |ui| {
                ui.add_enabled_ui(w["enabled"] != false, |ui| {
                    let id = w["id"].as_str().unwrap_or("");
                    let label = w["label"].as_str().unwrap_or("");
                    let text = w["text"].as_str().unwrap_or("").replace('&', "");
                    let kind = w["kind"].as_str().unwrap_or("");
                    if !label.is_empty() {
                        ui.label(label);
                    }
                    let mut changed = false;
                    let mut event = json!({});
                    match kind {
                        "label" => {
                            ui.label(data[id].as_str().unwrap_or(&text));
                        }
                        "separator" => {
                            ui.separator();
                            if !text.is_empty() {
                                ui.strong(text);
                            }
                        }
                        "button" => {
                            if ui.pc_button(&text).clicked() {
                                data[id] = json!(true);
                                changed = true;
                            }
                        }
                        "check" | "radio" => {
                            let mut value = data[id].as_bool().unwrap_or(false);
                            changed = ui.checkbox(&mut value, &text).changed();
                            data[id] = json!(value);
                        }
                        "entry" | "file" => {
                            let mut value = data[id].as_str().unwrap_or("").to_string();
                            changed = ui.text_edit_singleline(&mut value).changed();
                            if kind == "file" && ui.pc_small_button("Browse…").clicked() {
                                let path = if w["save"] == true {
                                    rfd::FileDialog::new().set_file_name(&value).save_file()
                                } else {
                                    rfd::FileDialog::new().pick_file()
                                };
                                if let Some(p) = path {
                                    value = p.to_string_lossy().to_string();
                                    changed = true;
                                }
                            }
                            data[id] = json!(value);
                        }
                        "number" => {
                            let mut value = data[id].as_f64().unwrap_or(0.);
                            changed = ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .max_decimals(w["decimals"].as_u64().unwrap_or(0) as usize),
                                )
                                .changed();
                            data[id] = json!(value);
                        }
                        "slider" => {
                            let mut value = data[id].as_f64().unwrap_or(0.);
                            changed = ui
                                .add(
                                    egui::Slider::new(
                                        &mut value,
                                        w["min"].as_f64().unwrap_or(0.)
                                            ..=w["max"].as_f64().unwrap_or(100.),
                                    )
                                    .integer(),
                                )
                                .changed();
                            data[id] = json!(value);
                        }
                        "combobox" => {
                            let mut value = data[id].as_str().unwrap_or("").to_string();
                            egui::ComboBox::from_id_salt((dialog_id, n))
                                .selected_text(&value)
                                .show_ui(ui, |ui| {
                                    if let Some(options) = w["options"].as_array() {
                                        for option in options {
                                            let s = option.as_str().unwrap_or("");
                                            if ui
                                                .selectable_value(&mut value, s.to_string(), s)
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        }
                                    }
                                });
                            data[id] = json!(value);
                        }
                        "color" => {
                            let c = &data[id];
                            let mut rgba = [
                                c["r"].as_u64().unwrap_or(0) as u8,
                                c["g"].as_u64().unwrap_or(0) as u8,
                                c["b"].as_u64().unwrap_or(0) as u8,
                                c["a"].as_u64().unwrap_or(255) as u8,
                            ];
                            changed = ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed();
                            data[id] = json!({"r":rgba[0],"g":rgba[1],"b":rgba[2],"a":rgba[3]});
                        }
                        "shades" => {
                            if let Some(colors) = w["colors"].as_array() {
                                ui.horizontal_wrapped(|ui| {
                                    for c in colors {
                                        let color = Color32::from_rgba_unmultiplied(
                                            c["r"].as_u64().unwrap_or(0) as u8,
                                            c["g"].as_u64().unwrap_or(0) as u8,
                                            c["b"].as_u64().unwrap_or(0) as u8,
                                            c["a"].as_u64().unwrap_or(255) as u8,
                                        );
                                        if ui.add(egui::Button::new("   ").fill(color)).clicked() {
                                            changed = true;
                                            event = json!({"color":c,"button":1});
                                        }
                                    }
                                });
                            }
                        }
                        _ => {
                            ui.label(format!("Unsupported widget: {kind}"));
                        }
                    }
                    if changed {
                        response =
                            Some(json!({"dialog":dialog_id,"widget":id,"data":data,"event":event}));
                    }
                });
            });
        }
    };
    let window_response = window.show(ctx, |ui| {
        if requested_bounds.is_some() {
            draw_widgets(ui);
        } else {
            egui::ScrollArea::vertical()
                .max_height(550.)
                .show(ui, draw_widgets);
        }
    });
    if let Some(window) = window_response {
        let rect = window.response.rect;
        bounds.push(json!({"id":dialog_id,"bounds":[
            rect.min.x.round() as i32, rect.min.y.round() as i32,
            rect.width().round() as i32, rect.height().round() as i32,
        ]}));
    }
    if !open {
        response = Some(json!({"dialog":dialog_id,"closed":true,"data":data}));
    }
    dialog["widgets"] = widgets;
    response
}

fn draw_tabs<'a>(
    ui: &mut egui::Ui,
    group: &'a Value,
    data: &mut Value,
    dialog_id: u64,
    active_tabs: &mut HashMap<&'a str, &'a str>,
    response: &mut Option<Value>,
) {
    let (Some(group_id), Some(data_id), Some(tabs)) = (
        group["hostId"].as_str(),
        group["id"].as_str(),
        group["tabs"].as_array(),
    ) else {
        return;
    };
    let visible = tabs.iter().filter(|tab| tab["visible"] != false);
    let key = |tab: &'a Value| tab["key"].as_str();
    let requested = if let Some(index) = data[data_id].as_u64() {
        index
            .checked_sub(1)
            .and_then(|index| tabs.get(index as usize))
            .and_then(key)
    } else {
        data[data_id].as_str()
    };
    let mut selected = visible
        .clone()
        .filter_map(key)
        .find(|tab| Some(*tab) == requested)
        .or_else(|| visible.clone().next().and_then(key));
    if let Some(label) = group["label"].as_str() {
        ui.label(label);
    }
    ui.push_id(group_id, |ui| {
        ui.horizontal_wrapped(|ui| {
            for tab in visible {
                let Some(tab_key) = key(tab) else {
                    continue;
                };
                let label = tab["text"].as_str().unwrap_or("").replace('&', "");
                let button = egui::Button::selectable(selected == Some(tab_key), label);
                if ui.add_enabled(tab["enabled"] != false, button).clicked() {
                    let changed = selected != Some(tab_key);
                    selected = Some(tab_key);
                    data[data_id] = json!(tab_key);
                    *response = Some(json!({
                        "dialog":dialog_id,"tabGroup":group_id,"tabId":tab_key,"tabChanged":changed,
                        "data":data,"event":{"tab":tab["id"].as_str().unwrap_or("")},
                    }));
                }
            }
        });
    });
    if let Some(selected) = selected {
        active_tabs.insert(group_id, selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_file_permission_denies_the_operation() {
        let ctx = egui::Context::default();
        let (tx, rx) = mpsc::channel();
        let (reply, response) = mpsc::channel();
        let mut ui = ScriptUi::default();
        ui.connect(rx, false);
        tx.send(Request {
            value: json!({"op":"filePermission","purpose":"Write artwork","path":"test.pixel"}),
            reply,
        })
        .unwrap();
        ctx.run_ui(egui::RawInput::default(), |_| ui.show(&ctx))
            .textures_delta
            .clear();
        assert!(response.try_recv().is_err());
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: Some(egui::Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        ctx.run_ui(input, |_| ui.show(&ctx)).textures_delta.clear();
        assert_eq!(response.try_recv().unwrap()["allowed"], false);
    }

    #[test]
    fn clipboard_permission_button_releases_the_pending_request() {
        let ctx = egui::Context::default();
        let (tx, rx) = mpsc::channel();
        let (reply, _response) = mpsc::channel();
        let mut ui = ScriptUi::default();
        ui.connect(rx, true);
        tx.send(Request {
            value: json!({"op":"clipboardRead"}),
            reply,
        })
        .unwrap();
        let mut frame = |events| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 700.),
                )),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |_| ui.show(&ctx));
            output.textures_delta.clear();
            output.shapes
        };
        frame(vec![]);
        let pos = frame(vec![])
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == "Allow for this run"
                {
                    Some(text.pos + text.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![
            egui::Event::PointerMoved(pos),
            button(true),
            button(false),
        ]);
        assert_eq!(ui.use_system_clipboard, Some(true));
        assert!(ui.pending.is_none());
        assert_eq!(ui.clipboard_request.unwrap().value["op"], "clipboardRead");
    }

    #[test]
    fn clicking_native_dialog_tabs_changes_the_visible_controls() {
        let ctx = egui::Context::default();
        let mut bounds = Vec::new();
        let mut applied_bounds = HashMap::new();
        let mut canvases = canvas::Canvases::default();
        let mut dialog = json!({
            "id":7,"title":"Tabbed dialog","bounds":[40,55,320,200],"boundsRevision":1,
            "data":{"page":"second"},
            "widgets":[
                {"kind":"tabs","id":"page","hostId":"tabs:1","tabs":[
                    {"id":"first","key":"first","text":"First tab"},
                    {"id":"second","key":"second","text":"Second tab"}
                ]},
                {"kind":"label","id":"a","tabGroup":"tabs:1","tabId":"first","text":"First body"},
                {"kind":"label","id":"b","tabGroup":"tabs:1","tabId":"second","text":"Second body"},
                {"kind":"label","id":"tail","text":"Always visible"}
            ]
        });
        let text = |output: &egui::FullOutput, label| {
            output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(item) = &shape.shape
                    && item.galley.text() == label
                {
                    return Some(item.pos + item.galley.size() * 0.5);
                }
                None
            })
        };
        let mut frame = |events, new_bounds: Option<[i32; 4]>| {
            if let Some(new_bounds) = new_bounds {
                dialog["bounds"] = json!(new_bounds);
                dialog["boundsRevision"] = json!(2);
            }
            let mut reply = None;
            bounds.clear();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(850., 620.),
                )),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |_| {
                reply = draw_dialog(
                    &ctx,
                    &mut dialog,
                    &mut bounds,
                    &mut applied_bounds,
                    &mut canvases,
                );
            });
            output.textures_delta.clear();
            (output, reply, bounds[0]["bounds"].clone())
        };
        frame(vec![], None);
        let (output, _, outer) = frame(vec![], None);
        assert_eq!(outer[0], 40);
        assert_eq!(outer[1], 55);
        assert!(outer[2].as_i64().unwrap() >= 300);
        assert!(outer[3].as_i64().unwrap() >= 175);
        assert!(text(&output, "Second body").is_some());
        assert!(text(&output, "Always visible").is_some());
        assert!(text(&output, "First body").is_none());
        let pos = text(&output, "First tab").unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(pos)], None);
        let (output, response, _) = frame(vec![button(true), button(false)], None);
        let response = response.expect("Tab button callback");
        assert_eq!(response["event"]["tab"], "first");
        assert_eq!(response["data"]["page"], "first");
        assert_eq!(response["tabChanged"], true);
        assert!(text(&output, "First body").is_some());
        assert!(text(&output, "Second body").is_none());
        frame(vec![], Some([82, 107, 276, 249]));
        let (output, _, outer) = frame(vec![], None);
        assert_eq!(outer, json!([82, 107, 276, 249]));
        assert!(text(&output, "First body").is_some());
    }

    #[test]
    fn extension_clipboard_requires_a_native_choice_and_remembers_internal_mode() {
        let context = egui::Context::default();
        let (requests, receiver) = mpsc::channel();
        let (reply, _response) = mpsc::channel();
        let mut ui = ScriptUi::default();
        ui.connect(receiver, true);
        let request = |op| Request {
            value: json!({"op":op}),
            reply: reply.clone(),
        };
        requests.send(request("clipboardRead")).unwrap();
        context
            .run_ui(egui::RawInput::default(), |root| ui.show(root.ctx()))
            .textures_delta
            .clear();
        assert!(ui.clipboard_request.is_none());
        assert!(ui.use_system_clipboard.is_none());
        let pressed_escape = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        context
            .run_ui(pressed_escape, |root| ui.show(root.ctx()))
            .textures_delta
            .clear();
        assert_eq!(ui.use_system_clipboard, Some(false));
        assert_eq!(
            ui.clipboard_request.take().unwrap().value["op"],
            "clipboardRead"
        );
        requests.send(request("clipboardWrite")).unwrap();
        context
            .run_ui(egui::RawInput::default(), |root| ui.show(root.ctx()))
            .textures_delta
            .clear();
        assert_eq!(
            ui.clipboard_request.take().unwrap().value["op"],
            "clipboardWrite"
        );
    }

    #[test]
    fn real_dialog_ui_renders_entries_and_buttons() {
        let ctx = egui::Context::default();
        let mut dialog = json!({"id":1,"title":"Extension UI","data":{"amount":"12","apply":false},"widgets":[{"kind":"entry","id":"amount","label":"Amount"},{"kind":"button","id":"apply","text":"Apply"}]});
        // Exercise the same widget renderer used by the running editor.
        let mut result = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(
                draw_dialog(
                    ui.ctx(),
                    &mut dialog,
                    &mut Vec::new(),
                    &mut HashMap::new(),
                    &mut canvas::Canvases::default()
                )
                .is_none()
            );
        });
        assert!(!result.shapes.is_empty());
        assert_eq!(dialog["data"]["amount"], "12");
        result.textures_delta.clear();
    }
}
