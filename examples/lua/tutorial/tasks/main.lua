-- The book's runner chapter: a task list on Kit, kept in a JSON file.
--
--   kui run examples/lua/tutorial/tasks
--   kui run examples/lua/tutorial/tasks -- --file /tmp/tasks.json

-- ANCHOR: args
local opts = kui.parse_args({
  about = "a task list on Kit",
  options = {
    file = { default = (kui.env.HOME or ".") .. "/.local/share/kui-tasks.json", value = "path",
      help = "where the tasks are kept" },
  },
})
-- ANCHOR_END: args

local kit = require("kit.ui")

-- ANCHOR: init
function init()
  local tasks = {}
  if kui.fs.exists(opts.file) then tasks = kui.json.decode(kui.fs.read(opts.file)) end
  return { tasks = tasks, draft = "", error = nil, saving = false }
end
-- ANCHOR_END: init

-- ANCHOR: save
-- Writes the file in a task, so the window keeps drawing, and says so
-- with a message when it is done.
local function save()
  model.saving = true
  local text = kui.json.encode(kui.json.array(model.tasks))
  kui.spawn(function()
    kui.fs.write(opts.file, text)
    kui.dispatch({ kind = "saved" })
  end)
end
-- ANCHOR_END: save

-- ANCHOR: update
function update(msg)
  if msg.kind == "draft" then
    model.draft, model.error = msg.change.text, nil
  elseif msg.kind == "add" then
    local text = model.draft:match("^%s*(.-)%s*$")
    if text == "" then
      model.error = "Name the task first"
      return
    end
    model.tasks[#model.tasks + 1] = { text = text, done = false }
    model.draft = ""
    save()
  elseif msg.kind == "toggle" then
    model.tasks[msg.index].done = msg.toggle.checked
    save()
  elseif msg.kind == "clear" then
    local left = {}
    for _, t in ipairs(model.tasks) do
      if not t.done then left[#left + 1] = t end
    end
    model.tasks = left
    save()
  elseif msg.kind == "saved" then
    model.saving = false
  end
end
-- ANCHOR_END: update

-- ANCHOR: view
function done_count()
  local n = 0
  for _, t in ipairs(model.tasks) do
    if t.done then n = n + 1 end
  end
  return n
end

function view(env)
  local rows = {}
  for i, t in ipairs(model.tasks) do
    rows[#rows + 1] = kit.checkbox({ label = t.text, checked = t.done,
      on_toggle = { kind = "toggle", index = i } })
  end
  local done = done_count()
  return column { width = "grow", height = "grow", pad = 16, gap = 12, bg = "$kitBg",
    kit.toolbar({}, {
      kit.text_field({ label = "New task", hide_label = true, placeholder = "What needs doing?",
        value = model.draft, error = model.error,
        on_change = { kind = "draft" }, on_submit = { kind = "add" } }),
      kit.button({ label = "Add", variant = "primary", on_press = { kind = "add" } }),
      trailing = {
        kit.button({ label = "Clear done", disabled = done == 0, on_press = { kind = "clear" } }),
      },
    }),
    kit.stat({ label = "Done", value = done .. " of " .. #model.tasks }),
    column { key = "tasks", gap = 4, width = "grow", height = "grow", scroll = true, table.unpack(rows) },
  }
end
-- ANCHOR_END: view
