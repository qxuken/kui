# State and messages

At the end of this chapter, three buttons change a number.

## The shape of an app

An app has three parts:

1. A **model**: the state, held in your struct.
2. A **view**: a function of the model that declares the frame.
3. An **`on_event`**: a function that receives what the user did and
   changes the model.

The loop is: `view` runs, the frame is drawn, the user clicks, the
click becomes an event, `on_event` changes the model, `view` runs
again. You write the three parts. kui runs the loop.

## The model

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/03_counter.rs:model}}
```

## The messages

A button needs to say *what* it means when clicked. In kui, it says so
with a value that travels through the frame and comes back in
`on_event`. The value is any `Value` — a map, a string, a number — but
building those by hand is tedious and untyped.

`#[derive(Message)]` does it for you:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/03_counter.rs:message}}
```

Each variant becomes a small map on the way out: `Msg::Inc` is
`{kind: "inc"}`. On the way back, `ev.message::<Msg>()` turns the map
into `Msg::Inc` again. Variants can carry fields; chapter 6 uses one.

## The view

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/03_counter.rs:view}}
```

`widgets::button(ui, text, message)` is the stock button. It draws its
rest, hover and pressed states, takes keyboard focus, and sends its
message when clicked. You give it the message and forget about it.

## The handler

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/03_counter.rs:on_event}}
```

`ev.message::<Msg>()` is `Some` when the event carries one of your
messages, and `None` when it is something else the frame produced — a
resize, a window losing focus. Match exhaustively and the compiler
tells you when you add a variant and forget to handle it.

`on_event` runs after the frame, once per event, in order. It never
runs during `view`. So the model changes between frames, never in the
middle of one.

## Why data, not callbacks

A closure on the button would be shorter. But a closure is opaque: it
cannot be logged, replayed, tested or sent across a language boundary.
A message can. The devtools show every message as it arrives. A test
asserts on them. Node, Lua and C receive the same maps.

## Try this

- Add `Msg::Double`. The compiler will point at the `match`.
- Put `.on_click(Msg::Inc)` on the root column instead of a button
  (`NodeSpec::column().fill().on_click(Msg::Inc)`). Now the whole
  window is a button. Any box can carry a message.
- Open the devtools (`.devtools(true)` on the launcher) and watch the
  events tab as you click.

## Where this is decided

- `#[derive(Message)]` and its attributes:
  [`kui_derive`](https://docs.rs/kui-derive) on docs.rs, and
  [`docs/howto.md`](https://github.com/qxuken/kui/blob/main/docs/howto.md).
- The full click payload, and every other event's shape:
  [`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#events), events.
- The reference counter, with a context menu:
  [`examples/rust/apps/counter.rs`](examples/apps/counter.md).
