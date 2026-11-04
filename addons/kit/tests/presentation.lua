local presentation = require("kit.presentation")
local function slide(key, steps)
  return { key = key, view = function(c)
    local children = {}
    for i = 1, steps do children[i] = c.reveal(i, { text("step " .. i) }) end
    return column(children)
  end }
end
local p = presentation.new({ slides = { slide("a", 2), slide("b", 1), slide("c", 0) } })
local trace = kui.json.decode(kui.fs.read("presentation-navigation.json"))
for _, row in ipairs(trace) do
  if row.action == "jump" then p:jump(row.slide)
  elseif row.action == "start" then p:pace()
  else p[row.action](p) end
  assert(p.model.key == row.key and p.model.step == row.step, row.action)
end
p:dispose()

local browsing = presentation.new({ slides = { slide("a", 3), slide("b", 2), slide("c", 0) } })
browsing:update({ presentation = "keys", key = { phase = "down", code = "right" } })
browsing:update({ presentation = "keys", key = { phase = "down", code = "right", ["repeat"] = true } })
assert(browsing.model.step == 2, "held navigation arrows repeat")
browsing:update({ presentation = "live-sync", toggle = { on = false } })
local generation = browsing.generation
browsing:update({ presentation = "jump", slide = "b", source = "presenter" })
browsing:update({ presentation = "next", source = "presenter" })
assert(browsing.model.key == "a" and browsing.model.step == 2, "private browsing leaves the audience alone")
assert(browsing.model.preview.key == "b" and browsing.model.preview.step == 1)
assert(browsing.generation == generation, "private browsing does not stop or activate live work")
browsing:update({ presentation = "present-slide" })
assert(browsing.model.key == "b" and browsing.model.step == 1)
browsing:update({ presentation = "previous", source = "presenter" })
browsing:update({ presentation = "next", source = "audience" })
assert(browsing.model.step == 2 and browsing.model.preview.step == 0)
local reloaded = presentation.new({ slides = { slide("c", 0), slide("b", 2), slide("a", 3) } }, {}, browsing)
assert(not reloaded.model.live_sync and reloaded.model.preview.key == "b" and reloaded.model.preview.step == 0)
reloaded:update({ presentation = "live-sync", toggle = { on = true } })
assert(reloaded.model.key == "b" and reloaded.model.step == 0)
reloaded:update({ presentation = "next", source = "presenter" })
assert(reloaded.model.step == 1 and reloaded.model.preview.step == 1)
reloaded:dispose()
local removed = presentation.new({ slides = { slide("a", 0), slide("b", 0), slide("c", 0), slide("d", 0) } })
removed:update({ presentation = "live-sync", toggle = { on = false } }); removed:jump("d", "presenter")
removed = presentation.new({ slides = { slide("a", 0), slide("b", 0), slide("c", 0) } }, {}, removed)
assert(removed.model.key == "a" and removed.model.preview.key == "c" and removed.model.preview.step == 0, "removed private slide selects nearest surviving index")
removed:dispose()

local independent = presentation.new({ theme = "paper", slides = { slide("a", 0) } })
assert(independent.model.layout == "minimal")
independent:update({ presentation = "theme", change = { value = "stage" } })
assert(independent.model.layout == "minimal" and independent.model.theme == "paper", "theme is authored, not a live presenter action")
independent:dispose()
local paired = presentation.new({ preset = "paper", theme = "signal", slides = { slide("a", 0) } })
assert(paired.model.theme == "signal" and paired.model.layout == "editorial")
paired:dispose()

local scheduled = {}
kui.timer.every = function(_, callback)
  local job = { callback = callback }
  function job:cancel() self.canceled = true end
  scheduled[#scheduled + 1] = job
  return job
end
local task_signal
local old_context
local live = { key = "live", init = function() return { n = 0 } end,
  activate = function(c)
    old_context = old_context or c; c.every(100, { kind = "tick" })
    c.spawn(function(signal) task_signal = signal; signal.on_abort(function() signal.cleaned = true end) end)
  end,
  update = function(s, msg) s.n = msg.change and msg.change.value or s.n + 1; return s end,
  view = function(c)
    if c.mode == "snapshot" then c.state.n = 99 end
    return c.reveal(2, { text("live") })
  end }
p = presentation.new({ slides = { live, slide("other", 0) } })
assert(p.model.states.live.n == 0 and #scheduled == 0, "discovery is isolated and inactive")
p:pace(); scheduled[1].callback(); assert(p.model.states.live.n == 1)
local message = old_context.msg({ kind = "edit" }); message.change = { value = 9 }; p:update(message)
assert(p.model.states.live.n == 9, "event fields reach the user message")
p:render_slide(1, 2, "snapshot"); assert(p.model.states.live.n == 9)
p:jump("other"); assert(scheduled[1].canceled and task_signal.aborted and task_signal.cleaned)
scheduled[1].callback(); assert(p.model.states.live.n == 9, "late callback ignored")
p:jump("live"); p.model.step = 2
local replacement = { key = "live", init = live.init, update = live.update, activate = live.activate,
  view = function(c) return c.reveal(1, { text("changed") }) end }
local fresh = presentation.new({ slides = { slide("other", 0), replacement } }, {}, p)
assert(fresh.model.index == 2 and fresh.model.step == 1 and fresh.model.states.live.n == 9)
fresh:pace(); fresh:update(old_context.msg({ kind = "tick" }));
old_context.dispatch({ kind = "tick" }); assert(fresh.model.states.live.n == 9, "queued pre-reload results are stale")
local plan = fresh:export_plan(true)
assert(#plan == 3 and plan[2].key == "live" and plan[3].step == 1)
replacement.state_version = 2
fresh = presentation.new({ slides = { replacement } }, {}, fresh)
assert(fresh.model.states.live.n == 0, "state compatibility marker")
fresh:dispose()
local commands = {}
local env = { window = { id = 7 }, window_name = function() return "presenter" end, viewport_w = 1180, viewport_h = 860,
  set_window_close_hides = function(id, on) commands[#commands + 1] = { "policy", id, on } end,
  set_window_visible = function(id, on) commands[#commands + 1] = { "visible", id, on } end,
  set_window_fullscreen = function(id, on, display) commands[#commands + 1] = { "fullscreen", id, on, display } end,
  focus_window = function(id) commands[#commands + 1] = { "focus", id } end,
  display_names = function() return { "Projector" } end,
}
fresh.model.display = "0"; fresh:update({ presentation = "audience" }); fresh:view(env)
assert(commands[2][1] == "visible" and commands[2][2] == 0 and commands[2][3] == true)
assert(commands[3][1] == "fullscreen" and commands[3][2] == 0 and commands[3][4] == 0)
commands = {}
fresh:update({ presentation = "keys", key = { phase = "down", code = "p", ctrl = true, shift = true } }); fresh:view(env)
assert(commands[1][1] == "visible" and commands[1][2] == 7, "presenter recovery from either window")
local guide = fresh:render_slide(1, 0, "snapshot", nil, true)
local audience = fresh:render_slide(1, 0, "snapshot")
assert(guide[1].opacity == 0.28 and guide[2][1].value == "1", "presenter guide shows faded, numbered future content")
assert(guide.border.w == 1 and guide.border.color == "$kitAccent", "future content is boxed")
assert(audience.type == "component" and audience.props.visible == false, "audience keeps normal reveal visibility")
fresh:pace()
local generation = fresh.generation
fresh:update({ presentation = "overview" })
assert(fresh.model.presenter and fresh.model.overview and fresh.generation == generation, "private overview keeps the live audience activation")
fresh:update({ presentation = "keys", key = { phase = "down", code = "?" } })
assert(fresh.model.help and fresh.model.presenter, "shortcuts open privately")
fresh:update({ presentation = "keys", key = { phase = "down", code = "escape" } })
assert(not fresh.model.help)
fresh:update({ presentation = "menu", menu = { item = "help" } })
assert(fresh.model.help, "native menu commands route to the presenter")
print("presentation contract OK")
kui.exit(0)
