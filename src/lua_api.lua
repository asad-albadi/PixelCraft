-- PixelCraft's supported native scripting API. Raw project data stays separate
-- from API handles, so metatables and callbacks never enter project persistence.
local clock,hook,fail=os.clock,debug.sethook,error
local raw_pcall,raw_xpcall=pcall,xpcall
local start,instructions,expired=clock(),0,false
local function installBudget(instructionLimit,timeLimit)
 instructionLimit,timeLimit=instructionLimit or 20000000,timeLimit or 3
 start,instructions,expired=clock(),0,false
 hook(function() instructions=instructions+10000;if instructions>instructionLimit or clock()-start>timeLimit then expired=true;fail('Script instruction/time budget exceeded') end end,'',10000)
end
installBudget()
_pw_reset_budget=installBudget
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
local function expandDocumentPixels(document)
 local function expand(pixels)
  if pixels.clear_pixels or pixels.blocks then
   local dense={}
   for i=1,pixels.clear_pixels or pixels.len do dense[i]={0,0,0,0} end
   for index,block in pairs(pixels.blocks or {}) do
    local offset=tonumber(index)*16
    for i,pixel in ipairs(block) do
     if offset+i<=pixels.len then dense[offset+i]=pixel end
    end
   end
   return dense
  end
  return pixels
 end
 for _,layer in ipairs(document.layers) do
  for f,pixels in ipairs(layer.cels) do layer.cels[f]=expand(pixels) end
 end
 for _,tileset in ipairs(document.tilesets or {}) do
  for i,pixels in ipairs(tileset.tiles) do tileset.tiles[i]=expand(pixels) end
 end
 for _,reference in ipairs(document.references or {}) do reference.pixels=expand(reference.pixels) end
 return document
end
local doc=expandDocumentPixels(_pw_document)
_pw_document=nil
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
-- Point_new 0x3e48f0 selects named fields when x is nonnil;
-- both coordinates use lua_tointegerx then low signed32 storage.
local pointFields=setmetatable({}, {__mode='k'})
-- Point_eq 0x3e4a40 compares the two stored int32 coordinates exactly.
local function pointEqual(a,b)
 local left,right=pointFields[a],pointFields[b]
 return left~=nil and right~=nil and left.x==right.x and left.y==right.y
end
function Point(x,y)
 if type(x)=='table' then
  local t=x
  if t.x~=nil then x,y=t.x,t.y else x,y=t[1],t[2] end
 end
 -- Point setters 0x3e5050/0x3e50c0 convert every assignment,
 -- including repeated writes and nil, before storing signed32 coordinates.
 local fields={x=colorInteger(x),y=colorInteger(y)}
 local result=setmetatable({}, {
  __name='Point',
  __eq=pointEqual,
  __index=fields,
  __newindex=function(t,k,v)
   if k=='x' or k=='y' then fields[k]=colorInteger(v)
   else rawset(t,k,v) end
  end,
  __pairs=function() return next,fields,nil end
 })
 pointFields[result]=fields
 return result
end
-- Size_new 0x3ef330 selects width/height, then w/h, then [1]/[2]
-- as a pair. All components use floor-mode integer conversion and signed32 storage.
local sizeFields=setmetatable({}, {__mode='k'})
-- Size_eq 0x3ef470 compares both stored int32 dimensions exactly.
local function sizeEqual(a,b)
 local left,right=sizeFields[a],sizeFields[b]
 return left~=nil and right~=nil and left.width==right.width and left.height==right.height
end
function Size(w,h)
 if type(w)=='table' then
  local t=w
  if t.width~=nil then w,h=t.width,t.height
  elseif t.w~=nil then w,h=t.w,t.h
  else w,h=t[1],t[2] end
 end
 -- Size setters 0x3efb20/0x3efb90 convert on every write;
 -- the property table 0x1cbcff0 maps w/h to the same storage.
 local fields={width=colorInteger(w),height=colorInteger(h)}
 local aliases={w='width',h='height'}
 local result=setmetatable({}, {
  __name='Size',
  __eq=sizeEqual,
  __index=function(_,k) return fields[aliases[k] or k] end,
  __newindex=function(t,k,v)
   k=aliases[k] or k
   if k=='width' or k=='height' then fields[k]=colorInteger(v)
   else rawset(t,k,v) end
  end,
  __pairs=function() return next,fields,nil end
 })
 sizeFields[result]=fields
 return result
end
-- Rectangle_new 0x3e9ea0 selects all array fields when x is nil.
-- Named coordinates select width/height or w/h as a pair by width nonnil.
local rectangleFields=setmetatable({}, {__mode='k'})
-- Rectangle_eq 0x3ea110 compares all four int32 fields, even when empty.
local function rectangleEqual(a,b)
 local left,right=rectangleFields[a],rectangleFields[b]
 return left~=nil and right~=nil and left.x==right.x and left.y==right.y
  and left.width==right.width and left.height==right.height
end
local rectangleMethods={}
local rectangleAliases={w='width',h='height'}
function rectangleMethods:contains(other)
 local point=pointFields[other]
 if point then
  return point.x>=self.x and point.x<self.x+self.width
   and point.y>=self.y and point.y<self.y+self.height
 end
 return rectangleFields[other]~=nil and not self.isEmpty and not other.isEmpty
  and other.x>=self.x and other.y>=self.y
  and other.x+other.width<=self.x+self.width and other.y+other.height<=self.y+self.height
end
function rectangleMethods:intersects(other)
 assert(rectangleFields[other],'Rectangle expected')
 return not self.isEmpty and not other.isEmpty
  and self.x<other.x+other.width and other.x<self.x+self.width
  and self.y<other.y+other.height and other.y<self.y+self.height
end
function rectangleMethods:intersect(other)
 if not rectangleMethods.intersects(self,other) then return Rectangle() end
 local x,y=math.max(self.x,other.x),math.max(self.y,other.y)
 local right=math.min(self.x+self.width,other.x+other.width)
 local bottom=math.min(self.y+self.height,other.y+other.height)
 return Rectangle(x,y,right-x,bottom-y)
end
function rectangleMethods:union(other)
 assert(rectangleFields[other],'Rectangle expected')
 if self.isEmpty then return Rectangle(other) end
 if other.isEmpty then return Rectangle(self) end
 local x,y=math.min(self.x,other.x),math.min(self.y,other.y)
 local right=math.max(self.x+self.width,other.x+other.width)
 local bottom=math.max(self.y+self.height,other.y+other.height)
 return Rectangle(x,y,right-x,bottom-y)
end
function Rectangle(x,y,w,h)
 if type(x)=='table' then
  local t=x
  if t.x==nil then x,y,w,h=t[1],t[2],t[3],t[4]
  elseif t.width~=nil then x,y,w,h=t.x,t.y,t.width,t.height
  else x,y,w,h=t.x,t.y,t.w,t.h end
 end
 -- Native setters 0x3ea6d0/0x3ea740/0x3ea7b0/0x3ea820 convert on
 -- every write. Keep fields behind the proxy so repeated writes also convert.
 local fields={x=colorInteger(x),y=colorInteger(y),width=colorInteger(w),height=colorInteger(h)}
 local result=setmetatable({}, {
  __name='Rectangle',
  __eq=rectangleEqual,
  __band=rectangleMethods.intersect,
  __bor=rectangleMethods.union,
  __index=function(_,k)
   -- Rectangle_get_isEmpty 0x3eaa00 tests signed width/height, not area.
   if k=='isEmpty' then return fields.width<=0 or fields.height<=0 end
   return fields[rectangleAliases[k] or k] or rectangleMethods[k]
  end,
  __newindex=function(t,k,v)
   k=rectangleAliases[k] or k
   if k=='x' or k=='y' or k=='width' or k=='height' then fields[k]=colorInteger(v)
   elseif k=='isEmpty' then error('Rectangle.isEmpty is read-only')
   else rawset(t,k,v) end
  end,
  __pairs=function() return next,fields,nil end
 })
 rectangleFields[result]=fields
 return result
end
ColorMode={RGB='Rgba',GRAY='Grayscale',INDEXED='Indexed'}
local imageSpecType={__name='ImageSpec'}
function imageSpecType.__eq(a,b)
 return getmetatable(a)==imageSpecType and getmetatable(b)==imageSpecType
  and a.width==b.width and a.height==b.height
  and a.colorMode==b.colorMode and a.transparentColor==b.transparentColor
end
function ImageSpec(options)
 options=options or {}
 return setmetatable({
  width=options.width or 1,
  height=options.height or 1,
  colorMode=options.colorMode or ColorMode.RGB,
  transparentColor=options.transparentColor or 0,
 },imageSpecType)
end
BlendMode={NORMAL='Normal',MULTIPLY='Multiply',SCREEN='Screen',OVERLAY='Overlay',DARKEN='Darken',LIGHTEN='Lighten',COLOR_DODGE='ColorDodge',COLOR_BURN='ColorBurn',HARD_LIGHT='HardLight',SOFT_LIGHT='SoftLight',DIFFERENCE='Difference',EXCLUSION='Exclusion',HSL_HUE='Hue',HSL_SATURATION='Saturation',HSL_COLOR='Color',HSL_LUMINOSITY='Luminosity',ADDITION='Addition',SUBTRACT='Subtract',DIVIDE='Divide'}
AniDir={FORWARD=0,REVERSE=1,PING_PONG=2,PING_PONG_REVERSE=3}
-- Native App_get_fgColor 0x50ac90 / App_get_bgColor 0x50ae00 copy
-- the preference Color into fresh Lua userdata on every read.
local scriptColors=_pw_colors
local imageMethods={}
local function image(p,w,h,mode,transparent)
 mode=mode or doc.color_mode
 return setmetatable({
  _pixels=p,width=w,height=h,colorMode=mode,
  _transparent=transparent or (mode==ColorMode.INDEXED and doc.transparent_index or 0),
  bytesPerPixel=mode=='Rgba' and 4 or mode=='Grayscale' and 2 or 1,
 },{
  __index=function(im,k)
   if k=='spec' then
    return ImageSpec{width=im.width,height=im.height,colorMode=im.colorMode,transparentColor=im._transparent}
   end
   if k=='bounds' then return Rectangle(0,0,im.width,im.height) end
   return imageMethods[k]
  end,
 })
end
function Image(w,h,mode)
 local transparent
 if type(w)=='table' then
  if w._pixels then return image(copy(w._pixels),w.width,w.height,w.colorMode,w._transparent) end
  w,h,mode,transparent=w.width,w.height,w.colorMode or mode,w.transparentColor
 end
 assert(type(w)=='number' and type(h)=='number' and w>0 and h>0 and w*h<=16000000,'Image dimensions exceed budget')
 local pixels={}
 local im=image(pixels,w,h,mode,transparent)
 local r,g,b,a=0,0,0,0
 if im.colorMode==ColorMode.RGB then
  r,g,b,a=table.unpack(unpackColor(im._transparent))
 elseif im.colorMode==ColorMode.GRAY then
  local mask=pixelInteger(im._transparent)
  local gray=mask&255
  r,g,b,a=gray,gray,gray,(mask>>8)&255
 end
 for i=1,w*h do pixels[i]={r,g,b,a} end
 return im
end
function imageMethods:drawPixel(x,y,c) x=math.floor(x);y=math.floor(y);if x>=0 and y>=0 and x<self.width and y<self.height then local value=unpackColor(c);self._pixels[y*self.width+x+1]=value;if self._put then self._put(x,y,value)end end end
imageMethods.putPixel=imageMethods.drawPixel
function imageMethods:getPixel(x,y) if x<0 or y<0 or x>=self.width or y>=self.height then return 0 end return rgba(table.unpack(self._pixels[y*self.width+x+1])) end
function imageMethods:clear(c)
 c=c or self._transparent
 for y=0,self.height-1 do for x=0,self.width-1 do self:drawPixel(x,y,c) end end
end
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
local withSprite=function(_,fn,...)return fn(...)end
local spriteOwners=setmetatable({},{__mode='k'})
local function bindSpriteHandle(handle,owner)
 local meta=getmetatable(handle)
 spriteOwners[handle]=owner
 return setmetatable(handle,{
  __index=function(self,key)
   local value=withSprite(owner,meta.__index,self,key)
   if type(value)=='function' and key~='close' then
    return function(...)return withSprite(owner,value,...)end
   end
   return value
  end,
  __newindex=meta.__newindex and function(...)return withSprite(owner,meta.__newindex,...)end,
  __len=meta.__len and function(...)return withSprite(owner,meta.__len,...)end,
 })
end
local function frameNumber(f)
 if type(f)=='table' then
  assert(not spriteOwners[f] or spriteOwners[f]==sprite,'Frame belongs to another sprite')
  return f.frameNumber
 end
 return tonumber(f)
end
local function layerNumber(l)
 if type(l)=='table' then
  assert(not spriteOwners[l] or spriteOwners[l]==sprite,'Layer belongs to another sprite')
  return l._index
 end
 return tonumber(l)
end
local function frameMilliseconds(seconds)
 -- Frame_set_duration 0x3ce440: double multiply, CVTTSD2SI to signed
 -- int32, then Sprite::setFrameDuration 0x61a420 clamps to 1..65535.
 local ms=seconds*1000
 if ms~=ms or ms>=2147483648 or ms<=-2147483649 then return 1 end
 ms=math.modf(ms)
 return math.max(1,math.min(65535,ms))
end
local function celImage(l,fi,resolve)
 local m=l.cel_meta[fi];local b=m.bounds or {0,0,doc.width,doc.height};local p={}
 for y=0,b[4]-1 do for x=0,b[3]-1 do local px,py=x+b[1],y+b[2];local color={0,0,0,0}
  if px>=0 and py>=0 and px<doc.width and py<doc.height then color=l.cels[fi][py*doc.width+px+1]
  else for _,o in ipairs(m.outside)do if o.x==px and o.y==py then color=o.color;break end end end;p[#p+1]=copy(color)
 end end
 local im=image(p,b[3],b[4])
 local owner=sprite
 local function put(x,y,color)
  local l,fi=resolve()
  local document=doc
  local link=l.cel_meta[fi].artwork_link
  for frame,target in ipairs(l.cel_meta) do
   if frame==fi or (link and target.artwork_link==link) then
    local bounds=target.bounds or {0,0,document.width,document.height}
    local px,py=x+bounds[1],y+bounds[2]
    if px>=0 and py>=0 and px<document.width and py<document.height then l.cels[frame][py*document.width+px+1]=copy(color)
    else
     local found=false
     for _,o in ipairs(target.outside)do
      if o.x==px and o.y==py then o.color=copy(color);found=true;break end
     end
     if not found then target.outside[#target.outside+1]={x=px,y=py,color=copy(color)} end
    end
   end
  end
 end
 im._put=function(...)return withSprite(owner,put,...)end
 im._sprite=owner
 return im
end
local function putCelImage(l,fi,im,pos)
 local m=l.cel_meta[fi];m.outside={};m.indices=nil;m.artwork_link=nil
 local pixels={} for i=1,doc.width*doc.height do pixels[i]={0,0,0,0}end;l.cels[fi]=pixels
 m.bounds=(pos.x==0 and pos.y==0 and im.width==doc.width and im.height==doc.height) and nil or {pos.x,pos.y,im.width,im.height}
 for y=0,im.height-1 do for x=0,im.width-1 do local px,py=pos.x+x,pos.y+y;local color=copy(im._pixels[y*im.width+x+1])
  if px>=0 and py>=0 and px<doc.width and py<doc.height then pixels[py*doc.width+px+1]=color elseif color[4]~=0 then m.outside[#m.outside+1]={x=px,y=py,color=color} end
 end end
end
local bindUserProperties,importUserData
bindUserProperties=function(handle)return handle end
local historyObjectKeys=setmetatable({},{__mode='k'})
local function historyObjectKey(object)
 local key=historyObjectKeys[object]
 if key then return key end
 key={}
 historyObjectKeys[object]=key
 return key
end
local function historyObjectIndex(list,key,description,hint)
 local current=hint and list and list[hint]
 if current and historyObjectKeys[current]==key then return hint,current end
 for index,value in ipairs(list or {})do
  if historyObjectKeys[value]==key then return index,value end
 end
 error(description..' is deleted or no longer belongs to this document state')
end
local function copyObjectKeys(source,target)
 for index,object in ipairs(source or {})do
  historyObjectKeys[target[index]]=historyObjectKeys[object]
 end
end
local setMetadataIdentity,metadataIdentityWire
do
 local keysByWire=setmetatable({},{__mode='v'})
 local kinds={'tags','slices','tilesets'}
 local nextKey=0
 local function bind(object,wire)
  if not wire then return end
  local key=keysByWire[wire] or {wire=wire}
  keysByWire[wire]=key
  historyObjectKeys[object]=key
 end
 local function wireOf(object)
  local key=historyObjectKey(object)
  if not key.wire then
   nextKey=nextKey+1
   key.wire='script:'..nextKey
   keysByWire[key.wire]=key
  end
  return key.wire
 end
 setMetadataIdentity=function(document,keys)
  if not keys then return end
  for _,kind in ipairs(kinds)do
   local objects=document[kind] or {}
   for index,object in ipairs(objects)do bind(object,(keys[kind] or {})[index]) end
  end
  local slot=0
  local tiles=keys.tiles or {}
  for _,set in ipairs(document.tilesets or {})do
   for _,tile in ipairs(set.tiles)do
    slot=slot+1
    bind(tile,tiles[slot])
   end
  end
 end
 metadataIdentityWire=function(document)
  document=document or doc
  local groups={}
  for _,kind in ipairs(kinds)do
   local keys={}
   for index,object in ipairs(document[kind] or {})do keys[index]=wireOf(object) end
   groups[kind]=keys
  end
  groups.tiles={}
  for _,set in ipairs(document.tilesets or {})do
   for _,tile in ipairs(set.tiles)do groups.tiles[#groups.tiles+1]=wireOf(tile) end
  end
  return groups
 end
end
local function isTagUserData(chunk)
 return chunk.frame==0 and chunk.kind==0x2020 and chunk.owner=='tags'
end
local tilesetHandle,spriteTilesets,assignLayerTileset,snapshotTileset,detachedTileset
local cloneDocumentState,insertFrameIdentity,retainFrameExportIds,cel,rebuildHandles
local setLayerIdentity,layerIdentityWire,celIdentityWire,reconcileDocumentState
local scriptLayerChanged
local layerEventFields={name='name',opacity='opacity',isVisible='visible',blendMode='blend'}
do
 -- Keep Lua handles and their identity tokens out of the serialized document.
 local states=setmetatable({},{__mode='k'})
 local layerKeys=setmetatable({},{__mode='k'})
 local layers=setmetatable({},{__mode='kv'})
 local frames=setmetatable({},{__mode='kv'})
 local cels=setmetatable({},{__mode='kv'})
 local keysByWire=setmetatable({},{__mode='v'})
 local celKeysByWire=setmetatable({},{__mode='v'})
 local nextLayerKey,nextCelKey=0,0
 local state

 local function layerKey(wire)
  local key=keysByWire[wire]
  if key then return key end
  key={wire=wire}
  keysByWire[wire]=key
  return key
 end

 setLayerIdentity=function(document,keys)
  if not keys then return end
  for i,layer in ipairs(document.layers)do
   if keys[i] then layerKeys[layer]=layerKey(keys[i]) end
  end
 end

 layerIdentityWire=function(document)
  local keys={}
  for i,layer in ipairs((document or doc).layers)do keys[i]=layerKeys[layer].wire end
  return keys
 end

 celIdentityWire=function(document)
  local value=state(document or doc)
  local keys={}
  for key,slot in pairs(value.celSlots)do
   keys[#keys+1]={key=key.wire,layer=value.layerSlots[slot.layer],frame=value.frameSlots[slot.frame]}
  end
  return keys
 end

 reconcileDocumentState=function(document,keys,celKeys)
  local current=state(doc)
  local oldIds=doc.frame_export_ids and doc.frame_export_ids.frames
  local newIds=document.frame_export_ids and document.frame_export_ids.frames
  local history=current.frameHistory
  for i,key in ipairs(current.frameKeys)do history[oldIds and oldIds[i] or i-1]=key end
  local next=state(document)
  next.frameHistory=history
  for i=1,#document.durations do
   next.frameKeys[i]=history[newIds and newIds[i] or i-1] or next.frameKeys[i]
  end
  setLayerIdentity(document,keys)
  if not celKeys then
   next.celRows=current.celRows
   return
  end
  for _,entry in ipairs(celKeys)do
   -- A retained handle keeps its token alive even when undo history is
   -- the only place the editor still has that cel.
   local key=celKeysByWire[entry.key]
   local layer=document.layers[entry.layer]
   local owner=layer and layerKeys[layer]
   local frame=next.frameKeys[entry.frame]
   if key and owner and frame then
    local row=next.celRows[owner] or {}
    row[frame]=key
    next.celRows[owner]=row
   end
  end
 end

 state=function(document)
  local value=states[document]
  if value then return value end
  local keys={}
  for i=1,#document.durations do keys[i]={} end
  value={frameKeys=keys,frameHistory=setmetatable({},{__mode='v'}),celRows={},celSlots={}}
  states[document]=value
  return value
 end

 cloneDocumentState=function(source)
  local target=copy(source)
  local previous=state(source)
  local keys=previous.frameKeys
  local rows={}
  for layer,row in pairs(previous.celRows)do
   local copied={}
   for frame,key in pairs(row)do copied[frame]=key end
   rows[layer]=copied
  end
  states[target]={frameKeys=table.move(keys,1,#keys,1,{}),frameHistory=previous.frameHistory,
   celRows=rows,celSlots={}}
  for i,layer in ipairs(source.layers)do layerKeys[target.layers[i]]=layerKeys[layer] end
  copyObjectKeys(source.tags,target.tags)
  copyObjectKeys(source.slices,target.slices)
  copyObjectKeys(source.tilesets,target.tilesets)
  for i,set in ipairs(source.tilesets or {})do copyObjectKeys(set.tiles,target.tilesets[i].tiles) end
  return target
 end

 insertFrameIdentity=function(index)
  table.insert(state(doc).frameKeys,index,{})
 end

 local function layerIndex(key)
  return assert(state(doc).layerSlots[key],'Layer is deleted or no longer belongs to this document state')
 end

 local function frameIndex(key)
  return assert(state(doc).frameSlots[key],'Frame is deleted or no longer belongs to this document state')
 end

 cel=function(li,fi)
  local l=assert(doc.layers[li],'Missing layer')
  local value=state(doc)
  local frame=assert(value.frameKeys[fi],'Missing frame')
  local owner=layerKeys[l]
  local row=value.celRows[owner]
  if not row then row={};value.celRows[owner]=row end
  local key=row[frame]
  if not key then
   nextCelKey=nextCelKey+1
   key={wire=nextCelKey}
   celKeysByWire[nextCelKey]=key
   row[frame]=key
   value.celSlots[key]={layer=owner,frame=frame}
  end
  if cels[key] then return cels[key] end

  local function resolve()
   local slot=assert(state(doc).celSlots[key],'Cel is deleted or no longer belongs to this document state')
   local layer,frame=layerIndex(slot.layer),frameIndex(slot.frame)
   return doc.layers[layer],frame,layer
  end
  local handle=bindSpriteHandle(setmetatable({},{
   __index=function(_,field)
    local layer,frame,index=resolve()
    local meta=layer.cel_meta[frame]
    if field=='image' then return celImage(layer,frame,resolve) end
    if field=='opacity' then return meta.opacity end
    if field=='zIndex' then return meta.z_index end
    if field=='position' then return meta.bounds and Point(meta.bounds[1],meta.bounds[2]) or Point() end
    if field=='bounds' then
     return Rectangle(table.unpack(meta.bounds or {0,0,doc.width,doc.height}))
    end
    if field=='frame' then return frameHandles[frame] end
    if field=='frameNumber' then return frame end
    if field=='layer' then return layerHandles[index] end
    if field=='sprite' then return sprite end
   end,
   __newindex=function(_,field,value)
    local layer,frame=resolve()
    local meta=layer.cel_meta[frame]
    if field=='opacity' then meta.opacity=opacityByte(value)
    elseif field=='zIndex' then meta.z_index=math.max(-32768,math.min(32767,math.floor(value)))
    elseif field=='image' then
     putCelImage(layer,frame,value,meta.bounds and Point(meta.bounds[1],meta.bounds[2]) or Point())
    elseif field=='position' then
     putCelImage(layer,frame,celImage(layer,frame,resolve),Point(value))
    else error('Unsupported cel field: '..field) end
   end,
  }),sprite)
  cels[key]=handle
  return bindUserProperties(handle,sprite,function()
   local layer,frame,index=resolve()
   local link=layer.cel_meta[frame].artwork_link
   -- Linked cels share user data with the first source cel in ASE.
   if link then
    for slot,metadata in ipairs(layer.cel_meta)do
     if metadata.artwork_link==link then frame=slot;break end
    end
   end
   return 'cel:'..(index-1),frame-1
  end)
 end

 local function layerHandle(key)
  if layers[key] then return layers[key] end
  local handle=bindSpriteHandle(setmetatable({},{
   __index=function(_,field)
    local index=layerIndex(key)
    local layer=doc.layers[index]
    if field=='_index' then return index end
    if field=='name' then return layer.name end
    if field=='opacity' then return layer.opacity end
    if field=='isVisible' then return layer.visible end
    if field=='isEditable' then return not layer.locked end
    if field=='isTransparent' then return not layer.background end
    if field=='isBackground' then return layer.background end
    if field=='isImage' then return not layer.group and not layer.tileset end
    if field=='tileset' then return layer.tileset and tilesetHandle(doc.tilesets[layer.tileset+1]) end
    if field=='isTilemap' then return layer.tileset~=nil end
    if field=='isGroup' then return layer.group end
    if field=='blendMode' then return layer.blend end
    if field=='sprite' then return sprite end
    if field=='cels' then
     local result={}
     for frame=1,#doc.durations do result[frame]=cel(index,frame) end
     return result
    end
    if field=='cel' then return function(_,frame)return cel(layerIndex(key),frameNumber(frame))end end
    if field=='parent' then return layer.parent and layerHandles[layer.parent+1] or sprite end
   end,
   __newindex=function(handle,field,value)
    local layer=doc.layers[layerIndex(key)]
    local property=layerEventFields[field]
    local previous=property and layer[property]
    if field=='name' then layer.name=tostring(value)
    elseif field=='opacity' then layer.opacity=opacityByte(value)
    elseif field=='isVisible' then layer.visible=not not value
    elseif field=='isEditable' then layer.locked=not value
    elseif field=='blendMode' then layer.blend=value
    elseif field=='tileset' then assignLayerTileset(layer,value)
    elseif field=='parent' then layer.parent=value~=sprite and layerNumber(value)-1 or nil
    else error('Unsupported layer field: '..field) end
    if property and previous~=layer[property] and scriptLayerChanged then
     scriptLayerChanged(property,handle)
    end
   end,
  }),sprite)
  layers[key]=handle
  return bindUserProperties(handle,sprite,function()
   return 'layer:'..(layerIndex(key)-1),0
  end)
 end

 local function frameHandle(key)
  if frames[key] then return frames[key] end
  local handle=bindSpriteHandle(setmetatable({},{
   __index=function(_,field)
    local index=frameIndex(key)
    if field=='frameNumber' then return index end
    if field=='duration' then return doc.durations[index]/1000 end
    if field=='sprite' then return sprite end
   end,
   __newindex=function(_,field,value)
    if field~='duration' then error('Unsupported frame field: '..field) end
    doc.durations[frameIndex(key)]=frameMilliseconds(value)
   end,
  }),sprite)
  frames[key]=handle
  return handle
 end

 rebuildHandles=function()
  local value=state(doc)
  value.layerSlots,value.frameSlots={},{}
  layerHandles,frameHandles={},{}
  for i,layer in ipairs(doc.layers)do
   local key=layerKeys[layer]
   if not key then
    nextLayerKey=nextLayerKey+1
    key=layerKey('script:'..nextLayerKey)
    layerKeys[layer]=key
   end
   value.layerSlots[key]=i
   layerHandles[i]=layerHandle(key)
  end
  for i,key in ipairs(value.frameKeys)do
   value.frameSlots[key]=i
   frameHandles[i]=frameHandle(key)
  end
  value.celSlots={}
  for owner,row in pairs(value.celRows)do
   if value.layerSlots[owner] then
    for frame,key in pairs(row)do
     if value.frameSlots[frame] then
      value.celSlots[key]={layer=owner,frame=frame}
     else row[frame]=nil end
    end
   else value.celRows[owner]=nil end
  end
 end
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
local function removeIndexedMetadata(kind,index)
 for i=#doc.metadata,1,-1 do
  local metadata=doc.metadata[i]
  local owner=tonumber(metadata.owner:match('^'..kind..':(%d+)$'))
  if owner==index-1 then table.remove(doc.metadata,i)
  elseif owner and owner>=index then metadata.owner=kind..':'..(owner-1) end
 end
end

local spriteSlices
do
 local handles=setmetatable({},{__mode='kv'})
 local objects=setmetatable({},{__mode='k'})
 local methods={}

 local function sliceObject(handle)
  local key=assert(objects[handle],'Invalid slice')
  local _,slice=historyObjectIndex(doc.slices,key,'Slice')
  return slice
 end

 local function rectValues(rect)
  assert(rect.width>=0 and rect.height>=0,'Invalid slice rectangle')
  return {rect.x,rect.y,rect.width,rect.height}
 end

 local function optionalCenter(value)
  if value==nil then return nil end
  local rect=Rectangle(value)
  local values=rectValues(rect)
  if not rect.isEmpty then return values end
 end

 local function optionalPivot(value)
  if value==nil then return nil end
  local point=Point(value)
  if point.x~=-2147483648 or point.y~=-2147483648 then return {point.x,point.y} end
 end

 local function sliceFrame(frame)
  local index=frameNumber(frame)
  assert(index and index%1==0 and doc.durations[index],'Invalid slice frame')
  return index-1
 end

 local function setKey(slice,key)
  for index,existing in ipairs(slice.keys)do
   if existing.frame==key.frame then slice.keys[index]=key;return end
   if existing.frame>key.frame then table.insert(slice.keys,index,key);return end
  end
  slice.keys[#slice.keys+1]=key
 end

 function methods:keyAt(frame)
  local slice,index=sliceObject(self),sliceFrame(frame)
  for i=#slice.keys,1,-1 do
   local key=slice.keys[i]
   if key.frame<=index then
    return {frame=frameHandles[key.frame+1],bounds=Rectangle(key.bounds),
     center=key.center and Rectangle(key.center),pivot=key.pivot and Point(key.pivot)}
   end
  end
 end

 function methods:setKey(frame,bounds,center,pivot)
  local slice,index=sliceObject(self),sliceFrame(frame)
  setKey(slice,{frame=index,bounds=rectValues(Rectangle(bounds)),
   center=optionalCenter(center),pivot=optionalPivot(pivot)})
 end

 function methods:deleteKey(frame)
  local slice,index=sliceObject(self),sliceFrame(frame)
  for i,key in ipairs(slice.keys)do
   if key.frame==index then table.remove(slice.keys,i);return true end
  end
  return false
 end

 local function sliceHandle(slice)
  local identity=historyObjectKey(slice)
  local handle=handles[identity]
  if handle then return handle end
  handle=bindSpriteHandle(setmetatable({},{
   __index=function(self,key)
    local value=sliceObject(self)
    if key=='sprite' then return sprite end
    if key=='name' then return value.name end
    if methods[key] then return methods[key] end
    local first=value.keys[1]
    if not first then return nil end
    if key=='bounds' then return Rectangle(first.bounds) end
    if key=='center' and first.center then return Rectangle(first.center) end
    if key=='pivot' and first.pivot then return Point(first.pivot) end
   end,
   __newindex=function(self,key,value)
    local slice=sliceObject(self)
    if key=='name' then
     local name=tostring(value)
     assert(#name<=65535,'Slice name is too long')
     slice.name=name
     return
    end
    local first=slice.keys[1]
    local entry=first and first.frame==0 and copy(first) or {frame=0,bounds={0,0,0,0}}
    if key=='bounds' then entry.bounds=rectValues(Rectangle(value))
    elseif key=='center' then entry.center=optionalCenter(value)
    elseif key=='pivot' then entry.pivot=optionalPivot(value)
    else error('Unsupported slice assignment: '..key) end
    setKey(slice,entry)
   end,
  }),sprite)
  handles[identity]=handle
  objects[handle]=identity
  return bindUserProperties(handle,sprite,function()
   local index=historyObjectIndex(doc.slices,identity,'Slice')
   return 'slice:'..(index-1),0
  end)
 end

 function spriteMethods:newSlice(x,y,w,h)
  local bounds=Rectangle(x,y,w,h)
  local values=rectValues(bounds)
  local slice={name='Slice',keys={}}
  if not bounds.isEmpty then setKey(slice,{frame=0,bounds=values}) end
  doc.slices=doc.slices or {}
  doc.slices[#doc.slices+1]=slice
  return sliceHandle(slice)
 end

 function spriteMethods:deleteSlice(value)
  local target=type(value)=='table' and sliceObject(value)
  for index,slice in ipairs(doc.slices or {})do
   if slice==target or slice.name==value then
    table.remove(doc.slices,index)
    removeIndexedMetadata('slice',index)
    return
   end
  end
  error('Slice not found')
 end

 spriteSlices=function()
  local result={}
  for index,slice in ipairs(doc.slices or {})do result[index]=sliceHandle(slice) end
  return result
 end
end

local function insertFrameMetadata(index)
 for _,slice in ipairs(doc.slices or {})do
  for _,key in ipairs(slice.keys)do
   if key.frame>=index then key.frame=key.frame+1 end
  end
 end
 for _,metadata in ipairs(doc.metadata)do
  local owner,kind=metadata.owner,metadata.kind
  local frameIndependent=metadata.frame==0 and (
   owner:match('^layer:') or owner:match('^slice:') or owner:match('^tileset:') or
   owner=='tags' or owner=='ase:header' or (owner=='palette' and kind==0x2020) or
   (owner=='' and kind==0x2008))
  if metadata.frame>=index and not frameIndependent then metadata.frame=metadata.frame+1 end
 end
end

local function explicitFrameExportIds(document)
 local ids=document.frame_export_ids
 if not ids then
  ids={frames={},next=#document.durations}
  for i=1,#document.durations do ids.frames[i]=i-1 end
  document.frame_export_ids=ids
 end
 return ids
end
retainFrameExportIds=function(previous,restored)
 local previousNext=previous.frame_export_ids and previous.frame_export_ids.next or #previous.durations
 local restoredNext=restored.frame_export_ids and restored.frame_export_ids.next or #restored.durations
 if previousNext>restoredNext then explicitFrameExportIds(restored).next=previousNext end
end
local function insertFrameExportId(index)
 local ids=doc.frame_export_ids
 if not ids then
  if index>#doc.durations then return end
  ids=explicitFrameExportIds(doc)
 end
 assert(ids.next<=4294967295,'Frame export ID limit exceeded')
 table.insert(ids.frames,index,ids.next)
 ids.next=ids.next+1
end
local function insertFrameName(index, source)
 local names=doc.frame_names
 if not names then return end
 local name=source and names[source] or ''
 if index<=#names or name~='' then table.insert(names,index,name) end
end
function spriteMethods:newFrame(f)
 assert((#doc.durations+1)*#doc.layers*doc.width*doc.height<=64000000,'Frame budget exceeded')
 local explicit=f~=nil
 f=frameNumber(f) or #doc.durations
 assert(f%1==0 and f>=1 and f<=#doc.durations,'Missing frame')
 -- Sprite_newFrame (0x3f3b10) -> DocApi::addFrame (0x19f0b0):
 -- copy the preceding frame into the insertion slot, shifting later frames.
 insertFrameExportId(f+1)
 insertFrameIdentity(f+1)
 insertFrameName(f+1,f)
 table.insert(doc.durations,f+1,doc.durations[f])
 for _,l in ipairs(doc.layers) do
  table.insert(l.cels,f+1,copy(l.cels[f]))
  local meta=copy(l.cel_meta[f]);meta.artwork_link=nil;table.insert(l.cel_meta,f+1,meta)
  table.insert(l.tile_cels,f+1,copy(l.tile_cels[f]))
 end
 -- Keep PixelCraft's parallel document data aligned with the inserted frame.
 if #doc.frame_palettes>0 then table.insert(doc.frame_palettes,f+1,copy(doc.frame_palettes[f] or doc.palette)) end
 for _,t in ipairs(doc.tags) do if t.from>=f then t.from=t.from+1 end;if t.to>=f-1 then t.to=t.to+1 end end
 insertFrameMetadata(f)
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
 insertFrameExportId(f)
 insertFrameIdentity(f)
 insertFrameName(f)
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
 insertFrameMetadata(f-1)
 rebuildHandles()
 return frameHandles[f]
end
local spriteTags
do
 local handles=setmetatable({},{__mode='kv'})
 local objects=setmetatable({},{__mode='k'})

 local function tagHandle(tag)
  local identity=historyObjectKey(tag)
  if handles[identity] then return handles[identity] end
  local function resolve()
   return historyObjectIndex(doc.tags,identity,'Tag')
  end
  local handle=bindSpriteHandle(setmetatable({},{
   __index=function(_,key)
    local _,tag=resolve()
    if key=='name' then return tag.name end
    if key=='fromFrame' then return frameHandles[tag.from+1] end
    if key=='toFrame' then return frameHandles[tag.to+1] end
    if key=='aniDir' then return tag.direction end
    if key=='sprite' then return sprite end
   end,
   __newindex=function(_,key,value)
    local _,tag=resolve()
    if key=='name' then tag.name=tostring(value)
    elseif key=='aniDir' then tag.direction=value
    elseif key=='fromFrame' then tag.from=frameNumber(value)-1
    elseif key=='toFrame' then tag.to=frameNumber(value)-1
    else error('Unsupported tag field: '..key) end
   end,
  }),sprite)
  handles[identity]=handle
  objects[handle]=identity
  return bindUserProperties(handle,sprite,function()
   local index=resolve()
   return 'tags',0,index
  end)
 end

 function spriteMethods:newTag(a,b)
  a=frameNumber(a);b=frameNumber(b)
  assert(a and b and a>=1 and b>=a and b<=#doc.durations,'Invalid tag range')
  local tag={name='Tag',from=a-1,to=b-1,direction=0,color={128,180,255}}
  table.insert(doc.tags,tag)
  return tagHandle(tag)
 end

 function spriteMethods:deleteTag(handle)
  local identity=objects[handle]
  local index
  if identity then
   index=historyObjectIndex(doc.tags,identity,'Tag')
  else
   for i,value in ipairs(doc.tags)do
    if value.name==handle.name then index=i;break end
   end
   if not index then return end
  end
  table.remove(doc.tags,index)
  local ordinal=0
  for i,chunk in ipairs(doc.metadata)do
   if isTagUserData(chunk) then
    ordinal=ordinal+1
    if ordinal==index then table.remove(doc.metadata,i);break end
   end
  end
 end

 spriteTags=function()
  local result={}
  for i,tag in ipairs(doc.tags)do result[i]=tagHandle(tag) end
  return result
 end
end
setmetatable(sprite,{__index=function(_,k)if k=='tags' then return spriteTags() elseif k=='slices' then return spriteSlices() elseif k=='tilesets' then return spriteTilesets() elseif spriteMethods[k] then return spriteMethods[k] elseif k=='width' or k=='height' then return doc[k] elseif k=='layers' then return layerHandles elseif k=='frames' then return frameHandles elseif k=='palettes' then return {palette} elseif k=='colorMode' then return doc.color_mode elseif k=='filename' then return '' elseif k=='cels' then local c={} for i,l in ipairs(doc.layers) do if not l.group then for f=1,#doc.durations do c[#c+1]=cel(i,f) end end end return c end end,__newindex=function(_,k,v)error('Unsupported sprite assignment: '..k)end})
-- Detached scratch sprites let a script compose an image without changing the
-- editor document. They are not tabs and do not participate in document undo.
function Sprite(w,h,mode)
 mode=mode or ColorMode.RGB
 assert(mode==ColorMode.RGB,'Detached sprites must use RGB mode')
 local layer,frame={name='Layer 1'},{frameNumber=1,duration=0.1}
 local scratch={width=w,height=h,colorMode=mode,layers={layer},frames={frame}}
 local cells={}
 local closed=false
 function scratch:newCel(target,at,im,pos)
  assert(not closed and target==layer and frameNumber(at)==1,'Invalid detached sprite cel')
  local cell={layer=layer,frame=frame,image=im and im:clone() or Image(w,h,mode),position=pos or Point(),opacity=255}
  cells[1]=cell
  return cell
 end
 function layer:cel(at)
  assert(not closed,'Detached sprite is closed')
  return cells[frameNumber(at)]
 end
 function scratch:close()
  closed=true
  cells={}
 end
 -- Validate the initial canvas and provide its first cel.
 scratch:newCel(layer,frame)
 return scratch
end
local activeLayer,activeFrame=_pw_layer+1,_pw_frame+1
local commands={}
app=setmetatable({apiVersion=1,version='PixelCraft 0.3',isUIAvailable=false,pixelColor={rgba=pixelRgba,rgbaR=function(c)return pixelInteger(c)&255 end,rgbaG=function(c)return(pixelInteger(c)>>8)&255 end,rgbaB=function(c)return(pixelInteger(c)>>16)&255 end,rgbaA=function(c)return(pixelInteger(c)>>24)&255 end},params={},transaction=function(a,b)return (b or a)() end,refresh=function()end,alert=function(t)print(type(t)=='table' and (t.text or t.title or '') or t)end},{__index=function(_,k)if k=='activeSprite' then return sprite elseif k=='activeLayer' then return layerHandles[activeLayer] elseif k=='activeFrame' then return frameHandles[activeFrame] elseif k=='activeCel' then return cel(activeLayer,activeFrame) elseif k=='activeImage' then return cel(activeLayer,activeFrame).image elseif k=='sprites' then return {sprite} elseif k=='fgColor' then return Color(scriptColors[1]) elseif k=='bgColor' then return Color(scriptColors[2]) else error('Unsupported app API: '..k) end end,__newindex=function(_,k,v)if k=='activeSprite' then assert(v==sprite,'Only the editor sprite can be made active') elseif k=='activeLayer' then activeLayer=assert(layerNumber(v)) elseif k=='activeFrame' then activeFrame=assert(frameNumber(v)) else error('Unsupported app assignment: '..k) end end})
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
local pluginRegistration
do
 local entries,positions={},{}
 local registered=false
 local function put(key,entry)
  registered=true
  local position=positions[key] or (#entries+1)
  positions[key]=position
  entries[position]=entry
 end
 local function identifier(value)
  if type(value)=='table' then value=value.id end
  assert(type(value)=='string' and #value>0 and #value<=200,'An extension entry requires an id')
  return value
 end
 local function title(value,id)
  assert(value==nil or type(value)=='string' and #value<=1024,'Invalid extension menu title')
  return value or id
 end
 local function parent(value)
  return value and identifier(value) or ''
 end
 function _pw_plugin:newCommand(c)
  local id=identifier(c)
  assert(type(c.onclick)=='function','Command requires onclick')
  put('command:'..id,{kind='command',id=id,title=title(c.title,id),group=parent(c.group)})
  commands[id]=c
 end
 function _pw_plugin:deleteCommand(value)
  local id=identifier(value)
  commands[id]=nil
  if positions['command:'..id] then put('command:'..id,false) end
 end
 function _pw_plugin:newMenuGroup(c)
  local id=identifier(c)
  put('group:'..id,{kind='group',id=id,title=title(c.title,id),group=parent(c.group)})
 end
 function _pw_plugin:deleteMenuGroup(value)
  local id=identifier(value)
  if positions['group:'..id] then
   entries[positions['group:'..id]].hidden=true
  end
 end
 function _pw_plugin:newMenuSeparator(c)
  put('separator:'..(#entries+1),{kind='separator',group=parent(c and c.group)})
 end
 pluginRegistration=function()
  local result={}
  for _,entry in ipairs(entries)do if entry then result[#result+1]=entry end end
  return {registered=registered,entries=result}
 end
end
local encodeJson
do
 local kind, walk, length, at, stringify = type, next, rawlen, rawget, tostring
 local numberKind = math.type
 local substitute, format, ascii, join = string.gsub, string.format, string.byte, table.concat
 local function escape(text)
  return '"' .. substitute(text, '[%z\1-\31\\"]', function(char)
   return format('\\u%04x', ascii(char))
  end) .. '"'
 end

 function encodeJson(value)
  -- Bound transient allocations when encoding millions of scalar pixel channels.
  local batch, chunks, size = {}, {}, 0
  local function append(text)
   size = size + 1
   batch[size] = text
   if size == 2048 then
    chunks[#chunks + 1] = join(batch)
    size = 0
   end
  end

  local function write(item, depth)
   local t = kind(item)
   if t == 'nil' then
    append('null')
   elseif t == 'boolean' or t == 'number' then
    append(stringify(item))
   elseif t == 'string' then
    append(escape(item))
   elseif t == 'table' then
    if depth >= 128 then fail('Script result exceeds the nesting limit') end
    local array = true
    for key in walk, item do
     if kind(key) ~= 'number' then array = false; break end
    end
    if array then
     local count = length(item)
     if count == 4 then
      local r, g, b, a = at(item, 1), at(item, 2), at(item, 3), at(item, 4)
      if numberKind(r) == 'integer' and numberKind(g) == 'integer' and
         numberKind(b) == 'integer' and numberKind(a) == 'integer' then
       -- Serialize common raster tuples without recursing per channel.
       append(r == 0 and g == 0 and b == 0 and a == 0 and '[0,0,0,0]' or
        format('[%d,%d,%d,%d]', r, g, b, a))
       return
      end
     end
     append('[')
     for i = 1, count do
      if i > 1 then append(',') end
      write(at(item, i), depth + 1)
     end
     append(']')
    else
     append('{')
     local first = true
     for key, entry in walk, item do
      if not first then append(',') end
      append(escape(key))
      append(':')
      write(entry, depth + 1)
      first = false
     end
     append('}')
    end
   else
    fail('Cannot serialize ' .. t)
   end
  end

  write(value, 0)
  chunks[#chunks + 1] = join(batch, '', 1, size)
  return join(chunks)
 end
end

function _pw_snapshot()
 -- Encoding the full document needs a larger allowance than one console command.
 installBudget(1000000000,5)
 return encodeJson(doc)
end
local initialized=false
function _pw_finish(command,discover)
 if not initialized then
  if type(init)=='function' then init(_pw_plugin) end
  initialized=true
 end
 if command and command~='' then
  local entry=assert(commands[command],'Plugin command was not registered: '..command)
  assert(not entry.onenabled or entry.onenabled(),'Plugin command is currently disabled: '..command)
  entry.onclick()
 end
 if not discover and _pw_loop then _pw_loop() end
 hook()
 local result=discover and pluginRegistration() or doc
 if not discover and _pw_workspace_snapshot then
  result=_pw_workspace_snapshot()
  result.registration=pluginRegistration()
 end
 _pw_result=encodeJson(result)
end
