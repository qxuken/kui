#!/usr/bin/env nu
# Prepares a release on any machine: what the tag's CI would refuse, found
# before the tag rather than after crates.io already holds the version.
#
#   nu scripts/release-prep.nu 0.1.0-alpha.60          check, then set the version
#   nu scripts/release-prep.nu                          the checks alone
#   nu scripts/release-prep.nu 0.1.0-alpha.60 --skip odin
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
# Since alpha.66 nothing runs Forgejo's `check` unasked (ci.yml is
# dispatched by hand), so there is no CI on main to ask about, and the
# step that read its last result is gone: the machine that cuts the
# release is the check, with what this prints at the end.
#
# The steps, each skippable by name with --skip (said in the summary, so a
# skip is a choice and not an accident): `fmt` (cargo fmt --all), then the
# generators, which nu scripts/gen.nu runs and lists — `lua`, `node`,
# `odin` (needs odin, or ODIN, and clang) and `book`.
#
# Then, given a version: the changelog's top heading must already read
# `## <version> (unreleased)` - the one heading set-version dates, and a
# trap hit four times as `## Unreleased` - and scripts/set-version.nu
# writes the version everywhere and runs check-version.nu. What is left
# (the tests, Native verification, the commit, the tag, the pushes) is
# printed at the end; docs/releasing.md has the why of each.

const ROOT = path self | path dirname | path dirname
const STEPS = [fmt lua node odin book]

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

def main [
    version?: string  # the version to release; without it, the checks alone
    --skip: string = ""  # steps to leave out, comma-separated: fmt,lua,node,odin,book
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

    # Both run even after one fails, so one run says all there is; gen.nu
    # runs each of its own steps the same way and names the ones that failed.
    mut failed = []
    if "fmt" not-in $skip {
        step "cargo fmt"
        let ok = try { must cargo fmt --all; true } catch {|e| print -e $"fmt failed: ($e.msg)"; false }
        if not $ok { $failed = ($failed | append fmt) }
    }
    let gen_skip = $skip | where {|s| $s != "fmt" }
    let generators = $STEPS | where {|s| $s != "fmt" and $s not-in $gen_skip }
    if ($generators | is-not-empty) {
        step "the generators (scripts/gen.nu)"
        let ok = try {
            must $nu.current-exe scripts/gen.nu --skip ($gen_skip | str join ",")
            true
        } catch {|e| print -e $"gen failed: ($e.msg)"; false }
        if not $ok { $failed = ($failed | append gen) }
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
  git push origin main ($tag)
  git push ($github) main ($tag)    the mirror does not start GitHub's pipeline
  the drydock9 crates, by hand once crates.io has them: dispatch Forgejo's ci on ($tag) with publish"
}
