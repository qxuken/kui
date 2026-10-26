-- Bars from the keyboard: Left and Right walk them, the one the keyboard
-- is on looks hovered and shows its tooltip, Enter picks it.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "Bars",
  note = "Left and Right walk the bars, the one the keyboard is on looks hovered and shows its tooltip, Enter picks it",
  call = 'kit.bar_chart({ data = by_status, label = "status", value = "n", on_select = { kind = "bar" } })',
  state = { bar = nil },
}

function demo.update(m, msg)
  if msg.kind == "bar" then m.bar = msg.select.row end
end

function demo.view(m)
  local statuses = sample.by_status(m.rows)
  return kit.bar_chart({ data = statuses, label = "status", value = "n", height = 140, on_select = { kind = "bar" } }),
    text(m.bar and #statuses > m.bar and ("bar = " .. tostring(statuses:get(m.bar, "status"))) or "bar = none",
      { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
