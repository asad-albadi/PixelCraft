use pixelcraft::formats::*;
use pixelcraft::model::ColorMode;

#[test]
fn indexed_png_preserves_packed_slots_transparency_and_exact_scaled_export() {
    let dir = std::env::temp_dir().join(format!("pw-png-indexed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("indexed.png");
    let mut enc = png::Encoder::new(std::fs::File::create(&input).unwrap(), 5, 2);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Two);
    let palette = [9, 60, 21, 9, 60, 21, 14, 30, 80, 50, 80, 100];
    enc.set_palette(palette.to_vec());
    enc.set_trns(vec![255, 255, 0, 117]);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[0x1b, 0x40, 0xe4, 0x80]).unwrap();
    writer.finish().unwrap();

    let mut doc = load(&input).unwrap();
    let slots = [0, 1, 2, 3, 1, 3, 2, 1, 0, 2];
    assert_eq!(doc.color_mode, ColorMode::Indexed);
    assert_eq!(doc.transparent_index, 2);
    assert_eq!(doc.palette[2], [14, 30, 80, 0]);
    assert_eq!(doc.palette[3], [50, 80, 100, 117]);
    assert_eq!(
        doc.layers[0].cel_meta[0]
            .indices
            .as_deref()
            .unwrap()
            .as_slice(),
        slots
    );
    doc.set_palette_color(1, [77, 55, 66, 255]);
    assert_eq!(doc.composite(0)[0], [9, 60, 21, 255]);
    assert_eq!(doc.composite(0)[1], [77, 55, 66, 255]);

    let output = dir.join("scaled.png");
    export_raster_scaled(&output, &doc, 0, false, 2).unwrap();
    let again = load(&output).unwrap();
    assert_eq!(again.color_mode, ColorMode::Indexed);
    assert_eq!(again.palette, doc.palette);
    let expected: Vec<_> = (0..4)
        .flat_map(|y| (0..10).map(move |x| slots[y / 2 * 5 + x / 2]))
        .collect();
    assert_eq!(
        *again.layers[0].cel_meta[0].indices.as_deref().unwrap(),
        expected
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn grayscale_png_round_trips_luminance_and_transparency() {
    let dir = std::env::temp_dir().join(format!("pw-png-grayscale-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("gray.png");
    let mut enc = png::Encoder::new(std::fs::File::create(&input).unwrap(), 3, 1);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Four);
    enc.set_trns(vec![0, 5]);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[0x15, 0xf0]).unwrap();
    writer.finish().unwrap();

    let doc = load(&input).unwrap();
    assert_eq!(doc.color_mode, ColorMode::Grayscale);
    assert_eq!(
        doc.composite(0),
        [[17, 17, 17, 255], [85, 85, 85, 0], [255; 4]]
    );
    let output = dir.join("gray-out.png");
    export_png(&output, &doc, 0, false).unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&output).unwrap(),
    ))
    .read_info()
    .unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::GrayscaleAlpha);
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(&bytes[..info.buffer_size()], [17, 255, 85, 0, 255, 255]);
    drop(reader);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn png_text_profile_and_gamma_survive_open_save_export() {
    let root = std::env::temp_dir().join(format!("pw-png-metadata-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = std::fs::File::create(root.join("source.png")).unwrap();
    let mut enc = png::Encoder::new(file, 2, 2);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.add_text_chunk("Author".into(), "PixelCraft".into())
        .unwrap();
    enc.set_source_gamma(png::ScaledFloat::new(0.45455));
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[255u8; 16]).unwrap();
    writer.finish().unwrap();
    let doc = load(&root.join("source.png")).unwrap();
    assert!(!doc.metadata.is_empty());
    save(&root.join("project.pixel"), &doc).unwrap();
    let doc = load(&root.join("project.pixel")).unwrap();
    export_png(&root.join("out.png"), &doc, 0, false).unwrap();
    let reader = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(root.join("out.png")).unwrap(),
    ))
    .read_info()
    .unwrap();
    assert_eq!(reader.info().uncompressed_latin1_text[0].text, "PixelCraft");
    assert!(reader.info().gama_chunk.is_some());
    std::fs::remove_dir_all(root).unwrap();
}
