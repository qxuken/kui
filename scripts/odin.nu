#!/usr/bin/env nu
# The Odin binding's chores: regenerate `kui/c` from kui.h, and build and
# run the Odin examples against this checkout's libkui_ffi.
#
#   nu scripts/odin.nu gen                 kui.h + the schema -> packages/odin/kui/c and kui/generated.odin
#   nu scripts/odin.nu gen --check         fail if a rerun would change them
#   nu scripts/odin.nu check               gen, then type-check the packages and every example
#   nu scripts/odin.nu run counter         build and open examples/odin/apps/counter
#   nu scripts/odin.nu run counter --headless
#   nu scripts/odin.nu test                every example's --headless self-check
#
# `odin` is found on PATH, or set ODIN to the compiler. `clang` dumps the
# header (the AST and the layout program both come from it), cargo dumps
# the prop schema (examples/rust/tools/schema-dump.rs) and builds libkui_ffi,
# debug unless --release.
#
# The examples link kui_ffi as a system library from target/<profile> and
# carry an rpath to it, so a built example runs from anywhere in this
# checkout without DYLD_LIBRARY_PATH.

const ROOT = path self | path dirname | path dirname
const PKG = $ROOT | path join packages odin
const OUT = $ROOT | path join target odin

# The compiler: ODIN when set, a path (`~` and relative ones expanded) or a
# command name; else `odin` from PATH.
def odin [] {
    let o = $env.ODIN? | default "odin"
    if ($o | str starts-with "~") or ($o | str contains "/") { $o | path expand } else { $o }
}

def profile [release: bool] {
    if $release { "release" } else { "debug" }
}

def build-lib [release: bool] {
    let args = if $release { [--release] } else { [] }
    cargo build -p kui-ffi ...$args
}

def link-flags [release: bool] {
    let dir = $ROOT | path join target (profile $release)
    if $nu.os-info.name == "windows" {
        $"-extra-linker-flags:/LIBPATH:($dir)"
    } else {
        $"-extra-linker-flags:-L($dir) -Wl,-rpath,($dir)"
    }
}

# Every example: examples/odin/<kind>/<name>.odin, one file a program.
def examples [] {
    glob ($ROOT | path join examples odin "*" "*.odin") | sort
}

def example [name: string] {
    let hit = examples | where {|p| ($p | path parse | get stem) == $name }
    if ($hit | is-empty) {
        error make { msg: $"no examples/odin/*/($name).odin; there is: (examples | each { path parse | get stem } | str join ', ')" }
    }
    $hit | first
}

# The files gen writes.
def generated [] {
    let kui_dir = $PKG | path join kui
    [($kui_dir | path join generated.odin) ($kui_dir | path join c kui_c.odin) ($kui_dir | path join c layout.odin)]
}

# Regenerate packages/odin/kui/c and packages/odin/kui/generated.odin from
# crates/kui-ffi/include/kui.h and kui_core::schema. With --check, fail
# (and leave the files as they were) when a rerun would change them.
def "main gen" [--check] {
    let before = generated | each {|f| { path: $f, text: (open --raw $f) } }
    mkdir $OUT
    let header = $ROOT | path join crates kui-ffi include kui.h
    let ast = $OUT | path join kui.ast.json
    let schema = $OUT | path join schema.json
    let layout_c = $OUT | path join layout.c
    let layout_exe = $OUT | path join layout
    let kui_dir = $PKG | path join kui
    clang -x c -Xclang -ast-dump=json -fsyntax-only $header | save -f $ast
    cargo run -q -p kui-core --example schema-dump -- $schema
    ^(odin) run ($PKG | path join gen) $"-out:($OUT | path join gen)" -- $ast $header $schema $kui_dir $layout_c
    clang -std=c11 $"-I($header | path dirname)" $layout_c -o $layout_exe
    ^$layout_exe | save -f ($kui_dir | path join c layout.odin)
    if $check {
        let stale = $before | where {|b| (open --raw $b.path) != $b.text } | get path
        $before | each {|b| $b.text | save -f --raw $b.path } | ignore
        if not ($stale | is-empty) {
            print -e $"the Odin binding is stale: run nu scripts/odin.nu gen \(($stale | path relative-to $ROOT | str join ', '))"
            exit 1
        }
        print "the Odin binding is current"
        return
    }
    print $"wrote ($kui_dir)/c and ($kui_dir)/generated.odin"
}

# Regenerate, then type-check both packages and every example.
def "main check" [--release] {
    main gen
    ^(odin) check ($PKG | path join gen) -vet -strict-style
    ^(odin) check ($PKG | path join kui) -no-entry-point -vet -strict-style
    for ex in (examples) {
        print $"check ($ex | path relative-to $ROOT)"
        ^(odin) check $ex -file -vet -strict-style
    }
}

# Build one example into target/odin/<name> and run it; extra arguments go to it.
def --wrapped "main run" [name: string, --release, ...args] {
    build-lib $release
    let exe = build-example (example $name) $release
    ^$exe ...$args
}

def build-example [src: string, release: bool] {
    mkdir $OUT
    let exe = $OUT | path join ($src | path parse | get stem)
    let opt = if $release { [-o:speed] } else { [-debug] }
    ^(odin) build $src -file $"-out:($exe)" (link-flags $release) ...$opt
    $exe
}

# Every example's --headless self-check; non-zero if any failed.
def "main test" [--release] {
    build-lib $release
    let results = examples | each {|src|
        let exe = build-example $src $release
        let r = do { ^$exe --headless } | complete
        print $r.stdout
        if $r.exit_code != 0 { print -e $r.stderr }
        { example: ($src | path parse | get stem), ok: ($r.exit_code == 0) }
    }
    print ($results | table)
    if ($results | any {|r| not $r.ok }) { exit 1 }
}

def main [] {
    print "nu scripts/odin.nu gen | check | run <example> [args] | test"
}
