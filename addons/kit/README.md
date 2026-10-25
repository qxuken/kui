# Kit

Kit is kui's UI kit for dense tool windows: components written once in
Teal, loaded as a Lua extension (ADR 0059), placed by a host as often as it
likes (ADR 0060) and painted by a skin. Every binding places them through
typed wrappers generated from [`schema.lua`](schema.lua) (ADR 0066).

| Group | Components |
| --- | --- |
| Layout | `panel`, `toolbar`, `statusbar`, `split`, `tabs`, `section`, `divider` |
| Inputs | `button`, `icon_button`, `text_field`, `search`, `select`, `checkbox`, `switch`, `radio_group`, `segmented`, `slider` |
| Data | `table`, `list`, `log` (over a channel, ADR 0065), `inspector`, `badge`, `stat`, `code`, `empty`, `progress` |
| Overlays | `dialog`, `popover`, `menu`, `toast` |
| Charts | `sparkline`, `line_chart`, `bar_chart`, `histogram`, `waterfall` |
| Look | `settings`: the skin (Ledger, Phosphor, Neutral), the density, the shadows, brand roles |

Every component is controlled: what it shows is the host's prop, and an
event proposes the next value as the host's own message, with the event's
data beside it (`select: { row }`). What the host does not keep — a select's
open menu, a section's fold, a waterfall's own window — Kit keeps per
instance.

## Placing Kit

Load `addons/kit/lua/kit.lua` under the namespace `kit`, then:

```rust
// Rust: the kui-kit crate (bindings/rust)
kit::Settings::new().skin(kit::Skin::Neutral).place(ui);
kit::Toolbar::new().place_with(ui, |group, ui| {
    if group.is_empty() {
        kit::Button::new("Run").variant(kit::Variant::Primary).on_press(Msg::Run).place(ui);
    }
});
```

```js
// Node: bindings/node/kit.js, functions that are also JSX components
import { Toolbar, Button } from './bindings/node/kit.js';
Toolbar({}, [Button({ label: 'Run', variant: 'primary', onPress: { kind: 'run' } })]);
```

```lua
-- Lua: bindings/lua on package.path
local kit = require("kit.ui")
kit.toolbar({}, { kit.button({ label = "Run", variant = "primary", on_press = { kind = "run" } }) })
```

```c
/* C: bindings/c/kit.h */
KitButton run = {.label = KUI_STR("Run"), .variant = KIT_VARIANT_PRIMARY, .on_press = msg};
kit_button(ui, &run);
```

```odin
// Odin: bindings/odin/kit
kit.button(ui, {label = "Run", variant = .Primary, on_press = Run{}})
```

## The Kit app

```bash
kui run addons/kit/app
```

`app/` is Kit on the `kui` runner (ADR 0064): every component on seven
pages, three skins, a worker building 100,000 rows as a channel, live
chart and log feeds, and an HTTP server — a living reference, and the
template for an app of your own (`kui run --watch addons/kit/app`, then
edit `pages.lua`).

Working hosts: `crates/kui-lua/tests/kit.rs` (Rust),
`examples/lua/features/kit/` (the spike scene and its benches),
`examples/c/features/kit/kit.c`, `examples/odin/features/kit/kit.odin`, and
the Kit test in `packages/kui/test.mjs`.

## Layout of this folder

| Path | What |
| --- | --- |
| `schema.lua` | The API: every component's props, events and child groups, with docs. |
| `src/*.tl` | The Teal source: `kit` (the entry), `skin` and the three skins, `ctx` (what every component paints through), `icons`, and the components by group: `layout`, `inputs`, `data`, `overlays`, `charts`, `waterfall`. |
| `lua/*.lua` | What a host loads, generated from `src/`. Edit the `.tl`, never these. |
| `bindings/` | The wrappers generated from `schema.lua`. Never edit these either. |
| `app/` | The Kit app, on the `kui` runner. |
| `build.lua` | Type-checks `src/` against `packages/lua/kui.d.tl` with the Teal compiler kui-lua vendors, and writes `lua/`. `--check` fails on a stale `lua/` instead. |
| `M0.md` | The spike that came first: its findings and numbers. |

```bash
nu scripts/gen.nu --skip lua,node,odin,book
```

runs both generators (the wrappers, then `build.lua`); `--check` fails on a
stale file.

```bash
cargo test -p kui-lua --test kit
```

```bash
cargo bench -p kui-lua --bench kit
```
