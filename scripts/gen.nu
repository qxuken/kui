#!/usr/bin/env nu
# Runs every generator in the repository. Each file one of them writes is
# committed, and each has a check that fails while it is stale (a CI step
# or a test), so a change to the schema, the prelude, kui.h, the addon's
# `#[napi]` surface or the examples is followed by this, rather than by
# knowing which of the generators it reaches.
#
#   nu scripts/gen.nu                  write every generated file
#   nu scripts/gen.nu --check          fail, writing nothing, if a rerun would change one
#   nu scripts/gen.nu --skip odin      leave steps out, comma-separated
#
# The steps, in order:
#
#   lua    packages/lua/kui.lua (lua-language-server) and kui.d.tl (Teal),
#          from the schema and the prelude (the `lua-types` example)
#   addons every add-on under addons/: its typed wrappers from schema.lua
#          (kui-addon-schema, ADR 0066), then its Teal built to Lua
#          (build.lua, which needs a Lua 5.4+ on PATH)
#   node   npm run gen in packages/kui: jsx-runtime.d.ts, index.d.ts and
#          docs/props.md, from the schema and the addon's #[napi] surface
#   odin   nu scripts/odin.nu gen: packages/odin from kui.h and the schema
#          (needs odin, or ODIN, and clang)
#   book   nu scripts/book-examples.nu: the book's Examples part
#
# Every step runs even after one fails, so one run says all there is.
# release-prep.nu runs this, in write mode on a clean tree, before a tag.

const ROOT = path self | path dirname | path dirname
const STEPS = [lua addons node odin book]

def --wrapped must [cmd: string, ...args] {
    ^$cmd ...$args
    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: $"failed \(exit ($env.LAST_EXIT_CODE)\): ($cmd) ($args | str join ' ')"}
    }
}

# Runs `write`, which rewrites `files`; with `check`, puts every file back
# as it was and fails naming those the rewrite changed.
def written [files: list<string>, check: bool, write: closure] {
    let before = $files | each {|f| { path: $f, text: (if ($f | path exists) { open --raw $f } else { "" }) } }
    do $write
    if not $check { return }
    let after = $before | each {|b| $b | insert now (open --raw $b.path) }
    $before | each {|b| $b.text | save -f --raw $b.path } | ignore
    let stale = $after | where {|b| $b.now != $b.text } | get path
    if ($stale | is-not-empty) {
        error make {msg: $"stale: ($stale | str join ', ')"}
    }
}

def main [
    --check  # fail, writing nothing, if a rerun would change a generated file
    --skip: string = ""  # steps to leave out, comma-separated: lua,node,odin,book
] {
    cd $ROOT
    let skip = $skip | split row "," | str trim | where {|s| $s != "" }
    let unknown = $skip | where {|s| $s not-in $STEPS }
    if ($unknown | is-not-empty) {
        error make {msg: $"unknown step ($unknown | str join ', '); one of ($STEPS | str join ', ')"}
    }
    let check_flag = if $check { [--check] } else { [] }

    let steps = [
        [name what run];
        [lua "the Lua type files (packages/lua)" {
            written [packages/lua/kui.lua packages/lua/kui.d.tl] $check {
                must cargo run -q -p kui-lua --example lua-types -- packages/lua
            }
        }]
        [addons "the add-ons' wrappers and Lua (addons/*)" {
            for dir in (ls addons | where type == dir | get name) {
                if ($"($dir)/schema.lua" | path exists) {
                    must cargo run -q -p kui-addon-schema -- gen $"($dir)/schema.lua" $"($dir)/bindings" ...$check_flag
                }
                if ($"($dir)/build.lua" | path exists) {
                    must lua $"($dir)/build.lua" ...$check_flag
                }
            }
        }]
        [node "npm run gen (jsx-runtime.d.ts, index.d.ts, docs/props.md)" {
            written [packages/kui/jsx-runtime.d.ts packages/kui/index.d.ts docs/props.md] $check {
                # A napi-type-defs file left from an earlier build gets
                # appended to, not rewritten, and index.d.ts then has every
                # addon member twice; without one, gen-types.mjs forces the
                # build that writes it.
                rm -rf target/napi-type-defs
                cd packages/kui
                must npm run gen
            }
        }]
        [odin "the Odin binding (packages/odin)" {
            let odin = $env.ODIN? | default "odin"
            if (which $odin | is-empty) and not ($odin | path expand | path exists) {
                error make {msg: "no odin on PATH: set ODIN to the compiler, or pass --skip odin"}
            }
            must $nu.current-exe scripts/odin.nu gen ...$check_flag
        }]
        [book "the book's Examples part" {
            must $nu.current-exe scripts/book-examples.nu ...$check_flag
        }]
    ]
    mut failed = []
    for s in ($steps | where {|s| $s.name not-in $skip }) {
        print $"\n=== ($s.what)"
        let ok = try { do $s.run; true } catch {|e| print -e $"($s.name) failed: ($e.msg)"; false }
        if not $ok { $failed = ($failed | append $s.name) }
    }
    let skipped = if ($skip | is-empty) { "" } else { $" \(skipped: ($skip | str join ', '))" }
    if ($failed | is-not-empty) {
        print -e $"\nfailed: ($failed | str join ', ')($skipped)"
        exit 1
    }
    let done = if $check { "every generated file is current" } else { "every generator ran" }
    print $"\n($done)($skipped)"
}
