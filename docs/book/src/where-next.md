# Where next

You have the whole model now: a view of the model, messages back,
keys, floats, motion, the three doors out, and a test. What follows is
where to look for the rest.

## The reference gallery

[`examples/README.md`](../../../examples/README.md) lists every example
by kind. Each shows one thing, in every state it has, inside the
devtools:

- **`widgets/`** — one element or stock widget each: `button`, `edit`,
  `select`, `table`, `virtual_list`, `cells` (a terminal grid),
  `image`, `fragment` (a box a shader paints), `menu_bar`, `titlebar`.
- **`features/`** — one behaviour each: `hover`, `drag`, `drop`,
  `focus`, `selection`, `clipboard`, `audio`, `accessibility`,
  `theme`, `metrics`, `transition`, `enter_exit`, `waker`.
- **`apps/`** — how it composes: `splitmux` (a tiling pane
  multiplexer), `modal_editor` (a Helix-style editor), `syntax_view`.

Reading order after this book: `features/focus.rs`, then
`widgets/edit.rs`, then `apps/splitmux.rs`.

## The two reference documents

- [`docs/howto.md`](../../howto.md) — "How do I…" questions, each with a
  two-sentence answer and a link. Animate a removal, draw a connector,
  make a popup taller than the window, give the app a menu bar, test
  the real window.
- [`docs/props.md`](../../props.md) — every prop, element, event,
  warning and theme colour, with its Node, Lua and C spelling in the
  same row. Generated from the schema, so it cannot drift.

## The decisions

[`docs/adr/`](../../adr/) holds the architecture decision records: why
focus is one node, why a modal is a scope, why effects are data, why
the devtools belong to the core. Read one when a rule in this book
seems arbitrary; the ADR has the context.

## The other bindings

The same tree, the same events, the same core. What changes is the
spelling:

| Rust | Node (JSX) | Lua | C |
|---|---|---|---|
| `NodeSpec::row().pad(8.0)` | `<box dir="row" pad={8}>` | `row { pad = 8 }` | `kui_open(ctx, &spec, NULL)` |
| `widgets::button(ui, "OK", Msg::Ok)` | `<button onClick={{ kind: 'ok' }}>OK</button>` | `button { label = "OK", on_click = { kind = "ok" } }` | `kui_button(ctx, "OK", payload)` |
| `on_event(ev)` | `update(model, msg)` | `on_event(ev)` | `kui_poll_event` |
| `testing::Drive` | `createApp(..)`, `app.press(..)` | the host's drive | `--headless` |

The Node package ([`packages/kui`](../../../packages/kui)) has the
richest second driver: an Elm-style `update` that returns the model and
effects, a headless `createApp` that runs `tick` too, and the same
devtools. Its README is the place to start.

## What this book left out

Multiple windows, the menu bar, drag and drop from the OS, images and
shaders, sound, selection and the clipboard's finer points, tokens and
themes, accessibility beyond names, and extensions (a Lua or C guest
filling a slot in your frame). Each has a reference example and a
`howto` entry.
