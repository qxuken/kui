# Hello, window

At the end of this chapter, a window shows one line of text.

## The whole program

```rust,noplayground
{{#include ../../../examples/rust/tutorial/01_hello.rs:all}}
```

Run it:

```bash
cargo run -p kui-native --example tutorial_01_hello
```

## What each line does

**`struct Hello;`** — the app is a type you own. It will hold your state.
This one has none, so it is a unit struct.

**`impl App for Hello`** — `App` is the trait the runner drives. It has
one required method, `view`. The others (`on_event`, `setup`,
`teardown`) have defaults, and later chapters fill them in.

**`fn view(&mut self, ui: &mut Ui<'_>)`** — called once per frame. `ui`
is the builder for this frame's tree. Whatever you declare through it
is what the frame shows. When `view` returns, the frame is complete.

**`ui.theme()`** — the colours the platform asked for: light or dark,
with the system accent. `theme.fg` is the foreground. Use the theme's
colours rather than literals and the app follows the OS.

**`ui.text(..)`** — one text node. `TextStyle::new(24.0)` is the size in
logical pixels; `.color(..)` sets the colour.

**`kui_native::app("Hello")`** — a launcher. `.size(w, h)` is the
window's initial size, `.run(app)` opens it and drives the loop until
the window closes.

## Why there is no `render()`

You never draw. You declare, and the frame is drawn from the
declaration. The next frame is declared from scratch, and kui works out
what changed. This is the rule from the introduction, and it is the
only thing to hold on to for the next few chapters.

## Try this

- Change the text and the size. Save, run again.
- Add a second `ui.text(..)` below the first. Where does it go?
  (Below: the root is a column. The next chapter says why.)

## Where this is decided

- The `App` trait: [`crates/kui-native/src/lib.rs`](../../../crates/kui-native/src/lib.rs).
- The frame as a function of its inputs: the README's
  [Testing without a window](../../../README.md#testing-without-a-window).
