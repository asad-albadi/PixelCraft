use pixelcraft::{extensions, formats::*, model::*};
fn tile_doc() -> Document {
    let mut d = Document::new(3, 2);
    d.tilesets.push(Tileset {
        name: "set".into(),
        width: 2,
        height: 2,
        tiles: vec![
            vec![
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 0, 255],
            ]
            .into(),
        ],
    });
    d.add_tile_layer(0).unwrap();
    d.paint_tile(1, 0, 0, 0, 1).unwrap();
    d
}
fn mutate_chunk(b: &mut Vec<u8>, kind: u16, change: impl FnOnce(&mut Vec<u8>)) {
    let mut p = 144;
    while p < b.len() {
        let n = u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize;
        if u16::from_le_bytes(b[p + 4..p + 6].try_into().unwrap()) == kind {
            let mut c = b[p + 6..p + n].to_vec();
            change(&mut c);
            let mut out = ((c.len() + 6) as u32).to_le_bytes().to_vec();
            out.extend(kind.to_le_bytes());
            out.extend(c);
            let delta = out.len() as i64 - n as i64;
            b.splice(p..p + n, out);
            let len = b.len() as u32;
            b[..4].copy_from_slice(&len.to_le_bytes());
            let size = (u32::from_le_bytes(b[128..132].try_into().unwrap()) as i64 + delta) as u32;
            b[128..132].copy_from_slice(&size.to_le_bytes());
            return;
        }
        p += n;
    }
    panic!("chunk missing");
}
#[test]
fn sparse_tileset_ids_are_not_layer_indices() {
    let d = tile_doc();
    let mut b = encode_ase(&d).unwrap();
    mutate_chunk(&mut b, 0x2023, |c| {
        c[..4].copy_from_slice(&42u32.to_le_bytes())
    });
    assert_eq!(decode_ase(&b).unwrap().composite(0), d.composite(0));
}
#[test]
fn negative_subtile_origin_survives_save() {
    let mut b = encode_ase(&tile_doc()).unwrap();
    let mut p = 144;
    loop {
        let n = u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as usize;
        if b[p + 4..p + 6] == 0x2005u16.to_le_bytes()
            && u16::from_le_bytes(b[p + 6..p + 8].try_into().unwrap()) == 1
        {
            break;
        }
        p += n;
    }
    b[p + 8..p + 10].copy_from_slice(&(-1i16).to_le_bytes());
    let d = decode_ase(&b).unwrap();
    assert_eq!(d.composite(0)[0], [0, 255, 0, 255]);
    assert_eq!(
        decode_ase(&encode_ase(&d).unwrap()).unwrap().composite(0),
        d.composite(0)
    );
}
#[test]
fn lua_selection_undo_color_and_commands() {
    let d = Document::new(3, 2);
    let out=extensions::run_source(&d,r#"
local s=app.activeSprite
s.selection:select(Rectangle(0,0,1,1))
assert(s.selection:contains(Point(0,0)) and not s.selection:contains(1,0))
app.transaction('paint',function() app.activeImage:drawPixel(0,0,Color{h=120,s=1,v=1}.rgbaPixel) end)
app.command.Undo(); assert(app.activeImage:getPixel(0,0)==0)
app.command.Redo(); assert(app.pixelColor.rgbaG(app.activeImage:getPixel(0,0))==255)
app.command.NewLayer{ name='Extension layer' }; assert(app.activeLayer.name=='Extension layer')
app.fgColor=Color{r=5,g=6,b=7};assert(app.fgColor.red==5)
"#,0,0,None).unwrap();
    assert_eq!(out.layers.len(), 2);
}
#[test]
fn compact_tile_encodings_and_legacy_empty_ids() {
    use std::io::Write;
    fn z(bytes: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }
    // Remove the image layer so the first cel is the tilemap under test.
    let mut d = tile_doc();
    d.remove_layer(0);
    for bits in [8u16, 16, 32] {
        let mut b = encode_ase(&d).unwrap();
        mutate_chunk(&mut b, 0x2005, |c| {
            c[20..22].copy_from_slice(&bits.to_le_bytes());
            let mask = (1u32 << (bits - 3)) - 1;
            for (offset, v) in [
                (22, mask),
                (26, 1 << (bits - 1)),
                (30, 1 << (bits - 2)),
                (34, 1 << (bits - 3)),
            ] {
                c[offset..offset + 4].copy_from_slice(&v.to_le_bytes());
            }
            let mut bytes = vec![];
            for v in &d.layers[0].tile_cels[0] {
                bytes.extend(&v.to_le_bytes()[..bits as usize / 8]);
            }
            c.truncate(48);
            c.extend(z(&bytes));
        });
        let decoded = decode_ase(&b).unwrap();
        assert_eq!(decoded.composite(0), d.composite(0));
        assert_eq!(
            decode_ase(&encode_ase(&decoded).unwrap())
                .unwrap()
                .composite(0),
            d.composite(0)
        );
    }
    let mut b = encode_ase(&d).unwrap();
    mutate_chunk(&mut b, 0x2023, |c| {
        c[4..8].copy_from_slice(&2u32.to_le_bytes());
        c[8..12].copy_from_slice(&1u32.to_le_bytes());
        let name = u16::from_le_bytes(c[32..34].try_into().unwrap()) as usize;
        let end = 34 + name;
        c.truncate(end);
        let bytes: Vec<u8> = d.tilesets[0].tiles[0].iter().flatten().copied().collect();
        let compressed = z(&bytes);
        c.extend((compressed.len() as u32).to_le_bytes());
        c.extend(compressed);
    });
    mutate_chunk(&mut b, 0x2005, |c| {
        c.truncate(48);
        c.extend(z(&[0u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat()));
    });
    let decoded = decode_ase(&b).unwrap();
    assert_eq!(decoded.composite(0), d.composite(0));
}
#[test]
fn external_tilesets_resolve_relative_to_file_and_embed_on_save() {
    let mut d = tile_doc();
    d.ase_tilesets.push(AseTilesetInfo {
        index: 0,
        id: 42,
        flags: 7,
        base_index: 1,
        external: Some([7, 42]),
    });
    let mut entry = vec![];
    entry.extend(1u32.to_le_bytes());
    entry.extend([0; 8]);
    entry.extend(7u32.to_le_bytes());
    entry.push(1);
    entry.extend([0; 7]);
    entry.extend(14u16.to_le_bytes());
    entry.extend(b"tiles.aseprite");
    d.metadata.push(MetadataChunk {
        frame: 0,
        kind: 0x2008,
        owner: String::new(),
        data: entry,
    });
    let mut bytes = encode_ase(&d).unwrap();
    mutate_chunk(&mut bytes, 0x2023, |c| {
        c[4..8].copy_from_slice(&5u32.to_le_bytes());
        let n = u16::from_le_bytes(c[32..34].try_into().unwrap()) as usize;
        c.truncate(34 + n + 8);
    });
    assert!(decode_ase(&bytes).unwrap_err().contains("resolver"));
    let mut calls = 0;
    let decoded = decode_ase_with_resolver(&bytes, &mut |file, id| {
        assert_eq!(file, "tiles.aseprite");
        assert_eq!(id, 42);
        calls += 1;
        Ok(d.tilesets[0].clone())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(decoded.composite(0), d.composite(0));
    let root = std::env::temp_dir().join(format!("pixelcraft-external-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    save(&root.join("tiles.aseprite"), &d).unwrap();
    std::fs::write(root.join("main.aseprite"), &bytes).unwrap();
    assert_eq!(
        load(&root.join("main.aseprite")).unwrap().composite(0),
        d.composite(0)
    );
    let standalone = encode_ase(&decoded).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(
        decode_ase(&standalone).unwrap().composite(0),
        d.composite(0)
    );
}
#[test]
fn lua_frame_commands_are_individually_undoable() {
    let d = Document::new(1, 1);
    extensions::run_source(&d,"app.command.NewFrame();assert(#app.activeSprite.frames==2);app.command.Undo();assert(#app.activeSprite.frames==1);app.command.Redo();assert(#app.activeSprite.frames==2)",0,0,None).unwrap();
}
#[test]
fn lua_clear_uses_sprite_coordinates_for_offset_cels() {
    let mut d = Document::new(6, 6);
    d.stamp(0, 0, 4, 4, [255, 0, 0, 255], 1);
    d.layers[0].cel_meta[0].bounds = Some([4, 4, 1, 1]);
    let out = extensions::run_source(
        &d,
        "app.activeSprite.selection:select(Rectangle(4,4,1,1));app.command.Clear()",
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(out.composite(0)[28][3], 0);
}
#[test]
fn lua_clear_uses_the_actual_transparent_index() {
    let mut d = Document::new(1, 1);
    d.palette = vec![[255, 0, 0, 255], CLEAR];
    d.transparent_index = 1;
    d.set_color_mode(ColorMode::Indexed);
    d.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    let out = extensions::run_source(&d, "app.command.Clear()", 0, 0, None).unwrap();
    assert_eq!(out.composite(0)[0][3], 0);
}
#[test]
fn lua_change_pixel_format_converts_before_next_api_call() {
    let mut d = Document::new(1, 1);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    let out=extensions::run_source(&d,"app.command.ChangePixelFormat{format='gray'};assert(app.activeImage:getPixel(0,0)==app.pixelColor.graya(54,255),'conversion must be immediate')",0,0,None).unwrap();
    assert_eq!(out.layers[0].cels[0][0], [54, 54, 54, 255]);
}
