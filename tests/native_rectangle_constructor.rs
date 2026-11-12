use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn rectangle_constructor_normalizes_native_integer_coordinates() {
    run_source(
        &Document::new(1, 1),
        r#"
        local function check(r,x,y,w,h)
            assert(r.x==x and r.y==y and r.width==w and r.height==h,
                'unexpected Rectangle: '..tostring(r.x)..','..tostring(r.y)..','..tostring(r.width)..','..tostring(r.height))
        end
        check(Rectangle(-1.1,'2.9',4294967297,2147483648),-2,2,1,-2147483648)
        check(Rectangle(false,'bad',0/0,math.huge),0,0,0,0)
        check(Rectangle(),0,0,0,0)
        check(Rectangle(7),7,0,0,0)
        local source=Rectangle(1,2,3,4)
        local duplicate=Rectangle(source)
        source.x=99
        check(duplicate,1,2,3,4)
        "#,
        0, 0, None,
    ).unwrap();
}

#[test]
fn rectangle_field_selection_controls_pixel_iteration() {
    run_source(
        &Document::new(1, 1),
        r#"
        local im=Image(8,8)
        local function check(r,expected)
            local coordinates={}
            for pixel in im:pixels(r) do
                coordinates[#coordinates+1]=pixel.x..','..pixel.y
            end
            assert(table.concat(coordinates,';')==expected, 'unexpected Rectangle pixel coverage: '..table.concat(coordinates,';'))
        end
        check(Rectangle{x=1,y=2,w=2,h=1},'1,2;2,2')
        check(Rectangle{y=7,width=7,height=7,1,2,2,1},'1,2;2,2')
        check(Rectangle{x=false,y=2,width=2,height=1,w=7,h=7},'0,2;1,2')
        check(Rectangle{x=1,y=2,width=false,height=1,w=2,h=1},'')
        check(Rectangle{x=1,y=2,w=2,height=1},'')
        check(Rectangle{1.9,'2.9',2,1},'1,2;2,2')
        "#,
        0, 0, None,
    ).unwrap();
}
