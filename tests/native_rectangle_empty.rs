use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn rectangle_empty_tracks_nonpositive_stored_extents() {
    run_source(
        &Document::new(1, 1),
        r#"
        assert(Rectangle(0,0,1,1).isEmpty==false, 'positive extents are nonempty')
        assert(Rectangle(0,0,0,1).isEmpty==true, 'zero width is empty')
        assert(Rectangle(0,0,1,0).isEmpty==true, 'zero height is empty')
        assert(Rectangle(0,0,-1,2).isEmpty==true, 'negative width is empty')
        assert(Rectangle(0,0,2,-1).isEmpty==true, 'negative height is empty')
        local r=Rectangle(-2147483648,2147483647,2,3)
        assert(r.isEmpty==false, 'position does not affect emptiness')
        r.width='-0.1'
        assert(r.isEmpty==true, 'getter reads converted current width')
        r.width=4294967297
        assert(r.isEmpty==false, 'getter reads signed32 stored width')
        r.height=2147483648
        assert(r.isEmpty==true, 'wrapped negative height is empty')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
