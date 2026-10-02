#!/usr/bin/env nu
# Fails unless the workspace version, every kui-* dependency requirement,
# packages/kui/package.json, the changelog's top heading, the book's setup
# page and the examples' npm lockfile all name `version` - the version a
# `v*` tag names.
#
#   nu scripts/check-version.nu 0.1.0-alpha.2

def main [version: string] {
    cd ($env.FILE_PWD | path dirname)
    let want = $version
    mut bad = []
    let meta = (^cargo metadata --no-deps --offline --format-version 1 | from json)
    for p in $meta.packages {
        if $p.version != $want { $bad = ($bad | append $"($p.name) is ($p.version)") }
        for d in $p.dependencies {
            if not ($d.name | str starts-with "kui") { continue }
            # kui-core dev-depends on itself to turn on the `conformance`
            # feature for its own tests. A self dependency resolves to the
            # crate being built, never to the registry, so it carries a path
            # and no version - and cargo publish strips it. Requiring a
            # version here would demand kui-core 0.1.0-alpha.6 from the
            # registry while publishing kui-core 0.1.0-alpha.6. Every other
            # dev-dependency is still checked: kui-lua names two, both with
            # versions.
            if $d.name == $p.name { continue }
            # A dev-dependency that names a path and no version is dropped
            # from the manifest cargo publishes - `cargo package -p
            # kui-native` ships an empty [dev-dependencies] - so it can never
            # be asked for at the registry and has no version to agree with.
            # That is kui-devtools, the harness the examples run in (publish
            # = false), which every crate with an example dev-depends on
            # since ADR 0021, and which this check refused for alpha.11 as
            # "requires kui-devtools *".
            if $d.kind? == "dev" and $d.req == "*" and ($d.path? != null) { continue }
            if ($d.req | str replace -r '^\^' '') != $want {
                $bad = ($bad | append $"($p.name) requires ($d.name) ($d.req)")
            }
        }
    }
    let npm = (open packages/kui/package.json | get version)
    if $npm != $want { $bad = ($bad | append $"packages/kui/package.json is ($npm)") }
    # examples/node links packages/kui by `file:` path, so its lockfile
    # mirrors the version in that manifest. Nothing wrote it and nothing
    # read it until now, which is how it sat at alpha.2 through two
    # releases.
    let lock_path = "examples/node/package-lock.json"
    let linked = (open $lock_path | get -o packages | default {} | get -o "../../packages/kui")
    if $linked == null {
        $bad = ($bad | append $"($lock_path) has no \"../../packages/kui\" entry")
    } else if $linked.version != $want {
        $bad = ($bad | append $"($lock_path) records ($linked.version)")
    }
    # The changelog heading is written by hand, so it is the one that ships
    # saying "unreleased" the day the release goes out.
    let heading = (open --raw CHANGELOG.md | lines | where {|l| $l starts-with "## " } | get -o 0)
    let names = ($heading != null) and (($heading | str substring 3.. | split row --regex '\s+') | any {|w| $w == $want })
    if not $names or ($heading | default "" | str lowercase | str contains "unreleased") {
        let shown = if $heading == null { "missing" } else { $"\"($heading)\"" }
        $bad = ($bad | append $"CHANGELOG.md top heading is ($shown)")
    }
    # The setup page of the book names the version a reader depends on, and
    # the book is published from the tag this checks.
    let setup = "docs/book/src/setup.md"
    let pin = (open --raw $setup | lines | parse -r '^kui-native = (?:\{ version = )?"(?<v>[^"]+)"' | get -o 0.v)
    if $pin == null {
        $bad = ($bad | append $"($setup) has no kui-native dependency line")
    } else if $pin != $want {
        $bad = ($bad | append $"($setup) asks for ($pin)")
    }
    if ($bad | is-not-empty) {
        print -e $"version mismatch, expected ($want):\n  ($bad | str join "\n  ")\n\(scripts/set-version.nu sets all of them\)"
        exit 1
    }
    print $"all manifests and the changelog at ($want)"
}
