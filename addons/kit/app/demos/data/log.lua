-- A log over a channel: main.lua publishes a line every 400 ms as
-- `model.log` while the Live switch is on, and `model.follow` is the
-- Choices card's "Follow the log".

local kit = require("kit.ui")

local demo = {
  title = "Log",
  note = "a channel published every 400 ms, following its end",
  call = 'kit.log({ rows = model.log, follow = model.follow, grow = true })',
}

function demo.view(m)
  return column { width = "grow", height = 180,
    m.log and kit.log({ rows = m.log, follow = m.follow, grow = true }) or kit.progress({}) }
end

return demo
