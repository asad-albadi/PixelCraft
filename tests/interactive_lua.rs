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
fn host_selection_is_available_to_existing_scripts() {
    let doc = Document::new(4, 4);
    run_source_with_host(&doc,"local s=app.activeSprite.selection;assert(s:contains(2,1));assert(not s:contains(0,0));s:deselect()",0,0,None,None,json!({"selection":{"x":2,"y":1,"width":1,"height":1}}),Box::new(|r|{assert_eq!(r["op"],"preferences");assert_eq!(r["selection"]["width"],0);Ok(json!({}))})).unwrap();
}
