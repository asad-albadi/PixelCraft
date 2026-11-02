use pixelcraft::{extensions::*, model::*};
use std::fs;
#[test]
fn lua_empty_frame_inserts_at_requested_slot() {
    let mut d = Document::new(1, 1);
    d.add_frame(0, true);
    d.add_frame(1, true);
    d.durations = vec![110, 220, 330];
    for (f, red) in [10, 20, 30].into_iter().enumerate() {
        d.layers[0].cels[f][0] = [red, 0, 0, 255];
    }
    for arg in ["2", "app.activeSprite.frames[2]"] {
        let code = format!(
            "local f=app.activeSprite:newEmptyFrame({arg}); assert(f.frameNumber==2, 'empty insertion slot'); assert(f.duration==0.110, 'preceding duration')"
        );
        let edited = run_source(&d, &code, 0, 0, None).unwrap();
        assert_eq!(edited.durations, [110, 110, 220, 330]);
        assert_eq!(
            edited.layers[0]
                .cels
                .iter()
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            [[10, 0, 0, 255], CLEAR, [20, 0, 0, 255], [30, 0, 0, 255]]
        );
    }
    let first = run_source(&d, "app.activeSprite:newEmptyFrame(1)", 0, 0, None).unwrap();
    assert_eq!(first.durations, [110, 110, 220, 330]);
    assert_eq!(first.layers[0].cels[0][0], CLEAR);
    assert_eq!(first.layers[0].cels[1][0], [10, 0, 0, 255]);
    let last = run_source(
        &d,
        "local f=app.activeSprite:newEmptyFrame(); assert(f.frameNumber==4)",
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(last.durations, [110, 220, 330, 330]);
    assert_eq!(last.layers[0].cels[3][0], CLEAR);
}

#[test]
fn lua_new_frame_inserts_after_argument_and_returns_source_slot() {
    // Native Sprite_newFrame inserts at source+1, but pushes source's index.
    let mut d = Document::new(1, 1);
    d.add_frame(0, true);
    d.add_frame(1, true);
    d.durations = vec![110, 220, 330];
    for (f, red) in [10, 20, 30].into_iter().enumerate() {
        d.layers[0].cels[f][0] = [red, 0, 0, 255];
    }
    for arg in ["1", "app.activeSprite.frames[1]"] {
        let code = format!(
            "local s=app.activeSprite; local f=s:newFrame({arg}); assert(f.frameNumber==1, 'returned source slot'); f.duration=0.444; s.layers[1]:cel(2).image:drawPixel(0,0,Color{{r=99}})"
        );
        let edited = run_source(&d, &code, 0, 0, None).unwrap();
        assert_eq!(edited.durations, [444, 110, 220, 330]);
        assert_eq!(
            edited.layers[0]
                .cels
                .iter()
                .map(|p| p[0][0])
                .collect::<Vec<_>>(),
            [10, 99, 20, 30]
        );
        assert_eq!(d.durations, [110, 220, 330]);
    }
}

#[test]
fn lua_draw_image_uses_native_integer_normal_blend() {
    // Literal results derived from rgba_blender_normal's byte arithmetic.
    for (dst, src, opacity, expected) in [
        (
            [10, 20, 30, 100],
            [200, 100, 0, 128],
            172,
            [117, 65, 14, 152],
        ),
        ([40, 50, 60, 0], [10, 20, 30, 0], 255, [10, 20, 30, 0]),
        ([40, 50, 60, 0], [10, 20, 30, 128], 0, [10, 20, 30, 0]),
        ([40, 50, 60, 100], [10, 20, 30, 0], 255, [40, 50, 60, 100]),
        // The image loop skips the packed zero mask color before blending.
        ([40, 50, 60, 0], CLEAR, 255, [40, 50, 60, 0]),
    ] {
        let mut d = Document::new(1, 1);
        d.layers[0].cels[0][0] = dst;
        let code = format!(
            "local src=Image(1,1); src:drawPixel(0,0,app.pixelColor.rgba({},{},{},{})); app.activeImage:drawImage(src,Point(),{})",
            src[0], src[1], src[2], src[3], opacity
        );
        let edited = run_source(&d, &code, 0, 0, None).unwrap();
        assert_eq!(
            edited.layers[0].cels[0][0], expected,
            "src={src:?}, dst={dst:?}, opacity={opacity}"
        );
        assert_eq!(d.layers[0].cels[0][0], dst);
    }
}

#[test]
fn embedded_lua_edits_selected_cel_and_registers_plugin_commands() {
    let original = Document::new(3, 2);
    let dir = std::env::temp_dir().join(format!("pw-native-lua-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("paint.lua");
    fs::write(
        &script,
        r#"
function init(plugin)
 plugin:newCommand{id="paint",title="Paint pixel",onclick=function()
   local cel=app.activeSprite:newCel(app.activeLayer,app.activeFrame)
   cel.image:drawPixel(1,0,app.pixelColor.rgba(12,34,56,255))
   app.activeSprite:newTag(1,1).name="Idle"
 end}
end
"#,
    )
    .unwrap();
    let edited = run_lua(&original, &script, 0, 0, Some("paint")).unwrap();
    assert_eq!(edited.layers[0].cels[0][1], [12, 34, 56, 255]);
    assert_eq!(edited.tags[0].name, "Idle");
    assert_eq!(original.composite(0)[1], CLEAR);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn script_errors_and_timeouts_preserve_input() {
    let d = Document::new(2, 2);
    for code in [
        "app.activeImage:drawPixel(0,0,Color{r=255}); error('failed deliberately')",
        "while true do end",
    ] {
        let e = run_source(&d, code, 0, 0, None).unwrap_err();
        assert!(
            e.contains("failed deliberately") || e.contains("budget"),
            "{e}"
        );
        assert_eq!(d.composite(0)[0], CLEAR);
    }
}
#[test]
fn extensions_install_disable_and_uninstall_inside_own_directory() {
    let root = std::env::temp_dir().join(format!("pw-manager-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("package.json"),r#"{"name":"test-package","displayName":"Test package","contributes":{"scripts":[{"path":"main.lua"}],"palettes":[{"path":"colors.json"}]}}"#).unwrap();
    fs::write(
        source.join("main.lua"),
        "function init(p) p:newCommand{id='paint',title='Paint',onclick=function() end} end",
    )
    .unwrap();
    fs::write(source.join("colors.json"), "[[255,0,0,255]]").unwrap();
    let manager = Manager::at(root.join("own-data")).unwrap();
    let id = manager.install(&source).unwrap();
    assert_eq!(manager.list().unwrap().len(), 1);
    assert_eq!(manager.commands().unwrap()[0].id, "paint");
    manager.set_enabled(&id, false).unwrap();
    assert!(manager.commands().unwrap().is_empty());
    manager.set_enabled(&id, true).unwrap();
    manager.uninstall(&id).unwrap();
    assert!(manager.list().unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn manifest_cannot_escape_its_package() {
    let root = std::env::temp_dir().join(format!("pw-bad-package-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"escape","contributes":{"scripts":[{"path":"../outside.lua"}]}}"#,
    )
    .unwrap();
    let manager = Manager::at(root.join("data")).unwrap();
    assert!(manager.install(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unsupported_apis_report_an_error_without_changing_artwork() {
    let d = Document::new(2, 2);
    let e = run_source(&d, "app.command.UnsupportedCommand()", 0, 0, None).unwrap_err();
    assert!(e.contains("Unsupported command"));
    assert_eq!(d.composite(0), vec![CLEAR; 4]);
}
#[test]
fn coroutine_and_caught_errors_cannot_bypass_script_budget() {
    let d = Document::new(1, 1);
    for code in [
        "coroutine.resume(coroutine.create(function() while true do end end))",
        "error=function() end; while true do pcall(function() for i=1,100000 do end end) end",
        "assert(_pw_finish==nil); while true do end",
    ] {
        assert!(run_source(&d, code, 0, 0, None).is_err());
    }
}
#[test]
fn native_lua_preserves_indices_alpha_and_root_group_assignment() {
    let mut d = Document::new(2, 1);
    d.palette = vec![CLEAR, [255, 0, 0, 255], [255, 0, 0, 255]];
    d.set_color_mode(ColorMode::Indexed);
    d.layers[0].cels[0][0] = [255, 0, 0, 255];
    d.layers[0].cels[0][1] = [255, 0, 0, 255];
    d.layers[0].cel_meta[0].indices = Some(vec![1, 2].into());
    let edited=run_source(&d,"app.activeSprite.palettes[1]:setColor(2,Color{r=0,g=255,b=0,a=255}); app.activeImage:drawImage(Image(1,1)); app.activeLayer.parent=app.activeSprite",0,0,None).unwrap();
    assert_eq!(
        edited.composite(0),
        vec![[255, 0, 0, 255], [0, 255, 0, 255]]
    );
}
