-- An empty state: what a view shows with nothing in it, and what to do.
-- `press` is the app's own message: main.lua says in a toast what was
-- pressed.

local kit = require("kit.ui")

local demo = {
  title = "Empty state",
  note = "a title, a sentence, and what to do",
  call = 'kit.empty({ title = "No requests yet", detail = "...", { kit.button(...) } })',
}

function demo.view(m)
  return column { width = "grow", height = 200,
    kit.empty({ title = "No requests yet", detail = "Start the server, then send one." }, {
      kit.button({ label = "Start the server", variant = "primary", on_press = { kind = "press", what = "Start" } }) }) }
end

return demo
