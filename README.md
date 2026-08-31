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

## Examples

```bash
cargo run -p kui --example counter        # pure Rust, Elm-ish flow
cargo run -p kui --example rich_text      # styled spans in one wrapped paragraph
cargo run -p kui --example editor         # multiline text editing: caret, selection, clipboard
cargo run -p kui-lua --example lua_panel  # Rust host + Lua panel sharing one frame
./examples/c/build.sh && ./examples/c/counter             # the same app from C
./examples/c/counter --headless           # C FFI self-test, no window needed
```

A Lua extension in full:

```lua
function view()
  return column { gap = 8, pad = 16, bg = 0x14161eff,
    text("count: " .. count, { size = 20 }),
    button { label = "bump", on_click = { kind = "bump" } },
  }
end

function on_event(ev)
  if ev.kind == "bump" then count = count + 1 end
end
```

## Design notes

- **Events are data, not callbacks.** Nodes declare an `on_click` payload
  (`Value`: null/bool/int/float/str/list/map). Input resolves against the
  previous frame's layout and produces `UiEvent`s tagged with the origin that
  declared them; the runner routes host events to `App::on_event` and
  extension events back into the script that owns them.
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
  editing reachable from Lua and C. The runner maps winit keys, IME commits,
  and platform clipboard shortcuts (arboard) onto those events.
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

Overflow: `.clip()` clips children; `.scroll_y()` / `.scroll_x()` make a
container scrollable (wheel/trackpad, offsets retained across frames by widget
key, clamped to content, with a scrollbar indicator). Clip rects ride on each
quad and are applied in the shader, so the whole UI is still one draw call.
Fully clipped nodes are culled from both drawing and hit-testing.

## Performance

`cargo bench -p kui-core` (M-series MacBook, release, steady-state warm
caches — full frame: build + layout + emit):

| bench | median |
|---|---|
| 1k nodes, text + hit regions ("typical app") | ~62 µs |
| 10k plain rects | ~455 µs |
| 10k rects + 1.2k texts + 2.5k hit regions | ~675 µs |
| 16×64-deep nesting chains | ~51 µs |

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

v0 scope: no images in the display list yet (resource registry exists), no
shrink pass, no scrollbar dragging (wheel/trackpad only), mask + color-emoji
glyphs only (no subpixel AA). Editing: no undo/redo, caret blink,
double-click word select, IME preedit display, Tab focus traversal, or
scroll-caret-into-view yet.
