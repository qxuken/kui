# Motion

At the end of this chapter, a panel springs between two widths, and
toasts slide in and out.

## The messages

Nothing new here — a toggle, a notify, and a dismiss that names a
toast:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/08_motion.rs:message}}
```

## You declare the target, kui eases

There is no animation API to call. A node says `.transition(ms)`, and
from then on any value on it that changes between frames eases from
the old value to the new one over that long. Width, colour, position,
opacity — whatever changed.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/08_motion.rs:transition}}
```

The view says the width is 160 or 360. Nothing else. `.easing(..)` picks
the curve: `EaseOut` is the default. The springs keep their momentum
when the target moves mid-flight, and they differ in one thing, how far
they overshoot: `Smooth` not at all, `Snappy` a trace, `Spring` a
little, `Bouncy` more. That amount is the spring's *bounce*, 0 to 0.9,
and `.bounce(..)` sets any other — with the duration, it is all a
spring takes. No stiffness or damping to tune.

This only works because the panel has a key. kui matches this frame's
`panel` to last frame's `panel` and sees the width changed. Without a
stable key, there is nothing to ease from.

## Entering and leaving

A row that appears this frame has no last frame to ease from. `.enter`
says where it comes from. A row that is gone this frame cannot be
drawn — it is not declared — so `.exit` says where it goes, and kui
replays the last frame's copy of it on the way out.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/08_motion.rs:enter_exit}}
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
- Keep `Spring` and add `.bounce(0.6)`, then `.bounce(0.0)`: the same
  spring, overshooting more, then not at all.
- Add `.opacity(0.0)` to the `Enter` (it is a builder: `Enter::from(240.0,
  0.0).opacity(0.0)`) so toasts fade as they slide.
- Remove the `id` and use `ui.with(..)`. Dismiss the first toast and
  watch the second one jump.

## Where this is decided

- Every animation prop — `transition`, `easing`, `bounce`, `enter`,
  `exit`, `keyframes`, `slide`: `docs/props.md`.
- Why a frame that removes too many exiting nodes animates none:
  ADR 0012.
- The reference examples:
  [`features/transition.rs`](examples/features/transition.md),
  [`features/spring.rs`](examples/features/spring.md) (a spring's
  duration and bounce on two sliders),
  [`features/enter_exit.rs`](examples/features/enter_exit.md).
