-- A self-contained kui extension: owns its state, describes its UI as data,
-- and receives its own click events. Loaded by the lua_panel example.

todos = { "ship the layout solver", "wire up wgpu", "write this panel" }
done = {}

function view()
  local items = {}
  for i, todo in ipairs(todos) do
    local checked = done[i] and "[x] " or "[ ] "
    local color = done[i] and 0x5c6174ff or 0xe8e8eaff
    items[#items + 1] = row {
      gap = 8,
      cross_align = "center",
      on_click = { kind = "toggle", index = i },
      pad = { t = 4, b = 4 },
      text(checked .. todo, { size = 14, color = color }),
    }
  end

  local remaining = #todos
  for i in pairs(done) do
    if done[i] then remaining = remaining - 1 end
  end

  return column {
    width = 300,
    height = "grow",
    pad = 16,
    gap = 10,
    bg = 0x14161eff,
    radius = 10,
    border = { w = 1, color = 0x2a2d3aff },
    text("lua panel", { size = 12, color = 0x8a8fa3ff }),
    text(remaining .. " left", { size = 22 }),
    column { height = "grow", scroll = true, table.unpack(items) },
    row {
      gap = 8,
      button { label = "add", on_click = { kind = "add" } },
      button { label = "clear done", on_click = { kind = "clear" } },
    },
  }
end

function on_event(ev)
  if ev.kind == "toggle" then
    done[ev.index] = not done[ev.index]
  elseif ev.kind == "add" then
    todos[#todos + 1] = "todo #" .. (#todos + 1)
  elseif ev.kind == "clear" then
    local kept, kept_done = {}, {}
    for i, todo in ipairs(todos) do
      if not done[i] then kept[#kept + 1] = todo end
    end
    todos, done = kept, kept_done
  end
end
