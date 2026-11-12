use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn point_equality_compares_both_converted_coordinates() {
    run_source(
        &Document::new(1, 1),
        r#"
        local a=Point(-2,3)
        local b=Point(-1.1,'3.9')
        assert(a==b, 'independent Points with equal stored coordinates must compare equal')
        assert(a~=Point(-1,3), 'x must participate in equality')
        assert(a~=Point(-2,4), 'y must participate in equality')
        b.x=4294967294
        assert(a==b, 'equality must use converted signed32 coordinates')
        b.y=4
        assert(a~=b, 'equality must read current coordinates')
        assert(a==Point(a), 'independent copy must compare equal')
        assert(not rawequal(a,Point(a)), 'value equality must preserve distinct objects')
        assert(a~={x=-2,y=3}, 'ordinary tables are not Points')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
