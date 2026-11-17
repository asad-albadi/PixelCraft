use eframe::egui::{
    self, Color32, Pos2, Rect, Response, Stroke, TextureHandle, Ui, Vec2, pos2, vec2,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

const MAX_COMMANDS: usize = 2048;
const MAX_IMAGE_PIXELS: usize = 131_072;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Command {
    FillRect {
        bounds: [f32; 4],
        color: [u8; 4],
    },
    StrokeRect {
        bounds: [f32; 4],
        color: [u8; 4],
        width: f32,
    },
    Text {
        position: [f32; 2],
        color: [u8; 4],
        text: String,
    },
    Line {
        points: Vec<[f32; 2]>,
        color: [u8; 4],
        width: f32,
    },
    Image {
        size: [usize; 2],
        pixels: Vec<[u8; 4]>,
        source: [f32; 4],
        destination: [f32; 4],
        opacity: u8,
    },
}

enum Paint {
    FillRect(Rect, Color32),
    StrokeRect(Rect, Stroke),
    Text(Vec2, Color32, String),
    Line(Vec<Vec2>, Stroke),
    Image {
        texture: TextureHandle,
        hash: u64,
        uv: Rect,
        destination: Rect,
        opacity: u8,
    },
}

type Surfaces = HashMap<String, Surface>;

#[derive(Default)]
pub(super) struct Canvases {
    dialogs: HashMap<u64, Surfaces>,
}

#[derive(Default)]
struct Surface {
    paint: Vec<Paint>,
    captured: u8,
}

impl Canvases {
    pub(super) fn retain_visible(&mut self, dialogs: &[Value]) {
        self.dialogs
            .retain(|id, _| dialogs.iter().any(|dialog| dialog["id"] == *id));
    }

    pub(super) fn receive(&mut self, ctx: &egui::Context, dialog: u64, widget: &mut Value) {
        let Some(wire) = widget
            .as_object_mut()
            .and_then(|object| object.remove("paint"))
        else {
            return;
        };
        let Some(id) = widget["hostId"].as_str() else {
            return;
        };
        if wire
            .as_array()
            .is_none_or(|commands| commands.len() > MAX_COMMANDS)
        {
            return;
        }
        let Ok(commands) = serde_json::from_value::<Vec<Command>>(wire) else {
            return;
        };
        let surfaces = self.dialogs.entry(dialog).or_default();
        let surface = surfaces.entry(id.to_owned()).or_default();
        let mut previous = std::mem::take(&mut surface.paint).into_iter();
        let mut image_pixels = 0;
        surface.paint = commands
            .into_iter()
            .filter_map(|command| {
                let before = previous.next();
                match command {
                    Command::FillRect { bounds, color } => {
                        Some(Paint::FillRect(rect(bounds)?, rgba(color)))
                    }
                    Command::StrokeRect {
                        bounds,
                        color,
                        width,
                    } => Some(Paint::StrokeRect(rect(bounds)?, stroke(width, color)?)),
                    Command::Text {
                        position,
                        color,
                        text,
                    } if text.len() <= 4096 => {
                        Some(Paint::Text(vector(position)?, rgba(color), text))
                    }
                    Command::Line {
                        points,
                        color,
                        width,
                    } if (2..=512).contains(&points.len()) => {
                        let points = points.into_iter().map(vector).collect::<Option<_>>()?;
                        Some(Paint::Line(points, stroke(width, color)?))
                    }
                    Command::Image {
                        size,
                        pixels,
                        source,
                        destination,
                        opacity,
                    } => {
                        let area = size[0].checked_mul(size[1])?;
                        if !size.into_iter().all(|extent| (1..=2048).contains(&extent))
                            || pixels.len() != area
                            || area > MAX_IMAGE_PIXELS - image_pixels
                        {
                            return None;
                        }
                        image_pixels += area;
                        let source = rect(source)?;
                        let destination = rect(destination)?;
                        let extent = vec2(size[0] as f32, size[1] as f32);
                        let clipped = source.intersect(Rect::from_min_size(Pos2::ZERO, extent));
                        if !clipped.is_positive() {
                            return None;
                        }
                        let image_origin = pos2(clipped.min.x / extent.x, clipped.min.y / extent.y);
                        let image_end = pos2(clipped.max.x / extent.x, clipped.max.y / extent.y);
                        let uv = Rect::from_min_max(image_origin, image_end);
                        let project = |point| {
                            destination.min
                                + ((point - source.min) / source.size()) * destination.size()
                        };
                        let destination =
                            Rect::from_min_max(project(clipped.min), project(clipped.max));
                        let mut hasher = DefaultHasher::new();
                        size.hash(&mut hasher);
                        pixels.hash(&mut hasher);
                        let hash = hasher.finish();
                        let image =
                            || egui::ColorImage::new(size, pixels.into_iter().map(rgba).collect());
                        let texture = if let Some(Paint::Image {
                            mut texture,
                            hash: old,
                            ..
                        }) = before
                        {
                            if old != hash {
                                texture.set(image(), egui::TextureOptions::NEAREST);
                            }
                            texture
                        } else {
                            ctx.load_texture(
                                "Extension canvas",
                                image(),
                                egui::TextureOptions::NEAREST,
                            )
                        };
                        Some(Paint::Image {
                            texture,
                            hash,
                            uv,
                            destination,
                            opacity,
                        })
                    }
                    _ => None,
                }
            })
            .collect();
    }

    pub(super) fn draw(&mut self, ui: &mut Ui, dialog: u64, widget: &Value) -> Option<Value> {
        let get_size = |field: &str, fallback| {
            widget[field].as_u64().unwrap_or(fallback).clamp(1, 1024) as f32
        };
        let scale = if widget["autoscaling"] == false {
            1.0 / ui.ctx().pixels_per_point()
        } else {
            1.0
        };
        let size = vec2(get_size("width", 128), get_size("height", 64)) * scale;
        let (frame, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let surface = self
            .dialogs
            .get_mut(&dialog)
            .and_then(|map| map.get_mut(widget["hostId"].as_str()?))?;
        let visible = frame.intersect(ui.clip_rect());
        if !visible.is_positive() {
            return None;
        }
        let painter = ui.painter().with_clip_rect(visible);
        let point = |offset: Vec2| frame.min + offset * scale;
        let bounds =
            |rect: Rect| Rect::from_min_max(point(rect.min.to_vec2()), point(rect.max.to_vec2()));
        let width = |stroke: Stroke| Stroke::new(stroke.width * scale, stroke.color);
        for paint in &surface.paint {
            match paint {
                Paint::FillRect(rect, color) => {
                    painter.rect_filled(bounds(*rect), 0., *color);
                }
                Paint::StrokeRect(rect, stroke) => {
                    painter.rect_stroke(
                        bounds(*rect),
                        0.,
                        width(*stroke),
                        egui::StrokeKind::Inside,
                    );
                }
                Paint::Text(pos, color, text) => {
                    painter.text(
                        point(*pos),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(13. * scale),
                        *color,
                    );
                }
                Paint::Line(points, stroke) => {
                    painter.add(egui::Shape::line(
                        points.iter().copied().map(point).collect(),
                        width(*stroke),
                    ));
                }
                Paint::Image {
                    texture,
                    uv,
                    destination,
                    opacity,
                    ..
                } => {
                    let tint = Color32::from_white_alpha(*opacity);
                    painter.image(texture.id(), bounds(*destination), *uv, tint);
                }
            }
        }
        let events = if ui.is_enabled() {
            surface.input(ui, widget, &response, frame.min, visible, scale)
        } else {
            surface.captured = 0;
            Vec::new()
        };
        (!events.is_empty())
            .then(|| json!({"dialog":dialog,"widget":widget["id"],"canvasEvents":events}))
    }
}

impl Surface {
    fn input(
        &mut self,
        ui: &Ui,
        widget: &Value,
        response: &Response,
        origin: Pos2,
        visible: Rect,
        scale: f32,
    ) -> Vec<Value> {
        let subscriptions = &widget["canvasEvents"];
        if subscriptions
            .as_object()
            .is_none_or(|events| events.is_empty())
        {
            return Vec::new();
        }
        let (events, pointer, modifiers) = ui.input(|input| {
            (
                input.events.clone(),
                input.pointer.hover_pos(),
                input.modifiers,
            )
        });
        let mut reply = Vec::new();
        let mut focused = response.has_focus();
        let has = |name: &str| subscriptions[name] == true;
        let pointer_event = |name, pos: Pos2, button| {
            let pos = (pos - origin) / scale;
            json!({"name":name,"x":pos.x.floor() as i32,"y":pos.y.floor() as i32,"button":button,
                "shiftKey":modifiers.shift,"ctrlKey":modifiers.ctrl,"altKey":modifiers.alt,"metaKey":modifiers.mac_cmd})
        };
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } => {
                    let mask = mouse_button(button);
                    if mask == 0 {
                        continue;
                    }
                    if pressed && visible.contains(pos) && response.contains_pointer() {
                        self.captured |= mask;
                        if has("onkeydown") || has("onkeyup") {
                            response.request_focus();
                            focused = true;
                        }
                        if has("onmousedown") {
                            reply.push(pointer_event("onmousedown", pos, mask));
                        }
                    } else if !pressed && self.captured & mask != 0 {
                        self.captured &= !mask;
                        if has("onmouseup") {
                            reply.push(pointer_event("onmouseup", pos, mask));
                        }
                    }
                }
                egui::Event::PointerMoved(pos)
                    if has("onmousemove")
                        && (self.captured != 0
                            || visible.contains(pos) && response.contains_pointer()) =>
                {
                    reply.push(pointer_event("onmousemove", pos, self.captured));
                }
                egui::Event::PointerGone => self.captured = 0,
                egui::Event::MouseWheel { delta, .. }
                    if has("onwheel") && response.contains_pointer() =>
                {
                    if let Some(pos) = pointer {
                        let mut event = pointer_event("onwheel", pos, self.captured);
                        event["deltaX"] = json!(delta.x / scale);
                        event["deltaY"] = json!(delta.y / scale);
                        reply.push(event);
                    }
                }
                egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } if focused => {
                    let name = if pressed { "onkeydown" } else { "onkeyup" };
                    if has(name) {
                        reply.push(json!({"name":name,"key":key.name(),"repeat":repeat,
                            "shiftKey":modifiers.shift,"ctrlKey":modifiers.ctrl,
                            "altKey":modifiers.alt,"metaKey":modifiers.mac_cmd}));
                    }
                }
                _ => {}
            }
        }
        if has("ondblclick")
            && let Some(pos) = response.interact_pointer_pos()
        {
            for button in [
                egui::PointerButton::Primary,
                egui::PointerButton::Secondary,
                egui::PointerButton::Middle,
            ] {
                if response.double_clicked_by(button) {
                    reply.push(pointer_event("ondblclick", pos, mouse_button(button)));
                }
            }
        }
        reply
    }
}

fn rgba([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn vector([x, y]: [f32; 2]) -> Option<Vec2> {
    (x.abs() <= 32768. && y.abs() <= 32768.).then_some(vec2(x, y))
}

fn rect([x, y, w, h]: [f32; 4]) -> Option<Rect> {
    if !(w > 0. && h > 0.) {
        return None;
    }
    Some(Rect::from_min_size(
        vector([x, y])?.to_pos2(),
        vector([w, h])?,
    ))
}

fn stroke(width: f32, color: [u8; 4]) -> Option<Stroke> {
    (width > 0. && width <= 32768.).then(|| Stroke::new(width, rgba(color)))
}

fn mouse_button(button: egui::PointerButton) -> u8 {
    match button {
        egui::PointerButton::Primary => 1,
        egui::PointerButton::Secondary => 2,
        egui::PointerButton::Middle => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_canvas_clips_drawings_reuses_textures_and_tracks_pointer_capture() {
        let ctx = egui::Context::default();
        let mut canvases = Canvases::default();
        let mut widget = json!({
            "id":"preview","hostId":"widget:1","width":48,"height":24,
            "canvasEvents":{"onmousedown":true,"onmouseup":true,"onmousemove":true,"onkeydown":true},
            "paint":[
                {"kind":"fillRect","bounds":[-10,-10,60,60],"color":[15,67,109,255]},
                {"kind":"image","size":[2,1],"pixels":[[0,0,0,0],[33,44,55,255]],
                    "source":[1,0,1,1],"destination":[10,3,14,14],"opacity":255}
            ]
        });
        let original_paint = widget["paint"].clone();
        canvases.receive(&ctx, 1, &mut widget);
        assert!(widget["paint"].is_null());
        let mut render = |events, paint| {
            if let Some(paint) = paint {
                widget["paint"] = paint;
                canvases.receive(&ctx, 1, &mut widget);
            }
            let mut reply = None;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500., 360.))),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                reply = canvases.draw(ui, 1, &widget);
            });
            let updates = output
                .textures_delta
                .set
                .iter()
                .filter(|(id, _)| **id != egui::TextureId::default())
                .count();
            output.textures_delta.clear();
            (output.shapes, reply, updates)
        };
        let (shapes, _, updates) = render(vec![], None);
        assert_eq!(updates, 1);
        let origin = shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Rect(rect) = &shape.shape
                    && rect.fill == Color32::from_rgb(15, 67, 109)
                {
                    assert_eq!(shape.clip_rect.size(), vec2(48., 24.));
                    return Some(shape.clip_rect.min);
                }
                None
            })
            .expect("Canvas fill");
        assert!(shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.vertices.iter().all(|v| v.uv.x >= 0.5))
        }));
        let at = origin + vec2(5., 7.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let (_, response, updates) = render(
            vec![egui::Event::PointerMoved(at), button(at, true)],
            Some(original_paint),
        );
        assert_eq!(updates, 0);
        let response = response.unwrap();
        let events = response["canvasEvents"].as_array().unwrap();
        assert!(events.iter().any(|event| {
            event["name"] == "onmousedown"
                && event["x"] == 5
                && event["y"] == 7
                && event["button"] == 1
        }));
        let (_, reply, _) = render(
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            None,
        );
        assert_eq!(reply.unwrap()["canvasEvents"][0]["name"], "onkeydown");
        let outside = origin + vec2(65., 31.);
        let (_, response, _) = render(
            vec![egui::Event::PointerMoved(outside), button(outside, false)],
            None,
        );
        let response = response.unwrap();
        let events = response["canvasEvents"].as_array().unwrap();
        assert_eq!(events[0]["name"], "onmousemove");
        assert_eq!(events[1]["name"], "onmouseup");
        assert_eq!(events[1]["x"], 65);
        assert_eq!(events[1]["y"], 31);
    }
}
