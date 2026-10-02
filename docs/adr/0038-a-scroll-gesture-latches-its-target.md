---
status: accepted
date: 2026-09-28
---

# A scroll gesture latches its target

> **Accepted and built 2026-09-28**, the day it was proposed (backlog
> F107). Raised by kawoosh's todo of that day, three items about where
> the wheel goes. A swipe that moved the pane strip carried a terminal
> under the pointer, and the terminal took the rest of the swipe, so
> the strip stopped hard at every terminal it met. A new swipe over a
> list already at its end moved nothing, where the scroller around it
> could have moved. And a swipe over a scroller did not always go the
> scroller's way.
>
> **Amended by the alpha.22 regression pass (2026-09-28):** the pick
> walks the scroller's ancestors in the tree, not the regions painted
> under the pointer (decision 3), and reads what it needs off the region
> the last finished frame drew rather than off the tree, so a wheel
> event that arrives during a build routes by that frame.
>
> **Amended 2026-10-02 (backlog F117, from kawoosh's sticky-swipe
> report):** a finger put down begins a gesture, pause or none
> (decision 2). A swipe started while the last one still glided was the
> glide's gesture, its target and its axis, until a pause came.
>
> **Amended 2026-10-02 (backlog F118, from the same report):** a wheel
> handler that is also a scroll container on an axis is answered there
> by its room (decision 3), so a code view or a table at its sideways
> edge passes a swipe that way to the strip around it.

## Context

- **Where a delta went.** `InputEvent::Scroll` walked the scroll
  regions under the pointer, innermost by paint order first. Each
  scroll container took the axes it scrolls, and the rest went on
  (DX13, so a `scrollY` list hands `x` to the `scrollX` strip around
  it). An `on_scroll` node took whatever was left and stopped the walk.
  Every delta walked afresh, from wherever the pointer was at that
  moment.
- **What moves under a still pointer.** A two-finger swipe does not
  move the pointer. The strip scrolls under it, so a swipe that began
  over a list ends over a terminal. kawoosh's terminal is an
  `on_scroll` node that takes every delta for its history, so the
  moment it arrived under the pointer it took the swipe.
- **What an `on_scroll` node cannot say.** The core keeps no offset for
  it and cannot ask whether it has anywhere to go. A container it can
  ask, from the last layout's geometry.
- **Where gestures begin.** The core is clock-free, as it is for
  clicks. The native runner already has a clock and already tells one
  swipe from the next: F104's axis lock ends a swipe after a 200 ms
  pause. winit's `TouchPhase` is not usable for this alone. It names a
  momentum run's start as it names a finger's (F117 tells them apart by
  order, decision 2).
- **What browsers do.** A wheel or trackpad gesture latches to the
  scroller it started on and keeps it until the gesture ends
  (*latching*). The scroller is picked at the start as the innermost
  one under the pointer that can scroll the way the gesture goes, so a
  scroller at its limit passes the gesture to its ancestor
  (*chaining*). A gesture that reaches a limit partway stops there.
  CSS's `overscroll-behavior: contain` stops the chaining.

## Decisions

1. **A gesture is delimited by the driver.** `InputEvent::ScrollGesture
   { delta, begins }` carries a delta and whether it is its gesture's
   first. A bare `InputEvent::Scroll` is a gesture of its own, so a
   driver that knows nothing of gestures (C's `kui_input_scroll`, Node's
   `ctx.scroll`, the corpus) gets per-event targeting, now with
   chaining. C has `kui_input_scroll_gesture(ctx, dx, dy, begins)` for a
   host with its own loop.
2. **The native runner begins a gesture** on its first event, after a
   pause of 200 ms (F104's `GAP`), on a switch between line and pixel
   deltas, and, for a wheel only, when the pointer moves.
   - A wheel spun without a pause is one gesture, as in a browser, so a
     list reaching its end under a spinning wheel does not hand the
     rest to the page.
   - Moving the mouse is aiming somewhere new, so it ends a wheel's
     gesture.
   - A swipe's momentum outlives the hand, and moving the pointer
     during the glide must not move the glide somewhere else, so it
     does not end a swipe's.
   - An event the axis lock keeps to zero is not dispatched. The
     beginning it carried is owed to the next one that is.
   - *Amended 2026-10-02 (F117):* a finger put down begins a gesture
     too, however soon after the last event, and ends the axis lock's
     swipe. A `Started` within 100 ms of a finger's `Ended` is the
     glide's (macOS starts it on the next event) and keeps the gesture;
     every other `Started` is a finger's — on nothing lifted, or on a
     glide, which macOS ends as the finger lands. Before this, a swipe
     begun mid-glide was the glide's gesture until a 200 ms pause, which
     a glide's events a frame apart never leave.
3. **The target is picked per axis, when the gesture first moves on
   that axis, and latched.**
   - Picking starts at the topmost region under the pointer and walks
     out through the regions around it in the tree (amended by the
     alpha.22 regression pass: it walked every region under the pointer
     by paint order, so a list at its end in a popover chained to a
     page it merely floated over). The first to take the axis is the
     target:
     - an `on_scroll` node takes it if its `scroll_axes` names it,
       whether or not it has anywhere to go — *amended 2026-10-02
       (F118):* unless it scrolls that axis as a container too, its
       offset the app's to set from what it hears; there the core can
       ask, and it is answered as a container is, below;
     - a container takes it if it scrolls on that axis and can still
       move the way the delta goes (more than half a pixel of room,
       measured from where the delta would be added);
     - a container that says `overscroll: contain` takes it in any
       case.
   - Later events of the gesture go to the latched target, found by key
     among the frame's scroll regions wherever the pointer or the
     content has gone.
   - A target that is gone from the frame, or is behind a modal now, is
     dropped, and the axis is picked again under the pointer.
   - An axis that found nothing stays unlatched, so a gesture that turns
     back can still find a scroller.
   - Per axis is what DX13's hand-off becomes: a list's `y` and the
     strip's `x` are two targets. With F104's lock a swipe has one in
     practice, and a lock that turns picks the new axis's target at the
     turn.
4. **No chaining partway through.** A latched container at its limit
   keeps the gesture and moves nothing, as a browser's does, and turning
   back within the gesture moves it again.
5. **`overscroll: "auto" | "contain"`** (`Overscroll`,
   `KUI_OVERSCROLL_*`) opts a container out of being chained past. It
   holds only on the axes the container scrolls. So a contained
   `scrollY` list still passes a sideways swipe to the strip, where CSS
   would stop that too. kui's one-axis scrollers are
   not scroll containers on their other axis.
6. **`scrollAxes: "both" | "x" | "y"`** (`ScrollAxes`,
   `KUI_SCROLL_AXES_*`) lets an `on_scroll` node decline an axis. A
   gesture on an axis it does not take passes it by. This is how a node
   the core cannot ask about says it does not need a gesture: a
   terminal that scrolls only its history says `y`. It is declared
   rather than answered per event, because the pick happens inside
   `handle_input`, before any app code could answer.
7. **Programmatic scrolls, scrollbar drags, keyboard scrolling, the
   primary drag's edge scrolling and `reveal` are untouched.** Only the
   wheel routes through the latch.

## Considered options

- **Re-target on every event, and let `on_scroll` nodes decline by
  axis only.** Declining alone fixes a y-only terminal met by a
  sideways swipe, but a terminal that takes both axes, such as an
  editor's text column, would still grab a strip swipe partway. The
  report's item 3 is about the swipe being cut off, and only latching
  fixes that.
- **An `on_scroll` node answers whether it can move.** An event whose
  reply says "not mine" would need the app in the middle of dispatch,
  and a frame's latency for a question the next delta has already
  asked. `scrollAxes` covers the case that exists: a node that never
  wants one axis.
- **Chain partway, when the target hits its limit.** This is what
  browsers moved away from. A list flicked to its end would start
  moving the page behind it mid-flick, which is the surprise item 2
  asks to avoid by deciding at the start.
- **Keep the gesture clock in the core** (`Core::set_time`). The
  frame clock ticks per frame, not per event, and a clock-free core
  is the rule clicks already follow. The runner has the clock and
  already draws F104's boundaries.
- **winit's `TouchPhase`.** It marks a momentum run as a new start, so
  the momentum would re-target. The gap is what tells a glide from a
  new swipe. *Amended (F117):* the gap alone kept a swipe begun
  mid-glide in the glide's gesture; the phases' order now tells the
  glide's `Started` from a finger's, and both are used.

## Consequences

- A notch over a list at its end now moves the scroller around it (a
  `Scroll` is its own gesture). That changes behaviour only where there
  is one to move. kawoosh's lists sit in an `x`-only strip, so nothing
  moves there.
- An `on_scroll` node in a `both`-axis layout hears a delta with one
  component zero when the other axis is latched elsewhere, where it
  heard the whole delta. F104's lock had already made that the
  trackpad's case.
- `KuiSpec` gains `overscroll` and `scroll_axes` under the unreleased
  ABI 20 (648 bytes on a 64-bit target). `InputEvent` gains a variant,
  a Rust break for a host that matches it exhaustively.
- The thresholds are F104's, a first reading to be tuned on a trackpad.
- Pinned by `tests/scroll_gestures.rs` in kui-core, by
  `scroll_gesture.rs`'s tests in kui-native, by `surface.c`'s check of
  the C door, and by the corpus's `scroll-gestures` scene in four
  adapters. The scene holds a contained list at its limit and a y-only
  handler met by a sideways notch.
