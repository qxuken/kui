# Examples

Every example in the repo lives here, one directory per binding — and a
binding's directory holds the *whole* example, in whatever languages it takes.
`c/` is not "the C files", it is the C story: C driving kui as a library, C
driven as an extension, and the Rust host and build script that make the
second one run.

| | |
|---|---|
| [`rust/`](rust) | kui from Rust — `kui`, plus `kui-core`'s corpus tool |
| [`c/`](c) | kui from C, both directions — `kui-ffi` |
| [`lua/`](lua) | kui from Lua, inside a Rust host — `kui-lua` |
| [`node/`](node) | kui from Node with JSX — the `kui` npm package |

The Rust files here belong to four different crates but share this one tree,
so each crate's `Cargo.toml` names its examples with an explicit `path`. Three
consequences worth knowing. `cargo run` needs the right `-p`, which the tables
below have. The published crates no longer carry their examples in the tarball
— cargo drops a target whose source sits outside the package, and says so as a
warning during `cargo publish`; read them here instead. And two target names
do not match their filename: `examples/c/panel.rs` is `--example c_panel` and
`examples/lua/panel.rs` is `--example lua_panel`, because every example binary
in the workspace lands in one flat `target/debug/examples/`, where two called
`panel` would collide.

## Rust — [`rust/`](rust)

| Example | Run | What it shows |
|---|---|---|
| [`counter.rs`](rust/counter.rs) | `cargo run -p kui --example counter` | Minimal Elm-ish flow: state → tree, clicks back as data. Sound is data too; right-click for a `modal` context menu |
| [`rich_text.rs`](rust/rich_text.rs) | `cargo run -p kui --example rich_text` | Styled spans shaped and wrapped as one paragraph flow |
| [`editor.rs`](rust/editor.rs) | `cargo run -p kui --example editor` | Multiline editing: caret, selection, clipboard, scrolling |
| [`waker.rs`](rust/waker.rs) | `cargo run -p kui --example waker` | A thread feeds lines and wakes the parked loop through `kui::Waker`; frames with no input |
| [`modal_editor.rs`](rust/modal_editor.rs) | `cargo run -p kui --example modal_editor` | Helix-flavored modal editing; the app owns the keymap |
| [`splitmux.rs`](rust/splitmux.rs) | `cargo run -p kui --example splitmux` | tmux-style splits, tabs, focus, ⌘-drag pane moves; the pane tree is data |
| [`syntax_view.rs`](rust/syntax_view.rs) | `cargo run -p kui --example syntax_view` | Syntax highlighting as coalesced style runs |
| [`gallery.rs`](rust/gallery.rs) | `cargo run -p kui --example gallery` | Registered images: Fit sizing, kept aspect, rounded corners |
| [`toasts.rs`](rust/toasts.rs) | `cargo run -p kui --example toasts` | `enter`/`exit`: toasts that slide in and back out |
| [`bulk_exit.rs`](rust/bulk_exit.rs) | `cargo run -p kui --example bulk_exit` | the exit budget at its boundary: a removal animates whole or not at all, a new one outranks the old, a virtual list keeps the picture small |
| [`connectors.rs`](rust/connectors.rs) | `cargo run -p kui --example connectors` | A mind map whose links are `line` nodes: curves between floats, a hovered card's links brighten by transition |
| [`accessibility.rs`](rust/accessibility.rs) | `cargo run -p kui --example accessibility` | Every accessibility prop in one window; the fixture `scripts/ax-audit.swift` drives |
| [`conformance-dump.rs`](rust/conformance-dump.rs) | `cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt` | Writes the scene corpus's reference report the other bindings diff against |

## C — [`c/`](c)

All three directions across the FFI. [`counter.c`](c/counter.c) is C as the
host with kui as a plain library; [`panel.c`](c/panel.c) is C as an
*extension*, a `dlopen`ed plugin owning its share of a frame; and
[`host.c`](c/host.c) is a C host loading that same plugin — in a *slot* the
host declares, under the namespace the host gave it, with a title passed in
and a reply coming back (ADR 0014). The plugin does not know which host it
is in: [`panel.rs`](c/panel.rs) is a Rust one loading the same file.
[`build.sh`](c/build.sh) checks `kui.h` against the Rust struct layout, then
builds all of it; [`build.ps1`](c/build.ps1) is the same round on Windows.

```bash
./examples/c/build.sh                    # ABI check, then the three C artifacts
./examples/c/counter                     # the counter app, C as the host
./examples/c/counter --headless          # FFI self-test, no window needed
./examples/c/host                        # a C host with the C panel inside it
./examples/c/host --headless             # the slot, the click, the reply
cargo run -p kui-ffi --example c_panel   # the same panel in a Rust host
```

```powershell
pwsh examples/c/build.ps1                # ABI check, then counter.exe, host.exe, panel*.dll
./examples/c/counter.exe --headless
./examples/c/host.exe --headless
cargo run -p kui-ffi --example c_panel -- --headless
```

Windows asks for three things the unixes do not, and the plugin half is where
they show. An MSVC-ABI compiler, because that is what the Rust target links
with. `kui_ffi.dll` beside each host executable, because there is no rpath.
And an import library at each link: a DLL may not leave a symbol undefined,
so a plugin names the module its `kui_*` come from. Which module is a choice,
and `build.ps1` builds both — `panel-dll.dll` imports `kui_ffi.dll` and loads
into any host shipping it (the shape a plugin you hand to somebody wants, and
what `host.exe` loads), `panel.dll` imports the Rust host's own executable and
loads into that host alone. `crates/kui-ffi/build.rs` makes the second
possible by exporting the host's `kui_*`, and `kui.h` marks the plugin's seven
entry points `KUI_EXT_EXPORT`, so `panel.c` is the same source everywhere.

Two copies of the library in one process — a statically linked host and a
plugin importing `kui_ffi.dll` — is fine as of ABI 10, and was not before it:
`kui_reply`'s sink used to be a `thread_local`, one per copy, so every reply
landed where nobody was reading. It rides on the event now.

## Lua — [`lua/`](lua)

A Lua extension has no window of its own, so both examples here are a Rust
host with Lua inside it. [`panel.lua`](lua/panel.lua) is deliberately the same
panel as [`c/panel.c`](c/panel.c): the extension contract is the contract and
the language is a detail.

| Example | Run | What it shows |
|---|---|---|
| [`panel.rs`](lua/panel.rs) + [`panel.lua`](lua/panel.lua) | `cargo run -p kui-lua --example lua_panel` | Rust host and Lua panel sharing one frame: the panel fills the slot the host declares as `todos/panel`, reads its title from the params, replies on a toggle; clicks routed by origin |
| [`bench.rs`](lua/bench.rs) | `cargo run -p kui-lua --example bench --release` | Frontend-lowering shootout: the same ~900-node view from Rust and from Lua |

## Node — [`node/`](node)

Build the addon with `cargo build -p kui-node --release`, then in
[`node/`](node):

```bash
npm install
npm start        # headless
npm run window   # a real winit + wgpu window, pumped from a timer
npm run mindmap  # a canvas of easing floats, panned by dragging it
npm run bench    # the JSX/Node side of lua/bench.rs
```

| Example | What it shows |
|---|---|
| [`counter.tsx`](node/counter.tsx) | The Elm loop headless, driven by its own hit tests |
| [`counter-window.tsx`](node/counter-window.tsx) | The same app in a real window, with images, sounds and an editor |
| [`mindmap.tsx`](node/mindmap.tsx) | A canvas of floats and `line` connectors, all easing, panned by an `onDrag` root |

An extension is a C shared library either way, and the same binary loads
into a Rust, C or Node host: `<slot name="ns/panel" params={…}/>` places it
and `ctx.addExtension(ns, path)` (or a window's `extensions` option) loads
it. There is no script-loads-script path — a Lua extension is loaded by a
Rust host or not at all.

`mindmap.tsx` is the one to open when a gesture works in a test and not on
screen. It moves every tween's target on every frame, which is what backlog
F15 turned out to be — the map panned in the model and stood still in the
window — so it is a by-hand check (`--headless` only asserts the model, which
was never the half that broke).
