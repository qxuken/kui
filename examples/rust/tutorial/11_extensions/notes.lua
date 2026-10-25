-- The notes plugin of step 11: it provides a list of notes, `notes/list`,
-- and adds a "New note" button to any slot named `app.toolbar`.

-- ANCHOR: provides
local notes = { "Buy milk", "Call the dentist", "Water the ferns" }

components = {}

-- Placed by the host as `notes/list`: `props` is what it passed.
function components.list(env, props, inst)
  local rows = {}
  for i, title in ipairs(notes) do
    rows[#rows + 1] = button { label = title, on_click = { kind = "pick", index = i } }
  end
  return column { key = "list", gap = 6, width = "grow",
    text(props.title or "Notes", { size = 12, color = env.theme.muted }),
    table.unpack(rows),
  }
end
-- ANCHOR_END: provides

-- ANCHOR: contributes
contributes = { ["app.toolbar"] = "new" }

function components.new(env, props, inst)
  return button { label = "New note", on_click = { kind = "new" } }
end
-- ANCHOR_END: contributes

-- ANCHOR: events
function on_event(ev)
  if ev.kind == "pick" then
    return emit("on_pick", { title = notes[ev.index] })
  elseif ev.kind == "new" then
    notes[#notes + 1] = "Note " .. (#notes + 1)
    return emit("on_run", { action = "new note" })
  end
end
-- ANCHOR_END: events
