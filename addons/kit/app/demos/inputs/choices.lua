-- Choices: checkboxes, switches, radios, segments and a slider. The
-- "Live feeds" switch sends the app's own `live` message, the one the
-- toolbar's Live switch sends: main.lua starts and stops the chart's and
-- the log's feeds with it.

local kit = require("kit.ui")

local demo = {
  title = "Choices",
  note = "boxes, switches, radios, segments, a slider",
  call = 'kit.checkbox({ label = "Follow", checked = model.follow, on_toggle = { kind = "follow" } })\nkit.slider({ label = "Zoom", value = model.zoom, value_label = "40%", on_change = { kind = "zoom" } })',
  state = { follow = true, pinned = true, level = "info", sort_by = "recent", view = "table", zoom = 0.4 },
}

function demo.update(m, msg)
  local k = msg.kind
  if k == "follow" then m.follow = msg.toggle.checked
  elseif k == "pinned" then m.pinned = msg.toggle.on
  elseif k == "level" then m.level = msg.change.value
  elseif k == "sort_by" then m.sort_by = msg.change.value
  elseif k == "view" then m.view = msg.change.value
  elseif k == "zoom" then m.zoom = msg.change.value
  end
end

function demo.view(m)
  return row { width = "grow", gap = 16, cross_gap = 16, min_column = 220,
    column { gap = 4,
      kit.checkbox({ label = "Follow the log", checked = m.follow, on_toggle = { kind = "follow" } }),
      kit.checkbox({ label = "Some of them", checked = false, mixed = true }),
      kit.switch({ label = "Live feeds", on = m.live, on_toggle = { kind = "live" } }),
      kit.switch({ label = "Pinned", on = m.pinned, on_toggle = { kind = "pinned" } }) },
    kit.radio_group({ label = "Level", value = m.level, on_change = { kind = "level" },
      options = { { value = "debug", label = "Debug" }, { value = "info", label = "Info" }, { value = "warn", label = "Warnings" } } }),
    column { gap = 4,
      text("hide_label: named, not drawn", { size = 11, family = "$kitUi", color = "$kitMuted" }),
      kit.radio_group({ label = "Sort", hide_label = true, value = m.sort_by, on_change = { kind = "sort_by" },
        options = { { value = "recent", label = "Most recent" }, { value = "name", label = "By name" } } }) },
    column { gap = 10,
      kit.segmented({ value = m.view, on_change = { kind = "view" },
        options = { { value = "table", label = "Table" }, { value = "list", label = "List" }, { value = "chart", label = "Chart" } } }),
      kit.slider({ label = "Zoom", value = m.zoom, min = 0, max = 1, step = 0.05,
        value_label = string.format("%d%%", math.floor(m.zoom * 100 + 0.5)), on_change = { kind = "zoom" } }) } }
end

return demo
