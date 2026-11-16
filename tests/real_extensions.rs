use pixelcraft::{extensions::*, model::*};
use serde_json::json;

#[test]
fn unchanged_glass_floor_reflects_selected_layers_across_frames() {
    let mut doc = Document::new(6, 8);
    doc.add_frame(0, false);
    doc.add_layer();
    doc.add_layer();
    for frame in 0..2 {
        doc.layers[0].cel_meta[frame].bounds = Some([1, 1, 1, 2]);
        doc.layers[1].cel_meta[frame].bounds = Some([2, 1, 1, 2]);
        doc.set_pixel(0, frame, 7, [255, 30 + frame as u8, 0, 255]);
        doc.set_pixel(1, frame, 14, [0, 255, 0, 255]);
        doc.set_pixel(2, frame, 9, [0, 0, 255, 255]);
    }

    let reflected = run_source_with_host(
        &doc,
        include_str!("fixtures/scripts/Glass Floor.lua"),
        1,
        0,
        None,
        None,
        json!({"timelineRange":{
            "first_layer":0,"last_layer":1,"first_frame":0,"last_frame":1
        }}),
        Box::new(|request| match request["op"].as_str() {
            Some("preferences") => Ok(json!({})),
            _ => panic!("Unexpected host request: {request}"),
        }),
    )
    .unwrap();

    assert_eq!(reflected.layers.len(), 4);
    let floor = &reflected.layers[3];
    assert_eq!(floor.name, "Glass Floor");
    assert_eq!(floor.opacity, 128);
    for frame in 0..2 {
        // The combined artwork bounds end at y=3. Reflection in the
        // eight-pixel canvas followed by a two-pixel upward shift gives y'=5-y.
        assert_eq!(
            floor.cels[frame][4 * 6 + 1],
            [255, 30 + frame as u8, 0, 255]
        );
        assert_eq!(floor.cels[frame][3 * 6 + 2], [0, 255, 0, 255]);
        assert_eq!(floor.cels[frame][4 * 6 + 3], CLEAR);
        assert_eq!(floor.cels[frame][7], CLEAR);
    }
    assert_eq!(&reflected.layers[..3], &doc.layers);
}

#[test]
fn rectangle_geometry_and_image_specs_follow_the_active_sprite() {
    let mut doc = Document::new(4, 3);
    doc.transparent_index = 2;
    doc.set_color_mode(ColorMode::Indexed);
    run_source(
        &doc,
        r#"
        local a, b = Rectangle(-2, 1, 4, 3), Rectangle(1, 2, 4, 4)
        assert(Rectangle():union(a) == a)
        assert(a:union(b) == Rectangle(-2, 1, 7, 5))
        assert((a | b) == a:union(b))
        assert(a:intersects(b) and b:intersects(a))
        assert(a:intersect(b) == Rectangle(1, 2, 1, 2))
        assert((a & b) == a:intersect(b))
        assert(not a:intersects(Rectangle(2, 1, 1, 1)))
        assert(a:intersect(Rectangle(2, 1, 1, 1)) == Rectangle())
        assert(a:contains(Point(-2, 1)) and not a:contains(Point(2, 1)))
        assert(a:contains(Rectangle(-1, 2, 1, 1)) and not a:contains(Rectangle()))
        assert(a.w == 4 and a.h == 3)

        local sprite = app.activeSprite
        local spec = sprite.spec
        assert(spec.width == 4 and spec.height == 3)
        assert(spec.colorMode == ColorMode.INDEXED and spec.transparentColor == 2)
        assert(spec == sprite.spec and Image(spec).spec == spec)
        spec.width = 2
        assert(sprite.spec.width == 4)
        assert(spec ~= sprite.spec and spec == ImageSpec(spec))

        spec.transparentColor = 5
        local im = Image(spec)
        assert(im.width == 2 and im.height == 3 and im.colorMode == ColorMode.INDEXED)
        assert(im.bounds == Rectangle(0, 0, 2, 3))
        assert(im.spec.transparentColor == 5 and im:getPixel(0, 0) == 5)
        im:drawPixel(0, 0, 5)
        assert(im:getPixel(0, 0) == 5)
        im:clear()
        assert(im:getPixel(0, 0) == 5)
        im.spec.transparentColor = 8
        assert(im.spec.transparentColor == 5)
        assert(im:clone().spec.transparentColor == 5)

        local defaults = ImageSpec()
        assert(defaults.width == 1 and defaults.height == 1 and defaults.colorMode == ColorMode.RGB)
        assert(defaults.transparentColor == 0)
        local gray = Image(ImageSpec{width=2,height=2,colorMode=ColorMode.GRAY})
        gray:drawPixel(1, 1, app.pixelColor.graya(70, 130))
        assert(gray.colorMode == ColorMode.GRAY and gray:getPixel(1, 1) == app.pixelColor.graya(70, 130))

        local rgbMask = app.pixelColor.rgba(10, 20, 30, 0)
        local rgb = Image(ImageSpec{width=2,height=1,transparentColor=rgbMask})
        assert(rgb:getPixel(0, 0) == rgbMask)
        rgb:drawPixel(0, 0, app.pixelColor.rgba(200, 100, 0, 255))
        rgb:clear()
        assert(rgb:getPixel(0, 0) == rgbMask and rgb:getPixel(1, 0) == rgbMask)

        local grayMask = app.pixelColor.graya(40, 0)
        local maskedGray = Image(ImageSpec{colorMode=ColorMode.GRAY,transparentColor=grayMask})
        assert(maskedGray:getPixel(0, 0) == grayMask)
        maskedGray:drawPixel(0, 0, app.pixelColor.graya(100, 255))
        maskedGray:clear()
        assert(maskedGray:getPixel(0, 0) == grayMask)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn unchanged_random_palette_and_pixels_scripts() {
    for (name, source) in [
        (
            "Random Palette",
            include_str!("fixtures/scripts/Random Palette.lua"),
        ),
        (
            "Random Pixels",
            include_str!("fixtures/scripts/Random Pixels.lua"),
        ),
    ] {
        for mode in [ColorMode::Rgba, ColorMode::Grayscale, ColorMode::Indexed] {
            let mut d = Document::new(4, 4);
            d.set_color_mode(mode);
            d.stamp(0, 0, 1, 1, [255, 0, 0, 255], 1);
            let out = run_source(&d, source, 0, 0, None)
                .unwrap_or_else(|e| panic!("{name} {mode:?}: {e}"));
            out.validate().unwrap();
            if name == "Random Palette" {
                assert_ne!(out.palette, d.palette);
            } else {
                assert_ne!(out.layers[0].cels[0], d.layers[0].cels[0]);
                assert_eq!(out.layers[0].cels[0][0][3], 0);
            }
        }
    }
}
#[test]
fn unchanged_move_cels_dialog_moves_pixels() {
    let mut d = Document::new(3, 2);
    d.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    let out = run_source_with_host(
        &d,
        include_str!("fixtures/scripts/Move Cels.lua"),
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(|r| match r["op"].as_str().unwrap() {
            "dialog" => Ok(json!({"widget":"ok","data":{"ok":true,"x":"1","y":"1"}})),
            "preferences" => Ok(json!({})),
            _ => panic!("{r}"),
        }),
    )
    .unwrap();
    assert_eq!(out.layers[0].cels[0][4], [255, 0, 0, 255]);
    assert_eq!(out.layers[0].cels[0][0], CLEAR);
}
#[test]
fn unchanged_wizard_chains_nonmodal_dialogs() {
    let mut calls = 0;
    run_source_with_host(&Document::new(1,1),include_str!("fixtures/scripts/Wizard Example.lua"),0,0,None,None,json!({}),Box::new(move |r|{
  if r["op"]=="pump"{calls+=1;assert!(calls<=3);let dialogs=r["dialogs"].as_array().unwrap();assert_eq!(dialogs.len(),1);let d=&dialogs[0];let w=d["widgets"].as_array().unwrap().last().unwrap();Ok(json!({"dialog":d["id"],"widget":w["id"],"data":{format!("data{calls}"):"hello"},"time":calls}))}else{Ok(json!({}))}
 })).unwrap();
}
