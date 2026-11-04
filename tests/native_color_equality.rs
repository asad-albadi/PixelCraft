use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn color_equality_compares_stored_rgb_instead_of_wrapped_pixels() {
    run_source(
        &Document::new(1, 1),
        r#"
        local a=Color{r=1,g=2,b=3,a=4}
        local b=Color{r=257,g=2,b=3,a=4}
        assert(a.rgbaPixel==b.rgbaPixel)
        assert(a~=b, 'wrapped RGB channels must remain distinct')
        assert(a==Color(a), 'equal RGB copies must compare equal')
        assert(a~=Color{r=1,g=2,b=3,a=260}, 'stored alpha must remain distinct')
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn color_equality_retains_gray_and_index_representations() {
    run_source(
        &Document::new(1, 1),
        r#"
        local g=Color{gray=5,alpha=255}
        assert(g~=Color{r=5,g=5,b=5,a=255}, 'gray and RGB are distinct representations')
        assert(g==Color(g))
        assert(g~=Color{gray=261,alpha=255})
        assert(g~=Color{gray=5,alpha=511})
        local a=Color{index=-1}
        local b=Color{index=-2}
        assert(a~=b, 'different stored indices must remain distinct')
        assert(a==Color(a))
        assert(a~=Color{r=0,g=0,b=0,a=255})
        b.index=-1
        assert(a==b)
        b.gray=5
        assert(b==g, 'equality must observe representation replacement')
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}
