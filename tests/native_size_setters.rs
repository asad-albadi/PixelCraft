use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn size_assignments_convert_on_every_write() {
    run_source(&Document::new(1, 1), r#"
        local s=Size(1,2)
        s.width=-1.1; s.height='2.9'
        assert(s.width==-2 and s.height==2, 'Size assignments must floor numeric inputs')
        s.width=4294967297; s.height=2147483648
        assert(s.width==1 and s.height==-2147483648, 'Size assignments must narrow signed32')
        s.width=nil; s.width='3.9'; s.height=false
        assert(s.width==3 and s.height==0, 'repeated writes and failed conversions')
        s.width=math.huge; s.height=0/0
        assert(s.width==0 and s.height==0, 'nonfinite assignments must become zero')
        local copy=Size(s); copy.width=7
        assert(s.width==0 and copy.width==7, 'Size copies must stay independent')
        s.w='4.9'; s.h=-1.1
        assert(s.width==4 and s.height==-2 and s.w==4 and s.h==-2, 'Size aliases must share canonical storage')
    "#, 0, 0, None).unwrap();
}

#[test]
fn mutated_size_controls_real_image_dimensions() {
    run_source(
        &Document::new(1, 1),
        r#"
        local s=Size(1,1)
        s.width='3.9'; s.height=2.9
        local im=Image(s.width,s.height)
        assert(im.width==3 and im.height==2, 'mutated Size must create a 3 by 2 image')
        im:drawPixel(2,1,app.pixelColor.rgba(12,34,56,255))
        assert(im:getPixel(2,1)==app.pixelColor.rgba(12,34,56,255))
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}
