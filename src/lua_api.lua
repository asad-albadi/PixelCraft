-- PixelCraft's supported native scripting API. Raw project data stays separate
-- from API handles, so metatables and callbacks never enter project persistence.
local clock,hook,fail=os.clock,debug.sethook,error
local raw_pcall,raw_xpcall=pcall,xpcall
local start,instructions,expired=clock(),0,false
hook(function() instructions=instructions+10000;if instructions>20000000 or clock()-start>3 then expired=true;fail('Script instruction/time budget exceeded') end end,'',10000)
pcall=function(...)local result=table.pack(raw_pcall(...));if expired then fail('Script instruction/time budget exceeded') end;return table.unpack(result,1,result.n)end
xpcall=function(...)local result=table.pack(raw_xpcall(...));if expired then fail('Script instruction/time budget exceeded') end;return table.unpack(result,1,result.n)end
io=nil;os={clock=clock,time=os.time,date=os.date,difftime=os.difftime};package=nil;require=nil;dofile=nil;loadfile=nil;debug=nil;collectgarbage=nil;coroutine=nil
local function copy(t) if type(t)~='table' then return t end local c={} for k,v in pairs(t) do c[k]=copy(v) end return c end
local function byte(n) return math.max(0,math.min(255,math.floor(n or 0))) end
-- Original opacity setters call lua_tointegerx in floor mode. Failed
-- conversion leaves its zero-initialized result; retain our byte storage
-- boundary for successful values outside the supported 0..255 domain.
local function opacityByte(v)
 local n=tonumber(v)
 if not n or n~=n or n < -9223372036854775808.0 or n >= 9223372036854775808.0 then return 0 end
 return byte(n)
end
local function rgba(r,g,b,a) return byte(r)|(byte(g)<<8)|(byte(b)<<16)|(byte(a==nil and 255 or a)<<24) end
-- Native PixelColor_rgba 0x3e2100 and extractors 0x3e21a0..0x3e2230
-- call lua_tointegerx (floor mode, failed conversion -> zero) then mask.
local function pixelInteger(v)
 local n=tonumber(v)
 if not n then return 0 end
 if math.type(n)=='integer' then return n end
 if n~=n or n < -9223372036854775808.0 or n >= 9223372036854775808.0 then return 0 end
 return math.floor(n)
end
local function pixelRgba(r,g,b,a)
 return (pixelInteger(r)&255)|((pixelInteger(g)&255)<<8)|((pixelInteger(b)&255)<<16)|(((a==nil and 255 or pixelInteger(a))&255)<<24)
end
local function colorInteger(v)
 local n=pixelInteger(v)&0xffffffff
 return n>=0x80000000 and n-0x100000000 or n
end
-- Color_new's gray table branch (0x3c5abb..0x3c5acc) uses
-- lua_tonumberx then CVTTSD2SI, unlike the integer-based gray setter.
local function constructorGray(v)
 local n=(tonumber(v) or 0)+0.0
 if n~=n or n>=2147483648 or n<=-2147483649 then return -2147483648 end
 return math.modf(n)
end
local function unpackColor(c)
 if type(c)=='table' then return {byte(c.r or c.red or c[1]),byte(c.g or c.green or c[2]),byte(c.b or c.blue or c[3]),byte(c.a or c.alpha or c[4] or 255)} end
 c=math.tointeger(c or 0) or 0;return {c&255,(c>>8)&255,(c>>16)&255,(c>>24)&255}
end
local doc=_pw_document
local colorChannels={r=1,red=1,g=2,green=2,b=3,blue=3,a=4,alpha=4}
local function hsvToRgb(h,s,v)
 h=(h%360)/60;s=math.max(0,math.min(1,s));v=math.max(0,math.min(1,v))
 local c=v*s;local x=c*(1-math.abs(h%2-1));local m=v-c
 local t= h<1 and {c,x,0} or h<2 and {x,c,0} or h<3 and {0,c,x} or h<4 and {0,x,c} or h<5 and {x,0,c} or {c,0,x}
 return {byte((t[1]+m)*255+0.5),byte((t[2]+m)*255+0.5),byte((t[3]+m)*255+0.5),255}
end
-- gfx::Hsl(double,double,double) 0x691370 and gfx::Rgb(Hsl) 0x6918e0.
local function hslToRgb(h,s,l)
 s=math.max(0,math.min(1,s));l=math.max(0,math.min(1,l))
 local c=(1-math.abs(2*l-1))*s;local hp=h/60
 local x=c*(1-math.abs(math.fmod(hp,2)-1));local sector=constructorGray(hp)
 local t=(sector==0 or sector==6) and {c,x,0} or sector==1 and {x,c,0} or sector==2 and {0,c,x} or sector==3 and {0,x,c} or sector==4 and {x,0,c} or sector==5 and {c,0,x} or {0,0,0}
 local m=l-c*0.5
 return {constructorGray((t[1]+m)*255+0.5),constructorGray((t[2]+m)*255+0.5),constructorGray((t[3]+m)*255+0.5),255}
end
local function rgbToHsv(p)
 local r,g,b=p[1]/255,p[2]/255,p[3]/255;local hi,lo=math.max(r,g,b),math.min(r,g,b);local d=hi-lo
 local h=d==0 and 0 or hi==r and ((g-b)/d)%6 or hi==g and (b-r)/d+2 or (r-g)/d+4
 return h*60,hi==0 and 0 or d/hi,hi,(hi+lo)/2,d
end
local function rgbToHsl(p)
 local hi,lo=math.max(p[1],p[2],p[3]),math.min(p[1],p[2],p[3])
 local d=colorInteger(hi-lo)/255.0
 local l=(colorInteger(hi+lo)/255.0)*0.5
 if d==0 then return 0,0,l end
 local r,g,b=p[1]/255.0,p[2]/255.0,p[3]/255.0
 local h=hi==p[1] and ((g-b)/d)%6 or hi==p[2] and (b-r)/d+2 or (r-g)/d+4
 return h*60,d/(1-math.abs(2*l-1)),l
end
-- Color::getGray 0x16c2b0 uses Hsl(Rgb).lightness, not sprite luma.
-- Match the native signed32 sum and double division/multiplication order.
local function rgbGray(p)
 local sum=colorInteger(math.max(p[1],p[2],p[3])+math.min(p[1],p[2],p[3]))
 local n=(sum/255.0)*0.5*255.0
 return n<0 and math.ceil(n) or math.floor(n)
end
-- Native Color_new 0x3c54f6..0x3c5509 copies the complete Color value.
-- Keep private state so copying preserves gray/index representation as well
-- as channels, without sharing mutable storage between Color objects.
local colorStates=setmetatable({}, {__mode='k'})
-- Color_eq 0x3c5c10 -> Color::operator== 0x16c3f0 compares tags
-- before payloads. Packing to bytes would lose stored signed32 values.
local function colorEqual(x,y)
 local xs,ys=colorStates[x],colorStates[y]
 if not xs or not ys then return false end
 local xi,xg,xh=xs();local yi,yg,yh=ys()
 if xh or yh then return xh~=nil and yh~=nil and math.abs(xh[1]-yh[1])<0.001 and math.abs(xh[2]-yh[2])<0.00001 and math.abs(xh[3]-yh[3])<0.00001 and x.alpha==y.alpha end
 if xi~=nil or yi~=nil then return xi~=nil and yi~=nil and xi==yi end
 if xg~=nil or yg~=nil then return xg~=nil and yg~=nil and xg==yg and x.alpha==y.alpha end
 return x.red==y.red and x.green==y.green and x.blue==y.blue and x.alpha==y.alpha
end
function Color(c,g,b,a)
 local index,gray,hsl
 if type(c)=='string' then
  local hex=c:gsub('^#','');assert(#hex==6 or #hex==8,'Color requires #RRGGBB or #RRGGBBAA')
  c={tonumber(hex:sub(1,2),16),tonumber(hex:sub(3,4),16),tonumber(hex:sub(5,6),16),#hex==8 and tonumber(hex:sub(7,8),16) or 255}
 end
 -- Color_new 0x3c54a0 selects short or long RGB fields as a family.
 -- fromRgb 0x16a5c0 stores signed32 channels without saturation.
 local p
 if type(c)=='table' and getmetatable(c) and getmetatable(c).__name=='Color' then
  p={c.red,c.green,c.blue,c.alpha}
  local state=colorStates[c]
  if state then index,gray,hsl=state();if hsl then hsl=copy(hsl) end end
 elseif type(c)=='table' and c.r~=nil then
  p={colorInteger(c.r),colorInteger(c.g),colorInteger(c.b),c.a==nil and 255 or colorInteger(c.a)}
 elseif type(c)=='table' and c.red~=nil then
  p={colorInteger(c.red),colorInteger(c.green),colorInteger(c.blue),c.alpha==nil and 255 or colorInteger(c.alpha)}
 elseif type(c)=='table' and (c.v~=nil or c.value~=nil) then
  -- Color_new 0x3c575e..0x3c58ec selects HSV by nonnil value,
  -- before HSL/gray/index, and keeps short/long field families separate.
  local h,s,v,alpha
  if c.v~=nil then h,s,v,alpha=c.h,c.s,c.v,c.a
  else h,s,v,alpha=c.hue,c.saturation,c.value,c.alpha end
  p=hsvToRgb((tonumber(h) or 0)+0.0,(tonumber(s) or 0)+0.0,(tonumber(v) or 0)+0.0)
  p[4]=alpha==nil and 255 or colorInteger(alpha)
 elseif type(c)=='table' and (c.l~=nil or c.lightness~=nil) then
  -- Color_new 0x3c5903..0x3c5a11 selects HSL by lightness,
  -- after HSV value and before gray/index. Never mix field families.
  local h,s,l,alpha
  if c.l~=nil then h,s,l,alpha=c.h,c.s,c.l,c.a
  else h,s,l,alpha=c.hue,c.saturation,c.lightness,c.alpha end
  hsl={(tonumber(h) or 0)+0.0,(tonumber(s) or 0)+0.0,(tonumber(l) or 0)+0.0}
  p=hslToRgb(hsl[1],hsl[2],hsl[3]);p[4]=alpha==nil and 255 or colorInteger(alpha)
 else
  p=type(c)=='number' and (g and {c,g,b,a or 255} or unpackColor(c)) or unpackColor(c or {})
 end
 if type(c)=='table' and not (getmetatable(c) and getmetatable(c).__name=='Color') then
  if c.r~=nil or c.red~=nil then -- RGB has priority over other table forms.
  elseif c.v~=nil or c.value~=nil or hsl then -- Already constructed above.
  -- Color_new 0x3c5a35 tests nonnil gray before index at 0x3c5a82.
  elseif c.gray~=nil then gray=constructorGray(c.gray);p={gray,gray,gray,c.alpha==nil and 255 or colorInteger(c.alpha)}
  -- Color_new 0x3c5a95 -> fromIndex 0x16a630 stores signed32(I(v)).
  elseif c.index~=nil then index=colorInteger(c.index);p=copy((doc.palette or {})[index+1] or {0,0,0,255})
  end
 end
 local result=setmetatable({}, {
  __name='Color',
  __eq=colorEqual,
  __index=function(_,k)
   local i=colorChannels[k];if i then return p[i] end
   local h,sat,v,l,d=rgbToHsv(p)
   if k=='rgbaPixel' then return pixelRgba(table.unpack(p))
   elseif k=='grayPixel' then return (pixelInteger(gray or (hsl and constructorGray(hsl[3]*255.0) or rgbGray(p)))&255)|((pixelInteger(p[4])&255)<<8)
   elseif k=='index' then
    if index then return index end;local best,dist=0,math.huge
    for n,c in ipairs(doc.palette) do local e=0;for j=1,4 do e=e+(p[j]-c[j])^2 end;if e<dist then best,dist=n-1,e end end;return best
   elseif k=='gray' then return gray or (hsl and constructorGray(hsl[3]*255.0) or rgbGray(p))
   elseif k=='hslHue' then return hsl and hsl[1] or h
   elseif k=='hue' then return hsl and hsl[1] or h
   elseif k=='hsvHue' then return h
   elseif k=='saturation' then return hsl and hsl[2] or sat
   elseif k=='hsvSaturation' then return sat
   elseif k=='value' or k=='hsvValue' then return v
   elseif k=='lightness' or k=='hslLightness' then return hsl and hsl[3] or l
   elseif k=='hslSaturation' then return hsl and hsl[2] or (d==0 and 0 or d/(1-math.abs(2*l-1))) end
  end,
  __newindex=function(t,k,v)
   local i=colorChannels[k]
   if i then local n=colorInteger(v);p[i]=i==4 and byte(n) or n;index=nil;if i~=4 then gray=nil;hsl=nil end
   -- Color_set_index 0x3c6860 replaces the represented value via fromIndex.
   elseif k=='index' then index=colorInteger(v);gray=nil;hsl=nil;p=copy(doc.palette[index+1] or {0,0,0,255})
   -- Color_set_gray 0x3c6920 -> fromGray 0x16a620 stores signed32,
   -- preserves alpha and replaces the representation (no byte clamp).
   elseif k=='gray' then gray=colorInteger(v);p={gray,gray,gray,p[4]};index=nil;hsl=nil
   elseif k=='rgbaPixel' or k=='grayPixel' then error('Color.'..k..' is read-only')
   elseif k=='hslHue' or k=='hslSaturation' or k=='hslLightness' or k=='lightness' or (hsl and (k=='hue' or k=='saturation')) then
    -- Native setters 0x3c6270/0x3c6360/0x3c6450 replace the
    -- full value through fromHsl 0x16a600; retain its doubles.
    hsl=hsl or (gray~=nil and {0,0,gray/255.0} or {rgbToHsl(p)})
    hsl[(k=='hslHue' or k=='hue') and 1 or (k=='hslSaturation' or k=='saturation') and 2 or 3]=(tonumber(v) or 0)+0.0
    local alpha=p[4];p=hslToRgb(hsl[1],hsl[2],hsl[3]);p[4]=alpha;index=nil;gray=nil
   elseif k=='hue' or k=='hsvHue' or k=='hsvSaturation' or k=='saturation' or k=='value' or k=='hsvValue' then
    -- Native HSV setters 0x3c5fa0/0x3c6090/0x3c6180 replace
    -- the complete value via fromHsv 0x16a5e0, discarding gray/index.
    -- lua_tonumberx 0x53e9c0 accepts numeric strings; failed
    -- conversion (including nil/boolean/table) supplies double zero.
    v=(tonumber(v) or 0)+0.0
    local h,sat,val=rgbToHsv(p);if k=='hue' or k=='hsvHue' then h=v elseif k=='saturation' or k=='hsvSaturation' then sat=v else val=v end;local alpha=p[4];p=hsvToRgb(h,sat,val);p[4]=alpha;index=nil;gray=nil;hsl=nil
   else rawset(t,k,v) end
  end
 })
 colorStates[result]=function()return index,gray,hsl end
 return result
end
function Point(x,y) if type(x)=='table' then return {x=x.x or x[1] or 0,y=x.y or x[2] or 0} end return {x=x or 0,y=y or 0} end
function Size(w,h) if type(w)=='table' then return {width=w.width or w[1] or 0,height=w.height or w[2] or 0} end return {width=w or 0,height=h or 0} end
function Rectangle(x,y,w,h) if type(x)=='table' then return copy(x) end return {x=x or 0,y=y or 0,width=w or 0,height=h or 0} end
ColorMode={RGB='Rgba',GRAY='Grayscale',INDEXED='Indexed'}
BlendMode={NORMAL='Normal',MULTIPLY='Multiply',SCREEN='Screen',OVERLAY='Overlay',DARKEN='Darken',LIGHTEN='Lighten',COLOR_DODGE='ColorDodge',COLOR_BURN='ColorBurn',HARD_LIGHT='HardLight',SOFT_LIGHT='SoftLight',DIFFERENCE='Difference',EXCLUSION='Exclusion',HSL_HUE='Hue',HSL_SATURATION='Saturation',HSL_COLOR='Color',HSL_LUMINOSITY='Luminosity',ADDITION='Addition',SUBTRACT='Subtract',DIVIDE='Divide'}
AniDir={FORWARD=0,REVERSE=1,PING_PONG=2,PING_PONG_REVERSE=3}
-- Native App_get_fgColor 0x50ac90 / App_get_bgColor 0x50ae00 copy
-- the preference Color into fresh Lua userdata on every read.
local scriptColors=_pw_colors
local imageMethods={}
local function image(p,w,h) return setmetatable({_pixels=p,width=w,height=h,colorMode=doc.color_mode,bytesPerPixel=doc.color_mode=='Rgba' and 4 or doc.color_mode=='Grayscale' and 2 or 1,spec={width=w,height=h,colorMode=doc.color_mode,transparentColor=doc.transparent_index}},{__index=imageMethods}) end
function Image(w,h,mode)
 if type(w)=='table' and w._pixels then return image(copy(w._pixels),w.width,w.height) end
 if type(w)=='table' then h=w.height;w=w.width end
 assert(type(w)=='number' and type(h)=='number' and w>0 and h>0 and w*h<=16000000,'Image dimensions exceed budget')
 local p={} for i=1,w*h do p[i]={0,0,0,0} end return image(p,w,h)
end
function imageMethods:drawPixel(x,y,c) x=math.floor(x);y=math.floor(y);if x>=0 and y>=0 and x<self.width and y<self.height then local value=unpackColor(c);self._pixels[y*self.width+x+1]=value;if self._put then self._put(x,y,value)end end end
function imageMethods:getPixel(x,y) if x<0 or y<0 or x>=self.width or y>=self.height then return 0 end return rgba(table.unpack(self._pixels[y*self.width+x+1])) end
function imageMethods:clear(c) for y=0,self.height-1 do for x=0,self.width-1 do self:drawPixel(x,y,c or 0)end end end
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
  local packed=rgba(table.unpack(src._pixels[y*src.width+x+1]))
  -- blend_image_templ<RgbTraits,RgbTraits> (0x5eff40) skips mask zero.
  if packed~=0 then
   local p=src._pixels[y*src.width+x+1];local dx,dy=x+pos.x,y+pos.y;local dst=(dx>=0 and dy>=0 and dx<self.width and dy<self.height) and self._pixels[dy*self.width+dx+1] or {0,0,0,0}
   self:drawPixel(x+pos.x,y+pos.y,normalOver(dst,p,opacity))
  end
 end end
end
function imageMethods:pixels(rect) rect=rect or Rectangle(0,0,self.width,self.height);local x,y=rect.x-1,rect.y;return function() x=x+1;if x>=rect.x+rect.width then x=rect.x;y=y+1 end;if y>=rect.y+rect.height then return nil end;return setmetatable({x=x,y=y},{__call=function(_,v)if v==nil then return self:getPixel(x,y) else self:drawPixel(x,y,v) end end}) end end
local sprite,layerHandles,frameHandles={}, {}, {}
local function frameNumber(f) return type(f)=='table' and f.frameNumber or tonumber(f) end
local function layerNumber(l) return type(l)=='table' and l._index or tonumber(l) end
local function frameMilliseconds(seconds)
 -- Frame_set_duration 0x3ce440: double multiply, CVTTSD2SI to signed
 -- int32, then Sprite::setFrameDuration 0x61a420 clamps to 1..65535.
 local ms=seconds*1000
 if ms~=ms or ms>=2147483648 or ms<=-2147483649 then return 1 end
 ms=math.modf(ms)
 return math.max(1,math.min(65535,ms))
end
local function celImage(l,fi)
 local m=l.cel_meta[fi];local b=m.bounds or {0,0,doc.width,doc.height};local p={}
 for y=0,b[4]-1 do for x=0,b[3]-1 do local px,py=x+b[1],y+b[2];local color={0,0,0,0}
  if px>=0 and py>=0 and px<doc.width and py<doc.height then color=l.cels[fi][py*doc.width+px+1]
  else for _,o in ipairs(m.outside)do if o.x==px and o.y==py then color=o.color;break end end end;p[#p+1]=copy(color)
 end end
 local im=image(p,b[3],b[4])
 im._put=function(x,y,color)
  local px,py=x+b[1],y+b[2]
  if px>=0 and py>=0 and px<doc.width and py<doc.height then l.cels[fi][py*doc.width+px+1]=copy(color)
  else for _,o in ipairs(m.outside)do if o.x==px and o.y==py then o.color=copy(color);return end end;m.outside[#m.outside+1]={x=px,y=py,color=copy(color)} end
 end
 return im
end
local function putCelImage(l,fi,im,pos)
 local m=l.cel_meta[fi];m.outside={};m.indices=nil
 local pixels={} for i=1,doc.width*doc.height do pixels[i]={0,0,0,0}end;l.cels[fi]=pixels
 m.bounds=(pos.x==0 and pos.y==0 and im.width==doc.width and im.height==doc.height) and nil or {pos.x,pos.y,im.width,im.height}
 for y=0,im.height-1 do for x=0,im.width-1 do local px,py=pos.x+x,pos.y+y;local color=copy(im._pixels[y*im.width+x+1])
  if px>=0 and py>=0 and px<doc.width and py<doc.height then pixels[py*doc.width+px+1]=color elseif color[4]~=0 then m.outside[#m.outside+1]={x=px,y=py,color=color} end
 end end
end
local function cel(li,fi)
 local l=assert(doc.layers[li],'Missing layer');assert(doc.durations[fi],'Missing frame');local m=l.cel_meta[fi]
 return setmetatable({},{__index=function(_,k)
  if k=='image' then return celImage(l,fi) elseif k=='opacity' then return m.opacity elseif k=='zIndex' then return m.z_index
  elseif k=='position' then return m.bounds and Point(m.bounds[1],m.bounds[2]) or Point()
  elseif k=='bounds' then local b=m.bounds or {0,0,doc.width,doc.height};return Rectangle(table.unpack(b))
  elseif k=='frame' then return frameHandles[fi] elseif k=='frameNumber' then return fi elseif k=='layer' then return layerHandles[li] elseif k=='sprite' then return sprite end
 end,__newindex=function(_,k,v)
  if k=='opacity' then m.opacity=opacityByte(v) elseif k=='zIndex' then m.z_index=math.max(-32768,math.min(32767,math.floor(v)))
  elseif k=='image' then putCelImage(l,fi,v,m.bounds and Point(m.bounds[1],m.bounds[2]) or Point())
  elseif k=='position' then putCelImage(l,fi,celImage(l,fi),Point(v)) else error('Unsupported cel field: '..k) end
 end})
end
local function rebuildHandles()
 layerHandles={} for i,l in ipairs(doc.layers) do layerHandles[i]=setmetatable({_index=i},{__index=function(_,k)if k=='name' then return l.name elseif k=='opacity' then return l.opacity elseif k=='isVisible' then return l.visible elseif k=='isEditable' then return not l.locked elseif k=='isTransparent' then return not l.background elseif k=='isBackground' then return l.background elseif k=='isImage' then return not l.group and not l.tileset elseif k=='isTilemap' then return l.tileset~=nil elseif k=='isGroup' then return l.group elseif k=='blendMode' then return l.blend elseif k=='sprite' then return sprite elseif k=='cels' then local c={} for f=1,#doc.durations do c[f]=cel(i,f) end return c elseif k=='cel' then return function(_,f)return cel(i,frameNumber(f))end elseif k=='parent' then return l.parent and layerHandles[l.parent+1] or sprite end end,__newindex=function(_,k,v)if k=='name' then l.name=tostring(v) elseif k=='opacity' then l.opacity=opacityByte(v) elseif k=='isVisible' then l.visible=not not v elseif k=='isEditable' then l.locked=not v elseif k=='blendMode' then l.blend=v elseif k=='parent' then if v==sprite then l.parent=nil else l.parent=layerNumber(v)-1 end else error('Unsupported layer field: '..k) end end}) end
 frameHandles={} for i in ipairs(doc.durations) do frameHandles[i]=setmetatable({frameNumber=i},{__index=function(_,k)if k=='duration' then return doc.durations[i]/1000 elseif k=='sprite' then return sprite end end,__newindex=function(_,k,v)if k=='duration' then doc.durations[i]=frameMilliseconds(v) else error('Unsupported frame field: '..k) end end}) end
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
function spriteMethods:newCel(l,f,im,pos) local li,fi=layerNumber(l),frameNumber(f);local c=cel(li,fi);if im then putCelImage(doc.layers[li],fi,im,pos or Point()) end return c end
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
app=setmetatable({apiVersion=1,version='PixelCraft 0.3',isUIAvailable=false,pixelColor={rgba=pixelRgba,rgbaR=function(c)return pixelInteger(c)&255 end,rgbaG=function(c)return(pixelInteger(c)>>8)&255 end,rgbaB=function(c)return(pixelInteger(c)>>16)&255 end,rgbaA=function(c)return(pixelInteger(c)>>24)&255 end},params={},transaction=function(a,b)return (b or a)() end,refresh=function()end,alert=function(t)print(type(t)=='table' and (t.text or t.title or '') or t)end},{__index=function(_,k)if k=='activeSprite' then return sprite elseif k=='activeLayer' then return layerHandles[activeLayer] elseif k=='activeFrame' then return frameHandles[activeFrame] elseif k=='activeCel' then return cel(activeLayer,activeFrame) elseif k=='activeImage' then return cel(activeLayer,activeFrame).image elseif k=='sprites' then return {sprite} elseif k=='fgColor' then return Color(scriptColors[1]) elseif k=='bgColor' then return Color(scriptColors[2]) else error('Unsupported app API: '..k) end end,__newindex=function(_,k,v)if k=='activeLayer' then activeLayer=assert(layerNumber(v)) elseif k=='activeFrame' then activeFrame=assert(frameNumber(v)) else error('Unsupported app assignment: '..k) end end})
-- NewFrameCommand::onExecute (0x45cfa0) passes active index+1 to
-- addFrame/addEmptyFrame. Sprite methods without arguments instead append.
rawset(app,'command',setmetatable({NewLayer=function()app.activeLayer=sprite:newLayer()end,NewFrame=function()
 local destination=activeFrame+1
 sprite:newFrame(activeFrame)
 app.activeFrame=frameHandles[destination]
end,NewEmptyFrame=function()
 app.activeFrame=sprite:newEmptyFrame(activeFrame+1)
end},{__index=function(_,k)return function()error('Unsupported command: '..k)end end}))
_pw_plugin={preferences={}}
function _pw_plugin:newCommand(c) assert(type(c.id)=='string' and type(c.onclick)=='function','Command requires id and onclick');commands[c.id]=c end
function _pw_plugin:newMenuGroup(_) end
function _pw_plugin:newMenuSeparator(_) end
function _pw_finish(command,discover)
 if type(init)=='function' then init(_pw_plugin) end
 local list={} for id,c in pairs(commands) do list[#list+1]={id=id,title=c.title or id} end
 if command and command~='' then assert(commands[command],'Plugin command was not registered: '..command);commands[command].onclick() end
 if not discover and _pw_loop then _pw_loop() end
 hook()
 local function escape(s) return '"'..s:gsub('[%z\1-\31\\"]',function(c)return string.format('\\u%04x',string.byte(c))end)..'"' end
 local function json(v) local t=type(v);if t=='nil' then return 'null' elseif t=='boolean' or t=='number' then return tostring(v) elseif t=='string' then return escape(v) elseif t=='table' then local array=true;local n=0;for k in pairs(v)do if type(k)~='number' then array=false end;n=n+1 end;local out={};if array then for i=1,#v do out[i]=json(v[i]) end return '['..table.concat(out,',')..']' end;for k,val in pairs(v)do out[#out+1]=escape(k)..':'..json(val) end;return '{'..table.concat(out,',')..'}' else error('Cannot serialize '..t) end end
 _pw_result=json(discover and list or doc)
end
