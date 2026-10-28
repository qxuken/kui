#!/usr/bin/env nu
# Publishes the workspace's crates to one cargo registry, in dependency
# order, skipping each crate whose version that registry already holds - so
# a run cut short (crates.io's rate limit on new crates, a network error, a
# job cancelled halfway) is finished by running it again rather than failed
# by "already exists". The release workflow publishes through it, and so
# does a maintainer finishing a release by hand.
#
#   nu scripts/publish-crates.nu crates-io
#   nu scripts/publish-crates.nu crates-io --dry-run
#
# crates.io is the one registry since alpha.68. Until then the crates went
# to the Forgejo registry (`drydock9`) as well, which still holds every
# version up to alpha.67 and takes no new ones.
#
# The token comes from cargo's own places: CARGO_REGISTRY_TOKEN in the
# environment, as CI sets it, or `cargo login` / ~/.cargo/credentials.toml.
# CI's is a trusted-publishing token (since alpha.70), which publishes a
# new version of a crate crates.io holds and cannot create one; `--trusted`
# says so before anything goes out, naming the crates crates.io has never
# held, which a maintainer publishes once by hand with a token of their own.

# Dependency order: each crate's verify build needs the ones before it.
# kui-trace (ADR 0058) is before kui-core, which writes its format;
# kui-value and kui-channel (ADR 0075) before both, which build on them.
const CRATES = [kui-derive kui-value kui-channel kui-trace kui-core kui-wgpu kui-native kui-lua kui-ffi]

const INDEX = {
    crates-io: "https://index.crates.io"
}

# The cargo sparse index path of a crate (lowercase; 1, 2, 3 and 4+ chars).
def index-path [name: string] {
    let n = ($name | str lowercase)
    match ($n | str length) {
        1 => $"1/($n)"
        2 => $"2/($n)"
        3 => $"3/($n | str substring 0..0)/($n)"
        _ => $"($n | str substring 0..1)/($n | str substring 2..3)/($n)"
    }
}

# Whether `registry`'s index lists `name` at `version`. An unreachable index
# reads as "not published", and cargo then says what is wrong.
export def crate-published [registry: string, name: string, version: string] {
    let url = $"($INDEX | get $registry)/(index-path $name)"
    let body = (try { http get --raw --headers [User-Agent "kui-release (publish-crates.nu)"] $url } catch { "" })
    $body | lines | any {|l| ($l | from json | get -o vers) == $version }
}

# Whether `registry`'s index holds `name` at all, at any version.
export def crate-known [registry: string, name: string] {
    let url = $"($INDEX | get $registry)/(index-path $name)"
    let body = (try { http get --raw --headers [User-Agent "kui-release (publish-crates.nu)"] $url } catch { "" })
    $body | lines | any {|l| ($l | from json | get -o name) == $name }
}

def main [
    registry: string  # crates-io
    --dry-run         # package and verify, publish nothing
    --trusted         # the token is crates.io's trusted publishing: refuse a crate it never held
] {
    cd ($env.FILE_PWD | path dirname)
    if $registry not-in ($INDEX | columns) {
        error make {msg: $"unknown registry ($registry); one of ($INDEX | columns | str join ', ')"}
    }
    let version = (open Cargo.toml | get workspace.package.version)
    let pending = if $dry_run { $CRATES } else {
        $CRATES | where {|c| not (crate-published $registry $c $version) }
    }
    if $trusted and not $dry_run {
        let new = ($pending | where {|c| not (crate-known $registry $c) })
        if ($new | is-not-empty) {
            let flags = ($new | each {|c| $"-p ($c)" } | str join ' ')
            error make {msg: $"($registry) has never held ($new | str join ', '), and a trusted-publishing token cannot create a crate: from this tag, `cargo publish ($flags) --registry ($registry)` with a token of your own, give each release.yml as its trusted publisher on crates.io, then re-run this job \(docs/releasing.md\)"}
        }
    }
    if ($pending | is-empty) {
        print $"($registry): every crate is already at ($version)"
        return
    }
    print $"($registry): publishing ($pending | str join ', ') at ($version)"
    # One invocation: cargo publishes in dependency order and waits for each
    # crate to reach the index before verifying the next against it.
    let args = ($pending | each {|c| ["-p" $c] } | flatten)
    let dry = if $dry_run { ["--dry-run"] } else { [] }
    ^cargo publish ...$dry ...$args --registry $registry
    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: $"cargo publish to ($registry) failed \(exit ($env.LAST_EXIT_CODE)\); run this again to publish what is left"}
    }
}
