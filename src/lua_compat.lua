-- Compatibility additions share the private handles of lua_api.lua.
local function json(v,key)
 local t=type(v)
 if t=='nil' then return 'null' elseif t=='boolean' or t=='number' then return tostring(v) elseif t=='string' then return '"'..v:gsub('[%z\1-\31\\"]',function(c)return string.format('\\u%04x',c:byte())end)..'"'
 elseif t=='table' then
  if getmetatable(v) and getmetatable(v).__name=='Color' then return json({r=v.red,g=v.green,b=v.blue,a=v.alpha}) end
  local out={};local objects={data=true,preferences=true,pluginPreferences=true,params=true,event=true};local array=#v>0 or next(v)==nil and not objects[key]
  if array then for i=1,#v do out[i]=json(v[i]) end else for k,val in pairs(v) do if type(val)~='function' and tostring(k):sub(1,1)~='_' then out[#out+1]=json(tostring(k))..':'..json(val,k) end end end
  return (array and '[' or '{')..table.concat(out,',')..(array and ']' or '}')
 end
 error('Unsupported host value: '..t)
end
local nativeHost=_pw_host
local function host(request)
 assert(nativeHost,'This operation requires an interactive editor session')
 local value=assert(load(nativeHost(json(request)),'host response','t',{}))();start=clock()
 if value.error then error(value.error) end
 return value
end
local function events()
 local handlers,nextId={},0;local e={}
 function e:on(name,fn) assert(type(fn)=='function');nextId=nextId+1;handlers[nextId]={name,fn};return nextId end
 function e:off(id) handlers[id]=nil end
 function e:_hasHandlers()return next(handlers)~=nil end
 function e:_emit(name,event) local pending={} for id,h in pairs(handlers) do if h[1]==name then pending[#pending+1]={id,h[2]} end end;for _,h in ipairs(pending) do if handlers[h[1]] then h[2](event or {}) end end end
 return e
end
local spriteEvents,appEvents=events(),events()
local selectionMethods={}
local function selection(mask)
 local s={_mask=mask or {}}
 return setmetatable(s,{__index=function(t,k)
  if selectionMethods[k] then return selectionMethods[k] end
  if k=='isEmpty' then return next(t._mask)==nil end
  if k=='bounds' then local x,y,r,b=math.huge,math.huge,-math.huge,-math.huge;for _,p in pairs(t._mask)do x=math.min(x,p.x);y=math.min(y,p.y);r=math.max(r,p.x);b=math.max(b,p.y)end;return x==math.huge and Rectangle() or Rectangle(x,y,r-x+1,b-y+1) end
 end})
end
local function points(value)
 if value._mask then return copy(value._mask) end
 local result={};for y=value.y,value.y+value.height-1 do for x=value.x,value.x+value.width-1 do result[x..','..y]=Point(x,y) end end;return result
end
function Selection(v) return selection(v and points(v) or nil) end
function selectionMethods:select(v) self._mask=points(v) end
function selectionMethods:deselect() self._mask={} end
function selectionMethods:selectAll() self:select(Rectangle(0,0,doc.width,doc.height)) end
function selectionMethods:contains(x,y) if type(x)=='table' then y=x.y;x=x.x end;return self._mask[x..','..y]~=nil end
function selectionMethods:add(v) for k,p in pairs(points(v))do self._mask[k]=p end end
function selectionMethods:subtract(v) for k in pairs(points(v))do self._mask[k]=nil end end
function selectionMethods:intersect(v) local m=points(v);for k in pairs(self._mask)do if not m[k]then self._mask[k]=nil end end end
function selectionMethods:invert() local m=points(Rectangle(0,0,doc.width,doc.height));for k in pairs(self._mask)do m[k]=nil end;self._mask=m end
local spriteSelection=Selection((_pw_options or {}).selection)
local filename=(_pw_options or {}).filename or ''
local spriteIndex=getmetatable(sprite).__index
getmetatable(sprite).__index=function(t,k)
 if k=='selection' then return spriteSelection elseif k=='events' then return spriteEvents elseif k=='bounds' then return Rectangle(0,0,doc.width,doc.height) elseif k=='filename' then return filename elseif k=='transparentColor' then return doc.transparent_index elseif k=='id' then return 1 end
 return spriteIndex(t,k)
end
local oldSpriteAssign=getmetatable(sprite).__newindex
getmetatable(sprite).__newindex=function(t,k,v) if k=='selection' then spriteSelection=Selection(v) elseif k=='filename' then filename=v elseif k=='transparentColor' then doc.transparent_index=v else oldSpriteAssign(t,k,v) end end
local undo,redo={},{}
local function snapshot() return {document=copy(doc),mask=copy(spriteSelection._mask),frame=activeFrame,layer=activeLayer} end
local function restore(s) doc=copy(s.document);spriteSelection=selection(copy(s.mask));activeFrame=s.frame;activeLayer=s.layer;rebuildHandles() end
local transactionDepth=0
rawset(app,'transaction',function(a,b)
 local before=snapshot();transactionDepth=transactionDepth+1;local result=table.pack(pcall(b or a));transactionDepth=transactionDepth-1
 if not result[1] then restore(before);error(result[2]) end
 if transactionDepth==0 then undo[#undo+1]=before;redo={};spriteEvents:_emit('change') end
 return table.unpack(result,2,result.n)
end)
local function command(fn) return function(p) return app.transaction(function()return fn(p or {})end) end end
app.command.Undo=function()if #undo>0 then redo[#redo+1]=snapshot();restore(table.remove(undo));spriteEvents:_emit('change') end end
app.command.Redo=function()if #redo>0 then undo[#undo+1]=snapshot();restore(table.remove(redo));spriteEvents:_emit('change') end end
for _,name in ipairs{'NewFrame','NewEmptyFrame'}do app.command[name]=command(app.command[name])end
local newLayerCommand=app.command.NewLayer
app.command.NewLayer=command(function(p) newLayerCommand();if p.name then app.activeLayer.name=p.name end;if p.group then doc.layers[activeLayer].group=true end end)
app.command.SelectAll=function()spriteSelection:selectAll()end
app.command.DeselectMask=function()spriteSelection:deselect()end
app.command.InvertMask=function()spriteSelection:invert()end
app.command.Clear=command(function()
 local l=doc.layers[activeLayer];local m=l.cel_meta[activeFrame]
 if l.tileset then
  local t=doc.tilesets[l.tileset+1];local g=m.tilemap or {x=0,y=0,columns=math.ceil(doc.width/t.width)}
  for i in ipairs(l.tile_cels[activeFrame])do
   local x,y=g.x+(i-1)%g.columns*t.width,g.y+math.floor((i-1)/g.columns)*t.height
   local selected=spriteSelection.isEmpty
   if not selected then for _,p in pairs(spriteSelection._mask)do if p.x>=x and p.y>=y and p.x<x+t.width and p.y<y+t.height then selected=true;break end end end
   if selected then l.tile_cels[activeFrame][i]=0 end
  end
  return
 end
 local im=app.activeImage;local pos=app.activeCel.position
 local color=l.background and scriptColors[2] or doc.color_mode=='Indexed' and doc.transparent_index or 0
 for y=0,im.height-1 do for x=0,im.width-1 do
  if spriteSelection.isEmpty or spriteSelection:contains(x+pos.x,y+pos.y) then im:drawPixel(x,y,color) end
 end end
end)
app.command.RemoveLayer=command(function()assert(#doc.layers>1,'Cannot remove the final layer');table.remove(doc.layers,activeLayer);for _,l in ipairs(doc.layers)do if l.parent==activeLayer-1 then l.parent=nil elseif l.parent and l.parent>activeLayer-1 then l.parent=l.parent-1 end end;activeLayer=math.min(activeLayer,#doc.layers);rebuildHandles()end)
app.command.GotoNextFrame=function()app.activeFrame=frameHandles[math.min(#doc.durations,activeFrame+1)]end
app.command.GotoPreviousFrame=function()app.activeFrame=frameHandles[math.max(1,activeFrame-1)]end
app.command.GotoFirstFrame=function()app.activeFrame=frameHandles[1]end
app.command.GotoLastFrame=function()app.activeFrame=frameHandles[#doc.durations]end
app.command.ChangePixelFormat=command(function(options)
 local mode=({rgb='Rgba',gray='Grayscale',indexed='Indexed'})[options.format] or options.format or doc.color_mode
 assert(mode=='Rgba' or mode=='Grayscale' or mode=='Indexed','Unsupported pixel format')
 local function convertPixel(p,palette,background)
  if mode=='Grayscale' then local g=math.floor((p[1]*2126+p[2]*7152+p[3]*722)/10000);return {g,g,g,p[4]} end
  if mode~='Indexed' then return copy(p) end
  local best,dist=0,math.huge
  if p[4]==0 and not background then best=doc.transparent_index else
   for i,c in ipairs(palette)do local e=0;for ch=1,4 do local value=ch==4 and i-1==doc.transparent_index and not background and 0 or c[ch];e=e+(p[ch]-value)^2 end;if e<dist then best,dist=i-1,e end end
  end
  local result=copy(palette[best+1] or {0,0,0,0});if best==doc.transparent_index and not background then result[4]=0 end
  return result,best
 end
 for _,l in ipairs(doc.layers)do for f,pixels in ipairs(l.cels)do
  local palette=doc.frame_palettes[f] or doc.palette;local indices=mode=='Indexed' and {} or nil
  for i,p in ipairs(pixels)do local color,index=convertPixel(p,palette,l.background);pixels[i]=color;if indices then indices[i]=index end end
  l.cel_meta[f].indices=indices
  for _,p in ipairs(l.cel_meta[f].outside)do p.color=convertPixel(p.color,palette,l.background)end
 end end
 for _,set in ipairs(doc.tilesets)do for _,tile in ipairs(set.tiles)do for i,p in ipairs(tile)do tile[i]=convertPixel(p,doc.palette,false)end end end
 doc.color_mode=mode
end)
function spriteMethods:deleteLayer(l) local old=activeLayer;activeLayer=layerNumber(l);app.command.RemoveLayer();activeLayer=math.min(old,#doc.layers)end
function spriteMethods:deleteTag(tag) for i,t in ipairs(doc.tags)do if t.name==tag.name then table.remove(doc.tags,i);return end end end
local appIndex,appAssign=getmetatable(app).__index,getmetatable(app).__newindex
local preferences=copy((_pw_options or {}).preferences or {})
preferences.general=preferences.general or {visible_timeline=true}
preferences.documentSettings=preferences.documentSettings or {grid={bounds=Rectangle(0,0,16,16)},onionskin={active=false}}
preferences.document=function()return preferences.documentSettings end
preferences.toolSettings=preferences.toolSettings or {}
preferences.tool=function(id)preferences.toolSettings[id]=preferences.toolSettings[id] or {opacity=255,tolerance=0,contiguous=true};return preferences.toolSettings[id]end
local tool={id='pencil'}
rawset(app,'params',copy((_pw_options or {}).params or {}))
rawset(app,'isUIAvailable',nativeHost~=nil)
rawset(app,'events',appEvents)
rawset(app,'preferences',preferences)
rawset(app,'apiVersion',24)
getmetatable(app).__index=function(t,k)
 if k=='editor' then return {sprite=sprite,frame=frameHandles[activeFrame],zoom=(_pw_options or {}).zoom or 1,scroll=Point(),mousePos=Point()} elseif k=='site' then return {sprite=sprite,layer=layerHandles[activeLayer],frame=frameHandles[activeFrame],cel=cel(activeLayer,activeFrame)} elseif k=='range' then return {sprite=sprite,layers={layerHandles[activeLayer]},frames={frameHandles[activeFrame]},cels={cel(activeLayer,activeFrame)},isEmpty=false} elseif k=='tool' then return tool elseif k=='fs' then return {
 joinPath=function(...)return table.concat({...},'/'):gsub('/+','/')end,
 fileName=function(p)return p:match('[^/\\]+$') or p end,
 filePath=function(p)return p:match('^(.*)[/\\]') or '' end,
 fileTitle=function(p)return (p:match('[^/\\]+$') or p):gsub('%.[^.]+$','')end,
 fileExtension=function(p)return p:match('%.([^./\\]+)$') or ''end,
 isFile=function(p)return host{op='isFile',path=p}.value end,
 isDirectory=function(p)return host{op='isDirectory',path=p}.value end,
 listFiles=function(p)return host{op='listFiles',path=p}.value end,
 makeDirectory=function(p)return host{op='makeDirectory',path=p}.value end,
 userConfigPath=(_pw_options or {}).configPath or '',userDocsPath=(_pw_options or {}).projectPath or ''}
 end
 return appIndex(t,k)
end
getmetatable(app).__newindex=function(t,k,v)
 if k=='fgColor' then scriptColors[1]=unpackColor(v);appEvents:_emit('fgcolorchange') elseif k=='bgColor' then scriptColors[2]=unpackColor(v);appEvents:_emit('bgcolorchange') elseif k=='tool' then tool=type(v)=='string' and {id=v} or v
 else appAssign(t,k,v);if k=='activeFrame' or k=='activeLayer' then appEvents:_emit('sitechange') end end
end
app.pixelColor.graya=function(g,a)return (pixelInteger(g)&255)|((pixelInteger(a==nil and 255 or a)&255)<<8)end
app.pixelColor.grayaV=function(v)return pixelInteger(v)&255 end
app.pixelColor.grayaA=function(v)return (pixelInteger(v)>>8)&255 end
app.pixelColor.tile=function(id,flags)return id|(flags or 0)end
app.pixelColor.tileI=function(v)return v&0x1fffffff end
app.pixelColor.tileF=function(v)return v&0xe0000000 end
TileFlags={XFLIP=0x80000000,YFLIP=0x40000000,DFLIP=0x20000000}
MouseButton={NONE=0,LEFT=1,RIGHT=2,MIDDLE=4};SelectMode={REPLACE=0,ADD=1,SUBTRACT=2,INTERSECT=3}
local imageGet,imageDraw=imageMethods.getPixel,imageMethods.drawPixel
function imageMethods:getPixel(x,y)
 local p=imageGet(self,x,y)
 if self.colorMode=='Grayscale' then local c=unpackColor(p);return app.pixelColor.graya(c[1],c[4])
 elseif self.colorMode=='Indexed' then local c=unpackColor(p);if c[4]==0 then return doc.transparent_index end;return Color(c).index end
 return p
end
function imageMethods:drawPixel(x,y,p)
 if type(p)=='number' then if self.colorMode=='Grayscale' then local g=p&255;p=rgba(g,g,g,(p>>8)&255) elseif self.colorMode=='Indexed' then local c=copy(doc.palette[p+1] or {0,0,0,0});if p==doc.transparent_index then c[4]=0 end;p=c end end
 imageDraw(self,x,y,p)
end
function imageMethods:isEqual(other)if self.width~=other.width or self.height~=other.height then return false end;for i,p in ipairs(self._pixels)do for c=1,4 do if p[c]~=other._pixels[i][c]then return false end end end;return true end
function imageMethods:drawSprite(s,f,pos)assert(s==sprite);pos=pos or Point();for li,l in ipairs(doc.layers)do if l.visible and not l.group then self:drawImage(cel(li,frameNumber(f)).image,pos,l.opacity)end end end
function spriteMethods:saveAs(path)host{op='save',path=path,document=doc};filename=path;return true end
function spriteMethods:saveCopyAs(path)host{op='save',path=type(path)=='table' and path.filename or path,document=doc};return true end
function spriteMethods:close()host{op='close'}end
app.command.SaveFile=function()assert(filename~='','Sprite has no filename');sprite:saveAs(filename)end
app.command.SaveFileAs=function(p)assert(p and p.filename,'SaveFileAs requires filename');sprite:saveAs(p.filename)end
app.command.SaveFileCopyAs=function(p)sprite:saveCopyAs(p.filename)end
rawset(app,'open',function(path) local loaded=host{op='open',path=path};doc=loaded.document;filename=path;activeFrame=1;activeLayer=1;rebuildHandles();appEvents:_emit('sitechange');return sprite end)
local timers,dialogs={},{}
function Timer(t)
 assert(type(t)=='table' and type(t.ontick)=='function','Timer needs ontick');local timer={interval=t.interval or 0.1,isRunning=false,_callback=t.ontick,_next=0}
 function timer:start()assert(self.interval>0,'Timer interval must be positive');self.isRunning=true;self._next=0 end
 function timer:stop()self.isRunning=false end
 timers[#timers+1]=timer;return timer
end
local function dialogSpec(d)
 local w={};for _,widget in ipairs(d._widgets)do local item={};for k,v in pairs(widget)do if type(v)~='function'then item[k]=v end end;item.id=widget.id or widget._hostId;w[#w+1]=item end
 return {id=d._id,title=d.title,widgets=w,data=d.data}
end
local function dispatch(d,response)
 if response.data then for k,v in pairs(response.data)do if type(d.data[k])=='table' and getmetatable(d.data[k]) and getmetatable(d.data[k]).__name=='Color' then v=Color(v)end;d.data[k]=v end end
 for _,w in ipairs(d._widgets)do if (w.id or w._hostId)==response.widget then local callback=w.onclick or w.onchange;if callback then callback(response.event or {})elseif w.kind=='button'then d:close() end;break end end
 if response.closed then d:close()end
end
function Dialog(t)
 if not nativeHost then return nil end
 t=type(t)=='table' and t or {title=t};local d={title=t.title or 'Extension',data={},bounds=Rectangle(0,0,300,200),_widgets={},_id=#dialogs+1,_visible=false,_onclose=t.onclose}
 for _,kind in ipairs{'label','entry','number','slider','check','radio','combobox','color','shades','button','separator','file'}do
  d[kind]=function(self,w)w=w or {};w.kind=kind;w._hostId="widget:"..(#self._widgets+1);self._widgets[#self._widgets+1]=w
   if w.id then self.data[w.id]=kind=='check' or kind=='radio' or kind=='button' and (w.selected or false) or kind=='color' and Color(w.color or {}) or kind=='shades' and (w.colors or {}) or kind=='slider' and (w.value or w.min or 0) or kind=='number' and tonumber(w.text or '0') or kind=='combobox' and (w.option or (w.options or {})[1] or '') or kind=='file' and (w.filename or '') or w.text or ''
    if kind=='check' or kind=='radio' or kind=='button' then self.data[w.id]=w.selected or false end
   end;return self end
 end
 function d:newrow()return self end
 function d:tab(t)return self:separator(t)end
 function d:endtabs()return self end
 function d:modify(w)for _,item in ipairs(self._widgets)do if item.id==w.id then for k,v in pairs(w)do item[k]=v end;if w.text and (item.kind=='entry' or item.kind=='number' or item.kind=='label')then self.data[w.id]=item.kind=='number' and tonumber(w.text) or w.text end;if w.value~=nil then self.data[w.id]=w.value end;if w.selected~=nil then self.data[w.id]=w.selected end end end;return self end
 function d:close()if self._visible then self._visible=false;if self._onclose then self._onclose()end end;return self end
 function d:repaint()return self end
 function d:show(options)
  options=options or {};self._visible=true
  if options.wait~=false then while self._visible do dispatch(self,host{op='dialog',dialog=dialogSpec(self)})end end
  return self
 end
 dialogs[#dialogs+1]=d;return d
end
rawset(app,'alert',function(t)if not nativeHost then print(type(t)=='table' and (t.text or t.title or '') or t);return 1 end;return host{op='alert',value=t}.button or 1 end)
rawset(app,'refresh',function()if nativeHost then host{op='refresh',document=doc,frame=activeFrame,layer=activeLayer,colors=scriptColors}end end)
_pw_loop=function()
 if not nativeHost then for _,timer in ipairs(timers)do assert(not timer.isRunning,'Timers require an interactive session')end;return end
 while true do
  local visible={};for _,d in ipairs(dialogs)do if d._visible then visible[#visible+1]=dialogSpec(d)end end
  local running=false;for _,t in ipairs(timers)do running=running or t.isRunning end
  if #visible==0 and not running and not appEvents:_hasHandlers() and not spriteEvents:_hasHandlers() then break end
  local response=host{op='pump',dialogs=visible,document=doc,frame=activeFrame,layer=activeLayer,colors=scriptColors}
  if response.editor then
   local editor=response.editor;doc=editor.document;activeFrame=editor.frame or activeFrame;activeLayer=editor.layer or activeLayer;rebuildHandles();spriteEvents:_emit('change',{fromUndo=false});appEvents:_emit('sitechange')
  end
  if response.cancelled then break end
  if response.dialog then dispatch(dialogs[response.dialog],response) end
  local now=response.time or 0
  for _,timer in ipairs(timers)do if timer.isRunning then if timer._next==0 then timer._next=now+timer.interval elseif now>=timer._next then timer._next=now+timer.interval;timer._callback()end end end
 end
 host{op='preferences',selection=spriteSelection.bounds,preferences=preferences,pluginPreferences=_pw_plugin.preferences,colors=scriptColors,frame=activeFrame,layer=activeLayer}
end
_pw_plugin.preferences=copy((_pw_options or {}).pluginPreferences or {})

-- Package-local Lua modules use the same bounded runtime and instruction hook.
local modules={}
function loadfile(path)
 if not nativeHost then return nil,'Modules require a file-backed script session' end
 local ok,response=pcall(host,{op='readScript',path=path})
 if not ok then return nil,response end
 return load(response.source,'@'..path,'t',_G)
end
function dofile(path)local fn,err=loadfile(path);assert(fn,err);return fn()end
function require(name)
 if modules[name]~=nil then return modules[name] end
 assert(type(name)=='string' and not name:find('%.%.'),'Invalid module name')
 local fn,err=loadfile(name:gsub('%.','/')..'.lua');assert(fn,err)
 modules[name]=true;local result=fn(name);if result~=nil then modules[name]=result end;return modules[name]
end
