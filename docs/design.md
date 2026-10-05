# Design notes and layout

How kui is built and why: one note per decision that shapes the API,
then the layout solver pass by pass. The decisions themselves are
recorded under [adr/](adr); this page is their consequences for an app.

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
  fragments (a box a WGSL function paints), polygons (a fill of up to eight
  points), paths (any outline, SVG path data, filled and stroked), lines
  (segments, polylines and curves), buttons, titlebar (plain or with
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
  ([ADR 0014](adr/0014-slots-an-extension-fills-in-place.md)).
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
- **Accessibility is data too** ([ADR 0001](adr/0001-accessibility-as-data.md)).
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
  baselines. A
  line of 4096 bytes or more — a minified bundle, a log line with a blob in
  it, plain or in spans — is shaped in ~1 KB chunks as they come on screen,
  so it costs the screenful it shows and a keystroke into it (or a span
  moving along it) costs the chunk it lands in; wrapped, its chunks stay
  shaped as they are and the rows are broken from their positions, each
  chunk's first row starting where the last one's ended, so a 100k-character
  paragraph costs the rows it shows (backlog C19, C42). Line breaking
  is a style choice: `wrap` (word / glyph / none / break-spaces, where
  every space takes its room and wraps like a glyph), `max_lines`, and
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
- **Keyboard focus is data** ([ADR 0002](adr/0002-keyboard-focus-as-data.md)).
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
  ([ADR 0022](adr/0022-focus-regions.md)). A node declaring
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
  ([ADR 0003](adr/0003-modal-surfaces.md)). A node declaring `modal`
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
  springs: `smooth`, `snappy`, `spring` and `bouncy` integrate a damped
  spring per frame with a velocity that survives retargets, so a value
  chased mid-flight keeps its momentum instead of restarting. A spring
  takes the two numbers a person tunes by eye rather than its physics:
  the duration (about how long it takes to get there) and a `bounce`,
  how far it overshoots — 0 for `smooth`, 0.5 for `bouncy`, any other
  with the `bounce` prop.
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
  (`kui_native::app(..).text_aa(TextAa::Grayscale)` or `KUI_TEXT_AA=gray` opt out);
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
  (`ctx.setEnv()` declares them headlessly), `kui_env_set_window` in C.
  The window's level is a per-frame fact the same way the title is:
  `ui.always_on_top(true)` / a root `alwaysOnTop` / `always_on_top =
  true` / `kui_set_always_on_top` asks for the window above every other
  app's, a frame that stops asking lowers it (so a pin button is a
  toggle), and `env.window.always_on_top` reports what the platform did —
  which can be nothing, on Wayland. Secure keyboard entry is the same kind
  of fact: `ui.secure_input(true)` / a root `secureInput` / `secure_input =
  true` / `kui_set_secure_input` on every frame a password prompt is up,
  and the runner turns macOS's Secure Keyboard Entry on while that window
  has the keyboard and off when it loses it, closes or stops asking,
  keeping the process-wide count balanced. The Option keys are one too:
  `ui.option_as_alt(OptionAsAlt::Left)` / a root `optionAsAlt="left"` /
  `option_as_alt = "left"` / `kui_set_option_as_alt` makes that Option
  Alt in the window, so a Mac's dead keys (⌥u, ⌥e, ⌥n) arrive as chords
  instead of starting an accent; `none`, the default, keeps the Mac's
  composing Option. The windows' icon is a launch
  option: `kui_native::app("t").icon(rgba, w, h).icon_resource(1)` — the pixels
  on X11, the executable's icon resource on Windows, for the title bar,
  Alt-Tab and the taskbar (`icon` in Node's `WindowOptions`,
  `kui_set_icon` in C); macOS and Wayland take the app's icon from the
  bundle and the `.desktop` file instead. Custom chrome is an opt-in:
  `kui_native::app("title").custom_titlebar().run(app)` — macOS keeps native traffic lights over your content; Windows/Linux go
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
  runs once per open window with `ui.window_name()` saying which (Node's
  `view(model, name, win)` gets `win` aimed at that window, so `editText`
  and `focus` answer for its tree — `win.useWindow(name)` re-aims it);
  the same drain carries the `Open` / `Close`, and the app hears
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
  What the user may resize *to* is a launch option: `kui_native::app("t").min_size(420.0,
  320.0).max_size(1600.0, 1200.0)` (`minWidth` / `minHeight` / `maxWidth` /
  `maxHeight` in the Node `WindowOptions`, where either half of a pair may
  stand alone). The OS enforces the bounds — including the synthesized edge
  resizing under custom chrome — and the initial size is clamped into them,
  so the pre-first-frame `window_size()` never reports a size the window
  cannot have; where the two bounds cross, the minimum wins. The Rust
  runner also reads `KUI_WINDOW=WxH` from the environment as the opening
  size — over `.size(..)`, inside the bounds — for driving an example at a
  size without editing it, and `KUI_WINDOW_AT=X,Y` as where it opens
  (`KUI_DEVTOOLS`, `KUI_TEXT_AA` and
  `KUI_LOSE_DEVICE=SECS` — the device treated as lost that long after
  launch, to watch the runner open a new one — are its other variables).
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
  "leave", by, tag}` events like any other interaction, including when a new
  frame moves a node under a still cursor (`by: "content"` then, `"pointer"`
  when the pointer moved; `Core::take_pending_events`,
  routed by every driver after a frame). `is_hovered` / `is_pressed` stay
  as queries for Rust and Lua views and are mirrored on `KuiWindow`.
  Files dragged in from the OS are the same shape one row over
  ([ADR 0031](adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md)):
  `on_drop` (JSX `onDrop`, `KuiSpec.on_drop`) makes a node a drop zone
  that hears `{kind="drop", phase="enter"|"move"|"leave"|"drop", paths,
  x, y, tag}`, and `drop_bg` lights it while they hover. The zone is the
  topmost zone under the pointer — a button inside it is its, a banner
  the view floats over it on `enter` is looked past — and the driver
  reports the position (the macOS runner reads it from AppKit, which
  winit does not surface). Files the app goes looking for arrive the same
  way: `request_files` (`ctx.requestFiles`, `env.request_files`,
  `kui_request_files`) asks for the platform's Open, Save or folder
  dialog, which the runner shows through rfd as a sheet on the window
  (the default-on `dialogs` feature), and the answer is one `{kind="files",
  paths, tag}` event — a drop's `paths`, none when the user cancelled.
  One dialog at a time; a headless host takes the ask with
  `take_file_requests` and answers it as input.
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
  [4096 nodes](adr/0005-the-paint-vocabulary.md) may be departing at
  once. An `exit` plays when its own node is removed — a node that goes
  because an ancestor went goes at once, as React's `AnimatePresence`
  has it — so put it on the node that leaves, under a parent that stays. That budget is judged [per frame and whole](adr/0012-the-exit-budget.md):
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
  Warnings table in [docs/props.md](props.md#warnings) — generated
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
  Both are decided in [ADR 0005](adr/0005-the-paint-vocabulary.md),
  which also says why there are no gradient *props*.
- **A fragment is a box a WGSL function paints.** What ADR 0005 declined to
  build one prop at a time — gradients, rings, noise, shimmer — an app
  writes as one function instead
  ([ADR 0015](adr/0015-a-fragment-element-and-the-painter-it-is-not.md)):
  `add_fragment(wgsl)` validates the source and hands back a handle, and
  `<fragment src params animate>` draws a box with it. The app writes
  `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>`
  and nothing else — kui owns the vertex stage, the node's rounded box, the
  inherited clip, the group opacity and the blend, so a fragment cannot
  look unlike a kui node. It lays out, takes input and holds children,
  which paint over it. Sixteen positional floats go in, the frame clock and
  the node's size come with them, and `animate` asks for a frame every
  frame. It reads an **image** too — `<fragment image={id}>`,
  `id.with_image(img)`, `kui_fragment_with` — through `kui_sample(uv)` and
  `kui_sample_nearest(uv)`, with the texel rect in `in.image`
  ([ADR 0025](adr/0025-the-image-is-the-canvas.md) decision 7): a
  data texture the app replaces with `update_image` is a spectrogram, a
  heatmap or a 50k-point line from one quad, and any registered image is
  an effect's input; the core binds the atlas or the image's own texture,
  whichever holds it. What it cannot do — multi-pass, geometry, compute,
  reading what is behind it — is the `painter` the ADR names and does not
  build.
- **Fonts are registered resources.** Beyond the generic sans / serif /
  mono families, `Core::load_fonts_dir("fonts")` / `load_font_file(path)` /
  `add_font_data(bytes)` load TTF/OTF/TTC files into the font database and
  `Core::add_system_font("Antonio")` names a family — installed or just
  loaded (`system_font_families()` lists them, `system_fonts()` with
  which are monospaced, their weights and italics; the system's are
  scanned once a process and again when macOS or Windows says a font was
  installed or removed, and `reload_system_fonts()` asks where nothing
  says; a rescan that found a change is a `fonts` event); all hand back a `FontId`
  slotmap handle for `TextStyle::font(id)` — JSX `<text font={id}>` via
  `ctx.addFont` / `addSystemFont`, Lua `font = id`, C `KuiTextStyle.font`
  via `kui_font_add*`. A view can also name the family itself — JSX
  `family="Antonio"`, Lua `family = "Antonio"` (ADR 0037) — resolved to
  the same handle; a name nothing matches (exactly, as fontdb spells it)
  draws sans with an `unknown-family` warning. The shaping cache keys on the handle, editors shape
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
  once and the UI runs on). Volumes are linear amplitude; `kui_native::audio::blip`
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
  a field to it is free or fatal ([ADR 0006](adr/0006-c-abi-versioning.md)).
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

Tables: `NodeSpec::table()` (`dir="table"` in JSX, `grid { }` in Lua,
`KUI_TABLE` in C) is a column whose rows' children line up in columns —
the nth in-flow child of every row is column n (a float in a row is not
a cell), and a column is as wide as its
widest cell — so a key/value list sits at its longest key with no width
picked by hand ([ADR 0033](adr/0033-a-table-is-a-column-whose-cells-align.md)).
A cell's sizing is its column's: `Fit` and `Fixed` are content, `Grow`
grows the column with the table, `Percent` takes its cut, and the cells'
clamps clamp the column; a bare text is a cell held to its column; the
rows are rows, with their own gap, padding, background, hover and click.
Resolved inside the passes above — the fit at the table in pass 1, the
columns once in pass 2, written into the cells — so it needs no widget
and costs a frame without one nothing.

Out-of-flow: `.float(FloatConfig)` takes a node out of flex flow — it doesn't
consume space in its parent, positions by attach points against its parent's
rect or the viewport (plus an offset), sizes Grow/Percent against that anchor,
and escapes ancestor clips — unless it is anchored to its parent and says
`.clipped()`, when the parent's clip cuts it as it cuts a child: the nodes
of a `clip` canvas stop at the canvas's edge. It paints as a layer of its own: above the in-flow
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
thumb, whose defaults are 4 px and the theme's two roles. A scroll
gesture (a swipe and its glide, a wheel spun without a pause) goes to the
innermost scroller under the pointer that can still move its way when it
starts, and stays with it to its end; `overscroll: "contain"` keeps a
scroller at its limit from passing a new gesture to the one around it
(ADR 0038). An app reaches
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
needs to declare only the rows that can be seen. `widgets::uniform_list`
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
`kui_native::Waker` once, before the window opens; it is `Clone + Send`, and
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
clicks. A node that declares `on_button` (JSX `onButton`) claims the others
— the middle, the secondary and any further button, narrowed by `buttons` —
and hears each claimed button as `{kind="button", phase="press"|"move"|
"release", button, x, y, tag}`, captured by it from the press to the
release wherever the pointer goes; a claimed secondary press is that
instead of a context menu (backlog F105). Otherwise a secondary (right)
press goes to `on_context_menu`, which emits
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
[ADR 0017](adr/0017-selection-as-a-scope.md) and
[ADR 0018](adr/0018-a-menu-bar-the-app-declares.md). A row is a label,
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
the platform's binds the accelerators its rows declare. `cargo run -p kui-native
--example menu_bar` is both, and `--example context_menu` the menus a
right-click gets.

Images: register RGBA pixels once (`resources.add_image`), then `ui.image(id,
spec)` draws them through the same atlas page and draw call as glyphs (the
page doubles up to 4096² when needed). `Fit` takes the pixel size, a `Fit`
height against a resolved width keeps aspect, and `style.radius` (all four
corners, or per corner) rounds them. **The image is the canvas**
([ADR 0025](adr/0025-the-image-is-the-canvas.md)): `update_image(id,
w, h, rgba)` replaces the pixels in place — a video frame, a camera, a plot
the app rasterised — and from then on the image draws from a texture of its
own, as does one too big for a page; `ui.image_with(id, ImageOpts { sampling,
fit }, spec)` says how the pixels meet the box (`nearest` for pixel art,
`contain` / `cover` for another aspect), and the `layout` event's `scale`
says how many pixels to render for it.

Polygons: `ui.polygon(&points, spec)` fills an outline of up to eight points
with the spec's `bg`, placed like a line — a float in the parent's box space
— and painted as one `fragment` quad by a stock WGSL source the core
registers itself, so an arrowhead, a pie wedge or the area under a curve is
one node ([ADR 0025](adr/0025-the-image-is-the-canvas.md), decision 6).

Paths: `ui.path(&Path::parse(d)?, spec)` — or `Path::sector(..)` for a
wedge, a donut's segment or a ring, or the builder's `move_to` / `line_to`
/ `quad_to` / `cubic_to` / `arc_to` — fills any outline with the spec's
`bg` by its rule and strokes it with its own `Stroke`, placed like a
line. The core flattens it for the hit outline, rasterizes it once per
shape and scale through zeno (swash's rasterizer) into the glyph atlas,
and draws one `GlyphMask` quad per paint, so a hover or a colour tween
re-tints the same mask; a mask too big for a page, or a path whose ops
change two frames running, draws from a texture of its own
([ADR 0040](adr/0040-a-path-is-a-mask-in-the-atlas.md)).

Lines: `ui.line(from, to, Stroke::new(width, color), spec)` draws a
round-capped segment, `ui.polyline(&points, stroke, spec)` a polyline, and
`Stroke::curve()` a smooth curve through the points, flattened in the core so
every binding gets the same segments
([ADR 0010](adr/0010-a-segment-primitive.md)). Points are in the
parent's box space — the space a floated sibling's offset is in — and a line
is never in layout: it floats, sized to its own bounding box, takes no room
in a row or column, and paints in the float pass in tree order, so a
connector goes under two cards by being declared before them. Its colour is
the node's `bg` slot, which is what makes `transition`, `enter` and `exit`
reach it; it takes no pointer input and has no access row.
