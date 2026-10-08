# kui

kui is a UI library for Rust. You describe what is on screen as a tree of
boxes and text; kui lays it out, draws it, and hands back what the user did
as plain data. Underneath is a clay-style flat-array layout solver, on top
an iced-style `view` / `on_event` loop, and text shaping is part of the
core. Because a frame is data all the way down, Lua, C and Node bind to
the same contract the Rust builders use.

## Crates

| crate | role | |
|---|---|---|
| `kui-native` | The crate most apps use: a window (winit), the GPU renderer, the stock widgets and the `App` trait | [crates.io](https://crates.io/crates/kui-native) · [docs.rs](https://docs.rs/kui-native) |
| `kui-core` | The bindable contract: the flat per-frame tree, the flex solver, the text stack (cosmic-text + a glyph atlas), events as data, resources, the quad display list | [crates.io](https://crates.io/crates/kui-core) · [docs.rs](https://docs.rs/kui-core) |
| `kui-wgpu` | The wgpu backend: one instanced pipeline (rounded rects, borders, glyphs), one draw call per frame | [crates.io](https://crates.io/crates/kui-wgpu) · [docs.rs](https://docs.rs/kui-wgpu) |
| `kui-derive` | `#[derive(Message)]`: an enum that travels through the frame as data and comes back typed | [crates.io](https://crates.io/crates/kui-derive) · [docs.rs](https://docs.rs/kui-derive) |
| `kui-lua` | Lua extensions via mlua: scripts return table trees and receive events as tables | [crates.io](https://crates.io/crates/kui-lua) · [docs.rs](https://docs.rs/kui-lua) |
| `kui-ffi` | The C API (a cdylib, a staticlib on request, plus [include/kui.h](crates/kui-ffi/include/kui.h)): flat builder calls, opaque `KuiValue` payloads, `repr(C)` draw data, a windowed runner via callbacks, and `CExtension`, a C shared library as a guest in another host's frame | [crates.io](https://crates.io/crates/kui-ffi) · [docs.rs](https://docs.rs/kui-ffi) |
| `kui-node` | The Node.js addon (napi-rs) behind the [`@qxuken/kui`](packages/kui) npm package: JSX views lowered into the IR in one call per frame, Elm-style messages as data. Ships through npm, not crates.io | [packages/kui](packages/kui/README.md) |

## Install

```bash
cargo add kui-native
```

which writes the one line a project needs:

```toml
[dependencies]
kui-native = "0.1.0-alpha.45"
```

Every release so far is an alpha. A requirement like `"0.1.0-alpha.34"`
is a floor that admits every later alpha; `Cargo.lock` keeps the version
you tested, and `"=0.1.0-alpha.34"` pins it in the manifest.

macOS and Windows need nothing beyond a Rust toolchain. On Linux the build
wants `pkg-config` and ALSA's headers (`libasound2-dev` on Debian and
Ubuntu). A window loads the rest at run time: `libxkbcommon` (plus
`libxkbcommon-x11` on X11), the X11 client libraries (`libX11`,
`libXcursor`, `libXrandr`, `libXi`) or `libwayland-client`, and
`libvulkan` with a driver or `libEGL`. A missing one fails when the window
opens, with the library's name in the error. File dialogs go through the
XDG desktop portal; without one on the session bus, every dialog answers
as if cancelled.

The first build takes a few minutes: it compiles a GPU renderer.

## Hello, window

The whole of kui in one program. An app is a type with a `view` that
declares the frame from scratch, and a launcher opens a window around it.

```rust
use kui_native::{App, TextStyle, Ui};

/// The app is any type. It holds the state; this one has none yet.
struct Hello;

impl App for Hello {
    /// Called once per frame. Everything on screen is declared here,
    /// from scratch, every time.
    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        ui.text("Hello, kui", TextStyle::new(24.0).color(theme.fg));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Hello").size(360.0, 200.0).run(Hello)
}
```

Put that in `src/main.rs` and `cargo run`. From a checkout of this
repository it is `cargo run -p kui-native --example tutorial_01_hello`.

That program is step one of [the kui book](https://kui-book.qxuken.dev):
ten short chapters, each the last plus one concept (layout, messages,
controls, lists, keys, floats, motion, effects, a test), and each a
program under [`examples/rust/tutorial`](examples/rust/tutorial). Read it
first.

## Documentation

- **[The kui book](https://kui-book.qxuken.dev)**: the path in, one
  concept a chapter, every code block pulled from a runnable program.
  Its source is [docs/book](docs/book).
- **API reference on docs.rs**:
  [kui-native](https://docs.rs/kui-native),
  [kui-core](https://docs.rs/kui-core),
  [kui-wgpu](https://docs.rs/kui-wgpu),
  [kui-derive](https://docs.rs/kui-derive),
  [kui-lua](https://docs.rs/kui-lua),
  [kui-ffi](https://docs.rs/kui-ffi).
- **[docs/howto.md](docs/howto.md)**: "How do I…", about twenty
  questions a developer arrives with, two sentences each and a pointer.
- **[docs/props.md](docs/props.md)**: every prop, element, event,
  warning, theme role and metric, with its JSX, Lua, C and Odin spelling in
  the same row. Generated from the schema, so it cannot drift.
- **[examples/README.md](examples/README.md)**: the map of every example
  in Rust, C, Lua, Node and Odin, what each shows and how to run it.
- **[docs/guide.md](docs/guide.md)**: testing without a window, the
  devtools panel, the same app from Node, Lua and C, and the reference
  documents in detail.
- **[docs/design.md](docs/design.md)**: the design notes (one schema for
  every binding, events, focus, modals, transitions, text, windows,
  audio, the C ABI) and the layout solver pass by pass.
- **[docs/performance.md](docs/performance.md)**: the benchmark table and
  what it says, editing latency, frame pacing.
- **[docs/releasing.md](docs/releasing.md)**: the registries, how a
  release is cut, CI, the smoke rounds and the audits.
- **[docs/status.md](docs/status.md)**: what v0 does not do, by area.
- **[docs/adr](docs/adr)**: design records, the decisions that are hard
  to reverse and would look arbitrary without their context.
- **[CHANGELOG.md](CHANGELOG.md)**: per release, what was added and,
  separately, what an app can delete.
- **[packages/kui](packages/kui/README.md)**: the Node package,
  `@qxuken/kui`: JSX views, an Elm-style `update`, a headless `createApp`
  and the same devtools.

## License

MIT, see [LICENSE](LICENSE).
