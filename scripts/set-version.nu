#!/usr/bin/env nu
# Writes one release version everywhere it is spelled out: the workspace
# version, the workspace's kui-* dependency requirements (cargo publish needs
# a version next to each path dependency - matched by pattern rather than by
# a list of names, because the list was `kui`, `kui-core` and `kui-wgpu` and
# `kui-ffi` was added to the workspace after it, so alpha.10's bump left that
# one requirement at alpha.9 and check-version below is what caught it),
# packages/kui/package.json, the changelog's top heading, the book's setup
# page, and the examples' npm lockfile, which mirrors the linked package's
# version. The release workflow refuses a tag that does not match all of
# them (scripts/check-version.nu), so bump, commit, then tag `v<version>`.
#
#   nu scripts/set-version.nu 0.1.0-alpha.2

# `path`'s lines with `edit` applied to each, written back only where one
# changed. Line by line, as the sed it replaces was, so a file's own line
# ending (CRLF in a Windows checkout) survives.
def edit-lines [path: string, edit: closure] {
    let text = (open --raw $path | decode utf-8)
    let crlf = ($text | str contains "\r\n")
    let out = ($text | lines | each $edit | str join (if $crlf { "\r\n" } else { "\n" }))
    let out = if ($text | str ends-with "\n") { $out + (if $crlf { "\r\n" } else { "\n" }) } else { $out }
    if $out != $text { $out | save -f --raw $path }
}

def main [version: string] {
    cd ($env.FILE_PWD | path dirname)
    let ver = $version
    edit-lines Cargo.toml {|l|
        $l
        | str replace -r '^version = "[^"]+"' $'version = "($ver)"'
        | str replace -r '^(kui(-[a-z]+)? = \{ path = "[^"]+", version = )"[^"]+"' $'${1}"($ver)"'
    }
    # The changelog's top heading is the fourth place the version is spelled
    # out and the only one a human writes, so it is the one that ships
    # stale: date the open section here. Only the first `## ` line is
    # looked at, so no released section below it can be touched. Only the
    # heading line - what the release adds and what you can delete stay a
    # person's to write.
    let today = (date now | format date "%Y-%m-%d")
    let changelog = (open --raw CHANGELOG.md | decode utf-8)
    let first = ($changelog | lines | enumerate | where {|r| $r.item starts-with "## " } | get -o 0)
    if $first != null and ($first.item =~ '^## .+ \(unreleased\)\r?$') {
        let dated = $"## ($ver) \(($today)\)"
        edit-lines CHANGELOG.md {|l| if $l == $first.item { $dated } else { $l } }
    }
    # The book's setup page spells out the dependency line a reader copies
    # into their own Cargo.toml. Nothing wrote it, so the alpha.28 book asked
    # for alpha.27; the book is published from the release tag, so the line
    # has to name the version that tag publishes. README.md's Install
    # section and docs/releasing.md show the same line. Either spelling,
    # `"<v>"` or `{ version = "<v>", ... }`.
    for page in [docs/book/src/setup.md README.md docs/releasing.md] {
        edit-lines $page {|l|
            $l | str replace -r '^(kui-native = (?:\{ version = )?)"[^"]+"' $'${1}"($ver)"'
        }
    }
    # Dating that heading moves its anchor, and docs/howto.md links the open
    # section by it while the release is being written -
    # `#010-alpha10-unreleased` for `## 0.1.0-alpha.10 (unreleased)`. Left
    # alone, three links stop landing between the last commit and the tag:
    # the drift backlog F33 was filed for, arriving through a link rather
    # than a sentence, and produced by the release round rather than by
    # whoever wrote the page. So the same script that moves the anchor moves
    # what points at it. The guard is `crates/kui-core/tests/docs.rs`, which
    # fails the workspace run on a howto anchor no heading answers - run the
    # tests after this script, not before.
    let slug = ($ver | str replace -a "." "")
    edit-lines docs/howto.md {|l| $l | str replace -a $"#($slug)-unreleased" $"#($slug)-($today)" }
    do {
        cd packages/kui
        ^npm version --no-git-tag-version --allow-same-version $ver | ignore
    }
    # examples/node depends on packages/kui by `file:` path, so its lockfile
    # carries a copy of that manifest's version in its `../../packages/kui`
    # entry - which the line above has just made stale. `--package-lock-only`
    # rewrites the lockfile and nothing else (no node_modules, no network),
    # so npm stays the only thing that writes its own format. This one went
    # from alpha.2 to alpha.5 unnoticed because nothing here wrote it and
    # nothing checked it; check-version now does the second half.
    do {
        cd examples/node
        ^npm install --package-lock-only --offline --no-audit --no-fund --silent
    }
    # Cargo.lock records the workspace crates' own versions.
    ^cargo update --workspace --offline --quiet
    ^$nu.current-exe scripts/check-version.nu $ver
}
