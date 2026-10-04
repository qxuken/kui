# kui-core

The headless model behind kui: a per-frame flat tree, flex layout, text
shaping, events as data and a quad display list.

A `Core` owns one window's worth of state. Each frame the app rebuilds a
flat tree of `NodeSpec`s through a `Ui`, the core runs a clay-style flex
layout over it, shapes the text and emits a `DisplayList` of quads for a
renderer. Input arrives as `InputEvent`s and comes back out as `UiEvent`s
whose payloads are plain-data `Value`s. There is no window, no GPU and no
clock in this crate. Most Rust apps use `kui-native`, which re-exports all
of `kui-core`; depend on `kui-core` directly for a custom runner, a binding
to another language, or headless tests.

## Install

```sh
cargo add kui-core
```

Features: `devtools` (default) draws the inspector panel on request;
`derive` re-exports `#[derive(Message)]`; `conformance` enables the scene
corpus and the headless test driver (test infrastructure only).

## Example

A frame built and clicked with no window at all:

```rust
use kui_core::{Core, InputEvent, NodeSpec, Size, TextStyle, widgets};

let mut core = Core::new();
core.set_inspect(true); // keep a readable snapshot of each finished frame

// One frame: begin, declare the tree, finish (layout and emission).
let mut ui = core.frame(Size::new(320.0, 200.0), 1.0);
ui.configure_root(NodeSpec::column().fill().pad(16.0).gap(8.0));
ui.text("Hello from kui-core", TextStyle::new(16.0));
widgets::button(&mut ui, "Save", "save");
ui.finish();

// What a renderer draws: a flat list of quads.
let (list, _atlas) = core.output();
assert!(!list.quads.is_empty());

// Input goes in as `InputEvent`s and comes out as `UiEvent`s carrying
// the payload the view declared.
let button = core
    .nodes()
    .into_iter()
    .find(|n| n.label.as_deref() == Some("Save"))
    .expect("the button was laid out");
core.handle_input(InputEvent::CursorMoved(button.rect.center()));
core.handle_input(InputEvent::mouse_down(1));
let events = core.handle_input(InputEvent::mouse_up());
assert!(events.iter().any(|e| e.payload.as_str() == Some("save")));
```

## Where it fits

| Crate | What it is |
| --- | --- |
| [kui-core](https://crates.io/crates/kui-core) | The model: tree, layout, text, events, display list (this crate) |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | The renderer: draws a `DisplayList` with wgpu |
| [kui-native](https://crates.io/crates/kui-native) | The windowed runner most Rust apps use; re-exports kui-core |
| [kui-derive](https://crates.io/crates/kui-derive) | `#[derive(Message)]` for typed payloads |
| [kui-lua](https://crates.io/crates/kui-lua) | Lua binding |
| [kui-ffi](https://crates.io/crates/kui-ffi) | C binding |
| kui-node (npm) | Node binding |

## Documentation

- API reference: <https://docs.rs/kui-core>
- The book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
