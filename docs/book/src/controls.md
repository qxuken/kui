# Controls

At the end of this chapter, a settings card has a switch, a checkbox, a
slider, a text field and a select, and each one reports through the
model.

## The controls keep no state

This is the thing to notice. `widgets::switch(ui, "Notifications",
self.notify, Msg::Notify)` draws the switch *on* when `self.notify` is
true. The switch does not remember whether it is on. Your model does.

Click it, and `Msg::Notify` arrives. Your handler flips `self.notify`.
The next frame draws the switch from the new value. If your handler did
nothing, the switch would not move.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:toggles}}
```

## The model and its messages

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:model}}
```

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:message}}
```

## A slider proposes a value

A toggle's message is the whole story: the click is the change. A
slider is different. The user drags to *some* value, and the slider
needs to tell you which.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:slider}}
```

The message you give a slider is what it sends when it moves. When the
user drags, `Msg::Volume` arrives as a `change` event with the proposed
value inside it. Store the value, and the next frame draws the knob
there. A slider without a message does not move at all, and kui warns
`slider-without-handler`: nothing would hear it.

The value is snapped to the slider's step (5, here) and clamped to its
range before it reaches you.

## A message and an event

This is the first event that carries more than your message, so it is
worth being precise about what `ev` holds.

`ev.event` says *what happened*: a click, a `change`, a key press.
`ev.payload` is a `Value`, a small JSON-like map, and it is your
message with the event's data written into it under one field named
for the event:

- **You** wrote the message. `Msg::Volume` becomes `{kind: "volume"}`,
  and `ev.message::<Msg>()` turns it back into the enum. The next
  chapter puts fields on a message — `Msg::Toggle { id }` — and those
  come back typed too, because the derive knows them.
- **The core** adds the event's own data, in one field. A slider's
  `change` arrives as `{kind: "volume", change: {value, phase}}`. A drag
  adds `drag: {x, y, dx, dy, phase, ..}`, a key press `key: {code,
  ..}`. A plain click has nothing to add, so it arrives exactly as you
  declared it.

`ev.split::<Msg>()` reads both halves at once: your message, and a
`kui_native::Event` with one variant per event, its data typed. So one
`match` says *which* control spoke and *what* it said, and a pair that
does not belong together falls through to `_`. For one event at a time
there are typed readers too — `ev.drag()`, `ev.key_press()`,
`ev.scroll()` — and `ev.data()` is the field itself as a `Value`. The
events table of
[`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#events)
lists every event's field and what is in it.

## A text field reports its text

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:field}}
```

An editor reports what it was given messages for. `change: Some(Msg::Name
.into())` asks for every edit, which arrives as `Msg::Name` with the
whole text after the edit inside, under `change`. The handler stores it
in `self.name`. An editor given no messages sends nothing at all.

The `true` says that `&self.name` is the field's *value*, not just its
first text: each frame, where the field's text differs from it, the
field is brought to it. Since the handler stored exactly what the field
reported, the two agree on every frame of ordinary typing, and the
caret, the selection and the undo history are left alone. When they do
not agree — the model was cleared after a submit, say — the field
changes only the span that differs.

`false` makes the text a seed instead: the editor starts from it and
then keeps its own text, which the view can read back with
`ui.edit_text(key)` and replace with `ui.set_edit_text(key, "..")`.
`widgets::text_input(ui, label, "")` is that form with no messages of
its own.

## A select speaks by key

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:select}}
```

A select opens a menu of its options. `widgets::select` takes no
message, so choosing one posts a `menu` event that is only the event's
field, `{menu: {role, item}}`, on the select's own key, with the chosen
label as `item`. `ev.split` reads it as `(None, Event::Menu(choice))`,
and the handler recognises it by comparing `ev.key` to the key the view
kept. A select built with `widgets::select_with` can declare
`.on_menu(Msg::..)` in its spec and be matched by message instead.

## The handler

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:on_event}}
```

## Try this

- Disable the checkbox while notifications are off. The stock widgets
  have a `_with` form that takes a spec: `toggle_with(.., toggle_spec(&m)
  .checked(..).disabled(!self.notify).on_click(..), None)`. Look at
  [`widgets/controls.rs`](examples/widgets/controls.md).
- Show the volume as a bar whose width is `Sizing::Percent(self.volume
  / 100.0)`.

## Where this is decided

- Every stock widget:
  [`kui_core::widgets`](https://docs.rs/kui-core/latest/kui_core/widgets/index.html),
  re-exported as `kui_native::widgets`.
- The stock controls are built over the accessibility roles, so a switch
  is a `switch` to a screen reader and a Tab stop with nothing more from
  you.
- The `change`, `submit` and `menu` events, and the handler each comes
  through:
  [`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#events), events.
- Why an event arrives as the message its handler declared:
  [ADR 0047](https://github.com/qxuken/kui/blob/main/docs/adr/0047-an-event-arrives-as-the-message-its-handler-declared.md).
