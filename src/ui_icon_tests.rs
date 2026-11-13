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
    for (action, title) in [("gif", "Export animated GIF"), ("mp4", "Export MP4 video")] {
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
    assert_eq!(doc.stored_pixels(), 62_914_560);
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
