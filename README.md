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
| `kui-ffi` | C API (cdylib/staticlib + [include/kui.h](crates/kui-ffi/include/kui.h)): flat builder calls, opaque `KuiValue` payloads, `repr(C)` draw data, windowed runner via callbacks |
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
assert.deepEqual(app.ctx.audioCommands().map((c) => c.kind), ['setVolume']);
assert.ok(decodeQuads(app.ctx.quads()).every((q) => q.x + q.w <= 320), 'nothing overflows');
assert.deepEqual(app.warnings, []);                   // nothing misconfigured
```

A button inside a drag strip still takes its own click, a compact tier fits
its window with nothing overflowing, the chime plays when it should: all of
it is assertable because all of it is data the core emits rather than a side
effect it performs. Rust tests drive `Core` the same way
([crates/kui-core/tests](crates/kui-core/tests)), and C runs the same API
headless (`./examples/c/counter --headless`).

## Examples

```bash
cargo run -p kui --example counter        # pure Rust, Elm-ish flow
cargo run -p kui --example rich_text      # styled spans in one wrapped paragraph
cargo run -p kui --example editor         # multiline text editing: caret, selection, clipboard
cargo run -p kui-lua --example lua_panel  # Rust host + Lua panel sharing one frame
./examples/c/build.sh && ./examples/c/counter             # the same app from C
./examples/c/counter --headless           # C FFI self-test, no window needed
cargo run -p kui --example modal_editor   # helix-flavored modal editing; the app owns the keymap
cargo run -p kui --example splitmux       # tmux-style splits, tabs, focus, ⌘-drag pane moves; the pane tree is data
cargo run -p kui --example syntax_view    # syntax highlighting as coalesced style runs
cargo run -p kui --example gallery        # registered images: Fit sizing, kept aspect, rounded corners
cargo run -p kui --example toasts         # enter: toasts that slide in, a panel that springs open
```

The same app from Node with JSX — build the addon with
`cargo build -p kui-node --release`, then in [examples/node](examples/node)
`npm install && npm start` (headless) or `npm run window` (a real winit +
wgpu window, its event loop pumped from a timer so it shares the main
thread with libuv):

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
addon's own object walk (`frameObject`) and a JSON string transport
(`frameJson`) stay available as readable reference paths;
`npm test` in [packages/kui](packages/kui) checks that all three produce
byte-identical quads for every schema prop and element, and
`examples/node/bench.mjs` compares their cost. A one-off measurement of the
same view built through Node 26's experimental `node:ffi` module, calling the
C API in [kui.h](crates/kui-ffi/include/kui.h) directly, put ~1500 flat calls
per frame at ~0.24 ms against ~0.19 ms for the single binary-stream call,
while plain C over the same API takes ~0.08 ms — the boundary itself is
~30 ns/call, the rest is JS-side argument marshalling (raw addresses beat
Buffer arguments by ~30%), so the binary stream stays the default.

A Lua extension in full:

```lua
function view(env)
  if note_key then note = env.edit_text(note_key) end   -- read editors back by key
  return column { gap = 8, pad = 16, bg = 0x14161eff,
    text({ "count: ", { tostring(count), bold = true } }, { size = 20 }),
    row { tooltip = "adds one", button { label = "bump", on_click = { kind = "bump" } } },
    edit { key = "note", initial = "", width = "grow" },
  }
end

function on_event(ev)                     -- ev = payload + node_key
  if ev.kind == "bump" then count = count + 1 end
  if ev.kind == "changed" then note_key = ev.node_key end
end
```

## Reference

[docs/props.md](docs/props.md) is the cross-binding reference: every prop
with its JSX, Lua and C name, the composites, the elements, the event
payload shapes and the resource APIs. It is generated from the schema
(`npm run gen` in `packages/kui`), so it cannot drift.

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
  buttons, titlebar (plain or with custom content), window buttons, latency
  graph/HUD, and tooltips — Lua reaches them through the prelude (`edit`,
  `tooltip`, `window_buttons`, `latency_hud`, ...), C through `kui_*`
  widget calls with body callbacks for the containers, and a `tooltip =
  "hint"` prop on any container is hover-gated in every binding.
- **Events are data, not callbacks.** Nodes declare an `on_click` payload
  (`Value`: null/bool/int/float/str/list/map). Input resolves against the
  previous frame's layout and produces `UiEvent`s tagged with the origin that
  declared them; the runner routes host events to `App::on_event` and
  extension events back into the script that owns them.
  Full keyboard input is data too: a node declaring `on_key` becomes a
  key sink, and while it holds key focus (`ui.take_key_focus`, or a
  click) every press arrives as `{kind="key", code, mods, text}` —
  modal keymaps live in the app, in any language, with no runner hook
  (the `modal_editor` and `splitmux` examples are built on this). Modifier
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
  is `Span` lists (color/bold/italic per run) shaped as one paragraph flow, so
  wrapping crosses style boundaries and emoji share baselines. Line breaking
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
  buttons.
- **Transitions animate layout inputs, not rects.** A node with
  `transition(ms)` (`transition={150}` in JSX, `transition = 150` in Lua,
  `KuiSpec.transition_ms` in C) has its sizing amounts, colors and radius
  eased toward whatever the view declares, keyed by node identity and
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
  (an offset for the position, plus width, height, bg, radius) start there
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
  platform by itself. Opt in with `kui::app("title").custom_titlebar().run(app)`:
  macOS keeps native traffic lights over your content; Windows/Linux go
  undecorated with drawn buttons. On Windows the runner also subclasses the
  window and answers `WM_NCHITTEST` from the frame's chrome regions
  (HTCAPTION / HTMINBUTTON / HTMAXBUTTON / HTCLOSE + resize borders), so
  snap layouts, native caption drag, and double-click maximize all work over
  the drawn controls; Linux falls back to synthesized edge resizing and
  double-click maximize. Lua declares `window = "drag"` etc.; C sets `KuiSpec.window_role`
  and drains `kui_take_window_commands`. The window's *size* travels the same
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
- **Diagnostics are data.** The failures that used to be silent — a
  `Grow(2)` that is the only grow child (or grows across the parent's main
  axis) and so has no weight to split, a `transition` on an auto-keyed child
  whose siblings changed count so it snapped, two nodes on one key — are
  `Warning { code, key, message }`s the core raises while finishing a
  frame, each distinct (code, node) once (the checks walk the tree, so
  they run on the first frames and every 16th after — a misconfiguration
  persists, so it surfaces within that, at no steady-state cost).
  `Core::take_warnings` drains them;
  the windowed runners print them, `createApp` collects them on
  `app.warnings`, C drains `kui_take_warnings`, and a test asserts the list
  is empty. They are a development aid, so the drivers decide by build: the
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

Out-of-flow: `.float(FloatConfig)` takes a node out of flex flow — it doesn't
consume space in its parent, positions by attach points against its parent's
rect or the viewport (plus an offset), sizes Grow/Percent against that anchor,
paints on top of in-flow content, hit-tests topmost, and escapes ancestor
clips. `FloatConfig::below()`/`above()` give tooltip placement in one call
(`widgets::tooltip` wraps it); `FloatConfig::viewport().at(End, End)` pins a
HUD to a corner.

Over-constrained: when in-flow children overflow the main axis and it doesn't
scroll, `Fit` children shrink toward their `min` (default 0), largest first —
equal children end up equal, clay-style. `Fixed`/`Percent` keep their declared
size; text and images shrink in width (rewrap / re-aspect) but never height.

Overflow: `.clip()` clips children; `.scroll_y()` / `.scroll_x()` make a
container scrollable (wheel/trackpad, offsets retained across frames by widget
key, clamped to content). Scrollbars are live: thumbs drag, track presses
jump, hovered bars widen. Clip rects ride on each quad and are applied in the
shader, so the whole UI is still one draw call. Fully clipped nodes are culled
from both drawing and hit-testing.

Dragging: `.on_drag(tag)` makes any node a pointer-captured drag source —
handlers get `{kind="drag", phase, x, y, dx, dy, parent, tag}` events (the
parent rect turns absolute positions into container fractions, e.g. a
splitter ratio; see the splitmux example's pane dividers). A drag past the
click slop suppresses the node's `on_click`.

Images: register RGBA pixels once (`resources.add_image`), then `ui.image(id,
spec)` draws them through the same atlas page and draw call as glyphs (the
page doubles up to 4096² when needed). `Fit` takes the pixel size, a `Fit`
height against a resolved width keeps aspect, and `style.radius` (all four
corners, or per corner) rounds them.

## Performance

`cargo bench -p kui-core` (M-series MacBook, release, steady-state warm
caches — full frame: build + layout + emit):

| bench | median |
|---|---|
| 1k nodes, text + hit regions ("typical app") | ~70 µs |
| 10k plain rects | ~510 µs |
| 10k rects + 1.2k texts + 2.5k hit regions | ~740 µs |
| 16×64-deep nesting chains | ~58 µs |

A built-in latency graph shows per-phase frame cost live —
`widgets::latency_hud(ui)` floats it in a viewport corner as a translucent
overlay (`latency_hud_at` picks the corner; `latency_graph` is the inline
form): the last 120 frames as stacked bars (input / view / layout / render /
vsync wait) against the display's frame budget (`env.refresh_hz`, 120 Hz
fallback), with a red cap on frames whose work exceeds it. The runner feeds
`core.stats` and `core.env` automatically; all examples show it.

Editing latency (`cargo bench -p kui-core --bench editing` — one keystroke:
apply + full frame, warm caches): ~0.1ms at 50-10k lines, ~2ms at 100k lines.
Glyph emission is viewport-culled (a huge document emits only the visible
screenful of quads) and single-line reshapes go through cosmic-text's
shape-run cache.

Layout solver, atlas packer, key scheme, event dispatch, editing,
measurement, layout events, diagnostics and the Lua binding are covered by
tests (`cargo test --workspace`); `npm test` in `packages/kui` covers the
three Node transports against each other. [CHANGELOG.md](CHANGELOG.md)
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

To cut a release: `scripts/set-version.sh 0.1.0-alpha.2` (workspace version,
the `kui-*` dependency requirements and package.json move together — registries
refuse a version that already exists), commit, `git tag v0.1.0-alpha.2`, push
the tag. [ci.yml](.forgejo/workflows/ci.yml) then runs `check`, builds one
addon per target in parallel, all on the one docker runner (`build-linux`
through cargo-zigbuild with a glibc 2.28 floor; `build-windows` through
cargo-xwin against the Windows SDK; `build-macos` through cargo-zigbuild
against a copy of Xcode's SDK, whose Apple license applies), and `publish`
verifies the tag against the
manifests, downloads the five prebuilds, runs the parity tests against the shipped binaries, publishes the
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

## Status / next

v0 scope: no z-index (floats stack in tree order). Transitions cover sizing, colors, radius and
position (`slide`, `enter`); a removed node vanishes at once (there is no exit animation yet).
Layout queries stop at the node: `measure_text` and `on_layout` give whole-string and whole-node
rects, not the boxes of lines or glyphs inside a paragraph. Accessibility and keyboard focus are
data (ADR 0001 and 0002); arrow keys inside radio groups, tab lists and lists, and a
configurable focus ring colour, are the next steps there.
Audio covers one-shots, loops, volume, pause and a finished-playback event; sounds decode fully
into memory, and synthesis, effects, positional audio and disk streaming are out of scope.
Editing: caret blink, double/triple-click
word/line select, scroll-caret-into-view, inline IME composition, Tab
focus traversal, and undo/redo (operational deltas with typing/delete
coalescing — the widget owns its buffer, so it owns its history; hosts
with their own text model take raw chords through `on_key` and bring
their own).

## License

MIT, see [LICENSE](LICENSE).
