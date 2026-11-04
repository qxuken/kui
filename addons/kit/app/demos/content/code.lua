local kit = require("kit.ui")
return {
  title = "Code, at reading size", note = "Lossless lexical syntax colors and emphasized lines; reusable outside decks",
  call = 'kit.code({ language = "lua", size = 18, highlight_lines = { 2, 3 },\n  text = source, line_numbers = true, copy = true })',
  view = function()
    return kit.code({ label = "project.lua", language = "lua", size = 18, line_numbers = true, copy = true,
      highlight_lines = { 3, 4 }, text = 'local kit = require("kit.ui")\nkit.settings({\n  theme = "paper",\n  sizes = { heading = 58, content = 24 },\n  font_title = { "Georgia", "serif" },\n  roles = { accent = 0x315ccaff },\n})' })
  end,
}
