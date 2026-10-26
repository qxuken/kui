-- A table over 100,000 rows. The rows are a channel the app's worker
-- built (jobs/generate.lua; main.lua keeps it as `model.rows` and the
-- worker's own messages arrive here too): sorting and filtering ask the
-- worker for a new view, and the table reads the channel it sends back.

-- Source also: jobs/generate.lua

local kit = require("kit.ui")

local demo = {
  title = "Table",
  note = "100,000 rows from a worker, sorted and filtered there; a badge column, a frame, a multiple selection the arrows do not change: Space or a click selects, ⌘ or Ctrl toggles, Shift extends, ⌘A takes them all, Enter or a double click opens",
  call = 'kit.table({ rows = model.rows, columns = COLUMNS, selection = "multiple", select_on_focus = false,\n  selected_ranges = model.ranges, sort = model.sort, descending = model.descending,\n  striped = true, border = true, grow = true, on_select = { kind = "select" },\n  on_cursor = { kind = "cursor" }, on_activate = { kind = "open" }, on_sort = { kind = "sort" } })',
  state = { ranges = {}, count = 0, focus_row = nil, cursor = nil, opened = nil, sort = nil, descending = false, data_query = "" },
}

-- Narrow, Size hides first, then the id, then Time; what still does not
-- fit scrolls sideways.
local COLUMNS = {
  { field = "id", title = "#", width = 70, align = "end", mono = true, priority = 2 },
  { field = "method", title = "Method", width = 80 },
  { field = "path", title = "Path", min_width = 160 },
  { field = "status", title = "Status", width = 90, badge = true, tone = "tone" },
  { field = "ms", title = "Time", width = 80, align = "end", mono = true, priority = 3 },
  { field = "size", title = "Size", width = 80, align = "end", mono = true, priority = 1 },
}

-- The worker builds the view the table asks for.
local function refresh(m)
  worker:dispatch({ kind = "view", query = m.data_query, sort = m.sort, descending = m.descending })
end

function demo.update(m, msg)
  local k = msg.kind
  if k == "rows" then m.ranges, m.count, m.focus_row, m.opened = {}, 0, nil, nil
  -- The detail loads here, on a selection, and on `open` — never on a
  -- cursor move, which only says where the keyboard is.
  elseif k == "select" then
    m.ranges, m.count = msg.select.ranges or {}, msg.select.count or 0
    m.focus_row = msg.select.row
  elseif k == "cursor" then m.cursor = msg.cursor.row
  elseif k == "open" then m.opened = msg.activate.row
  elseif k == "sort" then
    if m.sort == msg.sort.field then m.descending = not m.descending else m.sort, m.descending = msg.sort.field, false end
    refresh(m)
  elseif k == "data_query" then
    m.data_query = msg.change.text
    refresh(m)
  end
end

local function selected(m, entries)
  local title = m.count > 1 and (m.count .. " rows selected") or "Selected"
  return kit.panel({ title = title }, {
    #entries > 0 and kit.inspector({ entries = entries, key_width = 70 })
      or kit.empty({ title = "No row", detail = "Click a row, or Tab to the table, move with the arrows and press Space." }) })
end

-- Where the keyboard is, what is selected, what was opened.
local function said(m)
  local parts = { "Cursor: " .. (m.cursor and ("row " .. m.cursor) or "none") }
  if m.count == 0 then
    parts[#parts + 1] = "nothing selected"
  else
    local rs = {}
    for i, r in ipairs(m.ranges) do
      if i > 3 then rs[#rs + 1] = "…"; break end
      rs[#rs + 1] = r.first == r.last and tostring(r.first) or (r.first .. "–" .. r.last)
    end
    parts[#parts + 1] = m.count .. " selected (" .. table.concat(rs, ", ") .. ")"
  end
  if m.opened then parts[#parts + 1] = "opened row " .. m.opened end
  return table.concat(parts, " · ")
end

function demo.view(m)
  local entries = {}
  local shown = m.opened or m.focus_row
  if m.rows and shown and m.count > 0 then
    for _, c in ipairs(COLUMNS) do
      entries[#entries + 1] = { key = c.title, value = tostring(m.rows:get(shown, c.field)), mono = c.mono }
    end
  end
  return row { width = "grow", gap = 12, cross_align = "center",
      column { width = 320, kit.search({ value = m.data_query, placeholder = "Filter by path or method",
        on_change = { kind = "data_query" } }) },
      text("Sorting and filtering run in the worker; the table reads the channel it sends back.",
        { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }) },
    -- Beside the table while there is room, under it when the card is
    -- narrow: the row's own width (ADR 0069) turns it to a column and
    -- shows the panel that spans it instead of the one beside.
    row { width = "grow", gap = 16,
      when = {
        { width = { above = 720, band = 16 }, hidden = { "under" } },
        { width = { below = 720, band = 16 }, dir = "column", hidden = { "beside" } },
      },
      column { width = "grow", height = 420,
        m.rows and kit.table({ rows = m.rows, columns = COLUMNS, selection = "multiple", select_on_focus = false,
          selected_ranges = m.ranges, sort = m.sort, descending = m.descending, striped = true, border = true,
          grow = true, on_select = { kind = "select" }, on_cursor = { kind = "cursor" }, on_activate = { kind = "open" },
          on_sort = { kind = "sort" } })
          or kit.progress({ label = "The worker is building 100,000 rows…" }) },
      column { key = "beside", width = 260, selected(m, entries) },
      column { key = "under", width = "grow", selected(m, entries) } },
    text(said(m), { size = 11, family = "$kitData", color = "$kitMuted", max_lines = 1, ellipsis = true }),
    text('The status column is `badge = true, tone = "tone"`: a pill with a dot in the tone a field of the row names; `border = true` frames the table.',
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
