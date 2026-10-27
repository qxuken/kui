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
#   addons every add-on under addons/: its typed wrappers and its component
#          reference (bindings/reference.md) from schema.lua
#          (kui-addon-schema, ADRs 0066 and 0071), then its Teal built to Lua
#          (build.lua, which needs a Lua 5.4+ on PATH)
#   node   npm run gen in packages/kui: jsx-runtime.d.ts, index.d.ts and
#          docs/props.md, from the schema and the addon's #[napi] surface
#   docs   crates/kui-native/docs: props.md and howto.md as the crate's
#          rustdoc pages read them (kui_native::docs, ADR 0071), and an
#          index of the ADRs linking to them and to the book
#   odin   nu scripts/odin.nu gen: packages/odin from kui.h and the schema
#          (needs odin, or ODIN, and clang)
#   book   nu scripts/book-examples.nu: the book's Examples part
#
# Every step runs even after one fails, so one run says all there is.
# release-prep.nu runs this, in write mode on a clean tree, before a tag.

const ROOT = path self | path dirname | path dirname
const STEPS = [lua addons node docs odin book]

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

const GITHUB = "https://github.com/qxuken/kui/blob/main"
const BOOK = "https://kui-book.qxuken.dev"

# A Markdown file from docs/ as rustdoc reads it in the kui-native crate:
# a bare fence is text and a `rust` one is not run (neither is a doctest),
# a link to props.md or howto.md is the crate's own page, and a link to
# the rest of the repository goes to it on GitHub.
def rustdoc-page [text: string]: nothing -> string {
    mut fence = false
    mut out = []
    for line in ($text | lines) {
        let t = $line | str trim --left
        mut l = $line
        if ($t | str starts-with "```") {
            if not $fence {
                let lang = $t | str substring 3.. | str trim
                if $lang == "" {
                    $l = $line | str replace "```" "```text"
                } else if $lang == "rust" {
                    $l = $line | str replace "```rust" "```rust,ignore"
                }
            }
            $fence = not $fence
        }
        $out = ($out | append $l)
    }
    $out
    | str join "\n"
    | str replace --all "](props.md" "](crate::docs::props"
    | str replace --all "](howto.md" "](crate::docs::howto"
    | str replace --all "](book/src/SUMMARY.md)" $"]\(($BOOK))"
    | str replace --all "](adr/" $"]\(($GITHUB)/docs/adr/"
    | str replace --all "](../" $"]\(($GITHUB)/"
    | $in + "\n"
}

# What crates/kui-native/docs holds: the two pages and the ADR index.
def crate-docs []: nothing -> list<record<path: string, text: string>> {
    let adrs = ls docs/adr/*.md | get name | sort | each {|f|
        let title = open --raw $f | lines | where {|l| $l | str starts-with "# " } | first | str substring 2..
        # A URL's path: `ls` names it `docs\adr\…` on Windows, which wrote
        # every link with backslashes there (the alpha.69 release-prep).
        let url = $f | str replace --all '\' '/'
        $"- [($f | path basename | str substring 0..3) ($title)]\(($GITHUB)/($url))"
    }
    let index = [
        "# kui's design records"
        ""
        "Every decision that shaped kui is written down as an architecture"
        "decision record — the context, what was decided and why, the options"
        "turned down — and the numbers below are the ones the rest of these docs"
        "cite (\"ADR 0047\"). They are read on GitHub, at the head of `main`; [the"
        $"kui book]\(($BOOK)) is the path in, one concept a chapter, and"
        "[`props`](crate::docs::props) and [`howto`](crate::docs::howto) are the"
        "reference and the task index this crate carries (ADR 0071)."
        ""
        ...$adrs
    ]
    [
        {path: "crates/kui-native/docs/props.md", text: (rustdoc-page (open --raw docs/props.md))}
        {path: "crates/kui-native/docs/howto.md", text: (rustdoc-page (open --raw docs/howto.md))}
        {path: "crates/kui-native/docs/adr.md", text: ($index | str join "\n" | $in + "\n")}
    ]
}

def main [
    --check  # fail, writing nothing, if a rerun would change a generated file
    --skip: string = ""  # steps to leave out, comma-separated: lua,addons,node,docs,odin,book
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
        [addons "the add-ons' wrappers and Lua (addons/*), and Kit in the npm package" {
            for dir in (ls addons | where type == dir | get name) {
                if ($"($dir)/schema.lua" | path exists) {
                    must cargo run -q -p kui-addon-schema -- gen $"($dir)/schema.lua" $"($dir)/bindings" ...$check_flag
                }
                if ($"($dir)/build.lua" | path exists) {
                    must lua $"($dir)/build.lua" ...$check_flag
                }
            }
            # @qxuken/kui/kit ships Kit: its wrappers and the Lua the
            # extension loads, copied from addons/kit (packages/kui/kit/index.js
            # is hand-written beside them).
            let copies = (
                [[from to]; [addons/kit/bindings/node/kit.js packages/kui/kit/components.js]
                    [addons/kit/bindings/node/kit.d.ts packages/kui/kit/components.d.ts]
                    [addons/kit/bindings/reference.md packages/kui/kit/reference.md]]
                | append (ls addons/kit/lua/*.lua | get name | each {|f| {from: $f, to: $"packages/kui/kit/lua/($f | path basename)"} })
            )
            let stale = $copies | where {|c| not ($c.to | path exists) or (open --raw $c.from) != (open --raw $c.to) }
            if $check {
                if ($stale | is-not-empty) {
                    error make {msg: $"stale: ($stale | get to | str join ', ') — run nu scripts/gen.nu"}
                }
            } else {
                mkdir packages/kui/kit/lua
                for c in $stale { cp $c.from $c.to }
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
        [docs "the kui-native crate's rustdoc pages (crates/kui-native/docs)" {
            # After `node`, which writes docs/props.md.
            let want = crate-docs
            let stale = $want | where {|f| not ($f.path | path exists) or (open --raw $f.path) != $f.text }
            if $check {
                if ($stale | is-not-empty) {
                    error make {msg: $"stale: ($stale | get path | str join ', ') — run nu scripts/gen.nu"}
                }
            } else {
                mkdir crates/kui-native/docs
                for f in $stale { $f.text | save -f --raw $f.path }
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
