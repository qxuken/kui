-- Buttons: four variants, an icon, a shortcut, and icon buttons. `press`
-- is the app's own message: main.lua says in a toast what was pressed.

local kit = require("kit.ui")

local demo = {
  title = "Buttons",
  note = "four variants, an icon, a shortcut",
  call = 'kit.button({ label = "Run", variant = "primary", icon = "play", on_press = { kind = "run" } })\nkit.icon_button({ icon = "refresh", label = "Reload", on_press = ... })',
}

function demo.view(m)
  return row { gap = 8, cross_align = "center", wrap_children = true, cross_gap = 8,
    kit.button({ label = "Run", variant = "primary", icon = "play", on_press = { kind = "press", what = "Run" } }),
    kit.button({ label = "Save", shortcut = "⌘S", on_press = { kind = "press", what = "Save" } }),
    kit.button({ label = "Disabled", disabled = true }),
    kit.icon_button({ icon = "refresh", label = "Reload", on_press = { kind = "press", what = "Reload" } }),
    kit.icon_button({ icon = "sliders", label = "Settings" }),
    kit.icon_button({ icon = "trash", label = "Delete", variant = "danger", on_press = { kind = "press", what = "Delete" } }) }
end

return demo
