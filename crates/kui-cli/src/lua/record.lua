-- Record below the public raising/waiting wrappers, preserving all returns.
local graph=assert(load(...,'@kui:state-graph'))()
function kui.__record(kind,value)
  if not kui.__record_enabled() then return end
  kui.__record_graph(kind,graph.encode(value))
end
function kui.__record_wrap(service,tab)
  for name,fn in pairs(tab) do
    if type(fn)=='function' then
      tab[name]=function(...)
        if not kui.__record_enabled() then return fn(...) end
        kui.__record('call',{service=service,op=name,args=table.pack(...)})
        local out=table.pack(pcall(fn,...))
        kui.__record('return',{service=service,op=name,values=out})
        if not out[1] then error(out[2],0) end
        return table.unpack(out,2,out.n)
      end
    end
  end
end
for _,service in ipairs({'fs','http','process','sqlite','secrets'}) do kui.__record_wrap(service,kui[service]) end
local instrumented=setmetatable({}, {__mode="k"})
function kui.__instrument()
  if not kui.__record_enabled() then return end
  for _,name in ipairs({'update','on_event','on_message'}) do
    local fn=rawget(_G,name)
    if type(fn)=='function' and not instrumented[fn] then _G[name]=function(...)
      kui.__record('message',{handler=name,phase='before',args=table.pack(...)})
      local out=table.pack(pcall(fn,...))
      kui.__record('message',{handler=name,phase='after',returns=out})
      if not out[1] then error(out[2],0) end
      return table.unpack(out,2,out.n)
    end;instrumented[_G[name]]=true end
  end
end
