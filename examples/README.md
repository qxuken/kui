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
| [`modal_editor.rs`](rust/modal_editor.rs) | `cargo run -p kui --example modal_editor` | Helix-flavored modal editing; the app owns the keymap |
| [`splitmux.rs`](rust/splitmux.rs) | `cargo run -p kui --example splitmux` | tmux-style splits, tabs, focus, ⌘-drag pane moves; the pane tree is data |
| [`syntax_view.rs`](rust/syntax_view.rs) | `cargo run -p kui --example syntax_view` | Syntax highlighting as coalesced style runs |
| [`gallery.rs`](rust/gallery.rs) | `cargo run -p kui --example gallery` | Registered images: Fit sizing, kept aspect, rounded corners |
| [`toasts.rs`](rust/toasts.rs) | `cargo run -p kui --example toasts` | `enter`/`exit`: toasts that slide in and back out |
| [`accessibility.rs`](rust/accessibility.rs) | `cargo run -p kui --example accessibility` | Every accessibility prop in one window; the fixture `scripts/ax-audit.swift` drives |
| [`conformance-dump.rs`](rust/conformance-dump.rs) | `cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt` | Writes the scene corpus's reference report the other bindings diff against |

## C — [`c/`](c)

Both directions across the FFI. [`counter.c`](c/counter.c) is C as the host,
with kui as a plain library; [`panel.c`](c/panel.c) is C as an *extension*,
a `dlopen`ed plugin owning its share of a frame inside the Rust host
[`panel.rs`](c/panel.rs). [`build.sh`](c/build.sh) checks `kui.h` against
the Rust struct layout, then builds both C artifacts.

```bash
./examples/c/build.sh                    # ABI check, then counter and panel.so
./examples/c/counter                     # the counter app, C as the host
./examples/c/counter --headless          # FFI self-test, no window needed
cargo run -p kui-ffi --example c_panel   # the Rust host, dlopening panel.so
```

## Lua — [`lua/`](lua)

A Lua extension has no window of its own, so both examples here are a Rust
host with Lua inside it. [`panel.lua`](lua/panel.lua) is deliberately the same
panel as [`c/panel.c`](c/panel.c): the extension contract is the contract and
the language is a detail.

| Example | Run | What it shows |
|---|---|---|
| [`panel.rs`](lua/panel.rs) + [`panel.lua`](lua/panel.lua) | `cargo run -p kui-lua --example lua_panel` | Rust host and Lua panel sharing one frame, clicks routed by origin |
| [`bench.rs`](lua/bench.rs) | `cargo run -p kui-lua --example bench --release` | Frontend-lowering shootout: the same ~900-node view from Rust and from Lua |

## Node — [`node/`](node)

Build the addon with `cargo build -p kui-node --release`, then in
[`node/`](node):

```bash
npm install
npm start        # headless
npm run window   # a real winit + wgpu window, pumped from a timer
npm run bench    # the JSX/Node side of lua/bench.rs
```
