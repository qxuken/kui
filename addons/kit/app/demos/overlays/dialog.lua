-- A modal dialog: Tab stays inside it; Escape or a press outside asks it
-- to close, and the app closes it.

local kit = require("kit.ui")

local demo = {
  title = "Dialog",
  note = "modal: Tab stays inside; Escape or a press outside dismisses",
  call = 'if model.dialog then\n  kit.dialog({ title = "Discard?", on_dismiss = { kind = "close" } }, { ...body, actions = { ... } })\nend',
  state = { dialog = false },
}

function demo.update(m, msg)
  if msg.kind == "dialog" then m.dialog = msg.open end
end

function demo.view(m)
  return row { gap = 8,
      kit.button({ label = "Open a dialog", on_press = { kind = "dialog", open = true } }) },
    m.dialog and kit.dialog({ title = "Discard the draft?", on_dismiss = { kind = "dialog", open = false } }, {
      text("Three edits since the last save will be lost.",
        { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" }),
      actions = {
        kit.button({ label = "Keep editing", on_press = { kind = "dialog", open = false } }),
        kit.button({ label = "Discard", variant = "danger", on_press = { kind = "dialog", open = false } }),
      },
    }) or nil
end

return demo
