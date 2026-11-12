-- Give touch readers the audience's existing navigation actions.
function __book_presentation_controls()
  local kit = require("kit.ui")
  local audience = view
  function view(env)
    local root = audience(env)
    table.insert(root, row {
      pad = 12, gap = 8,
      kit.button({ label = "Previous", on_press = { presentation = "previous" } }),
      kit.button({ label = "Next", on_press = { presentation = "next" } }),
    })
    return root
  end
end
