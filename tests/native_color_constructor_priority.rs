use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn gray_precedes_index_and_reaches_image_pixels() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{gray=73.9,index=0,alpha=42.9}
        assert(c.gray==73 and c.alpha==42, 'gray must precede index')
        assert(c==Color{gray=73,alpha=42}, 'retain gray representation')
        local image=Image(1,1)
        image:drawPixel(0,0,c.rgbaPixel)
        assert(image:getPixel(0,0)==app.pixelColor.rgba(73,73,73,42))
        local duplicate=Color(c)
        c.gray=100
        assert(duplicate.gray==73 and duplicate.alpha==42)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn nonnil_gray_selects_gray_even_when_conversion_fails() {
    run_source(
        &Document::new(1, 1),
        r#"
        for _,value in ipairs({false,'bad',-1.9}) do
            local c=Color{gray=value,index=0,alpha=-1.1,a=99}
            local expected=type(value)=='number' and -1 or 0
            assert(c==Color{gray=expected,alpha=-2}, 'nonnil gray wins and uses long alpha')
        end
        assert(Color{gray=12,index=0}.alpha==255)
        assert(Color{r=7,gray=12,index=0}.red==7, 'RGB still wins')
        assert(Color{l=0.5,gray=12,index=0}.hslLightness==0.5, 'HSL still wins')
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}
