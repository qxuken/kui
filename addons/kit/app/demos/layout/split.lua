-- A split: two panes and a rule between them; the fraction is the app's.

local kit = require("kit.ui")

local demo = {
  title = "Split",
  note = "drag the rule",
  call = 'kit.split({ fraction = model.split, on_resize = { kind = "split" } }, { first = {...}, second = {...} })',
  state = { split = 0.5 },
}

function demo.update(m, msg)
  if msg.kind == "split" then m.split = msg.resize.fraction end
end

function demo.view(m)
  return column { width = "grow", height = 160,
    kit.split({ fraction = m.split, on_resize = { kind = "split" } }, {
      first = { column { width = "grow", height = "grow", pad = 12, bg = "$kitSunken",
        text(string.format("first: %.0f%%", m.split * 100), { size = 12, family = "$kitData", color = "$kitFg" }) } },
      second = { column { width = "grow", height = "grow", pad = 12,
        text("second", { size = 12, family = "$kitData", color = "$kitFg" }) } },
    }) }
end

return demo
