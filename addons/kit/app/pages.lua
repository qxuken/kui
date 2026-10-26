-- The Kit app's pages: one function a page, each showing a group of
-- components live, with the few lines that place them beside them. Every
-- component in addons/kit/schema.lua is placed on one of them (the CLI's
-- `the_kit_app_places_every_component` checks).

local kit = require("kit.ui")

local M = {}

-- A demo: a heading, what it shows, the code, and the thing itself.
local function demo(title, note, code, ...)
  return kit.panel({ title = title, subtitle = note }, {
    column { width = "grow", gap = 12, ... },
    footer = { column { width = "grow", pad = 10, kit.code({ text = code, wrap = true }) } },
  })
end

-- As many equal columns a line as fit at 320 px (ADR 0069 `min_column`).
local function grid(...)
  return row { width = "grow", gap = 16, cross_gap = 16, min_column = 320, ... }
end

-- A grid cell: as wide as a column, whatever `w` was (a width a cell once
-- held, kept as a note of what the demo needs).
local function cell(w, node)
  return column { width = "grow", node }
end

local function note(s)
  return text(s, { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

local function said(s)
  return text(s, { size = 11, family = "$kitData", color = "$kitMuted" })
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
    text("This app is a Lua file on the kui runner. A worker built the 100,000 rows on the Data page and handed them over as a channel; timers feed the live chart and log; the switch in the status bar serves this app's state over HTTP. The switches in the toolbar change the skin's density and its shadows for every page.",
      { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" }),
    grid(
      kit.stat({ label = "Components", value = "39" }),
      kit.stat({ label = "Rows in the table", value = tostring(rows), delta = m.rows_ms and string.format("built in %.0f ms off the UI thread", m.rows_ms), tone = "ok" }),
      kit.stat({ label = "Skins", value = "3", delta = "Ledger · Phosphor · Neutral" }),
      kit.stat({ label = "Live samples", value = tostring(#m.series.t) })
    ),
    demo("Start an app", "kui new, then kui run --watch",
      'kui new hello\nkui run --watch hello\n\n-- main.lua\nlocal kit = require("kit.ui")\nfunction init() return { n = 0 } end\nfunction update(msg) if msg.kind == "add" then model.n = model.n + 1 end end\nfunction view(env)\n  return column { kit.button({ label = "Add", on_press = { kind = "add" } }),\n    text("n = " .. model.n) }\nend',
      row { gap = 8, kit.button({ label = "Primary", variant = "primary", icon = "play", on_press = { kind = "press", what = "Primary" } }),
        kit.button({ label = "Default", shortcut = "⌘D", on_press = { kind = "press", what = "Default" } }),
        kit.button({ label = "Ghost", variant = "ghost", on_press = { kind = "press", what = "Ghost" } }),
        kit.button({ label = "Delete", variant = "danger", icon = "trash", on_press = { kind = "press", what = "Delete" } }) },
      note("On Ledger the primary button's face lifts under the pointer and sinks onto its shadow while it is held.")
    ),
  }
end

-- What is wrong with a version, or nil.
local function version_error(v)
  if v:match("^%d+%.%d+%.%d+$") then return nil end
  return "Three numbers: 1.4.0"
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
    demo("Fields", "controlled: the value is yours, change proposes the next; an error sits at the field's end and takes no room",
      'kit.text_field({ label = "Name", value = model.name, on_change = { kind = "name" } })\nkit.text_field({ label = "Version", value = model.version, mono = true,\n  error = version_error(model.version), on_change = { kind = "version" } })',
      grid(
        cell(260, kit.text_field({ label = "Name", value = m.name, on_change = { kind = "name" } })),
        cell(260, kit.text_field({ label = "Version", value = m.version, mono = true, error = version_error(m.version),
          on_change = { kind = "version" } })),
        cell(300, kit.search({ value = m.query, placeholder = "Search requests", count = m.query ~= "" and "12 of 340" or nil, on_change = { kind = "query" } })),
        cell(200, kit.select({ label = "Mode", value = m.mode, on_change = { kind = "mode" },
          options = { { value = "auto", label = "Automatic" }, { value = "manual", label = "Manual" }, { value = "off", label = "Off", disabled = true } } }))
      ),
      said("name = " .. m.name .. "   version = " .. m.version .. "   query = " .. m.query .. "   mode = " .. m.mode)
    ),
    demo("Text areas", "a well in every skin, growing with its text; Tab types, unless `tab_moves_focus`",
      'kit.text_area({ label = "Notes", value = model.notes, rows = 3, max_rows = 6, tab_moves_focus = true, on_change = { kind = "notes" } })\nkit.text_area({ label = "Body", value = model.code, mono = true, on_change = { kind = "code" } })',
      grid(
        cell(320, kit.text_area({ label = "Notes", value = m.notes, rows = 3, max_rows = 6, placeholder = "Write a note…", tab_moves_focus = true,
          on_change = { kind = "notes" } })),
        cell(320, kit.text_area({ label = "Body", value = m.code, rows = 3, mono = true,
          on_change = { kind = "code" } }))
      ),
      note("In Body, Tab types four spaces; Escape then Tab, or Ctrl+M (Ctrl+Shift+M on macOS), moves on. In Notes, Tab moves on.")
    ),
    demo("Choices", "boxes, switches, radios, segments, a slider",
      'kit.checkbox({ label = "Follow", checked = model.follow, on_toggle = { kind = "follow" } })\nkit.slider({ label = "Zoom", value = model.zoom, value_label = "40%", on_change = { kind = "zoom" } })',
      grid(
        column { gap = 4,
          kit.checkbox({ label = "Follow the log", checked = m.follow, on_toggle = { kind = "follow" } }),
          kit.checkbox({ label = "Some of them", checked = false, mixed = true }),
          kit.switch({ label = "Live feeds", on = m.live, on_toggle = { kind = "live" } }),
          kit.switch({ label = "Pinned", on = m.pinned, on_toggle = { kind = "pinned" } }) },
        kit.radio_group({ label = "Level", value = m.level, on_change = { kind = "level" },
          options = { { value = "debug", label = "Debug" }, { value = "info", label = "Info" }, { value = "warn", label = "Warnings" } } }),
        column { gap = 4,
          note("hide_label: named, not drawn"),
          kit.radio_group({ label = "Sort", hide_label = true, value = m.sort_by, on_change = { kind = "sort_by" },
            options = { { value = "recent", label = "Most recent" }, { value = "name", label = "By name" } } }) },
        column { gap = 10, width = 280,
          kit.segmented({ value = m.view, on_change = { kind = "view" },
            options = { { value = "table", label = "Table" }, { value = "list", label = "List" }, { value = "chart", label = "Chart" } } }),
          kit.slider({ label = "Zoom", value = m.zoom, min = 0, max = 1, step = 0.05,
            value_label = string.format("%d%%", math.floor(m.zoom * 100 + 0.5)), on_change = { kind = "zoom" } }) }
      )
    ),
  }
end

local COLUMNS = {
  { field = "id", title = "#", width = 70, align = "end", mono = true },
  { field = "method", title = "Method", width = 80 },
  { field = "path", title = "Path" },
  { field = "status", title = "Status", width = 90, badge = true, tone = "tone" },
  { field = "ms", title = "Time", width = 80, align = "end", mono = true },
  { field = "size", title = "Size", width = 80, align = "end", mono = true },
}

-- A few named things with a method and a tone, for the list.
local ENDPOINTS = channel({
  title = { "/api/items", "/api/users", "/api/search", "/api/orders", "/health", "/static/app.js" },
  detail = { "12 ms · 340 today", "40 ms · 88 today", "410 ms · 1,204 today", "1.2 s · 9 today", "2 ms · 8,640 today", "6 ms · 2,410 today" },
  method = { "GET", "POST", "GET", "PUT", "GET", "GET" },
  tone = { "ok", "accent", "warn", "danger", "ok", "neutral" },
})

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
      note("Sorting and filtering run in the worker; the table reads the channel it sends back."),
    },
    row { width = "grow", height = 420, gap = 16,
      column { width = "grow", height = "grow",
        m.rows and kit.table({ rows = m.rows, columns = COLUMNS, selected = m.selected, sort = m.sort, descending = m.descending,
          striped = true, border = true, grow = true, on_select = { kind = "select" }, on_sort = { kind = "sort" } })
          or kit.progress({ label = "The worker is building 100,000 rows…" }) },
      column { width = 260, gap = 12,
        kit.panel({ title = "Selected" }, {
          #entries > 0 and kit.inspector({ entries = entries, key_width = 70 })
            or kit.empty({ title = "No row", detail = "Click a row, or Tab to the table and use the arrows." }) }) },
    },
    note('The status column is `badge = true, tone = "tone"`: a pill with a dot in the tone a field of the row names; `border = true` frames the table.'),
    grid(
      cell(380, demo("List", "a title and a detail a row, a badge in its tone", 'kit.list({ rows = endpoints, title = "title", detail = "detail",\n  badge = "method", tone = "tone", badge_width = 48, selected = model.item, on_select = { kind = "item" } })',
        column { width = "grow", height = 220,
          kit.list({ rows = ENDPOINTS, title = "title", detail = "detail", badge = "method", tone = "tone", badge_width = 48,
            label = "Endpoints", selected = m.item, grow = true, on_select = { kind = "item" } }) })),
      cell(420, demo("Notices", "in the flow of the page, in a tone, with actions", 'kit.notice({ tone = "danger", title = "Could not save", message = "..." }, { kit.button(...) })',
        kit.notice({ message = "Saved drafts are kept for 30 days.", tone = "info" }),
        kit.notice({ tone = "warn", message = "The upstream answers slowly." }),
        kit.notice({ tone = "danger", title = "Could not save", message = "The disk is full." },
          { kit.button({ label = "Retry", icon = "refresh", on_press = { kind = "press", what = "Retry" } }) })))
    ),
    demo("Log", "a channel published every 400 ms, following its end", 'kit.log({ rows = model.log, follow = model.follow, grow = true })',
      column { width = "grow", height = 180,
        m.log and kit.log({ rows = m.log, follow = m.follow, grow = true }) or kit.progress({}) }),
    demo("Badges and progress", "badges carry a tooltip; `fade` brings one in and out; a progress bar springs to its value",
      'kit.badge({ label = "200", tone = "ok", dot = true, tooltip = "OK" })\nkit.badge({ label = "changed", tone = "accent", fade = true })\nkit.progress({ label = "Uploading", value = model.upload })',
      row { gap = 8, cross_align = "center",
        kit.badge({ label = "200", tone = "ok", dot = true, tooltip = "OK: the request succeeded" }), kit.badge({ label = "304", tooltip = "Not modified" }),
        kit.badge({ label = "slow", tone = "warn" }), kit.badge({ label = "500", tone = "danger", dot = true }),
        kit.badge({ label = "new", tone = "accent" }), kit.badge({ label = "info", tone = "info" }),
        m.name ~= "kit" and kit.badge({ label = "changed", tone = "accent", fade = true }) or nil },
      note("Change the name on the Inputs page and a `changed` badge fades in here."),
      kit.progress({ label = "Uploading", value = (math.floor(m.t) % 5) / 4 }),
      kit.progress({ label = "Under way" })),
  }
end

function M.charts(m)
  local ch = m.chart
  local hist = m.rows
  local statuses = M.by_status(m)
  local picked = m.bar and statuses and #statuses > m.bar and
    (tostring(statuses:get(m.bar, "status")) .. ": " .. tostring(statuses:get(m.bar, "n")) .. " requests") or "Pick a bar: click it, or Tab to the bars and use the arrows."
  return column { width = "grow", gap = 16,
    demo("Live line", "the last 30 s, a sample every 100 ms, its area filled",
      'kit.line_chart({ data = model.chart, x = "t", window = 30, area = true, x_unit = "s", y_unit = "%",\n  series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } })',
      ch and kit.line_chart({ data = ch, x = "t", window = 30, area = true, height = 180, x_unit = "s", y_unit = "%",
        series = { { field = "cpu", name = "CPU" }, { field = "mem", name = "Memory" } } }) or kit.progress({})),
    grid(
      cell(360, demo("Sparkline", "inline", 'kit.sparkline({ data = model.chart, field = "cpu" })',
        row { gap = 10, cross_align = "center", text("CPU", { size = 11, family = "$kitUi", color = "$kitMuted" }),
          ch and kit.sparkline({ data = ch, field = "cpu", width = 160, height = 24 }) or text("") })),
      cell(420, demo("Histogram", "request times across 100,000 rows", 'kit.histogram({ data = model.rows, field = "ms", bins = 30 })',
        hist and kit.histogram({ data = hist, field = "ms", bins = 30, height = 120 }) or kit.progress({})))
    ),
    demo("Bars", "values over the bars, a unit; one Tab stop when the host picks",
      'kit.bar_chart({ data = by_status, label = "status", value = "n", unit = "req",\n  on_select = { kind = "bar" } })',
      kit.bar_chart({ data = statuses, label = "status", value = "n", unit = "req", height = 160, on_select = { kind = "bar" } }),
      said(picked)),
    demo("Bars, lying", "for long names or many bars, each name whole beside its bar",
      'kit.bar_chart({ data = by_status, label = "status", value = "n", horizontal = true, label_width = 60 })',
      kit.bar_chart({ data = statuses, label = "status", value = "n", horizontal = true, label_width = 60, values = true })),
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
    demo("Waterfall", "18,000 spans: wheel to zoom about the pointer, drag to pan, click a span or Tab to the spans and use the arrows",
      'kit.waterfall({ bars = model.spans, start = model.start, length = model.length,\n  on_zoom = { kind = "zoom" }, on_select = { kind = "span" } })',
      column { width = "grow", height = 300,
        m.spans and kit.waterfall({ bars = m.spans, start = m.trace_start, length = m.trace_length or 2000, selected = m.span,
          grow = true, on_zoom = { kind = "zoom_trace" }, on_select = { kind = "span" } }) or kit.progress({ label = "The worker is building the trace…" }) },
      detail or note("Click a span to see it here.")),
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
          m.popover and kit.popover({ label = "Details", width = 260, on_dismiss = { kind = "popover", open = false } }, {
            note("A popover holds anything: here, a stat."),
            kit.stat({ label = "p95", value = "412 ms", delta = "-8%", tone = "ok" }),
          }) or nil },
        said(m.last_choice and ("chose " .. m.last_choice) or "") }),
    demo("Toasts", "stacked in a toaster in the corner, each in its tone; they go by themselves, an error stays, Undo answers",
      'kit.toaster({}, {\n  kit.toast({ message = "Saved.", tone = "ok", inline = true, on_dismiss = { kind = "toast_off", id = 1 } }),\n  kit.toast({ message = "Deleted 3 rows.", action = "Undo", inline = true, on_action = { kind = "undo" }, ... }),\n})',
      row { gap = 8, wrap_children = true, cross_gap = 8,
        kit.button({ label = "Info", on_press = { kind = "toast", message = "A new version is ready." } }),
        kit.button({ label = "Saved", on_press = { kind = "toast", message = "Saved to disk.", tone = "ok" } }),
        kit.button({ label = "Warning", on_press = { kind = "toast", message = "The upstream is slow.", tone = "warn" } }),
        kit.button({ label = "Error", on_press = { kind = "toast", message = "The request failed: 500.", tone = "danger", duration = 0 } }),
        kit.button({ label = "Delete, with Undo", icon = "trash", on_press = { kind = "toast", message = "Deleted 3 rows.", action = "Undo", duration = 8000 } }),
        kit.button({ label = "All of them", variant = "ghost", on_press = { kind = "toast_all" } }) },
      note("A toast with no tone is info. An error is kept (duration = 0) until it is dismissed; the others go after five seconds, counted again from when the pointer leaves them.")),
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
    demo("Divider", "a rule between groups, as the skin draws regions", 'kit.divider({})',
      note("Above the rule: one group."),
      kit.divider({}),
      note("Below it: the next.")),
    demo("Empty state", "", 'kit.empty({ title = "No requests yet", detail = "...", { kit.button(...) } })',
      column { width = "grow", height = 200, kit.empty({ title = "No requests yet", detail = "Start the server, then send one." }, {
        kit.button({ label = "Start the server", variant = "primary", on_press = { kind = "press", what = "Start" } }) }) }),
  }
end

-- What the arrows do, by component: each of these is one Tab stop.
function M.keyboard(m)
  local statuses = M.by_status(m)
  return column { width = "grow", gap = 16,
    text("Each of these is one Tab stop. The arrows move inside it; Enter or Space picks; Tab moves on.",
      { size = 15, bold = true, family = "$kitUi", color = "$kitFg", wrap = "word" }),
    note("Tab lands on the chosen item, not the first. A click focuses without the ring; the keyboard shows it. Components nobody picks in (no on_select) take no keyboard at all."),
    grid(
      cell(320, demo("Segments and radios", "arrows choose as they move", 'kit.segmented({ value = model.view, options = {...}, on_change = { kind = "view" } })',
        kit.segmented({ value = m.view, on_change = { kind = "view" },
          options = { { value = "table", label = "Table" }, { value = "list", label = "List" }, { value = "chart", label = "Chart" } } }),
        kit.radio_group({ label = "Level", value = m.level, on_change = { kind = "level" },
          options = { { value = "debug", label = "Debug" }, { value = "info", label = "Info" }, { value = "warn", label = "Warnings" } } }),
        said("view = " .. m.view .. "   level = " .. m.level))),
      cell(380, demo("A list", "Up and Down select, Home and End the ends", 'kit.list({ rows = endpoints, title = "title", selected = model.item, on_select = { kind = "item" } })',
        column { width = "grow", height = 200,
          kit.list({ rows = ENDPOINTS, title = "title", detail = "detail", label = "Endpoints", selected = m.item, grow = true,
            on_select = { kind = "item" } }) },
        said(m.item and ("item = " .. tostring(ENDPOINTS:get(m.item, "title"))) or "item = none")))
    ),
    demo("Bars", "Left and Right walk the bars, the one the keyboard is on looks hovered and shows its tooltip, Enter picks it",
      'kit.bar_chart({ data = by_status, label = "status", value = "n", on_select = { kind = "bar" } })',
      kit.bar_chart({ data = statuses, label = "status", value = "n", height = 140, on_select = { kind = "bar" } }),
      said(m.bar and statuses and #statuses > m.bar and ("bar = " .. tostring(statuses:get(m.bar, "status"))) or "bar = none")),
    demo("Tabs, a table, a trace, a text area", "", 'kit.tabs(...)  -- Left and Right\nkit.table({ ..., on_select = ... })  -- Up, Down, Home, End, Page Up and Down\nkit.waterfall({ ..., on_select = ... })  -- the arrows walk the spans\nkit.text_area({ ... })  -- Tab types; Escape then Tab, or Ctrl+M, leaves it (tab_moves_focus: Tab leaves)',
      note("The Layout page's tabs, the Data page's table, the Trace page's spans and the Inputs page's text areas answer the keyboard the same way.")),
  }
end

return M
