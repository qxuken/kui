#!/usr/bin/env nu
# Build and serve the browser examples with wasm-bindgen.

const ROOT = (path self | path dirname | path dirname)

def main [--port: int = 8080, --build-only, --svg, --devtools] {
    cd $ROOT
    let extras = [
        (if $svg { "svg" } else { null })
        (if $devtools { "devtools" } else { null })
    ] | compact
    ^cargo build --locked --release -p kui-web --examples --target wasm32-unknown-unknown --features ($extras | str join ",")
    let target_dir = ($env.CARGO_TARGET_DIR? | default "target")
    let out = ($target_dir | path join "web-counter")
    mkdir $out
    cp examples/web/index.html $out
    cp examples/web/notes.html $out
    cp crates/kui-web/editor.js $out
    cp crates/kui-web/canvas.js $out
    cp crates/kui-cli/assets/fonts/LICENSE ($out | path join "OFL-NotoSans.txt")
    cp examples/web/fonts/OFL-NotoSansSymbols2.txt $out
    let examples = ($target_dir | path join "wasm32-unknown-unknown" "release" "examples")
    cp ($examples | path join "counter.wasm") $out
    cp ($examples | path join "notes.wasm") $out
    ^cargo build --locked --release -p kui-web --examples --target wasm32-unknown-unknown --features ($extras | append rust-wgpu | str join ",")
    for app in [counter notes] {
        ^wasm-bindgen --target web --no-typescript --out-dir $out --out-name $"($app)-wgpu" ($examples | path join $"($app).wasm")
    }
    if $build_only {
        print $"Built ($out)"
        return
    }
    print $"Open http://127.0.0.1:($port)/notes.html for the notes workspace or / for the counter"
    ^python3 -m http.server $port --bind 127.0.0.1 --directory $out
}
