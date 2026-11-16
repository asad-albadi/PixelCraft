use pixelcraft::{formats::*, model::*};
#[test]
fn native_and_ase_projects_preserve_editable_drawing_grid_geometry() {
    let mut doc = Document::new(8, 8);
    let grid = DrawingGrid {
        origin: [-12, 9],
        size: [36, 17],
    };
    doc.drawing_grid = Some(grid);
    let native = decode_project(&encode_project(&doc).unwrap()).unwrap();
    assert_eq!(native.drawing_grid(), grid);
    let ase = encode_ase(&doc).unwrap();
    assert_eq!(&ase[36..44], &[244, 255, 9, 0, 36, 0, 17, 0]);
    let reopened = decode_ase(&ase).unwrap();
    assert_eq!(reopened.drawing_grid(), grid);

    // A zero-height grid is treated as absent in older ASE headers.
    let mut legacy = ase;
    legacy[42..44].fill(0);
    let reopened = decode_ase(&legacy).unwrap();
    assert_eq!(reopened.drawing_grid, None);
    assert_eq!(reopened.drawing_grid(), DrawingGrid::default());
    assert_eq!(&encode_ase(&reopened).unwrap()[36..44], &legacy[36..44]);

    doc.drawing_grid = Some(DrawingGrid {
        size: [0, 16],
        ..grid
    });
    assert!(encode_project(&doc).is_err());
    assert!(encode_ase(&doc).is_err());
}

#[test]
fn ase_roundtrip_preserves_pixels_layers_visibility_and_timing() {
    let mut d = Document::new(4, 3);
    d.layers[0].name = "Colors".into();
    d.layers[0].cels[0][3] = [30, 60, 90, 128];
    d.add_layer();
    d.layers[1].visible = false;
    d.layers[1].opacity = 172;
    d.add_frame(0, true);
    d.durations[1] = 230;
    d.layers[1].cels[1][9] = [250, 210, 50, 255];
    let bytes = encode_ase(&d).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 0xA5E0);
    assert_eq!(decode_ase(&bytes).unwrap(), d);
}
#[test]
fn malformed_or_unsupported_files_return_errors_without_panics() {
    for n in 0..128 {
        assert!(decode_ase(&vec![0; n]).is_err());
    }
    let mut d = Document::new(2, 2);
    d.set_pixel(0, 0, 0, [1, 2, 3, 255]);
    let mut b = encode_ase(&d).unwrap();
    b[12] = 8;
    assert!(decode_ase(&b).is_err());
    let mut b = encode_ase(&d).unwrap();
    b[128..132].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_ase(&b).is_err());
}

#[test]
fn png_sheet_and_gif_export_contain_the_frames() {
    use image::AnimationDecoder;
    let d = Document::demo();
    let dir = std::env::temp_dir().join(format!("pixelcraft-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("frame.png");
    let sheet = dir.join("sheet.png");
    let gif = dir.join("animation.gif");
    export_png(&png, &d, 0, false).unwrap();
    let read = load(&png).unwrap();
    assert_eq!(read.composite(0), d.composite(0));
    export_png(&sheet, &d, 0, true).unwrap();
    assert_eq!(
        image::open(&sheet).unwrap().width(),
        (d.width * d.durations.len()) as u32
    );
    export_gif(&gif, &d).unwrap();
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(&gif).unwrap(),
    ))
    .unwrap();
    let frames = decoder.into_frames().collect_frames().unwrap();
    assert_eq!(frames.len(), 4);
    assert_eq!(frames[0].delay().numer_denom_ms(), (100, 1));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn import_preserves_off_canvas_artwork() {
    let mut d = Document::new(2, 2);
    d.layers[0].cels[0][0] = [250, 0, 0, 255];
    let mut bytes = encode_ase(&d).unwrap();
    let mut c = 144;
    while c < bytes.len() {
        let size = u32::from_le_bytes(bytes[c..c + 4].try_into().unwrap()) as usize;
        let kind = u16::from_le_bytes(bytes[c + 4..c + 6].try_into().unwrap());
        if kind == 0x2005 {
            bytes[c + 8..c + 10].copy_from_slice(&(-1i16).to_le_bytes());
            break;
        }
        c += size;
    }
    let read = decode_ase(&bytes).unwrap();
    assert_eq!(
        read.layers[0].cel_meta[0].outside[0].color,
        [250, 0, 0, 255]
    );
}

#[test]
fn import_preserves_cel_opacity() {
    let mut d = Document::new(2, 2);
    d.layers[0].cels[0][0] = [250, 0, 0, 255];
    for opacity in [0, 100] {
        let mut bytes = encode_ase(&d).unwrap();
        let mut c = 144;
        while c < bytes.len() {
            let size = u32::from_le_bytes(bytes[c..c + 4].try_into().unwrap()) as usize;
            let kind = u16::from_le_bytes(bytes[c + 4..c + 6].try_into().unwrap());
            if kind == 0x2005 {
                bytes[c + 12] = opacity;
                break;
            }
            c += size;
        }
        let read = decode_ase(&bytes).unwrap();
        assert_eq!(read.layers[0].cel_meta[0].opacity, opacity);
    }
}

#[test]
fn full_hd_animation_can_add_layers_past_the_old_document_limit() {
    let mut d = Document::new(1920, 1080);
    for frame in 0..15 {
        d.add_frame(frame, true);
    }
    d.add_layer();
    assert_eq!(d.layers.len(), 2);
    d.validate().unwrap();
}

#[test]
fn large_blank_ase_timeline_roundtrips_without_full_canvas_cels() {
    let mut doc = Document::new(4096, 4096);
    for frame in 0..8 {
        doc.add_frame(frame, false);
    }
    let reopened = decode_ase(&encode_ase(&doc).unwrap()).unwrap();
    assert_eq!(reopened.durations.len(), 9);
    assert_eq!(reopened.stored_pixels(), 0);
}

#[test]
fn workspace_reader_streams_legacy_and_shared_cel_documents() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct WorkspaceDocument(#[serde(with = "pixelcraft::formats::recovery_document")] Document);

    let mut doc = Document::new(4, 4);
    doc.stamp(0, 0, 0, 0, [188, 40, 24, 255], 1);
    doc.add_frame(0, true);
    doc.link_cel(0, 1, 0).unwrap();

    let legacy = serde_json::to_vec(&doc).unwrap();
    let shared = serde_json::to_vec(&WorkspaceDocument(doc.clone())).unwrap();
    for bytes in [legacy, shared] {
        let WorkspaceDocument(restored) = serde_json::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(restored, doc);
        assert_eq!(
            restored.layers[0].cels[0].backing_id(),
            restored.layers[0].cels[1].backing_id()
        );
    }
}

#[test]
fn ase_rejects_legacy_tile_ids_that_alias_transform_bits() {
    use std::io::Write;
    let mut doc = Document::new(1, 1);
    doc.tilesets.push(Tileset {
        name: "Tile".into(),
        width: 1,
        height: 1,
        tiles: vec![vec![[255, 0, 0, 255]].into()],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0][0] = 1;
    let bytes = encode_ase(&doc).unwrap();
    let mut rebuilt = bytes[..144].to_vec();
    let mut cursor = 144;
    while cursor < bytes.len() {
        let size = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let mut chunk = bytes[cursor..cursor + size].to_vec();
        let kind = u16::from_le_bytes(chunk[4..6].try_into().unwrap());
        if kind == 0x2023 {
            chunk[10..14].copy_from_slice(&2u32.to_le_bytes());
        } else if kind == 0x2005 {
            chunk[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
            chunk[32..44].fill(0);
            chunk.truncate(54);
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            // A non-empty word using all bits as an ID and no flip masks.

            encoder.write_all(&0x2000_0000u32.to_le_bytes()).unwrap();
            chunk.extend(encoder.finish().unwrap());
            let new_size = chunk.len() as u32;
            chunk[..4].copy_from_slice(&new_size.to_le_bytes());
        }
        rebuilt.extend(chunk);
        cursor += size;
    }
    let size = rebuilt.len() as u32;
    rebuilt[..4].copy_from_slice(&size.to_le_bytes());
    rebuilt[128..132].copy_from_slice(&(size - 128).to_le_bytes());
    assert!(decode_ase(&rebuilt).is_err());
}
