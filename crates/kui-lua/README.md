# kui-lua

Lua scripts as kui extensions: a script returns its view as a table tree and gets events back as tables.

`kui-lua` loads a Lua script as an extension a kui host can place in its frame. The script defines `view(env, inst)` or a `components` table, whose functions return a plain table built with the injected `row` / `column` / `text` / `button` prelude, and optionally `on_event(ev)`, which hears the events the script's own nodes emit and may `emit` the messages the host passed. Only data crosses the boundary: no closure lives on either side. Use it when a Rust app on kui wants scriptable panels or plugins.

## Install

```
cargo add kui-lua kui-native
```

## Example

The Rust host places the script's component under a namespace and hears its events:

```rust
use kui_lua::LuaExtension;
use kui_native::{App, NodeSpec, Ui, UiEvent, Value};

struct Host { toggles: u32 }

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::row().fill().pad(16.0).gap(16.0));
        ui.component(
            "todos/panel",
            &Value::map([
                ("title", "todos".into()),
                ("on_toggle", Value::map([("kind", "toggled".into())])),
            ]),
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get_str("kind") == Some("toggled") {
            self.toggles += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = LuaExtension::from_file("panel.lua")?;
    kui_native::app("todos").extension_as("todos", script).run(Host { toggles: 0 })
}
```

`panel.lua` provides the `panel` component, reads the host's props and raises `on_toggle`, which the host hears as the message it passed:

```lua
local done = {}

components = {}

function components.panel(env, props)
  local t = env.theme
  return column {
    width = 300, pad = 16, gap = 10, bg = t.surface, radius = 10,
    text(props.title, { size = 12, color = t.muted }),
    row { on_click = { kind = "toggle", index = 1 },
          text((done[1] and "[x] " or "[ ] ") .. "wire up wgpu") },
  }
end

function on_event(ev)
  if ev.kind == "toggle" then
    done[ev.index] = not done[ev.index]
    return emit("on_toggle", { index = ev.index })
  end
end
```

`on_event(ev)` hears the message the node declared — `{ kind = "toggle", index = 1 }` here — with the event's data in one field named for the event (a drag's under `drag`, an edit's text under `change`), and the envelope beside it: `ev.event` (what happened: `"click"`, `"drag"`, `"change"`, …), `ev.node_key`, `ev.window`, `ev.instance` and `ev.component`. A message's own field of one of those names keeps its value there, so the envelope also comes whole as the second argument, `on_event(ev, at)`: `at.event`, `at.node_key`, `at.window`, and `at.instance`, `at.component` and `at.from` where they apply.

The crate docs list everything a script gets: the builders, the `env` facts and functions, the `inst` argument, the props shapes, `kui_lua::luals_meta()` for editor completion, and `kui_lua::teal_decls()` for a script written in [Teal](https://teal-language.org), typed Lua, checked by `tl check` and compiled by `tl gen` to the Lua this crate loads. Both files are also in the repository, under [`packages/lua`](https://github.com/qxuken/kui/tree/main/packages/lua), to copy.

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
