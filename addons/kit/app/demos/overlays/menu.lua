-- A menu and a popover, each floating beside the button that opened it.

local kit = require("kit.ui")

local demo = {
  title = "Menu and popover",
  note = "floats beside what opened them",
  call = 'row { kit.button({ label = "Actions", on_press = ... }),\n  model.menu and kit.menu({ items = { ... }, on_choose = ..., on_dismiss = ... }) }',
  state = { menu = false, popover = false, last_choice = nil },
}

function demo.update(m, msg)
  local k = msg.kind
  if k == "menu" then m.menu = msg.open
  elseif k == "popover" then m.popover = msg.open
  elseif k == "choose" then m.menu = false; m.last_choice = msg.choose.value
  end
end

function demo.view(m)
  return row { gap = 8,
    row { kit.button({ label = "Actions", icon = "chevron_down", on_press = { kind = "menu", open = true } }),
      m.menu and kit.menu({ on_choose = { kind = "choose" }, on_dismiss = { kind = "menu", open = false }, items = {
        { value = "copy", label = "Copy link", shortcut = "⌘C" },
        { value = "rename", label = "Rename…" },
        { value = "archive", label = "Archive", disabled = true },
        { value = "delete", label = "Delete", danger = true },
      } }) or nil },
    row { kit.button({ label = "Details", on_press = { kind = "popover", open = true } }),
      m.popover and kit.popover({ label = "Details", width = 260, on_dismiss = { kind = "popover", open = false } }, {
        text("A popover holds anything: here, a stat.", { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }),
        kit.stat({ label = "p95", value = "412 ms", delta = "-8%", tone = "ok" }),
      }) or nil },
    text(m.last_choice and ("chose " .. m.last_choice) or "", { size = 11, family = "$kitData", color = "$kitMuted" }) }
end

return demo
