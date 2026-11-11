use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn hsv_value_selects_before_gray_index_and_hsl() {
    run_source(
        &Document::new(1, 1),
        r#"
        local image=Image(1,1)
        for _,case in ipairs({
            {Color{value=1},app.pixelColor.rgba(255,255,255,255)},
            {Color{hue=120,saturation=1,value=1,gray=73,index=0,lightness=0.5},app.pixelColor.rgba(0,255,0,255)},
            {Color{h=120,s=1,v=1,gray=73,index=0,l=0.5},app.pixelColor.rgba(0,255,0,255)}
        }) do
            image:drawPixel(0,0,case[1].rgbaPixel)
            assert(image:getPixel(0,0)==case[2], 'value selects HSV without requiring hue')
        end
        local c=Color{hue=120,saturation=1,value=1,gray=73}
        assert(c.red==0 and c.green==255 and c.blue==0, 'HSV precedes gray')
        assert(Color{r=7,v=1}.red==7, 'RGB still precedes HSV')
        "#,
        0, 0, None,
    ).unwrap();
}

#[test]
fn hsv_constructor_uses_selected_field_family_and_native_numeric_conversion() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{v=1,hue=120,saturation=1,alpha=73}
        assert(c.red==255 and c.green==255 and c.blue==255 and c.alpha==255,
            'short value uses only short fields')
        c=Color{value='1',hue='120',saturation='1',alpha='73.9',h=0,s=0,a=99}
        assert(c.red==0 and c.green==255 and c.blue==0 and c.alpha==73,
            'long family numeric conversion')
        c=Color{v=false,value=1,h=120,s=1,a=false,gray=73}
        assert(c.red==0 and c.green==0 and c.blue==0 and c.alpha==0,
            'nonnil false selects short family with failed conversion zero')
        c=Color{v=1,h='bad',s=false,a=-1.1}
        assert(c.red==255 and c.green==255 and c.blue==255 and c.alpha==-2)
        local image=Image(1,1)
        image:drawPixel(0,0,c.rgbaPixel)
        assert(image:getPixel(0,0)==app.pixelColor.rgba(255,255,255,254))
        c=Color{value=1,alpha=4294967369}
        assert(c.alpha==73, 'constructor alpha narrows to signed32')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
