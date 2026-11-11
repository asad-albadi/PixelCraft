use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn hsv_setters_convert_failed_numbers_to_zero_and_accept_numeric_strings() {
    run_source(
        &Document::new(1, 1),
        r#"
        local image=Image(1,1)
        local function pixel(c,r,g,b)
            assert(c.alpha==73, 'HSV assignment preserves alpha')
            image:drawPixel(0,0,c.rgbaPixel)
            assert(image:getPixel(0,0)==app.pixelColor.rgba(r,g,b,73))
        end
        for _,key in ipairs({'hsvHue','hue','hsvSaturation','saturation','hsvValue','value'}) do
            for _,bad in ipairs({false,true,'bad',{},function()end}) do
                local c=Color{r=0,g=255,b=0,a=73}
                c[key]=bad
                if key=='hsvHue' or key=='hue' then pixel(c,255,0,0)
                elseif key=='hsvSaturation' or key=='saturation' then pixel(c,255,255,255)
                else pixel(c,0,0,0) end
            end
            local c=Color{r=0,g=255,b=0,a=73}
            c[key]=nil
            if key=='hsvHue' or key=='hue' then pixel(c,255,0,0)
            elseif key=='hsvSaturation' or key=='saturation' then pixel(c,255,255,255)
            else pixel(c,0,0,0) end
        end
        local c=Color{r=255,g=0,b=0,a=73}
        c.hsvHue='120';pixel(c,0,255,0)
        c.hsvSaturation='0';pixel(c,255,255,255)
        c.hsvValue='0.5';pixel(c,128,128,128)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
