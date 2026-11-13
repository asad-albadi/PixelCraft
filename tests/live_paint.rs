use pixelcraft::{
    automation::{apply_commands, document_revision, rpc},
    model::Document,
};
use serde_json::json;

#[test]
fn raster_batch_clips_replaces_and_blends_atomically() {
    let mut doc = Document::new(3, 1);
    doc.set_pixel(0, 0, 1, [255, 0, 0, 255]);
    let mut args = json!({"revision": document_revision(&doc), "commands": [
        {"op":"blit","layer":0,"frame":0,"x":-1,"y":0,"width":3,"height":1,
         "mode":"replace","data":"/wAA/wD/AP8AAAAA"},
        {"op":"blit","layer":0,"frame":0,"x":0,"y":0,"width":1,"height":1,
         "mode":"source-over","data":"AAAAAA=="}
    ]});
    let next = apply_commands(&doc, &args).unwrap();
    assert_eq!(
        &next.layers[0].cels[0][..],
        &[[0, 255, 0, 255], [0, 0, 0, 0], [0, 0, 0, 0]]
    );
    args["commands"][1]["data"] = json!("AAAA");
    assert!(apply_commands(&doc, &args).is_err());
    assert_eq!(doc.layers[0].cels[0][1], [255, 0, 0, 255]);
}

#[test]
fn native_stamp_uses_the_requested_cel_and_points() {
    let mut doc = Document::new(5, 2);
    doc.add_layer();
    let next = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"stroke","layer":1,"frame":0,"points":[[0,0],[4,0]],"spacing":2,
             "color":[255,120,0,255],"size":1}
        ]}),
    )
    .unwrap();
    assert_eq!(
        &next.layers[1].cels[0][..5],
        &[
            [255, 120, 0, 255],
            [0; 4],
            [255, 120, 0, 255],
            [0; 4],
            [255, 120, 0, 255]
        ]
    );
    assert!(next.layers[0].cels[0].iter().all(|p| *p == [0; 4]));
}

#[test]
fn render_schema_describes_crop_and_layer_filter() {
    let result = rpc(
        &json!({"id":1,"method":"tools/list"}),
        Some(std::path::Path::new("bridge")),
    )
    .unwrap();
    let tools = result["result"]["tools"].as_array().unwrap();
    let render = tools.iter().find(|t| t["name"] == "live-render").unwrap();
    assert_eq!(
        render["inputSchema"]["properties"]["region"]["type"],
        "object"
    );
    assert_eq!(
        render["inputSchema"]["properties"]["layers"]["type"],
        "array"
    );
}

#[test]
fn rendered_crop_uses_selected_layers_and_exact_integer_zoom() {
    use pixelcraft::automation::operation;
    let path = std::env::temp_dir().join(format!("pixelcraft-crop-{}.pixel", std::process::id()));
    let mut doc = Document::new(2, 1);
    doc.set_pixel(0, 0, 1, [15, 30, 45, 255]);
    doc.add_layer();
    doc.set_pixel(1, 0, 1, [255; 4]);
    pixelcraft::formats::save(&path, &doc).unwrap();
    let result = operation(
        "render",
        &json!({"input":path,"frame":0,"scale":4,
        "layers":[0],"region":{"x":1,"y":0,"width":1,"height":1}}),
    );
    std::fs::remove_file(path).unwrap();
    let value = result.unwrap();
    assert_eq!(value["width"], 4);
    assert_eq!(value["height"], 4);
    assert_eq!(value["layers"], json!([0]));
    assert_eq!(value["region"], json!({"x":1,"y":0,"width":1,"height":1}));
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value["data"].as_str().unwrap())
        .unwrap();
    let png = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert!(png.pixels().all(|p| p.0 == [15, 30, 45, 255]));
}

#[test]
fn render_to_file_honors_the_crop() {
    use pixelcraft::automation::operation;
    let root = std::env::temp_dir();
    let source = root.join(format!(
        "pixelcraft-file-render-{}.pixel",
        std::process::id()
    ));
    let target = source.with_extension("png");
    let mut doc = Document::new(2, 1);
    doc.set_pixel(0, 0, 1, [15, 30, 45, 255]);
    pixelcraft::formats::save(&source, &doc).unwrap();
    let result = operation(
        "render",
        &json!({"input":source,"output":target,
        "region":{"x":1,"y":0,"width":1,"height":1}}),
    );
    std::fs::remove_file(source).unwrap();
    result.unwrap();
    let png = image::open(&target).unwrap().into_rgba8();
    std::fs::remove_file(target).unwrap();
    assert_eq!(png.dimensions(), (1, 1));
    assert_eq!(png.get_pixel(0, 0).0, [15, 30, 45, 255]);
}

#[test]
fn seeded_native_placements_replay_and_preserve_other_layers() {
    let mut doc = Document::new(12, 12);
    doc.add_layer();
    let args = json!({"revision":document_revision(&doc),"commands":[{
        "op":"stroke","layer":1,"frame":0,"points":[[4,4],[6,5],[8,7]],
        "placements":true,"scatter":2,"seed":912,"size":2,
        "color":[0,0,0,255],"ramp":[[180,34,9,255],[230,110,13,255]]
    }]});
    let first = apply_commands(&doc, &args).unwrap();
    let second = apply_commands(&doc, &args).unwrap();
    assert_eq!(first, second);
    assert!(first.layers[0].cels[0].shares_storage(&doc.layers[0].cels[0]));
    assert!(first.layers[1].cels[0].iter().any(|p| p[3] == 255));
    assert!(
        first.layers[1].cels[0]
            .iter()
            .all(|p| [[0; 4], [180, 34, 9, 255], [230, 110, 13, 255]].contains(p))
    );
}

#[test]
fn region_move_handles_overlap_and_keeps_source_order() {
    let mut doc = Document::new(4, 1);
    doc.set_pixel(0, 0, 0, [10, 0, 0, 255]);
    doc.set_pixel(0, 0, 1, [20, 0, 0, 255]);
    let next = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"move-region","layer":0,"frame":0,"x":0,"y":0,"width":2,"height":1,
             "to_x":1,"to_y":0,"flip_x":true}
        ]}),
    )
    .unwrap();
    assert_eq!(
        &next.layers[0].cels[0][..],
        &[[0; 4], [20, 0, 0, 255], [10, 0, 0, 255], [0; 4]]
    );
}

#[test]
fn native_polygon_dither_keeps_hard_edges() {
    let doc = Document::new(5, 5);
    let next = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"polygon","layer":0,"frame":0,"points":[[0,0],[4,0],[0,4]],
             "color":[10,20,30,255],"alternate":[40,50,60,255]}
        ]}),
    )
    .unwrap();
    assert_eq!(next.layers[0].cels[0][6], [10, 20, 30, 255]);
    assert_eq!(next.layers[0].cels[0][1], [40, 50, 60, 255]);
    assert_eq!(next.layers[0].cels[0][18], [0; 4]);
}

#[test]
fn bitmap_stroke_composites_without_erasing_under_transparent_stamp_pixels() {
    let mut doc = Document::new(2, 1);
    doc.layers[0].cels[0].fill([0, 0, 255, 255]);
    let next = apply_commands(&doc, &json!({"revision":document_revision(&doc),"commands":[
        {"op":"stroke","layer":0,"frame":0,"points":[[1,0]],"color":[0,0,0,0],
         "mode":"source-over","opacity":128,"stamp":{"width":2,"height":1,"data":"AAAAAP8AAP8="}}
    ]})).unwrap();
    assert_eq!(
        &next.layers[0].cels[0][..],
        &[[0, 0, 255, 255], [128, 0, 127, 255]]
    );
}

#[test]
fn compressed_patch_requires_a_complete_stream_and_exact_decoded_length() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::io::Write;
    let doc = Document::new(2, 1);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&[2, 3, 4, 255, 5, 6, 7, 255]).unwrap();
    let packed = encoder.finish().unwrap();
    let mut args = json!({"revision":document_revision(&doc),"commands":[
        {"op":"blit","layer":0,"frame":0,"x":0,"y":0,"width":2,"height":1,
         "encoding":"base64-zlib","data":STANDARD.encode(&packed)}
    ]});
    assert_eq!(
        &apply_commands(&doc, &args).unwrap().layers[0].cels[0][..],
        &[[2, 3, 4, 255], [5, 6, 7, 255]]
    );
    args["commands"][0]["data"] = json!(STANDARD.encode(&packed[..packed.len() - 1]));
    assert!(apply_commands(&doc, &args).is_err());
    args["commands"][0]["data"] = json!(STANDARD.encode(&packed));
    args["commands"][0]["width"] = json!(1);
    assert!(apply_commands(&doc, &args).is_err());
}

#[test]
fn region_mask_follows_the_source_pixel_when_flipped() {
    let mut doc = Document::new(4, 1);
    doc.set_pixel(0, 0, 0, [10, 0, 0, 255]);
    doc.set_pixel(0, 0, 1, [20, 0, 0, 255]);
    let next = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"move-region","layer":0,"frame":0,"x":0,"y":0,"width":2,"height":1,
             "to_x":2,"to_y":0,"flip_x":true,"mask":"AQ=="}
        ]}),
    )
    .unwrap();
    assert_eq!(
        &next.layers[0].cels[0][..],
        &[[0; 4], [20, 0, 0, 255], [0; 4], [10, 0, 0, 255]]
    );
}

#[test]
fn overlapping_shade_dabs_step_once_per_stroke() {
    let mut doc = Document::new(1, 1);
    doc.palette = vec![[10, 0, 0, 255], [20, 0, 0, 255], [30, 0, 0, 255]];
    doc.set_pixel(0, 0, 0, [10, 0, 0, 255]);
    let next = apply_commands(
        &doc,
        &json!({"revision":document_revision(&doc),"commands":[
            {"op":"stroke","layer":0,"frame":0,"points":[[0,0],[0,0]],"placements":true,
             "shade":true,"color":[0,0,0,255]}
        ]}),
    )
    .unwrap();
    assert_eq!(next.layers[0].cels[0][0], [20, 0, 0, 255]);
}
