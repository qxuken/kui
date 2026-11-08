#!/usr/bin/env nu
# Build the book and stage its live examples in the same static site.

use wasi-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--out-dir: path = "target/book", --install-tools] {
    cd $ROOT
    let out = ($out_dir | path expand)
    let target = ($env.CARGO_TARGET_DIR? | default "target")
    let tools = ($target | path join "web-tools" | path expand)
    if $install_tools {
        let version = (open --raw Cargo.lock | from toml | get package
            | where name == "wasm-bindgen" | first | get version)
        ^rustup target add wasm32-unknown-unknown wasm32-wasip1
        ^cargo install --locked --root $tools --version $version wasm-bindgen-cli
    }

    ^mdbook build docs/book -d $out
    with-env {PATH: ($env.PATH | prepend ($tools | path join "bin"))} {
        ^nu scripts/web-counter.nu --build-only --book --out-dir ($out | path join "live")
    }

    let live = ($out | path join "live")
    let shim = ($live | path join "wasi")
    wasi-build stage-shim $shim
    cp examples/book-web/gallery-lua.js $live
    wasi-build build kui-book-web
    cp ($target | path join "wasm32-wasip1" "release" "kui_book_web.wasm") ($live | path join "gallery-lua.wasm")

    let runner = ($target | path join "web-tools" "book-runner.zip" | path expand)
    ^nu scripts/web-runner.nu --out $runner --shim $shim
    let tasks = ($live | path join "tasks")
    ^cargo run --locked --quiet -p kui-cli --bin kui -- pack examples/lua/tutorial/tasks --format web --runner $runner --out $tasks
    cp docs/book/book-tasks.js ($tasks | path join "kui")
    let index = ($tasks | path join "index.html")
    open --raw $index | str replace "./kui/web-app.js" "./kui/book-tasks.js" | save --force --raw $index
}
