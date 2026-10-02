# Setup

The introduction's three commands are the whole setup on macOS and
Windows. This page is the rest: the dependency in detail, your project,
Linux, and the devtools.

## The dependency

kui's crates publish to crates.io, so the dependency is one line:

```toml
[dependencies]
kui-native = "0.1.0-alpha.33"
```

`cargo add kui-native` writes that line for you. Every version is an
alpha for now, and a version requirement does not hold an alpha still:
`"0.1.0-alpha.33"` admits every later `0.1.0` alpha, so it is a floor,
and `Cargo.lock` is what keeps the version you tested. Write
`"=0.1.0-alpha.33"` to pin it in the manifest.

Versions before 0.1.0-alpha.33 are on kui's own Forgejo registry only. A
project that names it — `[registries.drydock9]` with index
`sparse+https://drydock9.qxuken.dev/api/packages/qxuken/cargo/` and
`registry = "drydock9"` on the dependency — keeps working; dropping the
`registry` key is the whole move to crates.io.
`kui-native` is the batteries-included crate: a window, a GPU renderer,
the stock widgets and the `App` trait. Everything the book uses is
reachable from it.

## Your project

The book's programs are examples in the kui repository, which is not
public, so each one is complete on the page: a chapter's code block
shows the lines it is about, and the eye icon in its corner reveals the
rest of the file. Copy the whole program into `src/main.rs` of a
project set up as above, and run it:

```bash
cargo run
```

Every chapter's *Run it* line is spelled that way. The testing
chapter's program carries its tests, and `cargo test` runs them.

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

## Building this book

The book is built with [mdBook](https://rust-lang.github.io/mdBook/),
from `docs/book` in the repository:

```bash
cargo install mdbook
mdbook serve docs/book --open
```

`serve` rebuilds on every save and reloads the browser.
