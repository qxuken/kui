-- Bars lying along rows, for long names or many bars: each name whole
-- beside its bar.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "Bars, lying",
  note = "for long names or many bars, each name whole beside its bar",
  call = 'kit.bar_chart({ data = by_status, label = "status", value = "n", horizontal = true, label_width = 60 })',
}

function demo.view(m)
  return kit.bar_chart({ data = sample.by_status(m.rows), label = "status", value = "n", horizontal = true,
    label_width = 60, values = true })
end

return demo
