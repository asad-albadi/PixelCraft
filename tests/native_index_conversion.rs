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
        &edited.layers[0].cels[0].to_vec(),
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
        &edited.layers[0].cels[0].to_vec(),
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

#[test]
fn rgb_color_lookup_prefers_exact_matches_then_weighted_palette_fit() {
    let mut document = Document::new(1, 1);
    document.palette = vec![
        [98, 99, 100, 255],
        [100, 120, 100, 255],
        [130, 100, 100, 255],
        [200, 200, 200, 255],
        [207, 207, 207, 255],
        [200, 200, 200, 0],
    ];
    run_source(
        &document,
        r#"
        assert(Color('#626364').index==0, 'exact match may use the mask slot')
        assert(Color('#646464').index==2, 'best fit must skip the mask and weight green more than red')
        assert(Color('#cecece').index==3, 'colors in the same five-bit bin tie by palette order')
        assert(Color('#cfcfcf').index==4, 'an exact match beats an earlier five-bit tie')
        assert(Color('#c8c8c800').index==5, 'exact transparent color beats the default mask')
        assert(Color('#c8c8c807').index==0, 'low alpha falls back to the mask')
        assert(Color{r=463,g=207,b=207,a=255}.index==4, 'exact matching uses packed byte channels')
        assert(Color{index=0}.index==0, 'authored indices bypass matching')
        local image=Image(2,1,ColorMode.INDEXED)
        image:clear(Color('#646464'))
        assert(image:getPixel(0,0)==2 and image:getPixel(1,0)==2)
        image:drawPixel(1,0,Color('#c8c8c800'))
        assert(image:getPixel(1,0)==5, 'transparent swatch keeps its authored slot')
        "#,
        0,
        0,
        None,
    )
    .unwrap();
}
