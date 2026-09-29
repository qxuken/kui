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
{{#include ../../../examples/rust/tutorial/04_controls.rs:toggles}}
```

## The model and its messages

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:model}}
```

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:message}}
```

## A slider proposes a value

A toggle's message is the whole story: the click is the change. A
slider is different. The user drags to *some* value, and the slider
needs to tell you which.

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:slider}}
```

The message you give a slider is a *tag*. When the user drags, a
`change` event arrives with your tag on it and the proposed value
beside it. `ev.message::<Msg>()` finds the tag; `ev.payload
.get_float("value")` reads the value. Store it, and the next frame
draws the knob there.

The value is snapped to the slider's step (5, here) and clamped to its
range before it reaches you.

## A text field owns its text

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:field}}
```

Typing is the one thing the frame cannot redo from the model every
frame — the caret, the selection and the undo history live between
frames. So an editor keeps its text, and the view reads it back with
`ui.edit_text(key)`. `text_input` returns the key.

If you need to *set* the text, `ui.set_edit_text(key, "..")` does it.

## A select speaks by key

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:select}}
```

A select opens a menu of its options. Choosing one posts a `menu` event
on the select's own key, with the chosen label as `item`. The handler
recognises it by comparing `ev.key` to the key the view kept.

## The handler

```rust,noplayground
{{#include ../../../examples/rust/tutorial/04_controls.rs:on_event}}
```

## Try this

- Disable the checkbox while notifications are off. The stock widgets
  have a `_with` form that takes a spec: `toggle_with(.., toggle_spec(&m)
  .checked(..).disabled(!self.notify).on_click(..), None)`. Look at
  [`widgets/controls.rs`](../../../examples/rust/widgets/controls.rs).
- Show the volume as a bar whose width is `Sizing::Percent(self.volume
  / 100.0)`.

## Where this is decided

- Every stock widget:
  [`crates/kui-core/src/widgets.rs`](../../../crates/kui-core/src/widgets.rs).
- Why the stock controls are built over the accessibility roles:
  [ADR 0034](../../adr/0034-stock-controls-over-the-roles.md).
- The `change`, `changed`, `submit` and `menu` events:
  [`docs/props.md`, events](../../props.md#events).
