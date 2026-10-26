-- A table over 100,000 rows. The rows are a channel the app's worker
-- built (jobs/generate.lua; main.lua keeps it as `model.rows` and the
-- worker's own messages arrive here too): sorting and filtering ask the
-- worker for a new view, and the table reads the channel it sends back.

local kit = require("kit.ui")

local demo = {
  title = "Table",
  note = "100,000 rows from a worker, sorted and filtered there; a badge column, a frame, a selection",
  call = 'kit.table({ rows = model.rows, columns = COLUMNS, selected = model.selected, sort = model.sort,\n  descending = model.descending, striped = true, border = true, grow = true,\n  on_select = { kind = "select" }, on_sort = { kind = "sort" } })',
  state = { selected = nil, sort = nil, descending = false, data_query = "" },
}

local COLUMNS = {
  { field = "id", title = "#", width = 70, align = "end", mono = true },
  { field = "method", title = "Method", width = 80 },
  { field = "path", title = "Path" },
  { field = "status", title = "Status", width = 90, badge = true, tone = "tone" },
  { field = "ms", title = "Time", width = 80, align = "end", mono = true },
  { field = "size", title = "Size", width = 80, align = "end", mono = true },
}

-- The worker builds the view the table asks for.
local function refresh(m)
  worker:dispatch({ kind = "view", query = m.data_query, sort = m.sort, descending = m.descending })
end

function demo.update(m, msg)
  local k = msg.kind
  if k == "rows" then m.selected = nil
  elseif k == "select" then m.selected = msg.select.row
  elseif k == "sort" then
    if m.sort == msg.sort.field then m.descending = not m.descending else m.sort, m.descending = msg.sort.field, false end
    refresh(m)
  elseif k == "data_query" then
    m.data_query = msg.change.text
    refresh(m)
  end
end

function demo.view(m)
  local entries = {}
  if m.rows and m.selected then
    for _, c in ipairs(COLUMNS) do
      entries[#entries + 1] = { key = c.title, value = tostring(m.rows:get(m.selected, c.field)), mono = c.mono }
    end
  end
  return row { width = "grow", gap = 12, cross_align = "center",
      column { width = 320, kit.search({ value = m.data_query, placeholder = "Filter by path or method",
        on_change = { kind = "data_query" } }) },
      text("Sorting and filtering run in the worker; the table reads the channel it sends back.",
        { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }) },
    row { width = "grow", height = 420, gap = 16,
      column { width = "grow", height = "grow",
        m.rows and kit.table({ rows = m.rows, columns = COLUMNS, selected = m.selected, sort = m.sort, descending = m.descending,
          striped = true, border = true, grow = true, on_select = { kind = "select" }, on_sort = { kind = "sort" } })
          or kit.progress({ label = "The worker is building 100,000 rows…" }) },
      column { width = 260, gap = 12,
        kit.panel({ title = "Selected" }, {
          #entries > 0 and kit.inspector({ entries = entries, key_width = 70 })
            or kit.empty({ title = "No row", detail = "Click a row, or Tab to the table and use the arrows." }) }) } },
    text('The status column is `badge = true, tone = "tone"`: a pill with a dot in the tone a field of the row names; `border = true` frames the table.',
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
