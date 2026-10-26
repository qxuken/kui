# Kit

Kit is kui's UI kit for dense tool windows: components written once in
Teal, loaded as a Lua extension (ADR 0059), placed by a host as often as it
likes (ADR 0060) and painted by a skin. Every binding places them through
typed wrappers generated from [`schema.lua`](schema.lua) (ADR 0066).

| Group | Components |
| --- | --- |
| Layout | `panel`, `toolbar`, `statusbar`, `split`, `tabs`, `section`, `divider` |
| Inputs | `button`, `icon_button`, `text_field`, `text_area`, `search`, `select`, `checkbox`, `checklist`, `switch`, `radio_group`, `segmented`, `slider` |
| Data | `table`, `list`, `log` (over a channel, ADR 0065), `inspector`, `badge`, `stat`, `code`, `empty`, `progress`, `notice`, `skeleton`, `shader` |
| Overlays | `dialog`, `popover`, `menu`, `toast` |
| Charts | `sparkline`, `line_chart`, `bar_chart`, `histogram`, `waterfall` — each reads out the value under the pointer (a line chart from the keyboard too, with `focusable`) |
| Look | `settings`: the skin (Ledger, Phosphor, Neutral), light or dark held or the OS's, the density, the shadows, the radius, hairline and rule widths, the faces, brand roles |

Every component is controlled: what it shows is the host's prop, and an
event proposes the next value as the host's own message, with the event's
data beside it (`select: { row }`). What the host does not keep — a select's
open menu, a section's fold, a waterfall's own window — Kit keeps per
instance.

## Selecting rows

A `list` or a `table` selects by `selection`: `single` (the default, the
host's `selected` row), `multiple` (the host's `selected_ranges`, a list
of `{ first, last }`) or `none`. Its events are three, each for one kind of
interaction:

- `on_cursor { row }`: the row the keyboard is on moved — an arrow, a
  page, a type-ahead, a click. Cheap, and often.
- `on_select { row }` in a single selection; `{ row, ranges, count, rows? }`
  in a multiple one, `rows` only up to 1,000, so ⌘A on 100,000 rows is one
  range.
- `on_activate { row }`: Enter or a double click. Without a handler,
  Enter selects, as it always did.

`select_on_focus = false` keeps the arrows off the selection: they move
the cursor, Space selects (toggles, in a multiple selection), Enter
activates and a click selects — a detail panel that loads on `on_select`
or `on_activate` is not loaded for every row the keyboard walks past. In
a multiple selection ⌘-click (Ctrl-click off a Mac) toggles a row,
Shift-click and Shift+arrows extend from the row last clicked or toggled,
and ⌘A (Ctrl+A) selects every row. Escape leaves the selection as it is,
as a Finder or Explorer list does.

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

```tsx
// Node: @qxuken/kui/kit, functions that are also JSX components
import * as kit from '@qxuken/kui/kit';
await runWindowed(app, { extensions: [kit.extension] });
// in a view:
<kit.Toolbar><kit.Button label="Run" variant="primary" onPress={{ kind: 'run' }} /></kit.Toolbar>
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

`app/` is Kit on the `kui` runner (ADR 0064): every component on nine
pages, Kit's design system on four more, a worker building 100,000 rows
as a channel, live chart and log feeds, and an HTTP server — a living
reference, and the template for an app of your own (`kui run --watch
addons/kit/app`, then edit a card). A feed's timer runs only while the
page in view reads it (`demos/feeds.lua`), so the app sits idle on a page
that shows nothing live.

- Every card is a file of its own under `app/demos/`: its data, its
  helpers, the state it keeps in the model, the messages it answers and
  what it draws. Its **Source** button shows that file, read from the
  app's folder, with the app's modules it requires (a read-only `code`
  block with line numbers and a Copy button), so what a card shows is
  what it does. `main.lua` is the shell and every card hears every
  message; `pages.lua` says which cards a page has.
- The **Theme** button opens a panel that sets everything `settings`
  takes — skin, appearance, density, shadows, accent, radius, hairline,
  rule, faces and the key colour roles — with every card following, and
  prints the result as the `kit.settings` call to copy.
- `app/.luarc.json` (and each sample's) points lua-language-server at
  the repository's declarations by relative path, with nothing to
  generate; an app of your own gets one from `kui types`.

## Shaders

Kit carries its own fragment shaders in `shaders/*.wgsl`, which
`build.lua` embeds (`{{wgsl:name}}` in a module) and Kit registers itself
with `env.add_fragment`, so they work under every host:

- `skeleton` draws each bar as the shimmer, the band crossing it per
  pixel, one period for every bar.
- `shader` paints a box with a preset (`shimmer`, or `noise`: gradient
  noise drifting between the well and the accent, both in the skin's
  colours), with WGSL handed over as `source`, or with a fragment the host
  registered (`id`). Kit rounds the box by the skin and keeps a shader
  that reads `time` still under reduced motion.

```lua
kit.shader({ preset = "noise", height = 120 })
kit.shader({ source = MY_WGSL, params = { 0.2, 0.4, 0.9, 1 }, animate = true, height = 120 })
```

Working hosts: `crates/kui-lua/tests/kit.rs` (Rust),
`examples/lua/features/kit/` (the spike scene and its benches),
`examples/c/features/kit/kit.c`, `examples/odin/features/kit/kit.odin`, and
the Kit test in `packages/kui/test.mjs`.

## The design system

Kit describes itself as a design system inside the Kit app, under the
sidebar's **Design system** section, from the files in `app/design/`:

- **Foundations** draws `app/design/README.md`, the brand book (the three
  skins, how Kit's labels and messages are worded, the visual
  foundations, the icons), with Kit's own `markdown` component.
- **Colours** lists every colour role of the skin and appearance the Theme
  panel sets, live from `env.tokens`: a swatch, its value, its usage note
  from `app/design/tokens.json`, and for a text role its contrast on each
  ground the note names.
- **Type and spacing** draws the type styles, the spacing and hairline
  scales to size, the radius and the shadow, in the current theme.
- **Icons** draws every icon with its name.
- Each card's **Guide** button (beside Source) draws the guideline of
  each component the card places, `app/design/components/<Component>.md`:
  when to use it, what the app provides, do's and don'ts.

`app/design/tokens.json` is the tokens for the web — every colour role per
skin and appearance (six themes, Ledger light first), the type styles,
spacing, radii, shadows, sizes and opacities — its values Kit's own, read
live by `app/design/tokens.lua`; the usage notes are the file's:

```bash
kui run --headless addons/kit/app/design/tokens.lua -- --write  # values from Kit
kui run --headless addons/kit/app/design/tokens.lua -- --check  # what the test runs
```

The runner tests keep the docs honest: tokens.json names exactly the
colour roles Kit exports with the values it computes (`--check`), every
component in `schema.lua` has a guideline and every guideline a component,
and the brand book's icon list is `src/icons.tl`'s. A new role, component
or icon fails them until its note, guideline or name is written.

## Layout of this folder

| Path | What |
| --- | --- |
| `schema.lua` | The API: every component's props, events and child groups, with docs. |
| `src/*.tl` | The Teal source: `kit` (the entry), `skin` and the three skins, `ctx` (what every component paints through), `icons`, and the components by group: `layout`, `inputs`, `data`, `overlays`, `charts`, `waterfall`. |
| `lua/*.lua` | What a host loads, generated from `src/`. Edit the `.tl`, never these. |
| `bindings/` | The wrappers generated from `schema.lua`. Never edit these either. |
| `app/` | The Kit app, on the `kui` runner: `main.lua` the shell, `pages.lua` the pages, `theme.lua` the Theme panel, `design.lua` the design system's pages, a card a file under `demos/`. |
| `app/design/` | Kit as a design system: the brand book, `tokens.json` (and `tokens.lua`, which writes its values from Kit), a guideline per component. |
| `shaders/` | Kit's own WGSL, embedded by `build.lua` and registered by Kit: `shimmer.wgsl` (the skeletons' sweep), `noise.wgsl`. |
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
