# Setup

The introduction's three commands are the whole setup on macOS and
Windows. This page is the rest: the registry in detail, Linux, the
repository, and the devtools.

## The registry

kui's crates publish to a cargo registry named `drydock9`, after the
host that serves it. Reading from it needs no account. Cargo learns about it from a `[registries]` table, in the
project's `.cargo/config.toml` or in `~/.cargo/config.toml` for every
project:

```toml
[registries.drydock9]
index = "sparse+https://drydock9.qxuken.dev/api/packages/qxuken/cargo/"
```

Then the dependency names the registry:

```toml
[dependencies]
kui-native = { version = "0.1.0-alpha.27", registry = "drydock9" }
```

`cargo add kui-native --registry drydock9` writes that line for you.
`kui-native` is the batteries-included crate: a window, a GPU renderer,
the stock widgets and the `App` trait. Everything the book uses is
reachable from it.

## The repository

The book's programs are examples in the kui repository, so a checkout
runs them without a project of your own:

```bash
git clone https://drydock9.qxuken.dev/qxuken/kui
cd kui
cargo run -p kui-native --example tutorial_01_hello
```

Every chapter's *Run it* line is spelled that way. In your own project
the program is `src/main.rs` and the command is `cargo run`.

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
