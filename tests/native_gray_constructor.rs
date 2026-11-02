use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn gray_constructor_truncates_before_packing_painted_pixels() {
    let d = Document::new(7, 1);
    let edited = run_source(
        &d,
        r#"
        local cases = {
            {-1.9,-1}, {'258.9',258}, {false,0},
            {2147483648,-2147483648}, {0/0,-2147483648},
            {-2147483648.9,-2147483648}, {2147483647.9,2147483647}
        }
        for i,v in ipairs(cases) do
            local c = Color{gray=v[1]}
            assert(c.gray==v[2], 'gray conversion at '..i)
            local p=c.grayPixel
            app.activeImage:drawPixel(i-1,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        end
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        &*edited.layers[0].cels[0],
        &[
            [255, 255, 255, 255],
            [2, 2, 2, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
            [255, 255, 255, 255]
        ]
    );
}

#[test]
fn gray_constructor_uses_long_alpha_and_retains_signed_integer_state() {
    let d = Document::new(4, 1);
    let edited = run_source(
        &d,
        r#"
        local cases = {
            {input={gray=80,alpha='258.9'}, alpha=258},
            {input={gray=80,alpha=false,a=99}, alpha=0},
            {input={gray=80,a=99}, alpha=255},
            {input={gray=80,alpha=0x100000063}, alpha=99}
        }
        for i,v in ipairs(cases) do
            local c=Color(v.input)
            assert(c.alpha==v.alpha, 'gray alpha conversion at '..i)
            local p=c.grayPixel
            app.activeImage:drawPixel(i-1,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        end
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        &*edited.layers[0].cels[0],
        &[
            [80, 80, 80, 2],
            [80, 80, 80, 0],
            [80, 80, 80, 255],
            [80, 80, 80, 99]
        ]
    );
}
