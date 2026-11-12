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
                if rect.fill == Color32::from_rgb(220, 30, 70)
                    && rect.rect.size() == Vec2::splat(27.) =>
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
    let document = formats::load(&path).unwrap();
    app.open_document(document, Some(path));
    assert_eq!(app.tabs.len(), 2);
    for _ in 0..3 {
        let mut out = frame(&mut app, &ctx, Vec2::new(1920., 1080.), 1., vec![]);
        let shape_count = out.shapes.len();
        out.textures_delta.clear();
        assert!(shape_count < 20_000, "Editor built {shape_count} shapes");
    }
    println!(
        "Opened {}x{} project with {} colors across {} tabs",
        app.doc.width,
        app.doc.height,
        app.used_colors.len(),
        app.tabs.len()
    );
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
        ("dark", 1.5, "editor", 1180.),
        ("light", 1.5, "editor", 1180.),
        ("dark", 2., "editor", 1180.),
        ("light", 2., "editor", 1180.),
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
        let size = Vec2::new(width, if width > 1800. { 1080. } else { 840. });
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
                    "edit" => 55.,
                    "frame" => 190.,
                    _ => -100.,
                };
                if x > 0. {
                    click(Pos2::new(x, 15.))
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
