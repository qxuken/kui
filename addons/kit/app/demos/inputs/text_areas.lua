-- Text areas: a well in every skin, growing with its text. Tab types,
-- unless `tab_moves_focus`.

local kit = require("kit.ui")

local demo = {
  title = "Text areas",
  note = "a well in every skin, growing with its text; Tab types, unless `tab_moves_focus`",
  call = 'kit.text_area({ label = "Notes", value = model.notes, rows = 3, max_rows = 6, tab_moves_focus = true, on_change = { kind = "notes" } })\nkit.text_area({ label = "Body", value = model.code, mono = true, on_change = { kind = "code" } })',
  state = { notes = "", code = "{\n    \"name\": \"kit\"\n}" },
}

function demo.update(m, msg)
  if msg.kind == "notes" then m.notes = msg.change.text
  elseif msg.kind == "code" then m.code = msg.change.text
  end
end

function demo.view(m)
  return row { width = "grow", gap = 16, cross_gap = 16, min_column = 280,
      kit.text_area({ label = "Notes", value = m.notes, rows = 3, max_rows = 6, placeholder = "Write a note…",
        tab_moves_focus = true, on_change = { kind = "notes" } }),
      kit.text_area({ label = "Body", value = m.code, rows = 3, mono = true, on_change = { kind = "code" } }) },
    text("In Body, Tab types four spaces; Escape then Tab, or Ctrl+M (Ctrl+Shift+M on macOS), moves on. In Notes, Tab moves on.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
