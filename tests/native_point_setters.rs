use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn point_assignments_convert_on_every_write() {
    run_source(
        &Document::new(1, 1),
        r#"
        local p=Point(1,2)
        p.x=-1.1; p.y='2.9'
        assert(p.x==-2 and p.y==2, 'Point writes must floor numeric inputs')
        p.x=4294967297; p.y=2147483648
        assert(p.x==1 and p.y==-2147483648, 'Point writes must narrow signed32')
        p.x=nil; p.x='3.9'; p.y=false
        assert(p.x==3 and p.y==0, 'repeated writes and failed conversions')
        p.x=math.huge; p.y=0/0
        assert(p.x==0 and p.y==0, 'nonfinite writes must become zero')
        p.x={}; p.y='invalid'
        assert(p.x==0 and p.y==0, 'invalid types must become zero')
        local copy=Point(p); copy.x=7
        assert(p.x==0 and copy.x==7, 'copies must be independent')
        local fields={}; for k,v in pairs(copy) do fields[k]=v end
        assert(fields.x==7 and fields.y==0)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn mutated_point_positions_actual_pixels() {
    run_source(
        &Document::new(4, 3),
        r#"
        local p=Point()
        p.x=4294967298; p.y=4294967297
        local im=Image(1,1)
        local ink=app.pixelColor.rgba(12,34,56,255)
        im:drawPixel(0,0,ink)
        local destination=Image(4,3)
        destination:drawImage(im,p)
        assert(destination:getPixel(2,1)==ink, 'converted Point must position the actual pixel')
        assert(destination:getPixel(1,1)==0)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}
