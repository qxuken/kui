-- The Kit app's pages. Every card is a file of its own under demos/ — its
-- data, its helpers, the state it keeps, the messages it answers and what
-- it draws — and its Source button shows that file, read from the app's
-- folder, so what a card says it does is what it does. Every component in
-- addons/kit/schema.lua is placed by one of them (the runner's
-- `the_kit_app_places_every_component` checks).

local kit = require("kit.ui")

local M = {}

-- The pages in the sidebar's order: each a list of cards, a card being a
-- demo's name under demos/ — or a list of names, side by side as room
-- allows.
M.PAGES = {
  { value = "overview", label = "Overview", cards = { "overview/start" } },
  { value = "inputs", label = "Inputs", cards = { "inputs/buttons", "inputs/fields", "inputs/text_areas", "inputs/choices", "inputs/checklist" } },
  { value = "data", label = "Data", cards = { "data/table", { "data/list", "data/notices" }, "data/log", "data/badges" } },
  { value = "charts", label = "Charts", cards = { "charts/live_line", { "charts/sparkline", "charts/histogram" }, "charts/bars", "charts/bars_lying" } },
  { value = "trace", label = "Trace", cards = { "trace/waterfall" } },
  { value = "overlays", label = "Overlays", cards = { "overlays/dialog", "overlays/menu", "overlays/toasts" } },
  { value = "layout", label = "Layout", cards = { "layout/split", "layout/tabs", "layout/conditions", "layout/divider", "layout/empty" } },
  { value = "loading", label = "Loading", cards = { { "loading/profile", "loading/skeleton" }, "loading/shader" } },
  { value = "keyboard", label = "Keyboard", cards = { { "keyboard/segments", "keyboard/list" }, "keyboard/bars", "keyboard/cells", "keyboard/elsewhere" } },
}

-- Every card's module, by its name.
M.DEMOS = {}
for _, p in ipairs(M.PAGES) do
  for _, c in ipairs(p.cards) do
    for _, name in ipairs(type(c) == "table" and c or { c }) do
      M.DEMOS[name] = require("demos." .. name:gsub("/", "."))
    end
  end
end

-- What every card keeps in the app's model, made once.
function M.state(m)
  for _, d in pairs(M.DEMOS) do
    for k, v in pairs(d.state or {}) do
      if m[k] == nil then m[k] = v end
    end
  end
end

-- Every card hears every message, as an app's parts would.
function M.update(m, msg)
  for _, d in pairs(M.DEMOS) do
    if d.update then d.update(m, msg) end
  end
end

-- The app's own files, read once each (a --watch reload reads them again).
local files = {}

-- A file's text by its path in the app's folder (`demos/inputs/buttons.lua`),
-- nil when it cannot be read. A packed app (`kui pack`) runs from its files
-- extracted, so this reads there too.
local function source(path)
  local text = files[path]
  if text == nil then
    local ok, t = pcall(kui.fs.read, path)
    if not ok then
      text = false
    elseif t:sub(1, 4) == "\27Lua" then
      text = "-- This app was packed with --bytecode, so its sources are compiled.\n-- Pack it without --bytecode to read them here."
    else
      text = t
    end
    files[path] = text
  end
  return text or nil
end

-- A card's full source: its own file, then each of the app's modules it
-- requires (the sample data two cards share) and each file a line
-- `-- Source also: path` names (where its data comes from: the live feeds,
-- the worker's job), each read-only, monospace, scrolled past 24 lines and
-- with a Copy button — so no card shows data the reader cannot see made.
function M.files_of(name)
  local paths, seen = {}, {}
  local function add(path)
    if seen[path] then return end
    seen[path] = true
    local text = source(path)
    if not text then return end
    paths[#paths + 1] = path
    for dep in text:gmatch('require%(%s*"(demos%.[%w_.]+)"%s*%)') do add(dep:gsub("%.", "/") .. ".lua") end
    for also in text:gmatch("%-%- Source also: ([%w_./]+)") do add(also) end
  end
  add("demos/" .. name .. ".lua")
  return paths
end

local function sources(name)
  local out = {}
  for _, path in ipairs(M.files_of(name)) do
    out[#out + 1] = kit.code({ label = path, text = source(path) or "", copy = true, line_numbers = true, max_lines = 24 })
  end
  if #out == 0 then
    out[1] = text("The source could not be read from the app's folder.", { size = 11, family = "$kitUi", color = "$kitMuted" })
  end
  return out
end

-- A card: its title and what it shows, the thing itself, and under it the
-- call that places it — or, with Source on, the card's whole file.
local function card(m, name)
  local d = M.DEMOS[name]
  local open = m.source[name] == true
  local foot = open and sources(name) or { kit.code({ text = d.call, wrap = true }) }
  return kit.panel({ title = d.title, subtitle = d.note }, {
    column { width = "grow", gap = 12, d.view(m) },
    actions = {
      kit.button({ label = "Source", icon = "code", variant = "ghost", pressed = open, name = "Source of " .. d.title,
        tooltip = open and "Show the call" or "Show the card's whole file", on_press = { kind = "source", name = name } }),
    },
    footer = { column { width = "grow", pad = 10, gap = 8, table.unpack(foot) } },
  })
end

-- A line of cards sharing its width, each 320 px at least: they wrap
-- onto lines of their own when the page is narrower.
local function grid(m, names)
  local t = { width = "grow", gap = 16, cross_gap = 16, wrap_children = true }
  for _, name in ipairs(names) do t[#t + 1] = column { width = "grow", min_width = 320, card(m, name) } end
  return row(t)
end

-- What the Overview page says before its card.
local function overview(m)
  local rows = m.rows and #m.rows or 0
  return text("Kit is kui's UI kit: components written once, placed from every binding.",
      { size = 18, bold = true, family = "$kitUi", color = "$kitFg", wrap = "word" }),
    text("This app is Lua on the kui runner. A worker built the 100,000 rows on the Data page and handed them over as a channel; timers feed the live chart and log; the switch in the status bar serves this app's state over HTTP. The Theme button tweaks Kit's look for every page, and each card's Source button shows the file it is.",
      { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" }),
    row { width = "grow", gap = 16, cross_gap = 16, min_column = 200,
      kit.stat({ label = "Components", value = "42" }),
      kit.stat({ label = "Rows in the table", value = tostring(rows),
        delta = m.rows_ms and string.format("built in %.0f ms off the UI thread", m.rows_ms), tone = "ok" }),
      kit.stat({ label = "Skins", value = "3", delta = "Ledger · Phosphor · Neutral" }),
      kit.stat({ label = "Live samples", value = tostring(#m.series.t) }) }
end

local HEADS = {
  overview = overview,
  keyboard = function()
    return text("Each of these is one Tab stop. The arrows move inside it; Enter or Space picks; Tab moves on.",
        { size = 15, bold = true, family = "$kitUi", color = "$kitFg", wrap = "word" }),
      text("Tab lands on the chosen item, not the first. A click focuses without the ring; the keyboard shows it. Components nobody picks in (no on_select) take no keyboard at all.",
        { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
  end,
  loading = function()
    return text("What a view shows while it waits: skeletons in the shape of what is coming, and a shader's box.",
      { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" })
  end,
}

function M.page(m, value)
  local page = M.PAGES[1]
  for _, p in ipairs(M.PAGES) do
    if p.value == value then page = p end
  end
  local t = { width = "grow", gap = 16 }
  local head = HEADS[page.value]
  if head then
    for _, n in ipairs({ head(m) }) do t[#t + 1] = n end
  end
  for _, c in ipairs(page.cards) do
    t[#t + 1] = type(c) == "table" and grid(m, c) or card(m, c)
  end
  return column(t)
end

function M.nav(m)
  local t = {}
  for _, p in ipairs(M.PAGES) do
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

return M
