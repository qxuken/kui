#!/usr/bin/env nu
# Assemble the versioned runtime that `kui pack --format web` consumes.
use wasi-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--out: path, --shim: path] {
    cd $ROOT
    let target = ($env.CARGO_TARGET_DIR? | default "target")
    let version = (open Cargo.toml | get workspace.package.version)
    let archive = ($out | default ($target | path join $"kui-($version)-web.zip") | path expand)
    let stage = (mktemp -d)
    let runtime = ($stage | path join "kui")
    mkdir $runtime
    try {
        wasi-build build kui-web-lua
        cp ($target | path join "wasm32-wasip1" "release" "kui_web_lua.wasm") ($runtime | path join "runner.wasm")
        cp crates/kui-web-lua/web/index.html $stage
        cp crates/kui-web-lua/web/web-app.js crates/kui-web-lua/web/filesystem.js $runtime
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
