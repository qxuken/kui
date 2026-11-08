#!/usr/bin/env nu
# Build and serve the browser examples with wasm-bindgen.

const ROOT = (path self | path dirname | path dirname)

def main [--port: int = 8080, --build-only, --out-dir: path, --book, --svg, --devtools] {
    cd $ROOT
    let extras = [
        (if $svg { "svg" } else { null })
        (if $devtools { "devtools" } else { null })
    ] | compact
    let target_dir = ($env.CARGO_TARGET_DIR? | default "target")
    let out = ($out_dir | default ($target_dir | path join "web-counter"))
    mkdir $out
    let package = if $book { "kui-book-web" } else { "kui-web" }
    let selection = if $book { "--lib" } else { "--examples" }
    let app_names = if $book { [gallery] } else { [counter notes] }
    if $book {
        cp examples/book-web/gallery.html $out
        cp examples/book-web/gallery-effects.js $out
        cp examples/book-web/gallery-audio.js $out
    } else {
        cp examples/web/index.html $out
        cp examples/web/notes.html $out
    }
    cp examples/web/mount.js $out
    cp crates/kui-web/editor.js $out
    cp crates/kui-web/canvas.js $out
    cp crates/kui-cli/assets/fonts/LICENSE ($out | path join "OFL-NotoSans.txt")
    cp examples/web/fonts/OFL-NotoSansSymbols2.txt $out
    let release = ($target_dir | path join "wasm32-unknown-unknown" "release")
    let artifact_dir = if $book { $release } else { $release | path join "examples" }
    ^cargo build --locked --release -p $package $selection --target wasm32-unknown-unknown --features ($extras | str join ",")
    for app in $app_names {
        let artifact = if $book { "kui_book_web.wasm" } else { $"($app).wasm" }
        cp ($artifact_dir | path join $artifact) ($out | path join $"($app).wasm")
    }
    ^cargo build --locked --release -p $package $selection --target wasm32-unknown-unknown --features ($extras | append rust-wgpu | str join ",")
    for app in $app_names {
        let artifact = if $book { "kui_book_web.wasm" } else { $"($app).wasm" }
        ^wasm-bindgen --target web --no-typescript --out-dir $out --out-name $"($app)-wgpu" ($artifact_dir | path join $artifact)
    }
    if $build_only {
        print $"Built ($out)"
        return
    }
    print $"Open http://127.0.0.1:($port)/notes.html for the notes workspace or / for the counter"
    ^python3 -m http.server $port --bind 127.0.0.1 --directory $out
}
