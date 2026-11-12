use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn rectangle_assignments_apply_native_integer_conversion_every_time() {
    run_source(
        &Document::new(1, 1),
        r#"
        local r=Rectangle(1,2,3,4)
        r.x=-1.1; r.y='2.9'; r.width=4294967297; r.height=2147483648
        assert(r.x==-2 and r.y==2 and r.width==1 and r.height==-2147483648,
            'rectangle assignments must convert to signed32 integers')
        r.x='bad'; r.y=false; r.width=0/0; r.height=math.huge
        assert(r.x==0 and r.y==0 and r.width==0 and r.height==0,
            'failed conversions must store zero')
        r.x=nil; r.x='3.9'; r.width=-1.1; r.width=2.9
        assert(r.x==3 and r.width==2, 'repeated assignments must still convert')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn rectangle_mutation_controls_image_pixel_coverage() {
    run_source(
        &Document::new(1, 1),
        r#"
        local r=Rectangle(0,0,1,1)
        r.x='1.9'; r.y=2.9; r.width='2.9'; r.height=1.9
        local im=Image(8,8)
        local visited={}
        for pixel in im:pixels(r) do
            visited[#visited+1]=pixel.x..','..pixel.y
            pixel(app.pixelColor.rgba(255,0,0,255))
        end
        assert(table.concat(visited,';')=='1,2;2,2', 'incorrect mutated rectangle coverage')
        assert(im:getPixel(1,2)==app.pixelColor.rgba(255,0,0,255))
        assert(im:getPixel(3,2)==0)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
