#!/usr/bin/env nu
# Assemble the versioned runtime that `kui pack --format web` consumes.
use wasi-build.nu
use web-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--out: path, --shim: path, --install-tools] {
    cd $ROOT
    let tools = (web-build tools --install=$install_tools)
    let target = (web-build target-dir)
    let version = (open Cargo.toml | get workspace.package.version)
    let archive = ($out | default ($target | path join $"kui-($version)-web.zip") | path expand)
    let stage = (mktemp -d)
    let runtime = ($stage | path join "kui")
    mkdir $runtime
    try {
        wasi-build build kui-web-lua
        let wasi = (web-build directory wasm32-wasip1 | path join kui_web_lua.wasm)
        web-build optimize $tools.wasm_opt $wasi ($runtime | path join runner.wasm)
        web-build build kui-web wasm32-unknown-unknown [--example packed_renderer --features "rust-wgpu,packed-gpu"]
        let gpu = (web-build directory wasm32-unknown-unknown | path join examples packed_renderer.wasm)
        web-build bindgen $tools.bindgen $tools.wasm_opt $gpu $runtime runner-gpu
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
