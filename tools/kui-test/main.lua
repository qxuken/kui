-- The runner's bundled test inspector shell (ADR 0072).
local inspector=require('inspector')
function init()
  local saved
  if kui.data_dir then local ok,text=kui.fs.try_read(kui.data_dir..'/layout.json');if ok then local fine,value=kui.json.try_decode(text);if fine then saved=value end end end
  local m=inspector.new(saved)
  inspector.update(m,{kind='refresh'})
  if kui.test_inspector.initial_tab then inspector.update(m,{kind='tab',change={value=kui.test_inspector.initial_tab}}) end
  return m
end
function update(msg)
  inspector.update(model,msg)
  if kui.data_dir and (msg.kind:match('^resize_') or msg.kind=='reset_layout') then
    kui.fs.try_mkdir(kui.data_dir)
    kui.fs.try_write(kui.data_dir..'/layout.json',kui.json.encode(model.layout))
  end
end
function view(env) return inspector.view(model,env) end
