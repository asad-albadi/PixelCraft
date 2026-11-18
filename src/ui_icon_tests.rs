//! Rendered UI checks exercise the same widget tree without a display server.
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn large_image_palette_only_builds_visible_color_rows() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut document = Document::new(256, 256);
    for (index, pixel) in document.layers[0].cels[0].iter_mut().enumerate() {
        *pixel = [(index >> 8) as u8, index as u8, 100, 255];
    }
    app.replace(document, None);
    let mut output = ctx.run_ui(input(Vec2::new(180., 1000.), 1., vec![]), |ui| {
        app.palette(ui);
    });
    assert!(!app.used_colors.is_empty() && app.used_colors.len() <= 10);
    let count = output.shapes.len();
    output.textures_delta.clear();
    assert!(
        count < 1_000,
        "Palette built {count} shapes for a 180px viewport"
    );
}

#[test]
fn dominant_swatch_click_preserves_saved_named_colors() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(8, 8);
    let color = [220, 30, 70, 255];
    doc.set_pixel(0, 0, 0, color);
    app.replace(doc, None);
    let saved = app.named_colors.clone();
    let size = Vec2::new(200., 1000.);
    let mut output = ctx.run_ui(input(size, 1., vec![]), |ui| app.palette(ui));
    let swatch = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Rect(rect)
                if rect.fill == Color32::from_rgb(220, 30, 70) && rect.rect.width() < 80. =>
            {
                Some(rect.rect.center())
            }
            _ => None,
        })
        .expect("dominant swatch is drawn");
    output.textures_delta.clear();
    let mut output = ctx.run_ui(input(size, 1., click(swatch)), |ui| app.palette(ui));
    output.textures_delta.clear();
    assert_eq!(app.fg, color);
    assert_eq!(app.named_colors, saved);
}

#[test]
#[ignore = "Set PIXELCRAFT_TEST_IMAGE to a local image or project file"]
fn large_project_opens_through_editor_document_flow() {
    let path = PathBuf::from(std::env::var_os("PIXELCRAFT_TEST_IMAGE").unwrap());
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let start = Instant::now();
    app.open_recent(path.clone());
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_eq!(app.tabs.len(), 1, "Open should queue the decode");
    let limit = Instant::now() + Duration::from_secs(120);
    while app.open_job.is_some() {
        assert!(Instant::now() < limit, "Background project open timed out");
        let mut out = frame(&mut app, &ctx, Vec2::new(1920., 1080.), 1., vec![]);
        out.textures_delta.clear();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(app.tabs.len(), 2);
    if path.extension().and_then(|s| s.to_str()) == Some("pixel") {
        assert_eq!(app.path.as_ref(), Some(&path));
    }
    for _ in 0..3 {
        let mut out = frame(&mut app, &ctx, Vec2::new(1920., 1080.), 1., vec![]);
        let shape_count = out.shapes.len();
        out.textures_delta.clear();
        assert!(shape_count < 20_000, "Editor built {shape_count} shapes");
    }
    let mut timings = Vec::new();
    for _ in 0..2 {
        for index in 0..app.doc.durations.len().min(60) {
            app.frame = index;
            let started = Instant::now();
            let mut out = frame(&mut app, &ctx, Vec2::new(1920., 1080.), 1., vec![]);
            timings.push(started.elapsed().as_secs_f64() * 1000.);
            out.textures_delta.clear();
        }
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "Editor frame build p95 {:.1}ms, max {:.1}ms",
        timings[((timings.len() * 95).div_ceil(100) - 1).min(timings.len() - 1)],
        timings.last().unwrap()
    );
    println!(
        "Opened {}x{} project with {} colors across {} tabs",
        app.doc.width,
        app.doc.height,
        app.used_colors.len(),
        app.tabs.len()
    );
}

#[test]
fn animation_exports_open_scale_dialogs() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    for (action, title) in [
        ("png-sequence", "Export PNG sequence"),
        ("gif", "Export animated GIF"),
        ("mp4", "Export MP4 video"),
    ] {
        app.action(action);
        assert_eq!(app.animation_export, Some(action));
        let mut rendered = false;
        for _ in 0..3 {
            let mut output = frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., vec![]);
            rendered |= output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) => text.galley.text() == title,
                _ => false,
            });
            output.textures_delta.clear();
        }
        assert!(rendered, "Missing dialog title {title}");
    }
}

#[test]
fn timeline_does_not_composite_unvisited_frames() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut document = Document::new(64, 64);
    for frame in 0..23 {
        document.add_frame(frame, true);
    }
    app.replace(document, None);
    frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., vec![])
        .textures_delta
        .clear();
    assert_eq!(app.frame_textures.iter().filter(|t| t.is_some()).count(), 1);
    app.frame = 12;
    frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., vec![])
        .textures_delta
        .clear();
    assert!(app.frame_textures[12].is_some());
    assert_eq!(app.frame_textures.iter().filter(|t| t.is_some()).count(), 2);
}

#[test]
fn playback_preserves_frame_deadlines_after_late_repaints() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(8, 8);
    for frame in 0..3 {
        doc.add_frame(frame, true);
    }
    doc.durations.fill(90);
    app.replace(doc, None);
    let start = Instant::now();
    app.last_tick = start;
    assert_eq!(
        app.update_playback_clock(start + Duration::from_millis(100)),
        Duration::from_millis(80)
    );
    assert_eq!(app.frame, 1);
    assert_eq!(
        app.update_playback_clock(start + Duration::from_millis(271)),
        Duration::from_millis(89)
    );
    assert_eq!(app.frame, 3);
    assert_eq!(
        app.update_playback_clock(start + Duration::from_millis(360)),
        Duration::from_millis(90)
    );
    assert_eq!(app.frame, 0);
}

#[test]
#[ignore = "Large document stress test; run with the optimized build"]
fn near_limit_document_survives_edits_and_playback() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(1024, 1024);
    for _ in 0..14 {
        doc.add_layer();
    }
    for f in 0..3 {
        doc.add_frame(f, true);
    }
    for (l, layer) in doc.layers.iter_mut().enumerate() {
        for (f, cel) in layer.cels.iter_mut().enumerate() {
            cel.fill([l as u8, f as u8, 1, 255]);
        }
    }
    assert_eq!(doc.stored_pixels(), 62_914_560);
    // Put three of the dense frames on shared backing so the complete
    // four-step history fits the editor's independently enforced undo budget.
    for layer in &mut doc.layers {
        for frame in 1..4 {
            layer.cels[frame] = layer.cels[0].clone();
        }
    }
    app.replace(doc, None);
    let mut elapsed = Vec::new();
    for step in 0..20 {
        app.frame = step % 4;
        if step % 5 == 0 {
            app.remember();
            app.doc
                .set_pixel(step % 15, app.frame, step, [22, 140, 90, 255]);
        }
        let start = Instant::now();
        let mut out = frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., vec![]);
        elapsed.push(start.elapsed().as_secs_f64() * 1000.);
        out.textures_delta.clear();
    }
    elapsed.sort_by(f64::total_cmp);
    println!(
        "Near-limit frame build p95 {:.1}ms, max {:.1}ms",
        elapsed[18], elapsed[19]
    );
    assert!(
        elapsed[19] < 500.,
        "Large-project frame took {:.1}ms",
        elapsed[19]
    );
    assert!(app.dirty());
    for _ in 0..4 {
        app.undo();
    }
    assert!(!app.dirty());
}

#[test]
fn background_save_updates_only_the_originating_tab_and_keeps_new_edits_dirty() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.replace(Document::new(8, 8), None);
    app.doc.set_pixel(0, 0, 0, [100, 30, 20, 255]);
    let snapshot = app.doc.clone();
    let path = std::env::temp_dir().join(format!(
        "pixelcraft-background-save-{}.pixel",
        std::process::id()
    ));
    app.queue_save(path.clone(), snapshot.clone());
    app.doc.set_pixel(0, 0, 1, [80, 60, 40, 255]);
    app.open_document(Document::new(4, 4), None);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.save_job.is_some() {
        assert!(Instant::now() < deadline);
        app.poll_save();
        std::thread::yield_now();
    }
    assert!(app.path.is_none());
    app.switch_tab(0);
    assert_eq!(app.path.as_ref(), Some(&path));
    assert_eq!(app.saved, snapshot);
    assert!(app.dirty());
    assert_eq!(formats::load(&path).unwrap(), snapshot);
    std::fs::remove_file(path).unwrap();
}

fn input(size: Vec2, scale: f32, events: Vec<egui::Event>) -> egui::RawInput {
    let mut i = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    i.viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(scale);
    i
}
fn click(p: Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]
}
fn frame(
    a: &mut PixelCraft,
    ctx: &egui::Context,
    size: Vec2,
    scale: f32,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut raw = input(size, scale, events);
    raw.time = Some(ctx.cumulative_frame_nr() as f64 * 0.1);
    ctx.run_ui(raw, |ui| a.render(ui))
}
fn discard_frame(
    a: &mut PixelCraft,
    ctx: &egui::Context,
    size: Vec2,
    scale: f32,
    events: Vec<egui::Event>,
) {
    let mut output = frame(a, ctx, size, scale, events);
    output.textures_delta.clear();
}
#[test]
fn icon_buttons_preserve_hit_targets_and_keyboard_activation() {
    for scale in [1., 1.5, 2.] {
        let ctx = egui::Context::default();
        let mut rect = Rect::NOTHING;
        let mut activated = false;
        for step in 0..4 {
            let events = match step {
                1 => click(rect.center()),
                3 => vec![egui::Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                _ => vec![],
            };
            let mut output = ctx.run_ui(input(Vec2::new(800., 620.), scale, events), |ui| {
                let response = ui_icons::button(ui, "play", "", false, false);
                assert!(response.rect.width() >= 28. && response.rect.height() >= 28.);
                rect = response.rect;
                if step == 2 {
                    response.request_focus();
                }
                if step == 3 {
                    activated = response.clicked();
                }
            });
            output.textures_delta.clear();
        }
        assert!(
            activated,
            "Enter activates a focused icon button at scale {scale}"
        );
    }
}
#[test]
fn tab_close_and_new_keep_dirty_confirmation_and_independent_targets() {
    let ctx = egui::Context::default();
    let mut a = PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-ui-review.png")));
    let size = Vec2::new(1180., 840.);
    for _ in 0..3 {
        discard_frame(&mut a, &ctx, size, 1., vec![]);
    }
    // Close lives to the right of the title in its own 28px control.
    a.doc.layers[0].cels[0][0] = [123, 45, 67, 255];
    discard_frame(&mut a, &ctx, size, 1., vec![]);
    let close = ctx
        .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", "closetab"))))
        .unwrap();
    discard_frame(&mut a, &ctx, size, 1., click(close.center()));
    assert!(matches!(a.pending, Some(Pending::CloseTab)));
    assert_eq!(a.tabs.len(), 1);
    assert!(!a.new_dialog);
    a.pending = None;
    for _ in 0..2 {
        discard_frame(&mut a, &ctx, size, 1., vec![]);
    }
    let new = ctx
        .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", "new"))))
        .unwrap();
    discard_frame(&mut a, &ctx, size, 1., click(new.center()));
    assert!(a.new_dialog);
    assert!(a.pending.is_none());
    assert_eq!(a.tabs.len(), 1);
}
#[test]
fn rendered_editor_themes_scaling_and_menu_snapshots() {
    let directory = std::env::var_os("PIXELCRAFT_UI_REVIEW_DIR").map(PathBuf::from);
    if let Some(dir) = &directory {
        std::fs::create_dir_all(dir).unwrap();
    }
    for (theme, scale, scene, width) in [
        ("dark", 1., "editor", 1920.),
        ("light", 1., "editor", 1920.),
        ("light", 1., "collapsed", 1366.),
        ("light", 2., "collapsed", 683.),
        ("dark", 1.5, "editor", 1180.),
        ("light", 1.25, "editor", 1536.),
        ("light", 1.5, "editor", 1280.),
        ("dark", 2., "editor", 1180.),
        ("light", 2., "editor", 960.),
        ("dark", 1., "tabs", 800.),
        ("light", 2., "tabs", 800.),
        ("dark", 1., "file", 1180.),
        ("light", 1., "file", 1180.),
        ("dark", 1., "edit", 1180.),
        ("light", 1., "edit", 1180.),
        ("dark", 1., "frame", 1180.),
        ("light", 1., "frame", 1180.),
        ("dark", 1., "dialog", 1180.),
        ("light", 1., "dialog", 1180.),
        ("custom", 1.5, "editor", 1180.),
        ("light", 1., "matrix-768", 1366.),
        ("light", 1.25, "matrix-768", 1366.),
        ("light", 1.5, "matrix-768", 1366.),
        ("light", 1.75, "matrix-768", 1366.),
        ("light", 2., "matrix-768", 1366.),
        ("light", 1., "matrix-1080", 1920.),
        ("light", 1.25, "matrix-1080", 1920.),
        ("light", 1.5, "matrix-1080", 1920.),
        ("light", 1.75, "matrix-1080", 1920.),
        ("light", 2., "matrix-1080", 1920.),
    ] {
        let ctx = egui::Context::default();
        let mut a =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-ui-review.png")));
        a.set_dark_mode(&ctx, theme != "light");
        if theme == "custom" {
            PixelCraft::apply_theme_colors(
                &ctx,
                &json!({"panel":[25,28,41],"text":[252,224,173],"accent":[83,63,122]}),
            );
        }
        let size = Vec2::new(
            if scene.starts_with("matrix") {
                width / scale
            } else {
                width
            },
            if scene == "matrix-768" {
                768. / scale
            } else if scene == "matrix-1080" || (scene == "editor" && theme == "light") {
                1080. / scale
            } else if width > 1800. {
                1080.
            } else {
                840.
            },
        );
        if scene == "collapsed" {
            a.inspector_open = false;
        }
        if scene == "tabs" {
            a.path = Some(PathBuf::from("Garden.pixel"));
            a.open_document(Document::new(16,16),Some(PathBuf::from("A very long sprite filename that must truncate without covering its close button.pixel")));
            a.open_document(
                Document::new(24, 24),
                Some(PathBuf::from("Walk cycle.pixel")),
            );
            a.doc.layers[0].cels[0][0] = [100, 200, 100, 255];
        }
        if scene == "dialog" {
            a.new_dialog = true;
        }
        let mut textures: BTreeMap<u64, egui::ColorImage> = BTreeMap::new();
        let mut nearest = std::collections::BTreeSet::new();
        let mut last = None;
        for step in 0..6 {
            let events = if step == 3 {
                let x = match scene {
                    "file" => 20.,
                    "edit" => 85.,
                    "frame" => 290.,
                    _ => -100.,
                };
                if x > 0. {
                    click(Pos2::new(x, 16.))
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            let mut out = frame(&mut a, &ctx, size, scale, events);
            assert!(!out.shapes.is_empty());
            for (id, deltas) in &out.textures_delta.set {
                for delta in deltas {
                    let egui::TextureId::Managed(id) = id else {
                        continue;
                    };
                    if delta.options.magnification == egui::TextureFilter::Nearest {
                        nearest.insert(*id);
                    } else {
                        nearest.remove(id);
                    }
                    let egui::ImageData::Color(image) = &delta.image;
                    if let Some([x, y]) = delta.pos {
                        let target = textures.get_mut(id).unwrap();
                        for row in 0..image.size[1] {
                            let dst = (y + row) * target.size[0] + x;
                            let src = row * image.size[0];
                            target.pixels[dst..dst + image.size[0]]
                                .copy_from_slice(&image.pixels[src..src + image.size[0]]);
                        }
                    } else {
                        textures.insert(*id, (**image).clone());
                    }
                }
            }
            out.textures_delta.clear();
            last = Some(out);
        }
        let out = last.unwrap();
        if scene.starts_with("matrix") {
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            for action in ["pencil", "fit", "closetab", "prev", "next"] {
                let rect = ctx
                    .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", action))))
                    .unwrap();
                assert!(
                    screen.contains_rect(rect),
                    "{action} outside {scene} at {scale}: {rect:?}"
                );
            }
            let (_, cell, clip, _) = timeline_cell(&ctx, 2, 0);
            assert!(
                clip.contains_rect(cell),
                "Timeline cell clipped at {scene} {scale}: {cell:?} in {clip:?}"
            );
            assert_eq!(ctx.zoom_factor(), 1.);
            assert_eq!(out.pixels_per_point, scale);
        }
        let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
        assert!(primitives.iter().any(|p| matches!(&p.primitive,egui::epaint::Primitive::Mesh(m) if m.texture_id!=egui::TextureId::default())));
        if let Some(dir) = &directory {
            let name = format!("{theme}-{scale}-{scene}");
            let pixels = rasterize(&primitives, &textures, &nearest, size, out.pixels_per_point);
            image::save_buffer(
                dir.join(format!("{name}.png")),
                &pixels,
                (size.x * out.pixels_per_point) as u32,
                (size.y * out.pixels_per_point) as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
// Test-only software rasterizer for egui's actual mesh and font atlas.
fn rasterize(
    primitives: &[egui::epaint::ClippedPrimitive],
    textures: &BTreeMap<u64, egui::ColorImage>,
    nearest: &std::collections::BTreeSet<u64>,
    size: Vec2,
    scale: f32,
) -> Vec<u8> {
    let w = (size.x * scale) as usize;
    let h = (size.y * scale) as usize;
    let mut output = vec![0u8; w * h * 4];
    for p in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = &p.primitive else {
            continue;
        };
        let egui::TextureId::Managed(id) = mesh.texture_id else {
            continue;
        };
        let Some(texture) = textures.get(&id) else {
            continue;
        };
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let v = [
                mesh.vertices[triangle[0] as usize],
                mesh.vertices[triangle[1] as usize],
                mesh.vertices[triangle[2] as usize],
            ];
            let pt = v.map(|v| v.pos * scale);
            let edge =
                |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            let area = edge(pt[0], pt[1], pt[2]);
            if area.abs() < 0.001 {
                continue;
            }
            let minx = pt
                .iter()
                .map(|p| p.x)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(p.clip_rect.min.x * scale)
                .max(0.) as usize;
            let miny = pt
                .iter()
                .map(|p| p.y)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(p.clip_rect.min.y * scale)
                .max(0.) as usize;
            let maxx = pt
                .iter()
                .map(|p| p.x)
                .fold(0., f32::max)
                .ceil()
                .min(p.clip_rect.max.x * scale)
                .min(w as f32) as usize;
            let maxy = pt
                .iter()
                .map(|p| p.y)
                .fold(0., f32::max)
                .ceil()
                .min(p.clip_rect.max.y * scale)
                .min(h as f32) as usize;
            for y in miny..maxy {
                for x in minx..maxx {
                    let point = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let weights = [
                        edge(pt[1], pt[2], point) / area,
                        edge(pt[2], pt[0], point) / area,
                        edge(pt[0], pt[1], point) / area,
                    ];
                    if weights.iter().any(|b| *b < -0.00001) {
                        continue;
                    }
                    let uv = v[0].uv.to_vec2() * weights[0]
                        + v[1].uv.to_vec2() * weights[1]
                        + v[2].uv.to_vec2() * weights[2];
                    let tx = (uv.x * texture.size[0] as f32 - 0.5)
                        .clamp(0., texture.size[0] as f32 - 1.);
                    let ty = (uv.y * texture.size[1] as f32 - 0.5)
                        .clamp(0., texture.size[1] as f32 - 1.);
                    let ix = tx as usize;
                    let iy = ty as usize;
                    let colors = [
                        texture.pixels[iy * texture.size[0] + ix],
                        texture.pixels[iy * texture.size[0] + (ix + 1).min(texture.size[0] - 1)],
                        texture.pixels[(iy + 1).min(texture.size[1] - 1) * texture.size[0] + ix],
                        texture.pixels[(iy + 1).min(texture.size[1] - 1) * texture.size[0]
                            + (ix + 1).min(texture.size[0] - 1)],
                    ];
                    let fx = tx - ix as f32;
                    let fy = ty - iy as f32;
                    let blend = [
                        (1. - fx) * (1. - fy),
                        fx * (1. - fy),
                        (1. - fx) * fy,
                        fx * fy,
                    ];
                    let mut src = [0f32; 4];
                    let nearest_pixel = if nearest.contains(&id) {
                        Some(
                            texture.pixels[(uv.y * texture.size[1] as f32)
                                .clamp(0., texture.size[1] as f32 - 1.)
                                as usize
                                * texture.size[0]
                                + (uv.x * texture.size[0] as f32)
                                    .clamp(0., texture.size[0] as f32 - 1.)
                                    as usize],
                        )
                    } else {
                        None
                    };
                    for (c, channel) in src.iter_mut().enumerate() {
                        let tex = (0..4)
                            .map(|i| colors[i].to_array()[c] as f32 * blend[i])
                            .sum::<f32>();
                        let tex = nearest_pixel.map(|p| p.to_array()[c] as f32).unwrap_or(tex);
                        let color = (0..3)
                            .map(|i| v[i].color.to_array()[c] as f32 * weights[i])
                            .sum::<f32>();
                        *channel = tex * color / 255.;
                    }
                    let dst = &mut output[(y * w + x) * 4..(y * w + x + 1) * 4];
                    for c in 0..4 {
                        dst[c] =
                            (src[c] + dst[c] as f32 * (1. - src[3] / 255.)).clamp(0., 255.) as u8;
                    }
                }
            }
        }
    }
    output
}

#[test]
fn value_choice_tooltips_do_not_inherit_command_bindings_from_names() {
    fn text(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(children) => {
                for child in children {
                    text(child, out)
                }
            }
            _ => {}
        }
    }
    for label in ["Forward", "pencil"] {
        let ctx = egui::Context::default();
        ctx.style_mut_of(ctx.theme(), |s| {
            s.interaction.show_tooltips_only_when_still = false;
            s.interaction.tooltip_delay = 0.;
        });
        ui_icons::set_bindings(
            &ctx,
            &pixelcraft::productivity::shortcut_preset("PixelCraft"),
        );
        let mut rect = Rect::NOTHING;
        let mut value = 0;
        let mut visible = vec![];
        for step in 0..8 {
            let mut raw = input(
                Vec2::new(400., 300.),
                1.,
                if step > 0 {
                    vec![egui::Event::PointerMoved(rect.center())]
                } else {
                    vec![]
                },
            );
            raw.time = Some(step as f64 * 0.3);
            let mut out = ctx.run_ui(raw, |ui| {
                rect = ui.pc_selectable_value(&mut value, 1, label).rect;
            });
            if step == 7 {
                for shape in &out.shapes {
                    text(&shape.shape, &mut visible);
                }
            }
            out.textures_delta.clear();
        }
        assert!(
            visible.iter().filter(|s| s.as_str() == label).count() >= 2,
            "choice plus visible tooltip: {visible:?}"
        );
        assert!(
            !visible
                .iter()
                .any(|s| s.contains("Space") || s.contains("(B)")),
            "{visible:?}"
        );
    }
}

#[test]
fn local_button_tooltips_never_borrow_icon_shortcuts() {
    for label in [
        "Choose sprites…",
        "Browse…",
        "Run .lua…",
        "Install folder…",
        "Open…",
        "Save",
    ] {
        let ctx = egui::Context::default();
        ctx.style_mut_of(ctx.theme(), |s| {
            s.interaction.show_tooltips_only_when_still = false;
            s.interaction.tooltip_delay = 0.;
        });
        let mut bindings = BTreeMap::new();
        bindings.insert("Ctrl+O".into(), "open".into());
        bindings.insert("Ctrl+T".into(), "terminal".into());
        bindings.insert("Ctrl+I".into(), "install".into());
        ui_icons::set_bindings(&ctx, &bindings);
        let mut rect = Rect::NOTHING;
        let mut visible = vec![];
        for step in 0..8 {
            let mut raw = input(
                Vec2::new(500., 300.),
                1.,
                if step == 0 {
                    vec![]
                } else {
                    vec![egui::Event::PointerMoved(rect.center())]
                },
            );
            raw.time = Some(step as f64 * 0.3);
            let mut out = ctx.run_ui(raw, |ui| {
                rect = ui.pc_button(label).rect;
            });
            if step == 7 {
                collect_text(
                    &out.shapes
                        .iter()
                        .map(|s| s.shape.clone())
                        .collect::<Vec<_>>(),
                    &mut visible,
                );
            }
            out.textures_delta.clear();
        }
        assert_eq!(
            visible.iter().filter(|s| s.as_str() == label).count(),
            2,
            "button and exactly one tooltip: {visible:?}"
        );
        assert!(!visible.iter().any(|s| s.contains("Ctrl+")), "{visible:?}");
    }
}

fn collect_text(shapes: &[egui::Shape], out: &mut Vec<String>) {
    for shape in shapes {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(children) => collect_text(children, out),
            _ => {}
        }
    }
}

#[test]
fn collapsed_groups_hide_nested_timeline_rows_without_hiding_artwork() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(2, 2);
    let group = doc.add_group("Foreground").unwrap();
    let nested = doc.add_group("Props").unwrap();
    doc.layers[0].parent = Some(nested);
    doc.layers[nested].parent = Some(group);
    doc.layers[0].cels[0].set(0, [203, 99, 22, 255]);
    let original = doc.composite(0);
    app.replace(doc, None);

    app.toggle_timeline_group(group);
    assert_eq!(app.layer, group);
    let mut rendered_frame = frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., Vec::new());
    rendered_frame.textures_delta.clear();
    let rendered = |layer| {
        ctx.data(|data| {
            data.get_temp::<(egui::Id, Rect, Rect, String)>(egui::Id::new((
                "timeline-cell",
                layer,
                0usize,
            )))
            .is_some()
        })
    };
    assert!(rendered(group));
    assert!(!rendered(nested));
    assert!(!rendered(0));
    assert_eq!(app.doc.composite(0), original);

    app.toggle_timeline_group(group);
    let mut rendered_frame = frame(&mut app, &ctx, Vec2::new(1180., 840.), 1., Vec::new());
    rendered_frame.textures_delta.clear();
    assert!(rendered(nested));
    assert!(rendered(0));
}

#[test]
fn timeline_cells_have_accessible_names_fit_and_support_keyboard_navigation() {
    for dark in [false, true] {
        for scale in [1., 1.5, 2.] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut a = PixelCraft::with_context(
                &ctx,
                Some(PathBuf::from("/tmp/unused-timeline-review.png")),
            );
            a.set_dark_mode(&ctx, dark);
            // Include both empty and occupied cells, using the sample's three layers.
            a.doc.layers[0].cels[0].fill([0, 0, 0, 0]);
            a.doc.layers[1].cels[0][0] = [1, 2, 3, 255];
            let size = Vec2::new(1180., 840.);
            for _ in 0..4 {
                discard_frame(&mut a, &ctx, size, scale, vec![]);
            }
            let mut out = frame(&mut a, &ctx, size, scale, vec![]);
            let tree = out.platform_output.accesskit_update.as_ref().unwrap();
            for li in 0..3 {
                for f in 0..a.doc.durations.len() {
                    let (_, rect, clip, description) = timeline_cell(&ctx, li, f);
                    assert!(
                        clip.contains_rect(rect),
                        "clipped at {scale}, dark={dark}: {rect:?} in {clip:?}"
                    );
                    let expected = format!(
                        "Layer {}, frame {}: {}",
                        a.doc.layers[li].name,
                        f + 1,
                        if a.cel_occupied[li][f] {
                            "occupied"
                        } else {
                            "empty"
                        }
                    );
                    assert_eq!(description, expected);
                    assert!(
                        tree.nodes
                            .iter()
                            .any(|(_, n)| n.label() == Some(expected.as_str())),
                        "missing accessible cell: {expected}"
                    );
                }
            }
            out.textures_delta.clear();
            // Start at frame 1 on the first layer; Tab must reach frame 2 and Enter select it.
            let (id, _, _, _) = timeline_cell(&ctx, 0, 0);
            ctx.memory_mut(|m| m.request_focus(id));
            discard_frame(&mut a, &ctx, size, scale, vec![key_event(Key::Tab)]);
            let (next_id, _, _, _) = timeline_cell(&ctx, 0, 1);
            assert_eq!(ctx.memory(|m| m.focused()), Some(next_id));
            discard_frame(&mut a, &ctx, size, scale, vec![key_event(Key::Enter)]);
            assert_eq!((a.layer, a.frame), (0, 1));
            assert!(!a.playing);
        }
    }
}
fn timeline_cell(
    ctx: &egui::Context,
    layer: usize,
    frame: usize,
) -> (egui::Id, Rect, Rect, String) {
    ctx.data(|d| d.get_temp(egui::Id::new(("timeline-cell", layer, frame))))
        .unwrap()
}
fn key_event(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn tool_has_one_tooltip_using_current_command_binding() {
    for binding in [Some("P"), None] {
        let ctx = egui::Context::default();
        let mut a =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-tooltip-review.png")));
        ctx.style_mut_of(ctx.theme(), |s| {
            s.interaction.show_tooltips_only_when_still = false;
            s.interaction.tooltip_delay = 0.;
        });
        a.custom_shortcuts.retain(|_, action| action != "pencil");
        if let Some(key) = binding {
            a.custom_shortcuts.insert(key.into(), "pencil".into());
        }
        let size = Vec2::new(1180., 840.);
        for _ in 0..3 {
            discard_frame(&mut a, &ctx, size, 1., vec![]);
        }
        let rect = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", "pencil"))))
            .unwrap();
        let mut baseline = vec![];
        let mut out = frame(&mut a, &ctx, size, 1., vec![]);
        collect_text(
            &out.shapes
                .iter()
                .map(|s| s.shape.clone())
                .collect::<Vec<_>>(),
            &mut baseline,
        );
        out.textures_delta.clear();
        let mut visible = vec![];
        for _ in 0..8 {
            let mut out = frame(
                &mut a,
                &ctx,
                size,
                1.,
                vec![egui::Event::PointerMoved(rect.center())],
            );
            visible.clear();
            collect_text(
                &out.shapes
                    .iter()
                    .map(|s| s.shape.clone())
                    .collect::<Vec<_>>(),
                &mut visible,
            );
            out.textures_delta.clear();
        }
        let expected = if binding.is_some() {
            "Pencil (P)"
        } else {
            "Pencil"
        };
        assert_eq!(
            visible.iter().filter(|s| s.as_str() == expected).count(),
            baseline.iter().filter(|s| s.as_str() == expected).count() + 1,
            "exactly one generated tooltip: {visible:?}"
        );
        assert_eq!(
            visible.iter().filter(|s| s.starts_with("Pencil")).count(),
            baseline.iter().filter(|s| s.starts_with("Pencil")).count() + 1
        );
    }
}

#[test]
fn compact_density_is_default_at_all_window_sizes() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    for size in [Vec2::new(1366., 768.), Vec2::new(1920., 1080.)] {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        assert_eq!(
            ctx.style_of(ctx.theme()).text_styles[&egui::TextStyle::Body].size,
            14.
        );
        assert_eq!(
            ctx.style_of(ctx.theme()).text_styles[&egui::TextStyle::Button].size,
            14.
        );
        assert_eq!(ctx.zoom_factor(), 1.);
    }
}

fn layout_rect(ctx: &egui::Context, name: &str) -> Rect {
    ctx.data(|d| d.get_temp(egui::Id::new(name))).unwrap()
}
fn drag_layout(app: &mut PixelCraft, ctx: &egui::Context, size: Vec2, from: Pos2, to: Pos2) {
    discard_frame(app, ctx, size, 1., vec![egui::Event::PointerMoved(from)]);
    discard_frame(
        app,
        ctx,
        size,
        1.,
        vec![egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    discard_frame(app, ctx, size, 1., vec![egui::Event::PointerMoved(to)]);
    discard_frame(
        app,
        ctx,
        size,
        1.,
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    discard_frame(app, ctx, size, 1., vec![]);
}
#[test]
fn panels_resize_in_both_directions_and_inspector_collapses() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-density.png")));
    let size = Vec2::new(1366., 768.);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let inspector = layout_rect(&ctx, "inspector-bounds");
    let timeline = layout_rect(&ctx, "timeline-bounds");
    assert!((250.0..=280.).contains(&inspector.width()), "{inspector:?}");
    assert!((180.0..=220.).contains(&timeline.height()), "{timeline:?}");
    drag_layout(
        &mut app,
        &ctx,
        size,
        inspector.right_center() - Vec2::new(2., 0.),
        inspector.right_center() + Vec2::new(60., 0.),
    );
    assert!(layout_rect(&ctx, "inspector-bounds").width() > inspector.width() + 40.);
    let from = timeline.center_top() + Vec2::new(0., 2.);
    drag_layout(&mut app, &ctx, size, from, from - Vec2::new(0., 70.));
    let taller = layout_rect(&ctx, "timeline-bounds");
    assert!(taller.height() > timeline.height() + 40.);
    drag_layout(
        &mut app,
        &ctx,
        size,
        taller.center_top() + Vec2::new(0., 2.),
        taller.center_top() + Vec2::new(0., 110.),
    );
    assert!(layout_rect(&ctx, "timeline-bounds").height() < timeline.height() - 20.);
    app.preview_settings.visible = false;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let old_canvas = layout_rect(&ctx, "canvas-bounds");
    let toggle = layout_rect(&ctx, "inspector-toggle");
    discard_frame(&mut app, &ctx, size, 1., click(toggle.center()));
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    assert!(!app.inspector_open);
    assert!(layout_rect(&ctx, "canvas-bounds").width() > old_canvas.width() + 200.);
    let restore = layout_rect(&ctx, "inspector-toggle");
    discard_frame(&mut app, &ctx, size, 1., click(restore.center()));
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    assert!(app.inspector_open);
    assert!(layout_rect(&ctx, "inspector-bounds").width() > inspector.width() + 40.);
    // Small windows keep the same dock position; only an explicit toggle hides it.
    let small = Vec2::new(683., 384.);
    discard_frame(&mut app, &ctx, small, 2., vec![]);
    let toggle = layout_rect(&ctx, "inspector-toggle");
    discard_frame(&mut app, &ctx, small, 2., click(toggle.center()));
    assert!(!app.inspector_open);
    discard_frame(&mut app, &ctx, small, 2., vec![]);
    let restore = layout_rect(&ctx, "inspector-toggle");
    discard_frame(&mut app, &ctx, small, 2., click(restore.center()));
    discard_frame(&mut app, &ctx, small, 2., vec![]);
    assert!(app.inspector_open);
    assert!(layout_rect(&ctx, "inspector-bounds").left() < 2.);
}

#[test]
fn preview_stays_docked_bottom_left_at_normal_and_small_window_sizes() {
    for size in [
        Vec2::new(1366., 768.),
        Vec2::new(1920., 1080.),
        Vec2::new(683., 384.),
    ] {
        let ctx = egui::Context::default();
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/preview-layout-check.png")));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        let inspector = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new("inspector-bounds")))
            .expect("left inspector stays docked until explicitly hidden");
        let preview = ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new("pinned-preview-bounds")))
            .expect("preview has a separate pinned panel");
        assert!(
            (preview.bottom() - inspector.bottom()).abs() <= 4.,
            "{size:?}: {preview:?} in {inspector:?}"
        );
        assert!(preview.left() >= inspector.left() && preview.right() <= inspector.right());
        assert!(
            preview.height() >= 120.,
            "preview stays readable: {preview:?}"
        );
    }
}

#[test]
#[ignore = "Manual CPU render profiling; run in both debug and release builds"]
fn profile_steady_editor_render() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let size = Vec2::new(1366., 768.);
    for _ in 0..8 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let start = std::time::Instant::now();
    for _ in 0..200 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    eprintln!(
        "Editor CPU render: {:.3} ms/frame (200 warm frames)",
        start.elapsed().as_secs_f64() * 1000. / 200.
    );
}

#[test]
fn inspector_header_and_restore_are_reachable_at_display_scales() {
    for physical in [Vec2::new(1366., 768.), Vec2::new(1920., 1080.)] {
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            let size = physical / scale;
            let ctx = egui::Context::default();
            let mut app =
                PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-native.png")));
            for _ in 0..3 {
                discard_frame(&mut app, &ctx, size, scale, vec![]);
            }
            let inspector = layout_rect(&ctx, "inspector-bounds");
            let collapse = layout_rect(&ctx, "inspector-toggle");
            assert!(
                inspector.contains_rect(collapse),
                "collapse must be in inspector header: {collapse:?} in {inspector:?}"
            );
            assert!(collapse.top() < inspector.top() + 40.);
            discard_frame(&mut app, &ctx, size, scale, click(collapse.center()));
            discard_frame(&mut app, &ctx, size, scale, vec![]);
            assert!(!app.inspector_open);
            let restore = layout_rect(&ctx, "inspector-toggle");
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(restore));
            assert!(restore.left() < 12. && restore.right() < 44.);
            discard_frame(&mut app, &ctx, size, scale, click(restore.center()));
            discard_frame(&mut app, &ctx, size, scale, vec![]);
            assert!(app.inspector_open);
        }
    }
}

#[test]
fn native_close_requests_protect_active_and_background_dirty_tabs() {
    for background in [false, true] {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
        if background {
            app.open_document(Document::new(8, 8), None);
        }
        let mut raw = input(Vec2::new(1180., 840.), 1., vec![]);
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let mut out = ctx.run_ui(raw, |ui| app.render(ui));
        assert!(
            out.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::CancelClose))
        );
        assert!(matches!(app.pending, Some(Pending::Close)));
        assert_eq!(app.active_tab, 0);
        assert!(app.dirty());
        out.textures_delta.clear();
    }
}

fn text_position(output: &mut egui::FullOutput, label: &str) -> Pos2 {
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing visible label: {label}"))
}

#[test]
fn view_menu_can_hide_and_restore_inspector() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-menu.png")));
    let size = Vec2::new(1180., 840.);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    for expected in [false, true] {
        let mut out = frame(&mut app, &ctx, size, 1., vec![]);
        let menu = text_position(&mut out, "View");
        discard_frame(&mut app, &ctx, size, 1., click(menu));
        let mut out = frame(&mut app, &ctx, size, 1., vec![]);
        let item = text_position(
            &mut out,
            if expected {
                "Show inspector"
            } else {
                "Hide inspector"
            },
        );
        discard_frame(&mut app, &ctx, size, 1., click(item));
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        assert_eq!(app.inspector_open, expected);
    }
}

#[test]
fn native_close_dialog_cancel_preserves_edits_and_discard_checks_remaining_tabs() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
    app.open_document(Document::new(8, 8), None);
    app.doc.set_pixel(0, 0, 0, [4, 5, 6, 255]);
    let size = Vec2::new(1180., 840.);
    let close = |app: &mut PixelCraft| {
        let mut raw = input(size, 1., vec![]);
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let mut out = ctx.run_ui(raw, |ui| app.render(ui));
        out.textures_delta.clear();
    };
    close(&mut app);
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let cancel = text_position(&mut out, "Cancel");
    discard_frame(&mut app, &ctx, size, 1., click(cancel));
    assert!(app.pending.is_none());
    assert!(app.dirty());
    close(&mut app);
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let discard = text_position(&mut out, "Discard changes");
    discard_frame(&mut app, &ctx, size, 1., click(discard));
    assert_eq!(app.active_tab, 0);
    assert!(app.dirty());
    assert!(matches!(app.pending, Some(Pending::Close)));
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let discard = text_position(&mut out, "Discard changes");
    let mut out = frame(&mut app, &ctx, size, 1., click(discard));
    assert!(
        out.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::Close))
    );
    out.textures_delta.clear();
}

#[test]
fn preview_fit_uses_available_width_and_preserves_sprite_aspect() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.doc = Document::new(200, 100);
    let mut output = ctx.run_ui(input(Vec2::new(640., 400.), 1., vec![]), |ui| {
        app.sprite_preview(ui);
    });
    output.textures_delta.clear();
    let texture = app.preview.as_ref().unwrap().id();
    let image = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
            _ => None,
        })
        .expect("sprite image painted");
    assert!(
        image.width() > 600.,
        "Fit should use the panel width: {image:?}"
    );
    assert!((image.width() / image.height() - 2.).abs() < 0.001);
    output.textures_delta.clear();
}

#[test]
fn seamless_preview_repeats_both_axes_and_png_export_writes_one_tile() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let tile = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
        [0, 255, 255, 255],
    ];
    app.replace(Document::new(3, 2), None);
    app.doc.add_frame(0, false);
    for (index, &color) in tile.iter().enumerate() {
        app.doc.set_pixel(0, 1, index, color);
    }
    app.frame = 1;
    app.action("seamless");
    assert_eq!(app.studio.seamless, WrapAxes::BOTH);

    let size = Vec2::new(1180., 840.);
    let mut texture = egui::TextureId::default();
    let mut texture_pixels = None;
    let mut tile_bounds = Vec::new();
    for _ in 0..4 {
        let mut output = ctx.run_ui(input(size, 1., vec![]), |ui| {
            texture = app.frame_texture(ui.ctx(), app.frame).id();
            egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
        });
        if let Some(deltas) = output.textures_delta.set.get(&texture) {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                assert_eq!(image.size, [3, 2]);
                texture_pixels = Some(image.pixels.clone());
            }
        }
        output.textures_delta.clear();
        tile_bounds = output
            .shapes
            .iter()
            .filter_map(|s| {
                let egui::Shape::Mesh(mesh) = &s.shape else {
                    return None;
                };
                if mesh.texture_id != texture {
                    return None;
                }
                let bounds = mesh.calc_bounds();
                assert!(s.clip_rect.contains_rect(bounds));
                assert!(mesh.vertices.iter().all(|vertex| {
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)).contains(vertex.uv)
                }));
                Some(bounds)
            })
            .collect();
    }
    assert_eq!(texture_pixels.unwrap(), color_image(&tile, 3, 2).pixels);
    assert_eq!(tile_bounds.len(), 9);
    let first = tile_bounds[0];
    assert!((first.width() / first.height() - 1.5).abs() < 0.02);
    for (index, bounds) in tile_bounds.iter().enumerate() {
        let offset = Vec2::new(
            (index % 3) as f32 * first.width(),
            (index / 3) as f32 * first.height(),
        );
        assert!((bounds.min - (first.min + offset)).length() < 2.);
        assert!((bounds.size() - first.size()).length() < 2.);
    }

    let path = std::env::temp_dir().join(format!("pixelcraft-seamless-{}.png", std::process::id()));
    app.start_export("png", path.clone());
    let (_, result) = app
        .export_job
        .take()
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    result.unwrap();
    let png = image::open(&path).unwrap().into_rgba8();
    std::fs::remove_file(path).unwrap();
    assert_eq!(png.dimensions(), (3, 2));
    assert_eq!(png.into_raw(), tile.concat());

    // Match both sets of opposite edges while the preview is still open.
    let fixed_tile = [tile[0], tile[1], tile[0], tile[0], tile[1], tile[0]];
    for (index, &color) in fixed_tile.iter().enumerate() {
        app.doc.set_pixel(0, 1, index, color);
    }
    let mut output = ctx.run_ui(input(size, 1., vec![]), |ui| {
        texture = app.frame_texture(ui.ctx(), app.frame).id();
        egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
    });
    let egui::ImageData::Color(image) = &output.textures_delta.set[&texture][0].image;
    assert_eq!(image.pixels, color_image(&fixed_tile, 3, 2).pixels);
    let refreshed_copies = output.shapes.iter().filter(|s| {
        matches!(
            &s.shape,
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture
        )
    });
    assert_eq!(refreshed_copies.count(), 9);
    output.textures_delta.clear();
}

#[test]
fn seamless_preview_keeps_tall_tile_rows_visible() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.replace(Document::new(4, 40), None);
    let size = Vec2::new(800., 480.);
    app.canvas_view_size = Some(size - Vec2::splat(16.));
    app.action("seamless");
    let mut visible_tiles = 0;
    for _ in 0..4 {
        let mut texture = egui::TextureId::default();
        let mut output = ctx.run_ui(input(size, 1., vec![]), |ui| {
            texture = app.frame_texture(ui.ctx(), app.frame).id();
            egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
        });
        output.textures_delta.clear();
        visible_tiles = output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(&shape.shape, egui::Shape::Mesh(mesh)
                    if mesh.texture_id == texture && shape.clip_rect.contains_rect(mesh.calc_bounds()))
            })
            .count();
    }
    assert_eq!(visible_tiles, 9);
}

#[test]
fn preview_resize_pan_and_preferences_survive_restart() {
    let size = Vec2::new(1366., 900.);
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let directory = std::env::temp_dir().join(format!("pixelcraft-preview-{}", std::process::id()));
    let path = directory.join("preview.json");
    let _ = std::fs::remove_dir_all(&directory);
    app.restore_preview_settings(path.clone());
    app.open_document(Document::new(256, 256), None);
    app.preview_settings.zoom = 4;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let panel = layout_rect(&ctx, "pinned-preview-bounds");
    let from = panel.center_top() + Vec2::new(0., 4.);
    drag_layout(&mut app, &ctx, size, from, from - Vec2::new(0., 90.));
    let taller = layout_rect(&ctx, "pinned-preview-bounds");
    assert!(
        taller.height() > panel.height() + 70.,
        "{panel:?} -> {taller:?}"
    );
    let viewport = layout_rect(&ctx, "preview-viewport");
    drag_layout(
        &mut app,
        &ctx,
        size,
        viewport.center(),
        viewport.center() + Vec2::new(30., 24.),
    );
    assert!(
        app.preview_pan.x > 20. && app.preview_pan.y > 15.,
        "{:?}",
        app.preview_pan
    );
    let mut restarted = PixelCraft::with_context(&egui::Context::default(), None);
    restarted.restore_preview_settings(path);
    assert_eq!(restarted.preview_settings.zoom, 4);
    assert_eq!(
        restarted.preview_settings.height,
        app.preview_settings.height
    );
    // A short window constrains the visible area without overwriting the user's height.
    let remembered = app.preview_settings.height;
    discard_frame(&mut app, &ctx, Vec2::new(683., 384.), 1., vec![]);
    assert_eq!(remembered, app.preview_settings.height);
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    assert!((layout_rect(&ctx, "pinned-preview-bounds").height() - taller.height()).abs() < 2.);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn editor_controls_dragging_indexed_swatches_preserves_pixels_and_undoes() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1366., 900.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.open_document(Document::new(4, 4), None);
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    app.doc.import_palette(vec![CLEAR, red, blue]).unwrap();
    app.doc.set_color_mode(ColorMode::Indexed);
    app.doc.set_pixel(0, 0, 0, red);
    app.palette_index = 1;
    app.fg = red;
    let before = app.doc.clone();
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let heading = text_position(&mut output, "Document palette");
    discard_frame(&mut app, &ctx, size, 1., click(heading));
    for _ in 0..8 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let swatch = |i: usize| {
        ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("document-swatch", i))))
            .unwrap()
            .center()
    };
    drag_layout(&mut app, &ctx, size, swatch(1), swatch(2));
    assert_eq!(app.doc.palette, vec![CLEAR, blue, red]);
    assert_eq!(app.doc.composite(0), before.composite(0));
    assert_eq!(app.palette_index, 2);
    app.undo();
    assert_eq!(app.doc, before);
}

#[test]
fn changing_default_sprite_width_preserves_the_current_pixel_grid() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1366., 900.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.grid = !app.power.default_grid();
    let current_grid = app.grid;
    let default_size = app.power.new_sprite_size();
    app.settings_open = true;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let heading = text_position(&mut output, "New sprites");
    discard_frame(&mut app, &ctx, size, 1., click(heading));
    for _ in 0..8 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let width = layout_rect(&ctx, "default-sprite-width").center();
    drag_layout(&mut app, &ctx, size, width, width + Vec2::new(15., 0.));
    assert_ne!(app.power.new_sprite_size(), default_size);
    assert_eq!(app.grid, current_grid);
}

#[test]
fn document_palette_preserves_opaque_first_swatch_in_rgba_mode() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1366., 900.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.open_document(Document::new(4, 4), None);
    let white = [255, 255, 255, 255];
    app.doc
        .import_palette(vec![white, [255, 0, 0, 255]])
        .unwrap();
    assert_eq!(app.doc.color_mode, ColorMode::Rgba);
    app.fg = [0, 0, 0, 255];
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let heading = text_position(&mut output, "Document palette");
    discard_frame(&mut app, &ctx, size, 1., click(heading));
    for _ in 0..8 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let swatch = ctx
        .data(|d| d.get_temp::<Rect>(egui::Id::new(("document-swatch", 0usize))))
        .unwrap()
        .center();
    discard_frame(&mut app, &ctx, size, 1., click(swatch));
    assert_eq!(app.fg, white);
}

#[test]
fn editor_controls_shade_direction_is_available_on_the_drawing_bar() {
    use pixelcraft::productivity::BrushMode;
    let ctx = egui::Context::default();
    let size = Vec2::new(800., 600.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.open_document(Document::new(3, 1), None);
    let palette = vec![[10, 0, 0, 255], [100, 0, 0, 255], [200, 0, 0, 255]];
    app.doc.palette = palette.clone();
    app.doc.set_pixel(0, 0, 0, palette[1]);
    app.doc.set_pixel(0, 0, 1, palette[1]);
    app.power.mode = BrushMode::Shade;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let previous = layout_rect(&ctx, "shade-previous").center();
    let next = layout_rect(&ctx, "shade-next").center();
    discard_frame(&mut app, &ctx, size, 1., click(previous));
    app.power_paint((0, 0), app.fg, 1);
    discard_frame(&mut app, &ctx, size, 1., click(next));
    app.power_paint((1, 0), app.fg, 1);
    assert_eq!(&app.doc.composite(0)[..2], &[palette[0], palette[2]]);
}

#[test]
fn preview_click_recenters_canvas_and_updates_viewport_border() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1366., 900.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.open_document(Document::new(128, 128), None);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 16.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);

    let image = layout_rect(&ctx, "preview-image");
    let picked = image.center() + image.size() * 0.2;
    discard_frame(&mut app, &ctx, size, 1., click(picked));
    assert!((app.pan - Vec2::splat(-409.6)).length() < 0.01);
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let border = layout_rect(&ctx, "preview-canvas-view");
    assert!((border.center() - picked).length() < 0.01);
}

#[test]
fn independent_preview_uses_authored_durations_and_keeps_edit_frame() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1366., 900.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.open_document(Document::new(16, 16), None);
    app.doc.add_frame(0, false);
    app.doc.durations = vec![50, 250];
    let mut render = |time, events| {
        let mut raw = input(size, 1., events);
        raw.time = Some(time);
        let mut output = ctx.run_ui(raw, |ui| app.render(ui));
        output.textures_delta.clear();
        assert_eq!(app.frame, 0);
        app.preview.as_ref().unwrap().id() == app.frame_texture(&ctx, 0).id()
    };
    assert!(render(0., vec![]));
    assert!(render(0.001, vec![]));
    let play = layout_rect(&ctx, "preview-play").center();
    assert!(render(0.01, click(play)));
    assert!(render(0.055, vec![]));
    assert!(!render(0.09, vec![]));
    assert!(!render(0.29, vec![]));
    assert!(!render(0.295, click(play)));
    assert!(!render(0.68, vec![]));
    assert!(!render(0.75, click(play)));
    assert!(!render(0.76, vec![]));
    assert!(render(0.78, vec![]));
}

#[test]
fn pixel_grid_clips_to_the_view_and_adapts_to_physical_zoom() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    app.replace(Document::new(8192, 80), None);
    app.initial_view = 0;
    app.pan = Vec2::new(109.5, 7.25);
    let window = Vec2::new(600., 280.);
    let dpi = 1.5;
    for (scale, step, window, aspect) in [
        (8., 1, window, [1, 1]),
        (3.5, 1, window, [1, 2]),
        (2.5, 2, window, [1, 1]),
        (1.5, 0, window, [1, 1]),
        (3.5, 4, Vec2::new(4500., 280.), [1, 1]),
    ] {
        app.zoom = scale / dpi;
        app.doc.pixel_aspect = Some(aspect);
        let mut lines = |show| {
            app.grid = show;
            let mut raw = input(window, dpi, vec![]);
            raw.max_texture_side = Some(8192);
            let mut frame = ctx.run_ui(raw, |ui| app.canvas(ui));
            frame.textures_delta.clear();
            frame
                .shapes
                .into_iter()
                .filter_map(|shape| match shape.shape {
                    egui::Shape::LineSegment { points, stroke } => Some((points, stroke)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let baseline = lines(false);
        let grid: Vec<_> = lines(true)
            .into_iter()
            .filter(|line| !baseline.contains(line))
            .collect();
        let view = layout_rect(&ctx, "canvas-bounds");
        let board = Rect::from_center_size(
            view.center() + app.pan,
            app.sprite_display_size() * app.zoom,
        );
        let visible = view.intersect(board).expand(0.001);
        if step == 0 {
            assert!(grid.is_empty());
            continue;
        }
        let pitch = app.canvas_pixel_size() * step as f32;
        let expected = (visible.width() / pitch.x + visible.height() / pitch.y).ceil() as usize;
        assert!(
            grid.len().abs_diff(expected) <= 3,
            "{scale}× physical zoom submitted {} lines, expected about {expected}",
            grid.len()
        );
        for (points, _) in grid {
            assert!(points.into_iter().all(|point| visible.contains(point)));
            let cell = if points[0].x == points[1].x {
                (points[0].x - board.min.x) / pitch.x
            } else {
                (points[0].y - board.min.y) / pitch.y
            };
            assert!(
                (cell - cell.round()).abs() < 0.002,
                "grid drifted off the sprite: {cell}"
            );
        }
    }
}

#[test]
fn fit_canvas_uses_the_active_pane_and_picks_clean_physical_zoom() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let viewport = Vec2::new(750., 480.);
    for (width, height) in [(120, 80), (1200, 720)] {
        app.replace(Document::new(width, height), None);
        for dpi in [1., 1.25, 1.5, 2.] {
            for split in [false, true] {
                if split != app.split_canvas.is_some() {
                    app.toggle_split_canvas();
                }
                let pane = if split {
                    CanvasPane::Secondary
                } else {
                    CanvasPane::Primary
                };
                app.active_canvas = pane;
                ctx.run_ui(input(viewport, dpi, vec![]), |ui| app.canvas_workspace(ui))
                    .textures_delta
                    .clear();
                app.set_active_canvas_pan(Vec2::new(25., -13.));
                let view = app.active_canvas_view().size.unwrap() - Vec2::splat(32.);
                let raw = (view.x / width as f32).min(view.y / height as f32) * dpi;
                let expected = if raw < 1. {
                    1. / (1. / raw).ceil()
                } else {
                    raw.floor()
                };
                let primary_zoom = app.zoom;
                app.action("fit");
                let actual = app.active_canvas_view().zoom * dpi;
                assert!(
                    (actual - expected).abs() < 0.0001,
                    "{pane:?}, {width}×{height} at {dpi}× DPI: {actual} versus {expected}"
                );
                assert_eq!(app.active_canvas_view().pan, Vec2::ZERO);
                if split {
                    assert_eq!(app.zoom, primary_zoom);
                }
            }
        }
    }
}

#[test]
fn preview_zoom_frame_sync_and_controls_across_sizes_and_scales() {
    for scale in [1., 1.25, 1.5, 2.] {
        for size in [
            Vec2::new(683., 384.),
            Vec2::new(1366., 768.),
            Vec2::new(1920., 1080.),
        ] {
            let ctx = egui::Context::default();
            let mut app = PixelCraft::with_context(&ctx, None);
            app.open_document(Document::new(320, 160), None);
            let tab = app.active_tab;
            app.doc.add_frame(0, false);
            for zoom in [0, 1, 2, 3, 4] {
                app.preview_settings.zoom = zoom;
                for _ in 0..3 {
                    discard_frame(&mut app, &ctx, size, scale, vec![]);
                }
                let inspector = layout_rect(&ctx, "inspector-bounds");
                let panel = layout_rect(&ctx, "pinned-preview-bounds");
                let viewport = layout_rect(&ctx, "preview-viewport");
                let image = layout_rect(&ctx, "preview-image");
                let controls = layout_rect(&ctx, "preview-controls");
                assert!((panel.bottom() - inspector.bottom()).abs() < 4.);
                assert!(
                    panel.contains_rect(controls),
                    "{size:?} @ {scale}: {controls:?} in {panel:?}"
                );
                assert!(panel.contains_rect(viewport));
                assert!(controls.bottom() <= viewport.top());
                if zoom == 0 {
                    assert!(
                        image.width() <= viewport.width() + 1.
                            && image.height() <= viewport.height() + 1.
                    );
                    assert!((image.width() / image.height() - 2.).abs() < 0.001);
                } else {
                    assert!(
                        (image.width() * ctx.pixels_per_point() - 320. * zoom as f32).abs() < 0.01
                    );
                }
                let _ = app.frame_texture(&ctx, app.next_playback_frame().unwrap().0);
                app.playing = true;
                app.last_tick =
                    Instant::now() - Duration::from_millis(u64::from(app.doc.durations[app.frame]));
                let previous = app.frame;
                discard_frame(&mut app, &ctx, size, scale, vec![]);
                assert_ne!(previous, app.frame);
                assert_eq!(
                    app.preview.as_ref().unwrap().id(),
                    app.frame_texture(&ctx, app.frame).id()
                );
                app.playing = false;
            }
            app.open_document(Document::new(40, 80), None);
            discard_frame(&mut app, &ctx, size, scale, vec![]);
            assert_eq!(app.preview.as_ref().unwrap().size(), [40, 80]);
            app.switch_tab(tab);
            discard_frame(&mut app, &ctx, size, scale, vec![]);
            assert_eq!(app.preview.as_ref().unwrap().size(), [320, 160]);
            assert_eq!(app.preview_settings.zoom, 4);
        }
    }
}

#[test]
fn preview_zoom_menu_selects_and_saves_fit_and_integer_choices() {
    let ctx = egui::Context::default();
    let size = Vec2::new(683., 500.);
    let mut app = PixelCraft::with_context(&ctx, None);
    let directory =
        std::env::temp_dir().join(format!("pixelcraft-preview-menu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    app.restore_preview_settings(directory.join("preview.json"));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1.5, vec![]);
    }
    for (zoom, label) in [(2, "2×"), (1, "1:1"), (3, "3×"), (4, "4×"), (0, "Fit")] {
        let previous = match app.preview_settings.zoom {
            0 => "Fit".into(),
            1 => "1:1".into(),
            n => format!("{n}×"),
        };
        let mut output = frame(&mut app, &ctx, size, 1.5, vec![]);
        output.textures_delta.clear();
        let controls = layout_rect(&ctx, "preview-controls");
        let selector = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == previous && controls.contains(text.pos) =>
                {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("Preview zoom selector");
        discard_frame(&mut app, &ctx, size, 1.5, click(selector));
        let mut output = frame(&mut app, &ctx, size, 1.5, vec![]);
        output.textures_delta.clear();
        let option = output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("Preview zoom option");
        discard_frame(&mut app, &ctx, size, 1.5, click(option));
        assert_eq!(app.preview_settings.zoom, zoom);
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("preview.json")).unwrap())
                .unwrap();
        assert_eq!(saved["zoom"], zoom);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn interaction_audit_buttons_and_switches_obey_enabled_and_keyboard_states() {
    for dark in [false, true] {
        let ctx = egui::Context::default();
        let mut app = PixelCraft::with_context(&ctx, None);
        app.set_dark_mode(&ctx, dark);
        let mut value = false;
        let mut hits = 0;
        let mut button = Rect::NOTHING;
        let mut switch = Rect::NOTHING;
        let mut draw = |enabled: bool, events: Vec<egui::Event>, focus: u8| {
            let mut out = ctx.run_ui(input(Vec2::new(400., 180.), 1., events), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let response = ui_icons::button(ui, "play", "Play", false, false);
                    button = response.rect;
                    assert_eq!(response.enabled(), enabled);
                    if focus == 1 {
                        response.request_focus();
                    }
                    if response.clicked() {
                        hits += 1;
                    }
                    let response = fluent::toggle(ui, &mut value, "Pixel grid");
                    switch = response.rect;
                    assert_eq!(response.enabled(), enabled);
                    if focus == 2 {
                        response.request_focus();
                    }
                });
            });
            out.textures_delta.clear();
            (button, switch, hits, value)
        };
        let (b, s, _, _) = draw(false, vec![], 0);
        assert_eq!(draw(false, click(b.center()), 0).2, 0);
        assert!(!draw(false, click(s.center()), 0).3);
        draw(true, vec![], 0);
        assert_eq!(draw(true, click(b.center()), 0).2, 1);
        assert!(draw(true, click(s.center()), 0).3);
        draw(true, vec![], 1);
        assert_eq!(draw(true, vec![key_event(Key::Enter)], 0).2, 2);
        let mut release = key_event(Key::Enter);
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        draw(true, vec![release], 2);
        assert!(!draw(true, vec![key_event(Key::Enter)], 0).3);
    }
}

#[test]
fn artwork_swatch_accessibility_updates_when_selected() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(8, 8);
    let color = [220, 30, 70, 128];
    doc.set_pixel(0, 0, 0, color);
    app.replace(doc, None);
    let size = Vec2::new(264., 768.);
    let draw = |app: &mut PixelCraft, events| {
        let mut out = ctx.run_ui(input(size, 1., events), |ui| app.palette(ui));
        out.textures_delta.clear();
        out
    };
    let swatch = |out: &egui::FullOutput| {
        out.platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Use #DC1E4680"))
            .expect("artwork swatch has an accessible RGBA name")
            .clone()
    };
    let (id, before) = swatch(&draw(&mut app, vec![]));
    assert_eq!(before.toggled(), Some(egui::accesskit::Toggled::False));
    let bounds = before.bounds().unwrap();
    let center = Pos2::new(
        ((bounds.x0 + bounds.x1) / 2.) as f32,
        ((bounds.y0 + bounds.y1) / 2.) as f32,
    );
    let (selected_id, after) = swatch(&draw(&mut app, click(center)));
    assert_eq!(id, selected_id);
    assert_eq!(after.toggled(), Some(egui::accesskit::Toggled::True));
    assert_eq!(app.fg, color);
}

#[test]
fn edit_menu_history_actions_follow_active_tab_history() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("/tmp/unused-history.png")));
    let size = Vec2::new(1180., 840.);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let edit = text_position(&mut frame(&mut app, &ctx, size, 1., vec![]), "Edit");
    discard_frame(&mut app, &ctx, size, 1., click(edit));
    for (undo_disabled, redo_disabled) in [(true, true), (false, true), (true, false), (true, true)]
    {
        let mut out = frame(&mut app, &ctx, size, 1., vec![]);
        out.textures_delta.clear();
        let tree = out.platform_output.accesskit_update.as_ref().unwrap();
        for (label, disabled) in [("Undo", undo_disabled), ("Redo", redo_disabled)] {
            let (_, node) = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap();
            assert_eq!(node.is_disabled(), disabled, "{label}");
        }
        match (undo_disabled, redo_disabled) {
            (true, true) => {
                app.remember();
                app.doc.set_pixel(0, 0, 0, [1, 2, 3, 255]);
            }
            (false, true) => app.undo(),
            (true, false) => app.open_document(Document::new(8, 8), None),
            _ => unreachable!(),
        }
    }
}

#[test]
fn modal_header_drags_and_backdrop_blocks_editor_clicks() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut app = PixelCraft::with_context(&ctx, None);
    let size = Vec2::new(1180., 840.);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let new_button = ctx.data(|data| {
        data.get_temp::<Rect>(egui::Id::new(("ui-icon-test-target", "new")))
            .unwrap()
    });
    app.new_dialog = true;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let close_bounds = |app: &mut PixelCraft, events| {
        let mut out = frame(app, &ctx, size, 1., events);
        out.textures_delta.clear();
        let bounds = out
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Close dialog"))
            .unwrap()
            .1
            .bounds()
            .unwrap();
        Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        )
    };
    let before = close_bounds(&mut app, vec![]);
    let hover = close_bounds(&mut app, vec![egui::Event::PointerMoved(before.center())]);
    assert_eq!(before.size(), Vec2::splat(34.));
    assert_eq!(before, hover);
    let header = before.center() - Vec2::new(100., 0.);
    drag_layout(&mut app, &ctx, size, header, header + Vec2::new(96., 54.));
    let after = close_bounds(&mut app, vec![]);
    assert_eq!(after.size(), before.size());
    assert!(after.left() - before.left() > 80.);
    assert!(after.top() - before.top() > 40.);
    assert!(app.new_dialog);

    discard_frame(&mut app, &ctx, size, 1., click(new_button.center()));
    assert!(!app.new_dialog);
    assert_eq!(app.tabs.len(), 1);

    app.doc.set_pixel(0, 0, 0, [12, 34, 56, 255]);
    app.pending = Some(Pending::CloseTab);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let close = close_bounds(&mut app, vec![]);
    discard_frame(&mut app, &ctx, size, 1., click(close.center()));
    assert!(app.pending.is_none());
    assert!(app.dirty());
    assert_eq!(app.tabs.len(), 1);
    app.help = true;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let close = close_bounds(&mut app, vec![]).center();
    let mut hover = frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(close)],
    );
    hover.textures_delta.clear();
    assert_eq!(
        hover.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    let [_, press, release] = click(close).try_into().unwrap();
    discard_frame(&mut app, &ctx, size, 1., vec![press]);
    assert!(app.help);
    discard_frame(&mut app, &ctx, size, 1., vec![release]);
    assert!(!app.help);
}

#[test]
fn frame_cache_retains_unaffected_frames_and_refreshes_linked_cels() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(8, 8);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.link_cel(0, 1, 0).unwrap();
    app.replace(doc, None);
    let linked = app.frame_texture(&ctx, 1);
    let unaffected = app.frame_texture(&ctx, 2);
    app.doc.set_pixel(0, 0, 0, [40, 90, 120, 255]);
    let edited = app.frame_texture(&ctx, 0);
    assert_eq!(app.frame_texture(&ctx, 2).id(), unaffected.id());
    assert_ne!(app.frame_texture(&ctx, 1).id(), linked.id());
    assert_eq!(app.frame_texture(&ctx, 0).id(), edited.id());
    assert_eq!(app.doc.composite(1)[0], [40, 90, 120, 255]);
}

#[test]
fn playback_prefetch_rejects_frames_from_an_obsolete_snapshot() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(32, 32);
    doc.add_frame(0, false);
    app.replace(doc, None);
    app.playing = true;
    let _ = app.frame_texture(&ctx, 0);
    assert!(app.frame_prefetch.is_some());
    app.doc.set_pixel(0, 1, 0, [190, 90, 20, 255]);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let _ = app.frame_texture(&ctx, 0);
        if app.frame_textures[1].is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "Next frame was not prefetched");
        std::thread::sleep(Duration::from_millis(1));
    }
    let texture = app.frame_textures[1].as_ref().unwrap().id();
    let mut updates = ctx.tex_manager().write().take_delta();
    let egui::ImageData::Color(image) = &updates.set[&texture][0].image;
    assert_eq!(image.pixels[0], Color32::from_rgb(190, 90, 20));
    updates.clear();
}

#[test]
fn playback_waits_for_a_cache_miss_without_skipping_the_frame() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(32, 32);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.durations.fill(17);
    app.replace(doc, None);
    let _ = app.frame_texture(&ctx, 0);
    app.playing = true;
    let start = Instant::now();
    app.last_tick = start;

    assert_eq!(
        app.update_playback_clock(start + Duration::from_millis(16)),
        Duration::from_millis(1),
        "An unready cel before its deadline is not a cache miss yet"
    );
    assert!(!app.playback_waiting_for_frame);
    app.update_playback_clock(start + Duration::from_millis(40));
    assert_eq!(
        app.frame, 0,
        "Hold the last frame while its successor renders"
    );

    let (image, error) = PixelCraft::compose_frame_image(&app.doc, 1, &[]);
    app.store_frame_texture(&ctx, 1, image, error);
    app.update_playback_clock(start + Duration::from_millis(80));
    assert_eq!(app.frame, 1, "Present the first ready successor");
    assert_eq!(
        app.update_playback_clock(start + Duration::from_millis(81)),
        Duration::from_millis(16)
    );
    assert_eq!(app.frame, 1);
}

#[test]
fn cached_fast_playback_keeps_frame_order_across_repaint_jitter() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(4, 4);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.durations = vec![17, 16, 17];
    app.replace(doc, None);
    for f in 0..3 {
        let _ = app.frame_texture(&ctx, f);
    }
    app.playing = true;
    let start = Instant::now();
    let repaint_schedules = [
        [16, 33, 50, 66, 83, 100, 116, 133, 150],
        [34, 50, 67, 84, 100, 117, 134, 150, 167],
    ];
    for schedule in repaint_schedules {
        app.frame = 0;
        app.last_tick = start;
        for (redraw, millis) in schedule.into_iter().enumerate() {
            app.update_playback_clock(start + Duration::from_millis(millis));
            assert_eq!(app.frame, (redraw + 1) % 3, "redraw at {millis} ms");
        }
    }
}

#[test]
fn preview_visibility_is_independent_and_survives_restart() {
    let size = Vec2::new(1180., 840.);
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let directory = std::env::temp_dir().join(format!(
        "pixelcraft-preview-visibility-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("preview.json");
    std::fs::write(&path, br#"{"height":220,"zoom":3}"#).unwrap();
    app.restore_preview_settings(path.clone());

    let select = |app: &mut PixelCraft, label| {
        let mut output = frame(app, &ctx, size, 1., vec![]);
        let view = text_position(&mut output, "View");
        discard_frame(app, &ctx, size, 1., click(view));
        let mut output = frame(app, &ctx, size, 1., vec![]);
        let item = text_position(&mut output, label);
        discard_frame(app, &ctx, size, 1., click(item));
        discard_frame(app, &ctx, size, 1., vec![]);
    };
    let preview_visible = |ctx: &egui::Context| {
        ctx.data(|data| {
            data.get_temp::<Rect>(egui::Id::new("pinned-preview-bounds"))
                .is_some()
        })
    };
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(
        preview_visible(&ctx),
        "older preferences default to visible"
    );
    select(&mut app, "Hide Preview");
    assert!(app.inspector_open);
    assert!(!preview_visible(&ctx));
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["visible"], false);
    assert_eq!(saved["height"], 220.0);
    assert_eq!(saved["zoom"], 3);

    let restarted = egui::Context::default();
    let mut loaded = PixelCraft::with_context(&restarted, None);
    loaded.restore_preview_settings(path);
    discard_frame(&mut loaded, &restarted, size, 1., vec![]);
    assert!(loaded.inspector_open);
    assert!(!preview_visible(&restarted));

    select(&mut app, "Hide inspector");
    assert!(!app.inspector_open);
    assert!(!preview_visible(&ctx));
    let wide = layout_rect(&ctx, "canvas-bounds").width();
    select(&mut app, "Show Preview");
    assert!(!app.inspector_open);
    assert!(preview_visible(&ctx));
    assert!(layout_rect(&ctx, "canvas-bounds").width() < wide);
    select(&mut app, "Show inspector");
    assert!(app.inspector_open && preview_visible(&ctx));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn marquee_drag_modifiers_preserve_mask_holes_and_history() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.replace(Document::new(6, 3), None);
    app.tool = Tool::Select;
    let mut original = vec![false; 18];
    for index in [0, 4, 12] {
        original[index] = true;
    }
    app.set_selection_mask(original.clone());
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let pixel = |x: f32, y: f32| {
        board.min
            + Vec2::new(
                (x + 0.5) * board.width() / 6.,
                (y + 0.5) * board.height() / 3.,
            )
    };
    let pointer = |pos, pressed, modifiers| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    let render = |app: &mut PixelCraft, modifiers, mut events: Vec<_>| {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let mut output = frame(app, &ctx, size, 1., events);
        output.textures_delta.clear();
    };

    let shift = egui::Modifiers::SHIFT;
    let from = pixel(2., 0.);
    let to = pixel(4., 1.);
    render(
        &mut app,
        shift,
        vec![egui::Event::PointerMoved(from), pointer(from, true, shift)],
    );
    render(&mut app, shift, vec![egui::Event::PointerMoved(to)]);
    assert_eq!(app.studio.mask.as_ref(), Some(&original));
    render(
        &mut app,
        egui::Modifiers::NONE,
        vec![pointer(to, false, egui::Modifiers::NONE)],
    );
    let mut added = original.clone();
    for index in [2, 3, 8, 9, 10] {
        added[index] = true;
    }
    assert_eq!(app.studio.mask.as_ref(), Some(&added));

    let command = egui::Modifiers::COMMAND;
    let from = pixel(3., 1.);
    let to = pixel(5., 1.);
    render(
        &mut app,
        command,
        vec![
            egui::Event::PointerMoved(from),
            pointer(from, true, command),
        ],
    );
    render(
        &mut app,
        command,
        vec![egui::Event::PointerMoved(to), pointer(to, false, command)],
    );
    let mut subtracted = added.clone();
    subtracted[9..=10].fill(false);
    assert_eq!(app.studio.mask.as_ref(), Some(&subtracted));
    assert_eq!(app.undo.len(), 2);
    app.undo();
    assert_eq!(app.studio.mask.as_ref(), Some(&added));
    app.redo();
    assert_eq!(app.studio.mask.as_ref(), Some(&subtracted));

    let intersection = shift | command;
    let from = pixel(1., 0.);
    let to = pixel(3., 1.);
    render(
        &mut app,
        intersection,
        vec![
            egui::Event::PointerMoved(from),
            pointer(from, true, intersection),
        ],
    );
    render(
        &mut app,
        intersection,
        vec![
            egui::Event::PointerMoved(to),
            pointer(to, false, intersection),
        ],
    );
    let mut intersected = vec![false; 18];
    for index in [2, 3, 8] {
        intersected[index] = true;
    }
    assert_eq!(app.studio.mask.as_ref(), Some(&intersected));
    let history = app.undo.len();
    render(
        &mut app,
        egui::Modifiers::NONE,
        vec![
            egui::Event::PointerMoved(from),
            pointer(from, true, egui::Modifiers::NONE),
        ],
    );
    app.cancel_gesture();
    render(
        &mut app,
        egui::Modifiers::NONE,
        vec![pointer(to, false, egui::Modifiers::NONE)],
    );
    assert_eq!(app.studio.mask.as_ref(), Some(&intersected));
    assert_eq!(app.undo.len(), history);

    render(
        &mut app,
        egui::Modifiers::NONE,
        vec![
            egui::Event::PointerMoved(to),
            pointer(to, true, egui::Modifiers::NONE),
            pointer(to, false, egui::Modifiers::NONE),
        ],
    );
    assert_eq!(app.selection, Some((3, 1, 3, 1)));
    assert!(app.studio.mask.is_none());
}

#[test]
fn secondary_palette_swatch_and_mouse_button_keep_their_own_indexed_stroke() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, None);
    let red = [223, 31, 41, 255];
    let blue = [27, 67, 190, 255];
    let mut doc = Document::new(6, 2);
    doc.import_palette(vec![CLEAR, red, blue, blue]).unwrap();
    doc.set_color_mode(ColorMode::Indexed);
    app.replace(doc, None);
    app.tool = Tool::Pencil;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let heading = text_position(&mut output, "Document palette");
    discard_frame(&mut app, &ctx, size, 1., click(heading));
    for _ in 0..8 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let swatch = |slot: usize| {
        ctx.data(|data| data.get_temp::<Rect>(egui::Id::new(("document-swatch", slot))))
            .unwrap()
            .center()
    };
    let button = |pos, button, pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let left = egui::PointerButton::Primary;
    let right = egui::PointerButton::Secondary;
    let mouse = |app: &mut PixelCraft, p, b, pressed| {
        discard_frame(
            app,
            &ctx,
            size,
            1.,
            vec![egui::Event::PointerMoved(p), button(p, b, pressed)],
        );
    };
    let primary = swatch(1);
    discard_frame(&mut app, &ctx, size, 1., click(primary));
    let secondary = swatch(3);
    mouse(&mut app, secondary, right, true);
    mouse(&mut app, secondary, right, false);
    assert_eq!((app.fg, app.bg), (red, blue));

    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let pixel = |x: f32, y: f32| {
        board.min
            + Vec2::new(
                (x + 0.5) * board.width() / 6.,
                (y + 0.5) * board.height() / 2.,
            )
    };
    mouse(&mut app, pixel(0., 0.), left, true);
    mouse(&mut app, pixel(0., 0.), left, false);
    mouse(&mut app, pixel(1., 0.), right, true);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(pixel(2., 0.))],
    );
    mouse(&mut app, pixel(2., 0.), right, false);
    app.tool = Tool::Line;
    mouse(&mut app, pixel(3., 0.), right, true);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(pixel(5., 0.))],
    );
    mouse(&mut app, pixel(5., 0.), right, false);

    app.tool = Tool::Pencil;
    let bottom = pixel(0., 1.);
    mouse(&mut app, bottom, right, true);
    mouse(&mut app, bottom, left, true);
    mouse(&mut app, bottom, right, false);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(pixel(3., 1.))],
    );
    mouse(&mut app, pixel(3., 1.), left, false);
    let indices = app.doc.layers[0].cel_meta[0].indices.as_deref().unwrap();
    assert_eq!(indices.as_slice(), &[1, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0]);
    app.undo();
    assert_eq!(
        app.doc.layers[0].cel_meta[0].indices.as_ref().unwrap()[6],
        0
    );
    app.move_palette_swatch(3, 2);
    assert_eq!(app.secondary_palette_index, Some(2));
    app.tool = Tool::Fill;
    mouse(&mut app, bottom, right, true);
    mouse(&mut app, bottom, right, false);
    let indices = app.doc.layers[0].cel_meta[0].indices.as_deref().unwrap();
    assert_eq!(indices.as_slice(), &[1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]);
    assert_eq!((app.fg, app.bg), (red, blue));
}

#[test]
fn file_menu_accepts_typed_paths_and_confirms_export_overwrite() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, None);
    let directory =
        std::env::temp_dir().join(format!("pixelcraft-typed-paths-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("Original sprite.pixel");
    let saved = directory.join("Saved sprite.pixel");
    let exported = directory.join("Exported sprite.png");
    let color = [41, 78, 127, 255];
    let mut doc = Document::new(3, 2);
    doc.set_pixel(0, 0, 0, color);
    formats::save(&source, &doc).unwrap();
    std::fs::write(&exported, b"preserve until confirmed").unwrap();

    let type_path = |app: &mut PixelCraft, path: &Path, action: &str| {
        let status = app.status.clone();
        app.file_path
            .as_mut()
            .unwrap_or_else(|| panic!("Missing {action} dialog for {path:?}: {status}"))
            .input
            .clear();
        for _ in 0..3 {
            discard_frame(app, &ctx, size, 1., vec![]);
        }
        let mut output = frame(
            app,
            &ctx,
            size,
            1.,
            vec![egui::Event::Text(format!("\"{}\"", path.display()))],
        );
        let _ = text_position(&mut output, "Browse…");
        let accept = output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == action => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("Path dialog submit button is visible");
        discard_frame(app, &ctx, size, 1., click(accept));
    };
    let poll = |app: &mut PixelCraft| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while (app.open_job.is_some() || app.save_job.is_some() || app.export_job.is_some())
            && Instant::now() < deadline
        {
            discard_frame(app, &ctx, size, 1., vec![]);
            std::thread::yield_now();
        }
        assert!(app.open_job.is_none() && app.save_job.is_none() && app.export_job.is_none());
    };

    app.action("open");
    type_path(&mut app, &source, "Open");
    poll(&mut app);
    assert_eq!(app.path.as_ref(), Some(&source));
    assert_eq!(app.doc.composite(0)[0], color);

    app.action("saveas");
    type_path(&mut app, &saved.with_extension(""), "Save");
    poll(&mut app);
    assert_eq!(app.path.as_ref(), Some(&saved));
    assert_eq!(formats::load(&saved).unwrap().composite(0)[0], color);

    app.action("png");
    type_path(&mut app, &exported, "Export");
    assert!(app.export_job.is_none());
    assert_eq!(
        std::fs::read(&exported).unwrap(),
        b"preserve until confirmed"
    );
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let cancel = text_position(&mut output, "Cancel");
    discard_frame(&mut app, &ctx, size, 1., click(cancel));
    assert!(app.file_path.is_none());
    assert_eq!(
        std::fs::read(&exported).unwrap(),
        b"preserve until confirmed"
    );
    app.action("png");
    type_path(&mut app, &exported, "Export");
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let replace = text_position(&mut output, "Replace file");
    discard_frame(&mut app, &ctx, size, 1., click(replace));
    poll(&mut app);
    assert_eq!(
        image::open(&exported).unwrap().to_rgba8().get_pixel(0, 0).0,
        color
    );
    app.action("import-layer");
    let layer_count = app.doc.layers.len();
    type_path(&mut app, &exported, "Import");
    poll(&mut app);
    assert_eq!(app.doc.layers.len(), layer_count + 1);
    assert_eq!(app.doc.layers[app.layer].cels[0][0], color);

    app.action("png-sequence");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let choose = text_position(&mut output, "Choose export location…");
    discard_frame(&mut app, &ctx, size, 1., click(choose));
    let sequence_path = directory.join(&app.sequence_name);
    type_path(&mut app, &directory, "Export");
    poll(&mut app);
    assert!(std::fs::read_dir(sequence_path).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "png")
    }));

    app.open_document(Document::new(1, 1), None);
    app.doc.set_pixel(0, 0, 0, color);
    let tabs = app.tabs.len();
    app.request(Pending::CloseTab);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let save = text_position(&mut output, "Save");
    discard_frame(&mut app, &ctx, size, 1., click(save));
    let closed = directory.join("Saved while closing.pixel");
    type_path(&mut app, &closed, "Save");
    assert_eq!(formats::load(&closed).unwrap().composite(0)[0], color);
    assert_eq!(app.tabs.len(), tabs - 1);
    assert!(app.pending.is_none());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn shortcut_capture_checks_conflicts_reserved_keys_escape_and_command_reset() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, None);
    app.tool = Tool::Eraser;
    let directory = std::env::temp_dir().join(format!(
        "pixelcraft-shortcut-capture-{}",
        std::process::id()
    ));
    app.shortcuts_path = Some(directory.join("shortcuts.json"));
    let key = |key, modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let click_button = |app: &mut PixelCraft, label| {
        let mut output = frame(app, &ctx, size, 1., vec![]);
        let center = text_position(&mut output, label);
        discard_frame(app, &ctx, size, 1., click(center));
    };
    let no_mods = egui::Modifiers::NONE;
    app.action("shortcuts");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    click_button(&mut app, "Capture shortcut…");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    discard_frame(&mut app, &ctx, size, 1., vec![key(Key::B, no_mods)]);
    assert!(
        app.tool == Tool::Eraser,
        "Capture must not trigger the editor action"
    );
    assert_eq!(
        app.custom_shortcuts.get("B").map(String::as_str),
        Some("pencil")
    );
    click_button(&mut app, "Replace shortcut");
    assert_eq!(
        app.custom_shortcuts.get("B").map(String::as_str),
        Some("wand")
    );

    let bindings = app.custom_shortcuts.clone();
    app.begin_shortcut_capture(&ctx, "wand".into());
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![key(Key::V, egui::Modifiers::COMMAND)],
    );
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let _ = text_position(&mut output, "Ctrl+V is reserved.");
    discard_frame(&mut app, &ctx, size, 1., vec![egui::Event::Copy]);
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let _ = text_position(&mut output, "Clipboard shortcuts are reserved.");
    discard_frame(&mut app, &ctx, size, 1., vec![key(Key::Escape, no_mods)]);
    assert!(app.shortcut_edit.is_none());
    assert_eq!(app.custom_shortcuts, bindings);

    app.custom_shortcuts.insert("Cmd+F8".into(), "fill".into());
    app.begin_shortcut_capture(&ctx, "wand".into());
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![key(Key::F8, egui::Modifiers::COMMAND)],
    );
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let _ = text_position(&mut output, "Ctrl+F8 is assigned to Paint bucket");
    click_button(&mut app, "Cancel");
    assert_eq!(
        app.custom_shortcuts.get("Cmd+F8").map(String::as_str),
        Some("fill")
    );

    app.custom_shortcuts.insert("W".into(), "hand".into());
    app.begin_shortcut_reset("wand".into());
    assert_eq!(
        app.custom_shortcuts.get("W").map(String::as_str),
        Some("hand")
    );
    click_button(&mut app, "Replace and reset");
    assert_eq!(
        app.custom_shortcuts.get("W").map(String::as_str),
        Some("wand")
    );
    assert!(!app.custom_shortcuts.contains_key("B"));
    let saved = std::fs::read(app.shortcuts_path.as_ref().unwrap()).unwrap();
    assert_eq!(
        pixelcraft::productivity::load_shortcuts(&saved).unwrap(),
        app.custom_shortcuts
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn eyedropper_source_and_channels_target_each_drawing_color() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-eyedropper.png")));
    let mut doc = Document::new(4, 2);
    doc.layers[0].cels[0].set(0, [170, 70, 30, 255]);
    doc.add_layer();
    doc.layers[1].cels[0].set(0, [21, 61, 171, 102]);
    doc.references
        .push(pixelcraft::productivity::ReferenceImage {
            name: "Ref".into(),
            width: 1,
            height: 1,
            pixels: vec![[200, 130, 80, 180]].into(),
            x: 0.,
            y: 0.,
            scale: 1.,
            opacity: 255,
            visible: true,
            behind: false,
            floating: false,
            options: Default::default(),
        });
    let merged = doc.composite(0)[0];
    app.replace(doc, None);
    app.layer = 1;
    app.tool = Tool::Picker;
    app.fg = [1, 2, 3, 255];
    app.bg = [50, 60, 70, 210];
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 22.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let point = board.min + Vec2::new(board.width() / 8., board.height() / 4.);
    let click_color = |app: &mut PixelCraft, button| {
        discard_frame(
            app,
            &ctx,
            size,
            1.,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        discard_frame(
            app,
            &ctx,
            size,
            1.,
            vec![egui::Event::PointerButton {
                pos: point,
                button,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    };
    let choose = |app: &mut PixelCraft, current, option| {
        let mut out = frame(app, &ctx, size, 1., vec![]);
        let combo = text_position(&mut out, current);
        discard_frame(app, &ctx, size, 1., click(combo));
        let mut out = frame(app, &ctx, size, 1., vec![]);
        let option = text_position(&mut out, option);
        discard_frame(app, &ctx, size, 1., click(option));
    };
    click_color(&mut app, egui::PointerButton::Primary);
    assert_eq!(app.fg, merged);
    choose(&mut app, "Visible layers", "Active layer");
    choose(&mut app, "Color and alpha", "Alpha only");
    click_color(&mut app, egui::PointerButton::Secondary);
    assert_eq!(app.bg, [50, 60, 70, 102]);
    assert_eq!(app.fg, merged);

    choose(&mut app, "Alpha only", "Red only");
    click_color(&mut app, egui::PointerButton::Primary);
    assert_eq!(app.fg, [21, merged[1], merged[2], 255]);

    choose(&mut app, "Active layer", "Reference images");
    choose(&mut app, "Red only", "Color and alpha");
    click_color(&mut app, egui::PointerButton::Primary);
    assert_eq!(app.fg, [200, 130, 80, 180]);

    let duplicate = [35, 85, 155, 255];
    app.doc
        .import_palette(vec![CLEAR, duplicate, duplicate])
        .unwrap();
    app.doc.set_color_mode(ColorMode::Indexed);
    app.doc.drawing_index = Some(2);
    app.doc.set_pixel(1, 0, 0, duplicate);
    app.doc.drawing_index = None;
    choose(&mut app, "Reference images", "Active layer");
    choose(&mut app, "Color and alpha", "Palette index");
    click_color(&mut app, egui::PointerButton::Secondary);
    assert_eq!(app.secondary_palette_index, Some(2));
    assert_eq!(app.bg, duplicate);
    assert!(app.undo.is_empty());
}

#[test]
fn symmetry_handles_move_independently_and_reflect_pencil_strokes() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-symmetry.png")));
    app.replace(Document::new(12, 10), None);
    app.studio.mirror_x = true;
    app.studio.mirror_y = true;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let zoom = app.zoom;
    let pixel = |x: f32, y: f32| board.min + Vec2::new(x, y) * zoom;
    let x_handle = layout_rect(&ctx, "symmetry-x-handle").center();
    drag_layout(
        &mut app,
        &ctx,
        size,
        x_handle,
        Pos2::new(pixel(3., 0.).x, x_handle.y),
    );
    let y_handle = layout_rect(&ctx, "symmetry-y-handle").center();
    drag_layout(
        &mut app,
        &ctx,
        size,
        y_handle,
        Pos2::new(y_handle.x, pixel(0., 3.5).y),
    );
    assert!(app.undo.is_empty(), "moving guides must not paint");

    discard_frame(&mut app, &ctx, size, 1., click(pixel(1.5, 1.5)));
    let painted: Vec<_> = app.doc.layers[0].cels[0]
        .iter()
        .enumerate()
        .filter_map(|(index, color)| (color[3] > 0).then_some(index))
        .collect();
    assert_eq!(painted, [13, 16, 61, 64]);
    app.undo();
    assert!(
        app.doc.layers[0].cels[0]
            .iter()
            .all(|color| *color == CLEAR)
    );
    assert_eq!([app.symmetry_axis(0), app.symmetry_axis(1)], [6, 7]);
    let original_tab = app.active_tab;
    app.open_document(Document::new(12, 10), None);
    assert_eq!([app.symmetry_axis(0), app.symmetry_axis(1)], [12, 10]);
    app.switch_tab(original_tab);
    assert_eq!([app.symmetry_axis(0), app.symmetry_axis(1)], [6, 7]);
}

#[test]
fn display_profile_changes_canvas_preview_and_prefetch_without_editing_artwork() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, None);
    let mut doc = Document::new(1, 1);
    doc.layers[0].cels[0].set(0, [128, 128, 128, 255]);
    doc.add_frame(0, true);
    app.replace(doc.clone(), None);
    let initial = app.frame_texture(&ctx, 0);
    app.update_preview_texture(&ctx, 1);
    app.preview_prefetch = Some(FramePrefetch::start(&ctx, &app.doc, 1, Arc::default()));
    let mut linear = moxcms::ColorProfile::new_srgb();
    linear.cicp = None;
    linear.red_trc = Some(moxcms::ToneReprCurve::Parametric(vec![1.]));
    linear.green_trc = linear.red_trc.clone();
    linear.blue_trc = linear.red_trc.clone();
    let target = pixelcraft::color::icc_profile(&linear.encode().unwrap()).unwrap();
    app.set_display_profile_bytes(target.clone());
    assert!(app.preview.is_none() && app.preview_prefetch.is_none());
    let rendered = app.frame_texture(&ctx, 0);
    assert_ne!(rendered.id(), initial.id());
    app.update_preview_texture(&ctx, 1);
    let (image, error) = PixelCraft::compose_frame_image(&app.doc, 1, &target);
    assert!(error.is_none());
    assert!((53..=57).contains(&image.pixels[0].r()));
    assert_eq!(app.doc, doc);
    assert!(app.undo.is_empty());
}

#[test]
fn polygonal_lasso_closes_click_vertices_without_changing_artwork() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-polygon-mask.png")));
    let doc = Document::new(7, 7);
    app.replace(doc.clone(), None);
    app.action("polygonal-lasso");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    for (x, y) in [(1., 1.), (5., 1.), (1., 5.)] {
        let p = board.min + Vec2::new(x + 0.5, y + 0.5) * 20.;
        discard_frame(&mut app, &ctx, size, 1., click(p));
        assert!(app.selection.is_none());
    }
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        click(board.min + Vec2::splat(30.)),
    );
    let mask = app.canvas_selection_mask();
    assert!(mask[8] && mask[16]);
    assert!(!mask[32] && !mask[0]);
    assert_eq!(app.doc, doc);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.selection.is_none());
}

#[test]
fn ellipse_marquee_uses_the_canvas_drag_and_preserves_corner_holes() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-ellipse-mask.png")));
    app.replace(Document::new(7, 7), None);
    app.action("elliptical-marquee");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    drag_layout(
        &mut app,
        &ctx,
        size,
        board.min + Vec2::splat(30.),
        board.min + Vec2::splat(110.),
    );
    let mask = app.canvas_selection_mask();
    assert!(mask[3 * 7 + 3]);
    assert!(!mask[8] && !mask[40]);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.selection.is_none());
}

#[test]
fn document_profile_dialog_converts_and_undoes_in_one_step() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-profile.png")));
    let mut doc = Document::new(2, 1);
    doc.layers[0].cels[0].set(0, [128, 128, 128, 91]);
    doc.color_profile = vec![1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let snapshot = doc.clone();
    app.replace(doc, None);
    app.action("color-profile");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut output = frame(&mut app, &ctx, size, 1., vec![]);
    let button = text_position(&mut output, "Convert profile");
    discard_frame(&mut app, &ctx, size, 1., click(button));
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.profile_dialog.is_some() {
        assert!(
            Instant::now() < deadline,
            "profile conversion did not complete"
        );
        std::thread::sleep(Duration::from_millis(1));
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.doc.color_profile.is_empty());
    assert!((186..=190).contains(&app.doc.layers[0].cels[0][0][0]));
    assert_eq!(app.doc.layers[0].cels[0][0][3], 91);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc, snapshot);
}

#[test]
fn color_wheel_applies_normal_map_colors_to_secondary_swatch() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-color-wheel.png")));
    let primary = app.fg;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let pick = |app: &mut PixelCraft, label| {
        let mut out = frame(app, &ctx, size, 1., vec![]);
        let pos = text_position(&mut out, label);
        discard_frame(app, &ctx, size, 1., click(pos));
    };
    pick(&mut app, "Color wheel");
    pick(&mut app, "RGB wheel");
    pick(&mut app, "Normal-map wheel");
    pick(&mut app, "Secondary swatch");
    let disk = layout_rect(&ctx, "color-wheel-disk");
    let old_alpha = app.bg[3];
    discard_frame(&mut app, &ctx, size, 1., click(disk.center()));
    assert_eq!(app.bg, [128, 128, 255, old_alpha]);
    assert_eq!(app.fg, primary);
    assert!(app.undo.is_empty());
}

#[test]
fn palette_ramp_controls_replace_span_and_undo_as_one_edit() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-palette-span.png")));
    app.replace(Document::new(4, 4), None);
    let red = [220, 30, 40, 255];
    let blue = [40, 80, 200, 255];
    app.doc.import_palette(vec![red, blue, CLEAR]).unwrap();
    app.fg = blue;
    app.palette_index = 1;
    app.action("workflow");
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let heading = text_position(&mut out, "Palette order & ramps");
    discard_frame(&mut app, &ctx, size, 1., click(heading));
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let button = text_position(&mut out, "Replace selected span");
    discard_frame(&mut app, &ctx, size, 1., click(button));
    assert_eq!(
        app.doc.palette,
        [
            red,
            [175, 43, 80, 255],
            [130, 55, 120, 255],
            [85, 68, 160, 255],
            blue,
            CLEAR
        ]
    );
    assert_eq!(app.palette_index, 4);
    assert_eq!(app.fg, blue);
    app.undo();
    assert_eq!(app.doc.palette, [red, blue, CLEAR]);
}

#[test]
fn gradient_controls_apply_patterns_to_preview_release_and_radial_strokes() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-gradient.png")));
    app.replace(Document::new(7, 7), None);
    app.tool = Tool::Gradient;
    app.fg = [20, 30, 41, 255];
    app.bg = [225, 180, 140, 255];
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let dither = text_position(&mut out, "Smooth");
    discard_frame(&mut app, &ctx, size, 1., click(dither));
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let bayer = text_position(&mut out, "Bayer 2×2");
    discard_frame(&mut app, &ctx, size, 1., click(bayer));

    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let pixel = |x, y| board.min + Vec2::new(x, y) * 20.;
    let from = pixel(1.5, 3.5);
    let to = pixel(5.5, 3.5);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(to)],
    );
    let artwork = &app.doc.layers[0].cels[0];
    assert_eq!([artwork[3], artwork[10]], [app.fg, app.bg]);
    let preview = artwork.clone();
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    assert!(app.doc.layers[0].cels[0].shares_storage(&preview));
    let end = pixel(3.5, 3.5);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(end), button(end, false)],
    );
    assert_eq!(app.doc.layers[0].cels[0][3], app.bg);
    app.undo();
    assert!(app.doc.layers[0].cels[0].iter().all(|p| *p == CLEAR));

    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let shape = text_position(&mut out, "Linear");
    discard_frame(&mut app, &ctx, size, 1., click(shape));
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let radial = text_position(&mut out, "Radial");
    discard_frame(&mut app, &ctx, size, 1., click(radial));
    drag_layout(&mut app, &ctx, size, pixel(3.5, 3.5), pixel(5.5, 3.5));
    let artwork = &app.doc.layers[0].cels[0];
    assert_eq!(
        [artwork[24], artwork[10], artwork[22], artwork[26]],
        [app.fg, app.bg, app.bg, app.bg]
    );

    app.undo();
    let duplicate = [72, 124, 189, 255];
    app.doc
        .import_palette(vec![CLEAR, duplicate, duplicate])
        .unwrap();
    app.doc.set_color_mode(ColorMode::Indexed);
    app.fg = duplicate;
    app.bg = duplicate;
    app.palette_index = 2;
    app.secondary_palette_index = Some(1);
    app.doc.dithering = Some(pixelcraft::contributions::DitheringMatrix {
        width: 1,
        height: 2,
        values: vec![1, 0],
    });
    let mut selection = vec![true; 49];
    selection[25] = false;
    app.set_selection_mask(selection);
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let dither = text_position(&mut out, "Bayer 2×2");
    discard_frame(&mut app, &ctx, size, 1., click(dither));
    let mut out = frame(&mut app, &ctx, size, 1., vec![]);
    let installed = text_position(&mut out, "Installed matrix");
    discard_frame(&mut app, &ctx, size, 1., click(installed));
    drag_layout(&mut app, &ctx, size, pixel(3.5, 3.5), pixel(5.5, 3.5));
    let indices = app.doc.layers[0].cel_meta[0].indices.as_ref().unwrap();
    assert_eq!(
        [indices[24], indices[25], indices[31], indices[23]],
        [2, 0, 2, 1]
    );
}

#[test]
fn canvas_transform_handles_edit_selected_cels_with_one_undo() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-transform.png")));
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    let mut doc = Document::new(12, 10);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.set_pixel(0, 0, 26, red);
    doc.set_pixel(0, 1, 39, green);
    doc.set_pixel(0, 2, 39, blue);
    app.replace(doc, None);
    app.selection = Some((2, 2, 3, 3));
    app.timeline_select(0, 0, false);
    app.timeline_select(0, 1, true);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let handle = layout_rect(&ctx, "canvas-transform-scale").center();
    drag_layout(&mut app, &ctx, size, handle, handle + Vec2::splat(40.));
    let deadline = Instant::now() + Duration::from_secs(5);
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][39], red);
    assert_eq!(app.doc.layers[0].cels[1][65], green);
    assert_eq!(app.doc.layers[0].cels[2][39], blue);
    assert_eq!(app.selection, Some((2, 2, 5, 5)));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc.layers[0].cels[0][39], CLEAR);
    assert_eq!(app.doc.layers[0].cels[1][39], green);
    assert_eq!(app.selection, Some((2, 2, 3, 3)));
}

#[test]
fn canvas_rotation_handle_rotates_selected_cels_and_mask_with_one_undo() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-rotation.png")));
    let mut doc = Document::new(12, 10);
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    doc.set_pixel(0, 0, 2 * 12 + 2, red);
    doc.set_pixel(0, 1, 4 * 12 + 3, green);
    doc.set_pixel(0, 2, 4 * 12 + 3, blue);
    app.replace(doc, None);
    let mut mask = vec![false; 120];
    for y in 2..6 {
        for x in 2..6 {
            mask[y * 12 + x] = (x, y) != (4, 3);
        }
    }
    app.set_selection_mask(mask.clone());
    app.timeline_select(0, 0, false);
    app.timeline_select(0, 1, true);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let handle = layout_rect(&ctx, "canvas-transform-rotate").center() + Vec2::new(3., 2.);
    let center =
        layout_rect(&ctx, "canvas-transform-scale").center() - 2. * app.canvas_pixel_size();
    let vector = handle - center;
    let end = center + Vec2::new(-vector.y, vector.x);
    drag_layout(&mut app, &ctx, size, handle, end);
    let deadline = Instant::now() + Duration::from_secs(5);
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][2 * 12 + 5], red);
    assert_eq!(app.doc.layers[0].cels[1][3 * 12 + 3], green);
    assert_eq!(app.doc.layers[0].cels[2][4 * 12 + 3], blue);
    assert!(!app.selected(4, 4));
    assert!(app.selected(4, 3));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc.layers[0].cels[0][2 * 12 + 2], red);
    assert_eq!(app.doc.layers[0].cels[1][4 * 12 + 3], green);
    assert_eq!(app.studio.mask.as_ref(), Some(&mask));
}

#[test]
fn canvas_combined_transform_resamples_original_once_and_applies_with_one_undo() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-combined.png")));
    let mut doc = Document::new(20, 20);
    let ink = [230, 110, 60, 255];
    doc.set_pixel(0, 0, 6 * 20 + 6, ink);
    app.replace(doc.clone(), None);
    app.selection = Some((4, 4, 7, 7));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let step = app.canvas_pixel_size();
    let scale = layout_rect(&ctx, "canvas-transform-scale").center();
    drag_layout(&mut app, &ctx, size, scale, scale + 4. * step);
    assert!(app.doc == doc);
    assert!(app.undo.is_empty());

    let pivot = layout_rect(&ctx, "canvas-transform-pivot").center();
    let handle = layout_rect(&ctx, "canvas-transform-rotate").center();
    let vector = (handle - pivot) / step;
    drag_layout(
        &mut app,
        &ctx,
        size,
        handle,
        pivot + Vec2::new(-vector.y, vector.x) * step,
    );
    assert!(app.doc == doc);
    assert!(app.undo.is_empty());

    let horizontal = layout_rect(&ctx, "canvas-transform-skew-x").center();
    // Local X after rotation points downward in the viewport.
    drag_layout(
        &mut app,
        &ctx,
        size,
        horizontal,
        horizontal + Vec2::new(0., 2. * step.y),
    );
    assert!(app.doc == doc);
    let apply = layout_rect(&ctx, "canvas-transform-apply").center();
    discard_frame(&mut app, &ctx, size, 1., click(apply));
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    let scale = pixelcraft::workflows::AffineTransform::about([2., 0.], [0., 2.], [4., 4.]);
    let rotation = pixelcraft::workflows::AffineTransform::rotation(90., [8., 8.]);
    let skew = pixelcraft::workflows::AffineTransform::about([1., 0.], [0.25, 1.], [4., 4.]);
    let matrix = skew.then(scale.then(rotation));
    let mut expected = doc.clone();
    pixelcraft::workflows::transform(
        &mut expected,
        0,
        0,
        [4, 4, 4, 4],
        None,
        pixelcraft::workflows::Transform::Affine(matrix),
    )
    .unwrap();
    assert!(app.doc.layers[0].cels[0] == expected.layers[0].cels[0]);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.doc == doc);
    assert_eq!(app.selection, Some((4, 4, 7, 7)));
    app.redo();
    assert!(app.doc.layers[0].cels[0] == expected.layers[0].cels[0]);
}

#[test]
fn cancelling_combined_canvas_transform_leaves_source_and_history_untouched() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-cancel-affine.png")));
    let mut doc = Document::new(20, 20);
    doc.set_pixel(0, 0, 6 * 20 + 6, [230, 110, 60, 255]);
    app.replace(doc.clone(), None);
    app.selection = Some((4, 4, 7, 7));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    for keyboard in [true, false] {
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        for _ in 0..2 {
            let handle = layout_rect(&ctx, "canvas-transform-scale").center();
            drag_layout(&mut app, &ctx, size, handle, handle + 2. * step);
        }
        if keyboard {
            discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Escape)]);
        } else {
            let cancel = layout_rect(&ctx, "canvas-transform-cancel").center();
            discard_frame(&mut app, &ctx, size, 1., click(cancel));
        }
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none());
        assert!(app.doc == doc);
        assert_eq!(app.selection, Some((4, 4, 7, 7)));
        assert!(app.undo.is_empty());
    }
}

#[test]
fn moving_then_rotating_floating_cels_keeps_one_apply_and_moves_the_pivot() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-floating-move.png")));
    let mut doc = Document::new(20, 20);
    let ink = [230, 110, 60, 255];
    doc.set_pixel(0, 0, 5 * 20 + 5, ink);
    app.replace(doc.clone(), None);
    app.selection = Some((4, 4, 7, 7));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let step = app.canvas_pixel_size();
    let pivot = layout_rect(&ctx, "canvas-transform-pivot").center();
    let from = layout_rect(&ctx, "canvas-transform-move").min + step;
    let displacement = Vec2::new(3., 2.) * step;
    drag_layout(&mut app, &ctx, size, from, from + displacement);
    assert!(app.doc == doc);
    assert!(app.undo.is_empty());
    let moved_pivot = layout_rect(&ctx, "canvas-transform-pivot").center();
    assert_eq!(moved_pivot, pivot + displacement);
    let rotate = layout_rect(&ctx, "canvas-transform-rotate").center();
    let direction = (rotate - moved_pivot) / step;
    let end = moved_pivot + Vec2::new(-direction.y, direction.x) * step;
    drag_layout(&mut app, &ctx, size, rotate, end);
    let from = layout_rect(&ctx, "canvas-transform-move").min + step;
    drag_layout(&mut app, &ctx, size, from, from + Vec2::new(2., 1.) * step);
    assert_eq!(
        layout_rect(&ctx, "canvas-transform-pivot").center(),
        moved_pivot + Vec2::new(2., 1.) * step
    );
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][8 * 20 + 11], ink);
    assert_eq!(app.doc.layers[0].cels[0][5 * 20 + 5], CLEAR);
    assert_eq!(app.selection, Some((9, 7, 12, 10)));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.doc == doc);
}

#[test]
fn every_canvas_scale_handle_keeps_its_opposite_edge_fixed() {
    for (label, sx, sy) in [
        ("canvas-transform-scale-nw", -1, -1),
        ("canvas-transform-scale-n", 0, -1),
        ("canvas-transform-scale-ne", 1, -1),
        ("canvas-transform-scale-w", -1, 0),
        ("canvas-transform-scale-e", 1, 0),
        ("canvas-transform-scale-sw", -1, 1),
        ("canvas-transform-scale-s", 0, 1),
        ("canvas-transform-scale", 1, 1),
    ] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-all-scale-handles.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        let handle = layout_rect(&ctx, label).center();
        let end = handle + Vec2::new(2. * sx as f32, 2. * sy as f32) * step;
        drag_layout(&mut app, &ctx, size, handle, end);
        assert!(app.doc == doc, "{label}");
        discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{label}: {}", app.status);
        let left = if sx < 0 { 6 } else { 8 };
        let right = if sx > 0 { 13 } else { 11 };
        let top = if sy < 0 { 6 } else { 8 };
        let bottom = if sy > 0 { 13 } else { 11 };
        assert_eq!(app.selection, Some((left, top, right, bottom)), "{label}");
        let pixel_x = (9 + sx) as usize;
        let pixel_y = (9 + sy) as usize;
        assert_eq!(
            app.doc.layers[0].cels[0][pixel_y * 24 + pixel_x],
            ink,
            "{label}"
        );
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn repeated_canvas_rotation_snaps_the_resulting_orientation() {
    for (initial, delta, expected) in [
        (30_f32, 14_f32, 45_f32),
        (-30., -14., -45.),
        (170., 14., 180.),
    ] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-absolute-snap.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        let pivot = layout_rect(&ctx, "canvas-transform-pivot").center();
        let first = layout_rect(&ctx, "canvas-transform-rotate").center();
        let rotate = |point: Pos2, degrees: f32| {
            let rotation = egui::emath::Rot2::from_angle(degrees.to_radians());
            pivot + rotation * ((point - pivot) / step) * step
        };
        drag_layout(&mut app, &ctx, size, first, rotate(first, initial));
        let handle = layout_rect(&ctx, "canvas-transform-rotate").center();
        let end = rotate(handle, delta);
        let modifiers = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        for mut events in [
            vec![egui::Event::PointerMoved(handle)],
            vec![egui::Event::PointerButton {
                pos: handle,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
            vec![egui::Event::PointerMoved(end)],
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        ] {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            discard_frame(&mut app, &ctx, size, 1., events);
        }
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let nw = layout_rect(&ctx, "canvas-transform-scale-nw").center();
        let ne = layout_rect(&ctx, "canvas-transform-scale-ne").center();
        let direction = (ne - nw) / step;
        let angle = direction.y.atan2(direction.x).to_degrees();
        let error = (angle - expected + 180.).rem_euclid(360.) - 180.;
        assert!(
            error.abs() < 0.001,
            "initial {initial}, drag {delta}, wanted {expected}, got {angle}"
        );
        assert!(app.doc == doc);
        assert!(app.undo.is_empty());
        discard_frame(
            &mut app,
            &ctx,
            size,
            1.,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                key_event(Key::Enter),
            ],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{}", app.status);
        let mut expected_doc = doc.clone();
        pixelcraft::workflows::transform(
            &mut expected_doc,
            0,
            0,
            [8, 8, 4, 4],
            None,
            pixelcraft::workflows::Transform::RotateAround {
                degrees: f64::from(expected),
                pivot: [10., 10.],
            },
        )
        .unwrap();
        let actual: Vec<_> = (0..576)
            .filter(|&i| app.doc.layers[0].cels[0][i] != CLEAR)
            .collect();
        let expected_pixels: Vec<_> = (0..576)
            .filter(|&i| expected_doc.layers[0].cels[0][i] != CLEAR)
            .collect();
        assert_eq!(actual, expected_pixels, "final pixels at {expected}");
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn every_canvas_skew_side_works_after_rotation() {
    for (label, horizontal) in [
        ("canvas-transform-skew-top", true),
        ("canvas-transform-skew-x", true),
        ("canvas-transform-skew-left", false),
        ("canvas-transform-skew-y", false),
    ] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-all-skew-handles.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        let pivot = layout_rect(&ctx, "canvas-transform-pivot").center();
        let rotate = layout_rect(&ctx, "canvas-transform-rotate").center();
        let vector = (rotate - pivot) / step;
        drag_layout(
            &mut app,
            &ctx,
            size,
            rotate,
            pivot + Vec2::new(-vector.y, vector.x) * step,
        );
        let handle = layout_rect(&ctx, label).center();
        let delta = if horizontal {
            Vec2::new(0., 2.)
        } else {
            Vec2::new(-2., 0.)
        } * step;
        drag_layout(&mut app, &ctx, size, handle, handle + delta);
        assert!(app.doc == doc);
        discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{label}: {}", app.status);
        let (bounds, pixel) = if horizontal {
            ((8, 8, 11, 13), 10 * 24 + 10)
        } else {
            ((6, 8, 11, 11), 9 * 24 + 9)
        };
        assert_eq!(app.selection, Some(bounds), "{label}");
        assert_eq!(app.doc.layers[0].cels[0][pixel], ink, "{label}");
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn canvas_scaling_can_cross_an_axis_and_lock_the_aspect_ratio() {
    for lock_aspect in [false, true] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-scale-modifiers.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        let handle = layout_rect(&ctx, "canvas-transform-scale").center();
        let distance = if lock_aspect { 8. } else { -8. };
        let end = handle + Vec2::new(distance * step.x, 0.);
        if lock_aspect {
            let modifiers = egui::Modifiers {
                shift: true,
                ..Default::default()
            };
            for events in [
                vec![egui::Event::PointerMoved(handle)],
                vec![egui::Event::PointerButton {
                    pos: handle,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                }],
                vec![egui::Event::PointerMoved(end)],
                vec![egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                }],
            ] {
                let mut raw = input(size, 1., events);
                raw.events
                    .insert(0, egui::Event::ModifiersChanged(modifiers));
                raw.time = Some(ctx.cumulative_frame_nr() as f64 * 0.1);
                let mut rendered = ctx.run_ui(raw, |ui| app.render(ui));
                rendered.textures_delta.clear();
            }
        } else {
            drag_layout(&mut app, &ctx, size, handle, end);
        }
        assert!(app.doc == doc);
        discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{}", app.status);
        if lock_aspect {
            assert_eq!(app.selection, Some((8, 8, 15, 15)));
            for y in 10..12 {
                for x in 10..12 {
                    assert_eq!(app.doc.layers[0].cels[0][y * 24 + x], ink);
                }
            }
        } else {
            assert_eq!(app.selection, Some((4, 8, 7, 11)));
            assert_eq!(app.doc.layers[0].cels[0][9 * 24 + 6], ink);
        }
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn floating_cel_movement_can_lock_an_axis_and_toggle_grid_snapping() {
    for (snap, modifiers, movement, displacement) in [
        (false, egui::Modifiers::SHIFT, [3., 2.], [3, 0]),
        (false, egui::Modifiers::SHIFT, [2., 3.], [0, 3]),
        (true, egui::Modifiers::NONE, [3., 2.], [5, 2]),
        (false, egui::Modifiers::ALT, [3., 2.], [5, 2]),
        (true, egui::Modifiers::ALT, [3., 2.], [3, 2]),
    ] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-move-modifiers.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        doc.drawing_grid = Some(DrawingGrid {
            origin: [1, 2],
            size: [4, 4],
        });
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        app.snap_to_grid = snap;
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        let from = layout_rect(&ctx, "canvas-transform-move").min + step;
        let end = from + Vec2::new(movement[0], movement[1]) * step;
        for mut events in [
            vec![egui::Event::PointerMoved(from)],
            vec![egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
            vec![egui::Event::PointerMoved(end)],
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        ] {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            discard_frame(&mut app, &ctx, size, 1., events);
        }
        assert!(app.doc == doc);
        discard_frame(
            &mut app,
            &ctx,
            size,
            1.,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                key_event(Key::Enter),
            ],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{}", app.status);
        let [dx, dy] = displacement;
        assert_eq!(
            app.selection,
            Some((8 + dx, 8 + dy, 11 + dx, 11 + dy)),
            "snap {snap}, modifiers {modifiers:?}"
        );
        assert_eq!(
            app.doc.layers[0].cels[0][(9 + dy) as usize * 24 + (9 + dx) as usize],
            ink
        );
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn canvas_scaling_around_a_custom_pivot_keeps_that_point_fixed() {
    for (lock_aspect, rotated) in [(false, false), (true, false), (false, true), (true, true)] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app =
            PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-scale-from-pivot.png")));
        let mut doc = Document::new(24, 24);
        let ink = [230, 110, 60, 255];
        doc.set_pixel(0, 0, 9 * 24 + 9, ink);
        app.replace(doc.clone(), None);
        app.selection = Some((8, 8, 11, 11));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        app.action("canvas-transform");
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let step = app.canvas_pixel_size();
        if rotated {
            let center = layout_rect(&ctx, "canvas-transform-pivot").center();
            let handle = layout_rect(&ctx, "canvas-transform-rotate").center();
            let vector = (handle - center) / step;
            drag_layout(
                &mut app,
                &ctx,
                size,
                handle,
                center + Vec2::new(-vector.y, vector.x) * step,
            );
        }
        let center = layout_rect(&ctx, "canvas-transform-pivot").center();
        let pivot = center - 4. * step;
        drag_layout(&mut app, &ctx, size, center, pivot);
        let handle = layout_rect(&ctx, "canvas-transform-scale").center();
        let end = handle + (handle - pivot);
        let modifiers = egui::Modifiers {
            alt: true,
            shift: lock_aspect,
            ..Default::default()
        };
        for mut events in [
            vec![egui::Event::PointerMoved(handle)],
            vec![egui::Event::PointerButton {
                pos: handle,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
            vec![egui::Event::PointerMoved(end)],
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        ] {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            discard_frame(&mut app, &ctx, size, 1., events);
        }
        assert_eq!(layout_rect(&ctx, "canvas-transform-pivot").center(), pivot);
        assert!(app.doc == doc);
        discard_frame(
            &mut app,
            &ctx,
            size,
            1.,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                key_event(Key::Enter),
            ],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.canvas_transform.is_some() && Instant::now() < deadline {
            std::thread::yield_now();
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        assert!(app.canvas_transform.is_none(), "{}", app.status);
        assert_eq!(app.selection, Some((10, 10, 17, 17)));
        for y in 12..14 {
            let left = if rotated { 14 } else { 12 };
            for x in left..left + 2 {
                assert_eq!(app.doc.layers[0].cels[0][y * 24 + x], ink);
            }
        }
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(app.doc == doc);
    }
}

#[test]
fn moving_canvas_rotation_pivot_does_not_edit_artwork() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-pivot.png")));
    let mut doc = Document::new(16, 16);
    let red = [255, 0, 0, 255];
    doc.set_pixel(0, 0, 6 * 16 + 6, red);
    app.replace(doc.clone(), None);
    app.selection = Some((4, 4, 7, 7));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);

    let center = layout_rect(&ctx, "canvas-transform-pivot").center();
    let step = app.canvas_pixel_size();
    // Move the pivot from (6, 6) to (3, 6), beyond the selection.
    let pivot = center - Vec2::new(3. * step.x, 0.);
    let grab = Vec2::new(2., 1.);
    drag_layout(&mut app, &ctx, size, center + grab, pivot + grab);
    assert_eq!(layout_rect(&ctx, "canvas-transform-pivot").center(), pivot);
    assert!(app.doc == doc);
    assert_eq!(app.selection, Some((4, 4, 7, 7)));
    assert!(app.undo.is_empty());
    assert!(app.canvas_transform_texture().is_none());

    discard_frame(&mut app, &ctx, size, 1., click(pivot));
    discard_frame(&mut app, &ctx, size, 1., click(pivot));
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    assert_eq!(layout_rect(&ctx, "canvas-transform-pivot").center(), center);
    assert!(app.undo.is_empty());
    drag_layout(&mut app, &ctx, size, center, pivot);

    let handle = layout_rect(&ctx, "canvas-transform-rotate").center();
    let vector = (handle - pivot) / step;
    let end = pivot + Vec2::new(-vector.y, vector.x) * step;
    drag_layout(&mut app, &ctx, size, handle, end);
    let deadline = Instant::now() + Duration::from_secs(5);
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][9 * 16 + 2], red);
    assert_eq!(app.doc.layers[0].cels[0][6 * 16 + 6], CLEAR);
    assert_eq!(app.selection, Some((1, 7, 4, 10)));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.doc == doc);
}

#[test]
fn canvas_rotsprite_rotation_preserves_selected_links_and_detaches_partial_links() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-rotsprite.png")));
    let mut original = Document::new(16, 16);
    original.add_layer();
    original.add_frame(0, false);
    original.add_frame(1, false);
    let color = [240, 100, 60, 255];
    for layer in 0..2 {
        for &(x, y) in &[(5, 7), (6, 7), (7, 6), (7, 7), (7, 8)] {
            original.set_pixel(layer, 0, y * 16 + x, color);
        }
        original.link_cel(layer, 1, 0).unwrap();
    }
    original.link_cel(1, 2, 0).unwrap();
    app.replace(original.clone(), None);
    app.selected_cels = Some(CelSelection::Rectangle(CelRange {
        first_layer: 0,
        last_layer: 1,
        first_frame: 0,
        last_frame: 1,
    }));
    app.selection = Some((4, 4, 11, 11));
    app.studio.rotation_rotsprite = true;
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let handle = layout_rect(&ctx, "canvas-transform-rotate").center();
    let pivot = layout_rect(&ctx, "canvas-transform-scale").center() - 4. * app.canvas_pixel_size();
    drag_layout(&mut app, &ctx, size, handle, pivot + Vec2::new(60., -60.));
    let deadline = Instant::now() + Duration::from_secs(5);
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    let mut expected = original.clone();
    let (_, coverage) = pixelcraft::workflows::transform_rotsprite(
        &mut expected,
        0,
        0,
        [4, 4, 8, 8],
        None,
        45.,
        [8., 8.],
    )
    .unwrap();
    for layer in 0..2 {
        for frame in 0..2 {
            let differences: Vec<_> = (0..256)
                .filter(|&i| app.doc.layers[layer].cels[frame][i] != expected.layers[0].cels[0][i])
                .collect();
            assert!(
                differences.is_empty(),
                "layer {layer}, frame {frame}, differences {differences:?}"
            );
        }
    }
    assert_eq!(app.studio.mask.as_ref(), Some(&coverage));
    assert!(app.doc.layers[0].cel_meta[0].artwork_link.is_some());
    assert_eq!(
        app.doc.layers[0].cel_meta[0].artwork_link,
        app.doc.layers[0].cel_meta[1].artwork_link
    );
    assert!(app.doc.layers[1].cels[2] == original.layers[1].cels[2]);
    assert_eq!(app.doc.layers[1].cel_meta[0].artwork_link, None);
    assert_eq!(app.doc.layers[1].cel_meta[1].artwork_link, None);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(app.doc == original);
}

#[test]
fn canvas_skew_preview_leaves_linked_cels_untouched_until_release() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-skew.png")));
    let mut doc = Document::new(12, 10);
    let red = [255, 0, 0, 255];
    doc.add_frame(0, false);
    doc.add_frame(1, false);
    doc.set_pixel(0, 0, 26, red);
    doc.set_pixel(0, 0, 50, red);
    doc.link_cel(0, 1, 0).unwrap();
    doc.link_cel(0, 2, 0).unwrap();
    app.replace(doc, None);
    app.selected_cels = Some(CelSelection::Rectangle(CelRange {
        first_layer: 0,
        last_layer: 0,
        first_frame: 0,
        last_frame: 1,
    }));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    app.action("canvas-transform");
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let anchor = layout_rect(&ctx, "canvas-transform-skew-x").center();
    let end = anchor + Vec2::new(60., 0.);
    let pointer_button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(anchor)],
    );
    discard_frame(&mut app, &ctx, size, 1., vec![pointer_button(anchor, true)]);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(end)],
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.canvas_transform_texture().is_none() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform_texture().is_some(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][50], red);
    assert_eq!(app.doc.layers[0].cels[0][52], CLEAR);
    assert!(app.undo.is_empty());
    discard_frame(&mut app, &ctx, size, 1., vec![pointer_button(end, false)]);
    let deadline = Instant::now() + Duration::from_secs(5);
    discard_frame(&mut app, &ctx, size, 1., vec![key_event(Key::Enter)]);
    while app.canvas_transform.is_some() && Instant::now() < deadline {
        std::thread::yield_now();
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    assert!(app.canvas_transform.is_none(), "{}", app.status);
    assert_eq!(app.doc.layers[0].cels[0][52], red);
    assert_eq!(app.doc.layers[0].cels[1][52], red);
    assert_eq!(app.doc.layers[0].cels[2][50], red);
    assert_eq!(app.doc.layers[0].cels[2][52], CLEAR);
    app.undo();
    let layer = &app.doc.layers[0];
    assert!(layer.cel_meta[0].artwork_link.is_some());
    assert_eq!(
        layer.cel_meta[0].artwork_link,
        layer.cel_meta[2].artwork_link
    );
}

#[test]
fn polygon_and_contour_draw_covered_pixels_in_one_undoable_edit() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-polygon.png")));
    let doc = Document::new(8, 8);
    app.replace(doc.clone(), None);
    app.action("polygon");
    app.filled = false;
    app.fg = [200, 40, 30, 255];
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let center = |x, y| board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * 20.;
    for (x, y) in [(1, 1), (6, 1), (6, 6), (1, 6)] {
        discard_frame(&mut app, &ctx, size, 1., click(center(x, y)));
        assert_eq!(app.doc, doc);
    }
    discard_frame(&mut app, &ctx, size, 1., click(center(1, 1)));
    assert_eq!(app.doc.layers[0].cels[0][11], app.fg);
    assert_eq!(app.doc.layers[0].cels[0][51], app.fg);
    assert_eq!(app.doc.layers[0].cels[0][27], CLEAR);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc, doc);

    app.action("contour");
    app.selection = Some((2, 2, 4, 4));
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let start = center(1, 1);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(start)],
    );
    discard_frame(&mut app, &ctx, size, 1., vec![press(start, true)]);
    for (x, y) in [(6, 1), (6, 6), (1, 6)] {
        discard_frame(
            &mut app,
            &ctx,
            size,
            1.,
            vec![egui::Event::PointerMoved(center(x, y))],
        );
        assert_eq!(app.doc, doc);
    }
    discard_frame(&mut app, &ctx, size, 1., vec![press(center(1, 6), false)]);
    let painted: Vec<_> = app.doc.layers[0].cels[0]
        .iter()
        .enumerate()
        .filter_map(|(index, color)| (*color == app.fg).then_some(index))
        .collect();
    assert_eq!(painted, [18, 19, 20, 26, 27, 28, 34, 35, 36]);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc, doc);
}

#[test]
fn cubic_curve_clicks_show_a_draft_then_commit_a_masked_stroke() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-curve.png")));
    let doc = Document::new(13, 9);
    app.replace(doc, None);
    app.action("curve");
    app.fg = [212, 50, 40, 255];
    app.selection = Some((2, 0, 12, 8));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let center = |(x, y): (i32, i32)| board.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * 20.;
    for point in [(1, 7), (11, 7), (1, 1)] {
        discard_frame(&mut app, &ctx, size, 1., click(center(point)));
        assert!(
            !app.doc.layers[0].cels[0].occupied(),
            "The draft wrote into the cel"
        );
        assert!(app.undo.is_empty());
    }
    let mut preview = frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(center((11, 1)))],
    );
    preview.textures_delta.clear();
    assert!(preview.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Path(path)
            if path.points.contains(&center((6, 3)))
                && path.points.first() == Some(&center((1, 7)))
                && path.points.last() == Some(&center((11, 7))))
    }));
    assert!(app.undo.is_empty());
    discard_frame(&mut app, &ctx, size, 1., click(center((11, 1))));
    let pixels = &app.doc.layers[0].cels[0];
    assert_eq!(pixels[7 * 13 + 1], CLEAR);
    assert_eq!(pixels[7 * 13 + 11], app.fg);
    assert_eq!(pixels[3 * 13 + 6], app.fg);
    assert_eq!(pixels[7 * 13 + 6], CLEAR);
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(!app.doc.layers[0].cels[0].occupied());
}

#[test]
fn spray_tool_keeps_emitting_at_rest_with_configured_rate_and_radius() {
    fn stroke(rate: u16) -> Vec<usize> {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-spray.png")));
        let doc = Document::new(64, 64);
        app.replace(doc, None);
        app.action("spray");
        app.fg = [240, 80, 55, 255];
        app.spray_rate = rate;
        app.spray_radius = 12;
        app.selection = Some((32, 0, 63, 63));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 4.;
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let board = Rect::from_center_size(
            layout_rect(&ctx, "canvas-bounds").center() + app.pan,
            app.sprite_display_size() * app.zoom,
        );
        let pos = board.min + Vec2::splat(32.5 * app.zoom);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let pixels = |app: &PixelCraft| {
            app.doc.layers[0].cels[0]
                .iter()
                .enumerate()
                .filter_map(|(i, color)| (*color != CLEAR).then_some(i))
                .collect::<Vec<_>>()
        };
        discard_frame(
            &mut app,
            &ctx,
            size,
            1.,
            vec![egui::Event::PointerMoved(pos)],
        );
        discard_frame(&mut app, &ctx, size, 1., vec![button(true)]);
        let initial = pixels(&app).len();
        for _ in 0..10 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        discard_frame(&mut app, &ctx, size, 1., vec![button(false)]);
        let finished = pixels(&app);
        assert!(finished.len() > initial);
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        assert_eq!(pixels(&app), finished);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(pixels(&app).is_empty());
        finished
    }

    let slow = stroke(20);
    let fast = stroke(180);
    assert!(fast.len() * 2 > slow.len() * 5);
    assert!(fast.into_iter().all(|i| {
        let x = (i % 64) as i32 - 32;
        let y = (i / 64) as i32 - 32;
        x >= 0 && x * x + y * y <= 12 * 12
    }));
}

#[test]
fn blur_and_jumble_brushes_clip_to_selection_and_commit_one_stroke() {
    for action in ["blur-brush", "jumble-brush"] {
        let ctx = egui::Context::default();
        let size = Vec2::new(1180., 840.);
        let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-retouch.png")));
        let mut doc = Document::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                let color = if (x + y) % 2 == 0 {
                    [250, 20, 40, 255]
                } else {
                    [20, 40, 250, 255]
                };
                doc.set_pixel(0, 0, y * 10 + x, color);
            }
        }
        app.replace(doc.clone(), None);
        app.action(action);
        app.brush = 3;
        app.selection = Some((2, 3, 7, 5));
        for _ in 0..3 {
            discard_frame(&mut app, &ctx, size, 1., vec![]);
        }
        app.zoom = 20.;
        discard_frame(&mut app, &ctx, size, 1., vec![]);
        let board = Rect::from_center_size(
            layout_rect(&ctx, "canvas-bounds").center() + app.pan,
            app.sprite_display_size() * app.zoom,
        );
        let center = |x, y| board.min + Vec2::new(x, y) * 20.;
        drag_layout(&mut app, &ctx, size, center(1.5, 4.5), center(8.5, 4.5));
        let original = &doc.layers[0].cels[0];
        let edited = &app.doc.layers[0].cels[0];
        let changes: Vec<_> = edited
            .iter()
            .enumerate()
            .filter_map(|(i, color)| (*color != original[i]).then_some(i))
            .collect();
        assert!(!changes.is_empty(), "{action} had no effect");
        assert!(
            changes
                .iter()
                .all(|i| (2..=7).contains(&(i % 10)) && (3..=5).contains(&(i / 10)))
        );
        if action == "blur-brush" {
            assert_eq!(edited[44], [147, 28, 133, 255]);
        } else {
            assert!(changes.iter().all(|&i| edited[i] == original[i - 1]));
        }
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(app.doc, doc);
    }
}

#[test]
fn sprite_text_previews_and_places_multiline_pixels_as_one_undoable_edit() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-sprite-text.png")));
    let doc = Document::new(48, 56);
    app.replace(doc.clone(), None);
    app.action("text");
    app.fg = [235, 186, 94, 255];
    app.selection = Some((9, 5, 40, 50));
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 5.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        click(board.min + Vec2::new(4.5, 5.5) * 5.),
    );
    assert!(app.dialog_open());
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::Text("Oak\nLake".into())],
    );
    assert!(app.text_preview_texture().is_some());
    assert!(app.undo.is_empty());
    assert!(!app.doc.layers[0].cels[0].occupied());

    let apply = layout_rect(&ctx, "sprite-text-apply").center();
    discard_frame(&mut app, &ctx, size, 1., click(apply));
    let pixels = &app.doc.layers[0].cels[0];
    let touched: Vec<_> = pixels
        .iter()
        .enumerate()
        .filter_map(|(i, p)| (p[3] != 0).then_some(i))
        .collect();
    assert!(!touched.is_empty());
    assert!(touched.iter().all(|i| (9..=40).contains(&(i % 48))));
    assert!(touched.iter().any(|i| i / 48 < 22));
    assert!(touched.iter().any(|i| i / 48 > 25));
    assert!(touched.iter().all(|&i| pixels[i] == app.fg));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.doc, doc);
}

#[test]
fn per_tool_inks_preserve_coverage_and_copy_translucent_color() {
    use pixelcraft::productivity::PaintInk;
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-tool-ink.png")));
    let mut doc = Document::new(8, 8);
    doc.set_pixel(0, 0, 18, [0, 0, 255, 128]);
    doc.set_pixel(0, 0, 19, [0, 0, 255, 255]);
    doc.set_pixel(0, 0, 20, [0, 0, 255, 128]);
    doc.set_pixel(0, 0, 27, [0, 0, 255, 72]);
    app.replace(doc, None);
    app.fg = [255, 0, 0, 192];
    app.action("pencil");
    app.set_tool_ink(PaintInk::LockAlpha);
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let point = |x: f32, y: f32| board.min + Vec2::new(x + 0.5, y + 0.5) * 20.;
    drag_layout(&mut app, &ctx, size, point(1., 2.), point(3., 2.));
    let cel = &app.doc.layers[0].cels[0];
    assert_eq!(cel[17], CLEAR);
    assert_eq!(cel[18][3], 128);
    assert!(cel[18][0] > 0 && cel[18][2] > 0);
    assert_eq!(cel[19][3], 255);

    app.action("line");
    assert_eq!(app.tool_ink(), PaintInk::Default);
    app.set_tool_ink(PaintInk::CopyColor);
    drag_layout(&mut app, &ctx, size, point(4., 2.), point(6., 2.));
    assert_eq!(app.doc.layers[0].cels[0][20], app.fg);
    assert_eq!(app.doc.layers[0].cels[0][22], app.fg);
    app.set_tool_ink(PaintInk::LockAlpha);
    drag_layout(&mut app, &ctx, size, point(2., 3.), point(4., 3.));
    let cel = &app.doc.layers[0].cels[0];
    assert_eq!(cel[26], CLEAR);
    assert_eq!(cel[27][3], 72);
    assert!(cel[27][0] > 0);
    assert_eq!(cel[28], CLEAR);
    app.action("pencil");
    assert_eq!(app.tool_ink(), PaintInk::LockAlpha);
    assert_eq!(app.undo.len(), 3);
}

#[test]
fn stabilized_pencil_lags_while_dragging_and_finishes_at_release() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-stabilizer.png")));
    app.replace(Document::new(20, 10), None);
    app.set_brush_dynamics(BrushDynamics {
        stabilizer: 3,
        ..Default::default()
    });
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let center = |x| board.min + Vec2::new((x as f32 + 0.5) * 20., 90.);
    let button = |x, pressed| egui::Event::PointerButton {
        pos: center(x),
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: Default::default(),
    };
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(center(2))],
    );
    discard_frame(&mut app, &ctx, size, 1., vec![button(2, true)]);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(center(10))],
    );
    let row = &app.doc.layers[0].cels[0];
    assert_ne!(row[87], CLEAR);
    assert_eq!(row[88], CLEAR);
    assert_eq!(row[90], CLEAR);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![button(11, false), egui::Event::PointerGone],
    );
    assert!((2..=11).all(|x| app.doc.layers[0].cels[0][80 + x] == app.fg));
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert!(!app.doc.layers[0].cels[0].occupied());
}

#[test]
fn tablet_pressure_controls_size_and_opacity_without_building_on_overlap() {
    let ctx = egui::Context::default();
    let size = Vec2::new(1180., 840.);
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-pressure.png")));
    app.replace(Document::new(18, 12), None);
    app.brush = 5;
    app.set_brush_dynamics(BrushDynamics {
        pressure_size: true,
        pressure_opacity: true,
        ..Default::default()
    });
    for _ in 0..3 {
        discard_frame(&mut app, &ctx, size, 1., vec![]);
    }
    app.zoom = 20.;
    discard_frame(&mut app, &ctx, size, 1., vec![]);
    let board = Rect::from_center_size(
        layout_rect(&ctx, "canvas-bounds").center() + app.pan,
        app.sprite_display_size() * app.zoom,
    );
    let center = |x| board.min + Vec2::new((x as f32 + 0.5) * 20., 110.);
    let button = |x, pressed| egui::Event::PointerButton {
        pos: center(x),
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: Default::default(),
    };
    let touch = |x, phase, force| egui::Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(0),
        phase,
        pos: center(x),
        force: Some(force),
    };
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![egui::Event::PointerMoved(center(2))],
    );
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![button(2, true), touch(2, egui::TouchPhase::Start, 0.2)],
    );
    assert_eq!(app.doc.layers[0].cels[0][92][3], 51);
    assert_eq!(app.doc.layers[0].cels[0][74], CLEAR);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![
            egui::Event::PointerMoved(center(10)),
            touch(10, egui::TouchPhase::Move, 0.8),
        ],
    );
    assert_eq!(app.doc.layers[0].cels[0][100][3], 204);
    assert_eq!(app.doc.layers[0].cels[0][82][3], 204);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![touch(10, egui::TouchPhase::Move, 0.4)],
    );
    assert_eq!(app.doc.layers[0].cels[0][100][3], 204);
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![button(10, false), touch(10, egui::TouchPhase::End, 0.)],
    );
    assert_eq!(app.doc.layers[0].cels[0][100][3], 204);
    assert_eq!(app.undo.len(), 1);
    app.action("eraser");
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![button(10, true), touch(10, egui::TouchPhase::Start, 0.5)],
    );
    discard_frame(
        &mut app,
        &ctx,
        size,
        1.,
        vec![button(10, false), touch(10, egui::TouchPhase::End, 0.)],
    );
    assert_eq!(app.doc.layers[0].cels[0][100][3], 102);
    app.action("pencil");
    discard_frame(&mut app, &ctx, size, 1., click(center(15)));
    assert_eq!(app.doc.layers[0].cels[0][105], app.fg);
}
