#!/usr/bin/env nu
# Prepares a release on any machine: what the tag's CI would refuse, found
# before the tag rather than after crates.io already holds the version.
#
#   nu scripts/release-prep.nu 0.1.0-alpha.60          check, then set the version
#   nu scripts/release-prep.nu                          the checks alone
#   nu scripts/release-prep.nu 0.1.0-alpha.60 --skip odin,ci
#
# alpha.59 is why it exists. F172 added a row to `schema::DOORS` and its
# round, on Windows, regenerated the Odin binding and index.d.ts but not
# docs/props.md, which the same `npm run gen` writes; Forgejo's `check` on
# the tag failed on the diff, and its `publish` - the drydock9 cargo copy -
# never ran, while GitHub's pipeline had already put the version on
# crates.io and npmjs, so the tag could not move. Each generated file has
# a CI step that diffs it; none of them ran on the machine that cut the
# release. This runs every one of them, in write mode, on a clean tree:
# whatever comes back changed was stale, and is left in the tree to commit.
#
# The steps, each skippable by name with --skip (said in the summary, so a
# skip is a choice and not an accident):
#
#   ci     the last completed Forgejo `check` on main or a release tag
#          passed (alpha.37 was tagged on a main that had been red for
#          days). Needs
#          $env.DRYDOCK9_TOKEN - `source ~/.local.nu` first.
#   fmt    cargo fmt --all
#   book   nu scripts/book-examples.nu (the book's Examples part)
#   node   npm run gen in packages/kui: jsx-runtime.d.ts, index.d.ts and
#          docs/props.md, from the schema and the addon's #[napi] surface
#   odin   nu scripts/odin.nu gen: packages/odin from kui.h and the schema
#          (needs odin, or ODIN, and clang)
#
# Then, given a version: the changelog's top heading must already read
# `## <version> (unreleased)` - the one heading set-version dates, and a
# trap hit four times as `## Unreleased` - and scripts/set-version.nu
# writes the version everywhere and runs check-version.nu. What is left
# (the tests, Native verification, the commit, the tag, the pushes) is
# printed at the end; docs/releasing.md has the why of each.

const ROOT = path self | path dirname | path dirname
const STEPS = [ci fmt book node odin]
const HOST = "https://drydock9.qxuken.dev"

def --wrapped must [cmd: string, ...args] {
    ^$cmd ...$args
    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: $"failed \(exit ($env.LAST_EXIT_CODE)\): ($cmd) ($args | str join ' ')"}
    }
}

def step [what: string] { print $"\n=== ($what)" }

# Every path git reports changed or untracked, relative to the root.
def changed [] {
    ^git status --porcelain --untracked-files=all | lines | each {|l| $l | str substring 3.. }
}

# The newest Forgejo `check` on main or a release tag that ran to an end,
# from the tasks API (the repository is private, so it needs the token). A
# tag counts: ci.yml's gate skips main's `check` for a commit a tag's run
# checked, so main's own newest can be one from before the last release.
def ci-on-main [] {
    let token = $env.DRYDOCK9_TOKEN? | default ""
    if $token == "" {
        error make {msg: "no $env.DRYDOCK9_TOKEN: `source ~/.local.nu` first, or pass --skip ci"}
    }
    let url = $"($HOST)/api/v1/repos/qxuken/kui/actions/tasks?limit=50"
    let runs = (http get --headers [Authorization $"token ($token)"] $url | get workflow_runs)
    let last = ($runs
        | where {|t| $t.name == "check" and ($t.head_branch == "main" or $t.head_branch =~ '^v\d') and $t.status in [success failure] }
        | get -o 0)
    if $last == null {
        error make {msg: "no finished `check` on main or a release tag among the last 50 Forgejo tasks"}
    }
    let on = if $last.head_branch == "main" { "main" } else { $last.head_branch }
    let sha = ($last.head_sha | str substring 0..7)
    let head = (^git rev-parse --short=8 HEAD | str trim)
    let at = if $sha == $head { "HEAD" } else { $"($sha), not HEAD ($head)" }
    let pending = ($runs | where {|t| $t.name == "check" and $t.id > $last.id and $t.status in [running waiting] } | length)
    let later = if $pending > 0 { $"; ($pending) newer `check` still running - wait for it" } else { "" }
    if $last.status != success {
        error make {msg: $"Forgejo `check` on ($on) failed at ($at): ($HOST)/qxuken/kui/actions/runs/($last.run_number)($later)"}
    }
    print $"Forgejo `check` on ($on) passed at ($at)"
}

def main [
    version?: string  # the version to release; without it, the checks alone
    --skip: string = ""  # steps to leave out, comma-separated: ci,fmt,book,node,odin
] {
    cd $ROOT
    let skip = $skip | split row "," | str trim | where {|s| $s != "" }
    let unknown = $skip | where {|s| $s not-in $STEPS }
    if ($unknown | is-not-empty) {
        error make {msg: $"unknown step ($unknown | str join ', '); one of ($STEPS | str join ', ')"}
    }

    # A clean tree is what makes "what the generators changed" readable.
    let dirty = changed
    if ($dirty | is-not-empty) {
        print -e $"the tree is not clean; commit or set aside first:\n  ($dirty | str join "\n  ")"
        exit 1
    }

    if $version != null {
        let heading = (open --raw CHANGELOG.md | lines | where {|l| $l starts-with "## " } | get -o 0 | default "")
        let want = $"## ($version) \(unreleased\)"
        if ($heading | str trim) != $want {
            print -e $"CHANGELOG.md's top heading is \"($heading)\"; set-version dates only \"($want)\""
            exit 1
        }
    }

    # Every step runs even after one fails, so one run says all there is.
    let steps = [
        [name what run];
        [ci "CI on main or the last tag" { ci-on-main }]
        [fmt "cargo fmt" { must cargo fmt --all }]
        [book "the book's Examples part" { must $nu.current-exe scripts/book-examples.nu }]
        [node "npm run gen (jsx-runtime.d.ts, index.d.ts, docs/props.md)" {
            # A napi-type-defs file left from an earlier build gets appended
            # to, not rewritten, and index.d.ts then has every addon member
            # twice; without one, gen-types.mjs forces the build that writes it.
            rm -rf target/napi-type-defs
            cd packages/kui
            must npm run gen
        }]
        [odin "the Odin binding" {
            let odin = $env.ODIN? | default "odin"
            if (which $odin | is-empty) and not ($odin | path expand | path exists) {
                error make {msg: "no odin on PATH: set ODIN to the compiler, or pass --skip odin"}
            }
            must $nu.current-exe scripts/odin.nu gen
        }]
    ]
    mut failed = []
    for s in ($steps | where {|s| $s.name not-in $skip }) {
        step $s.what
        let ok = try { do $s.run; true } catch {|e| print -e $"($s.name) failed: ($e.msg)"; false }
        if not $ok { $failed = ($failed | append $s.name) }
    }

    let stale = changed
    if ($stale | is-not-empty) {
        print -e $"\nstale, regenerated in the tree - review, commit, run this again:\n  ($stale | str join "\n  ")"
        ^git diff --stat
    }
    if ($failed | is-not-empty) {
        print -e $"\nfailed: ($failed | str join ', ')"
    }
    if ($stale | is-not-empty) or ($failed | is-not-empty) { exit 1 }
    let skipped = if ($skip | is-empty) { "" } else { $" \(skipped: ($skip | str join ', '))" }
    print $"\nevery generated file is current($skipped)"

    if $version == null { return }

    step $"set-version ($version)"
    must $nu.current-exe scripts/set-version.nu $version
    let tag = $"v($version)"
    # GitHub's remote by its URL, whatever this clone named it.
    let github = (^git remote -v | lines | parse -r '^(?<name>\S+)\s+\S*github\.com\S*\s+\(push\)$' | get -o 0.name | default "github")
    print $"
what is left, in order:
  nu scripts/test.nu --node         the workspace and Node, on this tree
  write CHANGELOG.md's ### Native verification for ($version)
  git commit -am '($version): ...'
  git tag ($tag)
  git push --atomic origin main ($tag)
  git push ($github) main ($tag)    the mirror does not start GitHub's pipeline"
}
