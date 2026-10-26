-- Badges and progress: badges carry a tooltip, `fade` brings one in and
-- out, a progress bar springs to its value. `model.t` is demos/feeds.lua's clock
-- for the live feeds, so the upload bar moves while they run.

-- Source also: demos/feeds.lua

local kit = require("kit.ui")

local demo = {
  title = "Badges and progress",
  note = "badges carry a tooltip; `fade` brings one in and out; a progress bar springs to its value",
  call = 'kit.badge({ label = "200", tone = "ok", dot = true, tooltip = "OK" })\nkit.badge({ label = "changed", tone = "accent", fade = true })\nkit.progress({ label = "Uploading", value = model.upload })',
  state = { changed = false },
}

function demo.update(m, msg)
  if msg.kind == "changed" then m.changed = msg.toggle.on end
end

function demo.view(m)
  return row { gap = 8, cross_align = "center", wrap_children = true, cross_gap = 8,
      kit.badge({ label = "200", tone = "ok", dot = true, tooltip = "OK: the request succeeded" }),
      kit.badge({ label = "304", tooltip = "Not modified" }),
      kit.badge({ label = "slow", tone = "warn" }),
      kit.badge({ label = "500", tone = "danger", dot = true }),
      kit.badge({ label = "new", tone = "accent" }),
      kit.badge({ label = "info", tone = "info" }),
      m.changed and kit.badge({ label = "changed", tone = "accent", fade = true }) or nil },
    kit.switch({ label = "Changed", on = m.changed, on_toggle = { kind = "changed" } }),
    kit.progress({ label = "Uploading", value = (math.floor(m.t) % 5) / 4 }),
    kit.progress({ label = "Under way" })
end

return demo
