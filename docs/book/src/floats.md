# Floats and modals

At the end of this chapter, hovering a badge shows a tooltip, and a
Delete button opens a dialog that Escape closes.

## A float is out of the flow

Everything so far sat inside its parent and took up room. A **float**
does neither: it is positioned against an anchor and drawn on top.
Tooltips, menus and dialogs are floats.

## The tooltip prop

```rust,noplayground
{{#include ../../../examples/rust/tutorial/07_floats.rs:tooltip}}
```

`.tooltip(text)` is enough. The node tracks its own hover, and kui
floats the text below it while hovered. Near the bottom of the window
it flips above, so it never hangs off the edge. The same text is what a
screen reader says after the node's name.

## A modal the app declares

A dialog is a float you declare when the model says so and stop
declaring when it is answered:

```rust,noplayground
{{#include ../../../examples/rust/tutorial/07_floats.rs:modal}}
```

Three props do the work:

- **`.float(FloatConfig::viewport().inside(Center, Center))`** —
  anchored to the window, centred. `FloatConfig::parent()` anchors to
  the parent box instead, and `.offset(x, y)` nudges either.
- **`.modal(tag)`** — while this float is declared, Tab stays inside
  it, the pointer cannot reach what is behind it, and Escape or a press
  outside becomes a `dismiss` event carrying the tag.
- **`.label("Delete?")`** — a dialog's name, for a screen reader.

kui does not close the dialog. It reports `dismiss`, and the app
decides. Here it stops declaring:

```rust,noplayground
{{#include ../../../examples/rust/tutorial/07_floats.rs:on_event}}
```

## Order matters

Floats stack in the order they are declared. The dialog is declared
last in `view`, so it is on top. Declare it first and the page would
paint over it — and kui would warn, `modal-behind-content`.

## Try this

- Right-click anywhere and open a menu there. `.on_context_menu(tag)`
  on the root delivers a `contextmenu` event with `x` and `y`; declare
  a modal float at `FloatConfig::viewport().inside(Start, Start)
  .offset(x, y).fit()`. The reference counter does exactly this.
- Give the dialog `.initial_focus()` on the Cancel button, so Enter
  cancels.

## Where this is decided

- Modal surfaces: [ADR 0003](../../adr/0003-modal-surfaces.md).
- Layers stack in the order they open: [ADR 0023](../../adr/0023-layers-stack-in-the-order-they-open.md).
- The reference examples:
  [`widgets/tooltip.rs`](../../../examples/rust/widgets/tooltip.rs),
  [`features/modal.rs`](../../../examples/rust/features/modal.rs),
  [`apps/counter.rs`](../../../examples/rust/apps/counter.rs) for the
  context menu.
