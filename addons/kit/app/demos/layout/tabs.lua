-- Tabs with counts, and a section that folds.

local kit = require("kit.ui")

local demo = {
  title = "Tabs and sections",
  note = "a count on a tab; a section folds and keeps its count in view",
  call = 'kit.tabs({ tabs = { ... }, value = model.tab, on_change = { kind = "tab" } }, { ...body })\nkit.section({ title = "Advanced", count = 2, collapsed = model.folded, on_toggle = { kind = "fold" } }, { ... })',
  state = { tab = "one", folded = false },
}

function demo.update(m, msg)
  if msg.kind == "tab" then m.tab = msg.change.value
  elseif msg.kind == "fold" then m.folded = msg.toggle.collapsed
  end
end

function demo.view(m)
  return column { width = "grow", height = 140,
      kit.tabs({ value = m.tab, on_change = { kind = "tab" }, tabs = {
        { value = "one", label = "Requests", count = 340 }, { value = "two", label = "Errors", count = 3 },
        { value = "three", label = "Settings" } } }, {
        column { pad = 12, text("The " .. m.tab .. " tab's body.", { size = "$kitSize", family = "$kitUi", color = "$kitFg" }) } }) },
    kit.section({ title = "Advanced", count = 2, collapsed = m.folded, on_toggle = { kind = "fold" } }, {
      kit.checkbox({ label = "Verbose logging", checked = false }),
      kit.checkbox({ label = "Keep history", checked = true }) })
end

return demo
