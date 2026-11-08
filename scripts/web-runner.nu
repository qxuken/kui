#!/usr/bin/env nu
# Assemble the versioned runtime that `kui pack --format web` consumes.
use wasi-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--out: path, --shim: path, --install-tools] {
    cd $ROOT
    let target = ($env.CARGO_TARGET_DIR? | default "target")
    let version = (open Cargo.toml | get workspace.package.version)
    let tools = ($target | path join "web-tools" | path expand)
    let archive = ($out | default ($target | path join $"kui-($version)-web.zip") | path expand)
    let stage = (mktemp -d)
    let runtime = ($stage | path join "kui")
    mkdir $runtime
    try {
        if $install_tools {
            let bindgen = (open --raw Cargo.lock | from toml | get package
                | where name == "wasm-bindgen" | first | get version)
            ^rustup target add wasm32-wasip1 wasm32-unknown-unknown
            ^cargo install --locked --root $tools --version $bindgen wasm-bindgen-cli
        }
        wasi-build build kui-web-lua
        cp ($target | path join "wasm32-wasip1" "release" "kui_web_lua.wasm") ($runtime | path join "runner.wasm")
        ^cargo build --locked --release -p kui-web --example packed_renderer --target wasm32-unknown-unknown --features rust-wgpu,packed-gpu
        let gpu = ($target | path join "wasm32-unknown-unknown" "release" "examples" "packed_renderer.wasm")
        with-env {PATH: ($env.PATH | prepend ($tools | path join "bin"))} {
            ^wasm-bindgen --target web --no-typescript --out-dir $runtime --out-name runner-gpu $gpu
        }
        cp crates/kui-web-lua/web/index.html $stage
        cp crates/kui-web-lua/web/web-app.js crates/kui-web-lua/web/filesystem.js crates/kui-web-lua/web/packed-gpu.js $runtime
        cp crates/kui-web/canvas.js crates/kui-web/editor.js examples/web/mount.js $runtime
        cp LICENSE ($runtime | path join "LICENSE-kui.txt")
        cp crates/kui-cli/assets/fonts/LICENSE ($runtime | path join "OFL-NotoSans.txt")
        cp examples/web/fonts/OFL-NotoSansSymbols2.txt $runtime
        if $shim == null {
            wasi-build stage-shim ($runtime | path join "wasi")
        } else {
            cp -r $shim ($runtime | path join "wasi")
        }
        $version | save ($stage | path join "runner-version.txt")
        do {
            cd $stage
            ^zip -q -r runner.zip index.html runner-version.txt kui
        }
        mkdir ($archive | path dirname)
        mv -f ($stage | path join "runner.zip") $archive
    } catch {|err|
        rm -r $stage
        error make {msg: $err.msg}
    }
    rm -r $stage
    print $archive
}
