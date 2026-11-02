use pixelcraft::{extensions::run_source, model::Document};

#[test]
fn indexed_color_assignment_converts_inputs_and_replaces_gray_state() {
    let mut document = Document::new(3, 1);
    document.palette = vec![[10, 20, 30, 255], [40, 50, 60, 255], [70, 80, 90, 255]];
    let edited = run_source(
        &document,
        r#"
        local c=Color{gray=199,alpha=5}
        c.index='2.9'
        assert(c.index==2, 'index assignment conversion')
        app.activeImage:drawPixel(0,0,c.rgbaPixel)
        assert(c.gray==80, 'index assignment retained gray representation')
        c.index=false
        assert(c.index==0, 'failed index conversion must store zero')
        app.activeImage:drawPixel(1,0,c.rgbaPixel)
        c.index=0x100000001
        assert(c.index==1, 'index assignment must narrow to signed32')
        app.activeImage:drawPixel(2,0,c.rgbaPixel)
        "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        &*edited.layers[0].cels[0],
        &[[70, 80, 90, 255], [10, 20, 30, 255], [40, 50, 60, 255]]
    );
}

#[test]
fn indexed_color_constructor_converts_inputs_before_exposing_index() {
    let edited = run_source(
        &Document::new(6, 1),
        r#"
        local cases={
          {'2.9',2}, {-1.2,-2}, {false,0},
          {0x100000003,3}, {0/0,0}, {2147483648,-2147483648}
        }
        for x,case in ipairs(cases) do
          local c=Color{index=case[1]}
          assert(c.index==case[2], 'index constructor conversion at '..x)
          app.activeImage:drawPixel(x-1,0,app.pixelColor.rgba(c.index&255,0,0,255))
        end
        "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        &*edited.layers[0].cels[0],
        &[
            [2, 0, 0, 255],
            [254, 0, 0, 255],
            [0, 0, 0, 255],
            [3, 0, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
        ]
    );
}
