# Presentations as code

Write a native deck in Lua, Teal or TSX using ordinary Kit components.
The launcher supplies navigation, reveals, overview, notes, presenter
previews, a timer, fullscreen and PNG export. A controller can also be
embedded in an existing application.

## Lua / Teal

The kui runner carries `kit.presentation` and Kit:

```lua
local kit = require("kit.ui")
local present = require("kit.presentation")

present({
  title = "Ideas, running.", preset = "minimal",
  slides = {
    { key = "intro", title = "An executable idea", notes = "Explain the example first.",
      view = function(c)
        return c.content({ grow = true, preset = "title", title = "An executable idea" }, {
          kit.paragraph({ text = "Ordinary code. Real components." }),
          c.reveal(1, { kit.code({ language = "lua", text = 'print("hello")' }) }),
        })
      end },
  },
})
```

Save this as `main.lua` in an app folder, then:

```sh
kui run path/to/deck --watch
kui run path/to/deck -- --presenter
kui run path/to/deck -- --theme paper --layout split
kui run path/to/deck -- --canvas 1280x720
kui run path/to/deck --export slides --size 1920x1080 --scale 1 --appearance light
kui run path/to/deck --export steps --steps --time 2000
```

Runner flags precede `--`; deck flags follow it. A deck is an ordinary
app: use `kui run`, including watch, packaging and headless capture. The
`present` function installs the deck controller; it needs no special CLI command. `kui types path/to/deck` includes the host
module's Lua source and Teal declaration for editor support.

[The runnable Lua example](samples/presentations/main.lua) includes charts,
syntax highlighting, a stateful demo and project settings.

## TSX

```tsx
import * as kit from '@qxuken/kui/kit';
import { present, type Deck } from '@qxuken/kui/kit/presentation';

const deck: Deck = {
  preset: 'minimal',
  slides: [{ key: 'intro', notes: 'Explain the example first.',
    view: c => <c.Content grow preset="title" title="An executable idea">
      <kit.Paragraph text="Ordinary code. Real components." />
      <c.Reveal step={1}><kit.Code language="typescript" text='console.log("hello")' /></c.Reveal>
    </c.Content>,
  }],
};
await present(deck, { width: 1440, height: 940 });
```

[The TSX example](../../examples/node/apps/presentation.tsx) runs with
`npm run presentation` from `examples/node`. Its launcher accepts
`--presenter`, `--theme`, `--layout`, `--canvas`, `--export`, `--steps`,
`--time`, `--size`, `--scale` and `--appearance`. `--size` sets the live
window's logical dimensions and `--appearance` pins light/dark mode.
`--steps`, `--time` and `--scale` apply to export; live windows use their
display's scale. Theme/layout/canvas launch overrides persist through watch reload.

For watch reload, export a pure deck as a module's default and pass its
path or URL to `present(new URL('./deck.mjs', import.meta.url), { watch: true })`.
The module must not call the launcher itself. The watcher reloads this
entry; for TSX/imported dependencies, bundle it and watch the bundle with
your compiler. Passing a deck object allows explicit `controller.reload(deck)`.

## Themes belong to Settings

`paper`, `signal`, `stage` and `minimal` are presets of existing Kit
Settings, available to ordinary apps too. They supply a palette, font
choices and a content type scale. Project settings override individual
fields; `roles` and `sizes` merge by name rather than replacing the whole
preset. Controls keep their compact label and control sizes.

```lua
settings = {
  font_ui = { "Inter", "sans" },
  font_title = { "Project Display", "serif" },
  font_content = { "Project Text", "sans" },
  font_data = { "Project Mono", "mono" },
  roles = { accent = 0x8c3e70ff, bg = { light = 0xfffcf8ff, dark = 0x18151aff } },
  sizes = { heading = 58, display = 88, content = 24, caption = 14,
    code = 18, chart = 14, gap = 24, pad = 56 },
}
```

In TSX these fields are `fontUi`, `fontTitle`, `fontContent`, `fontData`;
`roles` also accepts colors such as `'#8c3e70'`. Set `fonts` on the deck
to font-file paths to register bundled faces; family lists select among
registered/system fonts and stock `sans`, `serif`, `mono` fallbacks.
Lua resolves font paths against the app folder; Node uses its working directory.

Other `sizes` fields are `body`, `label`, `title`, for the existing tool
scale. Content tokens include `$kitHeading`, `$kitDisplay`, `$kitContentSize`,
`$kitCaptionSize`, `$kitCodeSize`, `$kitChartSize`, `$kitContentGap`,
`$kitContentPad`, `$kitTitle` and `$kitContent`. Use Settings before nodes
that read these tokens, including the application's background.

A deck's `preset` chooses a theme and a matching layout default:

| Preset | Content theme | Layout |
| --- | --- | --- |
| `paper` | Paper tones, editorial serif | `editorial` |
| `signal` | Technical type, green and lime | `briefing` |
| `stage` | Large type, warm accent | `split` |
| `minimal` | Sans serif, whitespace, little framing | `minimal` |

`theme` and `layout` override these independently. Theme and layout are authoring settings, not live presenter controls.
Edit them in code (or supply launch flags); watch reload applies the
authored settings while preserving compatible demo state. Layout defaults affect
`c.content` / `<c.Content>`; an ordinary authored tree retains its own
structure. Content's named `aside` child group supplies the companion
region in editorial/split layouts. Use `c.layout` and `c.theme` for content
that needs an explicit treatment, such as text over a split accent panel.

Base Kit now also provides Heading, Paragraph, Content, Columns,
Comparison, Quote, Figure and Visibility. Code accepts `language`,
`highlight_lines` / `highlightLines`, `size` and `line_height` / `lineHeight`.
The highlighter is lexical, preserves source bytes and leaves unknown
languages plain. Stat accepts `display = true` for a large number.
See the [component reference](bindings/reference.md) and `kui play`'s
Content page and Theme controls.

## Navigation and windows

| Key | Action |
| --- | --- |
| Right, Down, Page Down, Space | Next reveal, then next slide |
| Left, Up, Page Up, Shift+Space | Previous reveal, then prior slide's final step |
| Home / End | First / last slide, initial step |
| O | Open or close private presenter overview; click a thumbnail/title to jump |
| F | Toggle audience fullscreen |
| Escape | Close a dialog, overview or fullscreen |
| Cmd/Ctrl+Shift+P | Open or focus presenter |
| ? | Open keyboard shortcuts privately in presenter |
| Cmd/Ctrl+Enter | Present the selected slide/step from the presenter |

Navigation stops at deck boundaries. Focused editors, sliders, charts and
other controls keep their normal keys; deck shortcuts handle keys the
control leaves unclaimed. Hold arrows/Page Up/Page Down to repeat; toggle
shortcuts and Space remain single press. Tab enters and leaves controls.

The audience view contains only slide content, with no position counters,
restart or presenter buttons. On macOS, the Presentation menu includes
Show Presenter (Cmd+Shift+P), Slide Overview (Cmd+Shift+O) and Keyboard
Shortcuts (Cmd+/). On any platform, `?` opens shortcut help in the private
presenter, leaving the audience untouched. Focused controls keep their keys.

The presenter follows Kit's tool-window layout: the window titlebar,
a content toolbar with the deck name, navigation, overview and Restart…,
framed panels for current/next slide, speaker notes, timer and audience
output, and a status footer for slide/step position and Live sync.
Restart… asks before resetting all demo state. Upcoming reveals are faded
and boxed, with numbered badges; these guides never appear in audience
content or export. Overview is private and keeps the current audience
demo running. The layout stacks and scrolls at narrower window sizes.

Window chrome is never forced by the launcher. Ordinary samples retain
their own configured decorations. For a custom-chrome host, a separate
plain titlebar precedes the presenter toolbar by default. Authors can
explicitly opt into an integrated presenter toolbar with
`toolbar_in_titlebar = true` (Lua options) / `toolbarInTitlebar: true`
(Node options), while configuring the host's chrome as custom and its
titlebar as medium/tall. The shared Kit Toolbar `window_chrome` /
`windowChrome` prop handles the native-control inset and draggable gaps;
its default is false. Interactive controls keep their clicks.

Show audience reopens it and enters borderless fullscreen on the selected
display. Unsupported display/fullscreen requests fall back to what the
window manager can honor. Notes and shortcut help never appear in audience
content or export. Either window can close while the other remains;
closing the last ends the session. The recovery shortcut works from either
window and is configurable with `presenter_shortcut` (Lua) /
`presenterShortcut` (TSX): `{ key = "p", primary = true, shift = true }`.
`primary` accepts Cmd or Ctrl; explicit `ctrl`, `super`, `alt`, `shift`
fields are also supported. These are app-local keys.

Default layout is adaptive and reflows with the native viewport. Optional
`canvas = { w = 1280, h = 720 }` fits and letterboxes a fixed logical
composition. Pointer, focus and clipping follow its transform. Enlarged
static text rasterizes at the drawn resolution; images still need enough
source pixels. Reduced motion disables the slide entrance transition.

## State, lifecycle and snapshots

Slides require a unique nonempty `key` and `view(context)`. `title` and
`notes` are optional. Stateful slides add `init()`, `update(state, message)`
and optionally `activate(context)`; activation may return cleanup.

The context exposes `state`, `step`, `size`, `now` in milliseconds,
`theme`, `layout` and `mode` (`live` or `snapshot`). Wrap handlers in
`c.msg({ kind = "change" })` / `c.msg({ kind: 'change' })`; update receives
the original message plus the control's event data, for example
`message.change.value`. Primitive messages arrive under `tag`.

Declare all reveals on every render, using positive integer step numbers.
Several reveals can share a step. Unrevealed content keeps its layout
space while being absent from paint, hit testing, accessibility and Tab.
Use the reveal's `{ collapse = true }` / `{ collapse: true }` option to
remove its layout space too. Arbitrary conditional rendering that omits
future reveals cannot be discovered; supply an explicit `steps` count
when discovery alone is insufficient.

Create effects in `activate`, using `c.every(ms, message)`,
`c.after(ms, message)` or `c.spawn(task)`. Timers and returned cleanup are
disposed when leaving a slide, reloading, restarting or
shutting down. Late results from obsolete activations are ignored. Node
tasks receive an AbortSignal. Lua tasks receive a signal with `aborted`
and `on_abort(callback)` for cooperative cancellation and resource cleanup.
Returning to a slide reactivates its work with
its retained state. Restart talk resets state and navigation.

Views must be pure compositions. Snapshot rendering copies plain state
and does not call activation. Native resources such as channels remain
shared references; supply immutable deterministic data or a snapshot
substitute for resources that cannot be read safely. An optional
`snapshot(context)` replaces such a slide's composition and requires an
explicit nonnegative `steps` count. This contract does not sandbox
arbitrary code or stop work created outside the managed helpers.

Reload keeps the current stable key and compatible demo state, clamps the
step, and selects the nearest index if a slide was removed. Change
`state_version` / `stateVersion` to reset an incompatible state shape.
Managed work is disposed and recreated, rather than duplicated.

## Embedding and export

Lua `presentation.new(deck, options, previous?)` returns a controller with
`model`, `update`, `view`, `next`, `previous`, `jump`, `pace`, `dispose` and
`export_plan`. Route application messages to `update`, use `view(env)`
in the main window, provide `options.dispatch` for your event loop, call
`pace` when starting, and `dispose` at shutdown. Pass the old controller
to a new one for keyed reload.

TSX `createPresentation(deck, options)` offers the equivalent controller
with `start`, `renderSlide`, `exportPlan`, `reload` and `view(surface, window)`.
Wire dispatch in the host's setup, start after setup, and dispose at teardown.
The Node example's headless branch demonstrates this wiring.

Audience and presenter read the same session. Input in either window
invalidates the shared views immediately, including extension-handled
messages. The elapsed timer is independent; keeping it paused does not
pause navigation or demo-state synchronization. The Kit gallery's
Presentations page demonstrates an embedded controller with live audience
content, private reveal guides and retained demo state.

**Live sync** is enabled by default. Disable it in the presenter footer
to browse slides, notes, reveal steps and overview privately while the
audience stays on the current slide. **Present slide** (Cmd/Ctrl+Enter in
the presenter) publishes the selected slide and step without re-enabling
sync. Enabling Live sync publishes the current selection and resumes
following presenter navigation. The footer shows the audience position
while browsing privately. Audience demo effects keep running until you
actually present a different slide; private browsing never activates them.

Controllers keep the audience position in `model.index/key/step` and the
private selection in `model.preview`. Use `next/previous/jump` with the
optional `"presenter"` source when embedding presenter navigation, or pass
`source = "presenter"` in navigation messages (`source: 'presenter'` in
Node). `present_slide()` / `presentSlide()` publishes that selection.
Lua's `model.live_sync` and Node's `model.liveSync` reflect the toggle.
Watch reload preserves both positions by stable slide keys.

`exportPresentation(deck, { directory, size, scale, appearance, allSteps,
timestamp })` is the Node API. Lua uses the runner's `--export` command.
Each frame renders in an independent headless context, with no live
activation, a fixed clock, reduced motion and settled layout. Default
output is one final-step PNG per slide; `--steps` includes step zero and
all reveals. The manifest records viewport, scale, appearance and each
frame's slide key, title, one-based index, step, relative image filename
and timestamp. Export errors identify the slide/step. Export starts from
initial demo state, so provide deterministic initial data or snapshots.

PNG is the current export format. PDF, editable PPTX and web rendering
remain future work. Display selection and native window close/reopen
behavior require the native windowed driver; headless hosts receive
window commands as data.
