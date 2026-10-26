-- A sparkline inline, over the live feed's `cpu` column (`model.chart`,
-- which demos/feeds.lua publishes every 100 ms).

-- Source also: demos/feeds.lua

local kit = require("kit.ui")

local demo = {
  title = "Sparkline",
  note = "inline",
  call = 'kit.sparkline({ data = model.chart, field = "cpu" })',
}

function demo.view(m)
  return row { gap = 10, cross_align = "center",
    text("CPU", { size = 11, family = "$kitUi", color = "$kitMuted" }),
    m.chart and kit.sparkline({ data = m.chart, field = "cpu", width = 160, height = 24 }) or text("") }
end

return demo
