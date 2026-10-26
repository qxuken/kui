-- Segments and radios: one Tab stop each, entered on the chosen item; the
-- arrows choose as they move.

local kit = require("kit.ui")

local demo = {
  title = "Segments and radios",
  note = "arrows choose as they move",
  call = 'kit.segmented({ value = model.view, options = {...}, on_change = { kind = "view" } })',
  state = { view = "table", level = "info" },
}

function demo.update(m, msg)
  if msg.kind == "view" then m.view = msg.change.value
  elseif msg.kind == "level" then m.level = msg.change.value
  end
end

function demo.view(m)
  return kit.segmented({ value = m.view, on_change = { kind = "view" },
      options = { { value = "table", label = "Table" }, { value = "list", label = "List" }, { value = "chart", label = "Chart" } } }),
    kit.radio_group({ label = "Level", value = m.level, on_change = { kind = "level" },
      options = { { value = "debug", label = "Debug" }, { value = "info", label = "Info" }, { value = "warn", label = "Warnings" } } }),
    text("view = " .. m.view .. "   level = " .. m.level, { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
