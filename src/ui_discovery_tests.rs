//! Render real widget trees at physical desktop sizes and synthetic native DPI.
use super::*;

fn collect_textures(
    output: &egui::FullOutput,
    textures: &mut BTreeMap<u64, egui::ColorImage>,
    nearest: &mut std::collections::BTreeSet<u64>,
) {
    for (id, deltas) in &output.textures_delta.set {
        let egui::TextureId::Managed(id) = id else {
            continue;
        };
        for delta in deltas {
            if delta.options.magnification == egui::TextureFilter::Nearest {
                nearest.insert(*id);
            } else {
                nearest.remove(id);
            }
            let egui::ImageData::Color(image) = &delta.image;
            if let Some([x, y]) = delta.pos {
                let target = textures.get_mut(id).unwrap();
                for row in 0..image.size[1] {
                    let destination = (y + row) * target.size[0] + x;
                    let source = row * image.size[0];
                    target.pixels[destination..destination + image.size[0]]
                        .copy_from_slice(&image.pixels[source..source + image.size[0]]);
                }
            } else {
                textures.insert(*id, (**image).clone());
            }
        }
    }
}

#[test]
fn discovery_dialogs_follow_desktop_size_and_dpi() {
    let directory = std::env::var_os("PIXELCRAFT_DISCOVERY_REVIEW_DIR").map(PathBuf::from);
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory).unwrap();
    }
    for physical in [Vec2::new(1366., 768.), Vec2::new(1920., 1080.)] {
        for dpi in [1., 1.25, 1.5, 2.] {
            for dark in [false, true] {
                let ctx = egui::Context::default();
                let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused.png")));
                app.set_theme_preference(
                    &ctx,
                    if dark {
                        egui::ThemePreference::Dark
                    } else {
                        egui::ThemePreference::Light
                    },
                );
                app.open_settings_destination("settings-recovery");
                let logical = physical / dpi;
                let mut textures = BTreeMap::new();
                let mut nearest = std::collections::BTreeSet::new();
                for step in 0..6 {
                    let mut output = frame(&mut app, &ctx, logical, dpi, vec![]);
                    collect_textures(&output, &mut textures, &mut nearest);
                    if step == 5 {
                        let bounds = Rect::from_min_size(Pos2::ZERO, logical);
                        for id in [
                            egui::Id::new("settings-done"),
                            egui::Id::new("settings-scale-recovery"),
                            egui::Id::new(("dialog-close", "Settings")),
                        ] {
                            let rect = ctx.data(|data| data.get_temp::<Rect>(id)).unwrap();
                            assert!(
                                bounds.contains_rect(rect),
                                "{id:?} clipped at {physical:?}/{dpi}: {rect:?}"
                            );
                        }
                        assert!(app.power.bridge.is_none());
                        assert_eq!(output.pixels_per_point, dpi);
                        if let Some(directory) = &directory {
                            let meshes = ctx.tessellate(std::mem::take(&mut output.shapes), dpi);
                            let pixels = rasterize(&meshes, &textures, &nearest, logical, dpi);
                            image::save_buffer(
                                directory.join(format!(
                                    "settings-{}x{}-{dpi}-{}.png",
                                    physical.x,
                                    physical.y,
                                    if dark { "dark" } else { "light" }
                                )),
                                &pixels,
                                physical.x as u32,
                                physical.y as u32,
                                image::ColorType::Rgba8,
                            )
                            .unwrap();
                        }
                    }
                    output.textures_delta.clear();
                }
            }
        }
    }
}

#[test]
fn focused_destinations_render_at_full_hd_and_small_logical_viewports() {
    let directory = std::env::var_os("PIXELCRAFT_DISCOVERY_REVIEW_DIR").map(PathBuf::from);
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory).unwrap();
    }
    for (action, title) in [
        ("settings-appearance", "Settings"),
        ("settings-animation", "Settings"),
        ("settings-color", "Settings"),
        ("palette-tools", "Palette tools"),
        ("brushes", "Brushes and selection"),
        ("references", "Reference images"),
        ("import-grid", "Import sprite sheet"),
        ("batch-tags", "Export animation tags"),
        ("atlas", "Export multi-file atlas"),
        ("assistants", "Assistant access"),
        ("layer-properties", "Layer properties"),
        ("frame-properties", "Frame and animation properties"),
    ] {
        for (physical, dpi) in [(Vec2::new(1920., 1080.), 1.), (Vec2::new(1366., 768.), 2.)] {
            for dark in [false, true] {
                let ctx = egui::Context::default();
                let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("review.png")));
                app.set_theme_preference(
                    &ctx,
                    if dark {
                        egui::ThemePreference::Dark
                    } else {
                        egui::ThemePreference::Light
                    },
                );
                app.action(action);
                let logical = physical / dpi;
                let mut textures = BTreeMap::new();
                let mut nearest = std::collections::BTreeSet::new();
                for step in 0..6 {
                    let mut output = frame(&mut app, &ctx, logical, dpi, vec![]);
                    collect_textures(&output, &mut textures, &mut nearest);
                    if step == 5 {
                        let close = ctx
                            .data(|data| {
                                data.get_temp::<Rect>(egui::Id::new(("dialog-close", title)))
                            })
                            .unwrap_or_else(|| panic!("{action} did not open {title}"));
                        assert!(
                            Rect::from_min_size(Pos2::ZERO, logical).contains_rect(close),
                            "{action} close clipped at {logical:?}: {close:?}"
                        );
                        assert!(app.power.bridge.is_none());
                        if let Some(directory) = &directory {
                            let meshes = ctx.tessellate(std::mem::take(&mut output.shapes), dpi);
                            let pixels = rasterize(&meshes, &textures, &nearest, logical, dpi);
                            image::save_buffer(
                                directory.join(format!(
                                    "{action}-{}x{}-{dpi}-{}.png",
                                    physical.x,
                                    physical.y,
                                    if dark { "dark" } else { "light" }
                                )),
                                &pixels,
                                physical.x as u32,
                                physical.y as u32,
                                image::ColorType::Rgba8,
                            )
                            .unwrap();
                        }
                    }
                    output.textures_delta.clear();
                }
            }
        }
    }
}

#[test]
fn resizing_and_monitor_dpi_transitions_keep_settings_and_scale_recovery_usable() {
    let ctx = egui::Context::default();
    let mut app = PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-dpi-transition.png")));
    app.open_settings_destination("settings-recovery");
    let document = app.doc.clone();
    for (physical, dpi) in [
        (Vec2::new(1920., 1080.), 1.),
        (Vec2::new(1366., 768.), 1.25),
        (Vec2::new(1920., 1080.), 1.5),
        (Vec2::new(1366., 768.), 2.),
        (Vec2::new(1366., 768.), 1.),
    ] {
        let logical = physical / dpi;
        for _ in 0..4 {
            let mut output = frame(&mut app, &ctx, logical, dpi, vec![]);
            output.textures_delta.clear();
        }
        for id in ["settings-done", "settings-scale-recovery"] {
            let rect = ctx
                .data(|data| data.get_temp::<Rect>(egui::Id::new(id)))
                .unwrap();
            assert!(
                ctx.content_rect().contains_rect(rect),
                "{id} stranded after resize/DPI {physical:?}/{dpi}: {rect:?}"
            );
        }
    }
    ctx.set_zoom_factor(3.);
    let logical = Vec2::new(683., 384.);
    for _ in 0..3 {
        let mut output = frame(&mut app, &ctx, logical, 2., vec![]);
        output.textures_delta.clear();
    }
    let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    let mut output = frame(
        &mut app,
        &ctx,
        logical,
        2.,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key: Key::Num0,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
    );
    output.textures_delta.clear();
    for _ in 0..3 {
        let mut output = frame(&mut app, &ctx, logical, 2., vec![]);
        output.textures_delta.clear();
    }
    assert_eq!(ctx.zoom_factor(), 1.);
    let done = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new("settings-done")))
        .unwrap();
    assert!(ctx.content_rect().contains_rect(done));
    assert_eq!(app.doc, document);
    assert!(app.undo.is_empty());
    assert!(app.power.bridge.is_none());
}

#[test]
fn actual_recovery_checkbox_edit_survives_editor_preference_reload() {
    let root = std::env::temp_dir().join(format!(
        "pixelcraft-settings-ui-reload-{}",
        std::process::id()
    ));
    let path = root.join("workflow.json");
    let ctx = egui::Context::default();
    let mut app =
        PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-preference-reload.png")));
    app.restore_workflow_preferences(path.clone());
    app.open_settings_destination("settings-recovery");
    let before = app.doc.clone();
    let size = Vec2::new(1180., 840.);
    for _ in 0..4 {
        let mut output = frame(&mut app, &ctx, size, 1., vec![]);
        output.textures_delta.clear();
    }
    let checkbox = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new("settings-recovery")))
        .unwrap();
    let mut output = frame(&mut app, &ctx, size, 1., click(checkbox.center()));
    output.textures_delta.clear();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["autosave"], false);
    assert_eq!(saved["last_settings_page"], "files");
    assert_eq!(app.doc, before);
    assert!(app.undo.is_empty());
    assert!(app.power.bridge.is_none());
    let ctx = egui::Context::default();
    let mut restored =
        PixelCraft::with_context(&ctx, Some(PathBuf::from("unused-restored-settings.png")));
    restored.restore_workflow_preferences(path.clone());
    restored.action("settings");
    for _ in 0..4 {
        let mut output = frame(&mut restored, &ctx, size, 1., vec![]);
        output.textures_delta.clear();
    }
    assert!(
        ctx.data(|data| data.get_temp::<Rect>(egui::Id::new("settings-recovery")))
            .is_some(),
        "restored last category did not render its recovery control"
    );
    std::fs::remove_dir_all(root).unwrap();
}
