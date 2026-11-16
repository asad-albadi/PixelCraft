use pixelcraft::{
    automation::{rpc, run_cli},
    formats,
    model::{Document, SliceKey, SpriteSlice},
};
use serde_json::{Value, json};
use std::path::Path;

fn export(input: &Path, output: &Path, options: Value) -> Result<Value, String> {
    run_cli(&[
        "export".into(),
        json!({"input":input,"output":output,"format":"sheet","options":options}).to_string(),
    ])
}

#[test]
fn cli_extrudes_sprite_edges_and_corners_outside_frame_bounds() {
    let root = std::env::temp_dir().join(format!("pixelcraft-extrusion-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("sprite.pixel");
    let output = root.join("sprite.png");
    let mut sprite = Document::new(2, 1);
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 128];
    sprite.set_pixel(0, 0, 0, red);
    sprite.set_pixel(0, 0, 1, green);
    formats::save(&input, &sprite).unwrap();

    export(&input, &output, json!({"padding":0,"extrude":1})).unwrap();

    let image = image::open(&output).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (4, 3));
    assert_eq!(image.get_pixel(0, 0).0, red);
    assert_eq!(image.get_pixel(3, 2).0, green);
    let meta: Value =
        serde_json::from_slice(&std::fs::read(output.with_extension("json")).unwrap()).unwrap();
    assert_eq!(meta["frames"][0]["frame"], json!({"x":1,"y":1,"w":2,"h":1}));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cli_exports_one_sheet_per_slice_using_each_frames_active_bounds() {
    let root = std::env::temp_dir().join(format!("pixelcraft-slices-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("sprite.pixel");
    let output = root.join("sheets");
    let mut sprite = Document::new(3, 1);
    sprite.add_frame(0, false);
    sprite.add_frame(1, false);
    sprite.durations = vec![70, 110, 130];
    sprite.set_frame_name(2, "Landing").unwrap();
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    sprite.set_pixel(0, 1, 1, red);
    sprite.set_pixel(0, 2, 2, green);
    let key = |frame, x| SliceKey {
        frame,
        bounds: [x, 0, 1, 1],
        center: None,
        pivot: None,
    };
    sprite.slices = vec![
        SpriteSlice {
            name: "Walk/right".into(),
            keys: vec![key(1, 1), key(2, 2)],
        },
        SpriteSlice {
            name: "Walk?right".into(),
            keys: vec![key(0, 0)],
        },
    ];
    formats::save(&input, &sprite).unwrap();

    let result = export(&input, &output, json!({"split_slices":true,"padding":0})).unwrap();

    let files = result["outputs"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    let first = Path::new(files[0].as_str().unwrap());
    let second = Path::new(files[1].as_str().unwrap());
    assert_ne!(first, second);
    assert_eq!(first.parent(), Some(output.as_path()));
    assert_eq!(second.parent(), Some(output.as_path()));
    let meta: Value =
        serde_json::from_slice(&std::fs::read(first.with_extension("json")).unwrap()).unwrap();
    assert_eq!(meta["frames"][0]["sourceFrame"], 1);
    assert_eq!(meta["frames"][1]["sourceFrame"], 2);
    assert_eq!(meta["frames"][1]["frameId"], 2);
    assert_eq!(meta["frames"][1]["frameName"], "Landing");
    let reopened = formats::load(&first.with_extension("json")).unwrap();
    assert_eq!(reopened.durations, [110, 130]);
    assert_eq!(reopened.composite(0), [red]);
    assert_eq!(reopened.composite(1), [green]);

    let mcp_output = root.join("mcp-sheets");
    std::fs::create_dir(&mcp_output).unwrap();
    let second_sidecar = mcp_output
        .join(second.file_name().unwrap())
        .with_extension("json");
    std::fs::write(&second_sidecar, b"previous metadata").unwrap();
    let response = rpc(
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
            "name":"export", "arguments":{"input":input,"output":mcp_output,"format":"sheet", "options":{"split_slices":true}}
        }}),
        None,
    )
    .unwrap();
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        std::fs::read(&second_sidecar).unwrap(),
        b"previous metadata"
    );
    assert!(!mcp_output.join(first.file_name().unwrap()).exists());
    std::fs::remove_dir_all(&root).unwrap();
}
