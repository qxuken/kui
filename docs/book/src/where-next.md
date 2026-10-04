# Where next

You have the whole model now: a view of the model, messages back,
keys, floats, motion, the three doors out, and a test. What follows is
where to look for the rest.

## The reference gallery

[The Examples part](examples/index.md) of this book lists every
example by kind, with its whole source. Each shows one thing, in every
state it has, inside the devtools:

- **[`widgets/`](examples/widgets.md)** — one element or stock widget each: `button`, `edit`,
  `select`, `table`, `virtual_list`, `cells` (a terminal grid),
  `image`, `fragment` (a box a shader paints), `menu_bar`, `titlebar`.
- **[`features/`](examples/features.md)** — one behaviour each: `hover`, `drag`, `drop`,
  `focus`, `selection`, `clipboard`, `audio`, `accessibility`,
  `theme`, `metrics`, `transition`, `enter_exit`, `waker`.
- **[`apps/`](examples/apps.md)** — how it composes: `splitmux` (a tiling pane
  multiplexer), `modal_editor` (a Helix-style editor), `syntax_view`.

Reading order after this book: [`features/focus.rs`](examples/features/focus.md), then
[`widgets/edit.rs`](examples/widgets/edit.md), then [`apps/splitmux.rs`](examples/apps/splitmux.md).

## The reference documents

- [docs.rs/kui-native](https://docs.rs/kui-native) — the API reference:
  every type and method, with
  [kui-core](https://docs.rs/kui-core) underneath it.
- [`docs/howto.md`](https://github.com/qxuken/kui/blob/main/docs/howto.md) — "How do I…" questions, each
  with a two-sentence answer and a link. Animate a removal, draw a
  connector, make a popup taller than the window, give the app a menu
  bar, test the real window.
- [`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md) — every prop, element, event,
  warning and theme colour, with its Node, Lua and C spelling in the
  same row. Generated from the schema, so it cannot drift.

## The design records

[`docs/adr/`](https://github.com/qxuken/kui/tree/main/docs/adr) in the
repository holds the design records: why focus is one node, why a modal
is a scope, why effects are data, why the devtools belong to the core.
Read one when a rule in this book seems arbitrary.

## The other bindings

The same tree, the same events, the same core. What changes is the
spelling:

| Rust | Node (JSX) | Lua | C |
|---|---|---|---|
| `NodeSpec::row().pad(8.0)` | `<box dir="row" pad={8}>` | `row { pad = 8 }` | `kui_open(ctx, &spec, NULL)` |
| `widgets::button(ui, "OK", Msg::Ok)` | `<button onClick={{ kind: 'ok' }}>OK</button>` | `button { label = "OK", on_click = { kind = "ok" } }` | `kui_button(ctx, "OK", payload)` |
| `on_event(ev)` | `update(model, msg)` | `on_event(ev)` | `kui_poll_event` |
| `testing::Drive` | `createApp(..)`, `app.press(..)` | the host's drive | `--headless` |

The Node package ([`packages/kui`](https://github.com/qxuken/kui/blob/main/packages/kui/README.md)) has the
richest second driver: an Elm-style `update` that returns the model and
effects, a headless `createApp` that runs `tick` too, and the same
devtools. Its README is the place to start.

## What this book left out

Multiple windows, the menu bar, drag and drop from the OS, images and
shaders, sound, selection and the clipboard's finer points, tokens and
themes, accessibility beyond names, and extensions (a Lua or C guest
filling a slot in your frame). Each has a reference example and a
`howto` entry.
