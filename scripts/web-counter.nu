#!/usr/bin/env nu
# Build and serve the browser examples with wasm-bindgen.
use web-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [--port: int = 8080, --build-only, --out-dir: path, --book, --svg, --devtools, --install-tools] {
    cd $ROOT
    let tools = (web-build tools --install=$install_tools)
    let extras = [
        (if $svg { "svg" } else { null })
        (if $devtools { "devtools" } else { null })
    ] | compact
    let target_dir = (web-build target-dir)
    let out = ($out_dir | default ($target_dir | path join "web-counter"))
    mkdir $out
    let package = if $book { "kui-book-web" } else { "kui-web" }
    let selection = if $book { "--lib" } else { "--examples" }
    let app_names = if $book { [gallery] } else { [counter notes] }
    if $book {
        cp examples/book-web/gallery.html $out
        cp examples/book-web/gallery-effects.js $out
        cp examples/book-web/gallery-audio.js $out
        cp examples/book-web/gallery-popup.js $out
        cp examples/book-web/OFL-KUIBookEmoji.txt $out
    } else {
        cp examples/web/index.html $out
        cp examples/web/notes.html $out
    }
    cp examples/web/mount.js $out
    cp crates/kui-web/editor.js $out
    cp crates/kui-web/canvas.js $out
    cp crates/kui-cli/assets/fonts/LICENSE ($out | path join "OFL-NotoSans.txt")
    cp examples/web/fonts/OFL-NotoSansSymbols2.txt $out
    let profile_dir = (web-build directory wasm32-unknown-unknown)
    let artifact_dir = if $book { $profile_dir } else { $profile_dir | path join "examples" }
    web-build build $package wasm32-unknown-unknown [$selection --features ($extras | str join ",")]
    for app in $app_names {
        let artifact = if $book { "kui_book_web.wasm" } else { $"($app).wasm" }
        web-build optimize $tools.wasm_opt ($artifact_dir | path join $artifact) ($out | path join $"($app).wasm")
    }
    web-build build $package wasm32-unknown-unknown [$selection --features ($extras | append rust-wgpu | str join ",")]
    for app in $app_names {
        let artifact = if $book { "kui_book_web.wasm" } else { $"($app).wasm" }
        web-build bindgen $tools.bindgen $tools.wasm_opt ($artifact_dir | path join $artifact) $out $"($app)-wgpu"
    }
    if $build_only {
        print $"Built ($out)"
        return
    }
    print $"Open http://127.0.0.1:($port)/notes.html for the notes workspace or / for the counter"
    ^python3 -m http.server $port --bind 127.0.0.1 --directory $out
}
