# Floats and modals

At the end of this chapter, hovering a badge shows a tooltip, and a
Delete button opens a dialog that Escape closes.

## A float is out of the flow

Everything so far sat inside its parent and took up room. A **float**
does neither: it is positioned against an anchor and drawn on top.
Tooltips, menus and dialogs are floats.

## The tooltip prop

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/07_floats.rs:tooltip}}
```

`.tooltip(text)` is enough. The node tracks its own hover, and kui
floats the text below it while the pointer is over it or over anything
inside it; with tooltips nested, the innermost one shows. Near the bottom of the window
it flips above, so it never hangs off the edge. The same text is what a
screen reader says after the node's name.

## A modal the app declares

A dialog is a float you declare when the model says so and stop
declaring when it is answered:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/07_floats.rs:modal}}
```

Three props do the work:

- **`.float(FloatConfig::viewport().inside(Center, Center))`** —
  anchored to the window, centred. `FloatConfig::parent()` anchors to
  the parent box instead, and `.offset(x, y)` nudges either.
- **`.modal(msg)`** — while this float is declared, Tab stays inside
  it, the pointer cannot reach what is behind it, and Escape or a press
  outside arrives as `msg`, a `dismiss` event with its reason inside.
- **`.label("Delete?")`** — the dialog's name. A button is named by
  the text inside it, but a dialog is not: a screen reader announcing
  "dialog" with nothing after it tells the user nothing, and the
  platforms do not build a name from a dialog's contents. So a modal
  needs a `label`, and kui warns with `modal-without-name` if it has
  none. The label is not drawn; the title text inside the dialog is a
  separate node, and it is fine for the two to say the same thing.

kui does not close the dialog. It reports `dismiss`, and the app
decides. Here it stops declaring:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/07_floats.rs:on_event}}
```

## Order matters

Floats stack in the order they are declared. The dialog is declared
last in `view`, so it is on top. Declare it first and the page would
paint over it — and kui would warn, `modal-behind-content`.

## Where a float is cut

A float anchored to the window, like this dialog, is placed against the
window and shows whatever box it was declared in. One anchored to its
parent (`FloatConfig::parent()`, `below()`, `above()`) is part of what
that parent holds: the nearest box above it that clips or scrolls cuts
it, and it takes presses only where it shows. A menu hung off a button in
a scroller says it leaves, `.escaping()`, so it opens whole over
whatever is beside the scroller; the hint a `.tooltip` floats leaves on
its own. Nothing else takes a node out of its box — not a move, a turn
or an animation ([ADR
0074](https://github.com/qxuken/kui/blob/main/docs/adr/0074-paint-stays-in-the-box-it-is-in.md)).

## Try this

- Right-click anywhere and open a menu there. `.on_context_menu(msg)`
  on the root delivers `msg` as a `contextmenu` event, the point inside
  it (`ev.split` reads it as `Event::ContextMenu(at)`); declare
  a modal float at `FloatConfig::viewport().inside(Start, Start)
  .offset(x, y).fit()`. The reference counter does exactly this.
- Give the dialog `.initial_focus()` on the Cancel button, so Enter
  cancels.

## Where this is decided

- The reference examples:
  [`widgets/tooltip.rs`](examples/widgets/tooltip.md),
  [`features/modal.rs`](examples/features/modal.md),
  [`apps/counter.rs`](examples/apps/counter.md) for the
  context menu.
