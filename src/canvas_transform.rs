use super::*;
use pixelcraft::workflows::{self, Transform};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone, Copy)]
enum Handle {
    Scale,
    SkewX,
    SkewY,
}

impl Handle {
    fn transform(self, [_, _, width, height]: [i32; 4], [dx, dy]: [i32; 2]) -> Transform {
        let limit = MAX_DIMENSION as i32;
        match self {
            Self::Scale => Transform::Resize(
                (width + dx).clamp(1, limit) as usize,
                (height + dy).clamp(1, limit) as usize,
            ),
            Self::SkewX => Transform::SkewX(dx.clamp(width - limit, limit - width)),
            Self::SkewY => Transform::SkewY(dy.clamp(height - limit, limit - height)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Request {
    transform: Transform,
    finish: bool,
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
    wanted: Option<Request>,
    completed: Option<Request>,
    worker: Option<Worker>,
    texture: Option<egui::TextureHandle>,
}

impl PixelCraft {
    pub(super) fn start_canvas_transform(&mut self) {
        self.stop_playback(false);
        let timeline = self.selected_cels.clone();
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
            wanted: None,
            completed: None,
            worker: None,
            texture: None,
        });
        self.status = "Drag a corner to scale or an edge handle to skew. Esc cancels.".into();
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
            || state.timeline != self.selected_cels
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
                        Ok(Output::Preview(image)) => {
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
            let frame = state.frame;
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
                    for &(layer, target_frame) in targets.iter() {
                        if stop.load(Ordering::Relaxed) {
                            return Err("Canvas transform cancelled".into());
                        }
                        if !request.finish && target_frame != frame {
                            continue;
                        }
                        if from_timeline
                            && let Some(link) =
                                doc.layers[layer].cel_meta[target_frame].artwork_link
                        {
                            if shared.contains(&(layer, link)) {
                                if !edited.insert((layer, link)) {
                                    continue;
                                }
                            } else {
                                doc.unlink_cel(layer, target_frame)?;
                            }
                        }
                        workflows::transform(
                            &mut doc,
                            layer,
                            target_frame,
                            bounds,
                            mask.as_deref().map(Vec::as_slice),
                            request.transform,
                        )?;
                    }
                    Ok(if request.finish {
                        let updated_mask = mask.map(|mask| {
                            workflows::transform_mask(&doc, bounds, &mask, request.transform)
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
        let step = self.canvas_pixel_size();
        let accent = self.colors().accent;
        let Some(state) = &mut self.canvas_transform else {
            return;
        };
        let [x, y, w, h] = state.bounds;
        let point = |x: f32, y: f32| board.min + Vec2::new(x, y) * step;
        let [x, y, w, h] = [x, y, w, h].map(|v| v as f32);
        let [mut right, mut bottom] = [w, h];
        let mut skew = Vec2::ZERO;
        if let Some(request) = state.wanted {
            match request.transform {
                Transform::Resize(w, h) => [right, bottom] = [w as f32, h as f32],
                Transform::SkewX(dx) => skew.x = dx as f32,
                Transform::SkewY(dy) => skew.y = dy as f32,
                _ => unreachable!(),
            }
        }
        let corners = [
            point(x, y),
            point(x + right, y + skew.y),
            point(x + right + skew.x, y + bottom + skew.y),
            point(x + skew.x, y + bottom),
        ];
        for i in 0..4 {
            let edge = [corners[i], corners[(i + 1) % 4]];
            painter.line_segment(edge, Stroke::new(3., Color32::from_black_alpha(165)));
            painter.line_segment(edge, Stroke::new(1., accent));
        }
        for (handle, anchor, shown, label, cursor) in [
            (
                Handle::Scale,
                point(x + w, y + h),
                corners[2],
                "canvas-transform-scale",
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                Handle::SkewX,
                point(x + w / 2., y + h) + Vec2::new(-10., 0.),
                corners[3].lerp(corners[2], 0.5) + Vec2::new(-10., 0.),
                "canvas-transform-skew-x",
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Handle::SkewY,
                point(x + w, y + h / 2.) + Vec2::new(0., -10.),
                corners[1].lerp(corners[2], 0.5) + Vec2::new(0., -10.),
                "canvas-transform-skew-y",
                egui::CursorIcon::ResizeVertical,
            ),
        ] {
            let hit = Rect::from_center_size(anchor, Vec2::splat(12.));
            if !state.wanted.is_some_and(|request| request.finish) {
                let response = ui.interact(hit, ui.id().with(label), Sense::drag());
                let finish = response.drag_stopped_by(egui::PointerButton::Primary);
                if (response.dragged_by(egui::PointerButton::Primary) || finish)
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let origin = ui
                        .input(|input| input.pointer.press_origin())
                        .unwrap_or(anchor);
                    let delta = (pointer - origin) / step;
                    let transform = handle.transform(
                        state.bounds,
                        [
                            delta
                                .x
                                .round()
                                .clamp(-(MAX_DIMENSION as f32), MAX_DIMENSION as f32)
                                as i32,
                            delta
                                .y
                                .round()
                                .clamp(-(MAX_DIMENSION as f32), MAX_DIMENSION as f32)
                                as i32,
                        ],
                    );
                    let [_, _, w, h] = state.bounds;
                    if finish
                        && (matches!(transform, Transform::SkewX(0) | Transform::SkewY(0))
                            || transform == Transform::Resize(w as usize, h as usize))
                    {
                        state.wanted = None;
                        state.texture = None;
                        state.worker = None;
                        state.completed = None;
                    } else {
                        state.wanted = Some(Request { transform, finish });
                    }
                    ui.ctx().request_repaint();
                }
                response
                    .on_hover_cursor(cursor)
                    .on_hover_text(match handle {
                        Handle::Scale => "Drag to scale from the top left",
                        Handle::SkewX => "Drag to skew horizontally",
                        Handle::SkewY => "Drag to skew vertically",
                    });
            }
            painter.rect_filled(Rect::from_center_size(shown, Vec2::splat(9.)), 1., accent);
            painter.rect_stroke(
                Rect::from_center_size(shown, Vec2::splat(9.)),
                1.,
                Stroke::new(1., Color32::from_black_alpha(195)),
                egui::StrokeKind::Outside,
            );
            #[cfg(test)]
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(label), hit));
        }
    }
}
