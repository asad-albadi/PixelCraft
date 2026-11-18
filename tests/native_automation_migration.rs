use pixelcraft::{automation, formats, model::*};
use serde_json::json;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn native_cli_help_does_not_advertise_retired_script_execution() {
    assert!(!automation::CLI_HELP.contains("CLI Lua"));
    assert!(!automation::CLI_HELP.contains("lua-console"));
}

#[test]
fn obsolete_script_requests_fail_without_writing_or_changing_artwork() {
    let root = std::env::temp_dir().join(format!("pixelcraft-obsolete-cli-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let input = root.join("source.pixel");
    let output = root.join("must-not-exist.pixel");
    let mut original = Document::new(3, 2);
    original.set_pixel(0, 0, 1, [12, 34, 56, 255]);
    formats::save(&input, &original).unwrap();
    let bytes = fs::read(&input).unwrap();
    for operation in ["lua", "lua-console", "run-lua", "run-script"] {
        let args = json!({"input":input,"output":output,"script":"error('must never execute')"});
        let result = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
            .args(["--cli", operation, &args.to_string()])
            .output()
            .unwrap();
        assert!(!result.status.success(), "{operation}");
        let mut mcp = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
            .arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(mcp.stdin.take().unwrap(),"{}",json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":operation,"arguments":args}})).unwrap();
        let reply: serde_json::Value =
            serde_json::from_slice(&mcp.wait_with_output().unwrap().stdout).unwrap();
        assert!(
            reply["result"]["isError"] == true || reply.get("error").is_some(),
            "{reply}"
        );
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_command_batches_target_selected_cels_and_reject_partial_results() {
    let mut original = Document::new(3, 2);
    original.add_layer();
    original.add_frame(0, false);
    let painted = automation::apply_command_batch(&original,&json!({"commands":[{"op":"shape","layer":1,"frame":1,"kind":"rectangle","x":1,"y":0,"x2":1,"y2":0,"color":[12,34,56,255],"filled":true}]})).unwrap();
    assert_eq!(painted.layers[1].cels[1][1], [12, 34, 56, 255]);
    assert_eq!(original.layers[1].cels[1][1], CLEAR);
    assert_eq!(painted.layers[0].cels[0][1], CLEAR);
    assert!(
        automation::apply_command_batch(
            &painted,
            &json!({"commands":[{"op":"add-layer","name":"discard"},{"op":"run-lua"}]})
        )
        .is_err()
    );
    assert_eq!(painted.layers.len(), 2);
}

#[test]
fn native_region_clear_preserves_offset_geometry_and_indexed_transparency() {
    let mut offset = Document::new(6, 6);
    offset.stamp(0, 0, 4, 4, [255, 0, 0, 255], 1);
    offset.layers[0].cel_meta[0].bounds = Some([4, 4, 1, 1]);
    let cleared = automation::apply_command_batch(&offset,&json!({"commands":[{"op":"clear-region","layer":0,"frame":0,"x":4,"y":4,"width":1,"height":1}]})).unwrap();
    assert_eq!(cleared.composite(0)[28], CLEAR);
    let mut indexed = Document::new(1, 1);
    indexed.palette = vec![[255, 0, 0, 255], CLEAR];
    indexed.transparent_index = 1;
    indexed.set_color_mode(ColorMode::Indexed);
    indexed.stamp(0, 0, 0, 0, [255, 0, 0, 255], 1);
    let cleared = automation::apply_command_batch(&indexed,&json!({"commands":[{"op":"clear-region","layer":0,"frame":0,"x":0,"y":0,"width":1,"height":1}]})).unwrap();
    assert_eq!(cleared.composite(0)[0], CLEAR);
}

#[test]
fn native_flip_and_recolor_replace_the_old_script_workflow_atomically() {
    let mut original = Document::new(3, 2);
    original.set_pixel(0, 0, 0, [10, 20, 30, 255]);
    let result=automation::apply_command_batch(&original,&json!({"commands":[
        {"op":"move-region","layer":0,"frame":0,"x":0,"y":0,"width":3,"height":2,"to_x":0,"to_y":0,"flip_x":true},
        {"op":"recolor-region","layer":0,"frame":0,"x":0,"y":0,"width":3,"height":2,"colors":[[[10,20,30,255],[200,100,50,255]]]}
    ]})).unwrap();
    assert_eq!(result.layers[0].cels[0][2], [200, 100, 50, 255]);
    assert_eq!(result.layers[0].cels[0][0], CLEAR);
    assert_eq!(original.layers[0].cels[0][0], [10, 20, 30, 255]);
}

#[test]
fn public_cli_and_mcp_native_edits_replace_selected_cel_scripts() {
    let root =
        std::env::temp_dir().join(format!("pixelcraft-native-target-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let input = root.join("input.pixel");
    let cli_output = root.join("cli.pixel");
    let mcp_output = root.join("mcp.pixel");
    let mut original = Document::new(3, 2);
    original.add_layer();
    original.add_frame(0, false);
    formats::save(&input, &original).unwrap();
    let arguments = json!({"input":input,"output":cli_output,"revision":automation::document_revision(&original),"commands":[{"op":"shape","layer":1,"frame":1,"kind":"line","x":1,"y":0,"x2":1,"y2":0,"color":[12,34,56,255]}]});
    let cli = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
        .args(["--cli", "commands", &arguments.to_string()])
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let mut arguments = arguments;
    arguments["output"] = json!(mcp_output);
    let mut mcp = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(mcp.stdin.take().unwrap(),"{}",json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"commands","arguments":arguments}})).unwrap();
    let reply: serde_json::Value =
        serde_json::from_slice(&mcp.wait_with_output().unwrap().stdout).unwrap();
    assert_ne!(reply["result"]["isError"], true, "{reply}");
    let cli = formats::load(&cli_output).unwrap();
    let mcp = formats::load(&mcp_output).unwrap();
    assert_eq!(cli, mcp);
    assert_eq!(cli.layers[1].cels[1][1], [12, 34, 56, 255]);
    assert_eq!(cli.layers[0].cels[0][1], CLEAR);
    assert_eq!(formats::load(&input).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
