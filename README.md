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

## Examples

```bash
cargo run -p kui --example counter        # pure Rust, Elm-ish flow
cargo run -p kui --example rich_text      # styled spans in one wrapped paragraph
cargo run -p kui --example editor         # multiline text editing: caret, selection, clipboard
cargo run -p kui-lua --example lua_panel  # Rust host + Lua panel sharing one frame
./examples/c/build.sh && ./examples/c/counter             # the same app from C
./examples/c/counter --headless           # C FFI self-test, no window needed
cargo run -p kui --example modal_editor   # helix-flavored modal editing; the app owns the keymap
cargo run -p kui --example splitmux       # tmux-style splits, tabs, and focus; the pane tree is data
cargo run -p kui --example syntax_view    # syntax highlighting as coalesced style runs
cargo run -p kui --example gallery        # registered images: Fit sizing, kept aspect, rounded corners
```

The same app from Node with JSX — build the addon with
`cargo build -p kui-node --release`, then in [examples/node](examples/node)
`npm install && npm start` (headless) or `npm run window` (a real winit +
wgpu window, its event loop pumped from a timer so it shares the main
thread with libuv):

```tsx
// tsconfig: "jsx": "react-jsx", "jsxImportSource": "kui"
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
never a closure. The addon's own object walk (`frameObject`) and a JSON
string transport (`frameJson`) stay available as readable reference paths;
`npm test` in [packages/kui](packages/kui) checks that all three produce
byte-identical quads for every schema prop and element, and
`examples/node/bench.mjs` compares their cost.

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
  (the `modal_editor` and `splitmux` examples are built on this).
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
  wrapping crosses style boundaries and emoji share baselines.
- **Text editing is retained state, not captured state.** An edit node's
  buffer/cursor/selection live in the core keyed by widget identity (cosmic-
  text's `Editor` underneath, so motion, selection, and click-to-caret share
  the shaping truth). Input arrives as data (`InputEvent::Text` / `Key`) routed
  to the focused editor; hosts get "changed"/"submit" events and read text
  back by key — no `&mut String` captured in a view, which is what keeps
  editing reachable from Lua and C. The runner maps winit keys, IME input
  (preedit drawn at the caret, the OS candidate window anchored there too),
  and platform clipboard shortcuts (arboard) onto those events; the caret
  blinks on the runner's clock, scrolls itself into view, and Tab/Shift-Tab
  hop between edit widgets.
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
  from nowhere and a divider drag doesn't replay when it ends.
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
  and drains `kui_take_window_commands`.
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
height against a resolved width keeps aspect, and `style.radius` rounds
corners.

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

Layout solver, atlas packer, key scheme, event dispatch, editing, and the Lua
binding are covered by tests (`cargo test --workspace`).

## Status / next

v0 scope: mask + color-emoji glyphs only (no subpixel AA); no z-index
(floats stack in tree order). Transitions cover sizing, colors and radius;
a removed node vanishes at once (there is no exit animation yet). Editing: caret blink, double/triple-click
word/line select, scroll-caret-into-view, IME preedit at the caret, Tab
focus traversal, and undo/redo (operational deltas with typing/delete
coalescing — the widget owns its buffer, so it owns its history; hosts
with their own text model take raw chords through `on_key` and bring
their own).
