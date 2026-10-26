-- A card that loads: skeletons in the shape of what is coming, then the
-- content in their place. The load is a timer standing in for a request
-- (kui.http.get would answer the same way, by a message).

local kit = require("kit.ui")

local demo = {
  title = "Loading a card",
  note = "skeletons in the shape of the content, swapped for it when it arrives",
  call = 'if model.profile == "loading" then\n  kit.skeleton({ shape = "circle", width = 48 }), kit.skeleton({ lines = 3 })\nelse\n  ...the content...\nend',
  state = { profile = "loading" },
}

-- What a request would take, in ms.
local LOAD_MS = 1600

local function load()
  kui.timer.after(LOAD_MS, function() kui.dispatch({ kind = "profile_loaded" }) end)
end

-- The first load starts with the app.
load()

function demo.update(m, msg)
  if msg.kind == "profile_reload" then
    m.profile = "loading"
    load()
  elseif msg.kind == "profile_loaded" then
    m.profile = "loaded"
  end
end

local function loading()
  return row { width = "grow", gap = 12,
    kit.skeleton({ shape = "circle", width = 48, label = "Loading the avatar" }),
    column { width = "grow", gap = 10,
      kit.skeleton({ lines = 1, width = 160, label = "Loading the name" }),
      kit.skeleton({ lines = 3, label = "Loading the bio" }),
      row { gap = 12,
        kit.skeleton({ shape = "block", width = 96, height = 44, label = "Loading a count" }),
        kit.skeleton({ shape = "block", width = 96, height = 44, label = "Loading a count" }) } } }
end

local function loaded()
  return row { width = "grow", gap = 12,
    column { width = 48, height = 48, radius = 24, bg = "$kitAccent", main_align = "center", cross_align = "center",
      text("AL", { size = 16, bold = true, family = "$kitUi", color = "$kitOnAccent" }) },
    column { width = "grow", gap = 10,
      text("Ada Lovelace", { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = "$kitFg" }),
      text("Wrote the first program for a machine that was never finished, and saw that it could work on more than numbers.",
        { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" }),
      row { gap = 12,
        kit.stat({ label = "Notes", value = "7" }),
        kit.stat({ label = "Pages", value = "65" }) } } }
end

function demo.view(m)
  return m.profile == "loading" and loading() or loaded(),
    row { gap = 8,
      kit.button({ label = "Load again", icon = "refresh", disabled = m.profile == "loading",
        on_press = { kind = "profile_reload" } }) }
end

return demo
