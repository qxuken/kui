-- A self-contained kui extension: owns its state, describes its UI as data,
-- and receives its own click events. Loaded by the lua_panel example.
--
-- It fills the slot the host names "panel" (docs/adr/0014), reads the title
-- and the reply template the host passes with it from `slot.params`, and
-- answers a toggle by returning the host's template from on_event.

slots = { "panel" }

todos = { "ship the layout solver", "wire up wgpu", "write this panel" }
done = {}
filter = ""
filter_key = nil -- set by a "changed" event; read back in view(env)
title = "lua panel"
on_toggle = nil -- the host's reply template, kept from view for on_event

function view(env, slot)
  local params = slot and slot.params or {}
  title = params.title or "lua panel"
  on_toggle = params.on_toggle

  if filter_key then
    filter = env.edit_text(filter_key) or ""
  end

  local items = {}
  for i, todo in ipairs(todos) do
    if filter == "" or todo:find(filter, 1, true) then
      local checked = done[i] and "[x] " or "[ ] "
      local color = done[i] and 0x5c6174ff or 0xe8e8eaff
      items[#items + 1] = row {
        gap = 8,
        cross_align = "center",
        on_click = { kind = "toggle", index = i },
        tooltip = done[i] and "click to reopen" or "click to finish",
        pad = { t = 4, b = 4 },
        text(checked .. todo, { size = 14, color = color }),
      }
    end
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
    text({ title, { " · " .. remaining .. " left", color = "#8a8fa3" } },
         { size = 12, color = 0x8a8fa3ff }),
    edit { key = "filter", initial = "", size = 14, width = "grow",
           pad = { l = 8, r = 8, t = 6, b = 6 }, bg = 0x0e1016ff, radius = 6 },
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
    -- The host asked to hear about this: its template, with the index
    -- filled in, is the reply. Returned rather than sent - a reply is data.
    if on_toggle then
      local reply = { index = ev.index, done = done[ev.index] or false }
      for k, v in pairs(on_toggle) do reply[k] = v end
      return reply
    end
  elseif ev.kind == "changed" then
    filter_key = ev.node_key
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
