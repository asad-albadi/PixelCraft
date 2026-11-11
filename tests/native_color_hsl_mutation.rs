use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn hsl_assignments_change_pixels_and_preserve_alpha() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{red=255,green=0,blue=0,alpha=73}
        c.hslHue=120
        assert(c.red==0 and c.green==255 and c.blue==0, 'HSL hue must change the color')
        c.hslSaturation=0
        assert(c.red==128 and c.green==128 and c.blue==128, 'HSL saturation must change the color')
        c.hslLightness=1
        assert(c.red==255 and c.green==255 and c.blue==255, 'HSL lightness must change the color')
        assert(c.alpha==73)
        local gray=Color{gray=0,alpha=73}
        gray.hslLightness=1
        assert(gray.gray==255 and gray.grayPixel==18943, 'HSL edit must replace gray state')
        local duplicate=Color(gray)
        gray.hslLightness=0
        assert(gray.grayPixel==18688 and duplicate.grayPixel==18943)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn hsl_mutation_retains_unrounded_components_and_copy_state() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{red=255,green=0,blue=0,alpha=73}
        c.hslSaturation=0.123456789
        assert(c.hslSaturation==0.123456789, 'HSL component must not round through RGB')
        assert(c.hslLightness==0.5)
        local duplicate=Color(c)
        c.hslHue=120
        assert(duplicate.hslHue==0 and duplicate.hslSaturation==0.123456789)
        c.hslSaturation=0
        assert(c.hslHue==120, 'achromatic HSL must retain stored hue')
        c.alpha=55
        assert(c.hslHue==120 and c.alpha==55)
        c.red=42
        assert(c.red==42 and c.hslHue~=120, 'RGB assignment must replace HSL state')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn hsl_gray_and_equality_use_stored_components() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{red=255,green=0,blue=0,alpha=73}
        c.hslSaturation=0
        assert(c.gray==127 and c.grayPixel==18815, 'HSL gray truncates stored lightness')
        local near=Color(c)
        near.hslHue=0.0005
        assert(c==near, 'HSL hue equality has a 0.001 tolerance')
        near.hslHue=0.002
        assert(c~=near)
        near=Color(c)
        near.hslSaturation=0.000005
        assert(c==near, 'HSL saturation equality has a 0.00001 tolerance')
        near.hslSaturation=0.00002
        assert(c~=near)
        near=Color(c)
        near.hslLightness=0.500005
        assert(c==near)
        near.hslLightness=0.50002
        assert(c~=near)
        near=Color(c)
        near.alpha=74
        assert(c~=near)
        assert(c~=Color{red=128,green=128,blue=128,alpha=73})
        c.hslLightness=2
        assert(c.hslLightness==2 and c.red==255 and c.gray==510)
        c.hsvValue=0
        assert(c.hslLightness==0, 'HSV assignment must replace HSL state')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn generic_hue_and_saturation_follow_hsl_representation() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{red=255,green=0,blue=0,alpha=73}
        c.hslSaturation=0.25
        c.hue=120
        assert(c.hslHue==120 and c.hslSaturation==0.25 and c.hslLightness==0.5)
        c.saturation=0.125
        assert(c.hslSaturation==0.125 and c.saturation==0.125)
        c.lightness=0.25
        assert(c.hslLightness==0.25 and c.lightness==0.25 and c.alpha==73)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn hsl_transition_uses_full_signed_gray_lightness() {
    run_source(
        &Document::new(1, 1),
        r#"
        for _,gray in ipairs{2147483647,-2147483648} do
          local c=Color{gray=gray,alpha=73}
          c.hslHue=120
          assert(c.hslLightness==gray/255.0, 'gray transition must use the stored gray directly')
          assert(c.hslSaturation==0 and c.alpha==73)
          local want=gray>0 and 255 or 0
          assert(c.red==want and c.green==want and c.blue==want)
        end
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn hsl_rgb_sector_six_follows_original_jump_table() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{red=255,green=0,blue=0,alpha=73}
        c.hslHue=390
        assert(c.red==255 and c.green==128 and c.blue==0, 'sector six uses sector zero channel order')
        c.hslHue=720
        assert(c.red==0 and c.green==0 and c.blue==0, 'native HSL does not wrap all hue sectors')
        c.hslSaturation=0
        c.hslLightness=0
        c.hslHue=0
        local near=Color(c)
        near.hslHue=0.001
        assert(c~=near, 'hue tolerance is strict')
        near=Color(c)
        near.hslSaturation=0.00001
        assert(c~=near, 'saturation tolerance is strict')
        near=Color(c)
        near.hslLightness=0.00001
        assert(c~=near, 'lightness tolerance is strict')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
