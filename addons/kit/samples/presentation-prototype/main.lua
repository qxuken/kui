-- THROWAWAY UI SPIKE: which hierarchy, density and authoring feel fits Kit?
-- Four native presets with independently selectable themes and layouts.
-- Native equivalent of ?variant=: -- --variant paper|signal|stage|minimal.
-- This deliberately uses today's Kit + core; it is not the future public API.
local kit = require("kit.ui")

local variants = {
  { key = "paper", name = "Paper / editorial", skin = "neutral", appearance = "light",
    bg = 0xf2efe7ff, fg = 0x252722ff, muted = 0x76786dff, accent = 0x3c6048ff,
    panel = 0xe9e6dcff, rule = 0xcecdbfff, font = "Georgia" },
  { key = "signal", name = "Signal / control room", skin = "phosphor", appearance = "dark",
    bg = 0x101814ff, fg = 0xe1ebdbff, muted = 0x8a9b8dff, accent = 0xc4f279ff,
    panel = 0x19251dff, rule = 0x354739ff, font = "Helvetica Neue" },
  { key = "stage", name = "Stage / bold composition", skin = "neutral", appearance = "dark",
    bg = 0x171616ff, fg = 0xf6f0e8ff, muted = 0xa99f96ff, accent = 0xff8054ff,
    panel = 0x252221ff, rule = 0x443a35ff, font = "Helvetica Neue" },
  { key = "minimal", name = "Minimal / quiet composition", skin = "neutral", appearance = "light",
    bg = 0xffffffff, fg = 0x25272aff, muted = 0x74777cff, accent = 0x315ccaff,
    panel = 0xffffffff, rule = 0xe4e5e7ff, font = "Helvetica Neue" },
}
local layout_names = { "editorial", "briefing", "split", "minimal" }

local slides = {
  { title = "Make ideas executable.", steps = 2,
    notes = "Start with the idea: a slide can be a living view, not a screenshot.\n\nAdvance to reveal the argument. Try the same content in each direction." },
  { title = "Let the evidence speak.", steps = 2,
    notes = "The data is identical in every composition.\n\nHover the real Kit chart. Does it remain legible from across a room? Reveal the interpretation after the audience sees the evidence." },
  { title = "Show it. Change it.", steps = 1,
    notes = "Move the concurrency slider or press Run. The output changes in the same view.\n\nThen navigate away and return: the small demo keeps its state. These controls use Kit as it exists today." },
}

local args = {}
for i, arg in ipairs(kui.args) do
  if arg:sub(1, 2) == "--" then args[arg:sub(3)] = kui.args[i + 1] or "true" end
end

function init()
  local variant = 1
  for i, v in ipairs(variants) do if args.variant == v.key then variant = i end end
  local theme, layout = variant, variant
  for i, v in ipairs(variants) do
    if args.theme == v.key then theme = i end
    if args.layout == v.key or args.layout == layout_names[i] then layout = i end
  end
  local slide = math.max(1, math.min(3, tonumber(args.slide) or 1))
  return { variant = variant, theme = theme, layout = layout, chrome = args.clean == nil, slide = slide,
    step = math.max(0, math.min(slides[slide].steps, tonumber(args.step) or 0)), notes = args.presenter ~= nil,
    overview = false, workers = 4, runs = 0 }
end

local function state()
  print(string.format("prototype theme=%s layout=%s slide=%d step=%d workers=%d runs=%d notes=%s overview=%s",
    variants[model.theme or model.variant].key, layout_names[model.layout or model.variant],
    model.slide, model.step, model.workers, model.runs,
    tostring(model.notes), tostring(model.overview)))
end

local function move(direction)
  if direction > 0 then
    if model.step < slides[model.slide].steps then model.step = model.step + 1
    elseif model.slide < #slides then model.slide, model.step = model.slide + 1, 0 end
  elseif model.step > 0 then model.step = model.step - 1
  elseif model.slide > 1 then model.slide = model.slide - 1; model.step = slides[model.slide].steps end
end

function update(msg)
  if msg.kind == "next" then move(1)
  elseif msg.kind == "previous" then move(-1)
  elseif msg.kind == "variant" then model.variant, model.theme, model.layout = msg.index, msg.index, msg.index
  elseif msg.kind == "cycle" then
    model.variant = (model.variant - 1 + msg.direction) % #variants + 1
    model.theme, model.layout = model.variant, model.variant
  elseif msg.kind == "theme" then model.theme = (model.theme or model.variant) % #variants + 1
  elseif msg.kind == "layout" then model.layout = (model.layout or model.variant) % #variants + 1
  elseif msg.kind == "pick" then model.slide, model.step, model.overview = msg.slide, 0, false
  elseif msg.kind == "notes" then model.notes = not model.notes
  elseif msg.kind == "overview" then model.overview = not model.overview
  elseif msg.kind == "workers" then model.workers = math.floor(msg.change.value)
  elseif msg.kind == "run" then model.runs = model.runs + 1
  elseif msg.kind == "keys" and msg.key and msg.key.phase == "down" then
    local k = msg.key
    if k.ctrl or k.super or k.alt then return end
    if k.code == "right" or k.code == "pagedown" or k.code == "space" then move(k.shift and -1 or 1)
    elseif k.code == "left" or k.code == "pageup" then move(-1)
    elseif k.code:match("^[1-4]$") then
      model.variant = tonumber(k.code); model.theme, model.layout = model.variant, model.variant
    elseif k.code:lower() == "t" then model.theme = (model.theme or model.variant) % #variants + 1
    elseif k.code:lower() == "l" then model.layout = (model.layout or model.variant) % #variants + 1
    elseif k.code:lower() == "h" then model.chrome = model.chrome == false
    elseif k.code:lower() == "p" then model.notes = not model.notes
    elseif k.code:lower() == "o" then model.overview = not model.overview
    elseif k.code == "escape" then model.overview, model.notes = false, false
    elseif k.code == "tab" then
      kui.next_frame(function(env) if k.shift then env.focus_prev() else env.focus_next() end end)
    else return end
  else return end
  state()
end

local function words(s, size, color, extra)
  local p = { size = size, color = color, family = "Helvetica Neue", wrap = "word" }
  for k, value in pairs(extra or {}) do p[k] = value end
  return text(s, p)
end
local function mono(s, size, color)
  return words(s, size, color, { family = "Menlo" })
end
local function title(s, v, size)
  return words(s, size, v.fg, { family = v.font, bold = v.key ~= "paper" and v.key ~= "minimal", line_height = size * 1.06 })
end
local function rule(v) return column { width = "grow", height = 1, bg = v.rule } end
local function space() return column { height = "grow", min_height = 12 } end
local function reveal(step, v, s, size)
  -- Text-only invisible placeholder: keeps geometry without invisible controls.
  -- Production needs the general input/accessibility-safe visibility contract.
  return column { width = "grow", opacity = model.step >= step and 1 or 0,
    transition = 200, when = { { reduced_motion = true, transition = 0 } },
    words(s, size or 22, v.fg),
  }
end

local data = channel({ minute = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11 },
  baseline = { 86, 90, 89, 96, 95, 104, 101, 112, 107, 115, 112, 119 },
  improved = { 86, 87, 82, 80, 78, 77, 75, 74, 72, 71, 70, 69 } })

local function chart(v, h)
  return column { width = "grow", pad = 24, gap = 16, bg = v.panel,
    row { width = "grow", gap = 12, mono("P95 LATENCY", 12, v.muted), column { width = "grow" },
      mono("12 MINUTES / ms", 12, v.muted) },
    kit.line_chart({ key = "evidence", data = data, x = "minute", height = h or 230,
      focusable = true, y_unit = "ms", series = {
        { field = "baseline", name = "Before", color = v.muted },
        { field = "improved", name = "After", color = v.accent },
      } }),
    words("Same workload. Fewer waits.", 18, v.fg),
  }
end

local function topology(v)
  return column { width = "grow", pad = 26, gap = 22, bg = v.panel,
    mono("ONE VIEW / THREE POSSIBILITIES", 11, v.muted),
    column { width = "grow", pad = 22, border = { w = 1, color = v.rule }, gap = 9,
      mono("deck.lua", 13, v.accent),
      words("An idea, expressed as code.", 23, v.fg, { family = v.font }),
    },
    mono("                 ↓", 20, v.muted),
    row { width = "grow", gap = 10, min_column = 100,
      column { width = "grow", gap = 8, words("Compose", 18, v.fg), mono("native views", 11, v.muted) },
      column { width = "grow", gap = 8, words("Explore", 18, v.fg), mono("live controls", 11, v.muted) },
      column { width = "grow", gap = 8, words("Present", 18, v.fg), mono("one story", 11, v.muted) },
    },
    rule(v),
    reveal(1, v, "The demo belongs inside the story.", 23),
  }
end

local function code(v, compact)
  local rows = {
    { "local", " kit = require(", '"kit.ui"', ")" },
    { "", "", "", "" },
    { "return", " slide {", "", "" },
    { "", "  key = ", '"live-demo"', "," },
    { "", "  view = ", "function", "(ctx)" },
    { "", "    ", "-- compose a real interface", "" },
    { "", "    ", "return", " chart(ctx.state)" },
    { "", "  ", "end", "," },
    { "", "}", "", "" },
  }
  local body = { width = "grow", gap = compact and 8 or 14, pad = 24, bg = v.panel }
  body[#body + 1] = row { width = "grow", mono("live-demo.lua", 12, v.muted), column { width = "grow" },
    mono("LUA", 11, v.accent) }
  body[#body + 1] = rule(v)
  for i, line in ipairs(rows) do
    body[#body + 1] = row { width = "grow", gap = 14,
      mono(string.format("%02d", i), 12, v.muted),
      text({ { line[1], color = v.accent }, line[2], { line[3], color = v.accent }, line[4] },
        { size = compact and 15 or 18, family = "Menlo", color = v.fg }),
    }
  end
  return column(body)
end

local function demo(v)
  local throughput = math.floor(model.workers * 138 / (1 + model.workers * 0.035))
  return column { width = "grow", gap = 20, pad = 24, bg = v.panel,
    row { width = "grow", mono("LIVE WORKLOAD", 12, v.muted), column { width = "grow" },
      kit.badge({ label = "interactive", tone = "accent", dot = true }) },
    row { gap = 12, cross_align = "baseline", title(tostring(throughput), v, 68), words("jobs/s", 20, v.muted) },
    kit.slider({ key = "workers", label = "Concurrency", value = model.workers, min = 1, max = 16, step = 1,
      value_label = tostring(model.workers) .. " workers", on_change = { kind = "workers" } }),
    row { width = "grow", gap = 12, cross_align = "center",
      kit.button({ label = "Run workload", variant = "primary", on_press = { kind = "run" } }),
      words(model.runs == 0 and "Ready when you are." or string.format("Run %02d complete.", model.runs), 15, v.muted),
    },
    kit.code({ text = string.format("workers=%d  throughput=%d jobs/s\ncompleted_runs=%d", model.workers, throughput, model.runs),
      label = "Output", copy = true }),
  }
end

-- A: an editorial spread. Quiet masthead, generous page margins, reading order.
local function paper(v, narrow)
  local content = { key = "paper." .. model.slide, width = "grow", height = "grow", gap = 26, pad = narrow and 28 or 56 }
  content[#content + 1] = row { width = "grow", cross_align = "baseline", words("Fieldnotes", 24, v.fg, { family = v.font }),
    column { width = "grow" }, mono("KUI / A LIVING DECK", 11, v.muted) }
  content[#content + 1] = rule(v)
  local left = { width = "grow", gap = 24, mono(string.format("0%d / THE %s", model.slide,
    ({ "IDEA", "EVIDENCE", "EXPERIMENT" })[model.slide]), 12, v.accent),
    title(slides[model.slide].title, v, narrow and 44 or 67) }
  local right
  if model.slide == 1 then
    left[#left + 1] = words("A presentation can be a place to think.\nNot just a place to put the answer.", 23, v.muted)
    left[#left + 1] = reveal(2, v, "Write once. Explain, explore, present.", 23)
    right = topology(v)
  elseif model.slide == 2 then
    left[#left + 1] = words("42%", 82, v.accent, { family = v.font })
    left[#left + 1] = words("lower tail latency", 22, v.muted)
    left[#left + 1] = reveal(1, v, "Change the architecture, not the workload.", 22)
    left[#left + 1] = reveal(2, v, "Let the audience inspect the evidence.", 22)
    right = chart(v, narrow and 180 or 250)
  else
    left[#left + 1] = words("Use the interface to make the argument.", 24, v.muted)
    left[#left + 1] = code(v, true)
    right = column { width = "grow", gap = 18, demo(v), reveal(1, v, "A live view. An honest explanation.", 22) }
  end
  content[#content + 1] = space()
  content[#content + 1] = row { width = "grow", gap = 44, cross_align = "center",
    when = { { width = { below = 820 }, dir = "column" } }, column(left),
    column { width = "grow", right } }
  content[#content + 1] = space()
  content[#content + 1] = rule(v)
  content[#content + 1] = row { width = "grow", words("Ideas worth opening up.", 14, v.muted),
    column { width = "grow" }, mono(string.format("%02d — 03", model.slide), 12, v.muted) }
  return column(content)
end

-- B: a control-room briefing. Chapter rail, data first, visible system state.
local function signal(v, narrow)
  local rail = { width = 190, height = "grow", pad = { x = 22, y = 30 }, gap = 20, bg = v.panel,
    mono("K / SIGNAL", 20, v.accent), mono("NATIVE BRIEFING\nVOL. 001", 10, v.muted), rule(v) }
  for i, label in ipairs({ "01  THESIS", "02  EVIDENCE", "03  LIVE DEMO" }) do
    rail[#rail + 1] = column { width = "grow", pad = { x = 8, y = 12 }, bg = model.slide == i and v.rule or nil,
      cursor = "pointer", on_click = { kind = "pick", slide = i }, mono(label, 12, model.slide == i and v.accent or v.muted) }
  end
  rail[#rail + 1] = space()
  rail[#rail + 1] = mono("● ALL SYSTEMS READY", 10, v.accent)
  local body = { key = "signal." .. model.slide, width = "grow", height = "grow", pad = narrow and 24 or 40, gap = 24,
    row { width = "grow", mono("BRIEF / PRESENTATION AS CODE", 11, v.muted), column { width = "grow" },
      kit.badge({ label = "LIVE", tone = "accent", dot = true }) },
    title(slides[model.slide].title, v, narrow and 42 or 64),
  }
  if model.slide == 1 then
    body[#body + 1] = words("The story and the system, in the same runtime.", 24, v.muted)
    body[#body + 1] = row { width = "grow", gap = 20, when = { { width = { below = 720 }, dir = "column" } },
      column { width = "grow", topology(v) }, column { width = "grow", code(v, true) } }
    body[#body + 1] = reveal(2, v, "Every claim can have a working example.", 24)
  elseif model.slide == 2 then
    body[#body + 1] = chart(v, 240)
    body[#body + 1] = row { width = "grow", gap = 24, min_column = 210,
      column { width = "grow", gap = 8, title("−42%", v, 48), words("tail latency", 18, v.muted) },
      column { width = "grow", reveal(1, v, "One change.\nAn observable result.", 22) },
      column { width = "grow", reveal(2, v, "Inspect the chart.\nChallenge the claim.", 22) } }
  else
    body[#body + 1] = row { width = "grow", gap = 24, when = { { width = { below = 720 }, dir = "column" } },
      column { width = "grow", code(v, false) }, column { width = "grow", demo(v) } }
    body[#body + 1] = reveal(1, v, "Inputs and output belong in one conversation.", 24)
  end
  body[#body + 1] = space()
  body[#body + 1] = rule(v)
  body[#body + 1] = mono(string.format("SLIDE %02d/03     REVEAL %d/%d     SESSION / LOCAL",
    model.slide, model.step, slides[model.slide].steps), 11, v.muted)
  return row { width = "grow", height = "grow", not narrow and column(rail) or nil, column(body) }
end

-- C: an asymmetric stage. Large type on one side, a single visual on the other.
local function stage(v, narrow)
  local hero = { key = "hero", width = "grow", height = narrow and nil or "grow", gap = 24, pad = narrow and 28 or 48,
    mono("KUI PRESENTS / 001", 12, v.accent), space(),
    title(slides[model.slide].title, v, narrow and 48 or 84),
  }
  local visual
  if model.slide == 1 then
    hero[#hero + 1] = words("Less telling.\nMore showing.", 29, v.muted)
    hero[#hero + 1] = reveal(2, v, "Make the argument tangible.", 24)
    visual = column { width = "grow", height = "grow", bg = v.accent, pad = 40, gap = 28,
      mono("THIS IS A WORKING VIEW.", 12, v.bg), space(),
      words("LIVE.", narrow and 100 or 144, v.bg, { bold = true, line_height = narrow and 105 or 150 }),
      column { width = "grow", height = 2, bg = v.bg },
      words("Not an image of an idea.\nThe idea, running.", 29, v.bg), space(),
      column { opacity = model.step >= 1 and 1 or 0, transition = 200,
        words("One source. Every surface.", 21, v.bg) },
    }
  elseif model.slide == 2 then
    hero[#hero + 1] = title("−42%", v, narrow and 88 or 116)
    hero[#hero + 1] = reveal(1, v, "Fewer waits.\nMore possibility.", 28)
    visual = column { width = "grow", height = "grow", bg = v.panel, pad = 24, gap = 24,
      space(), chart(v, 270), reveal(2, v, "The evidence stays interactive.", 23), space() }
  else
    hero[#hero + 1] = reveal(1, v, "Leave room for\na real experiment.", 29)
    visual = column { width = "grow", height = "grow", bg = v.panel, pad = 28, gap = 24,
      space(), demo(v), code(v, true), space() }
  end
  hero[#hero + 1] = space()
  hero[#hero + 1] = mono(string.format("%02d / 03           IDEAS IN MOTION", model.slide), 12, v.muted)
  return row { key = "stage." .. model.slide, width = "grow", height = "grow",
    when = { { width = { below = 900 }, dir = "column" } }, column(hero), visual }
end

-- D: a quiet document. One reading column, plain type, no decorative panels.
local function minimal(v, narrow)
  local body = { key = "minimal." .. model.slide, width = "grow", max_width = 1120,
    height = "grow", pad = { x = narrow and 28 or 64, y = narrow and 28 or 48 }, gap = 24,
    row { width = "grow", words("kui", 16, v.fg), column { width = "grow" },
      words(string.format("%d / %d", model.slide, #slides), 14, v.muted) },
    space(),
    title(slides[model.slide].title, v, narrow and 40 or 56),
  }
  if model.slide == 1 then
    body[#body + 1] = words("A presentation can be a place to think.\nNot just a place to put the answer.", 24, v.muted,
      { line_height = 35 })
    body[#body + 1] = column { height = 16 }
    body[#body + 1] = reveal(1, v, "Compose a story with ordinary interface components.", 22)
    body[#body + 1] = reveal(2, v, "Let the audience explore the idea.", 22)
  elseif model.slide == 2 then
    body[#body + 1] = chart(v, narrow and 170 or 220)
    body[#body + 1] = reveal(1, v, "42% lower tail latency, with the same workload.", 23)
    body[#body + 1] = reveal(2, v, "Inspect the evidence before explaining the result.", 20)
  else
    body[#body + 1] = words("Change a value. Observe the result.", 23, v.muted)
    body[#body + 1] = row { width = "grow", gap = 24,
      when = { { width = { below = 720 }, dir = "column" } },
      column { width = "grow", code(v, true) }, column { width = "grow", demo(v) } }
    body[#body + 1] = reveal(1, v, "The explanation and the experiment stay together.", 21)
  end
  body[#body + 1] = space()
  body[#body + 1] = words("A living deck.", 14, v.muted)
  return column { width = "grow", height = "grow", cross_align = "center", column(body) }
end

local function overview(v)
  local t = { width = "grow", height = "grow", pad = 40, gap = 26,
    title("The story, at a glance.", v, 44), words("Choose a chapter. Each starts at its first reveal.", 20, v.muted) }
  for i, slide in ipairs(slides) do
    t[#t + 1] = row { width = "grow", pad = 24, gap = 24, bg = model.slide == i and v.panel or nil,
      border = { w = 1, color = v.rule }, cursor = "pointer", on_click = { kind = "pick", slide = i },
      mono("0" .. i, 22, v.accent), title(slide.title, v, 30), column { width = "grow" },
      mono(slide.steps .. " REVEALS  ↗", 12, v.muted) }
  end
  return column(t)
end

local function notes(v)
  return column { width = 290, height = "grow", bg = v.panel, pad = 24, gap = 24,
    mono("PRESENTER / NOTES STUDY", 11, v.accent),
    title(slides[model.slide].title, v, 26),
    words(slides[model.slide].notes, 17, v.fg), rule(v),
    mono("UP NEXT", 11, v.muted),
    words(slides[math.min(model.slide + 1, #slides)].title, 22, v.fg),
    space(),
    kit.button({ label = "Next reveal →", variant = "primary", on_press = { kind = "next" } }),
    kit.button({ label = "Hide notes", on_press = { kind = "notes" } }),
  }
end

local renderers = { paper, signal, stage, minimal }
function view(env)
  local theme_index, layout_index = model.theme or model.variant, model.layout or model.variant
  local v = variants[theme_index]
  local narrow = env.viewport_w - (model.notes and 290 or 0) < 1000
  local bar = row { key = "prototype-controls", width = "grow", max_width = 1120, min_height = 60,
    float = { anchor = "viewport", at = { "center", "end" }, self = { "center", "end" }, dy = -42 },
    bg = 0x25262aff, radius = 12, pad = { x = 12, y = 10 }, gap = 8, cross_align = "center", wrap_children = true,
    kit.button({ label = "‹", name = "Previous style", on_press = { kind = "cycle", direction = -1 } }),
  }
  for i, item in ipairs(variants) do
    bar[#bar + 1] = kit.button({ label = i .. "  " .. item.key,
      variant = i == theme_index and i == layout_index and "primary" or "default",
      name = item.name, on_press = { kind = "variant", index = i } })
  end
  bar[#bar + 1] = kit.button({ label = "›", name = "Next style", on_press = { kind = "cycle", direction = 1 } })
  bar[#bar + 1] = kit.button({ label = "Theme: " .. v.key, on_press = { kind = "theme" } })
  bar[#bar + 1] = kit.button({ label = "Layout: " .. layout_names[layout_index], on_press = { kind = "layout" } })
  bar[#bar + 1] = column { width = "grow" }
  bar[#bar + 1] = kit.button({ label = "Overview", on_press = { kind = "overview" } })
  bar[#bar + 1] = kit.button({ label = "Notes", on_press = { kind = "notes" } })
  bar[#bar + 1] = kit.button({ label = "←", name = "Previous reveal", on_press = { kind = "previous" } })
  bar[#bar + 1] = mono(string.format("%d/3 · %d/%d", model.slide, model.step, slides[model.slide].steps), 11, 0xf0f0eaff)
  bar[#bar + 1] = kit.button({ label = "→", name = "Next reveal", on_press = { kind = "next" } })
  return column { width = "grow", height = "grow", bg = v.bg, on_key = { kind = "keys" },
    key_focus = env.focus == nil or nil,
    kit.settings({ skin = v.skin, appearance = v.appearance, shadows = false, radius = 5,
      roles = { accent = v.accent, bg = v.bg, panel = v.panel } }),
    row { width = "grow", height = "grow", pad = { b = model.chrome == false and 0 or 146 },
      column { width = "grow", height = "grow", scroll_y = true,
        model.overview and overview(v) or renderers[layout_index](v, narrow) },
      model.notes and notes(v) or nil,
    },
    model.chrome ~= false and bar or nil,
    model.chrome ~= false and row {
      float = { anchor = "viewport", at = { "center", "end" }, self = { "center", "end" }, dy = -14 },
      mono("SPIKE / 1–4 PRESET · T THEME · L LAYOUT · H HIDE CONTROLS · ← → REVEALS · O OVERVIEW · P NOTES", 10, v.muted) } or nil,
  }
end
