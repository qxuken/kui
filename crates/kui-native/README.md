# kui-native

The windowed kui runner: a winit window and a wgpu renderer around kui-core,
with the `App` trait an application implements.

kui is a Rust UI library whose view is a plain data tree rebuilt every
frame. kui-native is the crate most apps depend on. It re-exports all of
kui-core (the model, the layout, the widgets), draws through kui-wgpu, and
adds the `App` trait, the `app(...)` launcher builder, multiple windows, the
clipboard, file dialogs, audio, AccessKit and a headless `testing` driver
that runs under plain `cargo test`.

## Install

```bash
cargo add kui-native
```

Default features: `audio`, `accesskit`, `derive` (`#[derive(Message)]`) and
`dialogs`; each can be turned off with `--no-default-features`.

On Linux the build needs `pkg-config` and ALSA's headers (`libasound2-dev`
on Debian and Ubuntu). A window loads `libxkbcommon`, the X11 or Wayland
client libraries and Vulkan or EGL at run time. macOS and Windows need only
a Rust toolchain.

## Example

```rust
use kui_native::widgets;
use kui_native::{App, Message, NodeSpec, TextStyle, Ui, UiEvent};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    Dec,
}

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    // Called once per frame; everything on screen is declared here.
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().center().gap(16.0).bg(t.bg), |ui| {
            ui.text(&self.count.to_string(), TextStyle::new(56.0).color(t.fg));
            ui.with(NodeSpec::row().gap(8.0), |ui| {
                widgets::button(ui, "-1", Msg::Dec);
                widgets::button(ui, "+1", Msg::Inc);
            });
        });
    }

    // Each event the frame produced, after the frame.
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Inc) => self.count += 1,
            Some(Msg::Dec) => self.count -= 1,
            None => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Counter").size(360.0, 240.0).run(Counter::default())
}
```

## Where it fits

| Crate | What it is |
| --- | --- |
| [kui-native](https://crates.io/crates/kui-native) | This crate: the windowed runner most apps use |
| [kui-core](https://crates.io/crates/kui-core) | The model, layout, widgets and events, no window |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | The GPU renderer |
| [kui-derive](https://crates.io/crates/kui-derive) | `#[derive(Message)]` |
| [kui-lua](https://crates.io/crates/kui-lua), [kui-ffi](https://crates.io/crates/kui-ffi) | Lua and C bindings; kui-node is the Node.js binding, on npm |

## Documentation

- API: <https://docs.rs/kui-native>
- The book, a tutorial from a window to a tested app: <https://kui-book.qxuken.dev>
- Repository and examples: <https://github.com/qxuken/kui>

## License

MIT
