use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn point_constructor_converts_coordinates_to_native_signed_integers() {
    run_source(
        &Document::new(1, 1),
        r#"
        local function check(p,x,y)
            assert(p.x==x and p.y==y, 'unexpected Point coordinates: '..tostring(p.x)..','..tostring(p.y))
        end
        check(Point(-1.1,'2.9'),-2,2)
        check(Point(4294967297,2147483648),1,-2147483648)
        check(Point(false,'bad'),0,0)
        check(Point(),0,0)
        check(Point(0/0,math.huge),0,0)
        check(Point({x=false,y=-1.1,9,8}),0,-2)
        check(Point({y=91,4,5}),4,5)
        check(Point({x=0,[1]=8,[2]=9}),0,0)
        check(Point({x='bad',y=true,8,9}),0,0)
        check(Point({'-2.1','3.9'}),-3,3)
        local original=Point(-1.1,2.9)
        local duplicate=Point(original)
        original.x=99
        check(duplicate,-2,2)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
