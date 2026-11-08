-- The browser runner services Lua tasks between frames. Its file
-- operations use an in-memory WASI mount backed by browser storage.
local tasks, messages = {}, {}

function kui.spawn(task)
  tasks[#tasks + 1] = task
end

function kui.dispatch(message)
  messages[#messages + 1] = message
end

function on_event(event)
  if update then update(event) end
end

function __web_run_tasks()
  local pending = tasks
  tasks = {}
  for _, task in ipairs(pending) do task() end
  local dispatched = messages
  messages = {}
  if update then
    for _, message in ipairs(dispatched) do update(message) end
  end
  return #pending > 0 or #dispatched > 0
end

function kui.fs.read(path)
  local ok, result = kui.fs.try_read(path)
  if not ok then error(result, 2) end
  return result
end

local function unavailable(name)
  return setmetatable({}, { __index = function(_, member)
    return function() error("kui." .. name .. "." .. member .. " is unavailable in the browser", 2) end
  end })
end
for _, name in ipairs({ "process", "http", "worker", "sqlite", "secrets" }) do
  kui[name] = unavailable(name)
end
package.loadlib = function() return nil, "native libraries are unavailable in the browser" end
