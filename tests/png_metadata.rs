use pixelcraft::formats::*;
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
