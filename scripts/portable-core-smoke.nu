#!/usr/bin/env nu
# Requires the pinned Rust toolchain, its wasm32-unknown-unknown target, and Node.

const ROOT = (path self | path dirname | path dirname)

def main [] {
    cd $ROOT
    let target = "wasm32-unknown-unknown"
    let work = "target/portable-core-smoke"
    let native = if $nu.os-info.name == "windows" { $"($work)/smoke.exe" } else { $"($work)/smoke" }
    let wasm = $"($work)/smoke.wasm"
    mkdir $work

    ^cargo build --locked -p kui-core --lib
    ^rustc --edition=2024 -L dependency=target/debug/deps --extern kui_core=target/debug/libkui_core.rlib scripts/portable-core-smoke.rs -o $native
    let expected = (^$native | str trim)

    ^cargo build --locked -p kui-core --lib --target $target
    ^rustc --edition=2024 --target $target --crate-type cdylib -L dependency=target/debug/deps -L $"dependency=target/($target)/debug/deps" --extern $"kui_core=target/($target)/debug/libkui_core.rlib" scripts/portable-core-smoke.rs -o $wasm
    let actual = (^node -e 'const fs = require("node:fs"); const mod = new WebAssembly.Module(fs.readFileSync(process.argv[1])); console.log(new WebAssembly.Instance(mod).exports.portable_core_smoke());' $wasm | str trim)

    if $expected != $actual {
        error make {msg: $"Native output ($expected) differs from Wasm output ($actual)"}
    }
    print $"Portable core produced ($actual) on both targets"
}
