-- The book runner services Lua tasks between frames. Its file
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

-- Give touch readers the audience's existing navigation actions.
function __book_presentation_controls()
  local kit = require("kit.ui")
  local audience = view
  function view(env)
    local root = audience(env)
    table.insert(root, row {
      pad = 12, gap = 8,
      kit.button({ label = "Previous", on_press = { presentation = "previous" } }),
      kit.button({ label = "Next", on_press = { presentation = "next" } }),
    })
    return root
  end
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
