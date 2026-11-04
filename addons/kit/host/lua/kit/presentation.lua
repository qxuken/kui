-- Presentation authoring for the kui runner. Callbacks remain in the host;
-- only the resulting component trees cross the Kit extension interface.
local kit = require("kit.ui")
local M = {}
local Session = {}; Session.__index = Session
local layouts = { paper = "editorial", signal = "briefing", stage = "split", minimal = "minimal" }
local function copy(v, seen)
  if type(v) ~= "table" then return v end
  seen = seen or {}; if seen[v] then return seen[v] end
  local out = {}; seen[v] = out
  for k, value in pairs(v) do out[k] = copy(value, seen) end
  return out
end
local function dimensions(s) return { width = s and (s.width or s.w) or 1280, height = s and (s.height or s.h) or 720 } end
local function message(kind, fields)
  local out = { presentation = kind }
  for k, v in pairs(fields or {}) do out[k] = v end
  return out
end
local function presenter_shortcut(deck, key)
  local chord = deck.presenter_shortcut
  if not chord then return (key.super or key.ctrl) and key.shift and key.code:lower() == "p" end
  local function same(a, b) return (not not a) == (not not b) end
  return key.code:lower() == chord.key:lower() and same(key.shift, chord.shift) and same(key.alt, chord.alt)
    and (chord.primary and (key.ctrl or key.super) or not chord.primary and same(key.ctrl, chord.ctrl) and same(key.super, chord.super))
end
local function presenter_accel(deck)
  local chord = deck.presenter_shortcut
  if not chord then return "mod+shift+p" end
  local parts = {}
  for _, name in ipairs({ "primary", "ctrl", "super", "alt", "shift" }) do
    if chord[name] then parts[#parts + 1] = name == "primary" and "mod" or name end
  end
  parts[#parts + 1] = chord.key:lower()
  return table.concat(parts, "+")
end
local function presenter_key_label(deck, platform)
  local chord = deck.presenter_shortcut or { primary = true, shift = true, key = "p" }
  local mac = platform == "macos"
  local parts = {}
  if chord.ctrl or (chord.primary and not mac) then parts[#parts + 1] = mac and "⌃" or "Ctrl" end
  if chord.alt then parts[#parts + 1] = mac and "⌥" or "Alt" end
  if chord.shift then parts[#parts + 1] = mac and "⇧" or "Shift" end
  if chord.super or (chord.primary and mac) then parts[#parts + 1] = mac and "⌘" or (platform == "windows" and "Win" or "Super") end
  parts[#parts + 1] = chord.key:upper()
  return table.concat(parts, mac and "" or "+")
end

function M.new(deck, options, previous)
  options = options or {}
  assert(type(deck) == "table" and type(deck.slides) == "table" and #deck.slides > 0, "a presentation needs slides")
  for _, theme in pairs({ deck.theme, deck.preset, deck.settings and deck.settings.theme }) do
    assert(layouts[theme], "theme must be paper, signal, stage or minimal")
  end
  if deck.layout then
    assert(deck.layout == "editorial" or deck.layout == "briefing" or deck.layout == "split" or deck.layout == "minimal",
      "layout must be editorial, briefing, split or minimal")
  end
  if deck.canvas then
    for _, n in pairs(dimensions(deck.canvas)) do
      assert(type(n) == "number" and n > 0 and n < math.huge, "canvas dimensions must be finite and positive")
    end
  end
  local seen = {}
  for _, slide in ipairs(deck.slides) do
    assert(type(slide.key) == "string" and slide.key ~= "" and not seen[slide.key], "slide keys must be nonempty and unique")
    assert(type(slide.view) == "function", "slide " .. slide.key .. " needs view(context)")
    assert(slide.steps == nil or (math.type(slide.steps) == "integer" and slide.steps >= 0), "slide " .. slide.key .. ": steps must be a nonnegative integer")
    assert(not slide.snapshot or (math.type(slide.steps) == "integer" and slide.steps >= 0),
      "slide " .. slide.key .. ": a replacement snapshot needs an explicit nonnegative steps count")
    seen[slide.key] = true
  end
  local self = setmetatable({ deck = deck, options = options, counts = {}, generation = (previous and previous.generation or 0) + 1,
    disposers = {}, requests = {}, size = dimensions(options.size), initialized_windows = {}, loaded_fonts = {} }, Session)
  local old = previous and previous.model
  local states, versions = {}, {}
  for _, slide in ipairs(deck.slides) do
    local version = slide.state_version or 0
    states[slide.key] = old and old.versions[slide.key] == version and old.states[slide.key]
      or (slide.init and slide.init() or {})
    versions[slide.key] = version
  end
  local index = 1
  if old then
    for i, slide in ipairs(deck.slides) do if slide.key == old.key then index = i end end
    if not seen[old.key] then index = math.min(old.index, #deck.slides) end
  end
  self.model = { index = index, key = deck.slides[index].key, step = old and old.step or 0,
    states = states, versions = versions, theme = deck.theme or deck.preset or (deck.settings and deck.settings.theme) or "minimal",
    layout = deck.layout or layouts[deck.preset] or "minimal",
    presenter = old and old.presenter or options.presenter == true, overview = false, help = false, confirm_restart = false,
    display = old and old.display or "current", elapsed = previous and previous:elapsed() or 0, running = false }
  for i = 1, #deck.slides do self:render_slide(i, 0, "snapshot", self.size) end
  self.model.step = math.min(self.model.step, self.counts[self.model.key] or 0)
  self.model.live_sync = not old or old.live_sync ~= false
  self.model.preview = self:position()
  if not self.model.live_sync and old.preview then
    local nearest = math.min(old.preview.index, #deck.slides)
    self.model.preview = { index = nearest, key = deck.slides[nearest].key, step = 0 }
    for i, slide in ipairs(deck.slides) do
      if slide.key == old.preview.key then
        self.model.preview = { index = i, key = slide.key, step = math.min(old.preview.step, self.counts[slide.key] or 0) }
      end
    end
  end
  if previous then previous:dispose() end
  if old and old.running and not options.snapshot then self:update(message("timer")) end
  return self
end

function Session:now() return self.options.clock and self.options.clock() or kui.now() end
function Session:send(msg)
  if self.options.dispatch then self.options.dispatch(msg) else self:update(msg) end
end
function Session:context(index, step, mode, size, reveal_guide)
  local slide = self.deck.slides[index]
  local generation = self.generation
  local reveal_count = 0
  local c = { step = step, mode = mode, layout = self.model.layout, theme = self.model.theme, size = dimensions(size), now = mode == "snapshot" and (self.options.snapshot_time or 0) or self:now(),
    state = mode == "snapshot" and copy(self.model.states[slide.key]) or self.model.states[slide.key] }
  function c.msg(msg)
    local out = type(msg) == "table" and copy(msg) or {}
    for k, v in pairs(message("demo", { slide = slide.key, generation = generation, mode = mode, message = msg })) do out[k] = v end
    return out
  end
  function c.dispatch(msg) if mode == "live" and generation == self.generation then self:send(c.msg(msg)) end end
  function c.reveal(at, children, options)
    assert(math.type(at) == "integer" and at >= 1, "reveal steps must be positive integers")
    self.counts[slide.key] = math.max(self.counts[slide.key] or 0, at)
    reveal_count = reveal_count + 1
    if reveal_guide then
      return column { key = "reveal." .. tostring(options and options.key or reveal_count), width = "grow",
        border = step < at and { w = 1, color = "$kitAccent" } or nil, radius = "$kitRadius",
        bg = step < at and "$kitHover" or nil,
        column { width = "grow", opacity = step >= at and 1 or 0.28, table.unpack(children) },
        column { width = 24, height = 24, radius = 12, bg = "$kitAccent", main_align = "center", cross_align = "center",
          float = { anchor = "parent", dx = -32 },
          text(tostring(at), { size = 14, bold = true, family = "$kitUi", color = "$kitOnAccent" }) } }
    end
    return kit.visibility({ key = "reveal." .. tostring(options and options.key or reveal_count),
      visible = step >= at, collapse = options and options.collapse }, children)
  end
  function c.content(props, children)
    props = copy(props or {}); props.layout = props.layout or self.model.layout
    return kit.content(props, children)
  end
  local function timer(ms, msg, once)
    assert(mode == "live" and self.active == slide.key and generation == self.generation, "managed timers belong in activate(context)")
    local schedule = once and kui.timer.after or kui.timer.every
    local handle = schedule(ms, function()
      if generation == self.generation then c.dispatch(type(msg) == "function" and msg() or msg) end
    end)
    self.disposers[#self.disposers + 1] = function() handle:cancel() end
    return handle
  end
  function c.every(ms, msg) return timer(ms, msg, false) end
  function c.after(ms, msg) return timer(ms, msg, true) end
  function c.spawn(fn)
    assert(mode == "live" and self.active == slide.key, "managed tasks belong in activate(context)")
    local listeners = {}
    local signal = { aborted = false }
    function signal.on_abort(callback)
      if signal.aborted then callback() else listeners[#listeners + 1] = callback end
    end
    self.disposers[#self.disposers + 1] = function()
      signal.aborted = true
      for _, callback in ipairs(listeners) do callback() end
    end
    kui.spawn(function() local msg = fn(signal); if msg and not signal.aborted then c.dispatch(msg) end end)
    return signal
  end
  return c
end

function Session:stop_active()
  self.generation = self.generation + 1
  local disposers = self.disposers; self.disposers = {}; self.active = nil
  for i = #disposers, 1, -1 do disposers[i]() end
end
function Session:pace()
  local wanted = not self.options.snapshot and self.model.key or nil
  if self.active == wanted then return end
  self:stop_active()
  if wanted then
    self.active = wanted
    local slide = self.deck.slides[self.model.index]
    if slide.activate then
      local dispose = slide.activate(self:context(self.model.index, self.model.step, "live", self.size))
      if dispose then self.disposers[#self.disposers + 1] = dispose end
    end
  end
end
function Session:dispose()
  self:stop_active()
  if self.timer then self.timer:cancel(); self.timer = nil end
end
function Session:position(source)
  local m = source == "presenter" and not self.model.live_sync and self.model.preview or self.model
  return { index = m.index, key = m.key, step = m.step }
end
function Session:navigate(position, source)
  local m = self.model
  if source == "presenter" and not m.live_sync then m.preview = position
  else
    m.index, m.key, m.step = position.index, position.key, position.step
    if m.live_sync then m.preview = self:position() end
    self:pace()
  end
end
function Session:present_slide() self:navigate(copy(self.model.preview)) end
function Session:next(source)
  local m = self:position(source)
  if m.step < (self.counts[m.key] or 0) then m.step = m.step + 1
  elseif m.index < #self.deck.slides then m.index, m.step = m.index + 1, 0 end
  m.key = self.deck.slides[m.index].key; self:navigate(m, source)
end
function Session:previous(source)
  local m = self:position(source)
  if m.step > 0 then m.step = m.step - 1
  elseif m.index > 1 then m.index = m.index - 1; m.step = self.counts[self.deck.slides[m.index].key] or 0 end
  m.key = self.deck.slides[m.index].key; self:navigate(m, source)
end
function Session:jump(key, source)
  for i, slide in ipairs(self.deck.slides) do
    if slide.key == key then
      self.model.overview = false; self:navigate({ index = i, key = key, step = 0 }, source); return
    end
  end
  error("unknown slide: " .. tostring(key))
end
function Session:elapsed()
  return self.model.elapsed + (self.model.running and self:now() - self.started_at or 0)
end
function Session:update(msg)
  local m, kind = self.model, msg.presentation
  if kind == "next" then self:next(msg.source)
  elseif kind == "previous" then self:previous(msg.source)
  elseif kind == "jump" then self:jump(msg.slide, msg.source)
  elseif kind == "present-slide" then self:present_slide()
  elseif kind == "live-sync" then
    local enabled = msg.toggle and msg.toggle.on == true
    if not enabled and m.live_sync then m.preview = self:position() end
    if enabled and not m.live_sync then self:present_slide() end
    m.live_sync = enabled
    if enabled then m.preview = self:position() end
  elseif kind == "start" then self:pace()
  elseif kind == "overview" then
    m.overview = not m.overview
    self:update(message("presenter"))
  elseif kind == "help" then m.help = true; m.confirm_restart = false; self:update(message("presenter"))
  elseif kind == "help-close" then m.help = false
  elseif kind == "restart-request" then m.confirm_restart = true; m.help = false
  elseif kind == "restart-cancel" then m.confirm_restart = false
  elseif kind == "menu" and msg.menu then
    if msg.menu.item == "presenter" or msg.menu.item == "overview" or msg.menu.item == "help" then self:update(message(msg.menu.item)) end
  elseif kind == "presenter" then
    m.presenter = true
    if self.presenter_id then self.requests[#self.requests + 1] = { kind = "focus", id = self.presenter_id } end
  elseif kind == "presenter-window" and msg.window and msg.window.phase == "closed" then m.presenter = false; self.presenter_id = nil
  elseif kind == "display" then m.display = msg.change.value
  elseif kind == "audience-layout" then
    self.audience_size = { width = msg.layout.w, height = msg.layout.h }; self.size = self.audience_size
  elseif kind == "audience" or kind == "fullscreen" then
    self.requests[#self.requests + 1] = { kind = "fullscreen", enabled = kind == "audience" or not self.fullscreen }
  elseif kind == "timer" then
    if m.running then m.elapsed = self:elapsed(); m.running = false; self.timer:cancel(); self.timer = nil
    else
      m.running = true; self.started_at = self:now()
      self.timer = kui.timer.every(1000, function() self:send(message("tick")) end)
    end
  elseif kind == "reset-timer" then m.elapsed = 0; self.started_at = self:now()
  elseif kind == "restart" then
    self:stop_active(); m.confirm_restart = false; m.elapsed = 0; self.started_at = self:now()
    for _, slide in ipairs(self.deck.slides) do m.states[slide.key] = slide.init and slide.init() or {} end
    self:jump(self.deck.slides[1].key)
    m.preview = self:position()
  elseif kind == "demo" and msg.mode == "live" and self.active == msg.slide and msg.generation == self.generation then
    local slide = self.deck.slides[m.index]
    if slide.update then
      local arrived = type(msg.message) == "table" and copy(msg.message) or { tag = msg.message }
      for name, value in pairs(msg) do
        if name ~= "presentation" and name ~= "slide" and name ~= "generation" and name ~= "mode" and name ~= "message" then arrived[name] = value end
      end
      local value = slide.update(m.states[slide.key], arrived)
      if value ~= nil then m.states[slide.key] = value end
    end
  elseif kind == "keys" and msg.key and msg.key.phase == "down" then
    local k, code = msg.key, msg.key.code:lower()
    if k["repeat"] and code ~= "left" and code ~= "right" and code ~= "up" and code ~= "down" and code ~= "pageup" and code ~= "pagedown" then return m end
    if presenter_shortcut(self.deck, k) then self:update(message("presenter"))
    elseif (k.super or k.ctrl) and code == "enter" and msg.source == "presenter" then self:present_slide()
    elseif not (k.super or k.ctrl or k.alt) then
      if code == "?" or (code == "/" and k.shift) then self:update(message("help"))
      elseif code == "right" or code == "down" or code == "pagedown" or code == "space" then if k.shift then self:previous(msg.source) else self:next(msg.source) end
      elseif code == "left" or code == "up" or code == "pageup" then self:previous(msg.source)
      elseif code == "home" then self:jump(self.deck.slides[1].key, msg.source)
      elseif code == "end" then self:jump(self.deck.slides[#self.deck.slides].key, msg.source)
      elseif code == "o" then self:update(message("overview"))
      elseif code == "f" then self:update(message("fullscreen"))
      elseif code == "escape" then
        if m.help then m.help = false elseif m.confirm_restart then m.confirm_restart = false
        elseif m.overview then m.overview = false; self:pace() elseif self.fullscreen then self:update(message("fullscreen")) end
      elseif code == "tab" then self.tab = { window = msg.window_id or 0, backwards = k.shift } end
    end
  end
  return m
end

-- Remove animation declarations from snapshot trees, including component
-- props. It is not a sandbox: view/snapshot callbacks must be pure.
local function still(node)
  if type(node) ~= "table" then return node end
  local out = {}
  for k, v in pairs(node) do
    if k ~= "enter" and k ~= "exit" and k ~= "keyframes" and k ~= "transition" and k ~= "animate" then out[k] = still(v) end
  end
  if node.animate ~= nil then out.animate = false end
  if node.type == "component" and node.name and node.name:match("/shader$") then out.props.animate = false end
  if node.type == "component" and node.name and node.name:match("/line_chart$") then out.props.readout, out.props.focusable = false, false end
  return out
end
function Session:render_slide(index, step, mode, size, reveal_guide)
  local slide = self.deck.slides[index]
  self.counts[slide.key] = slide.steps or (mode == "snapshot" and self.counts[slide.key]) or 0
  local c = self:context(index, step, mode or "live", self.deck.canvas or size or self.size, reveal_guide)
  local view = mode == "snapshot" and slide.snapshot or slide.view
  local ok, node = pcall(view or slide.view, c)
  if not ok then error("slide " .. slide.key .. ", step " .. step .. ": " .. tostring(node), 0) end
  return mode == "snapshot" and still(node) or node
end
function Session:settings()
  local settings = copy(self.deck.settings or {})
  settings.theme = self.model.theme
  return kit.settings(settings)
end
local function label(s, size, color, bold)
  return text(s, { size = size or "$kitSize", family = "$kitUi", color = color or "$kitFg", bold = bold, wrap = "word" })
end
function Session:preview(index, w, h, step, action)
  local size = dimensions(self.deck.canvas or self.size)
  local factor = math.min(w / size.width, h / size.height)
  return column { width = w, height = h, clip = true, role = "none", bg = "$kitBg",
    column { width = size.width, height = size.height, scale = factor, pivot_x = 0, pivot_y = 0,
      self:render_slide(index, step or 0, "snapshot", size, true) },
    column { width = w, height = h, float = { anchor = "parent" }, keep_focus = true,
      cursor = action and "pointer" or nil, on_click = action or message("preview"), on_hover = message("preview") } }
end
function Session:overview()
  local items = { width = "grow", gap = 20, cross_gap = 20, min_column = 320 }
  for i, slide in ipairs(self.deck.slides) do
    items[#items + 1] = column { width = "grow", gap = 10,
      self:preview(i, 320, 200, nil, message("jump", { slide = slide.key, source = "presenter" })),
      kit.button({ label = string.format("%02d  %s", i, slide.title or slide.key), on_press = message("jump", { slide = slide.key, source = "presenter" }) }) }
  end
  return column { width = "grow", height = "grow", pad = 32, gap = 24, scroll_y = true,
    label("Overview", "$kitSizeTitle", "$kitFg", true), row(items) }
end
function Session:menu(env)
  if self.options.snapshot or env.platform ~= "macos" then return nil end
  return menu_bar { standard_menus = true, on_menu = message("menu"), menu = {
    { label = "Presentation", items = {
      { label = "Show Presenter", id = "presenter", accel = presenter_accel(self.deck) },
      { label = "Slide Overview", id = "overview", accel = "mod+shift+o" },
      { label = "Keyboard Shortcuts", id = "help", accel = "mod+/" },
    } },
  } }
end
function Session:dialogs(env)
  local m = self.model
  if m.help then
    local keys = {
      { "→ / ↓ / Space", "Next reveal or slide (also Page Down)" }, { "← / ↑ / Shift+Space", "Previous reveal or slide (also Page Up)" },
      { "Home / End", "First or last slide" }, { "O", "Private slide overview" }, { "F", "Audience fullscreen" },
      { env.platform == "macos" and "⌘Enter" or "Ctrl+Enter", "Present the selected slide from the presenter" },
      { "Esc", "Close a dialog, overview or fullscreen" }, { presenter_key_label(self.deck, env.platform), "Open or focus the presenter" },
      { "?", "Show these shortcuts" }, { "Tab / Shift+Tab", "Move between controls; focused controls keep their keys" },
    }
    local body = { label("These controls are private. The audience stays on the current slide.", nil, "$kitMuted") }
    for _, key in ipairs(keys) do body[#body + 1] = row { width = "grow", gap = 12,
      column { width = 180, kit.badge({ label = key[1] }) }, column { width = "grow", label(key[2]) } } end
    body.actions = { kit.button({ label = "Close shortcuts", on_press = message("help-close") }) }
    return kit.dialog({ title = "Keyboard shortcuts", width = math.min(540, env.viewport_w - 32), on_dismiss = message("help-close") }, body)
  elseif m.confirm_restart then
    return kit.dialog({ title = "Restart this talk?", width = 420, on_dismiss = message("restart-cancel") }, {
      label("Return to the first slide and reset every demo and the timer."), actions = {
        kit.button({ label = "Keep presenting", on_press = message("restart-cancel") }),
        kit.button({ label = "Restart talk", variant = "danger", on_press = message("restart") }),
      },
    })
  end
end
function Session:presenter(env)
  local m, selected = self.model, self:position("presenter")
  local slide = self.deck.slides[selected.index]
  self.presenter_id = env.window.id
  local wide = env.viewport_w >= 900
  local width = math.max(240, math.min(900, env.viewport_w - (wide and 392 or 56)))
  local next_index = math.min(selected.index + 1, #self.deck.slides)
  local next_width = wide and 296 or math.min(400, width)
  local display_choices = { { value = "current", label = "Current display" } }
  for i, name in ipairs(env.display_names()) do display_choices[#display_choices + 1] = { value = tostring(i - 1), label = name } end
  local elapsed = math.floor(self:elapsed() / 1000)
  local integrated = self.options.toolbar_in_titlebar == true and env.window.custom_chrome
  local current = column { width = "grow", height = wide and "grow" or nil, gap = 16,
    kit.panel({ title = m.live_sync and "Current slide" or "Private preview", subtitle = slide.title or slide.key }, {
      self:preview(selected.index, width, width * self.size.height / self.size.width, selected.step),
    }),
    kit.panel({ title = "Speaker notes", grow = wide, scroll = wide }, {
      kit.markdown({ text = slide.notes and slide.notes ~= "" and slide.notes or "No notes for this slide." }),
    }),
  }
  local sidebar = column { width = wide and 320 or "grow", height = wide and "grow" or nil, gap = 16, scroll_y = wide,
    kit.panel({ title = "Up next", subtitle = self.deck.slides[next_index].title or self.deck.slides[next_index].key }, {
      self:preview(next_index, next_width, next_width * self.size.height / self.size.width, 0),
    }),
    kit.panel({ title = "Timer" }, {
      kit.stat({ label = "Elapsed", value = string.format("%02d:%02d", elapsed // 60, elapsed % 60) }),
      row { gap = 6,
        kit.button({ label = m.running and "Pause timer" or "Start timer", icon = m.running and "pause" or "play", on_press = message("timer") }),
        kit.button({ label = "Reset timer", on_press = message("reset-timer") }) },
    }),
    kit.panel({ title = "Audience" }, {
      kit.select({ label = "Display", value = m.display, options = display_choices, on_change = message("display") }),
      kit.button({ label = "Show audience", icon = "external", on_press = message("audience") }),
    }),
  }
  local body = m.overview and self:overview() or (wide and row or column) { width = "grow", height = "grow", pad = 16, gap = 16,
    scroll_y = not wide, current, sidebar }
  return column { width = "grow", height = "grow", bg = "$kitBg",
    env.window.custom_chrome and not integrated and titlebar { title = (self.deck.title or "Presentation") .. " — Presenter" } or nil,
    kit.toolbar({ window_chrome = integrated }, {
      label(self.deck.title or "Presentation", "$kitSizeTitle", "$kitFg", true),
      kit.button({ label = m.overview and "Back to notes" or "Overview", on_press = message("overview") }),
      trailing = {
        kit.button({ label = "Restart…", icon = "reset", on_press = message("restart-request") }),
        kit.button({ label = "Previous", disabled = selected.index == 1 and selected.step == 0,
          on_press = message("previous", { source = "presenter" }) }),
        kit.button({ label = "Next", variant = m.live_sync and "primary" or "default",
          disabled = selected.index == #self.deck.slides and selected.step >= (self.counts[selected.key] or 0),
          on_press = message("next", { source = "presenter" }) }),
        not m.live_sync and kit.button({ label = "Present slide", variant = "primary", on_press = message("present-slide") }) or nil,
      },
    }), body,
    kit.statusbar({}, {
      label(string.format("Slide %d of %d", selected.index, #self.deck.slides), nil, "$kitMuted"),
      label(string.format("Step %d of %d", selected.step, self.counts[selected.key] or 0), nil, "$kitMuted"),
      trailing = { not m.live_sync and label(string.format("Audience: slide %d · step %d", m.index, m.step), nil, "$kitMuted") or nil,
        kit.switch({ key = "live-sync", label = "Live sync", on = m.live_sync, on_toggle = message("live-sync") }), label("?  Shortcuts", nil, "$kitMuted") },
    }), self:dialogs(env),
  }
end
function Session:view(env)
  if not self.initialized_windows[env.window.id] then
    env.set_window_close_hides(env.window.id, true); self.initialized_windows[env.window.id] = true
  end
  for _, path in ipairs(self.deck.fonts or {}) do
    if not self.loaded_fonts[path] then
      assert(env.load_font_file(path), "could not load font " .. path)
      self.loaded_fonts[path] = true
    end
  end
  local m = self.model
  local is_presenter = env.window_name() == "presenter"
  if self.tab and self.tab.window == env.window.id then
    if self.tab.backwards then env.focus_prev() else env.focus_next() end
    self.tab = nil
  end
  if not is_presenter then
    self.size = self.audience_size or { width = env.viewport_w, height = env.viewport_h }; self.fullscreen = env.window.fullscreen
  end
  local requests = self.requests; self.requests = {}
  for _, request in ipairs(requests) do
    if request.kind == "focus" then env.set_window_visible(request.id, true); env.focus_window(request.id)
    else
      env.set_window_visible(0, true)
      env.set_window_fullscreen(0, request.enabled, tonumber(m.display))
      env.focus_window(0)
    end
  end
  local root = { width = "grow", height = "grow", window_title = (self.deck.title or "Presentation") .. (is_presenter and " — Presenter" or " — Audience"),
    on_key = message("keys", { window_id = env.window.id, source = is_presenter and "presenter" or "audience" }), key_focus = env.focus == nil or nil, self:settings(), self:menu(env) }
  local body
  if is_presenter then body = self:presenter(env)
  else
    root.windows = m.presenter and not self.options.snapshot and { { name = "presenter", width = 1180, height = 860,
      on_window = message("presenter-window") } } or {}
    local mode = self.options.snapshot and "snapshot" or "live"
    local node = self:render_slide(m.index, m.step, mode, self.size)
    if self.deck.canvas then
      local size = dimensions(self.deck.canvas)
      local scale = math.min(self.size.width / size.width, self.size.height / size.height)
      node = column { width = size.width, height = size.height, scale = scale, pivot_x = 0, pivot_y = 0,
        float = { anchor = "parent", dx = (self.size.width - size.width * scale) / 2, dy = (self.size.height - size.height * scale) / 2 }, node }
    end
    body = column { width = "grow", height = "grow",
      not self.options.snapshot and not env.window.fullscreen and env.window.custom_chrome and titlebar { title = self.deck.title or "Presentation" } or nil,
      column { key = "slide." .. m.key, width = "grow", height = "grow", clip = true, on_layout = message("audience-layout"),
        transition = self.options.snapshot and 0 or 180, enter = self.options.snapshot and nil or { opacity = 0 },
        when = { { reduced_motion = true, transition = 0 } }, node },
    }
  end
  root[#root + 1] = column { width = "grow", height = "grow", bg = "$kitBg", body }
  return column(root)
end
function Session:export_plan(all_steps)
  local out = {}
  for i, slide in ipairs(self.deck.slides) do
    self:render_slide(i, 0, "snapshot", self.size)
    local last = self.counts[slide.key] or 0
    for step = all_steps and 0 or last, last do
      out[#out + 1] = { key = slide.key, title = slide.title or slide.key, step = step, index = i }
    end
  end
  return out
end

function M.present(deck, options)
  options = copy(options or {})
  local flags = {}
  for i, arg in ipairs(kui.args or {}) do if arg:sub(1, 2) == "--" then flags[arg:sub(3)] = kui.args[i + 1] or true end end
  if flags.theme then deck = copy(deck); deck.theme = flags.theme end
  if flags.layout then deck = copy(deck); deck.layout = flags.layout end
  if flags.canvas then
    local w, h = tostring(flags.canvas):match("^(%d+)x(%d+)$")
    assert(w and tonumber(w) > 0 and tonumber(h) > 0, "--canvas needs WIDTHxHEIGHT")
    deck = copy(deck); deck.canvas = { w = tonumber(w), h = tonumber(h) }
  end
  options.clock = options.clock or kui.now
  options.snapshot = flags["presentation-capture"] ~= nil or flags["presentation-plan"] ~= nil
  if flags["presentation-width"] then
    options.size = { width = assert(tonumber(flags["presentation-width"])), height = assert(tonumber(flags["presentation-height"])) }
  end
  options.presenter = options.presenter or flags.presenter ~= nil
  options.snapshot_time = tonumber(flags["presentation-time"]) or 0
  options.dispatch = kui.dispatch
  local self = M.new(deck, options, rawget(_G, "__kit_presentation"))
  _G.__kit_presentation = self; _G.model = self.model
  if flags["presentation-capture"] then
    self:jump(flags["presentation-capture"])
    self.model.step = math.min(tonumber(flags["presentation-step"]) or 0, self.counts[self.model.key] or 0)
  end
  _G.init = function() return self.model end
  _G.update = function(msg) return self:update(msg) end
  _G.view = function(env) return self:view(env) end
  _G.on_exit = function() self:dispose() end
  _G.__kit_export_plan = function(all_steps) return kui.json.encode(self:export_plan(all_steps)) end
  if not options.snapshot then kui.dispatch(message("start")) end
  return self
end

return setmetatable(M, { __call = function(_, ...) return M.present(...) end })
