-- Dialog canvases record bounded drawing commands on the Lua worker.
local paintDialogCanvas, configureDialogCanvas, validateDialogCanvasSize
do
 local inputEvents={'onmousedown','onmouseup','onmousemove','ondblclick','onwheel','onkeydown','onkeyup'}

 validateDialogCanvasSize=function(width,height)
  assert(math.type(width)=='integer' and math.type(height)=='integer' and
    width>=1 and width<=1024 and height>=1 and height<=1024,'Invalid dialog canvas size')
 end

 configureDialogCanvas=function(widget,spec)
  spec.canvasEvents={}
  for _,name in ipairs(inputEvents)do
   if type(widget[name])=='function' then spec.canvasEvents[name]=true end
  end
  if widget._canvasDirty then
   widget._canvasDirty=false
   spec.paint=paintDialogCanvas(widget)
  end
 end

 local function measure(n)
  assert(type(n)=='number' and n==n and math.abs(n)<=32768,'Invalid canvas coordinate')
  return n
 end

 local function rect(...)
  local b=Rectangle(...)
  return {measure(b.x),measure(b.y),measure(b.width),measure(b.height)}
 end

 paintDialogCanvas=function(widget)
  local commands,state={},{}
  local gc={width=widget.width,height=widget.height,color=Color('#ffffff'),opacity=255,strokeWidth=1}
  local pixelsUsed=0
  local path,subpath={},nil
  local function emit(command)
   assert(#commands<2048,'Canvas has too many drawing operations')
   commands[#commands+1]=command
  end
  local function paintColor(self)
   local color=unpackColor(self.color)
   color[4]=math.floor((color[4]*byte(self.opacity)+127)/255)
   return color
  end
  function gc:save()
   assert(#state<32,'Canvas state stack exceeds its limit')
   state[#state+1]={Color(self.color),self.opacity,self.strokeWidth}
  end
  function gc:restore()
   local saved=table.remove(state)
   if saved then self.color,self.opacity,self.strokeWidth=saved[1],saved[2],saved[3] end
  end
  function gc:fillRect(...)
   emit{kind='fillRect',bounds=rect(...),color=paintColor(self)}
  end
  function gc:strokeRect(...)
   emit{kind='strokeRect',bounds=rect(...),color=paintColor(self),width=math.max(0,measure(self.strokeWidth))}
  end
  function gc:fillText(text,x,y)
   assert(type(text)=='string' and #text<=4096,'Canvas text exceeds its limit')
   emit{kind='text',text=text,position={measure(x),measure(y)},color=paintColor(self)}
  end
  function gc:beginPath()path,subpath={},nil end
  function gc:moveTo(x,y)
   assert(#path<256,'Canvas path has too many subpaths')
   subpath={{measure(x),measure(y)}}
   path[#path+1]=subpath
  end
  function gc:lineTo(x,y)
   if not subpath then self:moveTo(x,y);return end
   assert(#subpath<512,'Canvas subpath has too many points')
   subpath[#subpath+1]={measure(x),measure(y)}
  end
  function gc:closePath()
   if subpath then self:lineTo(table.unpack(subpath[1])) end
  end
  function gc:stroke()
   local color,width=paintColor(self),math.max(0,measure(self.strokeWidth))
   for _,points in ipairs(path)do
    if #points>1 then emit{kind='line',points=copy(points),color=color,width=width} end
   end
  end
  function gc:drawImage(image,...)
   assert(type(image)=='table' and image._pixels,'Canvas drawImage expects an Image')
   local n=select('#',...)
   local w,h=image.width,image.height
   local source,dest={0,0,w,h},nil
   if n==2 then
    local a,b=...
    if type(a)=='table' then source,dest=rect(a),rect(b)
    else dest=rect(a,b,w,h) end
   elseif n==8 then
    local sx,sy,sw,sh,dx,dy,dw,dh=...
    source,dest=rect(sx,sy,sw,sh),rect(dx,dy,dw,dh)
   else error('Canvas drawImage needs a position or source and destination rectangles') end
   assert(w<=2048 and h<=2048 and w*h+pixelsUsed<=131072,'Canvas images exceed the pixel budget')
   pixelsUsed=pixelsUsed+w*h
   emit{kind='image',size={w,h},pixels=copy(image._pixels),source=source,destination=dest,opacity=byte(self.opacity)}
  end

  if widget.onpaint then widget.onpaint{context=gc} end
  return commands
 end
end
