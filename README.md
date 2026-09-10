# kui

A clay-inspired, data-driven UI library for Rust: flat-array immediate-mode
layout underneath, an iced-style `view`/`on_event` flow on top, and a text
stack baked into the core. Because the IR is plain data all the way down,
scripting frontends bind to the same contract the Rust builders use — the
bundled Lua extension support is table-to-node conversion, not FFI gymnastics.

## Crates

| crate | role |
|---|---|
| `kui-core` | The bindable contract: flat per-frame tree, clay-style flex solver, core text stack (shaping/wrapping/caching via cosmic-text + glyph atlas), events-as-data, slotmap resources, quad display list |
| `kui-wgpu` | wgpu backend: one instanced über-pipeline (rounded rects, borders, glyphs), single draw call per frame |
| `kui` | Batteries-included runner: winit + wgpu around a `Core`, `App` trait, widget sugar |
| `kui-lua` | Lua extensions via mlua: scripts return table trees, receive events as tables |
| `kui-ffi` | C API (cdylib/staticlib + [include/kui.h](crates/kui-ffi/include/kui.h)): flat builder calls, opaque `KuiValue` payloads, `repr(C)` draw data, windowed runner via callbacks — and `CExtension`, the same contract inverted: a C shared library as a guest in someone else's frame |
| `kui-node` | Node.js addon (napi-rs) + the [`packages/kui`](packages/kui) npm package: JSX views (custom jsx-runtime, no React) lowered into the IR in one call per frame, Elm-style messages as data |

## Testing without a window

The core owns no clock, no window and no device: a frame is a function of
the tree, the input so far and the time the driver hands in, and everything
the frame produces — the display list, the events, the audio commands, the
warnings — comes back as data. So the same app runs headless, and a test
drives it the way a user would and asserts on what the core produced,
instead of testing the model and hoping about the view:

```ts
// A sketch of a real test against a volume slider (Node, node:test).
const app = createApp({ init, update, view }, { width: 320, height: 240 });
app.render();
app.ctx.setTime(0);                                   // the clock is yours
const knob = decodeQuads(app.ctx.quads()).find((q) => q.kind === 0 && q.radius === 8);
app.ctx.cursor(knob.x + 4, knob.y + 4);               // a drag is just input
app.ctx.mouse(true);
app.ctx.cursor(knob.x + 64, knob.y + 4);
app.ctx.mouse(false);
app.settle();                                         // events → update → render
assert.equal(app.model.volume, 0.5);
app.press('tab');                                     // a whole key, both channels
app.press('right');                                   // so the ring moves and the slider nudges
assert.equal(app.model.volume, 0.55);
assert.deepEqual(app.ctx.audioCommands().map((c) => c.kind), ['setVolume']);
assert.ok(decodeQuads(app.ctx.quads()).every((q) => q.x + q.w <= 320), 'nothing overflows');
assert.deepEqual(app.warnings, []);                   // nothing misconfigured
```

A button inside a drag strip still takes its own click, a compact tier fits
its window with nothing overflowing, the chime plays when it should: all of
it is assertable because all of it is data the core emits rather than a side
effect it performs. An app with a clock is no exception — Node's two drivers
are one loop over an injected surface, so `tick` runs headless too and
`app.advance(ms)` is the window's timer by hand: it fires every tick inside
the span and moves the frame clock with them, which is how a countdown or a
mid-flight transition gets stepped through in a test — and `app.runOut()`
advances until nothing animates, for a test that wants the settled frame
rather than the first one. A test against a *real* window awaits the same
two frames instead of sleeping for them, since its clock is the wall's and
nothing can move it: `await app.frame()` is the next painted frame and
`await app.settled()` the next still one, both answered from inside the
driver's pump. An effect the app defines and kui knows nothing
about — a file write, a request, the clipboard — is data on the same
terms: `update` returns it beside the model with `withEffects(model,
...effects)`, the loop performs it after the frame through the `effects`
handler the app registered, and `app.effects()` is where a test reads it
back ([ADR 0013](docs/adr/0013-effects-as-data.md)). Rust tests drive `Core` the same way
([crates/kui-core/tests](crates/kui-core/tests)), and C runs the same API
headless (`./target/debug/counter --headless`, the same drive every counter runs) — as does a C *extension* inside
a Rust host (`cargo run -p kui-ffi --example c_panel -- --headless`, which
clicks the plugin's list and checks the click reached the plugin and not the
host).

## Devtools

Every app has a devtools panel, drawn by the core into its own frame
([ADR 0024](docs/adr/0024-the-devtools-are-the-cores.md)). Ask for it with
`kui::app("x").devtools(true)` (or `core.set_devtools(true)`),
`win.setDevtools(true)` in Node, `kui_set_devtools(ctx, true)` in C — or
run any of them with `KUI_DEVTOOLS=1` in the environment and it is there
with no code at all. It docks beside the app's tree (`left`, `right`,
`bottom` — drag the pane's inner edge to resize it; the app's viewport is
what the pane leaves, and a change is a `resize`), pops out into a
window of its own (`window`) or hides with its chords live (`off`); a
button per placement sits in its header, `Ctrl+Shift+D` walks them,
`KUI_DEVTOOLS=bottom` starts there. Three tabs: **facts** (what the runtime believes right now — the theme,
the windows, the viewport, focus, the modifiers, audio), **events** (every
event the app is handed, from every window, each row opening into its
payload; filter, pause, follow) and **tree** (the last frame's nodes,
collapsible and filterable, with a picker — `Ctrl+Shift+P` — that finds
the node under the pointer over the app and an inspector for the one
selected). The panel's own clicks and its `Ctrl+Shift+<letter>` chords are
handled inside the core: nothing of it reaches `on_event` / `update`, the
app's keys do not move for it, and off it costs one bool per frame.

## Examples

All of them live under [examples/](examples), one directory per language
and, inside it, one per kind — `apps/`, `widgets/`, `features/`, `tools/`
— with one subject each and the name the repo already uses for it
([ADR 0021](docs/adr/0021-one-subject-per-example.md));
[examples/README.md](examples/README.md) is the full map. Every one runs
inside a harness that puts a dock beside it: the event stream as data, the
runtime's facts with a small button beside each one it can change, the
latency graph, the key legend — and `--headless` is a self-check with an
exit code, `--light` / `--dark` pin the base.

```bash
cargo run -p kui --example counter        # apps/: the Elm loop, the one every binding has
cargo run -p kui --example splitmux       # apps/: tmux-style splits, tabs, ⌘-drag pane moves
cargo run -p kui --example button         # widgets/: the stock button in every state
cargo run -p kui --example edit           # widgets/: multiline editing and the single-line field
cargo run -p kui --example text           # widgets/: spans, decorations, families, wrap
cargo run -p kui --example cells          # widgets/: a terminal grid that selects in cells
cargo run -p kui --example focus          # features/: the Tab ring and its verbs
cargo run -p kui --example transition     # features/: transition, easing, slide, keyframes
cargo run -p kui --example enter_exit     # features/: toasts that slide in and back out
cargo run -p kui --example theme          # features/: every Theme role over every stock widget
cargo run -p kui --example counter -- --headless   # the drive, no window
./examples/c/build.sh && ./target/debug/counter    # the same app from C (Windows: pwsh examples/c/build.ps1)
cargo run -p kui-ffi --example c_panel    # a Rust host + a dlopened C panel
cargo run -p kui-lua --example lua_panel  # a Rust host + a Lua panel sharing one frame
scripts/smoke-examples.sh                 # every windowed example, 120 frames, both bases
scripts/smoke-headless.sh --run           # every headless drive, what CI runs
```

The same app from Node with JSX — build the addon with
`cargo build -p kui-node --release`, then in [examples/node](examples/node)
`npm install && npm run counter` (a real winit + wgpu window, its event
loop pumped from a timer so it shares the main thread with libuv; `node
dist/apps/counter.mjs --headless` is the drive):

```tsx
// tsconfig: "jsx": "react-jsx", "jsxImportSource": "@qxuken/kui"
const view = (model: Model) => (
  <box pad={24} gap={16}>
    <button onClick={{ kind: 'add', by: 1 }}>+1</button>
    <text size={20}>{`count = ${model.count}`}</text>
  </box>
);
const app = createApp({ init, update, view }, { width: 640, height: 480 });   // headless
const final = await runWindowed({ init, update, view }, { title: 'counter' }); // a window
```

JSX elements are plain data, so the tree *is* the frame: the jsx-runtime has
no reconciler and no React — `Ctx.frame()` encodes the element tree into a
flat binary IR stream (one `Float64Array` + a string table) and the addon
lowers it in a single zero-copy call, and `onClick` carries a message value,
never a closure. That value is the app's own union in TypeScript: annotate
`update` with `type Msg = MyMsg | CoreMsg` and `createApp` / `runWindowed`
infer it, so events, `dispatch` and ticks all speak it and `update` is one
switch over `msg.kind` (see [packages/kui](packages/kui/README.md)). The
encoder is the only door: there is one element dispatcher in the addon, not
one per transport, and a malformed view is rejected in JS before anything
crosses the boundary. `npm test` in [packages/kui](packages/kui) pins the
encoder from both ends — every schema prop and element has to reach the
stream and lower — and the corpus scenes say what the result must be.
A one-off measurement of the
same view built through Node 26's experimental `node:ffi` module, calling the
C API in [kui.h](crates/kui-ffi/include/kui.h) directly, put ~1500 flat calls
per frame at ~0.24 ms against ~0.19 ms for the single binary-stream call,
while plain C over the same API takes ~0.08 ms — the boundary itself is
~30 ns/call, the rest is JS-side argument marshalling (raw addresses beat
Buffer arguments by ~30%), so the binary stream stays the default.

A Lua extension in full:

```lua
slots = { "panel" }                       -- what it fills, in its own words

function view(env, slot)                  -- slot = { name, namespace, params, key }
  if note_key then note = env.edit_text(note_key) end   -- read editors back by key
  return column { gap = 8, pad = 16, bg = 0x14161eff,
    text({ slot.params.title, { " · " .. tostring(count), bold = true } }, { size = 20 }),
    row { tooltip = "adds one", button { label = "bump", on_click = { kind = "bump" } } },
    edit { key = "note", initial = "", width = "grow" },
  }
end

function on_event(ev)                     -- ev = payload + node_key
  if ev.kind == "bump" then count = count + 1; return { kind = "bumped", count = count } end
  if ev.kind == "changed" then note_key = ev.node_key end
end                                       -- what it returns is a reply to the host
```

Where it draws is a **slot** the host declares in its own view
([ADR 0014](docs/adr/0014-slots-an-extension-fills-in-place.md)): a position
among the host's children, filled then and there, with parameters in and
replies out. The host loads the script under a namespace it chooses — the
way an importer picks an alias — and names the slot by `namespace/slot`:

```rust
kui::app("counter").extension_as("fs", LuaExtension::from_file("panel.lua")?).run(app)
// …and in the host's view, wherever the panel should sit:
ui.slot_with("fs/panel", &Value::map([("title", "notes".into())]));
```

The same plugin loaded twice under two namespaces is two panels with two
sets of parameters; an extension that names no slots draws after the host's
view, where every extension drew before slots existed.

The same functions from C, because the extension contract is the contract
and the language is a detail — `kui_ffi::CExtension` `dlopen`s a shared
library and hands it the same share of the frame
([examples/c/features/slots/panel.c](examples/c/features/slots/panel.c),
[examples/c/features/slots/panel.rs](examples/c/features/slots/panel.rs)):

```c
static const KuiStr SLOTS[] = {{(const uint8_t *)"panel", 5}};
const KuiStr *kui_ext_slots(size_t *n) { *n = 1; return SLOTS; }
void kui_ext_view(void *user, KuiCtx *ui) {
    const KuiValue *params = kui_slot_params(ui);   /* what the host passed  */
    KuiSpec panel = {.dir = KUI_COLUMN, .gap = 8, .bg = 0x14161eff};
    kui_open(ui, &panel, NULL);              /* ordinary builder calls, into  */
    kui_button(ui, KUI_STR("bump"), msg());  /* the host's own frame          */
    kui_close(ui);
}
void kui_ext_on_event(void *user, const KuiEvent *ev) {
    /* yours, never the host's - and kui_reply(ev, value) is how you answer it */
}
```

The plugin links against nothing: every `kui_*` call is left undefined and
resolved from the host executable at load, the way a Lua C module resolves
`lua_*`. That is the whole trick, and it costs the host one linker flag —
`--export-dynamic`, which is what `crates/kui-ffi/build.rs` exists for.

## Reference

[docs/howto.md](docs/howto.md) is the task index: about twenty questions a
developer actually arrives with — "how do I animate a removal", "how do I
test the real window", "how do I reset an editor" — each answered in two
sentences that point at the row, the release entry or the ADR with the rest.
Start there when you know what you want and not what it is called; the two
field reports that asked for it had both filed wishes for features already
sitting in their own `node_modules`.

[docs/props.md](docs/props.md) is the cross-binding reference: every prop
with its JSX, Lua and C name, the composites, the elements, the event
payload shapes and the resource APIs. It is generated from the schema
(`npm run gen` in `packages/kui`), so it cannot drift.

The same command generates the addon's half of `packages/kui/index.d.ts` —
`Ctx`, `KuiWindow` and their 39 shared methods each, between the generated
markers — out of napi-rs's own reading of the `#[napi]` attributes in
`crates/kui-node`. A method reaches TypeScript by being written in Rust and
regenerating, not by being typed out a second time; CI diffs the file.

[docs/adr](docs/adr) holds the architecture decision records: the choices
that are hard to reverse and would look arbitrary without their context.

## Design notes

- **One prop schema, three bindings.** `kui_core::schema::PROPS` is the
  single source of truth for the per-node surface: one row = name, wire id,
  value kind, apply fn, doc. Node interprets it for JSON, the binary stream,
  the JS encoder (`protocol()`), and the generated TS prop types; Lua looks
  each table key up by the same name in snake_case (`minWidth` is
  `min_width`); the C struct must be static, so `KuiSpec` stays hand-written
  and a parity test in `kui-ffi` fails when a row has no C field. Adding a
  simple prop is one row (+ `npm run gen`); the composites (`pad`, `border`,
  overflow, `float`) keep per-binding shapes on purpose. Elements are the
  same set everywhere too: containers, text and rich spans, editors, images,
  fragments (a box a WGSL function paints),
  lines (segments, polylines and curves), buttons, titlebar (plain or with
  custom content), window buttons, latency graph/HUD, and tooltips — Lua
  reaches them through the prelude (`edit`, `line`, `tooltip`,
  `window_buttons`, `latency_hud`, ...), C through `kui_*`
  widget calls with body callbacks for the containers, and a `tooltip =
  "hint"` prop on any container is hover-gated in every binding.
- **Events are data, not callbacks.** Nodes declare an `on_click` payload
  (`Value`: null/bool/int/float/str/list/map). Input resolves against the
  previous frame's layout and produces `UiEvent`s tagged with the origin that
  declared them; the runner routes host events to `App::on_event` and
  extension events back into the script — or the shared library — that owns
  them. An extension is one trait (`name`/`slots`/`view`/`on_event`) over a
  borrowed frame, so Lua and C are two implementations of it rather than
  two mechanisms; where it draws is a slot the host declares, under a
  namespace the host decides, with parameters in and replies out
  ([ADR 0014](docs/adr/0014-slots-an-extension-fills-in-place.md)).
  Full keyboard input is data too: a node declaring `on_key` becomes a
  key sink, and while it holds key focus (`ui.take_key_focus`, or a
  click) every press arrives as `{kind="key", phase="down", code, mods,
  text, repeat}` — which is all a keymap needs. A sink that also declares
  `key_up` hears releases as the same shape with `phase="up"`, so a
  held-key interaction (WASD, press-and-hold to preview, a key that arms
  a mode) is a pair of events rather than a guess about timing; one
  payload shape with a phase, the way a drag has three and a hover two.
  A key only comes up where it went down: a release whose press the sink
  never saw is dropped, and focus moving — or the window losing the
  keyboard — delivers the release first, so nothing is ever left stuck
  down. Modal keymaps live in the
  app, in any language, with no runner hook (the `modal_editor` and
  `splitmux` examples are built on this). Modifier
  state is data too: the host gets `{kind="modifiers", shift, ctrl, alt,
  super}` whenever it changes, keeps it in its model, and lets the view
  react — splitmux floats drop-zone overlays over every pane while ⌘ is
  held, so a ⌘-drag moves a pane and a plain click still focuses it, with
  no "modifier-gated drag" concept in the core at all.
- **Accessibility is data too** ([ADR 0001](docs/adr/0001-accessibility-as-data.md)).
  `role` and `label` are two more schema rows; from them and from what
  nodes already do (`on_click`, editors, scroll containers, window chrome)
  the core derives an *access tree* — roles, names, rects, values, the
  actions each node accepts — with plain boxes elided, as data any driver
  can ask for (`Core::access_tree`, `app.accessTree()`, `kui_access_tree`).
  Requests from assistive technology come back in as `InputEvent::Access`
  and resolve like their pointer equivalents, so a headless test can drive
  an app the way a screen reader would. The Rust runner hands the tree to
  the platform through AccessKit, and only once something attaches; an
  unnamed control or an unlabelled image is a warning, not a mystery.
  Editors go one level deeper: their laid-out lines are *runs* with every
  character placed, so a reader walks text by character and word and
  hears the caret move. An app that owns its text (the modal editor
  example) gets the same treatment from `role="line"` rows and two byte
  offsets, with selection requests coming back as events.
- **Identity is content-addressed.** `Key` is a hash of the path from the
  root (labels/sibling indices), reproducible from any language, with no
  allocation event tying identity to a slot. Slotmap handles are used where
  they belong: long-lived registered resources (generational, `u64`-FFI-able).
- **Text lives in the core.** Shaping/wrapping caches are keyed by
  (content, style, scale) and survive resizes; the layout solver measures
  through a `TextMeasure` trait (stubbed in tests, cosmic-text in production);
  emission reuses positioned glyph templates until wrap width or the atlas
  epoch changes. Renderers receive pre-rasterized atlas quads only. Rich text
  is `Span` lists (color/bold/italic per run, and underline, strikethrough
  or a background per span — paint built beside the glyphs, one rect per
  line the span covers, so a background follows it across a wrap) shaped as
  one paragraph flow, so wrapping crosses style boundaries and emoji share
  baselines. A plain
  line of 4096 bytes or more — a minified bundle, a log line with a blob in
  it — is shaped in ~1 KB chunks as they come on screen, so it costs the
  screenful it shows and a keystroke into it costs the chunk it lands in;
  wrapped, its chunks stay shaped as they are and the rows are broken from
  their positions, each chunk's first row starting where the last one's
  ended, so a 100k-character paragraph costs the rows it shows (backlog
  C19). Line breaking
  is a style choice: `wrap` (word / glyph / none), `max_lines`, and
  `ellipsis` (a single "…"-terminated line unless `max_lines` says
  otherwise); unwrapped text takes its box's width and clips to it.
- **Text editing is retained state, not captured state.** An edit node's
  buffer/cursor/selection live in the core keyed by widget identity (cosmic-
  text's `Editor` underneath, so motion, selection, and click-to-caret share
  the shaping truth). Input arrives as data (`InputEvent::Text` / `Key`) routed
  to the focused editor; hosts get "changed"/"submit" events and read text
  back by key — no `&mut String` captured in a view, which is what keeps
  editing reachable from Lua and C. The runner maps winit keys, IME input
  (the composition lives inline in the buffer as a marked range, so text
  after it shifts and wraps while you compose, and the OS candidate window
  is anchored at the caret inside it), and platform clipboard shortcuts
  (arboard) onto those events; the caret
  blinks on the runner's clock and scrolls itself into view.
- **Keyboard focus is data** ([ADR 0002](docs/adr/0002-keyboard-focus-as-data.md)).
  The core keeps one focus for every node, and every control the access
  tree knows is a Tab stop in tree order: editors, `on_key` sinks,
  `on_click` boxes, the control roles, and any node declaring `focusable`;
  never a `disabled` node, decoration or window chrome. Enter and Space
  press the focused control (the event a click would emit), the arrows
  nudge a focused slider (the access events a screen reader would send),
  and keyboard focus draws a ring on top of the frame in every binding
  unless the node declares `focus_bg`. A click's focus draws nothing. A
  key sink keeps every key, Tab included — it is an app that owns its
  keyboard — and hands focus on with `focus_next`. A screen reader's focus
  request lands on the same focus, so its cursor follows Tab into
  buttons. Everything in this bullet is the *second* of the two channels
  one key press travels — the first is the raw press an `on_key` sink
  hears — so a driver sends both, in that order, and a headless test
  presses a key with the one call that does the same (`Core::press`,
  `ctx.press` / `app.press`, `kui_input_press`).
- **Focus regions are a row**
  ([ADR 0022](docs/adr/0022-focus-regions.md)). A node declaring
  `focusRegion` (`focus_region()` in Rust, `focus_region = true` in Lua,
  `KuiSpec.focus_region` in C) is a Tab ring of its own: the ring outside
  never enters it, and inside it Tab wraps over its controls alone — a
  devtools dock, an inspector beside the app. It is entered on purpose:
  `ui.focus_region(key)` (`ctx.focusRegion('devtools')`,
  `env.focus_region`, `kui_focus_region`) from the chord that shows it, a
  click on it, or a focus on a node inside; `focus_region(None)` comes
  back to what the main ring last held, and a region that goes away hands
  focus back by itself. Only the ring is scoped — keys bubble through to
  the sink above, and the pointer and assistive technology see a plain
  node. Two rules landed beside it: an `autofocus` editor asks once, on
  the frame its declaration starts, and a key sink on the root hears every
  key when nothing is focused, so a shell never takes focus to have
  somewhere for its chords to land.
- **Modal surfaces are a row**
  ([ADR 0003](docs/adr/0003-modal-surfaces.md)). A node declaring `modal`
  (`modal` in JSX, `modal = true` in Lua, `KuiSpec.modal` in C) is the
  frame's modal surface: the Tab ring becomes its subtree, focus enters it
  (at its first control, or at whichever one declares `initialFocus` — so a
  destructive confirm opens on Cancel) and comes back where it was when it
  goes away, and everything outside is inert — no click, drag, hover, wheel, or assistive-technology
  activation; only window chrome stays live, so a dialog never traps the
  window. The access tree marks it `aria-modal` (AccessKit's
  `set_modal`) and derives a `dialog` role. Escape and a press outside
  emit `{kind: "dismiss", reason}` on the node: the core closes nothing —
  the app stops declaring it, or asks first. The last modal declared in
  tree order is the one in effect, so a confirm inside a dialog stacks
  without a stack, and a modal that has to cover the app is a float (the
  core warns when it is not).
- **Transitions animate layout inputs, not rects.** A node with
  `transition(ms)` (`transition={150}` in JSX, `transition = 150` in Lua,
  `KuiSpec.transition_ms` in C) has its sizing amounts, colors, radius,
  opacity and shadow eased toward whatever the view declares, keyed by node identity and
  retained in the core like scroll offsets. Because the *inputs* to layout
  move, a whole subtree lays out consistently every frame — a split's two
  halves glide while their contents wrap to the real widths. The core stays
  clock-free: drivers inject time (`Core::set_time`, `kui_set_time`,
  `ctx.setTime`) and ask `animating()` whether another frame is owed; a
  headless driver that never sets time gets snapping, and a node's first
  frame or a frame without the transition snaps too, so nothing animates in
  from nowhere and a divider drag doesn't replay when it ends. When a node
  *should* arrive from somewhere — a toast, a side panel — it says so with
  `enter` (`enter={{ dx: -320 }}` in JSX, `.enter(Enter::from(-320.0,
  0.0))` in Rust, `KuiSpec.enter` in C): on first sight the slots it names
  (an offset for the position, plus width, height, bg, radius, opacity) start there
  and ease to what the view declares, no staging frame needed, and a node
  that leaves and comes back enters again. A view that stages a starting
  state by hand instead (a new split drawn collapsed so it slides open)
  calls `ui.request_frame()` so the next frame comes without waiting for
  input. `slide`
  opts a node into easing its laid-out *position* too, subtree and all,
  which is what reordered siblings need (splitmux's tabs slide into their
  new order) and what a float whose `dx`/`dy` changes gets for free (its
  offset is a layout input like any other); it stays opt-in because a node
  whose position follows an already-easing sibling would lag twice, and an
  `enter` offset without it moves the node for the entrance only. Easings
  are timed curves or
  springs: `spring` / `bouncy` integrate a damped spring per frame with a
  velocity that survives retargets (`duration_ms` is the response time),
  so a value chased mid-flight keeps its momentum instead of restarting.
  For motion that never settles — a pulse, a cascade, a chase light — a
  node declares `keyframes`: CSS `@keyframes` stops for the same slots
  (`keyframes={[{ width: { grow: 0 } }, { width: { grow: 1 } }]}` in JSX,
  a `KuiKeyframe` array in C), cycled over the transition's duration in
  CSS's `animation-direction` (`repeat="alternate"`) and held back by
  `delay` ms so siblings stagger. A keyframed slot is sampled straight off
  the clock rather than retained as a tween, so nothing drifts, siblings
  stay in phase with each other, and the view never wakes up to flip a
  target; slots the stops don't name still tween as usual.
- **Subpixel text where the GPU can blend it.** Glyphs are already placed at
  quarter-pixel x offsets (cosmic-text's subpixel bins); on top of that the
  core can rasterize outline glyphs as LCD subpixel coverage — three
  rasterizations a third of a pixel apart landing in r, g and b — and emit
  them as `GlyphSubpixel` quads. The wgpu backend requests dual-source
  blending when the adapter has it (Metal, DX12, most Vulkan) and blends
  per channel: the fragment shader outputs premultiplied color plus a
  per-channel coverage, `out = src + dst * (1 - coverage)`. The runner turns
  subpixel rasterization on only when the renderer reports that capability
  (`kui::app(..).text_aa(TextAa::Grayscale)` or `KUI_TEXT_AA=gray` opt out);
  headless contexts and C hosts stay grayscale unless they ask
  (`kui_set_subpixel_text`), and a renderer without per-channel blending
  still draws subpixel quads correctly from their union coverage. On a 2×
  display the difference is subtle by design; it is 1× panels that gain.
- **Window chrome is data, both directions.** Any node can declare a chrome
  role (`window_drag()` / `window_button(...)`); interacting with it produces
  `WindowCommand`s (start drag, close, minimize, toggle maximize) that the
  frame driver drains and applies — the core never touches a window. Host
  facts flow the other way through `env.window` (custom chrome? maximized?
  where do the macOS traffic lights sit?), so `widgets::titlebar` adapts per
  platform by itself — and so does an app that would rather write its own:
  `ui.env()` in Rust, `env.window` in Lua, `ctx.env().window` in Node
  (`ctx.setEnv()` declares them headlessly), `kui_env_set_window` in C. Opt in with `kui::app("title").custom_titlebar().run(app)`:
  macOS keeps native traffic lights over your content; Windows/Linux go
  undecorated with drawn buttons. On Windows the runner also subclasses the
  window and answers `WM_NCHITTEST` from the frame's chrome regions
  (HTCAPTION / HTMINBUTTON / HTMAXBUTTON / HTCLOSE + resize borders), so
  snap layouts, native caption drag, and double-click maximize all work over
  the drawn controls; Linux falls back to synthesized edge resizing and
  double-click maximize. Lua declares `window = "drag"` etc.; C sets `KuiSpec.window_role`
  and drains `kui_take_window_command`. Windows themselves are declared the
  same way (`docs/adr/0004-multi-window.md`): `ui.window("palette", cfg)` in
  Rust, `windows: (model) => [...]` in a Node loop, `windows = { ... }` on a
  Lua root table, `kui_window_declare` in C — a window opens on the first
  frame that declares it, closes on the first that does not, and `view`
  runs once per open window with `ui.window_name()` saying which; the same
  drain carries the `Open` / `Close`, and the app hears
  `{kind="window", phase, name, id}`. What a declaration does *not* carry is
  a live window's geometry: a config is read on the frame it opens and never
  again, because the user owns a window's size once it exists, so moving one
  is an explicit request — `ui.set_window_size(window, size)` and
  `ui.focus_window(window)` (`setWindowSize` / `focusWindow` in Node,
  `kui_set_window_size` / `kui_focus_window` in C, `env.set_window_size` /
  `env.focus_window` in Lua), queued into that same drain for the driver to
  apply on its next pump. The window's *size* travels the same
  way: a frame begun at a different viewport or DPI than the last one posts
  `{kind="resize", width, height, scale}` on the root, routed with the frame's
  other pending events — `App::on_event` in Rust, `pollEvents` in Node, `kui_poll_event`
  in C. As a query it is `KuiWindow.size()` / `PumpRunner::window_size()`,
  which answer before the first frame too (`Core::viewport()` after it).
  What the user may resize *to* is a launch option: `kui::app("t").min_size(420.0,
  320.0).max_size(1600.0, 1200.0)` (`minWidth` / `minHeight` / `maxWidth` /
  `maxHeight` in the Node `WindowOptions`, where either half of a pair may
  stand alone). The OS enforces the bounds — including the synthesized edge
  resizing under custom chrome — and the initial size is clamped into them,
  so the pre-first-frame `window_size()` never reports a size the window
  cannot have; where the two bounds cross, the minimum wins.
- **Pointer state is declared, not queried.** A node says what it looks
  like while hovered or pressed (`hover_bg` / `pressed_bg`; JSX `hoverBg`,
  Lua `hover_bg`, `KuiSpec.hover_bg`) and the core swaps the color in when
  the node opens, so a data-only view — JSX re-encoded between pumps, a Lua
  table, flat C calls — gets hover feedback with no round trip and no
  `is_hovered` in the view; with `transition` the swap eases. `hover_group`
  ties nodes together (a two-piece elbow, a split button lights up as one).
  `widgets::button`, `<button>` and `kui_button` are all that same data
  (`widgets::button_spec()`). When hover must change *layout* — a close
  button that appears — `on_hover` emits `{kind="hover", phase="enter"|
  "leave", tag}` events like any other interaction, including when a new
  frame moves a node under a still cursor (`Core::take_pending_events`,
  routed by every driver after a frame). `is_hovered` / `is_pressed` stay
  as queries for Rust and Lua views and are mirrored on `KuiWindow`.
- **Measurement and layout are data, in that order.** "Declare it, the
  core resolves it" is a strategy of enumeration, and the first behaviour
  nobody enumerated needs a way out that is not an imperative hook. The
  way out is the numbers layout already computes, handed back as data.
  Before layout, `measure_text` (`ui.measure_text(s, &style, max_w)`,
  `ctx.measureText(content, style, maxWidth)` in Node, `env.measure_text`
  in Lua, `kui_measure_text` in C) answers what a string will take —
  width, height, lines — with the same shaping the node will use, wrapped
  to a width when asked, so a breakpoint table is arithmetic on the labels
  rather than constants found by screenshot, and it follows the font. After
  layout, a node that declares `on_layout` (`onLayout` in JSX, `on_layout`
  in Lua, `KuiSpec.on_layout` in C) gets its rect back as
  `{kind="layout", x, y, w, h, parent, tag}` on its first frame and
  whenever it changes — never on a frame that left it alone, so storing it
  in the model and redrawing does not loop; a `slide` reports every frame
  it moves. Together they cover the case that has no declared prop yet:
  a popover under a word is the measured prefix as a float offset, a
  minimap is the layout events of the panes. Line and glyph boxes inside a
  paragraph are the next payload on this road, not a different road.
- **A node can leave, not just arrive.** `enter` says where a node's slots
  start the first frame it is seen; `exit` — the same declaration read the
  other way, `{ dx, dy, width, height, bg, radius, opacity }` — says where
  they end the frame after the view stops declaring it. The view does not
  keep a dead node around to animate it away: the core copies the departing
  subtree out of the last frame that had it and replays *that*, frozen where
  layout left it, painted on top of everything and outside every clip its
  ancestors held (they may be gone; the clips inside the picture stay),
  and **inert** — no clicks, no Tab stop, no
  access row, because it is a picture of a node rather than a node. The
  ghost is dropped when its transition ends, and immediately if the key
  comes back, so a toast dismissed and re-shown never doubles. It is opt-in
  per node and needs a `transition`; without both, a removed node vanishes
  at once as it always did, and no more than
  [512 nodes](docs/adr/0005-the-paint-vocabulary.md) may be departing at
  once. That budget is judged [per frame and whole](docs/adr/0012-the-exit-budget.md):
  a removal that does not fit beside earlier exits takes the room from the
  oldest of them, and a removal larger than the budget on its own does not
  animate at all — every node of it vanishes at once, rather than half a
  list sliding out and the rest blinking — and an `exit-budget` warning
  says so, since a list dropping a thousand rows wants `exit` on the list
  and not on every row. `animating()` stays true while a ghost is in
  flight, so the driver keeps drawing until it is done and then idles.
- **Diagnostics are data.** The failures that used to be silent — a
  `Grow(2)` that is the only grow child (or grows across the parent's main
  axis) and so has no weight to split, a `transition` on an auto-keyed child
  whose siblings changed count so it snapped, two nodes on one key — are
  `Warning { code, key, message }`s the core raises while finishing a
  frame, each distinct (code, node) once (the checks walk the tree, so
  they run on the first frames and every 16th after — a misconfiguration
  persists, so it surfaces within that, at no steady-state cost).
  A prop name no table
  claims (`unknown-prop`) is the one code a frame cannot show, since the
  name is gone before the tree exists — so the binding that dropped it
  raises it, through `Core::warn`, behind the same gate and dedup, with
  the spelling it was probably meant to be.
  `Core::take_warnings` drains them;
  the windowed runners print them, the Node loop collects them on
  `app.warnings` either way, C drains `kui_take_warnings`, and a test
  asserts the list is empty. Every code, with what it means, is the
  Warnings table in [docs/props.md](docs/props.md#warnings) — generated
  from `diag.rs`, like the prop tables, so it cannot lag the core. They
  are a development aid, so the drivers decide by build: the
  Rust runner runs them in debug builds only (`Launcher::diagnostics`
  overrides), the Node loops unless `NODE_ENV=production`, a standalone C
  context not until `kui_set_diagnostics`; a bare `Core` has them on, since
  a headless test is development by definition. Correct behaviour that
  looks like a bug gets a sentence instead of a regression report.
- **Corners are four radii.** `VisualStyle::radius` is `[tl, tr, br, bl]`:
  `.radius(r)` rounds all four, `.radius_tl(r)` / `.radius_top(r)` / ...
  override some (later wins, like CSS shorthand then longhand; JSX
  `radiusTL`, Lua `radius_tl`, C `radius_tl` + `per_corner`). Quads carry all
  four and the SDF picks the corner's radius per fragment, so a tab, a
  header or an LCARS elbow is one box, and transitions ease each corner on
  its own.
- **Fading and lifting are paint props, and honest about their limits.**
  `opacity` (0..1) fades a node *and its whole subtree*: it multiplies down
  the tree and into the alpha of every quad the subtree emits — box, border,
  glyph, image, scrollbar, focus ring. It is a per-quad multiply, not an
  offscreen composite, so a subtree whose own pieces overlap shows its seams
  through the fade; and it changes nothing but paint, so a faded subtree
  still lays out, still takes clicks and is still read out (CSS's rule for
  `opacity: 0`, and the one that makes fading a live panel usable). It eases
  with `transition`, so `enter={{ opacity: 0 }}` fades a whole panel in, and
  `exit={{ opacity: 0 }}` fades it back out.
  `shadowColor` + `shadowBlur` / `shadowX` / `shadowY` / `shadowSpread` cast
  one drop shadow behind a node: the core emits the shape already offset,
  spread and inflated, and the shader softens the same SDF it uses for
  rounded rects, so a shadow is one more quad in the same draw call rather
  than a blur pass. The color is the switch — nothing draws without one.
  Outer shadows only, one per node, and the shape is not knocked out of the
  middle, so a translucent background shows its own shadow through itself.
  Both are decided in [ADR 0005](docs/adr/0005-the-paint-vocabulary.md),
  which also says why there are no gradient *props*.
- **A fragment is a box a WGSL function paints.** What ADR 0005 declined to
  build one prop at a time — gradients, rings, noise, shimmer — an app
  writes as one function instead
  ([ADR 0015](docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md)):
  `add_fragment(wgsl)` validates the source and hands back a handle, and
  `<fragment src params animate>` draws a box with it. The app writes
  `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>`
  and nothing else — kui owns the vertex stage, the node's rounded box, the
  inherited clip, the group opacity and the blend, so a fragment cannot
  look unlike a kui node. It lays out, takes input and holds children,
  which paint over it. Sixteen positional floats go in, the frame clock and
  the node's size come with them, and `animate` asks for a frame every
  frame. What it cannot do — multi-pass, geometry, compute, reading what is
  behind it — is the `painter` the ADR names and does not build.
- **Fonts are registered resources.** Beyond the generic sans / serif /
  mono families, `Core::load_fonts_dir("fonts")` / `load_font_file(path)` /
  `add_font_data(bytes)` load TTF/OTF/TTC files into the font database and
  `Core::add_system_font("Antonio")` names a family — installed or just
  loaded (`system_font_families()` lists them); all hand back a `FontId`
  slotmap handle for `TextStyle::font(id)` — JSX `<text font={id}>` via
  `ctx.addFont` / `addSystemFont`, Lua `font = id`, C `KuiTextStyle.font`
  via `kui_font_add*`. The shaping cache keys on the handle, editors shape
  through it too, and a removed font's stale handle shapes as sans rather
  than aliasing whatever took its slot.
- **Audio is data too.** Sounds are registered resources
  (`Core::add_sound(bytes)` → `SoundId`, any wav/ogg/mp3/flac), and playing
  one is a command the frame driver drains (`AudioCommand` via
  `take_audio_commands`) — the core never touches a device, so headless
  tests assert on the queue the way they assert on window commands. Three
  ways in: `click_sound` / `hover_sound` props on any node (schema rows, so
  JSX, Lua and C get them for free); an `audio` node — a playback retained
  by key: present means playing (once, or looped), gone means stopped,
  `volume` / `paused` apply live, a changed `src` restarts, like an HTML
  `<audio autoplay>`; and `play` / `stop` / `set_volume` / `pause` /
  `resume` / `set_master_volume` for hosts holding the core. A playback
  started with a tag that finishes on its own comes back as
  `{kind="sound", phase="ended", playback, tag}`. The runner plays through
  kira/cpal behind the default-on `audio` feature, opening the device on
  the first sound (no audio thread for silent apps; a missing device logs
  once and the UI runs on). Volumes are linear amplitude; `kui::audio::blip`
  / `wav_pcm16` synthesize test sounds without asset files.
- **The C API is translation, not architecture.** Frame building is flat
  calls on one opaque context (`kui_open`/`kui_close`/`kui_text`), payloads
  are opaque `KuiValue` handles with accessors, and `kui_draw_data` hands out
  the `repr(C)` quad list + atlas directly. `kui_run` drives the windowed
  runner through two C callbacks. The builder state living in `Core` (not in
  a borrowing wrapper) is what makes this a thin layer.
  The ABI is versioned: check `kui_abi_version()` against the header's
  `KUI_ABI_VERSION` before your first other call, and start every struct
  the library writes into from its `KUI_*_INIT` — those lead with a `size`
  you set, so a later kui that appends a field writes no further than what
  your build reserved. `kui.h`'s "Who writes what" block tags every struct
  `[in]`, `[out]`, `[out[]]` or `[lib]`, which is what says whether adding
  a field to it is free or fatal ([ADR 0006](docs/adr/0006-c-abi-versioning.md)).
- **The renderer boundary is a flat quad list.** A backend implements "mirror
  this RGBA atlas" and "draw these quads" — the wgpu backend does the whole UI
  in one instanced draw call with an SDF shader for rounded rects/borders.

## Layout

Clay-style passes over the flat tree (parents precede children):
fit widths ← · grow widths → · fit heights (text wraps here) ← ·
grow heights → · positions →. Sizing: `Fit`, `Grow(f)`, `Fixed(px)`,
`Percent`, plus per-axis `min`/`max` clamps (so `Grow` + `max_width(560)`
gives "track the window, cap at reading width" and text rewraps on resize);
row/column direction, padding, gap, start/center/end alignment on both axes.

Wrapping: `.wrap()` (`wrapChildren` in JSX, `wrap_children` in Lua) breaks a
row's children onto more lines when they don't fit the main axis, with
`.cross_gap()` between the lines — a tag list, a chip toolbar, a button row
that reflows when the window narrows. A row that happens to fit lays out
identically to one that never wraps, node for node, so the flag is safe to
leave on: main alignment places each line the way it placed the single run,
and the lines share the container's leftover cross space equally (CSS's
`align-content: stretch`), which with one line composes back into the plain
placement. Wrapping and shrinking are two answers to the same overflow, and
wrapping goes first — a child that can move to the next line moves rather
than being squeezed, and only a child too wide to fit a line on its own
falls through to the shrink pass, on that line alone. Rows only: breaking
needs a definite main size, and the pass order gives a row one (its width is
final before its height is measured) where a column's arrives two passes too
late; `wrapChildren` on a column, or on a `scroll_x` row, lays out as if it
were absent and raises a `wrap-ignored` warning.

Out-of-flow: `.float(FloatConfig)` takes a node out of flex flow — it doesn't
consume space in its parent, positions by attach points against its parent's
rect or the viewport (plus an offset), sizes Grow/Percent against that anchor,
and escapes ancestor clips. It paints as a layer of its own: above the in-flow
tree and every float that opened before it, under every float that opened
after — a tooltip that appears over an open menu is over it, a menu that opens
while a tooltip shows is over that — and it hit-tests in the same order, so a
popover over a scroller's bar takes the press there. A scroller's bars and
the focus ring are the chrome of the layer that owns them, above its content
and under the layers over it; there is no `zIndex`, and re-keying a float
reopens it on top
(`docs/adr/0023-layers-stack-in-the-order-they-open.md`).
`FloatConfig::below()`/`above()` give tooltip placement in one call
(`widgets::tooltip` wraps it); `FloatConfig::viewport().at(End, End)` pins a
HUD to a corner.

Over-constrained: when in-flow children overflow the main axis and it doesn't
scroll, `Fit` children shrink toward their `min` (default 0), largest first —
equal children end up equal, clay-style. `Fixed`/`Percent` keep their declared
size; text and images shrink in width (rewrap / re-aspect) but never height.

Overflow: `.clip()` clips children; `.scroll_y()` / `.scroll_x()` make a
container scrollable (wheel/trackpad, offsets retained across frames by widget
key, clamped to content). Scrollbars are live: thumbs drag, track presses
jump, hovered bars widen. They are overlays, and per node they are four
rows: `scrollbar` is `visible` (the default), `hidden` (no thumb, no track
to press — the wheel still scrolls) or `auto` (shown while the scroll state
changes or the pointer is on the track, gone a second and a quarter after
it stops, the way a macOS overlay bar goes; needs the driver's clock);
`scrollbarWidth`, `scrollbarColor` and `scrollbarActiveColor` restyle the
thumb, whose defaults are 4 px and the theme's two roles. An app reaches
the same offsets by name:
`ui.reveal(key)` scrolls whatever contains a node so the node shows (what Tab
does to the control it lands on), and `ui.scroll_offset(key)` /
`ui.set_scroll(key, offset)` read and write a container's offset — the next
layout clamps a written one, so `Vec2::ZERO` is "jump to the top" without
knowing the content height. Clip rects ride on each quad and are applied in
the shader, so the whole UI is still one draw call. Fully clipped nodes are
culled from both drawing and hit-testing.

Long lists: culling saves the *drawing*, not the building — a view that
declares 10k rows lays out 10k rows. `ui.scroll_geometry(key)` hands back
what the last layout resolved for a container (its box, its content size,
where it is scrolled to and how far it can go), which is everything a view
needs to declare only the rows that can be seen. `widgets::virtual_column`
is that for uniform rows: visible rows, two of overscan, and two spacers
holding the space of the rest, so the content height, the scrollbar and
`set_scroll` behave as if the whole list were there. 10k rows go from
~3.7 ms a frame to ~16 µs, and 100k rows cost the same ~16 µs. Rows are
opened at their data index (`ui.with_indexed`), so a row keeps its hover,
focus and edit buffer as the built range slides over it.

Text the app owns: a point on it is a byte offset, and a byte offset is a
caret rect. `ui.text_hit(key, point)` answers `{byte, line}` for the text
a keyed node drew — `point` being the logical viewport `x`/`y` a click or
drag event carries — and `ui.caret_rect(key, byte)` the zero-wide,
one-line-tall rect where a caret, a selection edge or an IME candidate
window goes. A node holding several text runs (a `line` row of syntax
runs, some wrapped in a selection box) answers across them in tree order,
the way the access tree reads a `line`, so a custom editor turns a click
into a caret with one call instead of measuring prefixes or assuming a
cell width. Both answer from the frame that finished — between frames the
layout the pointer was over, and during a build the last one — and from
the cache entry that frame already shaped, so a query costs a lookup.
Node spells them `textHit` / `caretRect`, Lua `env.text_hit` /
`env.caret_rect`, C `kui_text_hit` / `kui_caret_rect` with `KuiTextHit` /
`KuiCaretRect` out-structs (backlog C18).

A terminal's screen is one node: `ui.cells(&CellGrid { rows, cols, cells,
style, cursor })` draws `rows × cols` sixteen-byte cells — a character,
colours, attribute bits — by looking each glyph up in a table filled by
shaping that character once, placed at `col × cell_w`, never shaped again;
so a pane whose every cell changes every frame costs what a still one does
(~60 µs for 200 × 50, against ~2.2 ms as text nodes). `<cells>` in JSX takes
the cells as a `Uint32Array` of four entries each, Lua's `cells {}` a string
per row plus colour runs, C's `kui_cells` a `KuiCell` array; a click or drag
on the grid carries `cell: {row, col}`, an `onKey` makes it the terminal's
sink, and its access role is `terminal` with the screen as its value
(backlog C20).

Font features ride the style: `TextStyle::features(FontFeatures::parse("liga=0
calt=0"))` — `features="liga=0 calt=0"` in JSX and Lua, `KuiTextStyle.features`
in C — keeps a coding font from joining `->`, and `"tnum"` lines figures up in
a gutter; unset, the font's own defaults apply (backlog C23).

An editor the app owns hears an IME the way the stock editor does: while
a composition is under way the focused `onKey` sink gets `{kind:"preedit",
text, cursor, tag}` to draw inline, the commit arrives as `{kind:"text",
text, tag}` — the one committed text a `key` event never carries — and the
OS candidate window is anchored at the `line` carrying `caret` (backlog
C17). `ctx.preedit` / `ctx.commit` drive it headless in Node,
`kui_input_preedit` / `kui_input_commit` in C, and `kui_ime_rect` is where a
C host places the window.

Data from another thread: the windowed loop parks between events, so a
PTY reader, a file watcher, an LSP client or a socket that changed what
`view` will show has to say so. `App::setup(waker)` hands the app a
`kui::Waker` once, before the window opens; it is `Clone + Send`, and
`waker.wake()` from any thread asks every window for a frame — the path a
key press takes, minus the event, coalesced by the loop's queue so a
thread waking a thousand times a frame costs one (backlog C21). A host
that owns the loop gets the same from `PumpRunner::waker()`, and
`pump_until(deadline)` parks on OS events, wakes and the deadline together
instead of polling on a timer. The `waker` example is a thread feeding
lines every 40 ms; nothing else touches the window, and it draws.

Dragging: `.on_drag(tag)` makes any node a pointer-captured drag source —
handlers get `{kind="drag", phase, x, y, dx, dy, parent, tag}` events.
`dx`/`dy` are the displacement from the press point in every phase —
`start` is zero, a `move` is how far the pointer is from where it pressed,
`end` is the whole distance — so a handler sets `value = start + dx` rather
than summing, and can commit from `end` alone; the parent rect turns
absolute positions into container fractions, e.g. a splitter ratio (see the
splitmux example's pane dividers). A drag past the click slop (3 px from
the press) suppresses the node's `on_click`.

Pointer buttons: only the primary one presses, drags, places the caret and
clicks. A secondary (right) press goes to `on_context_menu`, which emits
`{kind="contextmenu", x, y, tag}` — logical viewport coordinates, i.e. where
the menu goes — and moves nothing else: no focus, no caret, no click, so
right-clicking a selection keeps it. It is routed like a click, so the
topmost node under the pointer answers. The menu is then an ordinary
`modal` float the app declares and stops declaring on `dismiss`; the C
counter example builds one. Drivers pass the button through
(`kui_input_mouse_button`, `ctx.mouse(down, clicks, "secondary")`,
`InputEvent::MouseDown { button, clicks }`), and `kui_input_mouse` /
`ctx.mouse(down, clicks)` still mean the primary one.

Menus are data, and there are two of them. A **context menu** is a list of
`MenuItem`s and a point (`ui.open_menu`, `ctx.openMenu`, `kui_open_menu`);
an **application menu bar** is a list of menus of the same rows, handed to
one call in the view (`<menuBar menu={…}/>`, `menu_bar { menu = … }`,
`kui_menu_bar`, `widgets::menu_bar`) —
[ADR 0017](docs/adr/0017-selection-as-a-scope.md) and
[ADR 0018](docs/adr/0018-a-menu-bar-the-app-declares.md). A row is a label,
an optional `id` the choice posts back, an accelerator to draw, `enabled`,
`checked`, and a `role`: the standard roles (`copy`, `cut`, `paste`,
`selectAll`, `lookUp`) the core performs itself, handing the clipboard half
to the host as a `MenuAction`, and `custom` is the app's own. Either way
choosing a row is one event, `{kind:"menu", role, item}`, so an app wires
Save once and gets it in both places.

Where they are drawn is the platform's business and not the app's. A host
says what it owns — `set_native_menus` for the context menu (macOS's
`NSMenu`, for the Look Up and Services rows nothing can draw),
`set_native_menu_bar` for the bar (macOS's, which is not in any window) —
and the core draws whatever is left: `widgets::context_menu` and
`widgets::menu_bar`, the second of which draws *nothing* where the platform
has a bar — so the call still says what the menu is, the strip simply is not
there, and one view is portable. The drawn bar keeps its own open menu, hovers across its
titles the way a menu bar does, and closes on Escape or a press below it;
the platform's binds the accelerators its rows declare. `cargo run -p kui
--example menu_bar` is both, and `--example context_menu` the menus a
right-click gets.

Images: register RGBA pixels once (`resources.add_image`), then `ui.image(id,
spec)` draws them through the same atlas page and draw call as glyphs (the
page doubles up to 4096² when needed). `Fit` takes the pixel size, a `Fit`
height against a resolved width keeps aspect, and `style.radius` (all four
corners, or per corner) rounds them.

Lines: `ui.line(from, to, Stroke::new(width, color), spec)` draws a
round-capped segment, `ui.polyline(&points, stroke, spec)` a polyline, and
`Stroke::curve()` a smooth curve through the points, flattened in the core so
every binding gets the same segments
([ADR 0010](docs/adr/0010-a-segment-primitive.md)). Points are in the
parent's box space — the space a floated sibling's offset is in — and a line
is never in layout: it floats, sized to its own bounding box, takes no room
in a row or column, and paints in the float pass in tree order, so a
connector goes under two cards by being declared before them. Its colour is
the node's `bg` slot, which is what makes `transition`, `enter` and `exit`
reach it; it takes no pointer input and has no access row.

## Performance

`cargo bench -p kui-core`, measured 2026-09-07 on an Apple M3 Pro MacBook Pro
(macOS 26.6.2, rustc 1.98.0, release, steady-state warm caches — full frame:
build + layout + emit). The suite was run twice back to back and the second
run read; most rows reproduce to within ~3% that way. Two do not:
`frame_10k_rects_all_transitioning` and `frame_10k_rects_all_declaring_exit`
disagree with themselves by 5–6% run to run where every other row holds to
~3%, so their medians below should be read as **±10%**, not as three
significant figures. Nothing separated the passes — the same binary on mains
and on battery agrees to within 1%, and every other row held steady across all
of them. Measuring a row alone after an idle also reads lower than measuring it
inside the whole suite (`frame_10k_rects` ~715 µs against ~725 µs), so these
are the numbers the command above gives, which is the point of quoting them.
The grid benches all go through the same builder at 1920×1080 and differ only
in which props are switched on, so the difference between two of them is what
that prop costs.

| bench | what it holds | median |
|---|---|---|
| `frame_1k_typical` | 32×32 grid, every 8th cell a label, every 4th clickable — a "typical app" frame | ~112 µs |
| `frame_1k_typical_with_100_floats` | the typical frame with a hundred tooltips floating over it every frame — what a hundred layers on the float stack cost at rest (ADR 0023) | ~145 µs |
| `frame_10k_rects` | 100×100 plain rects, nothing switched on | ~729 µs |
| `frame_10k_rects_with_text_and_hits` | the same grid plus 1.2k texts and 2.5k hit regions | ~1.14 ms |
| `frame_10k_rects_with_access_tree` | that frame with `core.access_tree()` derived after it — what a frame costs while assistive technology is attached | ~1.57 ms |
| `frame_10k_rects_with_shadows_and_opacity` | the plain grid with only the paint props on: every cell casts a shadow under a faded root | ~818 µs |
| `frame_10k_rects_square_clip` | the plain grid with every row clipping, so all 10k cells inherit a clip | ~781 µs |
| `frame_10k_rects_rounded_clip` | the same with a radius on every clipping row, so each cell pays the per-corner intersect | ~810 µs |
| `frame_10k_segments` | 10k one-segment `line` floats — the same 10k quads as `frame_10k_rects`, so the gap between the two is what a segment costs over a box | ~760 µs |
| `frame_1k_curves` | 1k curves through eight knots each, re-flattened by chord length every frame — 35 segments a curve | ~257 µs |
| `frame_10k_rects_all_transitioning` | every cell declares a `transition` — nine retained tween slots each | ~1.93 ms |
| `frame_10k_rects_all_declaring_exit` | every cell also declares an `exit`, so the whole frame is kept for the next one to diff against | ~2.51 ms |
| `frame_10k_rects_one_exit` | the same 10k grid with a single cell declaring an `exit` | ~748 µs |
| `drop_1k_rows_plain` | 1k rows removed from the tree in one frame, no exits declared | ~59.3 µs |
| `drop_1k_rows_declaring_exit` | the same removal with exits declared — over the 512-node budget, so ADR 0012 refuses it whole: the diff and the count, and no copies | ~163 µs |
| `drop_500_rows_declaring_exit` | 500 rows with exits declared, under the budget, so all 500 are copied into the store | ~138 µs |
| `replay_a_full_depart_store` | replaying a saturated depart store (the 512-node budget) for one frame | ~13.3 µs |
| `frame_10k_chips_unwrapped` | 10k chips in 100 rows, one line per row | ~656 µs |
| `frame_10k_chips_wrapped` | the same tree with every row breaking onto several lines | ~821 µs |
| `deep_nesting_64_levels` | 16 chains nested 64 levels deep | ~78.7 µs |
| `list_10k_rows_naive` | a 10k-row list held at its middle, built row by row | ~3.94 ms |
| `list_10k_rows_virtual` | the same list through `widgets::virtual_column` | ~16.3 µs |
| `list_100k_rows_virtual` | 100k rows through the same widget | ~16.4 µs |
| `warm_50x200` (`--bench stream`) | fifty 200-column mono lines, the same every frame — a terminal pane at rest | ~85 µs |
| `stream_50x200_log` | the same pane with every line new each frame, thirty-word log vocabulary plus numbers | ~25 ms |
| `stream_50x200_random` | every line new and random printable ASCII, nothing for the shape-run cache to hit | ~64 ms |
| `long_line_100k_first_frame` (`--bench long_line`) | a 100k-character no-wrap line opened in a horizontally scrolling view — shaped in chunks as they show | ~18 ms (was 662 ms whole) |
| `long_line_100k_scroll` | a viewport's width of scrolling through it per frame | ~160 µs, a few ms when a chunk first shows |
| `long_line_100k_wrapped_first_frame` | the same line as a `wrap: word` paragraph in a vertically scrolling view — rows broken from the chunks' positions | ~36 ms (a screenful of rows is seventeen times the text) |
| `long_line_100k_wrapped_scroll` | a viewport's height of scrolling through it per frame | ~180 µs, ~9 ms on a frame that brings a chunk in |
| `long_line_100k_edit` | one character inserted in the middle, the view held there | ~160 µs (was 102 ms) |
| `cells_200x50_warm` (`--bench cells`) | a terminal's screen as one `ui.cells` node, unchanged | ~58 µs |
| `cells_200x50_streaming` | the same grid with every character new each frame | ~60 µs |
| `cells_200x50_as_text_nodes` | the same 10k cells as one text node each — the path an app had | ~2.2 ms |

What the pairs say. Deriving the access tree costs **~1.35×** the frame it
follows. Shadows under a faded root are **twice the quads** (20k against 10k)
for **~9%** more frame time, because most of a frame is build and layout
rather than emitting quads. Clipping costs ~8% over the unclipped grid and
rounding that clip costs ~3% more — the radius is nearly free once a node
clips at all. A `line` costs ~8% over a plain rect at the same 10k quads —
the line store, the float placement, the endpoint encoding — and flattening
is cheaper than the node it hangs off: 1k eight-knot curves cut into ~35k
segments cost ~270 µs, under a third of the 10k-node segment grid, because
per-node work is most of what a frame is and 35 segments ride on one node.
Wrapping every row runs **~1.25×** the same tree laid out one
line per row, and that is the worst case: a row that does not wrap pays
nothing, because the break, the per-line grow and the per-line alignment are
all behind the flag. An exit on one node out of 10k costs ~3% over the plain
grid, so the `any_exit` gate holds — it is declaring exits on *every* node
that triples the frame. And virtualisation is the one difference worth
orders of magnitude: 10k rows cost ~3.7 ms built row by row and ~16 µs
through the widget, with 100k rows costing the same ~16 µs, because the frame
stops growing with the data. The two `stream` rows are the shaper's, not
the tree's: a pane whose fifty lines are all new every frame costs 25–64 ms
because each line is shaped from scratch, and cosmic-text alone on the same
fifty lines measures the same — a cell grid that never shapes ASCII is
backlog C20. What those frames leave behind is bounded: the shaped-text
cache holds a byte budget (`Core::set_text_cache_budget`, 64 MB by
default) and evicts the least recently drawn entries past it, never what
the last frame drew, so the stream that used to park 3.6 GB of shaped lines
settles at the budget (backlog C16, `tests/text_budget.rs`).

**These numbers went the wrong way once, and this is where that is
recorded.** The four rows this table used to carry were measured on
2026-08-31 (`dabe671`) at ~70 µs, ~510 µs, ~740 µs and ~58 µs. Re-measuring
for the alpha.6 release found them 2.4–2.7× worse — a frame cost that had
grown a little at a time across ~10 feature commits, on benches that declare
none of the features. The cause was `NodeSpec`: it had reached **728 bytes**,
it is moved by value for every node a frame builds, and 31% of a frame was
going into `memmove`.

The cold fields are now behind four boxed groups (events, animation,
accessibility, hover styling), which puts `NodeSpec` at **224 bytes** and
`memmove` back below the profiler's noise floor. That recovered about
two-thirds of the regression — `frame_10k_rects` 1.37 ms → 788 µs — and every
bench in the table moved with it, including ones that touch none of those
fields. A second profile then went after the rest (2026-09-07): the passes
that had grown a per-node read for a feature the frame does not use — the
float, wrap and text walks in layout, the float check in emission, the
hover-style and transition early-outs in the builder — are now behind
tree-level flags or inlined branches, and the open chain carries a spec by
pointer instead of moving it at every call. That took `frame_10k_rects`
from 816 µs to 725 µs and the chips, clip and exit grids 10–14% with it. What
is still above the 2026-08-31 baseline (~1.4×) is measured and is not a pass:
a padded copy of the old struct at 224 bytes reproduces most of it, and the
profile puts it in the app's own builder chain, where a `NodeSpec` is
default-constructed and moved by value before the core sees it. C15 carries
the bisect, both profiles, the padding experiments and both fixes in
[docs/backlog/closed-2026-09.md](docs/backlog/closed-2026-09.md).
`size_of::<NodeSpec>()` now has a
test with a bound on it, so the next inline field has a number to fail against
rather than a release audit to wait for. A fat struct is not the only way to
lose a frame, though, so there is a second guard for the case that test cannot
see: `scripts/bench-check.sh` benches HEAD against the previous `v*` tag in a
worktree and fails if one of four frame benches is more than 10% slower. It is
run before tagging rather than in CI — the docker runner is too weak to
measure a frame and would false-fail — and it prints the table above with the
run's own medians, so re-measuring these numbers is that same command.

A built-in latency graph shows per-phase frame cost live —
`widgets::latency_hud(ui)` floats it in a viewport corner as a translucent
overlay (`latency_hud_at` picks the corner; `latency_graph` is the inline
form): the last 120 frames as stacked bars (input / view / layout / render /
vsync wait) against the display's frame budget (`env.refresh_hz`, 120 Hz
fallback), with a red cap on frames whose work exceeds it. The runner feeds
`core.stats` and `core.env` automatically; all examples show it.

Editing latency (`cargo bench -p kui-core --bench editing`, same machine and
day — one keystroke: applying the edit, then the full frame it causes, warm
caches). Each cell is the median of four runs, because a single run of the
100k row moves by ~10%:

| document | apply | frame | quads |
|---|---|---|---|
| 50 lines | 0.087 ms | 0.050 ms | 1860 |
| 500 lines | 0.090 ms | 0.052 ms | 1860 |
| 2k lines | 0.096 ms | 0.067 ms | 1860 |
| 10k lines | 0.130 ms | 0.140 ms | 1860 |
| 100k lines | 1.79 ms | 2.08 ms | 1860 |

So a keystroke costs well under a frame's worth up to 10k lines, and ~3.9 ms
at 100k. The quad count is flat because glyph emission is viewport-culled (a
huge document emits only the visible screenful), and single-line reshapes go
through cosmic-text's shape-run cache. This bench never regressed with the
frame benches above — at `dabe671` it measured 0.084/0.045 ms at 50 lines and
0.134/0.116 ms at 10k — because its frame is 1860 quads, not 10k nodes, which
was itself a clue that the cost was per node.

Layout solver, atlas packer, key scheme, event dispatch, editing,
measurement, layout events, diagnostics and the Lua binding are covered by
tests (`cargo test --workspace`); `npm test` in `packages/kui` covers the
Node encoder and the corpus scenes. [CHANGELOG.md](CHANGELOG.md)
lists per release what was added and, separately, what an app can delete.

## Releases

Tagged commits publish to the self-hosted Forgejo: the library crates
(`kui-core`, `kui-wgpu`, `kui`, `kui-lua`, `kui-ffi`) to its cargo registry
and [`packages/kui`](packages/kui) to its npm registry as `@qxuken/kui`, with the Node addon
prebuilt for linux-x64, linux-arm64, darwin-arm64, darwin-x64 and win32-x64 bundled
under `prebuilds/` (`native.cjs` picks the one matching the running Node;
`KUI_NODE_LIB` still overrides it, and an in-repo `cargo build` still wins
for development). Consumers point at the registries once:

```toml
# .cargo/config.toml
[registries.forgejo]
index = "sparse+https://drydock9.qxuken.dev/api/packages/qxuken/cargo/"
# Cargo.toml
kui = { version = "0.1.0-alpha.1", registry = "forgejo" }
```

```bash
# scoped on purpose: Forgejo does not proxy npmjs, so only @qxuken/* goes there
npm config set @qxuken:registry https://drydock9.qxuken.dev/api/packages/qxuken/npm/
npm install @qxuken/kui@alpha    # prereleases publish under their identifier as the dist-tag
npm create @qxuken/kui-node my-app   # or scaffold an app from the template
```

Every release so far is a prerelease, so `latest` and `alpha` point at the same
thing — the newest alpha — and `npm install @qxuken/kui`, `npm view @qxuken/kui
version` and `npm outdated` all answer with it. `npm view @qxuken/kui@alpha
version` is the query that answers even if `latest` is ever missing, which it is
on a package published before this was arranged: `npm view` defaults to `latest`
and against a package without one prints nothing and exits 0.

A range does not pin a prerelease. Both `^0.1.0-alpha.8` and `~0.1.0-alpha.8`
admit every later alpha of the same `0.1.0` — that is npm's own semver, not a
quirk of the two spellings — so a range is a floor, not a choice. An app that
wants the version it tested writes that version exactly (`"@qxuken/kui":
"0.1.0-alpha.8"`) and commits its lockfile; the lockfile is what holds either
way, and without one a range reinstalls as whatever is newest.

To cut a release: `scripts/set-version.sh 0.1.0-alpha.2` (workspace version,
the `kui-*` dependency requirements, package.json and the changelog's open
`(unreleased)` heading move together — registries refuse a version that already
exists), commit, `git tag v0.1.0-alpha.2`, then push the branch and the tag
in one go: `git push --atomic origin main v0.1.0-alpha.2`. That next
`## <version> (unreleased)` heading is opened by hand; the script only dates
the open one, and a tag whose top heading is missing, stale or still says
unreleased fails the release. [ci.yml](.forgejo/workflows/ci.yml) then runs
`check`, builds one addon per target in parallel, all on the one docker
runner (`build-linux`
through cargo-zigbuild with a glibc 2.28 floor; `build-windows` through
cargo-xwin against the Windows SDK; `build-macos` through cargo-zigbuild
against a copy of Xcode's SDK, whose Apple license applies), and `publish`
verifies the tag against the
manifests and the changelog heading, downloads the five prebuilds, runs the parity tests against the shipped binaries, publishes the
crates in dependency order and finally the npm package. It needs a repository
secret `PACKAGES_TOKEN` (a personal access token with `write:packages`) and
nothing but that Linux runner: no Mac or Windows machine is involved.

That last property is also the limit of what CI proves. The Windows non-client
chrome ([windows_nc.rs](crates/kui/src/windows_nc.rs)), the macOS traffic-light
inset in `widgets::titlebar_with` and the whole AccessKit bridge
([access_bridge.rs](crates/kui/src/access_bridge.rs)) are compiled and linked by
the release build and never executed by it — and neither of those two files
carries a test, so the headless suite pins the data they hand the platform, not
the platform's acceptance of it. Two optional jobs, `smoke-macos` and
`smoke-windows`, build and run `cargo test --workspace` natively against the
real SDK; both are gated on the repository variables `SMOKE_MACOS` /
`SMOKE_WINDOWS` and skip unless a runner with the matching label is registered.
They live in their own workflow, [smoke.yml](.forgejo/workflows/smoke.yml),
for the reason `audit` does: they share nothing with what makes `check`
expensive and are on no release path — and a job whose runner label matches
nothing does not fail fast in Forgejo, it queues, which in `ci.yml` would
leave a run without a result long after `check` and `publish` had finished.

`smoke-windows` does one thing more, and it is the only automated check in the
repo that opens a window: [scripts/smoke-windows.ps1](scripts/smoke-windows.ps1)
runs every windowed example on the runner's own GPU for 120 frames apiece, on
both theme bases, and fails on a crash or a hang;
[scripts/smoke-examples.sh](scripts/smoke-examples.sh) is the same round on a
unix host, and `--node` adds the Node windows. `KUI_SMOKE_FRAMES=n` is what
makes an example self-terminating, and any dev build honours it —
`KUI_SMOKE_FRAMES=120 cargo run -p kui --example fragment` is the same check
by hand. A release build ignores it unless built with `--features smoke`, so
that an app you ship does not close its own window over a variable its author
never asked about. Run it before a tag, on Windows above all: the first round
found three crashes and a dead feature that the headless suite passes straight
through (backlog W3–W6). Which example is in which round — windowed,
headless, by hand — is enrolled from the manifests and the example itself
rather than a list somebody keeps ([ADR 0021](docs/adr/0021-one-subject-per-example.md));
[scripts/smoke-headless.sh](scripts/smoke-headless.sh) prints the headless
round and `--run` runs it, which CI does.

Pushing the tag does not check the commit twice. The push to main and the push
of the tag that names it share a concurrency group keyed by the commit, and
only the tag run may cancel — so it takes over a `check` already running for
that commit, and in the other order (the tag handled first) the main run is
stood down by ci.yml's `gate` job instead of running a second one. With a
single runner that is the difference between one `check` and two before
anything is published. It needs Forgejo v14 or newer for the `concurrency`
block; the trade is that a run on main is no longer cancelled when a newer
commit is pushed to main, since the group is the commit rather than the branch.

The two refs go in one `--atomic` push for the same reason. Either order ends
with one `check`, but pushing main first and the tag a minute later means the
run the tag cancels has spent that minute compiling; sent together, the loser
is cancelled — or stood down by `gate`, which answers in about three seconds —
before it has done any work. Not `--follow-tags`: it pushes annotated tags
only, and the tags here are lightweight, so it would push main and silently
leave the tag behind. Name both refs.

What that looks like afterwards: the tag's run is the one that checks, builds
and publishes, and the run for the same commit on main ends as **cancelled**,
or as a three-second `gate` with `check` skipped. Both are the success case.

Before tagging, run the macOS accessibility audit by hand:

```bash
cargo build -p kui --example accessibility
./target/debug/examples/accessibility &
swift scripts/ax-audit.swift $!
```

36 checks over roles, names, values, the text protocol and the actions, asked
through the same API VoiceOver uses. It stays a manual step rather than a CI
job: it needs a logged-in GUI session for the window to exist, a Metal device to
draw it, and Accessibility permission for the calling terminal (System Settings
→ Privacy & Security → Accessibility). That last one is a TCC grant — per
machine, given by hand, and not scriptable without disabling SIP — so an
ephemeral runner can never hold one. A permanently self-hosted Mac with
auto-login and the grant already in place could run it inside `smoke-macos`;
nothing else can.

Advisories are checked separately, by
[audit.yml](.forgejo/workflows/audit.yml): `cargo audit --deny warnings` over
`Cargo.lock`, on pushes that touch the lockfile and on a weekly schedule —
weekly because an advisory is published against code that has not moved, so a
commit-triggered check would only find it the next time someone happened to
push. `--deny warnings` means an unmaintained or unsound crate fails the same
as a vulnerability; the only way to accept one is an entry in
[.cargo/audit.toml](.cargo/audit.toml) with a comment saying what pulls it in
and what would let the line be deleted. One entry stands today: `ttf-parser`
([RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)),
unmaintained, reached through cosmic-text's `fontdb`.

The job also passes `--no-yanked`. Checking for yanked crates costs one
sparse-index request per crate — 440 of them — and this runner's egress is
slow enough that most time out; a timed-out lookup is not a warning, so
`--deny warnings` cannot see it, and run 141 printed 175 `error:` lines and
reported success. That check is off on purpose rather than silently not
happening. A local `cargo audit --deny warnings` still runs it.

That job is deliberately not in `publish`'s `needs`. A release is cut from a
tag, and an advisory landing between the last green `main` and the tag would
otherwise block a release whose code nobody had touched — so read the audit
job before tagging rather than having it read for you.

## Status / next

What v0 does not do, by area, with the ADR or backlog entry each limit
belongs to.

**Paint.** Fill, border, four radii, group opacity and one outer drop shadow
per node ([ADR 0005](docs/adr/0005-the-paint-vocabulary.md)), and one stroke
primitive: a round-capped segment, which the `line` element emits one of per
straight piece of a segment, a polyline or a curve flattened in the core
([ADR 0010](docs/adr/0010-a-segment-primitive.md)), and one escape hatch:
the `fragment` element, a box a registered WGSL function paints
([ADR 0015](docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md)),
which is where gradients, rings, noise and shimmer live. There are **no
gradient props** (a stop list, a type, a geometry and an interpolation space
are not a paint prop's worth of work — write a fragment, or use an image),
no inset or
multiple shadows, and the single shadow is not knocked out of the middle of
the shape, so a translucent background shows it through. There are **no
paths, fills, dashes or arrowheads**: a line is segments and nothing else, a
translucent polyline double-blends where its caps overlap at a join, its
width does not tween (its colour does), and it takes no pointer input — a
shape-aware hit test is the same unbuilt change rounded hit-testing below
waits on, and a `line` that declares one warns. Opacity is a per-quad
alpha multiply rather than an offscreen composite, so overlapping pieces of one
faded subtree show their seams. There is no z-index: floats stack in tree order.
A fragment is one draw call of its own, so a hundred of them is about 0.7% of
a 120 Hz frame and ten thousand is the wrong tool; it cannot read what is
behind it (no backdrop blur), sample anything but its own parameters, run a
second pass, or hit-test per pixel — its edge is its box, like everything
else here.
Transitions cover sizing, colors, radius, opacity, shadows, position (`slide`,
`enter`) and departure (`exit`) — a node the view stops declaring is copied out
of the last frame that had it and replayed frozen, in its place and inert
until its transition ends. `exit` is opt-in per node, capped at 512 departing nodes at
once — a frame's removal past that animates whole or not at all (ADR 0012) — and a
ghost cannot be re-laid-out: `exit`'s `width`/`height` resize the
departing node's own box and nothing inside it moves.

A `radius` on a node that clips or scrolls rounds the clip too, so a rounded
card's children stay inside its corners. What that gives up is nesting (the
inherited clip is one rect and four radii, so a corner both clippers round
takes the tighter of the two, and a corner an ancestor's straight edge crosses
goes square) and hit-testing, which stays rectangular — a click in the corner
of a rounded scroll container still reaches the row under it.

**Theme.** The colours a view paints with are named roles, derived from the
two facts the OS reports — the appearance picks a base, the accent recolours
the family that comes off it
([ADR 0019](docs/adr/0019-a-theme-derived-from-appearance-and-accent.md), and
the Theme table in [docs/props.md](docs/props.md) for every role and its value
on both bases). The core still acts on the appearance exactly as much as it
did before, which is not at all; the **widgets** do, so a button, a context
menu, a tooltip, a field, the scrollbars, the focus ring and a `<text>` with no
`color` all follow the OS without an app writing a line about it. `ui.theme()`
in Rust, `env.theme` in Lua, `ctx.theme()` in Node, `kui_theme` in C — the
same twenty-three roles, generated from one table. Three sources, defaulting
to the OS for both facts: follow it, follow its light/dark with an accent of
your own (`set_accent`), or pin a palette that follows nothing (`set_theme`).
An unknown appearance takes the dark base, which is exactly what kui painted
before there were themes — so a host that reports nothing sees no change. What
this is **not** is a cascade: there is no inherited colour, no `var()`, no
numbered ramp. A role is read off `ui` and put in a `bg`, and an app's own
non-role colours — a highlighter's keywords, a chart's series — stay the app's.

**Layout.** Wrapping is rows only, for the pass-order reason above: a
**column** that outgrows its height is still one line, so it shrinks its `Fit`
children toward their `min` (or overflows) rather than moving anything into a
second column (C12). `Dir` is `Row` or `Column` with no reverse. Beyond that
the alignment vocabulary is start/center/end and nothing else — no
`align-content` (a wrapping row's lines always share the leftover cross space
equally), no `space-between` / `around` / `evenly` on either axis (a `grow`
spacer node covers the first of the three), and no baseline cross-alignment, so
two text sizes on one row align by box and sit on different lines (C13). There
is no aspect ratio either: "square" or "16:9" needs one of the two dimensions
known (C14).

**Input.** Pointer buttons: the secondary one is routed to `on_context_menu`
and nothing else (C2); the middle button and anything past it (back, forward)
reach the core as data and route nowhere, so there is no middle-click-to-close,
no right-drag and no per-button `on_click`. Touch and pen are not input modes
of their own: a finger on a touchscreen arrives as whatever the platform
synthesises as mouse input, so a tap presses and clicks and nothing past that
exists — no multi-touch, no pinch, rotate or two-finger gestures, no pressure
and no stylus tilt (X3). v0 is desktop-first.

Keys are layout-resolved characters and a closed list of names, with the
US-QWERTY position beside them as `physical`, so a keymap can bind the finger
rather than the label ([ADR 0002](docs/adr/0002-keyboard-focus-as-data.md),
decision 11). The modifiers carry no left/right distinction, so a keymap
cannot tell the two Shifts apart, and a key that neither the layout nor the
position names is dropped rather than delivered as `unknown`. Presses and
releases route to the key sink and no further (C9): the core keeps no "which
keys are down" query, since the app that asked for the pair already has one.

A key sink owns its keyboard: while it holds focus every press is its data,
Tab included, and it hands the ring on with `focusNext` when it wants to
(ADR 0002, decision 3). A focused control owns only the keys the core
presses it with — Enter and Space where there is something to activate, a
slider's arrows, a composite's arrows, Home, End and type-ahead — and every
other press, chords included, walks up to the nearest enclosing sink
([ADR 0011](docs/adr/0011-keys-bubble-to-the-enclosing-sink.md)), which is
how an app shell keeps its shortcuts while the Tab ring works underneath
it.

The pointer shape is derived, not declared: the core resolves one per frame
from whatever is under the pointer, and the `cursor` prop overrides it — but
only from this list (`text`, `pointer`, `grab`, `grabbing`, `notAllowed`, the
four resize arrows and the default), so there are no custom bitmap cursors and
no hiding the pointer (C3).

**Focus and accessibility.** Accessibility, keyboard focus and modality are
data ([ADR 0001](docs/adr/0001-accessibility-as-data.md),
[0002](docs/adr/0002-keyboard-focus-as-data.md) and
[0003](docs/adr/0003-modal-surfaces.md)), and composites are derived from the
roles rather than declared
([ADR 0007](docs/adr/0007-composite-keyboard-patterns.md)): a tab list, radio
group, menu or picker list is one Tab stop with arrows, Home/End and type-ahead
inside it, and `initialFocus` says which control a modal opens on. The focus
ring is still not a prop — no node styles the default one, and the geometry is
fixed — but its *colour* is now `theme.focus_ring`
([ADR 0019](docs/adr/0019-a-theme-derived-from-appearance-and-accent.md)
revisiting ADR 0002): a pale blue ring reads on a dark page and is invisible
on a light one, and a focus indicator nobody can see is not one. ADR 0007's own follow-ups are the rest: grid navigation
(Left/Right into a row, Up/Down between rows, which wants a `grid` / `row` /
`cell` vocabulary), submenus, a `radio-without-group` warning, and
multi-select.

**Text and editing.** Editing covers caret blink, double/triple-click
word/line select, scroll-caret-into-view, inline IME composition, Tab focus
traversal, and undo/redo (operational deltas with typing/delete coalescing —
the widget owns its buffer, so it owns its history; hosts with their own text
model take raw chords through `on_key` and bring their own). Layout queries
stop at the node: `measure_text` and `on_layout` give whole-string and
whole-node rects, not the boxes of lines or glyphs inside a paragraph.

**Audio.** One-shots, loops, volume, pause and a finished-playback event.
Sounds decode fully into memory, and synthesis, effects, positional audio and
disk streaming are out of scope.

**Windows.** A frame declares which windows exist, by name, and the runner
opens them; `SetSize` and `Focus` are commands an app queues, because the
user owns a window's geometry once it exists
([ADR 0004](docs/adr/0004-multi-window.md)). A window is `kind: "normal"`
or `kind: "popup"` — a menu surface, borderless, off the taskbar, owned by
the window that declared it, placed in screen coordinates against an
`anchor` the app already has from `onLayout`, and non-activating, so the
field that opened it keeps its focus ring while the arrows walk the list. A
press outside it or Escape is the same `dismiss` a `modal` node gets and
closes nothing: the app stops declaring the window.

**A popup is the exception, not the default.** It costs an OS surface, a
swapchain, a `Core` and an accessibility adapter, where a float costs one
tree and one draw call — so a dropdown, a tooltip and a context menu stay
in-window (`FloatConfig::fit` flips across the anchor and clamps what still
overflows, a `modal` float takes the dismiss) until they provably do not
fit. The three cases that do not are what the kind exists for: a list
taller than the window, a menu near an edge with nowhere in-window to sit,
and a panel the user wants beside the app.

There is no window **position** an app can declare or read, no app-modal
window (decision 10 keeps modality per window), and no native menu bar.
Non-activating is a request, not a guarantee: on macOS a window that is
ordered front becomes key, so the runner hands the keyboard straight back
to the owner — the owner's `focused` stays true, but the popup does hold
key status for a moment.

The glyph atlas and the shaped-text cache stay per window (ADR 0004 step 1,
which amended decision 2 to say so): a `CachedText` entry stamps the atlas
epoch it was packed against, so it is only valid for that window's page, and a
second window re-rasterizes the same glyphs. Fonts, images, sounds and the one
audio queue are the session's and are shared.

## License

MIT, see [LICENSE](LICENSE).
