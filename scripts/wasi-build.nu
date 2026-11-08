# Build the vendored Lua interpreter for the browser and stage its WASI imports.
export def build [package: string] {
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
            ^cargo build --locked --release -p $package --lib --target wasm32-wasip1
        }
    } else {
        ^cargo build --locked --release -p $package --lib --target wasm32-wasip1
    }
}

export def stage-shim [out: path] {
    let target = ($env.CARGO_TARGET_DIR? | default "target")
    let tools = ($target | path join "web-tools" "book-lua" | path expand)
    mkdir $tools $out
    cp examples/book-web/package.json examples/book-web/package-lock.json $tools
    ^npm ci --ignore-scripts --no-audit --prefix $tools
    let wasi = ($tools | path join "node_modules" "@bjorn3" "browser_wasi_shim")
    for module in (glob $"($wasi)/dist/*.js") {
        cp $module $out
    }
    cp ($wasi | path join "LICENSE-APACHE") ($wasi | path join "LICENSE-MIT") $out
}
