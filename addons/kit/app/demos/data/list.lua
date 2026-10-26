-- A list: a title and a detail a row, and a badge in each row's tone.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "List",
  note = "a title and a detail a row, a badge in its tone",
  call = 'kit.list({ rows = endpoints, title = "title", detail = "detail",\n  badge = "method", tone = "tone", badge_width = 48, selected = model.item, on_select = { kind = "item" } })',
  state = { item = nil },
}

function demo.update(m, msg)
  if msg.kind == "item" then m.item = msg.select.row end
end

function demo.view(m)
  return column { width = "grow", height = 220,
    kit.list({ rows = sample.endpoints, title = "title", detail = "detail", badge = "method", tone = "tone", badge_width = 48,
      label = "Endpoints", selected = m.item, grow = true, on_select = { kind = "item" } }) }
end

return demo
