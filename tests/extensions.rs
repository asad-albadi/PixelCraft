use pixelcraft::{
    extensions::{Session, run_lua},
    model::Document,
};
use std::fs;

// Catches missing sprite/frame/layer context, lost results, and input mutation.
#[test]
fn original_lua_api_edits_the_selected_cel_and_round_trips() {
    let mut original = Document::new(3, 2);
    original.add_layer();
    original.add_frame(0, true);
    let session = Session::new(&original).unwrap();
    let script = session.directory.join("paint with spaces.lua");
    fs::write(
        &script,
        r#"
local cel = app.activeSprite:newCel(app.activeLayer, app.activeFrame)
cel.image:drawPixel(1, 0, app.pixelColor.rgba(12, 34, 56, 255))
"#,
    )
    .unwrap();
    let d = run_lua(&original, &script, 1, 1, None).unwrap();
    pixelcraft::formats::save(&session.result_path(), &d).unwrap();
    let edited = session.load_result().unwrap();
    assert_eq!(edited.layers[1].cels[1][1], [12, 34, 56, 255]);
    assert_eq!(edited.layers[0].cels[0][1], [0, 0, 0, 0]);
    assert_eq!(original.layers[1].cels[1][1], [0, 0, 0, 0]);
    fs::remove_dir_all(session.directory).unwrap();
}

#[test]
fn lua_errors_never_produce_an_importable_result() {
    let session = Session::new(&Document::new(2, 2)).unwrap();
    let script = session.directory.join("bad.lua");
    fs::write(&script, "error('extension failed deliberately')").unwrap();
    let error = run_lua(
        &pixelcraft::formats::load(&session.input_path()).unwrap(),
        &script,
        0,
        0,
        None,
    )
    .unwrap_err();
    assert!(error.contains("extension failed deliberately"), "{error}");
    assert!(session.load_result().is_err());
    fs::remove_dir_all(session.directory).unwrap();
}

#[test]
fn tagged_extension_results_are_kept_without_loss() {
    let session = Session::new(&Document::new(2, 2)).unwrap();
    let script = session.directory.join("tags.lua");
    fs::write(&script, "app.activeSprite:newTag(1, 1).name = 'Keep me'").unwrap();
    let d = run_lua(
        &pixelcraft::formats::load(&session.input_path()).unwrap(),
        &script,
        0,
        0,
        None,
    )
    .unwrap();
    pixelcraft::formats::save(&session.result_path(), &d).unwrap();
    assert_eq!(session.load_result().unwrap().tags[0].name, "Keep me");
    assert!(session.result_path().is_file());
    fs::remove_dir_all(session.directory).unwrap();
}

#[test]
fn palette_only_extensions_preserve_palette_edits() {
    let session = Session::new(&Document::new(2, 2)).unwrap();
    let script = session.directory.join("palette.lua");
    fs::write(
        &script,
        "app.activeSprite.palettes[1]:setColor(0, Color{r=12,g=34,b=56,a=255})",
    )
    .unwrap();
    let d = run_lua(
        &pixelcraft::formats::load(&session.input_path()).unwrap(),
        &script,
        0,
        0,
        None,
    )
    .unwrap();
    pixelcraft::formats::save(&session.result_path(), &d).unwrap();
    assert_eq!(session.load_result().unwrap().palette[0], [12, 34, 56, 255]);
    assert!(session.result_path().is_file());
    fs::remove_dir_all(session.directory).unwrap();
}
