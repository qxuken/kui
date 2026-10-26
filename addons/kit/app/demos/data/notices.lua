-- Notices: messages in the flow of the page, in a tone, with actions.
-- `press` is the app's own message: main.lua says in a toast what was
-- pressed.

local kit = require("kit.ui")

local demo = {
  title = "Notices",
  note = "in the flow of the page, in a tone, with actions",
  call = 'kit.notice({ tone = "danger", title = "Could not save", message = "..." }, { kit.button(...) })',
}

function demo.view()
  return kit.notice({ message = "Saved drafts are kept for 30 days.", tone = "info" }),
    kit.notice({ tone = "warn", message = "The upstream answers slowly." }),
    kit.notice({ tone = "danger", title = "Could not save", message = "The disk is full." },
      { kit.button({ label = "Retry", icon = "refresh", on_press = { kind = "press", what = "Retry" } }) })
end

return demo
