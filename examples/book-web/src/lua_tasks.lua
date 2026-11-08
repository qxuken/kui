-- The book runner services the example's tasks between frames. Its file
-- operations use an in-memory WASI mount backed by browser storage.
local tasks, messages = {}, {}

function kui.spawn(task)
  tasks[#tasks + 1] = task
end

function kui.dispatch(message)
  messages[#messages + 1] = message
end

function on_event(event)
  update(event)
end

function __book_run_tasks()
  local pending = tasks
  tasks = {}
  for _, task in ipairs(pending) do task() end
  local dispatched = messages
  messages = {}
  for _, message in ipairs(dispatched) do update(message) end
  return #pending > 0 or #dispatched > 0
end
