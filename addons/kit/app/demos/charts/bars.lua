-- Standing bars: requests by status, each value over its bar, one Tab
-- stop when the host picks with `on_select`.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "Bars",
  note = "values over the bars, a unit; one Tab stop when the host picks",
  call = 'kit.bar_chart({ data = by_status, label = "status", value = "n", unit = "req",\n  on_select = { kind = "bar" } })',
  state = { bar = nil },
}

function demo.update(m, msg)
  if msg.kind == "bar" then m.bar = msg.select.row end
end

function demo.view(m)
  local statuses = sample.by_status(m.rows)
  local picked = m.bar and #statuses > m.bar
    and (tostring(statuses:get(m.bar, "status")) .. ": " .. tostring(statuses:get(m.bar, "n")) .. " requests")
    or "Pick a bar: click it, or Tab to the bars and use the arrows."
  return kit.bar_chart({ data = statuses, label = "status", value = "n", unit = "req", height = 160, on_select = { kind = "bar" } }),
    text(picked, { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
