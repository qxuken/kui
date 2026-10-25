-- The Kit app: every Kit component, live, on the kui runner — and the
-- runner's own kit beside it: a worker building 100,000 rows as a
-- channel, timers feeding a live chart and a log, and an HTTP server
-- serving the app's state.
--
--   kui run addons/kit/app            (from the repository root)
--   kui run --watch addons/kit/app    edit pages.lua and watch it reload

local kit = require("kit.ui")
local pages = require("pages")

-- The pages, in the sidebar's order.
PAGES = {
  { value = "overview", label = "Overview" },
  { value = "inputs", label = "Inputs" },
  { value = "data", label = "Data" },
  { value = "charts", label = "Charts" },
  { value = "trace", label = "Trace" },
  { value = "overlays", label = "Overlays" },
  { value = "layout", label = "Layout" },
  { value = "keyboard", label = "Keyboard" },
}

-- The app's state, made once; a --watch reload keeps it.
function init()
  return {
    page = "overview",
    skin = "ledger",
    density = "comfortable",
    shadows = true,
    side = 0.18,
    -- Inputs
    name = "kit",
    version = "1.4",
    notes = "",
    code = "{\n    \"name\": \"kit\"\n}",
    pinned = true,
    sort_by = "recent",
    query = "",
    mode = "auto",
    follow = true,
    live = true,
    level = "info",
    view = "table",
    zoom = 0.4,
    -- Data
    rows = nil, rows_ms = nil, filtered = false,
    selected = nil, sort = nil, descending = false, data_query = "", item = nil,
    log_lines = { time = {}, level = {}, message = {} },
    log = nil,
    -- Charts
    t = 0, series = { t = {}, cpu = {}, mem = {} }, chart = nil, bar = nil,
    -- Trace
    spans = nil, trace_start = nil, trace_length = nil, span = nil,
    -- Overlays
    dialog = false, menu = false, popover = false, toasts = {}, toast_n = 0, last_choice = nil,
    -- Layout
    split = 0.5, tab = "one", folded = false,
    -- Server
    serving = false, port = nil, hits = 0,
  }
end

-- A worker builds the big data; what it sends comes back to `update`.
worker = worker or kui.worker("jobs/generate.lua")

local LEVELS = { "debug", "info", "info", "info", "warn", "error" }
local WORDS = { "request handled", "cache miss", "retrying upstream", "token refreshed",
  "query planned", "batch flushed", "connection reused", "slow response" }

-- The live feeds: a sample every 100 ms, a log line every 400 ms.
local function tick()
  local m = model
  if not m.live then return end
  m.t = m.t + 0.1
  local s = m.series
  local n = #s.t + 1
  s.t[n] = m.t
  s.cpu[n] = 40 + 25 * math.sin(m.t * 0.9) + math.random() * 12
  s.mem[n] = 55 + 10 * math.sin(m.t * 0.21) + math.random() * 4
  if n > 600 then
    table.remove(s.t, 1); table.remove(s.cpu, 1); table.remove(s.mem, 1)
  end
  m.chart = m.chart and m.chart:publish(s) or channel(s)
end

local function log_line()
  local m = model
  if not m.live then return end
  local l = m.log_lines
  local n = #l.time + 1
  l.time[n] = os.date("%H:%M:%S")
  l.level[n] = LEVELS[math.random(#LEVELS)]
  l.message[n] = WORDS[math.random(#WORDS)] .. " #" .. math.random(1000, 9999)
  m.log = m.log and m.log:publish(l) or channel(l)
end

-- Timers survive a reload only once: they live in globals too.
if not feeds then
  feeds = {
    kui.timer.every(100, function() kui.dispatch({ kind = "tick" }) end),
    kui.timer.every(400, function() kui.dispatch({ kind = "log" }) end),
  }
  worker:dispatch({ kind = "rows", n = 100000 })
  worker:dispatch({ kind = "trace", n = 3000 })
end

-- A toast on the stack: `t` has its message and, as it likes, a tone,
-- an action and a duration.
local function toast(t)
  local m = model
  m.toast_n = m.toast_n + 1
  t.id = m.toast_n
  m.toasts[#m.toasts + 1] = t
end

local function toast_off(id)
  local m = model
  for i, t in ipairs(m.toasts) do
    if t.id == id then table.remove(m.toasts, i); return end
  end
end

local function refresh_rows()
  worker:dispatch({ kind = "view", query = model.data_query, sort = model.sort, descending = model.descending })
end

local function serve()
  local s = kui.http.serve({ port = 3050 })
  s:get("/api/state", function(req)
    kui.dispatch({ kind = "hit" })
    return { skin = model.skin, page = model.page, rows = model.rows and #model.rows or 0, t = model.t }
  end)
  s:get("/", function()
    return { html = "<h1>Kit app</h1><p>Try <a href='/api/state'>/api/state</a>.</p>" }
  end)
  server = s
  return s.port
end

function update(msg)
  local m = model
  local k = msg.kind
  if k == "tick" then tick()
  elseif k == "log" then log_line()
  elseif k == "page" then m.page = msg.change.value
  elseif k == "skin" then m.skin = msg.change.value
  elseif k == "density" then m.density = msg.toggle.on and "compact" or "comfortable"
  elseif k == "shadows" then m.shadows = msg.toggle.on
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "rows" then m.rows, m.rows_ms, m.filtered = msg.rows, msg.ms, msg.filtered; m.selected = nil
  elseif k == "trace" then m.spans = msg.spans
  elseif k == "worker_exit" then toast({ message = "The data worker stopped: " .. tostring(msg.error), tone = "danger", duration = 0 })
  -- Inputs
  elseif k == "name" then m.name = msg.change.text
  elseif k == "version" then m.version = msg.change.text
  elseif k == "notes" then m.notes = msg.change.text
  elseif k == "code" then m.code = msg.change.text
  elseif k == "pinned" then m.pinned = msg.toggle.on
  elseif k == "sort_by" then m.sort_by = msg.change.value
  elseif k == "query" then m.query = msg.change.text
  elseif k == "mode" then m.mode = msg.change.value
  elseif k == "follow" then m.follow = msg.toggle.checked
  elseif k == "live" then m.live = msg.toggle.on
  elseif k == "level" then m.level = msg.change.value
  elseif k == "view" then m.view = msg.change.value
  elseif k == "zoom" then m.zoom = msg.change.value
  elseif k == "press" then toast({ message = "Pressed " .. msg.what })
  -- Data
  elseif k == "select" then m.selected = msg.select.row
  elseif k == "item" then m.item = msg.select.row
  elseif k == "bar" then m.bar = msg.select.row
  elseif k == "sort" then
    if m.sort == msg.sort.field then m.descending = not m.descending else m.sort, m.descending = msg.sort.field, false end
    refresh_rows()
  elseif k == "data_query" then
    m.data_query = msg.change.text
    refresh_rows()
  -- Trace
  elseif k == "zoom_trace" then m.trace_start, m.trace_length = msg.zoom.start, msg.zoom.length
  elseif k == "span" then m.span = msg.select.row
  -- Overlays
  elseif k == "dialog" then m.dialog = msg.open
  elseif k == "menu" then m.menu = msg.open
  elseif k == "popover" then m.popover = msg.open
  elseif k == "choose" then m.menu = false; m.last_choice = msg.choose.value
  elseif k == "toast" then toast({ message = msg.message, tone = msg.tone, action = msg.action, duration = msg.duration })
  elseif k == "toast_all" then
    toast({ message = "A new version is ready." })
    toast({ message = "Saved to disk.", tone = "ok" })
    toast({ message = "The upstream is slow.", tone = "warn" })
    toast({ message = "Deleted 3 rows.", action = "Undo", duration = 8000 })
  elseif k == "toast_off" then toast_off(msg.id)
  elseif k == "undo" then
    toast_off(msg.id)
    toast({ message = "Restored the 3 rows.", tone = "ok" })
  -- Layout
  elseif k == "split" then m.split = msg.resize.fraction
  elseif k == "tab" then m.tab = msg.change.value
  elseif k == "fold" then m.folded = msg.toggle.collapsed
  -- Server
  elseif k == "serve" then
    if msg.toggle.on and not m.serving then
      local ok, port = pcall(serve)
      if ok then m.serving, m.port = true, port
      else toast({ message = tostring(port), tone = "danger", duration = 0 }) end
    elseif not msg.toggle.on and m.serving then
      server:stop(); m.serving = false
    end
  elseif k == "hit" then m.hits = m.hits + 1
  end
end

-- The toasts, stacked in the corner while there are any.
local function toasts(m)
  if #m.toasts == 0 then return nil end
  local t = {}
  for i, s in ipairs(m.toasts) do
    t[i] = kit.toast({ key = "toast" .. s.id, message = s.message, tone = s.tone, action = s.action, duration = s.duration,
      inline = true, on_dismiss = { kind = "toast_off", id = s.id }, on_action = { kind = "undo", id = s.id } })
  end
  return kit.toaster({}, t)
end

function view(env)
  local m = model
  local page = pages[m.page] or pages.overview
  return column { width = "grow", height = "grow", bg = "$kitBg",
    kit.settings({ skin = m.skin, density = m.density, shadows = m.shadows }),
    kit.toolbar({}, {
      row { gap = 8, cross_align = "center",
        text("Kit", { size = 15, bold = true, family = "$kitUi", color = "$kitFg" }),
        kit.badge({ label = "v" .. kui.version, tone = "neutral" }),
      },
      kit.segmented({
        options = { { value = "ledger", label = "Ledger" }, { value = "phosphor", label = "Phosphor" }, { value = "neutral", label = "Neutral" } },
        value = m.skin, on_change = { kind = "skin" },
      }),
      trailing = {
        kit.switch({ label = "Compact", on = m.density == "compact", on_toggle = { kind = "density" } }),
        kit.switch({ label = "Shadows", on = m.shadows, on_toggle = { kind = "shadows" } }),
        kit.switch({ label = "Live", on = m.live, on_toggle = { kind = "live" } }),
      },
    }),
    kit.split({ fraction = m.side, min = 150, on_resize = { kind = "side" } }, {
      first = {
        column { width = "grow", height = "grow", bg = "$kitSide", pad = 8, gap = 2,
          pages.nav(m),
        },
      },
      second = {
        column { key = "page." .. m.page, width = "grow", height = "grow", scroll_y = true, pad = 16, gap = 16,
          page(m),
        },
      },
    }),
    kit.statusbar({}, {
      text(m.rows and string.format("%d rows%s", #m.rows, m.filtered and " (filtered)" or "") or "building rows…",
        { size = 11, family = "$kitData", color = "$kitMuted" }),
      text(m.rows_ms and string.format("worker %.0f ms", m.rows_ms) or "", { size = 11, family = "$kitData", color = "$kitMuted" }),
      row { width = "grow" },
      kit.switch({ label = m.serving and ("Serving :" .. m.port .. " · " .. m.hits .. " hits") or "Serve on :3050",
        on = m.serving, on_toggle = { kind = "serve" } }),
    }),
    toasts(m),
  }
end
