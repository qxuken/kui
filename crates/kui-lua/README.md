# kui-lua

Lua scripts as kui extensions: a script returns its view as a table tree and gets events back as tables.

`kui-lua` loads a Lua script as an extension a kui host can place in its frame. The script defines `view(env, slot)`, which returns a plain table built with the injected `row` / `column` / `text` / `button` prelude, and optionally `on_event(ev)`, which hears the events the script's own nodes emit and may return replies for the host. Only data crosses the boundary: no closure lives on either side. Use it when a Rust app on kui wants scriptable panels or plugins.

## Install

```
cargo add kui-lua kui-native
```

## Example

The Rust host reserves a position for the script under a namespace and hears its replies:

```rust
use kui_lua::LuaExtension;
use kui_native::{App, NodeSpec, Ui, UiEvent, Value};

struct Host { toggles: u32 }

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::row().fill().pad(16.0).gap(16.0));
        ui.slot_with(
            "todos/panel",
            &Value::map([
                ("title", "todos".into()),
                ("on_toggle", Value::map([("kind", "toggled".into())])),
            ]),
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.kind() == Some("toggled") {
            self.toggles += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = LuaExtension::from_file("panel.lua")?;
    kui_native::app("todos").extension_as("todos", script).run(Host { toggles: 0 })
}
```

`panel.lua` fills the `panel` slot, reads the host's params and replies with the host's template:

```lua
slots = { "panel" }

local done = {}
local on_toggle

function view(env, slot)
  local t = env.theme
  on_toggle = slot.params.on_toggle
  return column {
    width = 300, pad = 16, gap = 10, bg = t.surface, radius = 10,
    text(slot.params.title, { size = 12, color = t.muted }),
    row { on_click = { kind = "toggle", index = 1 },
          text((done[1] and "[x] " or "[ ] ") .. "wire up wgpu") },
  }
end

function on_event(ev)
  if ev.kind == "toggle" then
    done[ev.index] = not done[ev.index]
    local reply = { index = ev.index }
    for k, v in pairs(on_toggle) do reply[k] = v end
    return reply
  end
end
```

The crate docs list everything a script gets: the builders, the `env` facts and functions, the `slot` argument, the props shapes, and `kui_lua::luals_meta()` for editor completion.

## Where it fits

| Crate | Role |
| --- | --- |
| [kui-core](https://crates.io/crates/kui-core) | The model and layout |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | The renderer |
| [kui-native](https://crates.io/crates/kui-native) | The windowed runner most apps use |
| [kui-derive](https://crates.io/crates/kui-derive) | `#[derive(Message)]` |
| [kui-ffi](https://crates.io/crates/kui-ffi) | The C API and plugin loader |
| kui-lua | Lua scripts as extensions (this crate) |
| [@qxuken/kui](https://www.npmjs.com/package/@qxuken/kui) | The Node.js package |

## Documentation

- API: <https://docs.rs/kui-lua>
- Book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
