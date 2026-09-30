# Testing without a window

At the end of this chapter, `cargo test` clicks buttons, presses a key
and reads the screen — with no window.

## Why it works

The core owns no window. A frame is your `view` run against a viewport
and a clock the caller provides. Input is a value handed in. So a test
can run the real `view` and the real `on_event`, and assert on what the
frame produced, the way a user would see it.

`kui_native::testing::Drive` is the driver.

## The app under test

A smaller cousin of chapter 6's list: an Add button, a Clear button,
a row per item, and a summary line. Two boxes are given keys — `list`
and `summary` — so the test can find them by name; the buttons and the
checkboxes are found by their text, since the stock widgets key
themselves that way.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/10_testing.rs:view}}
```

## The tests

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/10_testing.rs:tests}}
```

Run them:

```bash
cargo test -p kui-native --example tutorial_10_testing
```

## What the drive does

- **`Drive::new(core, w, h)`** — a core and a viewport. `.framing()`
  makes it build a frame after every gesture, so the view has caught
  up before the next line.
- **`d.frame(&mut app)`** — one frame, as the runner would build it.
- **`d.key_of("Add")`** — the key of the node declared under that
  name. Buttons are keyed by their text; your own boxes by what you
  gave `with_keyed` or `text_in_keyed`.
- **`d.click_key(&mut app, key)`** — a click by key, the way a screen
  reader presses. No coordinates. `d.click(app, x, y)` is the pointer
  version, and `d.rect_of(key)` gives you the rect to aim at.
- **`d.key(&mut app, "a", KeyMods::NONE)`** — a key press and release,
  by name. `d.keys(app, "jj ww")` types a sequence.
- **`d.texts_under("list")`** — every text inside the node with that
  key, in order. What the user would read.
- **`d.warnings()`** — every warning the core raised. A test that
  asserts it empty catches an unnamed button or a duplicate key before
  a user does.

There is more — `hover`, `drag`, `wheel`, `advance(secs)` to move the
clock for a transition — in the module's docs.

## What to assert on

Two things, and both are cheap here:

- the **model** — `app.items.len()` — because that is what the app
  believes;
- the **frame** — `texts_under`, `rect_of` — because that is what the
  user sees, and a view that ignores its model would pass the first
  assert and fail this one.

## Try this

- Assert that the second row's checkbox is *not* ticked after ticking
  the first. (`app.items[1].done`, and the frame: the checkbox's
  `checked` is in the node's access tree — `d.core.access_tree()`.)
- Add `d.advance(1.0)` and a frame, then assert something about a
  transition. Chapter 9's toasts are a good subject.

## Where this is decided

- The headless driver: [`crates/kui-native/src/testing.rs`](../../../crates/kui-native/src/testing.rs).
- The README's [Testing without a window](../../../README.md#testing-without-a-window),
  which shows the Node version of the same test.
- A larger example with its own `mod tests`:
  [`apps/splitmux.rs`](../../../examples/rust/apps/splitmux.rs).
