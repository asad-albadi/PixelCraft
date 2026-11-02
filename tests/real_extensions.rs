use pixelcraft::{extensions::*, model::*};
use serde_json::json;
#[test]
fn unchanged_random_palette_and_pixels_scripts() {
    for (name, source) in [
        (
            "Random Palette",
            include_str!("fixtures/scripts/Random Palette.lua"),
        ),
        (
            "Random Pixels",
            include_str!("fixtures/scripts/Random Pixels.lua"),
        ),
    ] {
        for mode in [ColorMode::Rgba, ColorMode::Grayscale, ColorMode::Indexed] {
            let mut d = Document::new(4, 4);
            d.set_color_mode(mode);
            d.stamp(0, 0, 1, 1, [255, 0, 0, 255], 1);
            let out = run_source(&d, source, 0, 0, None)
                .unwrap_or_else(|e| panic!("{name} {mode:?}: {e}"));
            out.validate().unwrap();
            if name == "Random Palette" {
                assert_ne!(out.palette, d.palette);
            } else {
                assert_ne!(out.layers[0].cels[0], d.layers[0].cels[0]);
                assert_eq!(out.layers[0].cels[0][0][3], 0);
            }
        }
    }
}
#[test]
fn unchanged_move_cels_dialog_moves_pixels() {
    let mut d = Document::new(3, 2);
    d.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    let out = run_source_with_host(
        &d,
        include_str!("fixtures/scripts/Move Cels.lua"),
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(|r| match r["op"].as_str().unwrap() {
            "dialog" => Ok(json!({"widget":"ok","data":{"ok":true,"x":"1","y":"1"}})),
            "preferences" => Ok(json!({})),
            _ => panic!("{r}"),
        }),
    )
    .unwrap();
    assert_eq!(out.layers[0].cels[0][4], [255, 0, 0, 255]);
    assert_eq!(out.layers[0].cels[0][0], CLEAR);
}
#[test]
fn unchanged_wizard_chains_nonmodal_dialogs() {
    let mut calls = 0;
    run_source_with_host(&Document::new(1,1),include_str!("fixtures/scripts/Wizard Example.lua"),0,0,None,None,json!({}),Box::new(move |r|{
  if r["op"]=="pump"{calls+=1;assert!(calls<=3);let dialogs=r["dialogs"].as_array().unwrap();assert_eq!(dialogs.len(),1);let d=&dialogs[0];let w=d["widgets"].as_array().unwrap().last().unwrap();Ok(json!({"dialog":d["id"],"widget":w["id"],"data":{format!("data{calls}"):"hello"},"time":calls}))}else{Ok(json!({}))}
 })).unwrap();
}
