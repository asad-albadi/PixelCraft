use pixelcraft::{formats::*, model::*};
fn cel_chunk(b: &[u8]) -> usize {
    let mut c = 144;
    while c < b.len() {
        let n = u32::from_le_bytes(b[c..c + 4].try_into().unwrap()) as usize;
        if u16::from_le_bytes(b[c + 4..c + 6].try_into().unwrap()) == 0x2005 {
            return c;
        }
        c += n;
    }
    panic!("missing cel")
}
#[test]
fn off_canvas_cel_opacity_and_profile_are_retained() {
    let mut d = Document::new(2, 2);
    d.layers[0].cels[0][0] = [250, 0, 0, 255];
    let mut b = encode_ase(&d).unwrap();
    let c = cel_chunk(&b);
    b[c + 8..c + 10].copy_from_slice(&(-1i16).to_le_bytes());
    b[c + 12] = 100;
    let read = decode_ase(&b).unwrap();
    assert_eq!(read.layers[0].cel_meta[0].opacity, 100);
    assert_eq!(
        read.layers[0].cel_meta[0].outside[0].color,
        [250, 0, 0, 255]
    );
    let mut read = read;
    read.color_profile = vec![
        2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 1, 2, 3, 4,
    ];
    assert_eq!(decode_ase(&encode_ase(&read).unwrap()).unwrap(), read);
}
#[test]
fn indexed_gray_groups_tags_metadata_roundtrip() {
    for mode in [ColorMode::Indexed, ColorMode::Grayscale] {
        let mut d = Document::new(3, 2);
        d.set_color_mode(mode);
        d.stamp(0, 0, 1, 1, [45, 90, 135, 255], 1);
        let group = d.add_group("Group").unwrap();
        d.layers[0].parent = Some(group);
        d.layers[0].blend = BlendMode::Screen;
        d.layers[0].cel_meta[0].opacity = 190;
        d.tags.push(FrameTag {
            name: "Walk".into(),
            from: 0,
            to: 0,
            direction: 2,
            repeat: 0,
            color: [30, 40, 50],
        });
        d.metadata.push(MetadataChunk {
            frame: 0,
            kind: 0x2020,
            data: vec![0, 0, 0, 0],
            owner: "layer:0".into(),
        });
        let b = encode_ase(&d).unwrap();
        assert_eq!(
            u16::from_le_bytes([b[12], b[13]]),
            if mode == ColorMode::Indexed { 8 } else { 16 }
        );
        let read = decode_ase(&b).unwrap();
        assert_eq!(read.composite(0), d.composite(0));
        assert_eq!(read.tags, d.tags);
        assert_eq!(read.palette, d.palette);
        assert_eq!(read.layers[1].parent, Some(0));
        assert_eq!(read.layers[1].cel_meta[0].opacity, 190);
        assert_eq!(read.metadata[0].owner, "layer:1");
    }
}
#[test]
fn native_project_preserves_metadata_with_non_default_brush() {
    let mut d = Document::new(3, 3);
    d.brush_shape = BrushShape::Diamond;
    d.metadata.push(MetadataChunk {
        frame: 0,
        kind: 0x2022,
        data: vec![1, 2, 3],
        owner: "".into(),
    });
    assert_eq!(decode_project(&encode_project(&d).unwrap()).unwrap(), d);
}
#[test]
fn ase_tilemap_roundtrip_preserves_tile_ids_flips_and_pixels() {
    let mut d = Document::new(4, 2);
    d.tilesets.push(Tileset {
        name: "tiles".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], CLEAR].into()],
    });
    d.add_tile_layer(0).unwrap();
    d.paint_tile(1, 0, 0, 0, 1).unwrap();
    d.paint_tile(1, 0, 1, 0, 0x80000001).unwrap();
    let r = decode_ase(&encode_ase(&d).unwrap()).unwrap();
    assert_eq!(r, d);
}
#[test]
fn ase_palette_names_and_grid_metadata_are_preserved() {
    let d = Document::new(2, 2);
    let mut bytes = encode_ase(&d).unwrap();
    bytes[34] = 2;
    bytes[36..38].copy_from_slice(&(-3i16).to_le_bytes());
    bytes[40..42].copy_from_slice(&16u16.to_le_bytes());
    let read = decode_ase(&bytes).unwrap();
    let saved = encode_ase(&read).unwrap();
    assert_eq!(&saved[34..44], &bytes[34..44]);
}
#[test]
fn duplicate_palette_indices_survive_ase_roundtrip() {
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    d.layers[0].cels[0][1] = [255, 0, 0, 255];
    d.layers[0].cel_meta[0].indices = Some(vec![1, 2].into());
    let mut read = decode_ase(&encode_ase(&d).unwrap()).unwrap();
    assert_eq!(
        read.layers[0].cel_meta[0]
            .indices
            .as_ref()
            .unwrap()
            .as_ref(),
        &vec![1, 2]
    );
    read.set_palette_color(2, [0, 255, 0, 255]);
    assert_eq!(read.composite(0), vec![[255, 0, 0, 255], [0, 255, 0, 255]]);
}
#[test]
fn animated_palettes_survive_ase_roundtrip() {
    let mut d = Document::new(1, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    d.add_frame(0, true);
    d.frame_palettes = vec![d.palette.clone(), vec![CLEAR, [0, 255, 0, 255]]];
    d.layers[0].cels[1][0] = [0, 255, 0, 255];
    let read = decode_ase(&encode_ase(&d).unwrap()).unwrap();
    assert_eq!(read.composite(0), d.composite(0));
    assert_eq!(read.composite(1), d.composite(1));
    assert_eq!(read.frame_palettes, d.frame_palettes);
}
#[test]
fn indexed_background_keeps_opaque_transparent_index() {
    let mut d = Document::new(1, 1);
    d.palette = vec![[12, 34, 56, 255]];
    d.layers[0].cels[0][0] = [12, 34, 56, 255];
    d.color_mode = ColorMode::Indexed;
    let mut b = encode_ase(&d).unwrap();
    let mut c = 144;
    while c < b.len() {
        let len = u32::from_le_bytes(b[c..c + 4].try_into().unwrap()) as usize;
        if b[c + 4..c + 6] == 0x2004u16.to_le_bytes() {
            b[c + 6] |= 8;
            break;
        }
        c += len;
    }
    let read = decode_ase(&b).unwrap();
    assert_eq!(read.composite(0), vec![[12, 34, 56, 255]]);
    let reread = decode_ase(&encode_ase(&read).unwrap()).unwrap();
    assert_eq!(reread.composite(0), read.composite(0));
}
#[test]
fn ase_preserves_palette_entry_names_layer_uuid_flags_and_tag_repeat() {
    let mut d = Document::new(1, 1);
    d.palette_names = vec![String::new(); d.palette.len()];
    d.palette_names[1] = "Midnight ink".into();
    d.layers[0].uuid = vec![42; 16];
    d.layers[0].ase_flags = 32;
    d.tags.push(FrameTag {
        name: "Idle".into(),
        from: 0,
        to: 0,
        direction: 0,
        repeat: 3,
        color: [1, 2, 3],
    });
    assert_eq!(decode_ase(&encode_ase(&d).unwrap()).unwrap(), d);
}
#[test]
fn linked_indexed_cel_uses_destination_frames_palette() {
    let mut d = Document::new(1, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    d.add_frame(0, true);
    d.frame_palettes = vec![d.palette.clone(), vec![CLEAR, [0, 255, 0, 255]]];
    d.layers[0].cels[1][0] = [0, 255, 0, 255];
    let mut b = encode_ase(&d).unwrap();
    let f = 128 + u32::from_le_bytes(b[128..132].try_into().unwrap()) as usize;
    let mut c = f + 16;
    loop {
        let n = u32::from_le_bytes(b[c..c + 4].try_into().unwrap()) as usize;
        if b[c + 4..c + 6] == 0x2005u16.to_le_bytes() {
            let mut header = b[c + 6..c + 22].to_vec();
            header[7..9].copy_from_slice(&1u16.to_le_bytes());
            header.extend(0u16.to_le_bytes());
            let mut chunk = 24u32.to_le_bytes().to_vec();
            chunk.extend(0x2005u16.to_le_bytes());
            chunk.extend(header);
            b.splice(c..c + n, chunk);
            let size = u32::from_le_bytes(b[f..f + 4].try_into().unwrap()) - (n as u32) + 24;
            b[f..f + 4].copy_from_slice(&size.to_le_bytes());
            break;
        }
        c += n;
    }
    let len = b.len() as u32;
    b[..4].copy_from_slice(&len.to_le_bytes());
    assert_eq!(decode_ase(&b).unwrap().composite(1), vec![[0, 255, 0, 255]]);
}
