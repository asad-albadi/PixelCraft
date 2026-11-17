use pixelcraft::{extensions::*, model::*};
use std::fs;
#[test]
fn headless_lua_passes_params_and_writes_only_a_successful_result() {
    use pixelcraft::{automation, formats};
    use serde_json::json;
    let folder =
        std::env::temp_dir().join(format!("pixelcraft-headless-script-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let script = folder.join("paint.lua");
    let input = folder.join("input.pixel");
    let output = folder.join("output.pixel");
    let _ = fs::remove_file(&output);
    formats::save(&input, &Document::new(2, 1)).unwrap();
    fs::write(
        &script,
        r#"
        assert(not app.isUIAvailable and io == nil and os.execute == nil)
        assert(app.params.label == 'quote: "safe"' and app.params.paint)
        print(app.params.label, app.params.rgb[1])
        app.activeImage:drawPixel(1, 0, Color{r=app.params.rgb[1],g=42,b=24})
    "#,
    )
    .unwrap();
    let args = json!({
        "input":input, "script":script, "output":output,
        "params":{"label":r#"quote: "safe""#,"paint":true,"rgb":[160]},
    });
    let run = |args: &serde_json::Value| automation::run_cli(&["lua".into(), args.to_string()]);
    let result = run(&args).unwrap();
    assert_eq!(result["stdout"], concat!(r#"quote: "safe""#, "\t160\n"));
    assert_eq!(
        formats::load(&output).unwrap().layers[0].cels[0][1],
        [160, 42, 24, 255]
    );
    assert_eq!(formats::load(&input).unwrap().layers[0].cels[0][1], CLEAR);
    let saved = fs::read(&output).unwrap();
    assert!(run(&args).unwrap_err().contains("overwrite"));

    fs::write(
        &script,
        "app.activeImage:drawPixel(1, 0, Color{r=23}); error('script failed')",
    )
    .unwrap();
    let mut replace = args;
    replace["overwrite"] = json!(true);
    assert!(run(&replace).unwrap_err().contains("script failed"));
    assert_eq!(fs::read(&output).unwrap(), saved);
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn headless_lua_console_keeps_state_and_saves_on_request() {
    use pixelcraft::formats;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let folder =
        std::env::temp_dir().join(format!("pixelcraft-lua-console-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let output = folder.join("console.pixel");
    let _ = fs::remove_file(&output);

    let run = |width: &str, height: &str, commands: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pixelcraft"))
            .args([
                "--cli",
                "lua-console",
                "--width",
                width,
                "--height",
                height,
                "--output",
            ])
            .arg(&output)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(commands.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let result = run(
        "2",
        "1",
        r#"
        assert(not app.isUIAvailable and io == nil)
        value = 41
        function paint(n)
            app.activeImage:drawPixel(1, 0, Color{r=n,g=32,b=16})
            return n
        end
        error("ordinary error")
        while true do end
        =paint(value + 1)
        :save
    "#,
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .lines()
            .any(|line| line == "42")
    );
    let errors = String::from_utf8_lossy(&result.stderr);
    assert!(errors.contains("ordinary error"));
    assert!(errors.contains("Script instruction/time budget exceeded"));
    assert_eq!(
        formats::load(&output).unwrap().layers[0].cels[0][1],
        [42, 32, 16, 255]
    );

    fs::remove_file(&output).unwrap();
    let result = run("2", "1", "value = 23\n:quit\n");
    assert!(result.status.success());
    assert!(!output.exists());
    let result = run("2", "1", "function unfinished()\n");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Incomplete Lua command"));
    assert!(!output.exists());
    let result = run("1000", "1000", ":save\n");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = formats::load(&output).unwrap();
    assert_eq!((saved.width, saved.height), (1000, 1000));
    fs::remove_dir_all(folder).unwrap();
}

#[test]
fn lua_gray_assignment_preserves_signed_state_and_alpha() {
    // Saturating gray before packing loses the native low byte.
    for (value, gray, expected) in [
        ("-1.2", -2, [254, 254, 254, 2]),
        ("'258.9'", 258, [2, 2, 2, 2]),
        ("0x100000063", 99, [99, 99, 99, 2]),
        ("false", 0, [0, 0, 0, 2]),
    ] {
        let source = format!(
            r#"
            local c=Color{{r=10,g=20,b=30,a=258}}
            c.gray={value}
            assert(c.gray=={gray} and c.alpha==258)
            local p=c.grayPixel
            app.activeImage:drawPixel(0,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        "#
        );
        let edited = run_source(&Document::new(1, 1), &source, 0, 0, None).unwrap();
        assert_eq!(edited.layers[0].cels[0][0], expected, "{value}");
    }
}

#[test]
fn lua_gray_assignment_then_rgb_edit_recomputes_gray() {
    // An RGB setter reconstructs an RGB Color, discarding prior gray state.
    let edited = run_source(
        &Document::new(1, 1),
        r#"
        local c=Color{r=10,g=20,b=30,a=99}
        c.gray=40
        c.alpha=88
        c.red=200
        local p=c.grayPixel
        app.activeImage:drawPixel(0,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(edited.layers[0].cels[0][0], [120, 120, 120, 88]);
}

#[test]
fn lua_rgb_gray_uses_native_hsl_lightness() {
    // Luma weighting would make the three primary colors different shades.
    let edited = run_source(
        &Document::new(3, 1),
        r#"
        for x,c in ipairs({Color{r=255},Color{r=0,g=255},Color{r=0,b=255}}) do
            app.activeImage:drawPixel(x-1,0,app.pixelColor.rgba(c.gray,c.gray,c.gray))
        end
        "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        edited.layers[0].cels[0],
        vec![[127, 127, 127, 255]; 3].into()
    );
}

#[test]
fn lua_rgb_gray_pixel_converts_color_and_masks_alpha() {
    // Packing red directly or clamping alpha corrupts these painted pixels.
    let edited = run_source(
        &Document::new(2, 1),
        r#"
        local c=Color{r=0,g=255,b=0,a=258}
        local function paint(x)
            local p=c.grayPixel
            app.activeImage:drawPixel(x,0,app.pixelColor.rgba(p&255,p&255,p&255,(p>>8)&255))
        end
        paint(0)
        c.red=10;c.green=20;c.blue=30;c.alpha=99
        paint(1)
        assert(not pcall(function()c.grayPixel=123 end))
        "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        edited.layers[0].cels[0],
        vec![[127, 127, 127, 2], [20, 20, 20, 99]].into()
    );
}

#[test]
fn lua_rgb_constructor_preserves_native_integer_channels() {
    let d = Document::new(1, 1);
    let edited = run_source(
        &d,
        r#"
        local c=Color{red=-1.2,green='258.9',blue=0x100000063,alpha=258}
        assert(c.red==-2 and c.green==258 and c.blue==99 and c.alpha==258)
        local clone=Color(c)
        c.red=17
        assert(clone.red==-2 and clone.alpha==258)
        app.activeImage:drawPixel(0,0,clone.rgbaPixel)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(edited.layers[0].cels[0][0], [254, 2, 99, 2]);
}

#[test]
fn lua_rgb_constructor_uses_one_field_family() {
    let d = Document::new(1, 1);
    for (constructor, expected) in [
        ("{r=1,green=2,blue=3,alpha=4}", [1, 0, 0, 255]),
        ("{red=1,g=2,b=3,a=4}", [1, 0, 0, 255]),
        ("{r=false,red=99,g='bad',b=true,a=false}", [0, 0, 0, 0]),
        ("{red=1,green=2,blue=3,alpha='bad'}", [1, 2, 3, 0]),
        ("{r=1,g=2,b=3,a=-1}", [1, 2, 3, 255]),
    ] {
        let source =
            format!("local c=Color{constructor}; app.activeImage:drawPixel(0,0,c.rgbaPixel)");
        let edited = run_source(&d, &source, 0, 0, None).unwrap();
        assert_eq!(edited.layers[0].cels[0][0], expected, "{constructor}");
    }
}

#[test]
fn lua_color_rgba_pixel_reflects_red_mutation() {
    // A cached packed value paints the old red channel after a Color edit.
    let d = Document::new(1, 1);
    let edited = run_source(
        &d,
        "local c=Color{r=1,g=2,b=3,a=4}; c.red=99; app.activeImage:drawPixel(0,0,c.rgbaPixel)",
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(edited.layers[0].cels[0][0], [99, 2, 3, 4]);
}

#[test]
fn lua_color_channel_edits_feed_packed_pixels() {
    let d = Document::new(1, 1);
    for (edit, expected) in [
        ("c.green=88; c.blue=77; c.alpha=66", [1, 88, 77, 66]),
        (
            "c.red=-1; c.green=256; c.blue='258.9'; c.alpha=300",
            [255, 0, 2, 255],
        ),
        ("c.red=0x100000063; c.alpha=-1", [99, 2, 3, 0]),
        (
            "c.red=false; c.green='invalid'; c.alpha=false",
            [0, 0, 3, 0],
        ),
    ] {
        let edited=run_source(&d, &format!("local c=Color{{r=1,g=2,b=3,a=4}}; {edit}; app.activeImage:drawPixel(0,0,c.rgbaPixel)"),0,0,None).unwrap();
        assert_eq!(edited.layers[0].cels[0][0], expected, "{edit}");
    }
}

#[test]
fn lua_factory_colors_paint_white_foreground_and_black_background() {
    let d = Document::new(2, 1);
    let edited = run_source(
        &d,
        r#"
        local fg = app.fgColor
        fg.r = 17
        app.activeImage:drawPixel(0,0,app.fgColor)
        app.activeImage:drawPixel(1,0,app.bgColor)
    "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(edited.layers[0].cels[0][0], [255, 255, 255, 255]);
    assert_eq!(edited.layers[0].cels[0][1], [0, 0, 0, 255]);
}

#[test]
fn lua_pixel_color_wraps_channels_and_converts_like_native() {
    // Catch saturation in packing and missing integer conversion in extractors.
    // Expected pixels follow the original's low-byte masks, independently of
    // our implementation. Exercise the real Lua runtime and document import.
    let d = Document::new(1, 1);
    for (args, expected) in [
        ("-1,256,511,258", [255, 0, 255, 2]),
        ("'257.9',-0.1,258.9", [1, 255, 2, 255]),
        ("false,{},'invalid',false", [0, 0, 0, 0]),
        ("math.huge,-math.huge,0/0,nil", [0, 0, 0, 255]),
        ("0x7fffffffffffffff,0x100000000,0x101,0x100", [255, 0, 1, 0]),
        ("", [0, 0, 0, 255]),
    ] {
        let edited = run_source(
            &d,
            &format!("app.activeImage:drawPixel(0,0,app.pixelColor.rgba({args}))"),
            0,
            0,
            None,
        )
        .unwrap();
        assert_eq!(edited.layers[0].cels[0][0], expected, "rgba({args})");
    }
    run_source(
        &d,
        r#"
        local p=app.pixelColor
        assert(p.rgba(1,2,3,4)==0x04030201)
        assert(p.rgbaR('257.9')==1)
        assert(p.rgbaG('65535.9')==255)
        assert(p.rgbaB(-1.1)==255)
        assert(p.rgbaA(-1)==255)
        for _,f in ipairs{p.rgbaR,p.rgbaG,p.rgbaB,p.rgbaA} do
            assert(f('invalid')==0)
            assert(f(false)==0)
            assert(f({})==0)
            assert(f(math.huge)==0)
            assert(f()==0)
        end
    "#,
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn lua_opacity_failed_numeric_conversion_becomes_zero() {
    // Native lua_tointegerx uses floor mode, with a zero-initialized result
    // for failed conversion. Catch clamping infinity to opaque or raising
    // on values the native setter accepts as zero.
    let d = Document::new(1, 1);
    for field in ["app.activeLayer.opacity", "app.activeCel.opacity"] {
        for (value, opacity) in [
            ("math.huge", 0),
            ("-math.huge", 0),
            ("0/0", 0),
            ("'invalid'", 0),
            ("false", 0),
            ("{}", 0),
            ("nil", 0),
            ("'123.9'", 123),
            ("123.9", 123),
            ("255", 255),
        ] {
            let edited = run_source(&d, &format!("{field}={value}"), 0, 0, None).unwrap();
            let actual = if field == "app.activeLayer.opacity" {
                edited.layers[0].opacity
            } else {
                edited.layers[0].cel_meta[0].opacity
            };
            assert_eq!(actual, opacity, "{field}={value}");
        }
    }
}

#[test]
fn lua_duration_uses_native_integer_conversion_before_clamping() {
    let d = Document::new(1, 1);
    // Literal expectations from the x86_64 setter's CVTTSD2SI then 1..65535 clamp.
    for (seconds, millis) in [
        ("0.1239", 123),
        ("-0.1239", 1),
        ("0", 1),
        ("65.535", 65535),
        ("100", 65535),
        ("2147483.647", 65535),
        ("2147483.648", 1),
        ("math.huge", 1),
        ("-math.huge", 1),
        ("0/0", 1),
    ] {
        let edited = run_source(
            &d,
            &format!("app.activeFrame.duration={seconds}"),
            0,
            0,
            None,
        )
        .unwrap();
        assert_eq!(edited.durations, [millis], "seconds={seconds}");
    }
}

#[test]
fn lua_frame_commands_insert_after_active_frame() {
    // Catch command handlers accidentally using the sprite methods' append default.
    let mut d = Document::new(1, 1);
    d.add_frame(0, true);
    d.add_frame(1, true);
    d.durations = vec![110, 220, 330];
    for (f, red) in [10, 20, 30].into_iter().enumerate() {
        d.layers[0].cels[f][0] = [red, 0, 0, 255];
    }
    for (command, inserted) in [("NewFrame", [20, 0, 0, 255]), ("NewEmptyFrame", CLEAR)] {
        let edited = run_source(&d, &format!("app.command.{command}()"), 1, 0, None).unwrap();
        assert_eq!(edited.durations, [110, 220, 220, 330], "{command}");
        assert_eq!(
            edited.layers[0]
                .cels
                .iter()
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            [[10, 0, 0, 255], [20, 0, 0, 255], inserted, [30, 0, 0, 255]],
            "{command}"
        );
        // PixelCraft keeps its command convention of selecting the new slot.
        let painted = run_source(
            &d,
            &format!("app.command.{command}(); app.activeImage:drawPixel(0,0,Color{{r=99}})"),
            1,
            0,
            None,
        )
        .unwrap();
        assert_eq!(painted.layers[0].cels[2][0], [99, 0, 0, 255], "{command}");
        assert_eq!(painted.layers[0].cels[3][0], [30, 0, 0, 255], "{command}");
    }
}

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

#[test]
fn lua_selected_colors_paint_independent_rgba_copies() {
    let d = Document::new(2, 1);
    let edited = run_source_with_colors(
        &d,
        r#"
        local fg = app.fgColor
        fg.r = 99
        app.activeImage:drawPixel(0,0,app.fgColor)
        local bg = app.bgColor
        bg.a = 0
        app.activeImage:drawPixel(1,0,app.bgColor)
    "#,
        0,
        0,
        None,
        Some([[12, 34, 56, 78], [90, 87, 65, 43]]),
    )
    .unwrap();
    assert_eq!(edited.layers[0].cels[0][0], [12, 34, 56, 78]);
    assert_eq!(edited.layers[0].cels[0][1], [90, 87, 65, 43]);
}

#[test]
fn lua_edits_animated_slices_and_round_trips_their_keys() {
    let mut input = Document::new(8, 8);
    input.add_frame(0, false);
    input.slices.push(SpriteSlice {
        name: "Retired".into(),
        keys: Vec::new(),
    });
    input.metadata = vec![
        MetadataChunk {
            frame: 0,
            owner: "slice:0".into(),
            kind: 0x2020,
            data: vec![0; 4],
        },
        MetadataChunk {
            frame: 0,
            owner: "slice:1".into(),
            kind: 0x2020,
            data: vec![2, 0, 0, 0, 35, 50, 65, 255],
        },
    ];
    input.slices.push(SpriteSlice {
        name: "Old hitbox".into(),
        keys: vec![SliceKey {
            frame: 1,
            bounds: [2, 3, 4, 5],
            center: None,
            pivot: Some([2, 1]),
        }],
    });
    let output = run_source(
        &input,
        r#"
 local sprite=app.activeSprite
 sprite:deleteSlice(sprite.slices[1])
 local existing=sprite.slices[1]
 assert(existing==sprite.slices[1] and existing.sprite==sprite)
 assert(existing.bounds==Rectangle(2,3,4,5) and existing:keyAt(1)==nil)
 assert(existing:keyAt(sprite.frames[2]).pivot==Point(2,1))
 existing.name='Hitbox'
 existing.bounds=Rectangle(1,1,3,4)
 existing.center=Rectangle(1,1,1,2)
 existing.pivot=Point(-1,2)
 sprite:newEmptyFrame(2)
 assert(existing:keyAt(2).bounds==Rectangle(1,1,3,4))
 assert(existing:keyAt(3).bounds==Rectangle(2,3,4,5))
 existing:setKey(2,Rectangle(-1,2,5,4),nil,Point(3,2))
 assert(existing:keyAt(2).pivot==Point(3,2))
 assert(existing:deleteKey(2) and not existing:deleteKey(2))
 assert(not pcall(function() existing:setKey(4,Rectangle(0,0,1,1)) end))
 assert(not pcall(function() existing.bounds=Rectangle(0,0,-1,3) end))
 local temporary=sprite:newSlice(0,0,2,2)
 temporary.name='Temporary'
 sprite:deleteSlice('Temporary')
 assert(not pcall(function() return temporary.bounds end))
 local trim=sprite:newSlice(Rectangle(3,2,5,4))
 trim.name='Trim'
 trim.center=Rectangle(1,0,2,2)
 trim.pivot=Point(0,0)
 trim:setKey(2,Rectangle(3,3,5,4))
 assert(trim:keyAt(2).pivot==nil)
 assert(#sprite.slices==2)
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(output.slices[0].name, "Hitbox");
    assert_eq!(
        output.slices[0].keys,
        vec![
            SliceKey {
                frame: 0,
                bounds: [1, 1, 3, 4],
                center: Some([1, 1, 1, 2]),
                pivot: Some([-1, 2])
            },
            SliceKey {
                frame: 2,
                bounds: [2, 3, 4, 5],
                center: None,
                pivot: Some([2, 1])
            },
        ]
    );
    assert_eq!(output.slices[1].keys[0].center, Some([1, 0, 2, 2]));
    assert_eq!(output.slices[1].keys[0].pivot, Some([0, 0]));
    assert_eq!(output.slices[1].keys[1].pivot, None);
    assert_eq!(output.metadata.len(), 1);
    assert_eq!(output.metadata[0].owner, "slice:0");
    assert_eq!(output.metadata[0].data, input.metadata[1].data);
    let dir = std::env::temp_dir().join(format!("pixelcraft-lua-slices-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for extension in ["pixel", "aseprite"] {
        let path = dir.join("edited").with_extension(extension);
        pixelcraft::formats::save(&path, &output).unwrap();
        let reopened = pixelcraft::formats::load(&path).unwrap();
        assert_eq!(reopened.slices, output.slices);
        assert_eq!(reopened.durations, output.durations);
        assert_eq!(reopened.metadata, output.metadata);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn lua_edits_tilesets_and_remaps_placements_in_every_frame() {
    let mut input = Document::new(4, 2);
    input.add_frame(0, false);
    input.tilesets.push(Tileset {
        name: "unused".into(),
        width: 1,
        height: 1,
        tiles: vec![],
    });
    input.tilesets.push(Tileset {
        name: "terrain".into(),
        width: 2,
        height: 2,
        tiles: vec![
            vec![[255, 0, 0, 255]; 4].into(),
            vec![[0, 20, 80, 255]; 4].into(),
        ],
    });
    input.add_tile_layer(1).unwrap();
    input.layers[1].tile_cels = vec![vec![0x8000_0001, 0x4000_0002], vec![2, 1]];
    input.add_tile_layer(1).unwrap();
    input.layers[2].tile_cels[1] = vec![0x2000_0002, 0];
    for i in 0_u8..4 {
        input.metadata.push(MetadataChunk {
            frame: 0,
            kind: 0x2020,
            owner: "tileset:1".into(),
            data: vec![2, 0, 0, 0, i, 30, 40, 255],
        });
    }
    let output = run_source(
        &input,
        r#"
 local sprite=app.activeSprite
 local set=sprite.tilesets[2]
 assert(set==sprite.layers[2].tileset and set==sprite.tilesets[2])
 assert(#set==3 and set.grid.tileSize==Size(2,2))
 assert(not pcall(function() sprite:deleteTileset(set) end))
 assert(not pcall(function() sprite:deleteTile(set,0) end))
 sprite:deleteTileset(sprite.tilesets[1])
 set.name='Ground'
 set.baseIndex=12
 local old=set:tile(2)
 assert(old==set:tile(2) and old.index==2)
 local inserted=sprite:newTile(set,1)
 inserted.image:drawPixel(0,0,Color('#115599'))
 assert(set:getTile(1):getPixel(0,0)==app.pixelColor.rgba(17,85,153,255))
 assert(old.index==3)
 local removed=set:tile(2)
 local heldImage=removed.image
 sprite:deleteTile(removed)
 assert(not pcall(function() return removed.index end))
 assert(not pcall(function() heldImage:drawPixel(0,0,Color('#deadbe')) end))
 assert(old.index==2)
 old.image:drawPixel(1,0,Color('#aabbcc'))
 local other=sprite:newTileset(Rectangle(0,0,2,2),2)
 local layer=sprite:newTilemap(other)
 assert(layer.tileset==other and layer.isTilemap and #other==2)
 local clone=sprite:newTileset(set)
 assert(clone~=set and #clone==#set and clone.baseIndex==12)
 layer.tileset=clone
 sprite:deleteTileset(other)
 clone:tile(1).image:drawPixel(0,0,Color('#00ffaa'))
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(output.tilesets[0].name, "Ground");
    assert_eq!(output.layers[1].tileset, Some(0));
    assert_eq!(
        output.layers[1].tile_cels,
        vec![vec![0, 0x4000_0002], vec![2, 0]]
    );
    assert_eq!(output.layers[2].tile_cels[1], vec![0x2000_0002, 0]);
    assert_eq!(output.layers[3].tileset, Some(1));
    assert_eq!(output.layers[3].tile_cels, vec![vec![0; 2]; 2]);
    assert_eq!(output.tilesets[1].tiles[0][0], [0, 255, 170, 255]);
    assert_eq!(output.tilesets[0].tiles[0][0], [17, 85, 153, 255]);
    assert_eq!(output.tilesets[0].tiles[1][1], [170, 187, 204, 255]);
    assert_eq!(output.ase_tilesets[0].base_index, 12);
    assert_eq!(output.metadata.len(), 8);
    assert_eq!(
        output
            .metadata
            .iter()
            .filter(|item| item.owner == "tileset:0")
            .map(|item| item.data.clone())
            .collect::<Vec<_>>(),
        vec![
            input.metadata[0].data.clone(),
            input.metadata[1].data.clone(),
            vec![0; 4],
            input.metadata[3].data.clone()
        ]
    );
    assert_eq!(
        output
            .metadata
            .iter()
            .filter(|item| item.owner == "tileset:0")
            .map(|item| &item.data)
            .collect::<Vec<_>>(),
        output
            .metadata
            .iter()
            .filter(|item| item.owner == "tileset:1")
            .map(|item| &item.data)
            .collect::<Vec<_>>()
    );
    let dir = std::env::temp_dir().join(format!("pixelcraft-lua-tilesets-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for ext in ["pixel", "aseprite"] {
        let path = dir.join("tiles").with_extension(ext);
        pixelcraft::formats::save(&path, &output).unwrap();
        let reopened = pixelcraft::formats::load(&path).unwrap();
        assert_eq!(reopened.tilesets, output.tilesets);
        assert_eq!(reopened.layers[1].tile_cels, output.layers[1].tile_cels);
        assert_eq!(reopened.metadata, output.metadata);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn headless_lua_clipboard_is_local_and_cut_preserves_selection_holes() {
    let mut input = Document::new(4, 1);
    input.layers[0].cels[0].set(0, [100, 80, 60, 255]);
    input.layers[0].cels[0].set(1, [7, 8, 9, 255]);
    input.layers[0].cels[0].set(2, [60, 80, 100, 255]);
    let output = run_source(
        &input,
        r#"
 assert(not app.clipboard.hasImage)
 app.activeSprite.selection=Selection(Rectangle(0,0,3,1))
 app.activeSprite.selection:subtract(Rectangle(1,0,1,1))
 app.command.Cut()
 assert(app.clipboard.hasImage and app.clipboard.content.image.width==3)
 assert(app.clipboard.image:getPixel(1,0)==0)
 app.command.Paste{position=Point(1,0)}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        output.composite(0),
        vec![CLEAR, [100, 80, 60, 255], CLEAR, [60, 80, 100, 255]]
    );
    run_source(&input, "assert(not app.clipboard.hasImage)", 0, 0, None).unwrap();
}

#[test]
fn headless_lua_clipboard_text_and_clear_replace_previous_content() {
    let doc = Document::new(2, 1);
    run_source(
        &doc,
        r#"
 local clipboard=app.clipboard
 assert(not clipboard.hasText and clipboard.text==nil)
 clipboard.text='UTF-8: café 🌿'..string.char(10)..'next line'
 assert(clipboard.hasText and clipboard.content.text==clipboard.text)
 assert(not clipboard.hasImage)
 assert(not pcall(function() clipboard.content={text='ambiguous',image=Image(1,1)} end))
 assert(not pcall(function() clipboard.text='one'..string.char(0)..'two' end))
 assert(clipboard.hasText)
 clipboard.image=Image(1,1)
 assert(not clipboard.hasText and clipboard.hasImage)
 clipboard:clear()
 assert(not clipboard.hasText and not clipboard.hasImage)
 clipboard.content={text='second'}
 assert(clipboard.text=='second')
 clipboard.content={}
 assert(clipboard.text==nil)
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    run_source(
        &doc,
        "assert(not app.clipboard.hasImage and not app.clipboard.hasText)",
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn lua_clipboard_preserves_detached_selections_and_palettes() {
    let mut doc = Document::new(4, 1);
    doc.layers[0].cels[0].set(2, [8, 16, 24, 255]);
    let output = run_source(
        &doc,
        r#"
 local im=Image(3,1,ColorMode.RGB)
 im:clear(Color('#aabbcc'))
 local p=Palette(2)
 p:setColor(1,Color('#123456'))
 local mask=Selection(Rectangle(10,4,3,1))
 mask:subtract(Rectangle(11,4,1,1))
 app.clipboard.content={image=im,selection=mask,palette=p}
 im:clear()
 mask:deselect()
 p:setColor(1,Color('#ffffff'))
 local saved=app.clipboard.content
 assert(saved.selection:contains(10,4) and saved.selection:contains(12,4))
 assert(not saved.selection:contains(11,4))
 assert(saved.image:getPixel(1,0)==app.pixelColor.rgba(170,187,204,255))
 assert(#saved.palette==2 and saved.palette:getColor(1)==Color('#123456'))
 saved.palette:setColor(1,Color('#000000'))
 assert(app.clipboard.content.palette:getColor(1)==Color('#123456'))
 app.command.Paste{position=Point(1,0)}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        output.composite(0),
        vec![
            CLEAR,
            [170, 187, 204, 255],
            [8, 16, 24, 255],
            [170, 187, 204, 255]
        ]
    );
}

#[test]
fn lua_clipboard_tilesets_survive_source_deletion_and_import_independently() {
    let mut doc = Document::new(4, 2);
    doc.tilesets.push(Tileset {
        name: "shore".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![[20, 40, 60, 255]; 4].into()],
    });
    let text_color = [3, 0, b's', b'e', b'a', 4, 5, 6, 255];
    let mut user_data = vec![3, 0, 0, 0];
    user_data.extend(text_color);
    doc.metadata.push(pixelcraft::model::MetadataChunk {
        frame: 0,
        kind: 0x2020,
        owner: "tileset:0".into(),
        data: user_data,
    });
    let output = run_source(&doc, r#"
 local sprite=app.activeSprite
 local source=sprite.tilesets[1]
 source.baseIndex=7
 source.properties.terrain='sand'
 app.clipboard.content={image=Image(1,1),tileset=source}
 source:tile(1).image:drawPixel(0,0,Color('#ffffff'))
 sprite:deleteTileset(source)
 local saved=app.clipboard.content.tileset
 assert(saved.name=='shore' and #saved==2 and saved.baseIndex==7)
 assert(saved:tile(1).image:getPixel(0,0)==app.pixelColor.rgba(20,40,60,255))
 saved:tile(1).image:drawPixel(0,0,Color('#221144'))
 assert(app.clipboard.content.tileset:tile(1).image:getPixel(0,0)==app.pixelColor.rgba(20,40,60,255))
 local imported=sprite:newTileset(saved)
 assert(imported~=saved and imported.baseIndex==7)
 assert(imported.properties.terrain=='sand')
 saved.name='local change'
 assert(imported.name=='shore')
 assert(not pcall(function() sprite:newTilemap(saved) end))
 sprite:newTilemap(imported)
 "#,0,0,None).unwrap();
    assert_eq!(output.tilesets[0].tiles[0][0], [34, 17, 68, 255]);
    assert_eq!(output.tilesets[0].tiles[0][1], [20, 40, 60, 255]);
    let user_data = output
        .metadata
        .iter()
        .find(|chunk| chunk.owner == "tileset:0" && chunk.kind == 0x2020)
        .unwrap();
    assert_eq!(&user_data.data[4..13], &text_color);
    assert_eq!(output.ase_tilesets[0].base_index, 7);
    assert_eq!(output.layers[1].tileset, Some(0));
}

#[test]
fn lua_native_tools_target_cels_and_respect_selection_and_undo() {
    let mut doc = Document::new(6, 3);
    doc.add_frame(0, false);
    doc.add_layer();
    let output = run_source(&doc, r#"
 local sprite=app.activeSprite
 local original=app.activeCel
 local target=sprite.layers[2]:cel(2)
 assert(not app.isUIAvailable and _pw_host==nil)
 sprite.selection=Selection(Rectangle(1,0,4,3))
 sprite.selection:subtract(Rectangle(3,1,1,1))
 app.useTool{tool='pencil',cel=target,color=Color('#5599dd'),points={Point(0,1),Point(5,1)}}
 assert(app.activeCel.layer==original.layer and app.activeCel.frame==original.frame)
 assert(target.image:getPixel(1,1)==app.pixelColor.rgba(85,153,221,255))
 app.command.Undo()
 assert(sprite.layers[2]:cel(2).image:getPixel(1,1)==0)
 app.command.Redo()
 assert(sprite.layers[2]:cel(2).image:getPixel(1,1)==app.pixelColor.rgba(85,153,221,255))
 local layer=sprite.layers[2]
 layer.isEditable=false
 assert(not pcall(function() app.useTool{tool='eraser',layer=layer,frame=2,points={Point(1,1)}} end))
 -- Refresh the layer handle if the failed transaction restored a snapshot.
 sprite.layers[2].isEditable=true
 app.useTool{tool='filled_rectangle',layer=sprite.layers[2],frame=2,
  color=Color('#aa4422'),points={Point(1,0),Point(2,0)}}
 app.useTool{tool='eyedropper',layer=sprite.layers[2],frame=2,points={Point(1,0)}}
 assert(app.fgColor==Color('#aa4422'))
 assert(not pcall(function() app.useTool{tool='pencil',points={Point(1000000000,0)}} end))
 "#,0,0,None).unwrap();
    let cel = &output.layers[1].cels[1];
    assert_eq!(cel[1], [170, 68, 34, 255]);
    assert_eq!(cel[2], [170, 68, 34, 255]);
    assert_eq!(cel[6], CLEAR);
    assert_eq!(cel[7], [85, 153, 221, 255]);
    assert_eq!(cel[8], [85, 153, 221, 255]);
    assert_eq!(cel[9], CLEAR);
    assert_eq!(cel[10], [85, 153, 221, 255]);
    assert_eq!(cel[11], CLEAR);
    assert_eq!(output.layers[0].cels[0][1], CLEAR);
}

#[test]
fn lua_native_brush_applies_opacity_once_and_fill_uses_native_color_matching() {
    let mut doc = Document::new(6, 4);
    for (index, color) in [
        (18, [70, 60, 50, 255]),
        (22, [72, 59, 50, 255]),
        (23, [85, 60, 50, 255]),
    ] {
        doc.layers[0].cels[0].set(index, color);
    }
    let output = run_source(
        &doc,
        r#"
 app.bgColor=Color('#5080b0')
 app.useTool{button=2,brush=Brush{type=BrushType.SQUARE,size=2},opacity=128,
  points={Point(1,1),Point(2,1),Point(1,1)}}
 app.useTool{tool='paint_bucket',color=Color('#c86432'),tolerance=3,
  contiguous=false,points={Point(0,3)}}
 app.useTool{tool='eraser',opacity=128,points={Point(1,1)}}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let pixels = &output.layers[0].cels[0];
    assert_eq!(pixels[0], [80, 128, 176, 128]);
    assert_eq!(pixels[7], [80, 128, 176, 64]);
    assert_eq!(pixels[8], [80, 128, 176, 128]);
    assert_eq!(pixels[9], CLEAR);
    assert_eq!(pixels[18], [200, 100, 50, 255]);
    assert_eq!(pixels[22], [200, 100, 50, 255]);
    assert_eq!(pixels[23], [85, 60, 50, 255]);
}

#[test]
fn lua_tilemap_strokes_use_native_placements_and_tileset_modes() {
    let mut doc = Document::new(6, 2);
    doc.tilesets.push(Tileset {
        name: "Terrain".into(),
        width: 2,
        height: 2,
        tiles: vec![
            vec![[30, 60, 90, 255]; 4].into(),
            vec![[90, 60, 30, 255]; 4].into(),
        ],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0] = vec![1, 1, 2];
    doc.add_frame(0, true);
    doc.link_cel(1, 1, 0).unwrap();
    assert!(doc.layers[1].cel_meta[0].artwork_link.is_some());
    let output = run_source(&doc, r#"
 local sprite=app.activeSprite
 local layer=sprite.layers[2]
 app.fgTile=0x80000002
 sprite.selection=Selection(Rectangle(1,0,1,1))
 app.useTool{tool='paint_bucket',layer=layer,frame=2,points={Point(0,0)},tilemapMode=TilemapMode.TILES}
 sprite.selection:deselect()
 app.bgTile=0x40000001
 app.useTool{tool='pencil',button=2,layer=layer,frame=2,points={Point(4,0)},tilemapMode=TilemapMode.TILES}
 app.useTool{tool='eyedropper',layer=layer,frame=2,points={Point(4,0)},tilemapMode=TilemapMode.TILES}
 assert(app.fgTile==0x40000001)
 app.useTool{tool='eraser',layer=layer,frame=2,points={Point(2,0)},tilemapMode=TilemapMode.TILES}
 app.useTool{tool='pencil',layer=layer,frame=2,points={Point(0,0)},
  tilemapMode=TilemapMode.PIXELS,tilesetMode=TilesetMode.MANUAL,color=Color('#446688')}
 app.useTool{tool='pencil',layer=layer,frame=2,points={Point(4,0),Point(5,0)},
  tilemapMode=TilemapMode.PIXELS,tilesetMode=TilesetMode.STACK,color=Color('#882244')}
 assert(#sprite.tilesets[1]==4)
 app.command.Undo()
 assert(#sprite.tilesets[1]==3)
 app.command.Redo()
 assert(#sprite.tilesets[1]==4)
 "#,0,0,None).unwrap();
    assert_eq!(
        output.layers[1].tile_cels[0],
        vec![0x8000_0002, 0, 0x4000_0001]
    );
    assert_eq!(
        output.layers[1].tile_cels[1],
        vec![0x8000_0002, 0, 0x4000_0003]
    );
    let set = &output.tilesets[0];
    assert_eq!(set.tiles.len(), 3);
    assert_eq!(set.tiles[0][2], [30, 60, 90, 255]);
    assert_eq!(set.tiles[1][0], [90, 60, 30, 255]);
    assert_eq!(set.tiles[1][1], [68, 102, 136, 255]);
    assert_eq!(set.tiles[2][2], [136, 34, 68, 255]);
    assert_eq!(set.tiles[2][3], [136, 34, 68, 255]);
}

#[test]
fn lua_tilemap_auto_keeps_placement_edits_when_an_existing_tile_matches() {
    let mut doc = Document::new(2, 1);
    doc.tilesets.push(Tileset {
        name: "Variants".into(),
        width: 2,
        height: 1,
        tiles: vec![
            vec![[40, 50, 60, 255]; 2].into(),
            vec![[10, 20, 30, 255], [40, 50, 60, 255]].into(),
        ],
    });
    doc.add_tile_layer(0).unwrap();
    doc.layers[1].tile_cels[0] = vec![1];
    let output = run_source(
        &doc,
        r#"
 local layer=app.activeSprite.layers[2]
 app.useTool{layer=layer,tool='pencil',points={Point(0,0)},color=Color('#0a141e'),
   tilemapMode=TilemapMode.PIXELS,tilesetMode=TilesetMode.AUTO}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(output.layers[1].tile_cels[0], vec![2]);
    assert_eq!(output.tilesets, doc.tilesets);
}

#[test]
fn lua_native_selection_tools_combine_masks_and_undo_independently() {
    let mut doc = Document::new(6, 6);
    doc.layers[0].cels[0].set(0, [11, 22, 33, 255]);
    doc.layers[0].cels[0].set(35, [12, 22, 33, 255]);
    let output = run_source(&doc, r#"
 local sprite=app.activeSprite
 app.useTool{tool='magic_wand',points={Point(0,0)},tolerance=1,contiguous=false}
 assert(sprite.selection:contains(0,0) and sprite.selection:contains(5,5))
 app.useTool{tool='rectangular_marquee',points={Point(1,1),Point(3,3)},selection=SelectionMode.ADD}
 assert(sprite.selection:contains(2,2) and sprite.selection:contains(5,5))
 app.useTool{tool='rectangular_marquee',points={Point(2,2)},selection=SelectionMode.SUBTRACT}
 assert(not sprite.selection:contains(2,2))
 app.command.Undo()
 assert(sprite.selection:contains(2,2))
 app.command.Redo()
 assert(not sprite.selection:contains(2,2))
 app.useTool{tool='elliptical_marquee',points={Point(0,0),Point(5,5)},selection=SelectionMode.INTERSECT}
 assert(sprite.selection:contains(3,3) and not sprite.selection:contains(0,0))
 app.useTool{tool='polygonal_lasso',points={Point(0,0),Point(4,0),Point(0,4)}}
 assert(sprite.selection:contains(1,1) and not sprite.selection:contains(3,3))
 app.useTool{tool='pencil',points={Point(0,1),Point(5,1)},color=Color('#ff4400')}
 "#,0,0,None).unwrap();
    assert_eq!(output.layers[0].cels[0][6], [255, 68, 0, 255]);
    assert_eq!(output.layers[0].cels[0][7], [255, 68, 0, 255]);
    assert_eq!(output.layers[0].cels[0][8], CLEAR);
    assert_eq!(output.layers[0].cels[0][35], [12, 22, 33, 255]);
}

#[test]
fn lua_freehand_modes_keep_distinct_paths_and_preserve_existing_corners() {
    let mut doc = Document::new(7, 5);
    doc.layers[0].cels[0].set(7 + 2, [10, 20, 30, 255]);
    let output = run_source(
        &doc,
        r#"
 local red=Color('#dd5500')
 app.useTool{tool='pencil',points={Point(1,1),Point(2,1),Point(2,2)},
  freehandAlgorithm=FreehandAlgorithm.PIXEL_PERFECT,color=red}
 app.useTool{tool='pencil',points={Point(4,1),Point(5,1),Point(5,2)},
  freehandAlgorithm=FreehandAlgorithm.REGULAR,color=red}
 app.useTool{tool='pencil',points={Point(0,3),Point(6,3)},
  freehandAlgorithm=FreehandAlgorithm.DOTS,color=red,opacity=128}
 app.useTool{tool='pencil',points={Point(0,4),Point(6,4)},color=red}
 app.useTool{tool='eraser',points={Point(0,4),Point(6,4)},
  freehandAlgorithm=FreehandAlgorithm.DOTS}
 assert(not pcall(function()
  app.useTool{tool='pencil',points={Point(0,0)},freehandAlgorithm=99}
 end))
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let pixels = &output.layers[0].cels[0];
    let red = [221, 85, 0, 255];
    assert_eq!(pixels[8], red);
    assert_eq!(pixels[9], [10, 20, 30, 255]);
    assert_eq!(pixels[16], red);
    assert_eq!(pixels[12], red);
    assert_eq!(pixels[21], [221, 85, 0, 128]);
    assert_eq!(pixels[24], CLEAR);
    assert_eq!(pixels[27], [221, 85, 0, 128]);
    assert_eq!(pixels[28], CLEAR);
    assert_eq!(pixels[31], red);
    assert_eq!(pixels[34], CLEAR);
}

#[test]
fn lua_freehand_tile_strokes_use_cell_corners_and_supplied_dots() {
    let mut doc = Document::new(6, 6);
    doc.tilesets.push(Tileset {
        name: "Tiles".into(),
        width: 2,
        height: 2,
        tiles: vec![vec![[1, 2, 3, 255]; 4].into()],
    });
    doc.add_tile_layer(0).unwrap();
    let output = run_source(
        &doc,
        r#"
 local layer=app.activeSprite.layers[2]
 app.useTool{layer=layer,tool='pencil',points={Point(0,0),Point(2,0),Point(2,2)},
  freehandAlgorithm=FreehandAlgorithm.PIXEL_PERFECT}
 app.useTool{layer=layer,tool='pencil',points={Point(0,4),Point(4,4)},
  freehandAlgorithm=FreehandAlgorithm.DOTS}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        output.layers[1].tile_cels[0],
        vec![1, 0, 0, 0, 1, 0, 1, 0, 1]
    );
}

#[test]
fn lua_sprite_properties_roundtrip_without_rewriting_unedited_ase_values() {
    fn property(name: &str, kind: u16, bytes: &[u8]) -> Vec<u8> {
        let mut entry = (name.len() as u16).to_le_bytes().to_vec();
        entry.extend_from_slice(name.as_bytes());
        entry.extend_from_slice(&kind.to_le_bytes());
        entry.extend_from_slice(bytes);
        entry
    }
    let namespaced = property("u64", 9, &u64::MAX.to_le_bytes());
    let untouched = property("small", 2, &[42]);
    let mut maps = 2_u32.to_le_bytes().to_vec();
    for (id, entries) in [
        (7_u32, vec![namespaced.clone()]),
        (0, vec![untouched.clone()]),
    ] {
        maps.extend_from_slice(&id.to_le_bytes());
        maps.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for entry in entries {
            maps.extend(entry);
        }
    }
    let mut data = 7_u32.to_le_bytes().to_vec();
    data.extend_from_slice(&6_u16.to_le_bytes());
    data.extend_from_slice(b"legacy");
    data.extend_from_slice(&[10, 20, 30, 255]);
    let prefix = data.clone();
    data.extend_from_slice(&(maps.len() as u32 + 4).to_le_bytes());
    data.extend(maps);
    let mut doc = Document::new(2, 1);
    doc.metadata.push(MetadataChunk {
        frame: 0,
        kind: 0x2020,
        owner: "palette".into(),
        data,
    });
    let readback = r#"
 local p=app.activeSprite.properties
 assert(p.small==42 and p.removed==nil)
 assert(p.shared.name=='árbol' and p.shared.enabled==true)
 assert(p.shared.offset==Point(-3,4) and p.shared.area==Rectangle(0,1,2,3))
 assert(p.shared.size==Size(7,8))
 assert(p.vector[1]==-3 and p.vector[2]==2.5 and p.vector[3]==false)
 local found=false
 for k,v in pairs(p) do if k=='small' then found=v==42 end end
 assert(found)
 "#;
    let output = run_source(
        &doc,
        &format!(
            r#"
 local p=app.activeSprite.properties
 assert(p.small==42)
 p.shared={{name='árbol',enabled=true,offset=Point(-3,4),area=Rectangle(0,1,2,3),size=Size(7,8)}}
 p.vector={{-3,2.5,false}}
 p.removed='temporary'
 app.command.Undo()
 assert(p.removed==nil)
 app.command.Redo()
 assert(p.removed=='temporary')
 p.removed=nil
 p.small=42
 {readback}
 "#
        ),
        0,
        0,
        None,
    )
    .unwrap();
    let chunk = &output.metadata[0].data;
    assert!(chunk.starts_with(&prefix));
    assert!(
        chunk
            .windows(namespaced.len())
            .any(|part| part == namespaced)
    );
    assert!(chunk.windows(untouched.len()).any(|part| part == untouched));
    let directory =
        std::env::temp_dir().join(format!("pixelcraft-lua-properties-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    for extension in ["pixel", "aseprite"] {
        let path = directory.join("properties").with_extension(extension);
        pixelcraft::formats::save(&path, &output).unwrap();
        let reopened = pixelcraft::formats::load(&path).unwrap();
        assert!(
            reopened
                .metadata
                .iter()
                .any(|m| m.kind == 0x2020 && m.data == *chunk)
        );
        run_source(&reopened, readback, 0, 0, None).unwrap();
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lua_sprite_extension_properties_survive_ase_roundtrip() {
    let input = pixelcraft::formats::load(std::path::Path::new(
        "tests/fixtures/file-tests-props.aseprite",
    ))
    .unwrap();
    let script = r#"
 local sprite=app.activeSprite
 assert(sprite.properties.b==1)
 assert(sprite.properties('ext').a[2]=='two')
 assert(sprite.properties('new/plugin').phase==nil)
 sprite.properties('new/plugin').phase={steps={1,2,3},ready=true}
 sprite.properties('ext').updated=Point(2,7)
 sprite.properties={onlyDefault='replaced'}
 sprite:newEmptyFrame(1)
 "#;
    let edited = run_source(&input, script, 0, 0, None).unwrap();
    let path = std::env::temp_dir().join(format!(
        "pixelcraft-property-namespaces-{}.aseprite",
        std::process::id()
    ));
    pixelcraft::formats::save(&path, &edited).unwrap();
    let file = fs::read(&path).unwrap();
    let mut pos = 144;
    let mut kinds = Vec::new();
    for _ in 0..u16::from_le_bytes(file[134..136].try_into().unwrap()) {
        let size = u32::from_le_bytes(file[pos..pos + 4].try_into().unwrap()) as usize;
        kinds.push(u16::from_le_bytes(
            file[pos + 4..pos + 6].try_into().unwrap(),
        ));
        pos += size;
    }
    assert!(
        kinds.iter().position(|&kind| kind == 0x2008).unwrap()
            < kinds.iter().position(|&kind| kind == 0x2020).unwrap()
    );
    let reopened = pixelcraft::formats::load(&path).unwrap();
    run_source(
        &reopened,
        r#"
 local sprite=app.activeSprite
 assert(sprite.properties.b==nil and sprite.properties.onlyDefault=='replaced')
 assert(sprite.properties('ext').a[3]=='three')
 assert(sprite.properties('ext').updated==Point(2,7))
 local phase=sprite.properties('new/plugin').phase
 assert(phase.ready and phase.steps[3]==3)
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    fs::remove_file(path).unwrap();
}

#[test]
fn lua_layer_cel_slice_properties_follow_their_metadata_owners() {
    let mut input = Document::new(3, 2);
    input.add_layer();
    input.layers[1].name = "First".into();
    input.layers[1].cels[0].set(0, [123, 45, 67, 255]);
    input.add_frame(0, true);
    input.link_cel(1, 1, 0).unwrap();
    let readback = r#"
 local sprite=app.activeSprite
 local layer=sprite.layers[1]
 assert(layer.name=='First' and layer.properties.label=='map')
 assert(layer.properties('pc/test').weight==0.25)
 assert(layer:cel(1).properties.payload==nil)
 assert(layer:cel(2).properties.payload.x==7)
 assert(layer:cel(3).properties.payload.x==7)
 assert(sprite.slices[1].name=='Survivor')
 assert(sprite.slices[1].properties.pivot==Point(1,0))
 assert(sprite.properties.rank==42 and sprite.properties.label==nil)
 "#;
    let output = run_source(
        &input,
        &format!(
            r#"
 local sprite=app.activeSprite
 sprite.properties.rank=42
 sprite.layers[1].properties.label='Delete this'
 local layer=sprite.layers[2]
 local linked=layer:cel(2)
 layer.properties.label='map'
 layer.properties('pc/test').weight=0.25
 linked.properties.payload={{x=7}}
 assert(layer:cel(1).properties.payload.x==7)
 local removed=sprite:newSlice(Rectangle(0,0,1,1))
 removed.properties.pivot=Point(0,0)
 local keep=sprite:newSlice(Rectangle(1,0,2,1))
 keep.name='Survivor'
 keep.properties.pivot=Point(1,0)
 sprite:deleteSlice(removed)
 sprite:newEmptyFrame(1)
 sprite:deleteLayer(sprite.layers[1])
 assert(layer.properties.label=='map')
 assert(linked.properties.payload.x==7)
 assert(keep.properties.pivot==Point(1,0))
 {readback}
 "#
        ),
        0,
        0,
        None,
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!(
        "pixelcraft-object-properties-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    for ext in ["pixel", "aseprite"] {
        let file = dir.join("properties").with_extension(ext);
        pixelcraft::formats::save(&file, &output).unwrap();
        let reopened = pixelcraft::formats::load(&file).unwrap();
        run_source(&reopened, readback, 0, 0, None).unwrap();
    }
    let mut reordered = output;
    reordered.reverse_frames(&[0, 2]).unwrap();
    let reordered_readback = r#"
 local sprite=app.activeSprite
 local layer=sprite.layers[1]
 assert(sprite.properties.rank==42)
 assert(layer.properties('pc/test').weight==0.25)
 assert(layer:cel(1).properties.payload.x==7)
 assert(layer:cel(2).properties.payload.x==7)
 assert(layer:cel(3).properties.payload==nil)
 assert(sprite.slices[1].properties.pivot==Point(1,0))
 "#;
    run_source(&reordered, reordered_readback, 0, 0, None).unwrap();
    reordered.remove_frame(0);
    let file = dir.join("reordered.aseprite");
    pixelcraft::formats::save(&file, &reordered).unwrap();
    let reopened = pixelcraft::formats::load(&file).unwrap();
    run_source(
        &reopened,
        r#"
 local sprite=app.activeSprite
 local layer=sprite.layers[1]
 assert(sprite.properties.rank==42)
 assert(layer.properties('pc/test').weight==0.25)
 assert(layer:cel(1).properties.payload.x==7)
 assert(layer:cel(2).properties.payload==nil)
 assert(sprite.slices[1].properties.pivot==Point(1,0))
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn lua_tag_properties_keep_ase_chunk_order_and_follow_tag_deletion() {
    let fixture = pixelcraft::formats::load(std::path::Path::new(
        "tests/fixtures/file-tests-props.aseprite",
    ))
    .unwrap();
    run_source(
        &fixture,
        r#"
 local tag=app.activeSprite.tags[1]
 assert(tag.properties.a==Point(1,2))
 assert(tag.properties.b.c==Rectangle(1,2,3,4))
 "#,
        0,
        0,
        None,
    )
    .unwrap();

    let mut input = Document::new(2, 1);
    input.add_frame(0, false);
    input.add_frame(1, false);
    let readback = r#"
 local tags=app.activeSprite.tags
 assert(#tags==3)
 assert(tags[1].name=='Idle' and tags[1].properties.marker==nil)
 assert(tags[2].name=='Walk east' and tags[2].properties.marker==Point(2,3))
 assert(tags[2].properties('pc/test').speed==0.5)
 assert(tags[3].name=='Jump' and tags[3].properties.marker=='jump')
 "#;
    let output = run_source(
        &input,
        &format!(
            r#"
 local sprite=app.activeSprite
 local idle=sprite:newTag(1,1)
 idle.name='Idle'
 local throwaway=sprite:newTag(1,1)
 throwaway.name='Walk'
 local keep=sprite:newTag(2,2)
 keep.name='Walk'
 local jump=sprite:newTag(3,3)
 jump.name='Jump'
 keep.properties={{marker=Point(2,3)}}
 keep.properties('pc/test').speed=0.5
 throwaway.properties.marker='delete this'
 jump.properties.marker='jump'
 sprite:deleteTag(throwaway)
 keep.name='Walk east'
 assert(sprite.tags[2]==keep and keep.sprite==sprite)
 assert(not pcall(function()return throwaway.properties.marker end))
 {readback}
 "#
        ),
        0,
        0,
        None,
    )
    .unwrap();
    let chunks: Vec<_> = output
        .metadata
        .iter()
        .filter(|chunk| chunk.owner == "tags" && chunk.kind == 0x2020)
        .collect();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].data, [0, 0, 0, 0]);

    let encoded = pixelcraft::formats::encode_ase(&output).unwrap();
    let mut decoded = pixelcraft::formats::decode_ase(&encoded).unwrap();
    run_source(&decoded, readback, 0, 0, None).unwrap();
    let mut baked = output;
    baked.layers[0].adjustment = Some(pixelcraft::model::Adjustment::Levels {
        black: 0,
        white: 200,
    });
    let encoded = pixelcraft::formats::encode_ase(&baked).unwrap();
    let baked_reopened = pixelcraft::formats::decode_ase(&encoded).unwrap();
    run_source(&baked_reopened, readback, 0, 0, None).unwrap();
    decoded.remove_frame(0);
    run_source(
        &decoded,
        r#"
 local tags=app.activeSprite.tags
 assert(#tags==2)
 assert(tags[1].name=='Walk east' and tags[1].properties.marker==Point(2,3))
 assert(tags[2].name=='Jump' and tags[2].properties.marker=='jump')
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert!(decoded.remove_tag(0));
    let encoded = pixelcraft::formats::encode_project(&decoded).unwrap();
    let reopened = pixelcraft::formats::decode_project(&encoded).unwrap();
    run_source(
        &reopened,
        "assert(app.activeSprite.tags[1].properties.marker=='jump')",
        0,
        0,
        None,
    )
    .unwrap();
}

#[test]
fn lua_tileset_and_tile_properties_follow_definition_edits_and_ase_chunk_order() {
    use pixelcraft::model::TileDefinitionEdit;

    let fixture = pixelcraft::formats::load(std::path::Path::new(
        "tests/fixtures/file-tests-props.aseprite",
    ))
    .unwrap();
    run_source(
        &fixture,
        r#"
 local set=app.activeSprite.tilesets[1]
 assert(set.properties.a=="i'm a tilemap")
 assert(set.properties('ext').a=='text from extension')
 assert(set:tile(0).properties.a==nil)
 assert(set:tile(1).properties.a==320 and set:tile(2).properties.b==650)
 "#,
        0,
        0,
        None,
    )
    .unwrap();

    let mut output = run_source(
        &Document::new(2, 1),
        r#"
 local sprite=app.activeSprite
 local discarded=sprite:newTileset(Rectangle(0,0,1,1))
 local set=sprite:newTileset(Rectangle(0,0,1,1),4)
 set.name='Terrain'
 set.properties.biome='forest'
 set:tile(0).properties.empty=true
 local stone=set:tile(1)
 stone.properties.material='stone'
 local soil=set:tile(2)
 soil.properties('pc/test').density=1.5
 local inserted=sprite:newTile(set,1)
 assert(stone.index==2 and soil.index==3)
 assert(soil.properties('pc/test').density==1.5)
 assert(inserted.properties.material==nil)
 sprite:deleteTile(inserted)
 sprite:deleteTileset(discarded)
 assert(sprite.tilesets[1]==set and stone.index==1)
 assert(stone.properties.material=='stone')
 assert(set:tile(0).properties.empty)
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    output
        .edit_tile_definition(0, 2, TileDefinitionEdit::MoveTo(1))
        .unwrap();
    output
        .edit_tile_definition(0, 2, TileDefinitionEdit::Duplicate)
        .unwrap();
    output
        .edit_tile_definition(0, 2, TileDefinitionEdit::Delete)
        .unwrap();
    let readback = r#"
 local set=app.activeSprite.tilesets[1]
 assert(set.name=='Terrain' and set.properties.biome=='forest')
 assert(set:tile(0).properties.empty)
 assert(set:tile(1).properties('pc/test').density==1.5)
 assert(set:tile(2).properties.material==nil)
 assert(set:tile(3).properties.material=='stone')
 "#;
    run_source(&output, readback, 0, 0, None).unwrap();
    let native = pixelcraft::formats::encode_project(&output).unwrap();
    let native = pixelcraft::formats::decode_project(&native).unwrap();
    run_source(&native, readback, 0, 0, None).unwrap();
    let file = pixelcraft::formats::encode_ase(&output).unwrap();
    let mut pos = 144;
    let mut kinds = Vec::new();
    for _ in 0..u16::from_le_bytes(file[134..136].try_into().unwrap()) {
        let size = u32::from_le_bytes(file[pos..pos + 4].try_into().unwrap()) as usize;
        kinds.push(u16::from_le_bytes(
            file[pos + 4..pos + 6].try_into().unwrap(),
        ));
        pos += size;
    }
    let index = kinds.iter().position(|&kind| kind == 0x2023).unwrap();
    assert_eq!(&kinds[index + 1..index + 6], &[0x2020; 5]);
    let reopened = pixelcraft::formats::decode_ase(&file).unwrap();
    run_source(&reopened, readback, 0, 0, None).unwrap();

    let tileset_only = run_source(
        &Document::new(1, 1),
        "local set=app.activeSprite:newTileset(Rectangle(0,0,1,1),4); set.properties.ready=true",
        0,
        0,
        None,
    )
    .unwrap();
    let file = pixelcraft::formats::encode_ase(&tileset_only).unwrap();
    let reopened = pixelcraft::formats::decode_ase(&file).unwrap();
    let chunk_count = reopened
        .metadata
        .iter()
        .filter(|chunk| chunk.kind == 0x2020 && chunk.owner == "tileset:0")
        .count();
    assert_eq!(chunk_count, 5);
}

#[test]
fn lua_layer_frame_and_cel_handles_survive_edits_undo_and_redo() {
    let mut input = Document::new(3, 2);
    input.add_layer();
    input.add_frame(0, false);
    let output = run_source(
        &input,
        r#"
 local sprite=app.activeSprite
 local removed=sprite.layers[1]
 local layer=sprite.layers[2]
 local frame=sprite.frames[2]
 local cel=layer:cel(frame)
 local image=cel.image
 layer.name='Keep'
 assert(layer==sprite.layers[2] and cel==layer:cel(frame))
 app.transaction(function()
  sprite:newEmptyFrame(1)
  sprite:deleteLayer(removed)
 end)
 assert(sprite.layers[1]==layer and sprite.frames[3]==frame)
 assert(layer:cel(frame)==cel and cel.layer==layer and cel.frame==frame)
 assert(cel.frameNumber==3 and frame.frameNumber==3)
 assert(not pcall(function()return removed.name end))
 app.activeLayer=layer
 app.activeFrame=frame
 assert(app.activeCel==cel)
 image:drawPixel(1,1,Color{r=77,g=13,b=29,a=255})
 assert(cel.image:getPixel(1,1)==app.pixelColor.rgba(77,13,29,255))
 assert(layer:cel(2).image:getPixel(1,1)==0)
 app.command.Undo()
 assert(sprite.layers[1]==removed and sprite.layers[2]==layer)
 assert(sprite.frames[2]==frame and layer:cel(frame)==cel and cel.frameNumber==2)
 assert(cel.image:getPixel(1,1)==0)
 app.command.Redo()
 assert(sprite.layers[1]==layer and sprite.frames[3]==frame)
 assert(layer:cel(frame)==cel and cel.image:getPixel(1,1)==app.pixelColor.rgba(77,13,29,255))
 local orphan
 local ok=pcall(function()
  app.transaction(function()
   orphan=sprite:newEmptyFrame(1)
   error('Rollback')
  end)
 end)
 assert(not ok and frame.frameNumber==3)
 local replacement=sprite:newEmptyFrame(1)
 assert(orphan~=replacement and not pcall(function()return orphan.frameNumber end))
 frame.duration=0.125
 cel.opacity=150
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(output.layers.len(), 1);
    assert_eq!(output.layers[0].name, "Keep");
    assert_eq!(output.durations[3], 125);
    assert_eq!(output.frame_export_id(0), 4);
    assert_eq!(output.layers[0].cel_meta[3].opacity, 150);
    assert_eq!(output.layers[0].cels[3][4], [77, 13, 29, 255]);
}

#[test]
fn lua_metadata_object_handles_survive_history_and_reject_absent_objects() {
    let output = run_source(
        &Document::new(3, 2),
        r#"
 local sprite=app.activeSprite
 local tag=sprite:newTag(1,1)
 tag.name='Idle'
 tag.properties.motion='still'
 local other=sprite:newTag(1,1)
 other.name='Idle'
 local slice=sprite:newSlice(Rectangle(0,0,1,1))
 slice.name='Nine patch'
 slice.properties.anchor=Point(1,0)
 local set=sprite:newTileset(Rectangle(0,0,1,1),3)
 set.name='Terrain'
 set.properties.region='forest'
 local removed=set:tile(1)
 local retained=set:tile(2)
 retained.properties.material='soil'
 local image=retained.image
 app.transaction(function()
  sprite:deleteTag(tag)
  sprite:deleteSlice(slice)
  sprite:deleteTile(removed)
 end)
 assert(sprite.tags[1]==other and #sprite.slices==0)
 assert(not pcall(function() return tag.name end))
 assert(not pcall(function() return slice.bounds end))
 assert(not pcall(function() return removed.index end))
 assert(set:tile(1)==retained)
 app.command.Undo()
 assert(sprite.tags[1]==tag and sprite.tags[2]==other and tag.properties.motion=='still')
 assert(sprite.slices[1]==slice and slice.properties.anchor==Point(1,0))
 assert(sprite.tilesets[1]==set and set.properties.region=='forest')
 assert(set:tile(1)==removed and set:tile(2)==retained and retained.properties.material=='soil')
 app.command.Redo()
 assert(sprite.tags[1]==other and set:tile(1)==retained)
 assert(not pcall(function() return tag.name end))
 assert(not pcall(function() return slice.bounds end))
 assert(not pcall(function() return removed.index end))
 image:drawPixel(0,0,Color('#123456'))
 assert(retained.image:getPixel(0,0)==app.pixelColor.rgba(18,52,86,255))
 app.transaction(function() sprite:deleteTileset(set) end)
 assert(not pcall(function() return image:getPixel(0,0) end))
 assert(not pcall(function() return set.properties.region end))
 app.command.Undo()
 assert(sprite.tilesets[1]==set and set:tile(1)==retained)
 assert(image:getPixel(0,0)==app.pixelColor.rgba(18,52,86,255))
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    assert_eq!(output.tags.len(), 1);
    assert!(output.slices.is_empty());
    assert_eq!(output.tilesets[0].tiles[0][0], [18, 52, 86, 255]);
}

#[test]
fn lua_polygon_and_contour_use_native_boundary_and_fill_coverage() {
    let doc = Document::new(8, 8);
    let output = run_source(
        &doc,
        r#"
 local points={Point(1,1),Point(6,1),Point(6,6),Point(1,6)}
 app.useTool{tool='polygon',points=points,color=Color('#ff4422')}
 app.useTool{tool='filled_contour',points=points,color=Color('#44ff22'),opacity=128}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let cel = &output.layers[0].cels[0];
    assert_eq!(cel[27], [68, 255, 34, 128]);
    assert_eq!(cel[54][3], 255);
    assert_eq!(cel[0], CLEAR);
}

#[test]
fn lua_cubic_curve_renders_connected_pixels_between_native_endpoints() {
    let doc = Document::new(13, 9);
    let output = run_source(
        &doc,
        r#"
 local red=Color('#de4422')
 app.useTool{tool='curve',points={Point(1,7),Point(1,1),Point(11,1),Point(11,7)},color=red}
 assert(not pcall(function()
   app.useTool{tool='curve',points={Point(0,0),Point(1,1)},color=red}
 end))
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let cel = &output.layers[0].cels[0];
    let red = [222, 68, 34, 255];
    assert_eq!(cel[7 * 13 + 1], red);
    assert_eq!(cel[7 * 13 + 11], red);
    assert_eq!(cel[3 * 13 + 6], red);
    assert_eq!(cel[7 * 13 + 6], CLEAR);
}

#[test]
fn lua_retouch_reads_from_the_stroke_start_and_clips_to_mask() {
    let mut doc = Document::new(6, 5);
    doc.set_pixel(0, 0, 14, [231, 101, 24, 255]);
    let output = run_source(
        &doc,
        r#"
 local sel=app.activeSprite.selection
 sel:select(Rectangle(2,2,2,1))
 app.useTool{tool='blur',points={Point(2,2),Point(3,2),Point(2,2)}}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let cel = &output.layers[0].cels[0];
    assert_eq!(cel[14], [231, 101, 24, 28]);
    assert_eq!(cel[15], [231, 101, 24, 28]);
    assert_eq!(cel[13], CLEAR);
}

#[test]
fn lua_lock_alpha_ink_preserves_rgb_coverage_and_unpainted_pixels() {
    let mut doc = Document::new(4, 1);
    doc.set_pixel(0, 0, 1, [0, 20, 245, 112]);
    let result = run_source(
        &doc,
        r#"
 assert(Ink.LOCK_ALPHA)
 app.useTool{tool='line',points={Point(0,0),Point(3,0)},
  color=Color('#ed5500'),ink=Ink.LOCK_ALPHA,opacity=128}
 "#,
        0,
        0,
        None,
    )
    .unwrap();
    let cel = &result.layers[0].cels[0];
    assert_eq!(cel[0], CLEAR);
    assert_eq!(cel[1][3], 112);
    assert!(cel[1][0] > 0 && cel[1][2] > 0);
    assert_eq!(cel[2], CLEAR);
}
