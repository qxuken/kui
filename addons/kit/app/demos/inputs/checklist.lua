-- A checklist: one Tab stop, Up and Down walk the items, Space or a click
-- ticks the one focused, and an × removes it.

local kit = require("kit.ui")

local demo = {
  title = "Checklist",
  note = "one Tab stop: the arrows walk, Space ticks, an × removes",
  call = 'kit.checklist({ items = model.todo, removable = true,\n  on_toggle = { kind = "todo_tick" }, on_remove = { kind = "todo_remove" } })',
  state = { todo = {
    { label = "Write the release notes", checked = true },
    { label = "Redraw the samples' pictures", checked = false },
    { label = "Run the full suite", checked = false },
    { label = "Tag the release", checked = false },
  } },
}

function demo.update(m, msg)
  if msg.kind == "todo_tick" then
    local it = m.todo[msg.toggle.index + 1]
    if it then it.checked = msg.toggle.checked end
  elseif msg.kind == "todo_remove" then
    table.remove(m.todo, msg.remove.index + 1)
  end
end

function demo.view(m)
  local done = 0
  for _, it in ipairs(m.todo) do if it.checked then done = done + 1 end end
  return kit.checklist({ items = m.todo, label = "Release", removable = true,
      on_toggle = { kind = "todo_tick" }, on_remove = { kind = "todo_remove" } }),
    text(string.format("%d of %d done", done, #m.todo), { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
