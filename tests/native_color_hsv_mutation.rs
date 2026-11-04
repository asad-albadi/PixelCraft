use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn hsv_value_edit_discards_previous_gray_payload() {
    run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{gray=0,alpha=73}
        c.hsvValue=1
        assert(c.red==255 and c.green==255 and c.blue==255)
        assert(c.gray==255, 'HSV edit must discard the old gray payload')
        assert(c.grayPixel==18943, 'grayPixel must use the edited color and preserved alpha')
        local duplicate=Color(c)
        c.hsvValue=0
        assert(c.gray==0 and c.grayPixel==18688)
        assert(duplicate.gray==255 and duplicate.alpha==73)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}
