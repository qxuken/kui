# Motion

At the end of this chapter, a panel springs between two widths, and
toasts slide in and out.

## You declare the target, kui eases

There is no animation API to call. A node says `.transition(ms)`, and
from then on any value on it that changes between frames eases from
the old value to the new one over that long. Width, colour, position,
opacity — whatever changed.

```rust,noplayground
{{#include ../../../examples/rust/tutorial/08_motion.rs:transition}}
```

The view says the width is 160 or 360. Nothing else. `.easing(..)` picks
the curve: `EaseOut` is the default, `Spring` overshoots a little,
`Bouncy` more.

This only works because the panel has a key. kui matches this frame's
`panel` to last frame's `panel` and sees the width changed. Without a
stable key, there is nothing to ease from.

## Entering and leaving

A row that appears this frame has no last frame to ease from. `.enter`
says where it comes from. A row that is gone this frame cannot be
drawn — it is not declared — so `.exit` says where it goes, and kui
replays the last frame's copy of it on the way out.

```rust,noplayground
{{#include ../../../examples/rust/tutorial/08_motion.rs:enter_exit}}
```

`Enter::from(dx, dy)` is an offset in pixels. The toast starts 240px to
the right and eases to its place; on the frame it is removed, it eases
back out.

Keys again: each toast is `with_indexed(id, ..)`. Dismiss the first and
the second keeps its key, so it slides *up* into the gap instead of
being mistaken for the first.

## Frames while animating

While something is easing, kui asks the window for frames on its own.
Your `view` runs each time, declares the same target, and the ease
continues. When everything is still, the loop sleeps until the next
event. You never request frames for motion.

## Try this

- Replace `Easing::Spring` with `Easing::Linear`. Then `Bouncy`.
- Add `.opacity(0.0)` to the `Enter` (it is a builder: `Enter::from(240.0,
  0.0).opacity(0.0)`) so toasts fade as they slide.
- Remove the `id` and use `ui.with(..)`. Dismiss the first toast and
  watch the second one jump.

## Where this is decided

- Every animation prop — `transition`, `easing`, `enter`, `exit`,
  `keyframes`, `slide`: [`docs/props.md`](../../props.md#container-props).
- Why a frame that removes too many exiting nodes animates none:
  [ADR 0012](../../adr/0012-the-exit-budget.md).
- The reference examples:
  [`features/transition.rs`](../../../examples/rust/features/transition.rs),
  [`features/enter_exit.rs`](../../../examples/rust/features/enter_exit.rs).
