use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn hsl_table_retains_components_without_requiring_hue() {
    run_source(&Document::new(1, 1), r#"
        local c=Color{lightness=0.5}
        assert(c.red==128 and c.green==128 and c.blue==128, 'lightness alone selects HSL')
        assert(c.gray==127 and c.alpha==255)
        c=Color{h=120,s=0.123456789,l=0.5,a=73}
        assert(c.hslHue==120 and c.hslSaturation==0.123456789 and c.hslLightness==0.5, 'constructor retains HSL doubles')
        local duplicate=Color(c)
        c.saturation=0
        assert(c.hslHue==120 and duplicate.hslSaturation==0.123456789)
        assert(c.alpha==73)
        local image=Image(1,1)
        image:drawPixel(0,0,c.rgbaPixel)
        assert(image:getPixel(0,0)==1233158272, 'constructed color reaches image as 128,128,128,73')
        c=Color{hue=720,saturation=1,lightness=2,alpha=4294967369}
        assert(c.hslHue==720 and c.hslLightness==2 and c.gray==510 and c.alpha==73)
    "#, 0, 0, None).unwrap();
}

#[test]
fn hsl_constructor_uses_selected_field_family_and_native_priority() {
    run_source(&Document::new(1, 1), r#"
        local c=Color{l=0.5,hue=120,saturation=1,alpha=73}
        assert(c.hslHue==0 and c.hslSaturation==0 and c.alpha==255, 'short HSL must not borrow long fields')
        c=Color{lightness=0.5,h=120,s=1,a=73}
        assert(c.hslHue==0 and c.hslSaturation==0 and c.alpha==255, 'long HSL must not borrow short fields')
        c=Color{l=false,lightness=1,h=false,s='bad',a=false}
        assert(c.hslLightness==0 and c.hslHue==0 and c.hslSaturation==0 and c.alpha==0, 'nonnil failed conversions select short HSL and become zero')
        c=Color{l='0.5',h='120',s='1',a='73.9',gray=0,index=0}
        assert(c.green==255 and c.red==0 and c.alpha==73, 'HSL precedes gray and index')
        c=Color{lightness=0.5,hue=120,saturation=1,alpha='bad'}
        assert(c.green==255 and c.alpha==0)
        c=Color{l=0.5,lightness=1,h=120,s=1,a=-1.1}
        assert(c.hslLightness==0.5 and c.alpha==-2)
        c=Color{red=42,l=0.5,h=120,s=1}
        assert(c.red==42 and c.green==0, 'RGB precedes HSL')
        c=Color{h=120,s=1,v=0,l=0.5}
        assert(c.red==0 and c.green==0 and c.blue==0, 'HSV value precedes HSL lightness')
    "#, 0, 0, None).unwrap();
}
