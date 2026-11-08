local kit = require("kit.ui")
local present = require("kit.presentation")
present({ preset = "minimal", slides = {
  { key = "hello", notes = "Start with the working example.",
    view = function(c)
      return c.content({ grow = true, preset = "title", title = "Ideas, running." }, {
        c.reveal(1, { kit.paragraph({ text = "Make the explanation executable." }) }),
      })
    end },
  { key = "demo", notes = "Press Add. Return here later: the count is retained.",
    init = function() return { count = 0 } end,
    update = function(s, msg)
      if msg.kind == "add" then s.count = s.count + 1 end
    end,
    view = function(c)
      return c.content({ title = "An explanation you can use" }, {
        kit.paragraph({ text = "Count: " .. c.state.count }),
        kit.button({ label = "Add", on_press = c.msg({ kind = "add" }) }),
      })
    end },
} })
