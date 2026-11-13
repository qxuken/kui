local kit = require("kit.ui")

function init()
  return { name = "world", checked = true }
end

function update(message)
  if message.kind == "name" then
    model.name = message.change.text
  elseif message.kind == "toggle" then
    model.checked = message.toggle.checked
  end
end

function view(env)
  return column {
    width = "grow", height = "grow", pad = 24, gap = 16, bg = "$kitBg",
    text("Hello, " .. model.name, { size = 26, color = env.theme.fg }),
    kit.text_field({ label = "Your name", value = model.name,
      on_change = { kind = "name" } }),
    kit.checkbox({ label = "Keep exploring", checked = model.checked,
      on_toggle = { kind = "toggle" } }),
  }
end
