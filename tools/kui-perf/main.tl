-- kui-perf: kui's trace inspector (ADR 0075), a Kit app on the runner.
-- This is its shell — the file and the window: opening a recording from
-- the command line, File ▸ Open, a file dropped on the window or the
-- recent ones; the File and View menus; Live, which brings what the
-- watched file gains; the look and the preferences. Everything about one
-- recording is the inspector's (inspector.tl).
--
--   kui perf session.kuitrace
--   kui run tools/kui-perf -- session.kuitrace

local kit = require("kit.ui")
local inspector = require("inspector")
local state = require("state")

local type Model = state.Model
local type Msg = state.Msg
local type Prefs = state.Prefs

global model: Model

local PREFS = kui.data_dir and (kui.data_dir .. "/prefs.json") or nil
local RECENT = 8

local function load_prefs(): Prefs
  local p: Prefs = { recent = {}, lanes = {} }
  if not PREFS then return p end
  local ok, text = kui.fs.try_read(PREFS)
  if not ok then return p end
  local fine, v = kui.json.try_decode(text)
  if not fine or type(v) ~= "table" then return p end
  local t = v as {string:any}
  p.recent = (t.recent or {}) as {string}
  p.lanes = (t.lanes or {}) as {string:{string}}
  p.split = t.split as number
  p.dock = t.dock as number
  p.helped = t.helped as boolean
  p.name = t.name as string
  p.min_us = t.min_us as number
  p.frames = t.frames as string
  p.devtools = t.devtools as boolean
  return p
end

-- What the inspector holds that is a preference, written beside the app.
local function save_prefs(m: Model)
  if not PREFS then return end
  local p = m.prefs
  local st = m.ins
  if st then
    p.split, p.dock, p.name, p.min_us, p.frames, p.devtools = st.split, st.dock, st.name, st.min_us, st.frames, st.devtools
    local app = (st.session.app or "") as string
    p.lanes[app] = st.lanes
  end
  kui.fs.try_mkdir(kui.data_dir)
  kui.fs.try_write(PREFS, kui.json.encode(p))
end

local function remember(m: Model, path: string)
  local recent: {string} = { path }
  for _, r in ipairs(m.prefs.recent) do
    if r ~= path and #recent < RECENT then recent[#recent + 1] = r end
  end
  m.prefs.recent = recent
end

local function open(m: Model, path: string)
  local ok, rec = kui.trace.try_open(path, { message = { kind = "moved" } })
  if not ok then
    m.error = rec as string
    return
  end
  m.path, m.error, m.waiting = path, nil, 0
  m.ins = inspector.new(rec as kui.Recording, m.prefs)
  -- The keys' card, the first time a recording opens.
  if not m.prefs.helped then m.help, m.prefs.helped = true, true end
  remember(m, path)
  save_prefs(m)
end

global function init(): Model
  local m: Model = { live = true, waiting = 0, prefs = load_prefs() }
  local path = kui.args[1]
  if path then open(m, path) end
  return m
end

local function ask_open()
  kui.next_frame(function(env: kui.Env)
    env.request_files({ mode = "open", title = "Open a recording", tag = { kind = "picked" } })
  end)
end

-- What came while the file moved: in, or waiting for a refresh.
local function poll(m: Model)
  local st = m.ins
  if not st then return end
  local before = #st.rec.frames
  local how = st.rec:poll()
  if how == "reset" then
    m.ins = inspector.new(st.rec, m.prefs)
    m.waiting = 0
  elseif #st.rec.frames ~= before then
    inspector.grew(st)
  end
end

local function first_path(t: {string:any}): string
  local paths = t and t.paths as {string}
  return paths and paths[1] or nil
end

global function update(msg: Msg)
  local m = model
  local k = msg.kind
  if m.ins and inspector.update(m.ins, msg) then
    if k == "ins_split" or k == "ins_lane" or k:find("^ins_[nfmod]") then save_prefs(m) end
    return
  end
  if k == "help" then m.help = not m.help
  elseif k == "help_close" then m.help = false
  elseif k == "moved" then
    if m.live then poll(m) else m.waiting = m.waiting + 1 end
  elseif k == "refresh" then
    m.waiting = 0
    poll(m)
  elseif k == "live" then
    m.live = msg.toggle.on as boolean
    if m.live then m.waiting = 0; poll(m) end
  elseif k == "open" then ask_open()
  elseif k == "picked" then
    local p = first_path(msg.files)
    if p then open(m, p) end
  elseif k == "dropped" then
    -- Only a file let go over the window: one dragged across it on its
    -- way elsewhere replaced the recording as it came in.
    local d = msg.drop as {string:any}
    if d and d.phase ~= "drop" then return end
    local p = first_path(msg.files) or first_path(d)
    if p then open(m, p) end
  elseif k == "recent" then open(m, msg.name)
  elseif k == "menu" then
    local item = (msg.menu.item or {}) as {string:any}
    local id = item.id as string
    if id == "open" then ask_open()
    elseif id and id:find("^recent:") then open(m, id:sub(8))
    elseif id == "fit" and m.ins then inspector.update(m.ins, { kind = "ins_fit" })
    elseif id == "latest" and m.ins then inspector.update(m.ins, { kind = "ins_latest" })
    elseif id == "live" then m.live = not m.live
    elseif id == "help" then m.help = true
    end
  elseif k == "keys" then
    local key = msg.key
    if not key then return end
    if key.phase == "down" and (key.super or key.ctrl) and key.code == "o" then ask_open() return end
    if key.phase == "down" and (key.text == "?" or (key.code == "/" and key.shift)) then m.help = not m.help return end
    if m.help then
      if key.phase == "down" and key.code == "escape" then m.help = false end
      return
    end
    if m.ins then inspector.key(m.ins, key) end
  end
end

local function menus(m: Model): kui.Node
  local recent: {any} = {}
  for _, r in ipairs(m.prefs.recent) do
    recent[#recent + 1] = { label = r:match("[^/\\]+$") or r, id = "recent:" .. r }
  end
  if #recent == 0 then recent = { { label = "Nothing yet", disabled = true } } end
  return menu_bar {
    standard_menus = true,
    on_menu = { kind = "menu" },
    menu = {
      { label = "File", items = {
        { label = "Open…", id = "open", accel = "mod+o" },
        { label = "Open Recent", items = recent },
      } },
      { label = "View", items = {
        { label = "Fit the recording", id = "fit", accel = "0" },
        { label = "Go to the latest", id = "latest" },
        { label = "Live", id = "live", checked = m.live },
      } },
      { label = "Help", items = {
        { label = "Keys and gestures", id = "help", accel = "shift+/" },
      } },
    },
  }
end

local function toolbar(m: Model): kui.Node
  local st = m.ins
  local name = m.path and (m.path:match("[^/\\]+$") or m.path) or "No recording"
  return kit.toolbar({ key = "top" }, {
    text("kui-perf", { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" } as kui.text),
    kit.button({ key = "open", label = "Open…", icon = "folder", shortcut = "⌘O", on_press = { kind = "open" } }),
    text(name, { size = "$kitSizeLabel", family = "$kitData", color = "$kitMuted", max_lines = 1, ellipsis = true } as kui.text),
    trailing = {
      st and not m.live and m.waiting > 0 and kit.button({ key = "refresh", icon = "refresh", variant = "primary",
        label = ("%d new · refresh"):format(m.waiting), on_press = { kind = "refresh" } }) or nil,
      st and m.live and not st.follow and kit.button({ key = "latest", label = "↦ latest", variant = "ghost",
        on_press = { kind = "ins_latest" } }) or nil,
      st and kit.button({ key = "fit", label = "Fit", variant = "ghost", on_press = { kind = "ins_fit" } }) or nil,
      kit.switch({ key = "live", label = "Live", on = m.live, on_toggle = { kind = "live" } }),
      kit.icon_button({ key = "help", icon = "info", label = "Keys and gestures (?)", on_press = { kind = "help" } }),
    },
  })
end

local function status(m: Model): kui.Node
  local st = m.ins
  local parts: {any} = {}
  if st then
    local s = st.session
    local seconds = ((s.finish or 0) as number - (s.start or 0) as number) / 1000
    local line = ("%s · %s · %.0f Hz · %.2f ms budget · %.1f s · %d frames · %d spans"):format(
      (s.app ~= "" and s.app or "app") as string, (s.platform or "?") as string, (s.hz or 0) as number,
      (s.budget or 0) as number, seconds, s.frames as integer, s.spans as integer)
    parts[#parts + 1] = text(line, { size = "$kitSizeLabel", family = "$kitData", color = "$kitMuted", max_lines = 1, ellipsis = true } as kui.text)
    if inspector.filtering(st) then
      parts[#parts + 1] = kit.badge({ key = "filtered", label = ("filtered: %d spans, %d frames shown"):format(#st.rec.spans, #st.rec.frames), tone = "accent" })
    end
  elseif m.error then
    parts[1] = kit.badge({ key = "err", label = m.error, tone = "danger" })
  end
  return kit.statusbar({ key = "status" }, parts)
end

local function welcome(m: Model): kui.Node
  local recent: kui.Props = { key = "recents", gap = 2, cross_align = "center" }
  for i, r in ipairs(m.prefs.recent) do
    recent[#recent + 1] = kit.button({ key = "recent-" .. i, label = r, variant = "ghost", on_press = { kind = "recent", name = r } })
  end
  return column { key = "welcome", width = "grow", height = "grow", main_align = "center", cross_align = "center", gap = 12,
    kit.empty({ key = "empty", title = m.error and "That recording would not open" or "Open a recording",
      detail = m.error or "A .kuitrace from the devtools' Perf tab, KUI_PERF_TRACE or kui-trace synth — or drop one here." }),
    kit.button({ key = "open-big", label = "Open…", variant = "primary", on_press = { kind = "open" } }),
    #recent > 0 and column(recent) or nil,
  }
end

-- The keys' card: what the keys, the wheel and a drag do, a line each,
-- after two lines on how the window reads.
local KEYS: {{string}} = {
  { "Frames", "" },
  { "← →  h l", "the frame before or after" },
  { "[ ]", "the slow frame before or after" },
  { "{ }", "the burn before or after" },
  { "Spans", "" },
  { "↓ j", "the selected span's first child, or the top lane's first span" },
  { "↑ k", "the selected span's parent" },
  { "Esc", "clear the range, then the selection" },
  { "The window", "" },
  { "wheel · pinch", "zoom about the pointer, over the timeline, a lane or the overview" },
  { "swipe sideways · ⇧ wheel", "pan" },
  { "drag the lanes", "pan" },
  { "w s", "zoom in, out" },
  { "a d", "pan left, right" },
  { "drag the overview", "move the window" },
  { "The range", "" },
  { "drag the time axis", "select a range; a click clears it" },
  { "double-click", "a span, a frame's row or a burn's: its time is the range" },
  { "f · z", "zoom to the range, or fit the selected frame or span" },
  { "0", "the whole recording" },
  { "Elsewhere", "" },
  { "/", "filter spans by name" },
  { "a table's header", "sort by it; again, the other way" },
  { "⌘O", "open a recording" },
  { "?", "this card" },
}

local function help(): kui.Node
  local body: kui.Props = {
    text("The overview is every frame's work; the box on it is the window the timeline shows, and the tables under it read the window once it is still. Pick a frame or a span to read it in the detail.",
      { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" } as kui.text),
  }
  for i, k in ipairs(KEYS) do
    if k[2] == "" then
      body[#body + 1] = row { key = "h" .. i, pad = { t = 6 },
        text(k[1], { size = "$kitSizeLabel", family = "$kitUi", bold = true, color = "$kitAccent" } as kui.text) }
    else
      body[#body + 1] = row { key = "k" .. i, width = "grow", gap = 12, cross_align = "center",
        row { width = 170, text(k[1], { size = "$kitSizeLabel", family = "$kitData", color = "$kitFg", max_lines = 1 } as kui.text) },
        text(k[2], { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", wrap = "word" } as kui.text) }
    end
  end
  local children = body as {any:any}
  children.actions = { kit.button({ key = "help-ok", label = "Got it", variant = "primary", on_press = { kind = "help_close" } }) }
  return kit.dialog({ key = "help-card", title = "Keys and gestures", width = 520, on_dismiss = { kind = "help_close" } },
    children)
end

global function view(env: kui.Env): kui.Node
  local m = model
  return column { key = "root", width = "grow", height = "grow", bg = "$kitBg",
    on_key = { kind = "keys" }, key_focus = env.focus == nil or nil,
    on_drop = { kind = "dropped" },
    kit.settings(({
      skin = "phosphor", density = "compact", hair = 1.5,
      font_ui = { "Berkeley Mono Variable", "sans" },
      font_data = { "Berkeley Mono Variable", "mono" },
      roles = { accent = 0xffd400ff },
    } as {string:any}) as kit.SettingsProps),
    menus(m),
    toolbar(m),
    m.ins and inspector.view(m.ins, env) or welcome(m),
    status(m),
    m.help and help() or nil,
  }
end
