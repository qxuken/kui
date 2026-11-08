#!/usr/bin/env nu
# Build the book and stage its live examples in the same static site.

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

    let lua_tools = ($tools | path join "book-lua")
    mkdir $lua_tools
    cp examples/book-web/package.json examples/book-web/package-lock.json $lua_tools
    ^npm ci --ignore-scripts --no-audit --prefix $lua_tools
    let wasi = ($lua_tools | path join "node_modules" "@bjorn3" "browser_wasi_shim")
    let live = ($out | path join "live")
    let shim = ($live | path join "wasi")
    mkdir $shim
    for module in (glob $"($wasi)/dist/*.js") {
        cp $module $shim
    }
    cp ($wasi | path join "LICENSE-APACHE") ($wasi | path join "LICENSE-MIT") ($live | path join "wasi")
    cp examples/book-web/gallery-lua.js $live

    # wasi-sdk supplies the C headers and setjmp implementation Lua uses.
    if ($env.WASI_SDK_PATH? | is-not-empty) {
        let sdk = ($env.WASI_SDK_PATH | path expand)
        let sysroot = ($sdk | path join "share" "wasi-sysroot")
        let libraries = ($sysroot | path join "lib" "wasm32-wasip1")
        with-env {
            CC_wasm32_wasip1: ($sdk | path join "bin" "clang")
            AR_wasm32_wasip1: ($sdk | path join "bin" "llvm-ar")
            CFLAGS_wasm32_wasip1: $"--sysroot=($sysroot)"
            CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS: $"-Lnative=($libraries)"
        } {
            ^cargo build --locked --release -p kui-book-web --lib --target wasm32-wasip1
        }
    } else {
        ^cargo build --locked --release -p kui-book-web --lib --target wasm32-wasip1
    }
    cp ($target | path join "wasm32-wasip1" "release" "kui_book_web.wasm") ($live | path join "gallery-lua.wasm")
}
