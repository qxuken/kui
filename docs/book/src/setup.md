# Setup

The introduction's three commands are the whole setup on macOS and
Windows. This page is the rest: the dependency in detail, your project,
Linux, and the devtools.

## The dependency

kui's crates publish to crates.io, so the dependency is one line:

```toml
[dependencies]
kui-native = "0.1.0-alpha.47"
```

`cargo add kui-native` writes that line for you. Every version is an
alpha for now, and a version requirement does not hold an alpha still:
`"0.1.0-alpha.33"` admits every later `0.1.0` alpha, so it is a floor,
and `Cargo.lock` is what keeps the version you tested. Write
`"=0.1.0-alpha.33"` to pin it in the manifest.

`kui-native` is the batteries-included crate: a window, a GPU renderer,
the stock widgets and the `App` trait. Everything the book uses is
reachable from it, and its API reference is on
[docs.rs](https://docs.rs/kui-native). This book is the path in; docs.rs
is the map once you know the names.

Versions before 0.1.0-alpha.33 are on kui's own Forgejo registry only. A
project that still names that registry (`registry = "drydock9"` on the
dependency) should drop the key: from 0.1.0-alpha.34 on, a crate taken
from there resolves its `kui-*` siblings from crates.io, so naming two
`kui-*` crates from it puts two `kui_core`s in one build.

## Your project

The book's programs are the files under
[`examples/rust/tutorial`](https://github.com/qxuken/kui/tree/main/examples/rust/tutorial)
in the kui repository, and each is complete on the page: a chapter's code
block shows the lines it is about, and the eye icon in its corner reveals
the rest of the file. Copy the whole program into `src/main.rs` of a
project set up as above, and run it:

```bash
cargo run
```

Every chapter's *Run it* line is spelled that way. From a checkout of
the repository the same program is `cargo run -p kui-native --example
tutorial_01_hello`, with each file's own number and name. The testing
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

The published copy is at <https://kui-book.qxuken.dev>. It is built
with [mdBook](https://rust-lang.github.io/mdBook/) from `docs/book` in a
checkout of [the repository](https://github.com/qxuken/kui):

```bash
cargo install mdbook
mdbook serve docs/book --open
```

`serve` rebuilds on every save and reloads the browser.
