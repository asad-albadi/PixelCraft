use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn size_equality_compares_current_stored_dimensions() {
    run_source(
        &Document::new(1, 1),
        r#"
        local a,b=Size(-2,3),Size(-1.1,'3.9')
        assert(a==b, 'independent Sizes with equal dimensions must compare equal')
        assert(a~=Size(-1,3), 'width must participate')
        assert(a~=Size(-2,4), 'height must participate')
        b.w=4294967294
        assert(a==b, 'aliases and signed32 conversion must affect equality')
        b.h=4
        assert(a~=b, 'current height must participate')
        b.height=3
        assert(a==b)
        assert(a==Size(a) and not rawequal(a,Size(a)), 'copies have value equality')
        assert(a~={width=-2,height=3}, 'ordinary tables are not Sizes')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn rectangle_equality_compares_all_four_current_fields() {
    run_source(
        &Document::new(1, 1),
        r#"
        local a,b=Rectangle(-2,3,4,5),Rectangle(-1.1,'3.9',4,5)
        assert(a==b, 'independent Rectangles with equal fields must compare equal')
        for _,key in ipairs{'x','y','width','height'} do
          local c=Rectangle(a)
          c[key]=c[key]+1
          assert(a~=c, key..' must participate')
          c[key]=a[key]
          assert(a==c, 'comparison must read current '..key)
        end
        b.x=4294967294
        assert(a==b, 'comparison must use stored signed32 fields')
        assert(Rectangle(1,2,0,0)~=Rectangle(8,9,0,0), 'empty rectangles retain position')
        assert(Rectangle(1,2,-3,4)==Rectangle(1,2,-3,4), 'negative extents are not normalized')
        assert(a==Rectangle(a) and not rawequal(a,Rectangle(a)), 'copies have value equality')
        assert(a~={x=-2,y=3,width=4,height=5}, 'ordinary tables are not Rectangles')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
