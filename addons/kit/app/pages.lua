-- The Kit app's pages: one function a page, each showing a group of
-- components live, with the few lines that place them beside them.

local kit = require("kit.ui")

local M = {}

-- A demo: a heading, what it shows, the code, and the thing itself.
local function demo(title, note, code, ...)
  return kit.panel({ title = title, subtitle = note }, {
    column { width = "grow", gap = 12, ... },
    footer = { column { width = "grow", pad = 10, kit.code({ text = code }) } },
  })
end

local function grid(...)
  return row { width = "grow", gap = 16, wrap_children = true, cross_gap = 16, ... }
end

local function cell(w, node)
  return column { width = w, node }
end

function M.nav(m)
  local t = {}
  for _, p in ipairs(PAGES) do
    local here = p.value == m.page
    t[#t + 1] = row {
      key = "nav." .. p.value,
      role = "tab",
      label = p.label,
      selected = here,
      width = "grow",
      height = "$kitCtl",
      pad = { x = 10 },
      cross_align = "center",
      radius = "$kitRadius",
      bg = here and "$kitSel" or nil,
      hover_bg = here and "$kitSel" or "$kitHover",
      cursor = "pointer",
      on_click = { kind = "page", change = { value = p.value } },
      text(p.label, { size = "$kitSize", family = "$kitUi", bold = here, color = here and "$kitSelFg" or "$kitFg" }),
    }
  end
  return column { role = "tabList", width = "grow", gap = 2, table.unpack(t) }
end

function M.overview(m)
  local rows = m.rows and #m.rows or 0
  return column { width = "grow", gap = 16,
    text("Kit is kui's UI kit: components written once, placed from every binding.",
      { size = 18, bold = true, family = "$kitUi", color = "$kitFg", wrap = "word" }),
    text("This app is a Lua file on the kui runner. A worker built the 100,000 rows on the Data page and handed them over as a channel; timers feed the live chart and log; the switch in the status bar serves this app's state over HTTP.",
      { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" }),
    grid(
      kit.stat({ label = "Components", value = "36" }),
      kit.stat({ label = "Rows in the table", value = tostring(rows), delta = m.rows_ms and string.format("built in %.0f ms off the UI thread", m.rows_ms), tone = "ok" }),
      kit.stat({ label = "Skins", value = "3", delta = "Ledger · Phosphor · Neutral" }),
      kit.stat({ label = "Live samples", value = tostring(#m.series.t) })
    ),
    demo("Start an app", "kui new, then kui run --watch",
      'kui new hello\nkui run --watch hello\n\n-- main.lua\nlocal kit = require("kit.ui")\nfunction init() return { n = 0 } end\nfunction update(msg) if msg.kind == "add" then model.n = model.n + 1 end end\nfunction view(env)\n  return column { kit.button({ label = "Add", on_press = { kind = "add" } }),\n    text("n = " .. model.n) }\nend',
      row { gap = 8, kit.button({ label = "Primary", variant = "primary", icon = "play", on_press = { kind = "press", what = "Primary" } }),
        kit.button({ label = "Default", shortcut = "⌘D", on_press = { kind = "press", what = "Default" } }),
        kit.button({ label = "Ghost", variant = "ghost", on_press = { kind = "press", what = "Ghost" } }),
        kit.button({ label = "Delete", variant = "danger", icon = "trash", on_press = { kind = "press", what = "Delete" } }) }
    ),
  }
end

function M.inputs(m)
  return column { width = "grow", gap = 16,
    demo("Buttons", "four variants, an icon, a shortcut",
      'kit.button({ label = "Run", variant = "primary", icon = "play", on_press = { kind = "run" } })\nkit.icon_button({ icon = "refresh", label = "Reload", on_press = ... })',
      row { gap = 8, cross_align = "center",
        kit.button({ label = "Run", variant = "primary", icon = "play", on_press = { kind = "press", what = "Run" } }),
        kit.button({ label = "Save", shortcut = "⌘S", on_press = { kind = "press", what = "Save" } }),
        kit.button({ label = "Disabled", disabled = true }),
        kit.icon_button({ icon = "refresh", label = "Reload", on_press = { kind = "press", what = "Reload" } }),
        kit.icon_button({ icon = "sliders", label = "Settings" }),
        kit.icon_button({ icon = "trash", label = "Delete", variant = "danger", on_press = { kind = "press", what = "Delete" } }) }
    ),
    demo("Fields", "controlled: the value is yours, change proposes the next",
      'kit.text_field({ label = "Name", value = model.name, on_change = { kind = "name" } })\n-- update: model.name = msg.change.text',
      grid(
        cell(260, kit.text_field({ label = "Name", value = m.name, on_change = { kind = "name" } })),
        cell(300, kit.search({ value = m.query, placeholder = "Search requests", count = m.query ~= "" and "12 of 340" or nil, on_change = { kind = "query" } })),
        cell(200, kit.select({ label = "Mode", value = m.mode, on_change = { kind = "mode" },
          options = { { value = "auto", label = "Automatic" }, { value = "manual", label = "Manual" }, { value = "off", label = "Off", disabled = true } } }))
      ),
      text("name = " .. m.name .. "   query = " .. m.query .. "   mode = " .. m.mode, { size = 11, family = "$kitData", color = "$kitMuted" })
    ),
    demo("Choices", "boxes, switches, radios, segments, a slider",
      'kit.checkbox({ label = "Follow", checked = model.follow, on_toggle = { kind = "follow" } })\nkit.slider({ label = "Zoom", value = model.zoom, min = 0, max = 1, on_change = { kind = "zoom" } })',
      grid(
        column { gap = 4,
          kit.checkbox({ label = "Follow the log", checked = m.follow, on_toggle = { kind = "follow" } }),
          kit.checkbox({ label = "Some of them", checked = false, mixed = true }),
          kit.switch({ label = "Live feeds", on = m.live, on_toggle = { kind = "live" } }) },
        kit.radio_group({ label = "Level", value = m.level, on_change = { kind = "level" },
          options = { { value = "debug", label = "Debug" }, { value = "info", label = "Info" }, { value = "warn", label = "Warnings" } } }),
        column { gap = 10, width = 280,
          kit.segmented({ value = m.view, on_change = { kind = "view" },
            options = { { value = "table", label = "Table" }, { value = "list", label = "List" }, { value = "chart", label = "Chart" } } }),
          kit.slider({ label = "Zoom", value = m.zoom, min = 0, max = 1, step = 0.05, on_change = { kind = "zoom" } }) }
      )
    ),
  }
end

local COLUMNS = {
  { field = "id", title = "#", width = 70, align = "end", mono = true },
  { field = "method", title = "Method", width = 80 },
  { field = "path", title = "Path" },
  { field = "status", title = "Status", width = 70, align = "end", mono = true },
  { field = "ms", title = "Time", width = 80, align = "end", mono = true },
  { field = "size", title = "Size", width = 80, align = "end", mono = true },
}

function M.data(m)
  local entries = {}
  if m.rows and m.selected then
    for _, c in ipairs(COLUMNS) do
      entries[#entries + 1] = { key = c.title, value = tostring(m.rows:get(m.selected, c.field)), mono = c.mono }
    end
  end
  return column { width = "grow", gap = 16,
    row { width = "grow", gap = 12, cross_align = "center",
      column { width = 320, kit.search({ value = m.data_query, placeholder = "Filter by path or method", on_change = { kind = "data_query" } }) },
      text("Sorting and filtering run in the worker; the table reads the channel it sends back.", { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }),
    },
    row { width = "grow", height = 420, gap = 16,
      column { width = "grow", height = "grow",
        m.rows and kit.table({ rows = m.rows, columns = COLUMNS, selected = m.selected, sort = m.sort, descending = m.descending,
          striped = true, grow = true, on_select = { kind = "select" }, on_sort = { kind = "sort" } })
          or kit.progress({ label = "The worker is building 100,000 rows…" }) },
      column { width = 260, gap = 12,
        kit.panel({ title = "Selected" }, {
          #entries > 0 and kit.inspector({ entries = entries, key_width = 70 })
            or kit.empty({ title = "No row", detail = "Click a row, or use the arrows." }) }) },
    },
    demo("Log", "a channel published every 400 ms, following its end", 'kit.log({ rows = model.log, follow = model.follow, grow = true })',
      column { width = "grow", height = 180,
        m.log and kit.log({ rows = m.log, follow = m.follow, grow = true }) or kit.progress({}) }),
    demo("Badges and progress", "", 'kit.badge({ label = "200", tone = "ok" })',
      row { gap = 8, cross_align = "center",
        kit.badge({ label = "200", tone = "ok" }), kit.badge({ label = "304" }), kit.badge({ label = "slow", tone = "warn" }),
        kit.badge({ label = "500", tone = "danger" }), kit.badge({ label = "new", tone = "accent" }) },
      kit.progress({ label = "Uploading", value = (m.t % 10) / 10 }),
      kit.progress({ label = "Under way" })),
  }
end

function M.charts(m)
  local ch = m.chart
  local hist = m.rows
  return column { width = "grow", gap = 16,
    demo("Live line", "the last 30 s, a sample every 100 ms", 'kit.line_chart({ data = model.chart, x = "t", window = 30,\n  series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } })',
      ch and kit.line_chart({ data = ch, x = "t", window = 30, area = true, height = 180,
        series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } }) or kit.progress({})),
    grid(
      cell(360, demo("Sparkline", "inline", 'kit.sparkline({ data = model.chart, field = "cpu" })',
        row { gap = 10, cross_align = "center", text("CPU", { size = 11, family = "$kitUi", color = "$kitMuted" }),
          ch and kit.sparkline({ data = ch, field = "cpu", width = 160, height = 24 }) or text("") })),
      cell(420, demo("Histogram", "request times across 100,000 rows", 'kit.histogram({ data = model.rows, field = "ms", bins = 30 })',
        hist and kit.histogram({ data = hist, field = "ms", bins = 30, height = 120 }) or kit.progress({})))
    ),
    demo("Bars", "", 'kit.bar_chart({ data = by_status, label = "status", value = "n" })',
      kit.bar_chart({ data = M.by_status(m), label = "status", value = "n", height = 140 })),
  }
end

-- Requests by status, counted over the first 5,000 rows: a small channel.
local counted = nil
function M.by_status(m)
  if counted and counted.src == m.rows then return counted.ch end
  local counts, order = {}, {}
  if m.rows then
    for i = 0, math.min(#m.rows, 5000) - 1 do
      local s = tostring(m.rows:get(i, "status"))
      if not counts[s] then counts[s] = 0; order[#order + 1] = s end
      counts[s] = counts[s] + 1
    end
  end
  table.sort(order)
  local n = {}
  for i, s in ipairs(order) do n[i] = counts[s] end
  local ch = channel({ status = order, n = n })
  counted = { src = m.rows, ch = ch }
  return ch
end

function M.trace(m)
  local detail = nil
  if m.spans and m.span then
    detail = kit.inspector({ entries = {
      { key = "Span", value = tostring(m.spans:get(m.span, "name")) },
      { key = "Start", value = string.format("%.2f ms", m.spans:get(m.span, "start")), mono = true },
      { key = "Length", value = string.format("%.2f ms", m.spans:get(m.span, "len")), mono = true },
      { key = "Frame", value = tostring(m.spans:get(m.span, "target")) },
    } })
  end
  return column { width = "grow", gap = 16,
    demo("Waterfall", "18,000 spans: wheel to zoom about the pointer, drag to pan, click a span",
      'kit.waterfall({ bars = model.spans, start = model.start, length = model.length,\n  on_zoom = { kind = "zoom" }, on_select = { kind = "span" } })',
      column { width = "grow", height = 300,
        m.spans and kit.waterfall({ bars = m.spans, start = m.trace_start, length = m.trace_length or 2000, selected = m.span,
          grow = true, on_zoom = { kind = "zoom_trace" }, on_select = { kind = "span" } }) or kit.progress({ label = "The worker is building the trace…" }) },
      detail or text("Click a span to see it here.", { size = 11, family = "$kitUi", color = "$kitMuted" })),
  }
end

function M.overlays(m)
  return column { width = "grow", gap = 16,
    demo("Dialog", "modal: Tab stays inside; Escape or a press outside dismisses", 'if model.dialog then\n  kit.dialog({ title = "Discard?", on_dismiss = { kind = "close" } }, { ...body, actions = { ... } })\nend',
      row { gap = 8,
        kit.button({ label = "Open a dialog", on_press = { kind = "dialog", open = true } }) },
      m.dialog and kit.dialog({ title = "Discard the draft?", on_dismiss = { kind = "dialog", open = false } }, {
        text("Three edits since the last save will be lost.", { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" }),
        actions = {
          kit.button({ label = "Keep editing", on_press = { kind = "dialog", open = false } }),
          kit.button({ label = "Discard", variant = "danger", on_press = { kind = "dialog", open = false } }),
        },
      }) or nil),
    demo("Menu and popover", "floats beside what opened them", 'row { kit.button({ label = "Actions", on_press = ... }),\n  model.menu and kit.menu({ items = { ... }, on_choose = ..., on_dismiss = ... }) }',
      row { gap = 8,
        row { kit.button({ label = "Actions", icon = "chevron_down", on_press = { kind = "menu", open = true } }),
          m.menu and kit.menu({ on_choose = { kind = "choose" }, on_dismiss = { kind = "menu", open = false }, items = {
            { value = "copy", label = "Copy link", shortcut = "⌘C" },
            { value = "rename", label = "Rename…" },
            { value = "archive", label = "Archive", disabled = true },
            { value = "delete", label = "Delete", danger = true },
          } }) or nil },
        row { kit.button({ label = "Details", on_press = { kind = "popover", open = true } }),
          m.popover and kit.popover({ width = 260, on_dismiss = { kind = "popover", open = false } }, {
            text("A popover holds anything: here, a stat.", { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }),
            kit.stat({ label = "p95", value = "412 ms", delta = "-8%", tone = "ok" }),
          }) or nil },
        text(m.last_choice and ("chose " .. m.last_choice) or "", { size = 11, family = "$kitData", color = "$kitMuted" }) }),
    demo("Toast", "a message in the corner", 'kit.toast({ message = "Saved", tone = "ok", on_dismiss = { kind = "toast_off" } })',
      row { gap = 8,
        kit.button({ label = "Saved", on_press = { kind = "toast", message = "Saved to disk.", tone = "ok" } }),
        kit.button({ label = "Warning", on_press = { kind = "toast", message = "The upstream is slow.", tone = "warn" } }),
        kit.button({ label = "Error", on_press = { kind = "toast", message = "The request failed: 500.", tone = "danger" } }) }),
  }
end

function M.layout(m)
  return column { width = "grow", gap = 16,
    demo("Split", "drag the rule", 'kit.split({ fraction = model.split, on_resize = { kind = "split" } }, { first = {...}, second = {...} })',
      column { width = "grow", height = 160,
        kit.split({ fraction = m.split, on_resize = { kind = "split" } }, {
          first = { column { width = "grow", height = "grow", pad = 12, bg = "$kitSunken",
            text(string.format("first: %.0f%%", m.split * 100), { size = 12, family = "$kitData", color = "$kitFg" }) } },
          second = { column { width = "grow", height = "grow", pad = 12,
            text("second", { size = 12, family = "$kitData", color = "$kitFg" }) } },
        }) }),
    demo("Tabs and sections", "", 'kit.tabs({ tabs = { ... }, value = model.tab, on_change = { kind = "tab" } }, { ...body })',
      column { width = "grow", height = 140,
        kit.tabs({ value = m.tab, on_change = { kind = "tab" }, tabs = {
          { value = "one", label = "Requests", count = 340 }, { value = "two", label = "Errors", count = 3 }, { value = "three", label = "Settings" } } }, {
          column { pad = 12, text("The " .. m.tab .. " tab's body.", { size = "$kitSize", family = "$kitUi", color = "$kitFg" }) } }) },
      kit.section({ title = "Advanced", count = 2, collapsed = m.folded, on_toggle = { kind = "fold" } }, {
        kit.checkbox({ label = "Verbose logging", checked = false }),
        kit.checkbox({ label = "Keep history", checked = true }) })),
    demo("Empty state", "", 'kit.empty({ title = "No requests yet", detail = "...", { kit.button(...) } })',
      column { width = "grow", height = 200, kit.empty({ title = "No requests yet", detail = "Start the server, then send one." }, {
        kit.button({ label = "Start the server", variant = "primary", on_press = { kind = "press", what = "Start" } }) }) }),
  }
end

return M
