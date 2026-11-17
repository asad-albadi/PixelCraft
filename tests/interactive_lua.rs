use pixelcraft::{extensions::run_source_with_host, model::*};
use serde_json::json;
#[test]
fn dialog_callbacks_timer_events_and_params_change_the_sprite() {
    let d = Document::new(2, 1);
    let mut ticks = 0;
    let result=run_source_with_host(&d,r#"
 assert(app.isUIAvailable and app.params.amount=='12')
 local changes=0;local handler=app.activeSprite.events:on('change',function()changes=changes+1 end)
 local dlg=Dialog('A real dialog')
 dlg:number{id='amount',text='1'}:button{id='apply',text='Apply',onclick=function()
 app.transaction(function()app.activeImage:drawPixel(0,0,Color{r=dlg.data.amount})end);dlg:close()
 end}:show()
 assert(changes==1);app.activeSprite.events:off(handler)
 local timer;timer=Timer{interval=0.1,ontick=function()app.activeImage:drawPixel(1,0,Color{g=25});timer:stop()end};timer:start()
 "#,0,0,None,None,json!({"params":{"amount":"12"}}),Box::new(move |request|{
 match request["op"].as_str().unwrap(){
 "dialog"=>{assert_eq!(request["dialog"]["title"],"A real dialog");Ok(json!({"widget":"apply","data":{"amount":12}}))},
 "pump"=>{ticks+=1;Ok(json!({"time":ticks as f64*0.2}))},
 "preferences"=>Ok(json!({})),_=>panic!("Unexpected host request: {request}")}
 })).unwrap();
    assert_eq!(result.layers[0].cels[0][0], [12, 0, 0, 255]);
    assert_eq!(result.layers[0].cels[0][1], [0, 25, 0, 255]);
}
#[test]
fn tabbed_dialog_preserves_group_selection_and_reports_native_bounds() {
    let mut request_count = 0;
    run_source_with_host(
        &Document::new(1, 1),
        r#"
local clicked,changed=0,0
local dlg=Dialog{title='Tabbed extension'}
dlg.bounds=Rectangle(18,24,300,180)
dlg:tab{id='first',text='First',onclick=function(ev)
 assert(ev.tab=='first' and dlg.data.page=='first')
 assert(dlg.bounds.x==30 and dlg.bounds.y==40)
 dlg.bounds=Rectangle(42,60,280,200)
 clicked=clicked+1
end}:entry{id='one',text='first value'}
   :tab{id='second',text='Second'}:entry{id='two',text='second value'}
   :endtabs{id='page',selected='second',onchange=function(ev)
    assert(ev.tab=='first')
    changed=changed+1
   end}
   :button{id='done',text='Close'}
assert(dlg.data.page=='second')
dlg:show()
assert(clicked==1 and changed==1 and dlg.data.two=='second value')
assert(dlg.bounds.x==44 and dlg.bounds.height==222)
dlg:show{wait=false,bounds=Rectangle(30,32,260,180)}
local timer
timer=Timer{interval=0.05,ontick=function()
 assert(dlg.bounds.x==90 and dlg.bounds.height==185)
 timer:stop()
 dlg:close()
end}
timer:start()
"#,
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(move |request| {
            if request["op"] == "preferences" {
                assert_eq!(request_count, 4);
                return Ok(json!({}));
            }
            request_count += 1;
            if request["op"] == "pump" {
                assert_eq!(request["dialogs"][0]["bounds"], json!([30, 32, 260, 180]));
                return Ok(json!({
                    "time":request_count as f64,"dialogBounds":[{"id":1,"bounds":[90,70,260,185]}],
                }));
            }
            assert_eq!(request["op"], "dialog");
            let spec = &request["dialog"];
            let group = spec["widgets"][0]["hostId"].as_str().unwrap();
            assert_eq!(spec["widgets"][0]["kind"], "tabs");
            assert_eq!(spec["widgets"][1]["tabId"], "first");
            assert_eq!(spec["widgets"][2]["tabId"], "second");
            assert!(spec["widgets"][3]["tabId"].is_null());
            if request_count == 1 {
                assert_eq!(spec["bounds"], json!([18, 24, 300, 180]));
                assert_eq!(spec["data"]["page"], "second");
                Ok(json!({
                    "tabGroup":group,"tabId":"first","tabChanged":true,"event":{"tab":"first"},
                    "data":{"page":"first"}, "dialogBounds":[{"id":1,"bounds":[30,40,320,201]}]
                }))
            } else {
                assert_eq!(request_count, 2);
                assert_eq!(spec["bounds"], json!([42, 60, 280, 200]));
                Ok(json!({"widget":"done","dialogBounds":[{"id":1,"bounds":[44,62,299,222]}]}))
            }
        }),
    )
    .unwrap();
}

#[test]
fn dialog_canvas_paints_and_handles_input_without_resending_unchanged_artwork() {
    let mut pumps = 0;
    let output = run_source_with_host(
        &Document::new(2, 1),
        r#"
local dlg=Dialog('Canvas')
local sceneColor=Color('#bb3322')
local paints=0
local thumbnail=Image(2,1,ColorMode.RGB)
thumbnail:drawPixel(1,0,Color('#234567'))
dlg:canvas{id='preview',width=48,height=24,onpaint=function(ev)
 local gc=ev.context
 assert(gc.width==48 and gc.height==24)
 paints=paints+1
 gc.color=sceneColor
 gc:fillRect(Rectangle(1,2,8,9))
 gc:save()
 gc.opacity=128
 gc:drawImage(thumbnail,Rectangle(1,0,1,1),Rectangle(12,6,8,8))
 gc:restore()
 assert(gc.opacity==255)
end,onmousedown=function(ev)
 assert(ev.x==5 and ev.y==7 and ev.button==MouseButton.LEFT)
 sceneColor=Color('#31aa53')
 dlg:repaint()
end,onmouseup=function(ev)
 assert(ev.x==8 and ev.y==9 and paints==2)
 app.activeImage:drawPixel(1,0,sceneColor)
 dlg:close()
end}
dlg:show{wait=false}
"#,
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(move |request| {
            if request["op"] == "preferences" {
                assert_eq!(pumps, 3);
                return Ok(json!({}));
            }
            assert_eq!(request["op"], "pump");
            pumps += 1;
            let canvas = &request["dialogs"][0]["widgets"][0];
            assert_eq!(canvas["kind"], "canvas");
            assert_eq!(canvas["canvasEvents"]["onmousedown"], true);
            if pumps == 2 {
                assert!(canvas["paint"].is_null());
                return Ok(json!({"dialog":1,"widget":"preview","canvasEvents":[
                    {"name":"onmousedown","x":5,"y":7,"button":1}
                ]}));
            }
            assert_eq!(canvas["paint"][0]["kind"], "fillRect");
            assert_eq!(
                canvas["paint"][0]["color"],
                if pumps == 1 {
                    json!([187, 51, 34, 255])
                } else {
                    json!([49, 170, 83, 255])
                }
            );
            assert_eq!(canvas["paint"][1]["kind"], "image");
            assert_eq!(canvas["paint"][1]["pixels"][1], json!([35, 69, 103, 255]));
            assert_eq!(canvas["paint"][1]["opacity"], 128);
            Ok(if pumps == 1 {
                json!({})
            } else {
                json!({"dialog":1,"widget":"preview","canvasEvents":[
                    {"name":"onmouseup","x":8,"y":9,"button":1}
                ]})
            })
        }),
    )
    .unwrap();
    assert_eq!(output.composite(0)[1], [49, 170, 83, 255]);
}

#[test]
fn actual_file_bridge_saves_and_reopens_a_project() {
    use pixelcraft::{extensions::script_file_request, formats};
    let dir = std::env::temp_dir().join(format!("pixelcraft-script-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.clone();
    let out = run_source_with_host(
        &Document::new(2, 2),
        r#"
 app.activeImage:drawPixel(1,1,Color('#123456'))
 app.activeSprite:saveAs('saved.aseprite')
 app.activeImage:clear()
 local s=app.open('saved.aseprite')
 assert(s.width==2 and app.activeImage:getPixel(1,1)==Color('#123456').rgbaPixel)
 "#,
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(move |r| {
            if r["op"] == "preferences" {
                Ok(json!({}))
            } else {
                script_file_request(&root, &r)
            }
        }),
    )
    .unwrap();
    assert_eq!(
        out.composite(0),
        formats::load(&dir.join("saved.aseprite"))
            .unwrap()
            .composite(0)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn interactive_file_workspace_keeps_sprites_and_inactive_handles_independent() {
    let original = Document::new(3, 1);
    let opened = Document::new(2, 2);
    let fixture = serde_json::to_value(opened).unwrap();
    let full_budget = serde_json::to_value(Document::new(1000, 1000)).unwrap();
    let result = run_source_with_host(
        &original,
        r##"
 local first=app.activeSprite
 local firstFrame=app.activeFrame
 local firstImage=app.activeImage
 firstImage:drawPixel(2,0,Color('#561234'))
 local second=app.open('second.pixel')
 assert(second~=first and second.id~=first.id and app.activeSprite==second)
 assert(#app.sprites==2 and first.width==3 and second.width==2)
 assert(not pcall(function()first:newCel(second.layers[1],first.frames[1])end))
 first.selection:selectAll()
 assert(first.selection.bounds.width==3 and second.selection.isEmpty)
 first.palettes[1]:setColor(1,Color('#1199dd'))
 assert(first.palettes[1]:getColor(1).red==17 and second.palettes[1]:getColor(1).red~=17)
 firstFrame.duration=0.125
 firstImage:drawPixel(1,0,Color('#083060'))
 assert(first.layers[1]:cel(1).image:getPixel(1,0)==Color('#083060').rgbaPixel)
 app.activeImage:drawPixel(1,1,Color('#004480'))
 first:saveCopyAs('first.pixel')
 app.activeSprite=first
 assert(app.activeFrame.duration==0.125 and app.activeImage:getPixel(2,0)==Color('#561234').rgbaPixel)
 assert(second.layers[1]:cel(1).image:getPixel(1,1)==Color('#004480').rgbaPixel)
 second:close()
 assert(#app.sprites==1 and app.activeSprite==first)
 assert(not pcall(function()app.activeSprite=second end))
 assert(not pcall(function()return second.width end))
 local third=app.open('third.pixel')
 assert(not pcall(function()app.open('large.pixel')end))
 assert(app.activeSprite==third and #app.sprites==2)
 third:close()
 assert(app.activeSprite==first and #app.sprites==1)
 "##,
        0,
        0,
        None,
        None,
        json!({}),
        Box::new(move |request| {
            match request["op"].as_str() {
                Some("open") if request["path"] == "large.pixel" => {
                    Ok(json!({"document":full_budget}))
                }
                Some("open") => Ok(json!({"document":fixture})),
                Some("save") => {
                    assert_eq!(request["path"], "first.pixel");
                    assert_eq!(request["document"]["width"], 3);
                    assert_eq!(request["document"]["durations"][0], 125);
                    Ok(json!({}))
                }
                Some("close" | "preferences") => Ok(json!({})),
                _ => panic!("Unexpected host request: {request}"),
            }
        }),
    ).unwrap();
    assert_eq!(result.durations, [125]);
    assert_eq!(result.composite(0)[1], [8, 48, 96, 255]);
    assert_eq!(result.composite(0)[2], [86, 18, 52, 255]);
}

#[test]
fn host_selection_is_available_to_existing_scripts() {
    let doc = Document::new(4, 4);
    run_source_with_host(&doc,"local s=app.activeSprite.selection;assert(s:contains(2,1));assert(not s:contains(0,0));s:deselect()",0,0,None,None,json!({"selection":{"x":2,"y":1,"width":1,"height":1}}),Box::new(|r|{assert_eq!(r["op"],"preferences");assert_eq!(r["selection"]["width"],0);Ok(json!({}))})).unwrap();
}
