-- A live line chart: the last 30 s of a sample every 100 ms, its area
-- filled. main.lua publishes the samples as `model.chart` (a channel
-- with `t`, `cpu` and `mem` columns) while the Live switch is on.

local kit = require("kit.ui")

local demo = {
  title = "Live line",
  note = "the last 30 s, a sample every 100 ms, its area filled; the axes measure their labels and glide to a new range",
  call = 'kit.line_chart({ data = model.chart, x = "t", window = 30, area = true, x_unit = "s", y_unit = "%",\n  series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } })',
}

function demo.view(m)
  if not m.chart then return kit.progress({}) end
  return kit.line_chart({ data = m.chart, x = "t", window = 30, area = true, height = 180, x_unit = "s", y_unit = "%",
    series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } })
end

return demo
