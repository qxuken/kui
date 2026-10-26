-- A histogram of request times across the worker's 100,000 rows
-- (`model.rows`, built by jobs/generate.lua).

-- Source also: jobs/generate.lua

local kit = require("kit.ui")

local demo = {
  title = "Histogram",
  note = "request times across 100,000 rows",
  call = 'kit.histogram({ data = model.rows, field = "ms", bins = 30 })',
}

function demo.view(m)
  return m.rows and kit.histogram({ data = m.rows, field = "ms", bins = 30, height = 120 }) or kit.progress({})
end

return demo
