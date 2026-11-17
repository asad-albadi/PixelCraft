-- ASE user properties on an open sprite. The codec edits the original metadata
-- chunk so unrelated namespaces and values retain their binary representation.
do
 local proxies=setmetatable({},{__mode='k'})
 local unusualValues=setmetatable({},{__mode='k'})
 local encodeValue,encodeMap

 encodeMap=function(value,seen)
  local entries={}
  for key,child in pairs(value)do
   assert(type(key)=='string','Property map names must be strings')
   entries[#entries+1]={key=key,value=encodeValue(child,seen)}
  end
  table.sort(entries,function(a,b)return a.key<b.key end)
  return entries
 end

 encodeValue=function(value,seen)
  local kind=type(value)
  if kind=='boolean' then return {kind='bool',value=value} end
  if kind=='string' then return {kind='string',value=value} end
  if kind=='number' then
   assert(value==value and math.abs(value)<math.huge,'Property number must be finite')
   return {kind=math.type(value)=='integer' and 'integer' or 'number',value=value}
  end
  assert(kind=='table','Unsupported property value: '..kind)
  local unusual=unusualValues[value]
  if unusual then return unusual end
  local meta=getmetatable(value)
  local name=meta and meta.__name
  if name=='Point' then return {kind='point',value={value.x,value.y}} end
  if name=='Size' then return {kind='size',value={value.width,value.height}} end
  if name=='Rectangle' then return {kind='rectangle',value={value.x,value.y,value.width,value.height}} end
  assert(not seen[value],'Cyclic property value')
  seen[value]=true
  local count=0
  local dense=true
  for key in pairs(value)do
   count=count+1
   if math.type(key)~='integer' or key<1 or key>#value then dense=false end
  end
  local result
  if dense and count==#value then
   local entries={}
   for index,child in ipairs(value)do entries[index]=encodeValue(child,seen) end
   result={kind='vector',value=entries}
  else
   result={kind='map',value=encodeMap(value,seen)}
  end
  seen[value]=nil
  return result
 end

 local function decodeValue(property)
  if not property then return nil end
  local kind,value=property.kind,property.value
  if kind=='point' then return Point(value) end
  if kind=='size' then return Size(value) end
  if kind=='rectangle' then return Rectangle(value) end
  if kind=='vector' then
   local result={}
   for index,item in ipairs(value)do result[index]=decodeValue(item) end
   return result
  end
  if kind=='map' then
   local result={}
   for _,entry in ipairs(value)do result[entry.key]=decodeValue(entry.value) end
   return result
  end
  if kind=='unsigned' or kind=='uuid' or kind=='opaque' then
   local result={kind=kind,value=copy(value)}
   unusualValues[result]=property
   return result
  end
  return value
 end

 local function spriteMetadata()
  for _,chunk in ipairs(doc.metadata)do
   if chunk.frame==0 and chunk.kind==0x2020 and chunk.owner=='palette' then return chunk end
  end
 end

 local function call(action,key,value,namespace)
  local previous=spriteMetadata()
  local data=previous and previous.data or {0,0,0,0}
  local request={op='aseProperties',action=action,data=data,key=key,namespace=namespace}
  local firstExternal
  if namespace and namespace~='' then
   request.external={}
   for _,chunk in ipairs(doc.metadata)do
    if chunk.frame==0 and chunk.kind==0x2008 and chunk.owner=='' then
     firstExternal=firstExternal or chunk
     request.external[#request.external+1]=chunk.data
    end
   end
  end
  if action=='set' and value~=nil then request.property=encodeValue(value,{}) end
  if action=='replace' then
   assert(type(value)=='table','Expected a property map')
   request.properties=encodeMap(value,{[value]=true})
  end
  local response=host(request,true)
  if action=='read' then return response end
  local identical=#response.data==#data
  if identical then
   for i,byte in ipairs(data)do
    if response.data[i]~=byte then identical=false;break end
   end
  end
  if identical and not response.external then return end
  app.transaction(function()
   if previous then previous.data=response.data
   else table.insert(doc.metadata,{frame=0,kind=0x2020,owner='palette',data=response.data}) end
   if response.external then
    if firstExternal then firstExternal.data=response.external
    else table.insert(doc.metadata,{frame=0,kind=0x2008,owner='',data=response.external}) end
   end
  end)
 end

 local function properties(owner,namespace)
  namespace=namespace or ''
  local scoped=proxies[owner]
  if not scoped then scoped=setmetatable({},{__mode='v'});proxies[owner]=scoped end
  if scoped[namespace] then return scoped[namespace] end
  local proxy=setmetatable({},{
   __index=function(_,key)
    assert(type(key)=='string','Property name must be a string')
    return withSprite(owner,function()return decodeValue(call('read',key,nil,namespace).property)end)
   end,
   __newindex=function(_,key,value)
    assert(type(key)=='string','Property name must be a string')
    withSprite(owner,call,'set',key,value,namespace)
   end,
   __pairs=function()
    local result=withSprite(owner,call,'read',nil,nil,namespace)
    local values={}
    for _,entry in ipairs(result.properties)do values[entry.key]=decodeValue(entry.value) end
    return next,values,nil
   end,
   __call=function(_,name,value)
    assert(type(name)=='string','Expected an extension name')
    if value~=nil then withSprite(owner,call,'replace',nil,value,name) end
    return properties(owner,name)
   end,
  })
  scoped[namespace]=proxy
  return proxy
 end

 local get,set=spriteMeta.__index,spriteMeta.__newindex
 -- The initial sprite already has the owner wrapper built by bindSpriteHandle.
 -- Keep that wrapper, and add the same properties field to the shared metatable
 -- that will be used when another sprite is opened.
 local function install(meta,get,set)
  meta.__index=function(handle,key)
   if key=='properties' then return properties(handle) end
   return get(handle,key)
  end
  meta.__newindex=function(handle,key,value)
   if key=='properties' then return withSprite(handle,call,'replace',nil,value) end
   return set(handle,key,value)
  end
 end
 install(spriteMeta,get,set)
 local current=getmetatable(sprite)
 install(current,current.__index,current.__newindex)
end
