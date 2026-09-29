# Setup

By the end of this page, `cargo run` opens an empty window.

## The dependency

kui is an alpha. It is published to a private registry, not to
crates.io, so the simplest way to follow the book is from a checkout of
the repository:

```bash
git clone https://drydock9.qxuken.dev/qxuken/kui
cd kui
cargo run -p kui-native --example tutorial_01_hello
```

That builds the whole workspace once (a few minutes) and opens the
first chapter's window.

For an app of your own, depend on `kui-native` by path or by git:

```toml
[dependencies]
kui-native = { git = "https://drydock9.qxuken.dev/qxuken/kui" }
```

`kui-native` is the batteries-included crate: a window, a GPU renderer,
the stock widgets and the `App` trait. Everything the book uses is
reachable from it.

## Linux

The build wants `pkg-config` and ALSA's headers (`libasound2-dev` on
Debian and Ubuntu). A window loads the rest at run time: `libxkbcommon`
for the keyboard, the X11 or Wayland client libraries, and Vulkan or
EGL for the GPU. A missing one fails when the window opens, with the
library's name in the error.

macOS and Windows need nothing beyond a Rust toolchain.

## The devtools

Every app can open a panel that shows the frame's events, the node tree
and the runtime's facts. Turn it on with one call on the launcher:

```rust,noplayground
kui_native::app("Hello").devtools(true).run(Hello)
```

or from outside, with `KUI_DEVTOOLS=1` in the environment. Keep it open
while you read. Chapter 12 walks through its tabs.

## Running the book itself

The book is built with [mdBook](https://rust-lang.github.io/mdBook/):

```bash
cargo install mdbook
mdbook serve docs/book --open
```

`serve` rebuilds on every save and reloads the browser.
