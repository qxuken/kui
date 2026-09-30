# Effects, the clock and the world

At the end of this chapter, the window's title counts seconds, a button
copies to the clipboard, another pastes, and a third closes the window.

So far the app was pure: a model, a view of it, events that change it.
Real apps touch the world — the clipboard, a file, a socket, a timer.
kui has three doors for that, and each keeps the loop's shape.

## Door one: the handler gets the core

`on_event` is `&mut self` and an event. Its sibling `on_event_with`
also gets the **core** of the window the event came from. The core is
where the clipboard, focus and scrolling live:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/09_effects.rs:on_event_with}}
```

`core.set_clipboard(text, None)` writes. There is no `read_clipboard`,
because the core never reads the OS clipboard itself — the host does.
`core.request_paste()` asks, and the text comes back as an event: a
`text` event on the focused key sink. That is why the root of this app
is a sink with focus, as in the keyboard chapter.

Override `on_event_with` and the default `on_event` is not called.
Override only `on_event` and nothing changes.

## Door two: a thread wakes the loop

The loop sleeps between events. A thread that has news — a socket read,
a timer — cannot draw, but it can wake the loop, and then `view` runs:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/09_effects.rs:setup}}
```

`setup` is called once before the window opens, with a `Waker`. Clone
it into as many threads as you like. `waker.wake()` is cheap and safe
at any rate; wakes coalesce into the next turn of the loop.

The thread writes into shared state; the view reads it. There is no
channel into `on_event` — the model *is* the channel.

## Door three: the window is declared

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/09_effects.rs:window}}
```

`ui.window_title(..)` is said every frame, and applied when it changes.
`ui.window_command(WindowCommand::Close(id))` asks the runner to close
a window; `ui.env().window.id` is this one's id. A second window is
declared the same way, with `ui.window(name, ..)` — the README's
multi-window section shows it.

## The messages

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/09_effects.rs:message}}
```

## Try this

- Print something from `teardown`. It runs once as the window goes.
- Make the tick thread stop after 10 seconds. The loop keeps running;
  only the wakes stop.
- Copy, then paste into another app. Then paste from another app into
  this one.

## Where this is decided

- The handler gets its window: ADR 0036.
- Effects as data, the Node driver's version of the same idea:
  ADR 0013.
- The reference examples:
  [`features/waker.rs`](examples/features/waker.md),
  [`features/clipboard.rs`](examples/features/clipboard.md).
