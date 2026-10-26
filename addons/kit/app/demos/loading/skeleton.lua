-- Skeletons: lines a text high, a circle and a block in the skin's well,
-- rounded as its controls are, a shimmer crossing every bar in step. Under
-- reduced motion they hold still.

local kit = require("kit.ui")

local demo = {
  title = "Skeletons",
  note = "lines, a circle, a block; the shimmer holds still under reduced motion",
  call = 'kit.skeleton({ lines = 3 })\nkit.skeleton({ shape = "circle", width = 40 })\nkit.skeleton({ shape = "block", height = 96 })',
}

function demo.view(m)
  return row { width = "grow", gap = 12, cross_align = "center",
      kit.skeleton({ shape = "circle", width = 40, label = "Loading a picture" }),
      column { width = "grow", kit.skeleton({ lines = 2, label = "Loading a name" }) } },
    kit.skeleton({ lines = 3, label = "Loading a paragraph" }),
    kit.skeleton({ shape = "block", height = 96, label = "Loading a chart" })
end

return demo
