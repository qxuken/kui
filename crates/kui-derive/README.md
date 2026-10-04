# kui-derive

`#[derive(Message)]` for kui: a Rust enum or struct to and from the
plain-data payload a kui message is.

kui carries every message as a `Value` map shaped `{kind, ...fields}`, so
one view model serves Rust, Lua, C and Node alike. This derive lets a Rust
app keep a typed enum instead: `widgets::button(ui, "Save", Msg::Save)`
sends it and `ev.message::<Msg>()` reads it back. Most apps get the derive
through kui-native (`use kui_native::Message`; cargo feature `derive`, on
by default) and never name this crate.

## Install

```bash
cargo add kui-native
```

Only a crate that depends on kui-core alone needs this crate directly:
`cargo add kui-derive`, then `#[message(crate = "kui_core")]` on each type.

## Example

```rust
use kui_native::widgets;
use kui_native::{App, Message, NodeSpec, Ui, UiEvent};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,                          // {kind: "inc"}
    Pick { id: u64 },             // {kind: "pick", id: 3}
    #[message(kind = "add10")]
    AddTen,                       // {kind: "add10"}
}

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.with(NodeSpec::row().gap(8.0), |ui| {
            widgets::button(ui, "+1", Msg::Inc);
            widgets::button(ui, "+10", Msg::AddTen);
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Inc) => self.count += 1,
            Some(Msg::AddTen) => self.count += 10,
            Some(Msg::Pick { id }) => println!("picked {id}"),
            None => {}
        }
    }
}
```

The derive generates `From<Msg> for Value`, `TryFrom<Value>` (with a
`MessageError` naming what did not fit) and `MessageField`, so a message
can be a field of another. `#[message(string)]` on an enum of unit
variants encodes it as a bare string instead of a map.

## Where it fits

| Crate | What it is |
| --- | --- |
| [kui-native](https://crates.io/crates/kui-native) | The windowed runner most apps use; re-exports this derive |
| [kui-core](https://crates.io/crates/kui-core) | The model, layout, widgets and events, no window |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | The GPU renderer |
| [kui-derive](https://crates.io/crates/kui-derive) | This crate |
| [kui-lua](https://crates.io/crates/kui-lua), [kui-ffi](https://crates.io/crates/kui-ffi) | Lua and C bindings; kui-node is the Node.js binding, on npm |

## Documentation

- API: <https://docs.rs/kui-derive>
- The book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
