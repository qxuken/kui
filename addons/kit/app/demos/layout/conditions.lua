-- Props by condition (`when`): a row that reads its own width and stacks
-- when narrow, a card that colours itself by its own hover and press and
-- the appearance, and as many columns a line as fit (`min_column`). A
-- condition repaints; the view is not run again. `press` is the app's
-- own message: main.lua says in a toast what was pressed.

local kit = require("kit.ui")

local demo = {
  title = "Conditions",
  note = "props by condition: drag the rule, hover and press the card, Tab to it, switch the system's light and dark",
  call = 'row { width = "grow", gap = 12,\n  when = { { width = { below = 360, band = 16 }, dir = "column", gap = 4, hidden = { "hint" } } },\n  ... }\nrow { on_click = ..., bg = "$kitPanel",\n  when = { { hovered = true, bg = "$kitHover" }, { pressed = true, bg = "$kitPress" },\n           { dark = true, border = { w = 1, color = "$kitRule" } } } }\nrow { min_column = 140, ... }',
  state = { cond_split = 0.6, cond_name = "kit" },
}

function demo.update(m, msg)
  if msg.kind == "cond_split" then m.cond_split = msg.resize.fraction
  elseif msg.kind == "cond_name" then m.cond_name = msg.change.text
  end
end

local function slot()
  return column { height = 24, bg = "$kitSunken", radius = "$kitRadius" }
end

function demo.view(m)
  return column { width = "grow", height = 120,
      kit.split({ fraction = m.cond_split, on_resize = { kind = "cond_split" } }, {
        first = { column { width = "grow", height = "grow", pad = 12, gap = 8, bg = "$kitSunken",
          -- A row that reads its own width: label, field and hint in a
          -- line, stacked and the hint dropped when the pane is narrow.
          row { width = "grow", gap = 12, cross_align = "center",
            when = { { width = { below = 360, band = 16 }, dir = "column", cross_align = "start", gap = 4, hidden = { "hint" } } },
            text("Name", { size = "$kitSize", family = "$kitUi", bold = true, color = "$kitFg" }),
            row { width = "grow", min_width = 0,
              kit.text_field({ label = "Name", hide_label = true, value = m.cond_name, on_change = { kind = "cond_name" } }) },
            row { key = "hint", text("what the others see", { size = 11, family = "$kitUi", color = "$kitMuted" }) } } } },
        second = { column { width = "grow", height = "grow", pad = 12,
          text("the other pane", { size = 11, family = "$kitUi", color = "$kitMuted" }) } },
      }) },
    row { width = "grow", gap = 12, cross_align = "center",
      -- A card that colours itself by its own state: a hover repaints, the
      -- view is not run.
      row { key = "card", role = "button", label = "A card", width = 180, height = 56, pad = 12, radius = "$kitRadius",
        cross_align = "center", cursor = "pointer", bg = "$kitPanel", transition = 120,
        border = { w = 1, color = "$kitRule" },
        on_click = { kind = "press", what = "the card" },
        when = {
          { hovered = true, bg = "$kitHover" },
          { pressed = true, bg = "$kitPress" },
          { dark = true, border = { w = 1, color = "$kitInk" } },
          { reduced_motion = true, transition = 0 },
        },
        text("Hover, press, Tab", { size = "$kitSize", family = "$kitUi", color = "$kitFg" }) },
      -- As many columns as fit at 140 px.
      row { width = "grow", gap = 8, cross_gap = 8, min_column = 140, slot(), slot(), slot(), slot(), slot() } }
end

return demo
