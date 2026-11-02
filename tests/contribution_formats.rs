use pixelcraft::extensions::Manager;
use std::fs;
#[test]
fn original_theme_directory_language_and_dither_manifests_install() {
    let root = std::env::temp_dir().join(format!("pixelcraft-formats-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("source/dark")).unwrap();
    fs::write(root.join("source/package.json"),r#"{"name":"original-formats","contributes":{"themes":[{"id":"dark","path":"./dark"}],"languages":[{"id":"fr","path":"./fr.ini"}],"ditheringMatrices":[{"id":"test","path":"./matrix.bmp"}]}}"#).unwrap();
    fs::write(
        root.join("source/dark/theme.xml"),
        "<theme><colors><color id=\"face\" value=\"#123456\"/></colors></theme>",
    )
    .unwrap();
    fs::write(root.join("source/fr.ini"), "[menu]\nfile = &Fichier\n").unwrap();
    fs::write(root.join("source/matrix.bmp"), b"BM").unwrap();
    let manager = Manager::at(root.join("data")).unwrap();
    manager.install(&root.join("source")).unwrap();
    assert_eq!(manager.list().unwrap().len(), 1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn xml_colors_shortcuts_ini_and_bmp_thresholds_are_read() {
    use pixelcraft::{contributions::*, model::*};
    let theme=theme(br##"<theme><colors><color id="face" value="#123456"/><color id="text" value="#eeeeee"/><color id="selected" value="#abcdef"/></colors></theme>"##).unwrap();
    assert_eq!(theme["panel"], serde_json::json!([18, 52, 86]));
    assert_eq!(theme["dark"], true);
    let keys=shortcuts(br#"<keyboard><commands><key command="SaveFile" shortcut="Ctrl+S"/><key command="Unavailable" shortcut="A"/></commands><tools><key tool="pencil" shortcut="B"/></tools></keyboard>"#).unwrap();
    assert_eq!(keys.bindings["Ctrl+S"], "save");
    assert_eq!(keys.bindings["B"], "pencil");
    assert_eq!(keys.unsupported.len(), 1);
    assert_eq!(
        language(b"[menu]\nfile = &Fichier\n;ignored\n").unwrap()["menu.file"],
        "Fichier"
    );
    let path = std::env::temp_dir().join(format!("matrix-{}.bmp", std::process::id()));
    image::GrayImage::from_raw(2, 2, vec![0, 2, 3, 1])
        .unwrap()
        .save(&path)
        .unwrap();
    let matrix = DitheringMatrix::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let palette = vec![[0, 0, 0, 255], [255, 255, 255, 255]];
    assert_ne!(
        matrix.index([128, 128, 128, 255], &palette, 0, 0),
        matrix.index([128, 128, 128, 255], &palette, 0, 1)
    );
    let mut d = Document::new(2, 2);
    d.layers[0].background = true;
    d.palette = palette;
    d.dithering = Some(matrix);
    d.layers[0].cels[0] = vec![[128, 128, 128, 255]; 4].into();
    d.set_color_mode(ColorMode::Indexed);
    assert!(d.layers[0].cels[0].contains(&[0, 0, 0, 255]));
    assert!(d.layers[0].cels[0].contains(&[255, 255, 255, 255]));
}
