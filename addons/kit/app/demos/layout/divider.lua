-- A divider: a rule between groups, drawn as the skin draws regions.

local kit = require("kit.ui")

local demo = {
  title = "Divider",
  note = "a rule between groups, as the skin draws regions",
  call = 'kit.divider({})',
}

local function note(s)
  return text(s, { size = 11, family = "$kitUi", color = "$kitMuted" })
end

function demo.view()
  return note("Above the rule: one group."), kit.divider({}), note("Below it: the next.")
end

return demo
