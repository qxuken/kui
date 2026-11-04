local kit = require("kit.ui")
local present = require("kit.presentation")

local measurements = channel({ minute = { 0, 1, 2, 3, 4, 5 },
  before = { 86, 90, 96, 104, 112, 119 }, after = { 86, 82, 78, 75, 71, 69 } })

local project_settings
for _, arg in ipairs(kui.args) do
  if arg == "--project-theme" then
    project_settings = {
      font_ui = { "Helvetica Neue", "sans" }, font_title = { "Georgia", "serif" },
      font_content = { "Helvetica Neue", "sans" }, font_data = { "Menlo", "mono" },
      roles = { accent = 0x8c3e70ff },
      sizes = { heading = 58, content = 23, code = 18, chart = 14, gap = 24, pad = 56 },
    }
  end
end

present({
  title = "Ideas, running.", preset = "minimal", settings = project_settings,
  slides = {
    { key = "idea", title = "Make ideas executable.",
      notes = "A slide can be a living interface. Advance to reveal the argument.\n\nOpen the presenter with **Cmd/Ctrl+Shift+P**. Themes and layouts are authored independently in the deck or supplied at launch.",
      view = function(c)
        return c.content({ grow = true, preset = "title", title = "Make ideas executable.",
          subtitle = "A presentation can be a place to think. Not just a place to put the answer." }, {
          c.reveal(1, { kit.paragraph({ text = "Compose a story with ordinary Kit components." }) }),
          c.reveal(2, { kit.paragraph({ text = "Let the audience explore the idea." }) }),
          aside = { text("LIVE.", { size = "$kitDisplay", family = "$kitTitle", color = c.layout == "split" and "$kitOnAccent" or "$kitAccent", bold = true }) },
        })
      end },
    { key = "evidence", title = "Let the evidence speak.",
      notes = "These numbers are illustrative, not benchmark results.\n\nThe chart is a real Kit chart. Hover or focus it and use its arrows: those keys belong to the chart.",
      view = function(c)
        return c.content({ grow = true, title = "Let the evidence speak.", eyebrow = "THE EVIDENCE" }, {
          kit.figure({ caption = "Illustrative p95 latency over six minutes; the same workload." }, {
            kit.line_chart({ data = measurements, x = "minute", height = 250, focusable = true, y_unit = "ms",
              series = { { field = "before", name = "Before", color = "$kitMuted" }, { field = "after", name = "After" } } }),
          }),
          c.reveal(1, { kit.stat({ label = "Lower tail latency", value = "42%", display = true, tone = "ok" }) }),
          c.reveal(2, { kit.paragraph({ text = "Show the result. Then let people inspect it." }) }),
        })
      end },
    { key = "source", title = "The source is the story.",
      notes = "The author writes a render function. Notes and live state are optional.\n\nThe code block and its emphasized lines are reusable base Kit features.",
      view = function(c)
        return c.content({ grow = true, title = "The source is the story." }, {
          kit.code({ label = "deck.lua", language = "lua", line_numbers = true, copy = true,
            highlight_lines = { 4, 5 }, text = 'local present = require("kit.presentation")\npresent({\n  theme = "paper",\n  settings = { sizes = { heading = 58 } },\n  slides = { { key = "intro", view = function(c)\n    return c.content({ title = "An idea" }, {})\n  end } },\n})' }),
          c.reveal(1, { kit.paragraph({ text = "Choose a preset. Supply your project's fonts, colors and sizes." }) }),
        })
      end },
    { key = "demo", title = "Show it. Change it.",
      notes = "Move the slider or run the workload. Its state survives navigation.\n\nThe active-seconds timer pauses when this slide is inactive. Snapshot previews and export never start it.",
      init = function() return { workers = 4, runs = 0, seconds = 0 } end,
      update = function(state, msg)
        if msg.kind == "workers" then state.workers = math.floor(msg.change.value)
        elseif msg.kind == "run" then state.runs = state.runs + 1
        elseif msg.kind == "tick" then state.seconds = state.seconds + 1 end
        return state
      end,
      activate = function(c) c.every(1000, { kind = "tick" }) end,
      view = function(c)
        local s = c.state
        local throughput = math.floor(s.workers * 138 / (1 + s.workers * 0.035))
        return c.content({ grow = true, title = "Show it. Change it.", subtitle = "A real interface belongs in the explanation." }, {
          kit.stat({ label = "Illustrative throughput", value = tostring(throughput) .. " jobs/s", display = true }),
          kit.slider({ label = "Concurrency", value = s.workers, min = 1, max = 16, step = 1,
            value_label = s.workers .. " workers", on_change = c.msg({ kind = "workers" }) }),
          kit.button({ label = "Run workload", variant = "primary", on_press = c.msg({ kind = "run" }) }),
          kit.code({ label = "Output", language = "json", copy = true,
            text = string.format('{ "workers": %d, "runs": %d, "active_seconds": %d }', s.workers, s.runs, s.seconds) }),
          c.reveal(1, { kit.paragraph({ text = "The experiment and its state are part of the deck." }) }),
        })
      end },
    { key = "comparison", title = "Keep the useful parts.",
      notes = "A comparison is an adaptive base Kit composition, usable outside a deck too.",
      view = function(c)
        return c.content({ grow = true, title = "Keep the useful parts." }, {
          kit.comparison({ first_title = "The familiar slide", second_title = "A living view" }, {
            first = { kit.paragraph({ text = "A conclusion, written down." }) },
            second = { kit.paragraph({ text = "The conclusion, its evidence and a working example." }) },
          }), c.reveal(1, { kit.quote({ text = "Make the explanation something people can explore.", attribution = "Kit presentations" }) }),
        })
      end },
  },
})
