-- The packaged host services queued Lua tasks between frames.
local tasks, messages = {}, {}
local host = kui.platform == "web" and "in the browser" or ("on " .. kui.platform)

function kui.spawn(fn, ...)
  assert(type(fn) == "function", "kui.spawn takes a function")
  local thread = coroutine.create(fn)
  tasks[#tasks + 1] = { thread = thread, args = table.pack(...) }
  return thread
end

function kui.dispatch(message)
  messages[#messages + 1] = message
end

function on_event(event)
  local handle = rawget(_G, "update")
  if handle then handle(event) end
end

function __kui_run_tasks()
  local pending = tasks
  tasks = {}
  for _, task in ipairs(pending) do
    local ok, err = coroutine.resume(task.thread, table.unpack(task.args, 1, task.args.n))
    if not ok then
      kui.__task_error(tostring(err))
    elseif coroutine.status(task.thread) ~= "dead" then
      kui.__task_error("kui.spawn: yielding from a task is unavailable " .. host)
    end
  end
  local dispatched = messages
  messages = {}
  for _, message in ipairs(dispatched) do
    local handle = rawget(_G, "update") or rawget(_G, "on_event")
    if handle then
      local ok, err = pcall(handle, message)
      if not ok then kui.__task_error(tostring(err)) end
    end
  end
  return #pending > 0 or #dispatched > 0
end

local function raising(try)
  return function(...)
    local ok, result = try(...)
    if not ok then error(result, 2) end
    return result
  end
end

for _, name in ipairs({ "read", "write", "mkdir", "remove" }) do
  kui.fs[name] = raising(kui.fs["try_" .. name])
end
kui.json.decode = raising(kui.json.try_decode)

local function unavailable(name)
  local message = "kui." .. name .. " is unavailable " .. host
  return function() return false, message, "unavailable" end
end
for _, name in ipairs({ "process", "http", "sqlite", "secrets" }) do
  kui[name] = setmetatable({}, { __index = function(_, member)
    if type(member) ~= "string" then return nil end
    local try = unavailable(name .. "." .. member)
    if member:sub(1, 4) == "try_" then return try end
    return raising(try)
  end })
end
kui.try_worker = unavailable("worker")
kui.worker = raising(kui.try_worker)
package.loadlib = function() return nil, "native libraries are unavailable " .. host end
