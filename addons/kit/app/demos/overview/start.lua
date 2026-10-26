-- Start an app: `kui new`, then `kui run --watch`, and the four button
-- variants. `press` is the app's own message: main.lua says in a toast
-- what was pressed.

local kit = require("kit.ui")

local demo = {
  title = "Start an app",
  note = "kui new, then kui run --watch",
  call = 'kui new hello\nkui run --watch hello\n\n-- main.lua\nlocal kit = require("kit.ui")\nfunction init() return { n = 0 } end\nfunction update(msg) if msg.kind == "add" then model.n = model.n + 1 end end\nfunction view(env)\n  return column { kit.button({ label = "Add", on_press = { kind = "add" } }),\n    text("n = " .. model.n) }\nend',
}

function demo.view()
  return row { gap = 8,
      kit.button({ label = "Primary", variant = "primary", icon = "play", on_press = { kind = "press", what = "Primary" } }),
      kit.button({ label = "Default", shortcut = "⌘D", on_press = { kind = "press", what = "Default" } }),
      kit.button({ label = "Ghost", variant = "ghost", on_press = { kind = "press", what = "Ghost" } }),
      kit.button({ label = "Delete", variant = "danger", icon = "trash", on_press = { kind = "press", what = "Delete" } }) },
    text("On Ledger the primary button's face lifts under the pointer and sinks onto its shadow while it is held.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
