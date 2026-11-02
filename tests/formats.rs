use pixelcraft::{formats::*, model::*};
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
    let d = Document::new(2, 2);
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
