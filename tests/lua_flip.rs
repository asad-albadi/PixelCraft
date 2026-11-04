use pixelcraft::{extensions::run_source, model::*};

#[test]
fn lua_flip_moves_offset_cels_across_canvas_and_supports_undo() {
    for (orientation, expected) in [("horizontal", 13), ("vertical", 50)] {
        let mut d = Document::new(8, 8);
        d.set_pixel(0, 0, 10, [255, 0, 0, 255]);
        d.layers[0].cel_meta[0].bounds = Some([2, 1, 1, 1]);
        let source = format!(
            r#"
            app.command.Flip{{orientation='{orientation}'}}
            app.command.Undo()
            assert(app.activeCel.position.x==2 and app.activeCel.position.y==1)
            assert(app.activeImage:getPixel(0,0)==app.pixelColor.rgba(255,0,0,255))
            app.command.Redo()
        "#
        );
        let out = run_source(&d, &source, 0, 0, None).unwrap();
        let mut pixels = vec![CLEAR; 64];
        pixels[expected] = [255, 0, 0, 255];
        assert_eq!(out.composite(0), pixels, "{orientation}");
        out.validate().unwrap();
    }
}

#[test]
fn lua_flip_selection_can_write_outside_original_cel_bounds() {
    let mut d = Document::new(8, 1);
    d.set_pixel(0, 0, 2, [255, 0, 0, 255]);
    d.set_pixel(0, 0, 3, [0, 255, 0, 255]);
    d.layers[0].cel_meta[0].bounds = Some([2, 0, 2, 1]);
    let out = run_source(
        &d,
        r#"
        app.activeSprite.selection:select(Rectangle(1,0,5,1))
        app.activeSprite.selection:subtract(Rectangle(3,0,1,1))
        app.command.Flip{target='mask',orientation='horizontal'}
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        out.composite(0),
        vec![
            CLEAR,
            CLEAR,
            CLEAR,
            [0, 255, 0, 255],
            [255, 0, 0, 255],
            CLEAR,
            CLEAR,
            CLEAR
        ]
    );
    out.validate().unwrap();
}

#[test]
fn lua_flip_keeps_off_canvas_pixels_and_cel_size() {
    let d = Document::new(8, 1);
    let out = run_source(
        &d,
        r#"
        local image=Image(2,1)
        image:drawPixel(0,0,Color{r=255,g=0,b=0})
        image:drawPixel(1,0,Color{r=0,g=255,b=0})
        app.activeCel.image=image
        app.activeCel.position=Point(-1,0)
        app.command.Flip{orientation='horizontal'}
        assert(app.activeCel.position.x==7)
        assert(app.activeImage.width==2)
        assert(app.activeImage:getPixel(0,0)==app.pixelColor.rgba(0,255,0,255))
        assert(app.activeImage:getPixel(1,0)==app.pixelColor.rgba(255,0,0,255))
        app.command.Flip{orientation='horizontal'}
        assert(app.activeCel.position.x==-1)
        assert(app.activeImage:getPixel(0,0)==app.pixelColor.rgba(255,0,0,255))
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(out.composite(0)[0], [0, 255, 0, 255]);
    out.validate().unwrap();
}
