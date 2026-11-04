-- An embedded controller: the same slides supply a live audience view
-- and an isolated presenter preview. No timer or extra window is needed.
local kit = require("kit.ui")
local presentation = require("kit.presentation")

local deck = { title = "Ideas, running.", preset = "minimal", slides = {
  { key = "idea", title = "A deck is an app", notes = "Reveal the two points, then continue to the live demo.",
    view = function(c)
      return column { width = "grow", gap = 16,
        kit.heading({ text = "A deck is an app", size = 28 }),
        c.reveal(1, { kit.paragraph({ text = "Ordinary code. Reusable Kit components.", size = 16 }) }),
        c.reveal(2, { kit.paragraph({ text = "One session for audience and presenter.", size = 16 }) }),
      }
    end },
  { key = "demo", title = "Make it interactive", notes = "Press Add in the audience pane. Its state appears in the presenter preview too.",
    init = function() return { count = 0 } end,
    update = function(s, msg) if msg.kind == "add" then s.count = s.count + 1 end end,
    view = function(c)
      return column { width = "grow", gap = 16,
        kit.heading({ text = "Make it interactive", size = 28 }),
        kit.paragraph({ text = "Count: " .. c.state.count, size = 20 }),
        kit.button({ label = "Add", variant = "primary", disabled = c.mode ~= "live",
          on_press = c.msg({ kind = "add", presentation_gallery = true }) }),
        c.reveal(1, { kit.paragraph({ text = "State survives a trip to another slide.", size = 16 }) }),
      }
    end },
} }

local controller = presentation.new(deck)
local demo = {
  title = "A presentation, inside an app",
  note = "Advance reveals, navigate slides, and try the stateful demo. The presenter sees future reveals in numbered boxes.",
  call = 'local kit = require("kit.ui")\nlocal presentation = require("kit.presentation")\npresentation({ preset = "minimal", slides = {\n  { key = "hello", notes = "Private speaker notes", view = function(c)\n    return c.content({ title = "Ideas, running." }, {\n      c.reveal(1, { kit.paragraph({ text = "Make it executable." }) }),\n    })\n  end },\n} })\n\n-- Save as main.lua, then:\n-- kui run . --watch -- --presenter\n-- kui run . --export slides --steps',
  state = { presentation_demo = controller.model },
}

local function session(m)
  controller.model = m.presentation_demo
  controller:pace()
  return controller
end

function demo.update(m, msg)
  if msg.kind == "presentation-gallery" then
    session(m):update({ presentation = msg.action, slide = msg.slide, toggle = msg.toggle, source = "presenter" })
  elseif msg.presentation_gallery then
    session(m):update(msg)
  end
end

function demo.view(m)
  local p = session(m)
  local state, selected = p.model, p:position("presenter")
  local slide = deck.slides[selected.index]
  return column { width = "grow", gap = 16,
    kit.toolbar({}, {
      text("Ideas, running.", { family = "$kitUi", size = "$kitSizeTitle", bold = true, color = "$kitFg" }),
      trailing = {
        kit.button({ label = "Previous", disabled = selected.index == 1 and selected.step == 0,
          on_press = { kind = "presentation-gallery", action = "previous" } }),
        kit.button({ label = "Next", variant = state.live_sync and "primary" or "default", disabled = selected.index == #deck.slides and selected.step == p.counts[selected.key],
          on_press = { kind = "presentation-gallery", action = "next" } }),
        not state.live_sync and kit.button({ label = "Present slide", variant = "primary",
          on_press = { kind = "presentation-gallery", action = "present-slide" } }) or nil,
      },
    }),
    row { width = "grow", gap = 16, cross_gap = 16, wrap_children = true,
      column { width = "grow", min_width = 320, height = 340,
      kit.panel({ title = "Audience", subtitle = "Live content", grow = true }, {
        column { width = "grow", height = 240, pad = { l = 40, r = 16, t = 20, b = 20 },
          p:render_slide(state.index, state.step, "live") },
      }) },
      column { width = "grow", min_width = 320, height = 340,
      kit.panel({ title = "Presenter", subtitle = "Private preview and notes", grow = true }, {
        column { width = "grow", height = 240, pad = { l = 40, r = 16, t = 20, b = 20 }, role = "none",
          p:render_slide(selected.index, selected.step, "snapshot", nil, true) },
        footer = { column { width = "grow", pad = 10,
          text(slide.notes, { size = "$kitSize", family = "$kitUi", color = "$kitMuted", wrap = "word" }) } },
      }) },
    },
    kit.statusbar({}, {
      text(string.format("Audience: slide %d / %d · step %d", state.index, #deck.slides, state.step),
        { size = "$kitSizeLabel", family = "$kitData", color = "$kitMuted" }),
      not state.live_sync and text(string.format("Private preview: slide %d · step %d", selected.index, selected.step),
        { size = "$kitSizeLabel", family = "$kitData", color = "$kitMuted" }) or nil,
      trailing = { kit.switch({ key = "live-sync", label = "Live sync", on = state.live_sync,
        on_toggle = { kind = "presentation-gallery", action = "live-sync" } }) },
    }),
    kit.markdown({ text = "Run a standalone deck with `kui run . --watch -- --presenter`. The audience window contains only slides. " ..
      "Disable **Live sync** to browse privately, then use **Present slide** to update the audience. " ..
      "Use **?** for private shortcut help and **Cmd/Ctrl+Shift+P** to reopen the presenter. " ..
      "Paper, Signal, Stage and Minimal are Settings presets; supply your project's fonts, colors and sizes in code. " ..
      "See the book's **Presentations as code** chapter for a complete starting point." }),
  }
end

return demo
