# Props by condition

At the end of this chapter, a settings row puts its control under its
label when the window is narrow, a button colours itself by hover and
press, a card grid fits as many columns as there is room for, and a chart
is built only when it can be shown — and none of it runs the view again.

## A view says it once

A view runs when its model changes. A resize, a theme switch or a hover
does not change the model, so a view that read its size or the pointer
to pick its props would be run for each of them, and would learn its own
size a frame late. Instead a node carries **rules**: a `when` list, each
entry the conditions it tests and the props it sets while every one of
them holds.

```rust,noplayground
use kui_core::{Color, Cond, Dir, NodeSpec};

let row = NodeSpec::row()
    .grow_width()
    .gap(16.0)
    // Narrow: a column, the hint dropped.
    .when(Cond::width_below(560.0).band(16.0), |p| {
        p.dir(Dir::Column).gap(6.0).hide("hint")
    })
    // A dark palette: a darker card.
    .when(Cond::Dark, |p| p.bg(Color::hex(0x202024ff)));
```

Every entry that matches applies, in list order, over the node's own
props: a later one wins a prop an earlier one set. So `dark` and a width
entry compose, with no entry for the pair.

## What a condition reads

| condition | holds |
|---|---|
| `width`, `height` | the node's own size: `below` under it, `above` at or over it |
| `window: { width, height }` | the same against the window's content size |
| `dark`, `light` | the palette's appearance |
| `reduced_motion`, `contrast = "more"` | the user's settings |
| `platform`, `scale` | where it runs |
| `hovered`, `pressed`, `focused`, `focus_visible`, `disabled`, `selected` | the node's state |

A threshold is a length, so `below = "$narrow"` reads a length token. A
`band` holds a range past its threshold once it holds: `{ below = 560,
band = 16 }` starts under 560 and lets go at 576, so a width resting on
560 does not flicker between two layouts.

## A node's own size

An entry over the node's own size resolves inside layout: once the
node's width is final, before its children are laid across it. The frame
the window crosses the threshold is already laid out the new way. Such
an entry sets the layout of the node's *children* — direction, gaps,
padding, wrap, alignment, `min_column`, and which keyed children it
hides — and never the node's own width, which is what it reads. A node
that reads its own width is a *container* there: its width cannot come
from its content, so a `fit` width is refused and grows instead, with a
`query-on-fit` warning.

## State and the environment

```lua
row {
  key = "button", on_click = "save", label = "Save",
  bg = "$surface",
  hover_bg = "$hover",
  transition = 120,
  when = {
    { pressed = true, shadow_y = 0, bg = "$pressed" },
    { focus_visible = true, border = { w = 2, color = "$focus" } },
    { reduced_motion = true, transition = 0 },
  },
}
```

These resolve as the node opens, where `hover_bg` always did — it is an
entry ahead of the list, so `pressed` here wins over it — and what they
set eases under the node's `transition`. A hover repaints; the view is
not run, and a component that colours itself this way is replayed from
what it built (the ADR 0067 instance) instead of filled again on every
move of the pointer.

## As many columns as fit

```tsx
<box dir="row" width="grow" gap={12} crossGap={12} minColumn={220}>
  {cards}
</box>
```

The row wraps into equal columns, as many as fit at 220 or more, every
card a column wide — the last line's too, which do not stretch.

## A branch built only when it can be shown

```rust,noplayground
use kui_core::{Needs, NodeSpec, TextStyle};

ui.with(NodeSpec::row().grow_width().gap(12.0), |ui| {
    ui.text("Latency", TextStyle::new(13.0));
    ui.lazy(Needs::w(320.0), |ui| chart(ui))
        .fallback(|ui| {
            ui.text("widen the window for the chart", TextStyle::new(12.0));
        });
});
```

The chart is built when the row offers it 320 px — its content box less
the label and the gap — and laid out at exactly 320, so the decision
never feeds back into the room it was taken from. The decision comes
from the last layout; when this frame's says otherwise, the runner
builds the frame once more before drawing it. On the first frame there
is no last layout, so it lays out first and builds after. `lazy_query`
picks by a condition on the parent's size, the window's or the
environment's instead; a hover cannot pick one, since it must not build
or drop a subtree.

## What the devtools show

The tree tab marks a node with rules `rules`, a lazy branch `lazy` and a
hidden node `hidden`, and the inspector lists each entry as a person
reads it — `width < 560 ±16 → dir, gap, hidden (children)` — with the
ones this frame matched marked.
