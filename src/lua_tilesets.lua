-- Tileset operations share the document and image bindings from lua_api.lua.
do
 local TILE_ID_MASK=0x1fffffff
 local tilesetHandles=setmetatable({},{__mode='kv'})
 local detached=setmetatable({},{__mode='k'})
 local tilesetObjects=setmetatable({},{__mode='k'})
 local tileHandles=setmetatable({},{__mode='kv'})
 local tileObjects=setmetatable({},{__mode='k'})
 local retired=setmetatable({},{__mode='k'})
 local methods={}

 local function setIndex(set)
  for i,candidate in ipairs(doc.tilesets)do if candidate==set then return i end end
  error('Tileset is deleted or belongs to another document state')
 end

 local function propertySlot(set,ordinal)
  local information=detached[set]
  if information then return 'tileset:0',0,ordinal,information.metadata end
  return 'tileset:'..(setIndex(set)-1),0,ordinal
 end

 local function metadataChunks(metadata,owner)
  local user,external={},{}
  local firstExternal
  for _,chunk in ipairs(metadata)do
   if chunk.frame==0 then
    if chunk.owner==owner and chunk.kind==0x2020 then
     user[#user+1]=chunk.data
    elseif chunk.owner=='' and chunk.kind==0x2008 then
     firstExternal=firstExternal or chunk
     external[#external+1]=chunk.data
    end
   end
  end
  return user,external,firstExternal
 end

 local function importTilesetProperties(source,owner,target)
  local data,external=metadataChunks(source,owner)
  if #data==0 then return end
  return importUserData(data,external,target)
 end

 local function appendTilesetProperties(metadata,owner,imported,firstExternal)
  if not imported then return end
  for _,data in ipairs(imported.data)do
   metadata[#metadata+1]={frame=0,kind=0x2020,owner=owner,data=data}
  end
  if imported.external then
   if firstExternal then firstExternal.data=imported.external
   else metadata[#metadata+1]={frame=0,kind=0x2008,owner='',data=imported.external} end
  end
 end

 local function tilesetObject(handle)
  local set=assert(tilesetObjects[handle],'Invalid tileset')
  if detached[set] then return set end
  return set,setIndex(set)
 end

 local function documentTileset(handle)
  local set,index=tilesetObject(handle)
  assert(index,'Import the clipboard tileset into this sprite first')
  return set,index
 end

 local function tileObject(handle)
  local entry=assert(tileObjects[handle],'Invalid tile')
  if not detached[entry.set] then setIndex(entry.set) end
  if not entry.pixels then return entry.set,0 end
  for i,pixels in ipairs(entry.set.tiles)do
   if pixels==entry.pixels then return entry.set,i,pixels end
  end
  error('Tile is deleted')
 end

 local function infoAt(index)
  for _,info in ipairs(doc.ase_tilesets)do if info.index==index-1 then return info end end
 end

 local function preserveInfo(index)
  local info=infoAt(index)
  if not info then
   info={index=index-1,id=index-1,flags=6,base_index=1}
   doc.ase_tilesets[#doc.ase_tilesets+1]=info
  end
  return info
 end

 local function rememberInsertedSet(index,sourceInfo)
  local used={}
  for i=1,index-1 do
   local info=infoAt(i)
   used[info and info.id or i-1]=true
  end
  if sourceInfo or used[index-1] then
   local id=index-1
   while used[id] do id=id+1 end
   local dst=preserveInfo(index)
   dst.id=id
   if sourceInfo then
    dst.base_index=sourceInfo.base_index
    dst.flags=sourceInfo.flags & ~1
   end
  end
 end

 local function tilesetBudget(width,height,count)
  assert(width>0 and height>0 and width<=16384 and height<=16384,'Invalid tile dimensions')
  local area=width*height*count
  for _,set in ipairs(doc.tilesets)do area=area+set.width*set.height*#set.tiles end
  assert(area<=1000000,'Tilesets exceed the Lua pixel budget')
 end

 local function tileImage(handle)
  local set,_,pixels=tileObject(handle)
  if detached[set] then
   if not pixels then return Image(set.width,set.height,ColorMode.RGB) end
   return image(pixels,set.width,set.height,ColorMode.RGB,0)
  end
  if not pixels then return Image(set.width,set.height) end
  local document=doc
  local im=image(pixels,set.width,set.height)
  im._put=function()
   assert(doc==document and not retired[set] and not retired[pixels],'Tile image is no longer editable')
  end
  return bindSpriteHandle(im,sprite)
 end

 local function tileHandle(set,index)
  local pixels=set.tiles[index]
  if index~=0 and not pixels then return nil end
  local key=pixels or set
  if tileHandles[key] then return tileHandles[key] end
  local handle=setmetatable({},{
   __index=function(self,k)
    if k=='image' then return tileImage(self) end
    local _,index=tileObject(self)
    if k=='index' then return index end
   end,
   __newindex=function(self,k,value)
    local set,index,pixels=tileObject(self)
    assert(k=='image','Unsupported tile assignment')
    assert(index~=0,'The empty tile cannot be replaced')
    assert(value and value._pixels and value.width==set.width and value.height==set.height,
     'Tile image dimensions must match the tileset')
    local replacement=copy(value._pixels)
    assert(#replacement==set.width*set.height,'Invalid tile pixels')
    for i,color in ipairs(replacement)do pixels[i]=color end
   end,
  })
  if not detached[set] then bindSpriteHandle(handle,sprite) end
  tileObjects[handle]={set=set,pixels=pixels}
  tileHandles[key]=handle
  bindUserProperties(handle,not detached[set] and sprite or false,function()
   local current,tileIndex=tileObject(handle)
   return propertySlot(current,tileIndex+2)
  end)
  return handle
 end

 function methods:tile(index)
  local set=tilesetObject(self)
  assert(math.type(index)=='integer','Tile index must be an integer')
  return tileHandle(set,index)
 end

 function methods:getTile(index)
  local handle=methods.tile(self,index)
  return handle and tileImage(handle)
 end

 tilesetHandle=function(set)
  if tilesetHandles[set] then return tilesetHandles[set] end
  local handle=setmetatable({},{
   __index=function(self,k)
    local value,index=tilesetObject(self)
    if methods[k] then return methods[k] end
    if k=='name' then return value.name end
    if k=='grid' then
     return {tileSize=Size(value.width,value.height),origin=Point(),
      bounds=Rectangle(0,0,value.width,value.height)}
    end
    if k=='baseIndex' then
     local info=detached[value] or infoAt(index)
     return info and info.base_index or 1
    end
   end,
   __newindex=function(self,k,value)
    local set,index=tilesetObject(self)
    if k=='name' then
     local name=tostring(value)
     assert(#name<=65535,'Tileset name is too long')
     set.name=name
    elseif k=='baseIndex' then
     assert(math.type(value)=='integer' and value>=-32768 and value<=32767,'Invalid tileset base index')
     local info=detached[set] or preserveInfo(index)
     info.base_index=value
    else error('Unsupported tileset assignment: '..k) end
   end,
   __len=function(self) return #tilesetObject(self).tiles+1 end,
  })
  if not detached[set] then bindSpriteHandle(handle,sprite) end
  tilesetObjects[handle]=set
  tilesetHandles[set]=handle
  bindUserProperties(handle,not detached[set] and sprite or false,function()
   return propertySlot(set,1)
  end)
  return handle
 end

 snapshotTileset=function(handle)
  local function snapshot()
   local set,index=tilesetObject(handle)
   local info=detached[set] or infoAt(index)
   local data,external
   if not index then data,external=metadataChunks(info.metadata,'tileset:0')
   else
    local portable=importTilesetProperties(doc.metadata,'tileset:'..(index-1),{})
    if portable then data,external=portable.data,portable.external and {portable.external} end
   end
   return {set=copy(set),flags=info and (info.flags & ~1) or 6,
    base_index=info and info.base_index or 1,user_data=data,external=external}
  end
  local owner=spriteOwners[handle]
  if owner then return withSprite(owner,snapshot) end
  return snapshot()
 end

 detachedTileset=function(snapshot)
  local set=copy(snapshot.set)
  expandDocumentPixels({layers={},tilesets={set}})
  local metadata={}
  for _,data in ipairs(snapshot.user_data or {})do
   metadata[#metadata+1]={frame=0,kind=0x2020,owner='tileset:0',data=copy(data)}
  end
  for _,data in ipairs(snapshot.external or {})do
   metadata[#metadata+1]={frame=0,kind=0x2008,owner='',data=copy(data)}
  end
  detached[set]={flags=snapshot.flags,base_index=snapshot.base_index,metadata=metadata}
  return tilesetHandle(set)
 end

 spriteTilesets=function()
  local result={}
  for index,set in ipairs(doc.tilesets)do result[index]=tilesetHandle(set) end
  return result
 end

 local function remapPlacements(setIndex,tileIndex,insert)
  for _,layer in ipairs(doc.layers)do
   if layer.tileset==setIndex-1 then
    for _,cells in ipairs(layer.tile_cels)do
     for i,packed in ipairs(cells)do
      local id=packed&TILE_ID_MASK
      if insert then
       if id>=tileIndex then cells[i]=(packed&~TILE_ID_MASK)|(id+1) end
      elseif id==tileIndex then cells[i]=0
      elseif id>tileIndex then cells[i]=(packed&~TILE_ID_MASK)|(id-1) end
     end
    end
   end
  end
 end

 local function remapTileMetadata(setIndex,tileIndex,insert)
  local owner='tileset:'..(setIndex-1)
  local chunks={}
  for i,metadata in ipairs(doc.metadata)do
   if metadata.frame==0 and metadata.owner==owner and metadata.kind==0x2020 then
    chunks[#chunks+1]=i
   end
  end
  -- The tileset record and empty tile occupy the first two entries.
  local position=tileIndex+2
  if insert then
   local before=chunks[position]
   if before then
    table.insert(doc.metadata,before,{frame=0,kind=0x2020,owner=owner,data={0,0,0,0}})
   end
  elseif chunks[position] then
   table.remove(doc.metadata,chunks[position])
  end
 end

 function spriteMethods:newTileset(grid,count)
  local source=tilesetObjects[grid]
  local set,sourceInfo,sourceIndex
  if source then
   source,sourceIndex=tilesetObject(grid)
   tilesetBudget(source.width,source.height,#source.tiles)
   set=copy(source)
   sourceInfo=detached[source] or infoAt(sourceIndex)
  else
   local bounds
   if grid==nil then
    local size=doc.drawing_grid and doc.drawing_grid.size or {16,16}
    bounds=Rectangle(0,0,size[1],size[2])
   elseif type(grid)=='table' and grid.tileSize then
    local origin=Point(grid.origin)
    bounds=Rectangle(origin.x,origin.y,grid.tileSize.width,grid.tileSize.height)
   else
    assert(type(grid)=='table','Tile grid must be a rectangle or grid')
    bounds=Rectangle(grid)
   end
   assert(bounds.x==0 and bounds.y==0,'The tileset grid origin must be zero')
   count=count or 1
   assert(math.type(count)=='integer' and count>=1 and count<=1000000,'Invalid tile count')
   tilesetBudget(bounds.width,bounds.height,count-1)
   set={name='Tileset',width=bounds.width,height=bounds.height,tiles={}}
   for i=1,count-1 do set.tiles[i]=Image(set.width,set.height)._pixels end
  end
  local index=#doc.tilesets+1
  local owner='tileset:'..(index-1)
  local imported,firstExternal
  if sourceInfo and sourceInfo.metadata then
   local _,external,first=metadataChunks(doc.metadata,owner)
   firstExternal=first
   imported=importTilesetProperties(sourceInfo.metadata,'tileset:0',external)
  end
  doc.tilesets[index]=set
  rememberInsertedSet(index,sourceInfo)
  appendTilesetProperties(doc.metadata,owner,imported,firstExternal)
  if sourceIndex then
   local owner='tileset:'..(sourceIndex-1)
   for i=1,#doc.metadata do
    local metadata=doc.metadata[i]
    if metadata.owner==owner then
     local duplicate=copy(metadata)
     duplicate.owner='tileset:'..(index-1)
     doc.metadata[#doc.metadata+1]=duplicate
    end
   end
  end
  return tilesetHandle(set)
 end

 function spriteMethods:deleteTileset(value)
  local index=math.type(value)=='integer' and value+1 or setIndex(tilesetObjects[value])
  assert(doc.tilesets[index],'Tileset not found')
  for _,layer in ipairs(doc.layers)do
   assert(layer.tileset~=index-1,'Tileset is still used by a tilemap layer')
  end
  for i=index+1,#doc.tilesets do preserveInfo(i) end
  retired[table.remove(doc.tilesets,index)]=true
  for _,layer in ipairs(doc.layers)do
   if layer.tileset and layer.tileset>=index then layer.tileset=layer.tileset-1 end
  end
  for i=#doc.ase_tilesets,1,-1 do
   local info=doc.ase_tilesets[i]
   if info.index==index-1 then table.remove(doc.ase_tilesets,i)
   elseif info.index>=index then info.index=info.index-1 end
  end
  removeIndexedMetadata('tileset',index)
 end

 function spriteMethods:newTile(handle,index)
  local set,setIndex=documentTileset(handle)
  index=index or #set.tiles+1
  assert(math.type(index)=='integer' and index>=1 and index<=#set.tiles+1,'Invalid tile index')
  tilesetBudget(set.width,set.height,1)
  table.insert(set.tiles,index,Image(set.width,set.height)._pixels)
  remapPlacements(setIndex,index,true)
  remapTileMetadata(setIndex,index,true)
  return tileHandle(set,index)
 end

 function spriteMethods:deleteTile(handle,index)
  local set,position
  if tilesetObjects[handle] then set,position=documentTileset(handle)
  else
   set,index=tileObject(handle)
   position=setIndex(set)
  end
  assert(math.type(index)=='integer' and index>0 and set.tiles[index],'Invalid or empty tile index')
  retired[table.remove(set.tiles,index)]=true
  remapPlacements(position,index,false)
  remapTileMetadata(position,index,false)
 end

 assignLayerTileset=function(layer,handle)
  assert(layer.tileset,'Choose a tilemap layer')
  local set,index=documentTileset(handle)
  local old=doc.tilesets[layer.tileset+1]
  assert(set.width==old.width and set.height==old.height,'Replacement tile size must match')
  for _,cells in ipairs(layer.tile_cels)do
   for _,packed in ipairs(cells)do
    assert((packed&TILE_ID_MASK)<=#set.tiles,'Replacement tileset is missing a placed tile')
   end
  end
  layer.tileset=index-1
 end

 function spriteMethods:newTilemap(handle)
  local set,index=documentTileset(handle)
  local layer=self:newLayer()
  local value=doc.layers[layer._index]
  value.name='Tile layer'
  value.tileset=index-1
  local count=math.ceil(doc.width/set.width)*math.ceil(doc.height/set.height)
  for _,cells in ipairs(value.tile_cels)do for i=1,count do cells[i]=0 end end
  return layer
 end
end
