local kit = require("kit.ui")
local demo = {
  title = "Reading and composition", note = "Base Kit content; themes and project fonts/sizes come from Settings",
  call = 'kit.content({ title = "Ideas, running.", layout = "editorial" }, {\n  kit.paragraph({ text = "Content stays adaptive." }),\n  aside = { kit.quote({ text = "Make room for evidence." }) },\n})',
  state = { content_shown = true },
}
function demo.update(m, msg) if msg.kind == "content_visibility" then m.content_shown = msg.toggle.on end end
function demo.view(m)
  return column { width = "grow", gap = 24,
    kit.content({ pad = 20, title = "Ideas, running.", layout = "editorial" }, {
      kit.paragraph({ text = "Ordinary applications benefit from readable content, clear hierarchy and adaptive compositions too." }),
      aside = { kit.quote({ text = "Make room for evidence.", attribution = "Kit content" }) },
    }),
    kit.columns({ collapse_at = 640 }, {
      first = { kit.heading({ text = "A reading column", level = 2 }), kit.paragraph({ text = "Columns stack when the available space narrows." }) },
      second = { kit.figure({ caption = "Any image, SVG, chart or custom view can be a figure." }, {
        kit.shader({ preset = "noise", height = 100, animate = false }),
      }) },
    }),
    kit.comparison({ first_title = "Preset", second_title = "Project" }, {
      first = { kit.paragraph({ text = "Paper, Signal, Stage or Minimal." }) },
      second = { kit.paragraph({ text = "Your fonts, colors and sizes, merged over a preset." }) },
    }),
    kit.switch({ label = "Show retained content", on = m.content_shown, on_toggle = { kind = "content_visibility" } }),
    kit.visibility({ visible = m.content_shown }, { kit.button({ label = "Visible content can receive input" }) }),
  }
end
return demo
