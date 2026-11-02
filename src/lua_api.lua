-- PixelCraft's supported native scripting API. Raw project data stays separate
-- from API handles, so metatables and callbacks never enter project persistence.
local clock,hook,fail=os.clock,debug.sethook,error
local raw_pcall,raw_xpcall=pcall,xpcall
local start,instructions,expired=clock(),0,false
hook(function() instructions=instructions+10000;if instructions>20000000 or clock()-start>3 then expired=true;fail('Script instruction/time budget exceeded') end end,'',10000)
pcall=function(...)local result=table.pack(raw_pcall(...));if expired then fail('Script instruction/time budget exceeded') end;return table.unpack(result,1,result.n)end
xpcall=function(...)local result=table.pack(raw_xpcall(...));if expired then fail('Script instruction/time budget exceeded') end;return table.unpack(result,1,result.n)end
io=nil;os=nil;package=nil;require=nil;dofile=nil;loadfile=nil;debug=nil;collectgarbage=nil;coroutine=nil
local function copy(t) if type(t)~='table' then return t end local c={} for k,v in pairs(t) do c[k]=copy(v) end return c end
local function byte(n) return math.max(0,math.min(255,math.floor(n or 0))) end
local function rgba(r,g,b,a) return byte(r)|(byte(g)<<8)|(byte(b)<<16)|(byte(a==nil and 255 or a)<<24) end
local function unpackColor(c)
 if type(c)=='table' then return {byte(c.r or c.red or c[1]),byte(c.g or c.green or c[2]),byte(c.b or c.blue or c[3]),byte(c.a or c.alpha or c[4] or 255)} end
 c=math.tointeger(c or 0) or 0;return {c&255,(c>>8)&255,(c>>16)&255,(c>>24)&255}
end
function Color(c,g,b,a) local p=type(c)=='number' and (g and {c,g,b,a or 255} or unpackColor(c)) or unpackColor(c or {});return {r=p[1],g=p[2],b=p[3],a=p[4],red=p[1],green=p[2],blue=p[3],alpha=p[4],rgbaPixel=rgba(table.unpack(p))} end
function Point(x,y) if type(x)=='table' then return {x=x.x or x[1] or 0,y=x.y or x[2] or 0} end return {x=x or 0,y=y or 0} end
function Size(w,h) return {width=w or 0,height=h or 0} end
function Rectangle(x,y,w,h) if type(x)=='table' then return copy(x) end return {x=x or 0,y=y or 0,width=w or 0,height=h or 0} end
ColorMode={RGB='Rgba',GRAY='Grayscale',INDEXED='Indexed'}
BlendMode={NORMAL='Normal',MULTIPLY='Multiply',SCREEN='Screen',OVERLAY='Overlay',DARKEN='Darken',LIGHTEN='Lighten',COLOR_DODGE='ColorDodge',COLOR_BURN='ColorBurn',HARD_LIGHT='HardLight',SOFT_LIGHT='SoftLight',DIFFERENCE='Difference',EXCLUSION='Exclusion',HSL_HUE='Hue',HSL_SATURATION='Saturation',HSL_COLOR='Color',HSL_LUMINOSITY='Luminosity',ADDITION='Addition',SUBTRACT='Subtract',DIVIDE='Divide'}
AniDir={FORWARD=0,REVERSE=1,PING_PONG=2,PING_PONG_REVERSE=3}
local doc=_pw_document
local imageMethods={}
local function image(p,w,h) return setmetatable({_pixels=p,width=w,height=h,colorMode=doc.color_mode,bytesPerPixel=4},{__index=imageMethods}) end
function Image(w,h,mode)
 if type(w)=='table' and w._pixels then return image(copy(w._pixels),w.width,w.height) end
 if type(w)=='table' then h=w.height;w=w.width end
 assert(type(w)=='number' and type(h)=='number' and w>0 and h>0 and w*h<=16000000,'Image dimensions exceed budget')
 local p={} for i=1,w*h do p[i]={0,0,0,0} end return image(p,w,h)
end
function imageMethods:drawPixel(x,y,c) x=math.floor(x);y=math.floor(y);if x>=0 and y>=0 and x<self.width and y<self.height then self._pixels[y*self.width+x+1]=unpackColor(c) end end
function imageMethods:getPixel(x,y) if x<0 or y<0 or x>=self.width or y>=self.height then return 0 end return rgba(table.unpack(self._pixels[y*self.width+x+1])) end
function imageMethods:clear(c) for i=1,#self._pixels do self._pixels[i]=unpackColor(c or 0) end end
function imageMethods:clone() return Image(self) end
-- doc::rgba_blender_normal (0x5e81e0): rounded byte products, signed
-- channel division truncated toward zero. Keep transparent branch ordering.
local function mulUn8(a,b) local t=a*b+128;return (t+(t>>8))>>8 end
local function normalOver(dst,src,opacity)
 local s=mulUn8(src[4],opacity)
 if dst[4]==0 then return {src[1],src[2],src[3],s} end
 if src[4]==0 then return dst end
 local a=s+dst[4]-mulUn8(s,dst[4]);local result={0,0,0,a}
 for i=1,3 do result[i]=dst[i]+math.modf((src[i]-dst[i])*s/a) end
 return result
end
function imageMethods:drawImage(src,pos,opacity)
 pos=pos or Point();opacity=byte(opacity or 255)
 for y=0,src.height-1 do for x=0,src.width-1 do
  local packed=src:getPixel(x,y)
  -- blend_image_templ<RgbTraits,RgbTraits> (0x5eff40) skips mask zero.
  if packed~=0 then
   local p=unpackColor(packed);local dst=unpackColor(self:getPixel(x+pos.x,y+pos.y))
   self:drawPixel(x+pos.x,y+pos.y,rgba(table.unpack(normalOver(dst,p,opacity))))
  end
 end end
end
function imageMethods:pixels(rect) rect=rect or Rectangle(0,0,self.width,self.height);local x,y=rect.x-1,rect.y;return function() x=x+1;if x>=rect.x+rect.width then x=rect.x;y=y+1 end;if y>=rect.y+rect.height then return nil end;return setmetatable({x=x,y=y},{__call=function(_,v)if v==nil then return self:getPixel(x,y) else self:drawPixel(x,y,v) end end}) end end
local sprite,layerHandles,frameHandles={}, {}, {}
local function frameNumber(f) return type(f)=='table' and f.frameNumber or tonumber(f) end
local function layerNumber(l) return type(l)=='table' and l._index or tonumber(l) end
local function cel(li,fi)
 local l=assert(doc.layers[li],'Missing layer');assert(doc.durations[fi],'Missing frame');local m=l.cel_meta[fi]
 return setmetatable({},{__index=function(_,k)if k=='image' then return image(l.cels[fi],doc.width,doc.height) elseif k=='opacity' then return m.opacity elseif k=='position' then return Point() elseif k=='frame' then return frameHandles[fi] elseif k=='layer' then return layerHandles[li] end end,__newindex=function(_,k,v)if k=='opacity' then m.opacity=byte(v) elseif k=='image' then assert(v.width==doc.width and v.height==doc.height,'Replacement image must match sprite');l.cels[fi]=copy(v._pixels) elseif k=='position' then error('Moving cels is not supported by the native API') else error('Unsupported cel field: '..k) end end})
end
local function rebuildHandles()
 layerHandles={} for i,l in ipairs(doc.layers) do layerHandles[i]=setmetatable({_index=i},{__index=function(_,k)if k=='name' then return l.name elseif k=='opacity' then return l.opacity elseif k=='isVisible' then return l.visible elseif k=='isEditable' then return not l.locked elseif k=='isGroup' then return l.group elseif k=='blendMode' then return l.blend elseif k=='sprite' then return sprite elseif k=='cels' then local c={} for f=1,#doc.durations do c[f]=cel(i,f) end return c elseif k=='cel' then return function(_,f)return cel(i,frameNumber(f))end elseif k=='parent' then return l.parent and layerHandles[l.parent+1] or sprite end end,__newindex=function(_,k,v)if k=='name' then l.name=tostring(v) elseif k=='opacity' then l.opacity=byte(v) elseif k=='isVisible' then l.visible=not not v elseif k=='isEditable' then l.locked=not v elseif k=='blendMode' then l.blend=v elseif k=='parent' then if v==sprite then l.parent=nil else l.parent=layerNumber(v)-1 end else error('Unsupported layer field: '..k) end end}) end
 frameHandles={} for i in ipairs(doc.durations) do frameHandles[i]=setmetatable({frameNumber=i},{__index=function(_,k)if k=='duration' then return doc.durations[i]/1000 elseif k=='sprite' then return sprite end end,__newindex=function(_,k,v)if k=='duration' then doc.durations[i]=math.max(1,math.min(65535,math.floor(v*1000))) else error('Unsupported frame field: '..k) end end}) end
end
local palette={}
function palette:getColor(i) return Color(doc.palette[i+1]) end
function palette:setColor(i,c)
 assert(doc.palette[i+1],'Palette index out of range');local old=doc.palette[i+1];local p=unpackColor(c);doc.palette[i+1]=p
 if doc.color_mode=='Indexed' then for _,l in ipairs(doc.layers) do for f,pixels in ipairs(l.cels) do for n,v in ipairs(pixels) do local stored=l.cel_meta[f].indices;local matches=stored and stored[n]==i or not stored and v[1]==old[1] and v[2]==old[2] and v[3]==old[3] and v[4]==old[4];if matches then pixels[n]=copy(p);if i==doc.transparent_index and not l.background then pixels[n][4]=0 end end end end end end
end
function palette:resize(n) assert(n>=1 and n<=256,'Palette must contain 1–256 colors');while #doc.palette>n do table.remove(doc.palette) end while #doc.palette<n do table.insert(doc.palette,{0,0,0,255}) end end
setmetatable(palette,{__len=function()return #doc.palette end})
local spriteMethods={}
function spriteMethods:newCel(l,f,im,pos) local li,fi=layerNumber(l),frameNumber(f);local c=cel(li,fi);if im then local target=c.image;target:clear();target:drawImage(im,pos or Point()) end return c end
function spriteMethods:deleteCel(c) c.image:clear() end
function spriteMethods:newLayer() assert((#doc.layers+1)*#doc.durations*doc.width*doc.height<=64000000,'Layer budget exceeded');local l={name='Layer '..(#doc.layers+1),visible=true,locked=false,opacity=255,cels={},cel_meta={},group=false,blend='Normal',tile_cels={}};for f=1,#doc.durations do l.cels[f]=Image(doc.width,doc.height)._pixels;l.cel_meta[f]={opacity=255,z_index=0,outside={}};l.tile_cels[f]={} end table.insert(doc.layers,l);rebuildHandles();return layerHandles[#doc.layers] end
function spriteMethods:newGroup() local l=self:newLayer();doc.layers[l._index].group=true;return l end
function spriteMethods:newFrame(f)
 assert((#doc.durations+1)*#doc.layers*doc.width*doc.height<=64000000,'Frame budget exceeded')
 local explicit=f~=nil
 f=frameNumber(f) or #doc.durations
 assert(f%1==0 and f>=1 and f<=#doc.durations,'Missing frame')
 -- Sprite_newFrame (0x3f3b10) -> DocApi::addFrame (0x19f0b0):
 -- copy the preceding frame into the insertion slot, shifting later frames.
 table.insert(doc.durations,f+1,doc.durations[f])
 for _,l in ipairs(doc.layers) do
  table.insert(l.cels,f+1,copy(l.cels[f]))
  table.insert(l.cel_meta,f+1,copy(l.cel_meta[f]))
  table.insert(l.tile_cels,f+1,copy(l.tile_cels[f]))
 end
 -- Keep PixelCraft's parallel document data aligned with the inserted frame.
 if #doc.frame_palettes>0 then table.insert(doc.frame_palettes,f+1,copy(doc.frame_palettes[f] or doc.palette)) end
 for _,t in ipairs(doc.tags) do if t.from>=f then t.from=t.from+1 end;if t.to>=f-1 then t.to=t.to+1 end end
 for _,m in ipairs(doc.metadata) do if m.frame>=f then m.frame=m.frame+1 end end
 rebuildHandles()
 -- The native explicit-argument path pushes the source index, not source+1.
 return frameHandles[explicit and f or #doc.durations]
end
function spriteMethods:newEmptyFrame(f)
 assert((#doc.durations+1)*#doc.layers*doc.width*doc.height<=64000000,'Frame budget exceeded')
 -- Sprite_newEmptyFrame 0x3f3d80 passes the requested slot unchanged,
 -- unlike newFrame's insert-after path. No argument means append.
 f=frameNumber(f) or (#doc.durations+1)
 assert(f%1==0 and f>=1 and f<=#doc.durations+1,'Missing frame')
 local previous=math.max(1,f-1)
 table.insert(doc.durations,f,doc.durations[previous])
 for _,l in ipairs(doc.layers) do
  local tiles={} for i=1,#(l.tile_cels[previous] or {}) do tiles[i]=0 end
  table.insert(l.cels,f,Image(doc.width,doc.height)._pixels)
  table.insert(l.cel_meta,f,{opacity=255,z_index=0,outside={}})
  table.insert(l.tile_cels,f,tiles)
 end
 if #doc.frame_palettes>0 then table.insert(doc.frame_palettes,f,copy(doc.frame_palettes[previous] or doc.palette)) end
 -- DocApi::adjustTags 0x19f2d0, insertion with TagsHandling=1.
 for _,t in ipairs(doc.tags) do if t.from>=f-1 then t.from=t.from+1 end;if t.to>=f-2 then t.to=t.to+1 end end
 for _,m in ipairs(doc.metadata) do if m.frame>=f-1 then m.frame=m.frame+1 end end
 rebuildHandles()
 return frameHandles[f]
end
function spriteMethods:newTag(a,b) a=frameNumber(a);b=frameNumber(b);assert(a and b and a>=1 and b>=a and b<=#doc.durations,'Invalid tag range');local t={name='Tag',from=a-1,to=b-1,direction=0,color={128,180,255}};table.insert(doc.tags,t);return setmetatable({},{__index=function(_,k)if k=='name' then return t.name elseif k=='fromFrame' then return frameHandles[t.from+1] elseif k=='toFrame' then return frameHandles[t.to+1] elseif k=='aniDir' then return t.direction end end,__newindex=function(_,k,v)if k=='name' then t.name=tostring(v) elseif k=='aniDir' then t.direction=v elseif k=='fromFrame' then t.from=frameNumber(v)-1 elseif k=='toFrame' then t.to=frameNumber(v)-1 else error('Unsupported tag field: '..k) end end}) end
setmetatable(sprite,{__index=function(_,k)if spriteMethods[k] then return spriteMethods[k] elseif k=='width' or k=='height' then return doc[k] elseif k=='layers' then return layerHandles elseif k=='frames' then return frameHandles elseif k=='palettes' then return {palette} elseif k=='colorMode' then return doc.color_mode elseif k=='filename' then return '' elseif k=='cels' then local c={} for i,l in ipairs(doc.layers) do if not l.group then for f=1,#doc.durations do c[#c+1]=cel(i,f) end end end return c end end,__newindex=function(_,k,v)error('Unsupported sprite assignment: '..k)end})
rebuildHandles()
local activeLayer,activeFrame=_pw_layer+1,_pw_frame+1
local commands={}
app=setmetatable({apiVersion=1,version='PixelCraft 0.2',isUIAvailable=false,pixelColor={rgba=rgba,rgbaR=function(c)return c&255 end,rgbaG=function(c)return(c>>8)&255 end,rgbaB=function(c)return(c>>16)&255 end,rgbaA=function(c)return(c>>24)&255 end},params={},transaction=function(a,b)return (b or a)() end,refresh=function()end,alert=function(t)print(type(t)=='table' and (t.text or t.title or '') or t)end},{__index=function(_,k)if k=='activeSprite' then return sprite elseif k=='activeLayer' then return layerHandles[activeLayer] elseif k=='activeFrame' then return frameHandles[activeFrame] elseif k=='activeCel' then return cel(activeLayer,activeFrame) elseif k=='activeImage' then return cel(activeLayer,activeFrame).image elseif k=='sprites' then return {sprite} elseif k=='fgColor' then return Color{r=0,g=0,b=0,a=255} elseif k=='bgColor' then return Color{r=255,g=255,b=255,a=255} else error('Unsupported app API: '..k) end end,__newindex=function(_,k,v)if k=='activeLayer' then activeLayer=assert(layerNumber(v)) elseif k=='activeFrame' then activeFrame=assert(frameNumber(v)) else error('Unsupported app assignment: '..k) end end})
rawset(app,'command',setmetatable({NewLayer=function()app.activeLayer=sprite:newLayer()end,NewFrame=function()app.activeFrame=sprite:newFrame()end,NewEmptyFrame=function()app.activeFrame=sprite:newEmptyFrame()end},{__index=function(_,k)return function()error('Unsupported command: '..k)end end}))
_pw_plugin={preferences={}}
function _pw_plugin:newCommand(c) assert(type(c.id)=='string' and type(c.onclick)=='function','Command requires id and onclick');commands[c.id]=c end
function _pw_plugin:newMenuGroup(_) end
function _pw_plugin:newMenuSeparator(_) end
function _pw_finish(command,discover)
 if type(init)=='function' then init(_pw_plugin) end
 local list={} for id,c in pairs(commands) do list[#list+1]={id=id,title=c.title or id} end
 if command and command~='' then assert(commands[command],'Plugin command was not registered: '..command);commands[command].onclick() end
 hook()
 local function escape(s) return '"'..s:gsub('[%z\1-\31\\"]',function(c)return string.format('\\u%04x',string.byte(c))end)..'"' end
 local function json(v) local t=type(v);if t=='nil' then return 'null' elseif t=='boolean' or t=='number' then return tostring(v) elseif t=='string' then return escape(v) elseif t=='table' then local array=true;local n=0;for k in pairs(v)do if type(k)~='number' then array=false end;n=n+1 end;local out={};if array then for i=1,#v do out[i]=json(v[i]) end return '['..table.concat(out,',')..']' end;for k,val in pairs(v)do out[#out+1]=escape(k)..':'..json(val) end;return '{'..table.concat(out,',')..'}' else error('Cannot serialize '..t) end end
 _pw_result=json(discover and list or doc)
end
