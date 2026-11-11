use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn size_constructor_uses_native_signed_integer_conversion() {
    run_source(
        &Document::new(1, 1),
        r#"
        local function check(s,w,h)
            assert(s.width==w and s.height==h, 'unexpected Size: '..tostring(s.width)..','..tostring(s.height))
        end
        check(Size(-1.1,'2.9'),-2,2)
        check(Size(4294967297,2147483648),1,-2147483648)
        check(Size(false,'bad'),0,0)
        check(Size(),0,0)
        check(Size(0/0,math.huge),0,0)
        local source=Size(-1.1,2.9)
        local duplicate=Size(source)
        source.width=99
        check(duplicate,-2,2)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn size_constructor_selects_a_complete_field_family() {
    run_source(
        &Document::new(1, 1),
        r#"
        local function check(s,w,h)
            assert(s.width==w and s.height==h, 'unexpected Size fields: '..tostring(s.width)..','..tostring(s.height))
        end
        check(Size{w=7,h=9},7,9)
        check(Size{width=0,w=7,h=9,[1]=11,[2]=12},0,0)
        check(Size{width=false,height=-1.1,w=7,h=9},0,-2)
        check(Size{height=99,w='2.9',h='-1.1'},2,-2)
        check(Size{height=99,h=98,4,5},4,5)
        check(Size{w=false,h=6,4,5},0,6)
        check(Size{w=7,height=9,[2]=12},7,0)
        check(Size{width='bad',height=true,4,5},0,0)
        check(Size{'-2.1','3.9'},-3,3)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
