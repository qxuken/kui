#!/usr/bin/env nu
# Build the book and stage its live examples in the same static site.

use wasi-build.nu
use web-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--out-dir: path = "target/book", --install-tools] {
    cd $ROOT
    let tools = (web-build tools --install=$install_tools)
    let out = ($out_dir | path expand)
    let target = (web-build target-dir)

    ^mdbook build docs/book -d $out
    ^nu scripts/web-counter.nu --build-only --book --out-dir ($out | path join "live")

    let live = ($out | path join "live")
    let shim = ($live | path join "wasi")
    wasi-build stage-shim $shim
    cp examples/book-web/gallery-lua.js $live
    wasi-build build kui-book-web
    let wasi = (web-build directory wasm32-wasip1 | path join kui_book_web.wasm)
    web-build optimize $tools.wasm_opt $wasi ($live | path join gallery-lua.wasm)

    let runner = ($target | path join "web-tools" "book-runner.zip" | path expand)
    ^nu scripts/web-runner.nu --out $runner --shim $shim
    let tasks = ($live | path join "tasks")
    ^cargo run --locked --quiet -p kui-cli --bin kui -- pack examples/lua/tutorial/tasks --format web --runner $runner --out $tasks
    cp docs/book/book-tasks.js ($tasks | path join "kui")
    let index = ($tasks | path join "index.html")
    open --raw $index | str replace "./kui/web-app.js" "./kui/book-tasks.js" | save --force --raw $index
}
