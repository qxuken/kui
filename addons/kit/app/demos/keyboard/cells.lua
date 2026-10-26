-- A table of cells: the arrows walk cells, Up and Down keep the column,
-- and the header is a row of column headers.

local kit = require("kit.ui")
local sample = require("demos.sample")

local demo = {
  title = "A table of cells",
  note = "cells = true: the arrows walk cells, Up and Down keep the column, the header a row of column headers",
  call = 'kit.table({ rows = endpoints, cells = true, columns = { ... }, selected = model.item, on_select = { kind = "item" } })',
  state = { item = nil },
}

function demo.update(m, msg)
  if msg.kind == "item" then m.item = msg.select.row end
end

function demo.view(m)
  return column { width = "grow", height = 180,
    kit.table({ rows = sample.endpoints, cells = true, selected = m.item, grow = true, on_select = { kind = "item" },
      columns = { { field = "method", title = "Method", width = 80 }, { field = "title", title = "Path" },
        { field = "detail", title = "Traffic" } } }) }
end

return demo
