#!/usr/bin/env nu
# Writes the book's Examples part: a page per Rust example under
# examples/rust/{apps,widgets,features,tools}, a page per kind listing them,
# an overview, and the part's entries at the end of docs/book/src/SUMMARY.md
# (everything from its `# Examples` line down is this script's).
#
# A page holds no source of its own - it `{{#include}}`s the example's file,
# so the book shows what the file says the day it is built. What can drift is
# the list: an example added, renamed or removed without a rerun. The book is
# published on its own (.forgejo/workflows/book.yml), so these pages are
# the one place a reader of the site sees an example without opening the
# repository.
#
# A page also shows the example as it draws, when a picture of it is in
# docs/book/src/img/examples/<kind>/: its recording (`<name>.gif`, from the
# harness's `--record`) for an example whose subject is motion, else its
# settled frame (`<name>.png`, from `--shot`). The pictures are drawn
# headlessly by the example itself on a GPU (ADR 0054), which CI runners
# have none of, so they are made here and committed; `--check` only reads
# which exist.
#
#   nu scripts/book-examples.nu           rewrite the pages and the SUMMARY part
#   nu scripts/book-examples.nu --check   fail, with the diff, if a rerun would
#                                         change anything (ci.yml's `check` runs it;
#                                         release-prep.nu runs the rewrite)
#   nu scripts/book-examples.nu pictures  draw every picture again from scratch,
#                                         then rewrite the pages (needs a GPU)

# One line each, from the kinds table in examples/README.md.
const KINDS = {
    apps: "How it composes: an app owning its state, keymap or pane tree, touching whatever it needs."
    widgets: "One element or one stock widget, in every state it has."
    features: "One cross-cutting behaviour, with exactly the widgets it touches."
    tools: "Registered as an example for want of a better slot, and not one: a corpus dump."
}

# The kinds whose examples run inside the harness and so can be drawn;
# `tools/` print rather than draw.
const PICTURED = [apps widgets features]

# Where the pictures live, under the book's src.
const IMG = "img/examples"

# The examples of one kind, by name, sorted bytewise.
def examples-of [kind: string] {
    ls $"examples/rust/($kind)"
        | where type == file and ($it.name | str ends-with ".rs")
        | get name
        | each {|f| $f | path basename | str replace -r '\.rs$' '' }
        | sort
}

# The `<img>` for an example's picture under `src`, or nothing without
# one. The width is the picture's in logical px — it is drawn at the
# harness's scale of 2 — so the page shows it at the size the window does.
def picture [src: string, kind: string, name: string] {
    let gif = ($src | path join $IMG $kind $"($name).gif")
    let png = ($src | path join $IMG $kind $"($name).png")
    let found = if ($gif | path exists) {
        # A GIF's logical screen width: two bytes, little-endian, at 6.
        {file: $"($name).gif", px: (open --raw $gif | bytes at 6..<8 | into int --endian little), what: "its recording, played headlessly by `--record`"}
    } else if ($png | path exists) {
        # A PNG's IHDR width: four bytes, big-endian, at 16.
        {file: $"($name).png", px: (open --raw $png | bytes at 16..<20 | into int --endian big), what: "its settled frame, drawn headlessly by `--shot`"}
    } else {
        return []
    }
    [
        $"<img src=\"../../($IMG)/($kind)/($found.file)\" width=\"($found.px // 2)\" alt=\"($kind)/($name): ($found.what | str replace -a '`' '')\">"
        ""
        $"*The example as it draws: ($found.what).*"
        ""
    ]
}

# Lines joined with "\n" and ended by one, the way `echo` wrote them.
def text [lines: list<string>] { ($lines | str join "\n") + "\n" }

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
        "The picture at the top of a page is the example drawing itself: its"
        "harness renders it headlessly on the GPU with the light theme and"
        "grayscale text and keeps the frame (`--shot`), or plays a few seconds"
        "of clicks on a stepped clock and keeps each frame (`--record`), and"
        "`nu scripts/book-examples.nu pictures` draws them all again."
        ""
    ] | append ($KINDS | transpose kind blurb | each {|k| $"- [`($k.kind)/`]\(($k.kind).md\): ($k.blurb)" }))
    | save -f --raw ($out | path join index.md)
    for k in ($KINDS | transpose kind blurb) {
        let names = (examples-of $k.kind)
        mkdir ($out | path join $k.kind)
        text ([$"# `($k.kind)/`" "" $k.blurb ""] | append ($names | each {|n| $"- [`($n)`]\(($k.kind)/($n).md\)" }))
        | save -f --raw ($out | path join $"($k.kind).md")
        for n in $names {
            text ([
                $"# `($k.kind)/($n).rs`"
                ""
                "<!-- Written by scripts/book-examples.nu; the source is included when the book is built. -->"
                ""
            ] | append (picture $src $k.kind $n) | append [
                "```rust,noplayground"
                $"{{#include ../../../../../examples/rust/($k.kind)/($n).rs}}"
                "```"
            ]) | save -f --raw ($out | path join $k.kind $"($n).md")
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
    text ([$head] | append $part) | save -f --raw $summary
}

# Draws every harnessed example's picture again, from nothing: the
# settled frame as `<name>.png`, and the recording as `<name>.gif` for an
# example that declares one (`Example::RECORDING`). Then rewrites the
# pages, so a picture added or gone shows on them.
def "main pictures" [
    --jobs: int = 4     # examples drawn at once
] {
    cd ($env.FILE_PWD | path dirname)
    ^cargo build -p kui-native --examples
    let bin = (^cargo metadata --format-version 1 --no-deps | from json | get target_directory | path join debug examples)
    let img = ("docs/book/src" | path join $IMG)
    rm -rf --permanent $img
    let todo = ($PICTURED | each {|kind| examples-of $kind | each {|name| {kind: $kind, name: $name} } } | flatten)
    for kind in $PICTURED { mkdir ($img | path join $kind) }
    let failed = ($todo | par-each --threads $jobs {|j|
        let dir = ($img | path join $j.kind)
        let r = (^($bin | path join $j.name) --shot ($dir | path join $"($j.name).png") --record ($dir | path join $"($j.name).gif") | complete)
        print -n $r.stdout
        # A page shows the recording where there is one, so its still is
        # not kept.
        let gif = ($dir | path join $"($j.name).gif")
        if ($gif | path exists) { rm ($dir | path join $"($j.name).png") }
        if $r.exit_code != 0 { {example: $"($j.kind)/($j.name)", why: ($r.stderr | str trim)} }
    } | compact)
    generate docs/book/src
    if ($failed | is-not-empty) {
        print -e ($failed | table)
        exit 1
    }
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
