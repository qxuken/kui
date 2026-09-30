# Keyboard and focus

At the end of this chapter, the arrow keys move a cursor over a grid,
space lights a cell, Tab walks the cells and the button, and the screen
says where focus is.

## Focus is one node

At any moment, one node has keyboard focus, or none does. Tab moves it
to the next node in the *ring*: every button, control and editor, in
tree order. Shift-Tab moves back. Enter or space presses the focused
button.

You get all of that without writing anything. The stock widgets are in
the ring. A plain box joins it with `.focusable()`.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/06_keyboard.rs:grid}}
```

`.focus_bg(..)` is what a focusable box shows while focused. Without
it, kui draws a ring.

## Reading focus

The view can ask where focus is:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/06_keyboard.rs:readout}}
```

`ui.focus_visible()` is true when focus got there by keyboard, and
false after a click. It is how a view shows a focus ring only when the
ring is useful.

## Keys are events on a sink

A key press is data, like a click. It is delivered to a **key sink**: a
box with `.on_key(tag)`. The press goes to the focused node and bubbles
up to the nearest sink above it. So a sink at the root hears every key,
whatever is focused inside it.

The tag is a message of yours, like a button's — except a sink sends it
on every key rather than on a click. This app has one sink, so one
variant with no fields is enough to say "a key arrived":

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/06_keyboard.rs:message}}
```

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/06_keyboard.rs:sink}}
```

A sink hears keys only while something under it has focus. On the
first frame nothing does, so the view gives the sink focus itself with
`ui.take_key_focus(sink)`. That runs on the frame the declaration
*starts* and not again, so a later Tab is not undone.

## Reading a key

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/06_keyboard.rs:on_event}}
```

The message says which sink. `ev.key_press()` reads the key itself:
a `KeyPhase` (down or up — a sink hears releases only if it asks with
`.key_up()`) and a `KeyPress` with the `code`, the modifiers and the
text the press would type.

`KeyCode::Char('a')` is a character key, as the keyboard layout
produced it. `KeyCode::Left` and the rest are the named keys.

## Try this

- Bind `KeyCode::Enter` to lighting the cell too.
- Press Tab until a cell has focus, then space. The cell lights —
  and it was the sink that heard the space, not the cell. Now make the
  cell a real button (`.on_click`) and see Enter press it.

## Where this is decided

- One focus, every control reachable: [ADR 0002](../../adr/0002-keyboard-focus-as-data.md).
- Keys bubble to the enclosing sink: [ADR 0011](../../adr/0011-keys-bubble-to-the-enclosing-sink.md).
- The full `key` event: [`docs/props.md`, events](../../props.md#events).
- The reference example, with focus regions and the verbs that move
  focus: [`examples/rust/features/focus.rs`](../../../examples/rust/features/focus.rs).
