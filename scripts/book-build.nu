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
        ^rustup target add wasm32-unknown-unknown
        ^cargo install --locked --root $tools --version $version wasm-bindgen-cli
    }

    ^mdbook build docs/book -d $out
    with-env {PATH: ($env.PATH | prepend ($tools | path join "bin"))} {
        ^nu scripts/web-counter.nu --build-only --book --out-dir ($out | path join "live")
    }
}
