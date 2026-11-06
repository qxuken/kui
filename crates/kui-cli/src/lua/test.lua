-- The public test vocabulary. Collection has no app startup side effects.
local t = kui.__test or {}
local cases, groups = {}, {}
local function trace(err) local text=tostring(err);if text:find('stack traceback:',1,true) then return text end;return debug.traceback(text,2) end
local function callable(fn, what) assert(type(fn) == 'function', what .. ' needs a function') end
function t.describe(name, fn)
  callable(fn, 'describe')
  groups[#groups + 1] = { name = name, before = {}, after = {} }
  local ok, err = pcall(fn)
  groups[#groups] = nil
  if not ok then error(err, 0) end
end
local root = { before = {}, after = {} }
function t.before_each(fn) callable(fn, 'before_each'); local g = groups[#groups] or root; g.before[#g.before + 1] = fn end
function t.after_each(fn) callable(fn, 'after_each'); local g = groups[#groups] or root; g.after[#g.after + 1] = fn end
function t.it(name, fn)
  callable(fn, 'it')
  local path, ancestry = {}, { root }
  for _, g in ipairs(groups) do path[#path + 1] = g.name; ancestry[#ancestry + 1] = g end
  path[#path + 1] = name
  local source = debug.getinfo(2, 'Sl')
  cases[#cases + 1] = { name = table.concat(path, ' / '), fn = fn, ancestry = ancestry,
    file = source.source:gsub("^@", ""), line = source.currentline }
end
local function equal(a, b, seen)
  if rawequal(a,b) then return true end
  if type(a) ~= 'table' or type(b) ~= 'table' then return false end
  seen = seen or {}; if seen[a] then return seen[a] == b end; seen[a] = b
  for k,v in next,a do if not equal(v, rawget(b,k), seen) then return false end end
  for k in next,b do if rawget(a,k) == nil then return false end end
  return true
end
local function repr(v, seen)
  if type(v) == 'string' then return string.format('%q',v) end
  if type(v) ~= 'table' then return tostring(v) end
  seen = seen or {}; if seen[v] then return '<ref ' .. seen[v] .. '>' end
  local n = 0; for _ in pairs(seen) do n=n+1 end; seen[v] = n+1
  local keys={}; for k in next,v do keys[#keys+1]=k end
  table.sort(keys,function(a,b) return type(a)..tostring(a)<type(b)..tostring(b) end)
  local rows={}; for _,k in ipairs(keys) do rows[#rows+1]='['..repr(k,seen)..']='..repr(v[k],seen) end
  return '{'..table.concat(rows,', ')..'}'
end
function t.expect(actual)
  local function make(negative)
    local e = {}
    local function matcher(name, fn)
      e[name] = function(a, ...)
        local args = a == e and table.pack(...) or table.pack(a, ...)
        local ok, description = fn(table.unpack(args,1,args.n))
        if negative then ok = not ok end
        if kui.__record then local info=debug.getinfo(2,'Sl');kui.__record('assertion',{ok=ok,actual=repr(actual),expected=(negative and 'never ' or '')..description,source=info and {file=info.source:gsub('^@',''),line=info.currentline}}) end
        if not ok then error('expected '..repr(actual)..(negative and ' never ' or ' ')..description, 2) end
        return e
      end
    end
    matcher('to_be', function(want) return rawequal(actual,want), 'to be '..repr(want) end)
    matcher('to_equal', function(want) return equal(actual,want), 'to equal '..repr(want) end)
    matcher('to_be_near', function(want, epsilon) return type(actual)=='number' and math.abs(actual-want)<=(epsilon or 0.000001), 'to be near '..repr(want)..' ± '..tostring(epsilon or 0.000001) end)
    matcher('to_contain', function(want)
      if type(actual)=='string' then return actual:find(want,1,true)~=nil, 'to contain '..repr(want) end
      if type(actual)=='table' then for _,v in pairs(actual) do if equal(v,want) then return true,'to contain '..repr(want) end end end
      return false, 'to contain '..repr(want)
    end)
    matcher('to_match', function(pattern) return type(actual)=='string' and actual:find(pattern)~=nil, 'to match '..repr(pattern) end)
    matcher('to_throw', function(pattern)
      local ok,err=pcall(actual)
      return not ok and (pattern==nil or tostring(err):find(pattern,1,true)~=nil), 'to throw '..repr(pattern)
    end)
    matcher('to_have_been_called', function(n) return actual.calls and (#actual.calls>0 and (n==nil or #actual.calls==n)), 'to have been called '..(n and tostring(n)..' times' or '') end)
    matcher('to_have_been_called_with', function(...)
      local want=table.pack(...)
      for _,call in ipairs(actual.calls or {}) do if equal(call.args,want) then return true,'to have been called with '..repr(want) end end
      return false,'to have been called with '..repr(want)
    end)
    return setmetatable(e,{__index=function(_,key) if key=='never' then return make(not negative) end end})
  end
  return make(false)
end
local restores={}
function t.spy(target, key)
  local original=key and target[key] or target
  callable(original,'spy')
  local spy={calls={}}
  setmetatable(spy,{__call=function(_, ...)
    local call={args=table.pack(...)}; spy.calls[#spy.calls+1]=call
    local out=table.pack(pcall(original,...))
    if not out[1] then call.error=out[2]; error(out[2],0) end
    call.returns=table.pack(table.unpack(out,2,out.n))
    return table.unpack(out,2,out.n)
  end})
  if key then target[key]=spy; restores[#restores+1]=function() target[key]=original end end
  return spy
end
function t.stub(target,key,replacement)
  local original=target[key]
  local fn=type(replacement)=='function' and replacement or function() return replacement end
  local spy=t.spy(fn); target[key]=spy
  restores[#restores+1]=function() target[key]=original end
  return spy
end
function t.replay(adapter)
  assert(type(adapter.capture)=='function' and type(adapter.renderer)=='string','replay needs capture and renderer')
  t.__adapter=adapter
end
function t.step(name, fn)
  if t.__source then local info=debug.getinfo(2,'Sl');t.__source(info.source:gsub('^@',''),info.currentline) end
  t.__boundary(name,'before')
  local out=table.pack(xpcall(fn,trace))
  t.__boundary(name,'after')
  if not out[1] then error(out[2],0) end
  return table.unpack(out,2,out.n)
end
function t.wait_for(cond, timeout)
  local elapsed=0
  while not cond() do
    if elapsed >= (timeout or 3000) then error('wait_for timed out after '..elapsed..' ms',2) end
    if t.advance then t.advance(10);t.__yield_io() else kui.sleep(10) end
    elapsed=elapsed+10
  end
end
function t.__list()
  local list={};for _,case in ipairs(cases) do list[#list+1]={name=case.name,file=case.file,line=case.line} end
  return list
end
function t.__run(filter, exact)
  local count=0
  for _,case in ipairs(cases) do
    if filter==nil or (exact and case.name==filter) or (not exact and case.name:find(filter,1,true)) then
      count=count+1
      t.__begin(case.name,case.file,case.line)
      local start=t.__wall_time()
      local ok,err=xpcall(function()
        for _,g in ipairs(case.ancestry) do for _,fn in ipairs(g.before) do fn() end end
        case.fn()
      end,trace)
      for i=#case.ancestry,1,-1 do
        for _,fn in ipairs(case.ancestry[i].after) do
          local clean,why=xpcall(fn,trace)
          if not clean then ok=false; err=(err and err..'\n' or '')..why end
        end
      end
      for i=#restores,1,-1 do restores[i]() end; restores={}
      t.__finish(case.name,ok,err,t.__wall_time()-start,case.file,case.line)
    end
  end
  return count
end
package.loaded['kui.test']=t
kui.test=t
return t
