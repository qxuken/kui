# A longer tour of kui

What follows used to be the middle of the repository's README: testing
without a window, the devtools panel, the examples across the four
bindings, and the reference documents. [The book](https://kui-book.qxuken.dev)
is the path in; this page is for a reader who has read it and wants the
rest in one place. The API reference is on [docs.rs](https://docs.rs/kui-native).

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
back ([ADR 0013](adr/0013-effects-as-data.md)). Rust tests drive `Core` the same way
([crates/kui-core/tests](../crates/kui-core/tests)), and C runs the same API
headless (`./target/debug/counter --headless`, the same drive every counter runs) — as does a C *extension* inside
a Rust host (`cargo run -p kui-ffi --example c_panel -- --headless`, which
clicks the plugin's list and checks the click reached the plugin and not the
host).

## Devtools

Every app has a devtools panel, drawn by the core into its own frame
([ADR 0024](adr/0024-the-devtools-are-the-cores.md)). Ask for it with
`kui_native::app("x").devtools(true)` (or `core.set_devtools(true)`),
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
`Ctrl+Shift+I` moves the keyboard into the panel and back out (and brings
a hidden one back); an app that wants that chord for itself respells it —
`.devtools_key(Accel::parse("f12").unwrap())`, `win.setDevtoolsKey('f12')`,
`kui_set_devtools_key(ctx, KUI_STR("f12"))` — and the old one is its own
again. An app can add **tabs of its own** beside the three
([ADR 0032](adr/0032-a-devtools-tab-mounts-a-slot.md)): a slot a
plugin fills (`ui.devtools_tab("syntax", "Tree-sitter", "ts/panel")`), or
the app's own content, built only while the tab is on show —
`ui.devtools_tab_with(name, label, |ui| …)`, `<devtoolsTab name label>{()
=> …}</devtoolsTab>`, `if (kui_devtools_tab_open(ctx, name, label)) { …
kui_close(ctx); }` — drawn over the panel's tab body as the app's own
nodes, reading the panel's selection through `devtools_selected`,
raising its picker with `set_devtools_pick` and jumping to the tab from
a command of its own with `set_devtools_tab("syntax")`; the inspector a
tree-sitter app wants is `examples/rust/features/devtools_tab.rs`.

## Examples

All of them live under [examples/](../examples), one directory per language
and, inside it, one per kind — `apps/`, `widgets/`, `features/`, `tools/`
— with one subject each and the name the repo already uses for it
([ADR 0021](adr/0021-one-subject-per-example.md));
[examples/README.md](../examples/README.md) is the full map. Every one runs
inside a harness that puts a dock beside it: the event stream as data, the
runtime's facts with a small button beside each one it can change, the
latency graph, the key legend — and `--headless` is a self-check with an
exit code, `--light` / `--dark` pin the base.

```bash
cargo run -p kui-native --example counter        # apps/: the Elm loop, the one every binding has
cargo run -p kui-native --example splitmux       # apps/: tmux-style splits, tabs, ⌘-drag pane moves
cargo run -p kui-native --example button         # widgets/: the stock button in every state
cargo run -p kui-native --example controls       # widgets/: checkbox, radio group, switch, slider
cargo run -p kui-native --example edit           # widgets/: multiline editing and the single-line field
cargo run -p kui-native --example text           # widgets/: spans, decorations, families, wrap
cargo run -p kui-native --example cells          # widgets/: a terminal grid that selects in cells
cargo run -p kui-native --example focus          # features/: the Tab ring and its verbs
cargo run -p kui-native --example transition     # features/: transition, easing, slide, keyframes
cargo run -p kui-native --example enter_exit     # features/: toasts that slide in and back out
cargo run -p kui-native --example theme          # features/: every Theme role over every stock widget
cargo run -p kui-native --example counter -- --headless   # the drive, no window
cargo run -p kui-devtools --bin cbuild && ./target/debug/counter   # the same app from C, every platform
cargo run -p kui-ffi --example c_panel    # a Rust host + a dlopened C panel
cargo run -p kui-lua --example lua_panel  # a Rust host + a Lua panel sharing one frame
cargo run -p kui-devtools --bin smoke     # every windowed example, 120 frames, both bases
cargo run -p kui-devtools --bin smoke -- --headless   # every headless drive, what CI runs
```

The same app from Node with JSX — build the addon with
`cargo build -p kui-node --release`, then in [examples/node](../examples/node)
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
switch over `msg.kind` (see [packages/kui](../packages/kui/README.md)). The
encoder is the only door: there is one element dispatcher in the addon, not
one per transport, and a malformed view is rejected in JS before anything
crosses the boundary. `npm test` in [packages/kui](../packages/kui) pins the
encoder from both ends — every schema prop and element has to reach the
stream and lower — and the corpus scenes say what the result must be.
A one-off measurement of the
same view built through Node 26's experimental `node:ffi` module, calling the
C API in [kui.h](../crates/kui-ffi/include/kui.h) directly, put ~1500 flat calls
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
([ADR 0014](adr/0014-slots-an-extension-fills-in-place.md)): a position
among the host's children, filled then and there, with parameters in and
replies out. The host loads the script under a namespace it chooses — the
way an importer picks an alias — and names the slot by `namespace/slot`:

```rust
kui_native::app("counter").extension_as("fs", LuaExtension::from_file("panel.lua")?).run(app)
// …and in the host's view, wherever the panel should sit:
ui.slot_with("fs/panel", &Value::map([("title", "notes".into())]));
```

The same plugin loaded twice under two namespaces is two panels with two
sets of parameters; an extension that names no slots draws after the host's
view, where every extension drew before slots existed.

The same functions from C, because the extension contract is the contract
and the language is a detail — `kui_ffi::CExtension` `dlopen`s a shared
library and hands it the same share of the frame
([examples/c/features/slots/panel.c](../examples/c/features/slots/panel.c),
[examples/c/features/slots/panel.rs](../examples/c/features/slots/panel.rs)):

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

[The book](https://kui-book.qxuken.dev) ([docs/book](book/src/SUMMARY.md) in the repository) is the path in: fourteen short
chapters, one concept each, every code block pulled from a step under
`examples/rust/tutorial` so it cannot go stale
([ADR 0039](adr/0039-a-tutorial-is-a-sequence.md)). It is written for
the reader who has none of the names yet; the two documents below are
for the reader who has.

[docs/howto.md](howto.md) is the task index: about twenty questions a
developer actually arrives with — "how do I animate a removal", "how do I
test the real window", "how do I reset an editor" — each answered in two
sentences that point at the row, the release entry or the ADR with the rest.
Start there when you know what you want and not what it is called; the two
field reports that asked for it had both filed wishes for features already
sitting in their own `node_modules`.

[docs/props.md](props.md) is the cross-binding reference: every prop
with its JSX, Lua and C name, the composites, the elements, the event
payload shapes and the resource APIs. It is generated from the schema
(`npm run gen` in `packages/kui`), so it cannot drift.

The same command generates the addon's half of `packages/kui/index.d.ts` —
`Ctx`, `KuiWindow` and their 39 shared methods each, between the generated
markers — out of napi-rs's own reading of the `#[napi]` attributes in
`crates/kui-node`. A method reaches TypeScript by being written in Rust and
regenerating, not by being typed out a second time; CI diffs the file.

[docs/adr](adr) holds the architecture decision records: the choices
that are hard to reverse and would look arbitrary without their context.
