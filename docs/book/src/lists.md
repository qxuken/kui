# Lists and keys

At the end of this chapter, a to-do list adds, ticks, removes and
filters rows, in a box that scrolls.

## Rows from a `Vec`

A list is a loop in `view`. For each item in the model, declare a row.
That is all.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/05_list.rs:model}}
```

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/05_list.rs:list}}
```

## Keys

Here is the question keys answer. Frame 1 declares rows A, B, C. Frame
2 declares B, C — A was removed. Is frame 2's first row a new row, or
is it B moved up?

kui needs to know, because a row carries things between frames: its
hover, its focus, its scroll position, a transition in flight. If the
first row is "B moved up", B keeps its hover. If it is "a new row",
everything resets.

By default a child's key is its *position*, so frame 2's first row
would be taken for A. That is wrong for a list. So each row gets a key
of its own, from the data:

- **`ui.with_indexed(id, spec, ..)`** — the key is a number you own: an
  id, a database row.
- **`ui.with_keyed("name", spec, ..)`** — the key is a string. The
  stock widgets use their text this way, which is why a button is found
  by its label.
- **`ui.with(spec, ..)`** — the key is the position. Fine for a layout
  that does not change shape.

Two rows with the same key in one frame is a mistake, and kui says so
with a `duplicate-key` warning.

## Messages with fields

Each row's checkbox and remove button need to say *which* row. A
message variant carries the id:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/05_list.rs:message}}
```

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/05_list.rs:on_event}}
```

## Scrolling

`.scroll_y()` on a box with a fixed height makes its content scroll.
The wheel moves it, a scrollbar appears when it overflows, and the
scroll position is kept between frames — under the box's key, which is
why the list box would need a key of its own if it ever moved.

## When the list is long

A loop over ten thousand rows declares ten thousand rows every frame.
For that, `widgets::uniform_list` declares only the rows in view and
two spacers, from a row height and a count. It is the same idea with
the loop inverted; see
[`widgets/virtual_list.rs`](../../../examples/rust/widgets/virtual_list.rs)
when you need it.

## Try this

- Remove `with_indexed` and use `ui.with(..)` for the rows. Hover a
  row, then remove the row above it. The hover jumps.
- Add a "Clear done" button. One `retain`.

## Where this is decided

- Keys, and the `duplicate-key` warning:
  [`docs/props.md`, warnings](../../props.md#warnings).
- Caching against the last frame, which is what keys make possible:
  [ADR 0016](../../adr/0016-caching-against-the-last-frame.md).
