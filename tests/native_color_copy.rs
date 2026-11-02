use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn copied_gray_color_retains_signed_storage_and_independent_mutation() {
    let edited = run_source(
        &Document::new(2, 1),
        r#"
        local original=Color{gray=2147483647,alpha=258}
        local cloned=Color(original)
        assert(cloned.gray==2147483647, 'copy lost gray representation')
        assert(cloned.alpha==258, 'copy lost stored alpha')
        original.gray=40
        original.alpha=99
        assert(cloned.gray==2147483647 and cloned.alpha==258, 'copy aliases original')
        local p=cloned.grayPixel
        app.activeImage:drawPixel(0,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        cloned.gray=80
        assert(original.gray==40 and original.alpha==99, 'original aliases copy')
        p=cloned.grayPixel
        app.activeImage:drawPixel(1,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        &*edited.layers[0].cels[0],
        &[[255, 255, 255, 2], [80, 80, 80, 2]]
    );
}
