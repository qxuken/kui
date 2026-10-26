-- Where else the keyboard goes: the components on other pages that
-- answer the same way.

local demo = {
  title = "Tabs, a table, a trace, a text area",
  note = "the same keys on the other pages",
  call = 'kit.tabs(...)  -- Left and Right\nkit.table({ ..., on_select = ... })  -- Up, Down, Home, End, Page Up and Down\nkit.waterfall({ ..., on_select = ... })  -- the arrows walk the spans\nkit.text_area({ ... })  -- Tab types; Escape then Tab, or Ctrl+M, leaves it (tab_moves_focus: Tab leaves)',
}

function demo.view(m)
  return text("The Layout page's tabs, the Data page's table, the Trace page's spans and the Inputs page's text areas answer the keyboard the same way.",
    { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
