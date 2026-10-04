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

The message you give a slider is a *tag*. When the user drags, a
`change` event arrives with your tag on it and the proposed value
beside it. Store the value, and the next frame draws the knob there.

The value is snapped to the slider's step (5, here) and clamped to its
range before it reaches you.

## Two kinds of data on an event

This is the first event that carries more than your message, so it is
worth being precise about what `ev` holds.

`ev.payload` is a `Value`: a small JSON-like map. Two things write into
it.

- **You**, through the message. `Msg::Volume` becomes `{kind:
  "volume"}`, and `ev.message::<Msg>()` turns it back into the enum.
  The next chapter puts fields on a message — `Msg::Toggle { id }` —
  and those come back typed too, because the derive knows them.
- **The core**, through the event's own fields. A `change` event adds
  `value`. A drag adds `x`, `y` and `phase`. A key press adds `code`
  and the modifiers. The core does not know your enum, so these sit
  beside your message in the map, and you read them by name:
  `ev.payload.get_float("value")`, `get_str("code")`, `get_int(..)`,
  `get_bool(..)`. Each returns an `Option`, `None` when the field is
  absent.

So the pattern for any event that carries more than a click is: match
the message to learn *which* control spoke, then read the core's fields
to learn *what* it said. The events table of
[`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#events) lists every event's fields.

## A text field owns its text

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:field}}
```

Typing is the one thing the frame cannot redo from the model every
frame — the caret, the selection and the undo history live between
frames. So an editor keeps its text, and the view reads it back with
`ui.edit_text(key)`. `text_input` returns the key.

If you need to *set* the text, `ui.set_edit_text(key, "..")` does it.

## A select speaks by key

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/04_controls.rs:select}}
```

A select opens a menu of its options. Choosing one posts a `menu` event
on the select's own key, with the chosen label as `item`. The handler
recognises it by comparing `ev.key` to the key the view kept.

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
- The `change`, `changed`, `submit` and `menu` events:
  [`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#events), events.
