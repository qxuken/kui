-- A list from the keyboard: Up and Down select, Home and End the ends,
-- and typing finds a row by its title.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "A list",
  note = "Up and Down select, Home and End the ends",
  call = 'kit.list({ rows = endpoints, title = "title", selected = model.item, on_select = { kind = "item" } })',
  state = { item = nil },
}

function demo.update(m, msg)
  if msg.kind == "item" then m.item = msg.select.row end
end

function demo.view(m)
  return column { width = "grow", height = 200,
      kit.list({ rows = sample.endpoints, title = "title", detail = "detail", label = "Endpoints", selected = m.item, grow = true,
        on_select = { kind = "item" } }) },
    text(m.item and ("item = " .. tostring(sample.endpoints:get(m.item, "title"))) or "item = none",
      { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
