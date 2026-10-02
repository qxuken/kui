#!/usr/bin/env nu
# Publishes the workspace's crates to one cargo registry, in dependency
# order, skipping each crate whose version that registry already holds - so
# a run cut short (crates.io's rate limit on new crates, a network error, a
# job cancelled halfway) is finished by running it again rather than failed
# by "already exists". The release workflow and scripts/release-local.nu
# both publish through it.
#
#   nu scripts/publish-crates.nu crates-io
#   nu scripts/publish-crates.nu drydock9
#   nu scripts/publish-crates.nu crates-io --dry-run
#
# crates.io goes first. The kui-* dependencies name crates.io (the
# workspace's [workspace.dependencies] carry no `registry` key, because
# crates.io refuses a dependency from any other registry), so a crate's
# verify build against either registry resolves its kui-* dependencies
# there - the Forgejo copies included.
#
# The token comes from cargo's own places: CARGO_REGISTRY_TOKEN (crates.io)
# or CARGO_REGISTRIES_DRYDOCK9_TOKEN (`Bearer <token>`) in the environment,
# as CI sets them, or `cargo login` / ~/.cargo/credentials.toml.

# Dependency order: each crate's verify build needs the ones before it.
const CRATES = [kui-derive kui-core kui-wgpu kui-native kui-lua kui-ffi]

const INDEX = {
    crates-io: "https://index.crates.io"
    drydock9: "https://drydock9.qxuken.dev/api/packages/qxuken/cargo"
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

def main [
    registry: string  # crates-io or drydock9
    --dry-run         # package and verify, publish nothing
] {
    cd ($env.FILE_PWD | path dirname)
    if $registry not-in ($INDEX | columns) {
        error make {msg: $"unknown registry ($registry); one of ($INDEX | columns | str join ', ')"}
    }
    let version = (open Cargo.toml | get workspace.package.version)
    let pending = if $dry_run { $CRATES } else {
        $CRATES | where {|c| not (crate-published $registry $c $version) }
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
