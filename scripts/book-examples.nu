#!/usr/bin/env nu
# Writes the book's Examples part: a page per Rust example under
# examples/rust/{apps,widgets,features,tools}, a page per kind listing them,
# an overview, and the part's entries at the end of docs/book/src/SUMMARY.md
# and the trailing playground link (everything from its `# Examples` line
# down is this script's).
#
# A page holds no source of its own - it `{{#include}}`s the example's file,
# so the book shows what the file says the day it is built. What can drift is
# the list: an example added, renamed or removed without a rerun. The book is
# published on its own (.github/workflows/book.yml), so these pages are
# the one place a reader of the site sees an example without opening the
# repository.
#
# Runnable Rust examples also point to the Wasm gallery. Its registry
# loads the same source files that the page includes.
#
#   nu scripts/book-examples.nu           rewrite the pages and the SUMMARY part
#   nu scripts/book-examples.nu --check   fail if the generated pages are stale

# One line each, from the kinds table in examples/README.md.
const KINDS = {
    apps: "How it composes: an app owning its state, keymap or pane tree, touching whatever it needs."
    widgets: "One element or one stock widget, in every state it has."
    features: "One cross-cutting behaviour, with exactly the widgets it touches."
    tools: "Registered as an example for want of a better slot, and not one: a corpus dump."
}

# The examples of one kind, by name, sorted bytewise.
def examples-of [kind: string] {
    ls $"examples/rust/($kind)"
        | where type == file and ($it.name | str ends-with ".rs")
        | get name
        | each {|f| $f | path basename | str replace -r '\.rs$' '' }
        | sort
}

# Lines joined with "\n" and ended by one, the way `echo` wrote them.
def text [lines: list<string>] { ($lines | str join "\n") + "\n" }

def live-example [kind: string, name: string] {
    let id = $"($kind)/($name)"
    if $kind == "tools" { return [] }
    if $id in (open --raw examples/book-web/desktop-only.txt | lines) {
        return [
            "This example uses a separate, non-activating desktop window."
            "For an in-frame menu, try the [live select example](../widgets/select.md)."
            ""
        ]
    }
    [
        $'<a data-kui-example="480" href="../../live/gallery.html?example=($id)">Live ($name) example</a>'
        ""
    ]
}

def generate [src: string] {
    let out = ($src | path join examples)
    rm -rf --permanent $out
    mkdir $out
    text ([
        "# Examples"
        ""
        "Every Rust example in the kui repository, one page each, with its whole"
        "source. An example has one subject and shows it in every state it has."
        "Each runs inside the repository's devtools harness (`kui-devtools`,"
        "which is not published): in your own project, take the view and the"
        "`on_event` and launch them with `kui_native::app` as the book does."
        "The comment at the top of each file says what it shows and how the"
        "repository runs it."
        ""
        "Live examples compile the Rust source shown below them to WebAssembly."
        "Use Reload example to start a fresh instance. The command-line"
        "tools below write text and have no canvas. The browser hands the"
        "example file names instead of desktop paths, and downloads recordings"
        "as MP4 or WebM. Fragment, line and backdrop blur need WebGPU."
        ""
    ] | append ($KINDS | transpose kind blurb | each {|k| $"- [`($k.kind)/`]\(($k.kind).md\): ($k.blurb)" }))
    | save -f --raw ($out | path join index.md)
    for k in ($KINDS | transpose kind blurb) {
        let names = (examples-of $k.kind)
        mkdir ($out | path join $k.kind)
        text ([$"# `($k.kind)/`" "" $k.blurb ""] | append ($names | each {|n| $"- [`($n)`]\(($k.kind)/($n).md\)" }))
        | save -f --raw ($out | path join $"($k.kind).md")
        for n in $names {
            let stem = $"examples/rust/($k.kind)/($n)"
            let modules = if ($stem | path type) == "dir" {
                glob $"($stem)/*.rs" | sort | each {|f| $"($stem)/($f | path basename)" }
            } else {
                []
            }
            let source = ([$"($stem).rs"] | append $modules | each {|file|
                ["```rust,noplayground" $"{{#include ../../../../../($file)}}" "```" ""]
            } | flatten | drop 1)
            let preview = (live-example $k.kind $n)
            text ([
                $"# `($k.kind)/($n).rs`"
                ""
                "<!-- Written by scripts/book-examples.nu; the source is included when the book is built. -->"
                ""
            ] | append $preview | append $source)
            | save -f --raw ($out | path join $k.kind $"($n).md")
        }
    }
    # Everything above the part stays, its trailing blank lines dropped so
    # a rerun is byte-identical.
    let summary = ($src | path join SUMMARY.md)
    let head = (open --raw $summary
        | lines
        | take until {|l| $l == "# Examples" }
        | str join "\n"
        | str replace -r '\n+$' '')
    let part = (["" "# Examples" "" "- [Overview](examples/index.md)"]
        | append ($KINDS | columns | each {|kind|
            [$"- [($kind)]\(examples/($kind).md\)"]
            | append (examples-of $kind | each {|n| $"  - [($n)]\(examples/($kind)/($n).md\)" })
        } | flatten))
    text ([$head] | append $part | append ["" "# Playground" "" "- [Lua playground](playground.md)"])
        | save -f --raw $summary
}

def main [
    --check     # fail, with the diff, if a rerun would change anything
] {
    cd ($env.FILE_PWD | path dirname)
    if not $check {
        generate docs/book/src
        return
    }
    let tmp = (mktemp -d)
    let copy = ($tmp | path join src)
    cp -r docs/book/src $copy
    generate $copy
    # git's diff rather than diff(1): it is on every runner, Windows' too.
    let r = (^git diff --no-index --exit-code docs/book/src $copy | complete)
    rm -rf --permanent $tmp
    if $r.exit_code != 0 {
        print $r.stdout
        print -e "the book's Examples part is stale: run nu scripts/book-examples.nu"
        exit 1
    }
}
