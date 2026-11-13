local kit = require("kit.ui")

function init()
  return { count = 0 }
end

function update(message)
  if message.kind == "increment" then
    model.count = model.count + 1
  end
end

function view(env)
  return column {
    width = "grow", height = "grow", pad = 24, gap = 16, bg = "$kitBg",
    text("Hello from Lua", { size = 26, color = env.theme.fg }),
    text("Change this source and press Run.", { color = env.theme.muted }),
    kit.stat({ label = "Count", value = tostring(model.count) }),
    kit.button({ label = "Increment", variant = "primary",
      on_press = { kind = "increment" } }),
  }
end
