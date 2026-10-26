-- The Kit app: every Kit component, live, on the kui runner — and the
-- runner's own kit beside it: a worker building 100,000 rows as a
-- channel, timers feeding a live chart and a log, and an HTTP server
-- serving the app's state.
--
--   kui run addons/kit/app            (from the repository root)
--   kui run --watch addons/kit/app    edit a card under demos/ and watch it reload
--
-- This file is the shell: the toolbar, the sidebar, the Theme panel, the
-- status bar, and what every page shares (the worker's data, the live
-- feeds, the toasts). Each card is a file of its own under demos/.

local kit = require("kit.ui")
local pages = require("pages")
local theme = require("theme")
local toasts = require("demos.overlays.toasts")
local live = require("demos.feeds")

-- The pages, in the sidebar's order.
PAGES = pages.PAGES

-- The app's state, made once; a --watch reload keeps it.
function init()
  local m = {
    page = "overview",
    side = 0.18,
    -- The Theme panel: open or not, and what it sets.
    theme_open = false, theme_colors = false, theme = theme.defaults(),
    -- Which cards show their source, and which their guideline.
    source = {}, guide = {},
    -- The worker's data.
    rows = nil, rows_ms = nil, filtered = false, spans = nil,
    -- The HTTP server.
    serving = false, port = nil, hits = 0,
  }
  for k, v in pairs(live.state) do m[k] = v end
  pages.state(m)
  return m
end

-- A worker builds the big data; what it sends comes back to `update`.
worker = worker or kui.worker("jobs/generate.lua")

-- Timers survive a reload only once: they live in globals too.
if not feeds then
  feeds = {
    kui.timer.every(100, function() kui.dispatch({ kind = "tick" }) end),
    kui.timer.every(400, function() kui.dispatch({ kind = "log" }) end),
  }
  worker:dispatch({ kind = "rows", n = 100000 })
  worker:dispatch({ kind = "trace", n = 3000 })
end

local function serve()
  local s = kui.http.serve({ port = 3050 })
  s:get("/api/state", function()
    kui.dispatch({ kind = "hit" })
    return { skin = model.theme.skin, page = model.page, rows = model.rows and #model.rows or 0, t = model.t }
  end)
  s:get("/", function()
    return { html = "<h1>Kit app</h1><p>Try <a href='/api/state'>/api/state</a>.</p>" }
  end)
  server = s
  return s.port
end

-- Whether a text editor had the keyboard at the last frame: it keeps
-- ⌥↑ and ⌥↓ (they bubble from it, as every key it does not use does).
local editing = false
-- Whether ⌥↑ or ⌥↓ changed the page since the last frame: the control
-- that had the keyboard went with the page, and the key sink takes it.
local paged = false

function update(msg)
  local m = model
  local k = msg.kind
  if k == "tick" then live.tick(m)
  elseif k == "log" then live.log_line(m)
  elseif k == "page" then m.page = msg.change.value
  elseif k == "keys" then
    -- ⌥↑ the previous page, ⌥↓ the next: the key sink around the app hears
    -- what a focused control leaves.
    local key = msg.key
    if key and key.phase == "down" and key.alt and not (key.ctrl or key.super or key.shift) and not editing
      and (key.code == "up" or key.code == "down") then
      m.page = pages.step(m.page, key.code == "up" and -1 or 1)
      paged = true
    elseif key and key.phase == "down" and key.code == "tab" and not (key.ctrl or key.alt or key.super) then
      -- The sink hears Tab only while it holds the focus itself — a press
      -- on the page's background, or on a control inside it, gives it the
      -- focus — and Tab is the ring's: handed on from where the press was
      -- (ADR 0011, decision 4).
      local back = key.shift
      kui.next_frame(function(env) if back then env.focus_prev() else env.focus_next() end end)
    end
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "live" then m.live = msg.toggle.on
  elseif k == "source" then m.source[msg.name] = not m.source[msg.name] or nil; m.guide[msg.name] = nil
  elseif k == "guide" then m.guide[msg.name] = not m.guide[msg.name] or nil; m.source[msg.name] = nil
  elseif k == "rows" then m.rows, m.rows_ms, m.filtered = msg.rows, msg.ms, msg.filtered
  elseif k == "trace" then m.spans = msg.spans
  elseif k == "press" then toasts.push(m, { message = "Pressed " .. msg.what })
  elseif k == "worker_exit" then
    toasts.push(m, { message = "The data worker stopped: " .. tostring(msg.error), tone = "danger", duration = 0 })
  elseif k == "serve" then
    if msg.toggle.on and not m.serving then
      local ok, port = pcall(serve)
      if ok then m.serving, m.port = true, port
      else toasts.push(m, { message = tostring(port), tone = "danger", duration = 0 }) end
    elseif not msg.toggle.on and m.serving then
      server:stop(); m.serving = false
    end
  elseif k == "hit" then m.hits = m.hits + 1
  end
  theme.update(m, msg)
  -- And the cards: each answers the messages its own controls send.
  pages.update(m, msg)
end

function view(env)
  local m = model
  -- The app's own `dark` / `light` rules (the Conditions card) read the
  -- appearance the Theme panel holds Kit to, as Kit's do: each origin
  -- says its own.
  local held = m.theme.appearance
  env.set_rules_appearance(held ~= "system" and held or nil)
  editing = env.focus ~= nil and env.edit_text(env.focus) ~= nil
  local refocus = paged
  paged = false
  local page = column { key = "page." .. m.page, width = "grow", height = "grow", scroll_y = true, pad = 16, gap = 16,
    pages.page(m, m.page, env) }
  -- The whole app inside one key sink: the page shortcut. Its keys are
  -- the ones nothing focused inside it uses, so it takes the keyboard
  -- itself whenever nothing has it — at launch, and when the focused
  -- control went with the page ⌥↓ left — and hands Tab on (`update`).
  -- It is no Tab stop: a sink around stops is a keymap over them.
  return column { width = "grow", height = "grow", bg = "$kitBg", on_key = { kind = "keys" },
    key_focus = (env.focus == nil or refocus) or nil,
    kit.settings(theme.settings(m.theme)),
    kit.toolbar({}, {
      row { gap = 8, cross_align = "center",
        text("Kit", { size = 15, bold = true, family = "$kitUi", color = "$kitFg" }),
        kit.badge({ label = "v" .. kui.version, tone = "neutral" }),
      },
      trailing = {
        kit.switch({ label = "Live", on = m.live, on_toggle = { kind = "live" } }),
        kit.button({ label = "Theme", icon = "sliders", pressed = m.theme_open, name = "Theme settings",
          on_press = { kind = "theme_open" } }),
      },
    }),
    kit.split({ fraction = m.side, min = 150, on_resize = { kind = "side" } }, {
      first = {
        column { width = "grow", height = "grow", bg = "$kitSide", pad = 8, gap = 2,
          pages.nav(m),
          column { height = "grow" },
          text(pages.shortcut(env.platform), { size = 11, family = "$kitUi", color = "$kitMuted", max_lines = 1, ellipsis = true }),
        },
      },
      second = {
        row { width = "grow", height = "grow",
          page,
          -- The Theme panel beside the page, so every card is in view
          -- while it changes.
          m.theme_open and column { width = "$kitRuleW", height = "grow", bg = "$kitInk" } or nil,
          m.theme_open and column { key = "theme", width = 320, height = "grow", scroll_y = true, pad = 16,
            bg = "$kitSide", theme.view(m, env) } or nil,
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
    toasts.toaster(m),
  }
end
