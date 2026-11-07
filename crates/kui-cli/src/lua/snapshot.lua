-- Immutable value graphs: table entries, aliases, cycles and channel revisions.
local S={}
function S.capture(root)
  local objects, seen = {}, {}
  local function encode(v)
    local kind=type(v)
    if kind=='nil' then return {kind='nil'} end
    if kind=='boolean' then return {kind=kind,value=v} end
    if kind=='string' then return {kind=kind,base64=kui.base64.encode(v)} end
    if kind=='number' then return {kind=math.type(v),value=math.type(v)=='integer' and tostring(v) or string.format('%.17g',v)} end
    if seen[v] then return {kind='ref',id=seen[v]} end
    local id=#objects+1; seen[v]=id
    local o={kind=kind,id=id}; objects[id]=o
    if kind=='table' then
      o.entries=kui.json.array({})
      local keys={}; for key in next,v do keys[#keys+1]=key end
      table.sort(keys,function(a,b)
        if type(a)==type(b) and (type(a)=='number' or type(a)=='string') then return a<b end
        return type(a)..tostring(a)<type(b)..tostring(b)
      end)
      for _,key in ipairs(keys) do o.entries[#o.entries+1]={key=encode(key),value=encode(rawget(v,key))} end
      local mt=getmetatable(v)
      if type(mt)=='table' and rawget(mt,'__kui_array') then o.array=true
      elseif mt~=nil then o.metatable='opaque'; o.restorable=false end
    elseif kind=='userdata' then
      local ok,names=pcall(function() return v.names end)
      if ok and type(names)=='table' then
        o.kind='channel'; o.revision=v.rev; o.columns={}
        for _,name in ipairs(names) do
          local values=kui.json.array({})
          for row=0,#v-1 do values[#values+1]=v:get(row,name) end
          o.columns[name]=encode(values)
        end
      else o.opaque=true; o.restorable=false end
    else o.opaque=true; o.restorable=false end
    return {kind='ref',id=id}
  end
  local value=encode(root)
  return {version=1,root=value,objects=kui.json.array(objects)}
end
function S.restore(graph, inspection)
  assert(graph.version==1,'unsupported state graph version')
  local values={}
  for _,o in ipairs(graph.objects) do
    if (o.opaque or o.metatable) and not inspection then error('state object '..o.id..' ('..o.kind..') needs a capture/render adapter') end
    values[o.id]=o.array and kui.json.array({}) or {}
  end
  local function decode(v)
    if v.kind=='nil' then return nil end
    if v.kind=='boolean' then return v.value end
    if v.kind=='string' then return kui.base64.decode(v.base64) end
    if v.kind=='integer' or v.kind=='float' then
      if v.value=='nan' or v.value=='-nan' or v.value=='NaN' then return 0/0 end
      if v.value=='inf' then return math.huge end
      if v.value=='-inf' then return -math.huge end
      local number=assert(tonumber(v.value),'invalid captured number')
      if v.kind=='float' then if v.value=='-0' then return -0.0 end;return number+0.0 end
      return number
    end
    assert(v.kind=='ref' and values[v.id],'invalid state graph reference')
    return values[v.id]
  end
  for _,o in ipairs(graph.objects) do
    local out=values[o.id]
    if o.kind=='table' then
      for _,entry in ipairs(o.entries) do out[decode(entry.key)]=decode(entry.value) end
    elseif o.kind~='channel' then out.__opaque=o.kind end
  end
  for _,o in ipairs(graph.objects) do
    if o.kind=='channel' then
      if o.columns_data then values[o.id]=kui.__restore_channel(o)
      else local columns={}; for name,v in pairs(o.columns) do columns[name]=decode(v) end;values[o.id]=channel(columns) end
    end
  end
  -- Resolve references to channels after channels have been materialized.
  for _,o in ipairs(graph.objects) do if o.kind=='table' then
    for key in next,values[o.id] do values[o.id][key]=nil end
    for _,entry in ipairs(o.entries) do values[o.id][decode(entry.key)]=decode(entry.value) end
  end end
  return decode(graph.root)
end
function S.encode(value)
  if kui.__snapshot_encode then return kui.__snapshot_encode(value) end
  return kui.json.encode(S.capture(value))
end
function S.decode(text,inspection) return S.restore(kui.json.decode(text),inspection) end
return S
