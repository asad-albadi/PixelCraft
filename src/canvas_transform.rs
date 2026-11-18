use super::*;
use pixelcraft::workflows::{self, AffineTransform, Transform};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone, Copy)]
enum Handle {
    Move,
    Scale([i8; 2]),
    SkewX(i8),
    SkewY(i8),
    Rotate,
}

fn bounds_center([x, y, width, height]: [i32; 4]) -> Vec2 {
    Vec2::new(x as f32 + width as f32 / 2., y as f32 + height as f32 / 2.)
}

fn rotation_angle(from: Vec2, to: Vec2, initial: f64, snap: bool) -> Option<f64> {
    if from == Vec2::ZERO || to == Vec2::ZERO {
        return None;
    }
    let [fx, fy, tx, ty] = [from.x, from.y, to.x, to.y].map(f64::from);
    let angle = (fx * ty - fy * tx).atan2(fx * tx + fy * ty).to_degrees();
    if !snap {
        return Some(angle);
    }
    let angle = (initial + angle + 180.).rem_euclid(360.) - 180.;
    let quarter = (angle.abs() / 90.).floor() * 90.;
    let subdivision = [0_f64, 26.565051, 45., 63.434949, 90.]
        .into_iter()
        .min_by(|a, b| {
            (angle.abs() - quarter - a)
                .abs()
                .total_cmp(&(angle.abs() - quarter - b).abs())
        })
        .unwrap();
    Some((quarter + subdivision).copysign(angle) - initial)
}

#[derive(Clone, Copy, PartialEq)]
struct Request {
    matrix: AffineTransform,
    finish: bool,
}

#[derive(Clone, Copy)]
struct CanvasDrag {
    origin: Pos2,
    matrix: AffineTransform,
    pivot: Vec2,
}

enum Output {
    Preview(egui::ColorImage),
    Commit(Box<Document>, Option<Vec<bool>>),
}

struct Worker {
    request: Request,
    receiver: Receiver<Result<Output, String>>,
    cancelled: Arc<AtomicBool>,
}

fn shared_cel_links(doc: &Document, targets: &[(usize, usize)]) -> HashSet<(usize, usize)> {
    let mut layers = HashMap::new();
    let origin = |cel: &CelMeta| cel.bounds.map_or([0, 0], |[x, y, _, _]| [x, y]);
    for &(layer, frame) in targets {
        let cel = &doc.layers[layer].cel_meta[frame];
        if let Some(link) = cel.artwork_link {
            let (count, _) = layers
                .entry(layer)
                .or_insert_with(HashMap::new)
                .entry(link)
                .or_insert((0, origin(cel)));
            *count += 1;
        }
    }
    let mut shared = HashSet::new();
    for (layer, mut links) in layers {
        for cel in &doc.layers[layer].cel_meta {
            if let Some(link) = cel.artwork_link
                && let Some((remaining, first_origin)) = links.get_mut(&link)
            {
                *remaining = if *first_origin == origin(cel) {
                    *remaining - 1
                } else {
                    -1
                };
            }
        }
        shared.extend(
            links
                .into_iter()
                .filter_map(|(link, (remaining, _))| (remaining == 0).then_some((layer, link))),
        );
    }
    shared
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub(super) struct CanvasTransform {
    tab: usize,
    generation: u64,
    frame: usize,
    layer: usize,
    timeline: Option<CelSelection>,
    targets: Arc<[(usize, usize)]>,
    bounds: [i32; 4],
    mask: Option<Arc<Vec<bool>>>,
    rotsprite: bool,
    pivot: Vec2,
    pivot_grab: Option<Vec2>,
    drag: Option<CanvasDrag>,
    wanted: Option<Request>,
    completed: Option<Request>,
    worker: Option<Worker>,
    texture: Option<egui::TextureHandle>,
}

impl PixelCraft {
    fn canvas_transform_timeline(&self) -> Option<CelSelection> {
        (self.selected_cels.is_some() || self.timeline_anchor.is_some())
            .then(|| self.timeline_cel_selection())
    }

    pub(super) fn start_canvas_transform(&mut self) {
        self.stop_playback(false);
        let timeline = self.canvas_transform_timeline();
        let targets: Arc<[_]> = match &timeline {
            Some(selection) => selection.cells().collect(),
            None => Arc::new([(self.layer, self.frame)]),
        };
        if targets.is_empty()
            || targets.iter().any(|&(layer, frame)| {
                frame >= self.doc.durations.len()
                    || !self.doc.editable(layer)
                    || self.doc.layers[layer].tileset.is_some()
            })
        {
            self.status = "Choose cels in unlocked image layers".into();
            return;
        }
        let mask = self
            .selection
            .map(|_| Arc::new(self.canvas_selection_mask()));
        let bounds = if let Some(mask) = &mask {
            if !mask.contains(&true) {
                self.status = "The pixel selection is empty".into();
                return;
            }
            self.selection_bounds()
        } else {
            let (mut left, mut top, mut right, mut bottom) =
                (self.doc.width as i32, self.doc.height as i32, 0, 0);
            for &(layer, frame) in targets.iter() {
                self.doc.layers[layer].cels[frame].visit_nonclear(|index, _| {
                    let x = (index % self.doc.width) as i32;
                    let y = (index / self.doc.width) as i32;
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                });
            }
            if left >= right || top >= bottom {
                self.status = "The selected cels are empty".into();
                return;
            }
            [left, top, right - left, bottom - top]
        };
        self.canvas_transform = Some(CanvasTransform {
            tab: self.active_tab,
            generation: self.power.bridge_generation,
            frame: self.frame,
            layer: self.layer,
            timeline,
            targets,
            bounds,
            mask,
            rotsprite: self.studio.rotation_rotsprite,
            pivot: bounds_center(bounds),
            pivot_grab: None,
            drag: None,
            wanted: None,
            completed: None,
            worker: None,
            texture: None,
        });
        self.status = "Drag handles to transform cels. Enter applies. Esc cancels.".into();
    }

    pub(super) fn apply_canvas_transform(&mut self) {
        let Some(state) = &mut self.canvas_transform else {
            return;
        };
        if let Some(request) = &mut state.wanted {
            request.finish = true;
        } else {
            self.canvas_transform = None;
        }
    }

    pub(super) fn poll_canvas_transform(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.canvas_transform.take() else {
            return;
        };
        if self.playing
            || state.tab != self.active_tab
            || state.generation != self.power.bridge_generation
            || state.frame != self.frame
            || state.layer != self.layer
            || state.timeline != self.canvas_transform_timeline()
        {
            self.status = "Canvas transform cancelled".into();
            return;
        }
        if let Some(worker) = state.worker.as_ref() {
            match worker.receiver.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.canvas_transform = Some(state);
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    self.status = "Canvas transform stopped unexpectedly".into();
                    return;
                }
                Ok(result) => {
                    state.completed = Some(worker.request);
                    match result {
                        Ok(Output::Preview(image)) if state.wanted == Some(worker.request) => {
                            if let Some(texture) = &mut state.texture {
                                texture.set(image, egui::TextureOptions::NEAREST);
                            } else {
                                state.texture = Some(ctx.load_texture(
                                    "canvas-transform",
                                    image,
                                    egui::TextureOptions::NEAREST,
                                ));
                            }
                        }
                        Ok(Output::Preview(_)) => {}
                        Ok(Output::Commit(doc, mask)) => {
                            self.remember_action("Transform cels");
                            self.doc = *doc;
                            if let Some(mask) = mask {
                                self.studio.polygon.clear();
                                self.set_selection_mask(mask);
                            }
                            self.status = format!("Transformed {} cels", state.targets.len());
                            return;
                        }
                        Err(error) => {
                            self.status = error;
                            return;
                        }
                    }
                    state.worker = None;
                }
            }
        }
        if let Some(request) = state.wanted
            && state.completed != Some(request)
        {
            let mut doc = self.doc.clone();
            let (active_layer, frame) = (state.layer, state.frame);
            let use_rotsprite = request.finish && state.rotsprite;
            let bounds = state.bounds;
            let mask = state.mask.clone();
            let targets = state.targets.clone();
            let from_timeline = state.timeline.is_some();
            let display = self.display_profile.bytes.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let stop = cancelled.clone();
            let (sender, receiver) = mpsc::channel();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = (|| {
                    let shared = if from_timeline {
                        shared_cel_links(&doc, &targets)
                    } else {
                        HashSet::new()
                    };
                    let mut edited = HashSet::new();
                    let artwork_mask = mask.as_deref().map(Vec::as_slice);
                    let mut rotated_mask = None;
                    for &(layer, target_frame) in targets.iter() {
                        if stop.load(Ordering::Relaxed) {
                            return Err("Canvas transform cancelled".into());
                        }
                        if !request.finish && target_frame != frame {
                            continue;
                        }
                        let mut is_active_cel = (layer, target_frame) == (active_layer, frame);
                        if from_timeline
                            && let Some(link) =
                                doc.layers[layer].cel_meta[target_frame].artwork_link
                        {
                            if shared.contains(&(layer, link)) {
                                if !edited.insert((layer, link)) {
                                    continue;
                                }
                                is_active_cel |= layer == active_layer
                                    && doc.layers[layer].cel_meta[frame].artwork_link == Some(link);
                            } else {
                                doc.unlink_cel(layer, target_frame)?;
                            }
                        }
                        if use_rotsprite {
                            let (_, coverage) = workflows::transform_affine_rotsprite(
                                &mut doc,
                                layer,
                                target_frame,
                                bounds,
                                artwork_mask,
                                request.matrix,
                            )?;
                            if artwork_mask.is_some() && (rotated_mask.is_none() || is_active_cel) {
                                rotated_mask = Some(coverage);
                            }
                        } else {
                            workflows::transform(
                                &mut doc,
                                layer,
                                target_frame,
                                bounds,
                                artwork_mask,
                                Transform::Affine(request.matrix),
                            )?;
                        }
                    }
                    Ok(if request.finish {
                        let updated_mask = rotated_mask.or_else(|| {
                            mask.map(|mask| {
                                workflows::transform_mask(
                                    &doc,
                                    bounds,
                                    &mask,
                                    Transform::Affine(request.matrix),
                                )
                            })
                        });
                        Output::Commit(Box::new(doc), updated_mask)
                    } else {
                        Output::Preview(Self::compose_frame_image(&doc, frame, &display).0)
                    })
                })();
                if sender.send(result).is_ok() {
                    ctx.request_repaint();
                }
            });
            state.worker = Some(Worker {
                request,
                receiver,
                cancelled,
            });
        }
        self.canvas_transform = Some(state);
    }

    pub(super) fn canvas_transform_texture(&self) -> Option<&egui::TextureHandle> {
        self.canvas_transform.as_ref()?.texture.as_ref()
    }

    pub(super) fn draw_canvas_transform(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        board: Rect,
        pane: CanvasPane,
    ) {
        if pane != CanvasPane::Primary {
            return;
        }
        if self.canvas_transform.is_none() {
            return;
        }
        let origin = ui.clip_rect().min + Vec2::splat(8.);
        let apply = Rect::from_min_size(origin, Vec2::new(66., 26.));
        let cancel = apply.translate(Vec2::new(72., 0.));
        if ui.put(apply, egui::Button::new("Apply")).clicked() {
            self.apply_canvas_transform();
        }
        if ui.put(cancel, egui::Button::new("Cancel")).clicked() {
            self.canvas_transform = None;
            self.status = "Canvas transform cancelled".into();
            return;
        }
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::new("canvas-transform-apply"), apply);
            data.insert_temp(egui::Id::new("canvas-transform-cancel"), cancel);
        });
        let step = self.canvas_pixel_size();
        let grid = self.doc.drawing_grid();
        let snap_to_grid = self.snap_to_grid;
        let accent = self.colors().accent;
        let Some(state) = &mut self.canvas_transform else {
            return;
        };
        let [x, y, w, h] = state.bounds;
        let point = |x: f32, y: f32| board.min + Vec2::new(x, y) * step;
        let [x, y, w, h] = [x, y, w, h].map(|v| v as f32);
        let matrix = state
            .wanted
            .map_or(AffineTransform::IDENTITY, |request| request.matrix);
        let transformed = |x, y| {
            let [x, y] = matrix.map([f64::from(x), f64::from(y)]);
            point(x as f32, y as f32)
        };
        let corners = [
            transformed(x, y),
            transformed(x + w, y),
            transformed(x + w, y + h),
            transformed(x, y + h),
        ];
        let midpoint = corners[0].lerp(corners[1], 0.5);
        let rotate = midpoint + 18. * (midpoint - corners[3].lerp(corners[2], 0.5)).normalized();
        let pivot_point = point(state.pivot.x, state.pivot.y);
        for i in 0..4 {
            let edge = [corners[i], corners[(i + 1) % 4]];
            painter.line_segment(edge, Stroke::new(3., Color32::from_black_alpha(165)));
            painter.line_segment(edge, Stroke::new(1., accent));
        }
        for (handle, anchor, label, cursor) in [
            (
                Handle::Move,
                corners[0].lerp(corners[2], 0.5),
                "canvas-transform-move",
                egui::CursorIcon::Move,
            ),
            (
                Handle::Scale([1, 1]),
                corners[2],
                "canvas-transform-scale",
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                Handle::Scale([-1, -1]),
                corners[0],
                "canvas-transform-scale-nw",
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                Handle::Scale([0, -1]),
                corners[0].lerp(corners[1], 0.5),
                "canvas-transform-scale-n",
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Handle::Scale([1, -1]),
                corners[1],
                "canvas-transform-scale-ne",
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                Handle::Scale([-1, 0]),
                corners[0].lerp(corners[3], 0.5),
                "canvas-transform-scale-w",
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Handle::Scale([1, 0]),
                corners[1].lerp(corners[2], 0.5),
                "canvas-transform-scale-e",
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Handle::Scale([-1, 1]),
                corners[3],
                "canvas-transform-scale-sw",
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                Handle::Scale([0, 1]),
                corners[3].lerp(corners[2], 0.5),
                "canvas-transform-scale-s",
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Handle::SkewX(1),
                corners[3].lerp(corners[2], 0.5) + Vec2::new(-10., 0.),
                "canvas-transform-skew-x",
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Handle::SkewY(1),
                corners[1].lerp(corners[2], 0.5) + Vec2::new(0., -10.),
                "canvas-transform-skew-y",
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Handle::SkewX(-1),
                corners[0].lerp(corners[1], 0.5) + Vec2::new(-10., 0.),
                "canvas-transform-skew-top",
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Handle::SkewY(-1),
                corners[0].lerp(corners[3], 0.5) + Vec2::new(0., -10.),
                "canvas-transform-skew-left",
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Handle::Rotate,
                rotate,
                "canvas-transform-rotate",
                egui::CursorIcon::Grab,
            ),
        ] {
            let hit = if matches!(handle, Handle::Move) {
                let mut rect = Rect::NOTHING;
                for corner in corners {
                    rect.extend_with(corner);
                }
                rect
            } else {
                Rect::from_center_size(anchor, Vec2::splat(12.))
            };
            if !state.wanted.is_some_and(|request| request.finish) {
                let mut sense = Sense::drag();
                if matches!(handle, Handle::Move)
                    && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
                {
                    let position = (pointer - board.min) / step;
                    let [px, py] = matrix
                        .inverse()
                        .unwrap()
                        .map([f64::from(position.x), f64::from(position.y)]);
                    if !(f64::from(x)..f64::from(x + w)).contains(&px)
                        || !(f64::from(y)..f64::from(y + h)).contains(&py)
                    {
                        // Keep a captured drag active beyond the transformed quadrilateral.
                        if state.drag.is_none() {
                            sense = Sense::hover();
                        }
                    }
                }
                let response = ui.interact(hit, ui.id().with(label), sense);
                let finish = response.drag_stopped_by(egui::PointerButton::Primary);
                if (response.dragged_by(egui::PointerButton::Primary) || finish)
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let drag = *state.drag.get_or_insert_with(|| CanvasDrag {
                        origin: ui
                            .input(|input| input.pointer.press_origin())
                            .unwrap_or(anchor),
                        matrix,
                        pivot: state.pivot,
                    });
                    let inverse = drag
                        .matrix
                        .inverse()
                        .expect("canvas transform is invertible");
                    let coordinates = |v: Vec2| [f64::from(v.x), f64::from(v.y)];
                    let next = match handle {
                        Handle::Move => {
                            let mut delta = ((pointer - drag.origin) / step).round();
                            let modifiers = ui.input(|input| input.modifiers);
                            if modifiers.shift {
                                if delta.x.abs() < delta.y.abs() {
                                    delta.x = 0.;
                                } else {
                                    delta.y = 0.;
                                }
                            }
                            if snap_to_grid != modifiers.alt {
                                let [left, top] = drag.matrix.map([f64::from(x), f64::from(y)]);
                                let point = (
                                    (left + f64::from(delta.x)).round() as i32,
                                    (top + f64::from(delta.y)).round() as i32,
                                );
                                let extent = (MAX_DIMENSION * 8) as i32;
                                let snapped = grid.snap(point, (-extent, extent, -extent, extent));
                                delta = Vec2::new(
                                    (f64::from(snapped.0) - left) as f32,
                                    (f64::from(snapped.1) - top) as f32,
                                );
                            }
                            let limit = (MAX_DIMENSION * 2) as f32;
                            let delta = delta.clamp(Vec2::splat(-limit), Vec2::splat(limit));
                            drag.matrix
                                .then(AffineTransform::translation(coordinates(delta)))
                        }
                        Handle::Rotate => {
                            let center = point(drag.pivot.x, drag.pivot.y);
                            let from = (drag.origin - center) / step;
                            let to = (pointer - center) / step;
                            let snap = ui.input(|input| input.modifiers.shift);
                            let [axis_x, axis_y] = drag.matrix.map_vector([1., 0.]);
                            let initial = axis_y.atan2(axis_x).to_degrees();
                            rotation_angle(from, to, initial, snap)
                                .map(|degrees| {
                                    drag.matrix.then(AffineTransform::rotation(
                                        degrees,
                                        coordinates(drag.pivot),
                                    ))
                                })
                                .unwrap_or(matrix)
                        }
                        _ => {
                            let [dx, dy] =
                                inverse.map_vector(coordinates((pointer - drag.origin) / step));
                            let [w, h] = [f64::from(w), f64::from(h)];
                            let limit = MAX_DIMENSION as f64;
                            let mut fixed = [f64::from(x), f64::from(y)];
                            let (x_axis, y_axis) = match handle {
                                Handle::Scale(sides) => {
                                    fixed = if ui.input(|input| input.modifiers.alt) {
                                        inverse.map(coordinates(drag.pivot))
                                    } else {
                                        std::array::from_fn(|axis| {
                                            let start = [f64::from(x), f64::from(y)][axis];
                                            let extent = [w, h][axis];
                                            start + (1. - f64::from(sides[axis])) * extent / 2.
                                        })
                                    };
                                    let [sx, sy] = if ui.input(|input| input.modifiers.shift) {
                                        let fixed = drag.matrix.map(fixed);
                                        let origin = coordinates((drag.origin - board.min) / step);
                                        let pointer = coordinates((pointer - board.min) / step);
                                        let u = [origin[0] - fixed[0], origin[1] - fixed[1]];
                                        let v = [pointer[0] - fixed[0], pointer[1] - fixed[1]];
                                        let denominator = u[0] * u[0] + u[1] * u[1];
                                        let scale = if denominator > 1e-10 {
                                            (u[0] * v[0] + u[1] * v[1]) / denominator
                                        } else {
                                            1.
                                        };
                                        let scale = scale
                                            .abs()
                                            .clamp(1. / w.min(h), limit / w.max(h))
                                            .copysign(scale);
                                        [scale, scale]
                                    } else {
                                        let signed_extent = |value: f64| {
                                            value.abs().clamp(1., limit).copysign(value)
                                        };
                                        std::array::from_fn(|axis| {
                                            let extent = [w, h][axis];
                                            let start = [f64::from(x), f64::from(y)][axis];
                                            let handle =
                                                start + (1. + f64::from(sides[axis])) * extent / 2.;
                                            let distance = handle - fixed[axis];
                                            if sides[axis] == 0 || distance.abs() < 1e-10 {
                                                return 1.;
                                            }
                                            let scale = 1. + [dx, dy][axis].round() / distance;
                                            signed_extent(extent * scale) / extent
                                        })
                                    };
                                    ([sx, 0.], [0., sy])
                                }
                                Handle::SkewX(side) => {
                                    fixed[1] += (1. - f64::from(side)) * h / 2.;
                                    let shear =
                                        f64::from(side) * dx.round().clamp(-limit, limit) / h;
                                    ([1., 0.], [shear, 1.])
                                }
                                Handle::SkewY(side) => {
                                    fixed[0] += (1. - f64::from(side)) * w / 2.;
                                    let shear =
                                        f64::from(side) * dy.round().clamp(-limit, limit) / w;
                                    ([1., shear], [0., 1.])
                                }
                                Handle::Rotate | Handle::Move => unreachable!(),
                            };
                            AffineTransform::about(x_axis, y_axis, fixed).then(drag.matrix)
                        }
                    };
                    if next.fits_selection(state.bounds) {
                        let [px, py] = next.map(inverse.map(coordinates(drag.pivot)));
                        state.pivot = Vec2::new(px as f32, py as f32);
                        state.wanted = (next != AffineTransform::IDENTITY).then_some(Request {
                            matrix: next,
                            finish: false,
                        });
                        if state.wanted.is_none() {
                            state.texture = None;
                            state.completed = None;
                        }
                    }
                    if response.drag_stopped() {
                        state.drag = None;
                    }
                    ui.ctx().request_repaint();
                }
                response
                    .on_hover_cursor(cursor)
                    .on_hover_text(match handle {
                        Handle::Move => "Drag to move. Shift locks an axis. Alt toggles grid snapping.",
                        Handle::Scale(_) => {
                            "Drag to scale. Shift keeps proportions. Alt scales around the pivot. Cross an edge to flip."
                        }
                        Handle::SkewX(_) => "Drag to skew horizontally",
                        Handle::SkewY(_) => "Drag to skew vertically",
                        Handle::Rotate => "Drag to rotate. Hold Shift to snap the angle.",
                    });
            }
            if !matches!(handle, Handle::Move) {
                painter.rect_filled(Rect::from_center_size(anchor, Vec2::splat(9.)), 1., accent);
                painter.rect_stroke(
                    Rect::from_center_size(anchor, Vec2::splat(9.)),
                    1.,
                    Stroke::new(1., Color32::from_black_alpha(195)),
                    egui::StrokeKind::Outside,
                );
            }
            #[cfg(test)]
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(label), hit));
        }

        let label = "canvas-transform-pivot";
        let hit = Rect::from_center_size(pivot_point, Vec2::splat(14.));
        if state.drag.is_none() && !state.wanted.is_some_and(|request| request.finish) {
            let response = ui.interact(hit, ui.id().with(label), Sense::click_and_drag());
            let finish = response.drag_stopped_by(egui::PointerButton::Primary);
            if (response.dragged_by(egui::PointerButton::Primary) || finish)
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let offset = *state.pivot_grab.get_or_insert_with(|| {
                    let origin = ui
                        .input(|input| input.pointer.press_origin())
                        .unwrap_or(pointer);
                    state.pivot - (origin - board.min) / step
                });
                let limit = (MAX_DIMENSION * 2) as f32;
                state.pivot = ((pointer - board.min) / step + offset)
                    .clamp(Vec2::splat(-limit), Vec2::splat(limit));
                ui.ctx().request_repaint();
            }
            if response.drag_stopped() {
                state.pivot_grab = None;
            }
            if response.double_clicked() {
                let center = bounds_center(state.bounds);
                let [x, y] = matrix.map([f64::from(center.x), f64::from(center.y)]);
                state.pivot = Vec2::new(x as f32, y as f32);
                ui.ctx().request_repaint();
            }
            response
                .on_hover_cursor(egui::CursorIcon::Move)
                .on_hover_text("Drag the rotation pivot. Double-click to center it.");
        }
        let mark = point(state.pivot.x, state.pivot.y);
        painter.circle_stroke(mark, 5., Stroke::new(3., Color32::from_black_alpha(195)));
        painter.circle_stroke(mark, 5., Stroke::new(1., accent));
        painter.circle_filled(mark, 1.5, accent);
        #[cfg(test)]
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(label), hit));
    }
}
