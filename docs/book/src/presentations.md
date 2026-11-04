# Presentations as code

A Kit deck is ordinary Lua, Teal or TSX. Each slide is a render function
with a stable key, optional notes, and any native Kit components you need.
The `present` function handles reveals, navigation, overview, private
presenter windows, fullscreen and export. A deck runs through the ordinary
application lifecycle: `kui run`, watch reload and packaging all work.

Open `kui play` and choose **Presentations** for an interactive introduction.
The gallery embeds the same controller in a card, with live audience
content beside a private presenter preview. Try Next to reveal each point,
then Add on the second slide to see state shared by both views.

To create a standalone deck, save this as `main.lua`:

```lua
local kit = require("kit.ui")
local present = require("kit.presentation")
present({ preset = "minimal", slides = {
  { key = "hello", notes = "Start with the working example.",
    view = function(c)
      return c.content({ grow = true, preset = "title", title = "Ideas, running." }, {
        c.reveal(1, { kit.paragraph({ text = "Make the explanation executable." }) }),
      })
    end },
  { key = "demo", notes = "Press Add. Return here later: the count is retained.",
    init = function() return { count = 0 } end,
    update = function(s, msg)
      if msg.kind == "add" then s.count = s.count + 1 end
    end,
    view = function(c)
      return c.content({ title = "An explanation you can use" }, {
        kit.paragraph({ text = "Count: " .. c.state.count }),
        kit.button({ label = "Add", on_press = c.msg({ kind = "add" }) }),
      })
    end },
} })
```

Use `kui run . --watch -- --presenter`. Omit `-- --presenter` to start with
only the audience window. No separate presentation command is needed:
requiring `kit.presentation` and calling `present` defines the application.
Right/Space advances; Left rewinds; O opens private presenter overview; F toggles audience
fullscreen; Cmd/Ctrl+Shift+P opens or focuses the presenter. The audience
contains only slide content. `?` opens private shortcut help; macOS also
has a Presentation menu with the recovery accelerator.
Focused controls keep their normal keys. Closing one presentation window
keeps the other alive.

Both windows read one session. Navigation from either window updates the
other on the next frame while **Live sync** is enabled; live demo input in the audience updates the
presenter's snapshot too. The elapsed timer is independent and can stay
paused. Presenter previews show unrevealed content faintly inside numbered
boxes. Overview, notes and shortcuts remain private.

Disable **Live sync** in the presenter footer to find a slide privately.
Previous, Next, arrow keys and overview then move the private preview;
the audience stays on its current slide, with its live demo still running.
Press **Present slide** or Cmd/Ctrl+Enter to publish the selected slide and
reveal step. Sync remains off so you can keep browsing. Turning Live sync
back on presents the selection and resumes following navigation. The
footer shows where the audience is while you browse. Hold an arrow key
to navigate repeatedly; focused controls retain their own arrow behavior.

Paper, Signal, Stage and Minimal are Kit Settings presets. Override the
project's `font_title`, `font_content`, `font_data`, `roles` and `sizes`
without replacing the preset's whole palette or scale. Themes and layouts
are independent. Adaptive layout is the default; `-- --canvas 1280x720`
opts into a fixed logical composition.

For example, add these authored settings to the table passed to `present`:

```lua
theme = "paper",
layout = "editorial",
settings = {
  font_title = { "Georgia", "serif" },
  font_content = { "Inter", "sans" },
  roles = { accent = 0x315ccaff },
  sizes = { heading = 64, content = 24 },
},
```

Watch reload applies these changes while keeping state for slides whose
keys and state compatibility versions still match. Theme and layout are
authoring choices; the presenter has navigation, notes and timer controls.

TSX uses `present` from `@qxuken/kui/kit/presentation` and the context's
`<c.Content>` / `<c.Reveal>` with ordinary Kit JSX. Stateful slides add
`init`, `update` and `activate`; their state survives navigation, and
managed activation work is canceled when inactive. Pure snapshot views
provide safe previews and export without activating live demonstrations.

`kui run . --export slides --steps --size 1920x1080 --appearance light`
writes numbered PNGs and a manifest. Omit `--steps` for each slide's final
step. Export uses isolated snapshots and a fixed clock without starting
live slide effects. Node offers the same export through `exportPresentation`.

The [authoring guide](https://github.com/qxuken/kui/blob/main/addons/kit/PRESENTATIONS.md)
covers project overrides, lifecycle, embedding, reload and export. Start
from the [Lua deck](https://github.com/qxuken/kui/tree/main/addons/kit/samples/presentations)
or [TSX deck](https://github.com/qxuken/kui/blob/main/examples/node/apps/presentation.tsx).
