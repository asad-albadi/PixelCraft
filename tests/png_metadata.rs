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
fn indexed_apng_preserves_slots_through_partial_frames_and_disposal() {
    let dir = std::env::temp_dir().join(format!("pw-apng-indexed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("animated.apng");
    let mut enc = png::Encoder::new(std::fs::File::create(&path).unwrap(), 3, 1);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Two);
    enc.set_palette(vec![10, 20, 45, 10, 20, 45, 150, 70, 10, 10, 130, 100]);
    enc.set_trns(vec![255, 255, 128, 0]);
    enc.set_animated(4, 0).unwrap();
    enc.set_sep_def_img(true).unwrap();
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[0b10_10_10_00]).unwrap(); // Poster, outside the timeline.
    writer.set_frame_delay(5, 100).unwrap();
    writer.write_image_data(&[0b00_01_00_00]).unwrap();
    writer.set_frame_delay(7, 100).unwrap();
    writer.set_frame_dimension(1, 1).unwrap();
    writer.set_frame_position(1, 0).unwrap();
    writer.set_dispose_op(png::DisposeOp::Previous).unwrap();
    writer.write_image_data(&[0b10_00_00_00]).unwrap();
    writer.set_frame_position(0, 0).unwrap();
    writer.set_frame_delay(9, 100).unwrap();
    writer.set_dispose_op(png::DisposeOp::Background).unwrap();
    writer.write_image_data(&[0b01_00_00_00]).unwrap();
    writer.set_frame_position(2, 0).unwrap();
    writer.set_dispose_op(png::DisposeOp::None).unwrap();
    writer.set_blend_op(png::BlendOp::Over).unwrap();
    writer.set_frame_delay(11, 100).unwrap();
    writer.write_image_data(&[0b10_00_00_00]).unwrap();
    writer.finish().unwrap();

    let doc = load(&path).unwrap();
    assert_eq!(doc.color_mode, ColorMode::Indexed);
    assert_eq!(doc.durations, [50, 70, 90, 110]);
    let slots: Vec<_> = doc.layers[0]
        .cel_meta
        .iter()
        .map(|cel| cel.indices.as_deref().unwrap().as_slice())
        .collect();
    assert_eq!(slots, [&[0, 1, 0], &[0, 2, 0], &[1, 1, 0], &[3, 1, 4]]);
    assert_eq!(doc.palette[0], doc.palette[1]);
    assert_eq!(doc.palette.len(), 5);
    let output = dir.join("last.png");
    export_png(&output, &doc, 3, false).unwrap();
    assert_eq!(load(&output).unwrap().composite(0), doc.composite(3));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn indexed_apng_falls_back_to_rgba_when_a_blend_cannot_fit() {
    let dir = std::env::temp_dir().join(format!("pw-apng-blend-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("blended.png");
    let mut enc = png::Encoder::new(std::fs::File::create(&path).unwrap(), 1, 1);
    enc.set_color(png::ColorType::Indexed);
    let mut palette = [10; 768];
    palette[..6].copy_from_slice(&[0, 0, 0, 200, 0, 0]);
    enc.set_palette(palette.to_vec());
    enc.set_trns(vec![255, 128]);
    enc.set_animated(2, 0).unwrap();
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[0]).unwrap();
    writer.set_blend_op(png::BlendOp::Over).unwrap();
    writer.write_image_data(&[1]).unwrap();
    writer.finish().unwrap();

    let doc = load(&path).unwrap();
    assert_eq!(doc.color_mode, ColorMode::Rgba);
    assert_eq!(doc.composite(0), [[0, 0, 0, 255]]);
    // Match the existing RGBA decoder's rounding on the fallback path.
    assert_eq!(doc.composite(1), [[100, 0, 0, 254]]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn grayscale_apng_retains_mode_and_frame_durations() {
    let dir = std::env::temp_dir().join(format!("pw-apng-gray-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("animated.png");
    let mut enc = png::Encoder::new(std::fs::File::create(&path).unwrap(), 2, 1);
    enc.set_color(png::ColorType::GrayscaleAlpha);
    enc.set_source_gamma(png::ScaledFloat::new(0.45455));
    enc.set_animated(2, 0).unwrap();
    let mut writer = enc.write_header().unwrap();
    writer.set_frame_delay(12, 100).unwrap();
    writer.write_image_data(&[20, 255, 50, 120]).unwrap();
    writer.set_frame_delay(18, 100).unwrap();
    writer.write_image_data(&[30, 255, 70, 60]).unwrap();
    writer.finish().unwrap();
    let doc = load(&path).unwrap();
    assert_eq!(doc.color_mode, ColorMode::Grayscale);
    assert_eq!(doc.durations, [120, 180]);
    assert!(
        doc.metadata
            .iter()
            .any(|entry| entry.data.get(4..8) == Some(b"gAMA"))
    );
    assert_eq!(doc.composite(0), [[20, 20, 20, 255], [50, 50, 50, 120]]);
    assert_eq!(doc.composite(1), [[30, 30, 30, 255], [70, 70, 70, 60]]);
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
