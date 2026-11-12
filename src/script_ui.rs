//! The Lua worker blocks on replies; egui remains responsive and owns the widgets.
use crate::ui_icons::UiIcons;
use eframe::egui::{self, Color32};
use serde_json::{Value, json};
use std::{
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
    pub modeless: bool,
    pub editor_update: Option<Value>,
    pub final_context: Option<Value>,
}
impl ScriptUi {
    pub fn connect(&mut self, rx: mpsc::Receiver<Request>) {
        *self = Self {
            receiver: Some(rx),
            started: Some(Instant::now()),
            ..Self::default()
        };
    }
    pub fn disconnect(&mut self) {
        self.receiver = None;
        self.pending = None;
        self.modeless = false;
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if self.pending.is_none()
            && let Some(Ok(request)) = self.receiver.as_ref().map(|rx| rx.try_recv())
        {
            let op = request.value["op"].as_str().unwrap_or("");
            self.modeless = op == "pump";
            if op == "refresh" || op == "pump" {
                self.preview = Some(request.value.clone());
            }
            if op == "refresh" {
                let _ = request.reply.send(json!({}));
            } else if op == "preferences" {
                self.final_context = Some(request.value.clone());
                let _ = request.reply.send(json!({}));
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
        if op == "alert" {
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
            for dialog in &mut dialogs {
                if let Some(value) = draw_dialog(ctx, dialog) {
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
                    response = Some(json!({"cancelled":true,"closed":true}));
                }
            });
        if let Some(mut value) = response {
            if let Some(editor) = self.editor_update.take() {
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
fn draw_dialog(ctx: &egui::Context, dialog: &mut Value) -> Option<Value> {
    let title = dialog["title"].as_str().unwrap_or("Extension").to_string();
    let dialog_id = dialog["id"].as_u64().unwrap_or(0);
    let widgets = dialog["widgets"].as_array().cloned().unwrap_or_default();
    let data = dialog.get_mut("data").unwrap();
    let mut response = None;
    let mut open = true;
    egui::Window::new(title).id(egui::Id::new(("lua-dialog",dialog_id))).open(&mut open).default_width(330.).show(ctx,|ui|{
        egui::ScrollArea::vertical().max_height(550.).show(ui,|ui|{for (n,w) in widgets.iter().enumerate(){
            if w["visible"]==false {continue;}
            ui.push_id(n,|ui|{ui.add_enabled_ui(w["enabled"]!=false,|ui|{
                let id=w["id"].as_str().unwrap_or("");
                let label=w["label"].as_str().unwrap_or("");
                let text=w["text"].as_str().unwrap_or("").replace('&',"");
                let kind=w["kind"].as_str().unwrap_or("");
                if !label.is_empty(){ui.label(label);}
                let mut changed=false;let mut event=json!({});
                match kind {
                    "label"=>{ui.label(data[id].as_str().unwrap_or(&text));},
                    "separator"=>{ui.separator();if !text.is_empty(){ui.strong(text);}},
                    "button"=>{if ui.pc_button(&text).clicked(){data[id]=json!(true);changed=true;}},
                    "check"|"radio"=>{let mut value=data[id].as_bool().unwrap_or(false);changed=ui.checkbox(&mut value,&text).changed();data[id]=json!(value);},
                    "entry"|"file"=>{let mut value=data[id].as_str().unwrap_or("").to_string();changed=ui.text_edit_singleline(&mut value).changed();if kind=="file"&&ui.pc_small_button("Browse…").clicked(){let path=if w["save"]==true {rfd::FileDialog::new().set_file_name(&value).save_file()}else{rfd::FileDialog::new().pick_file()};if let Some(p)=path{value=p.to_string_lossy().to_string();changed=true;}}data[id]=json!(value);},
                    "number"=>{let mut value=data[id].as_f64().unwrap_or(0.);changed=ui.add(egui::DragValue::new(&mut value).max_decimals(w["decimals"].as_u64().unwrap_or(0)as usize)).changed();data[id]=json!(value);},
                    "slider"=>{let mut value=data[id].as_f64().unwrap_or(0.);changed=ui.add(egui::Slider::new(&mut value,w["min"].as_f64().unwrap_or(0.)..=w["max"].as_f64().unwrap_or(100.)).integer()).changed();data[id]=json!(value);},
                    "combobox"=>{let mut value=data[id].as_str().unwrap_or("").to_string();egui::ComboBox::from_id_salt((dialog_id,n)).selected_text(&value).show_ui(ui,|ui|{if let Some(options)=w["options"].as_array(){for option in options{let s=option.as_str().unwrap_or("");if ui.selectable_value(&mut value,s.to_string(),s).changed(){changed=true;}}}});data[id]=json!(value);},
                    "color"=>{let c=&data[id];let mut rgba=[c["r"].as_u64().unwrap_or(0)as u8,c["g"].as_u64().unwrap_or(0)as u8,c["b"].as_u64().unwrap_or(0)as u8,c["a"].as_u64().unwrap_or(255)as u8];changed=ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed();data[id]=json!({"r":rgba[0],"g":rgba[1],"b":rgba[2],"a":rgba[3]});},
                    "shades"=>{if let Some(colors)=w["colors"].as_array(){ui.horizontal_wrapped(|ui|{for c in colors{let color=Color32::from_rgba_unmultiplied(c["r"].as_u64().unwrap_or(0)as u8,c["g"].as_u64().unwrap_or(0)as u8,c["b"].as_u64().unwrap_or(0)as u8,c["a"].as_u64().unwrap_or(255)as u8);if ui.add(egui::Button::new("   ").fill(color)).clicked(){changed=true;event=json!({"color":c,"button":1});}}});}},
                    _=>{ui.label(format!("Unsupported widget: {kind}"));},
                }
                if changed {response=Some(json!({"dialog":dialog_id,"widget":id,"data":data,"event":event}));}
            });});
        }});
    });
    if !open {
        response = Some(json!({"dialog":dialog_id,"closed":true,"data":data}));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_dialog_ui_renders_entries_and_buttons() {
        let ctx = egui::Context::default();
        let mut dialog = json!({"id":1,"title":"Extension UI","data":{"amount":"12","apply":false},"widgets":[{"kind":"entry","id":"amount","label":"Amount"},{"kind":"button","id":"apply","text":"Apply"}]});
        // Exercise the same widget renderer used by the running editor.
        let mut result = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(draw_dialog(ui.ctx(), &mut dialog).is_none());
        });
        assert!(!result.shapes.is_empty());
        assert_eq!(dialog["data"]["amount"], "12");
        result.textures_delta.clear();
    }
}
