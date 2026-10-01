#!/usr/bin/env nu
# Publishes a tagged release from this Mac, doing what the `build-*` and
# `publish` jobs of .forgejo/workflows/ci.yml do on the runner: the five
# Node prebuilds, the verification, `cargo publish`, `npm publish` and the
# `latest` guard. For when the runner cannot reach Forgejo (alpha.17's tag
# job hung in checkout on 2026-09-25 and was released this way).
#
#   nu scripts/release-local.nu              build, verify, ask, publish
#   nu scripts/release-local.nu --dry-run    build and verify, publish nothing
#   nu scripts/release-local.nu --yes        no question before publishing
#
# Needs: the tag `v<version>` on HEAD and a clean tree; Docker running (the
# two Linux prebuilds build in node:24-bookworm, with CI's pinned
# cargo-zigbuild and zig); cargo-xwin (Windows); node on PATH; and
# $env.DRYDOCK9_TOKEN, a token with write:package — `source ~/.local.nu`
# first if it lives there. The token reaches cargo through the environment
# and npm through a throwaway userconfig that names the variable, so it is
# never written to disk or printed.
#
# Safe to run again after a partial publish: a crate or npm version the
# registry already holds is skipped, not failed on. If the runner comes
# back afterwards, skip the tag's CI run — it would fail on "already
# exists".
#
# What differs from CI: the macOS prebuilds link against this Mac's SDK
# (targeting macOS 11.0 arm64 and 10.12 x64, as CI's do) rather than the
# pinned 15.5 SDK, and cargo-xwin uses brew's LLVM rather than Debian's.

const HOST = "https://drydock9.qxuken.dev"
const NPM_REGISTRY = "https://drydock9.qxuken.dev/api/packages/qxuken/npm/"
const CRATES = [kui-derive kui-core kui-wgpu kui-native kui-lua kui-ffi]
const PREBUILDS = [darwin-arm64 darwin-x64 linux-arm64 linux-x64 win32-x64]

# Runs an external command and fails the script when it fails.
def --wrapped must [cmd: string, ...args] {
    ^$cmd ...$args
    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: $"failed \(exit ($env.LAST_EXIT_CODE)\): ($cmd) ($args | str join ' ')"}
    }
}

# `-p a -p b ...`: cargo takes one crate per `-p`.
def package-args [crates: list<string>] { $crates | each {|c| ["-p" $c] } | flatten }

def step [what: string] { print $"\n=== ($what)" }

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

def crate-published [name: string, version: string] {
    let url = $"($HOST)/api/packages/qxuken/cargo/(index-path $name)"
    let body = (try { http get --raw $url } catch { "" })
    $body | str contains $'"vers":"($version)"'
}

def npm-published [version: string] {
    let r = (^npm view $"@qxuken/kui@($version)" version --registry $NPM_REGISTRY | complete)
    ($r.exit_code == 0) and (($r.stdout | str trim) == $version)
}

# CI's build-linux job, both targets in one container of the host's arch.
const LINUX_BUILD = '
set -eux
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl python3-pip pkg-config libasound2-dev zstd
dpkg --add-architecture amd64
apt-get update
apt-get install -y --no-install-recommends libasound2-dev:amd64
curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
  -t x86_64-unknown-linux-gnu -t aarch64-unknown-linux-gnu
export PATH="$HOME/.cargo/bin:$PATH"
pip3 install --break-system-packages "cargo-zigbuild==0.23.3" "ziglang==0.16.0"
nu_dir="nu-0.116.0-$(uname -m)-unknown-linux-gnu"
curl -fL --retry 3 "https://github.com/nushell/nushell/releases/download/0.116.0/${nu_dir}.tar.gz" \
  | tar -xz -C /usr/local/bin --strip-components=1 "${nu_dir}/nu"
mkdir -p /work && cd /work && tar -xf /src.tar
cargo zigbuild -p kui-node --release --target aarch64-unknown-linux-gnu.2.28
nu scripts/collect-prebuild.nu aarch64-unknown-linux-gnu linux-arm64
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig \
PKG_CONFIG_LIBDIR=/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo zigbuild -p kui-node --release --target x86_64-unknown-linux-gnu.2.28
nu scripts/collect-prebuild.nu x86_64-unknown-linux-gnu linux-x64
cp -r packages/kui/prebuilds/linux-arm64 packages/kui/prebuilds/linux-x64 /out/
'

def main [
    --dry-run   # build and verify, publish nothing
    --yes       # publish without asking
] {
    let root = ($env.FILE_PWD | path dirname)
    cd $root
    let version = (open packages/kui/package.json | get version)
    let tag = $"v($version)"

    step $"checks for ($tag)"
    if (^git status --porcelain --untracked-files=no | str trim | is-not-empty) {
        error make {msg: "the working tree is not clean"}
    }
    let tagged = (^git tag --points-at HEAD | lines)
    if $tag not-in $tagged {
        error make {msg: $"($tag) does not point at HEAD \(tags here: ($tagged | str join ', ')\)"}
    }
    must $nu.current-exe scripts/check-version.nu $version
    if (which node | is-empty) or (which npm | is-empty) { error make {msg: "node and npm must be on PATH"} }
    if (which cargo-xwin | is-empty) { error make {msg: "cargo-xwin is not installed (cargo install cargo-xwin --locked)"} }
    if ((^docker info | complete).exit_code != 0) { error make {msg: "Docker is not running"} }
    if not $dry_run and ($env.DRYDOCK9_TOKEN? | is-empty) {
        error make {msg: "$env.DRYDOCK9_TOKEN is not set (source ~/.local.nu?)"}
    }

    let scratch = (mktemp -d -t kui-release.XXXXXX)
    print $"scratch: ($scratch)"
    rm -rf packages/kui/prebuilds

    step "linux prebuilds (docker)"
    ^git archive --format=tar $tag | save --raw $"($scratch)/src.tar"
    $LINUX_BUILD | save $"($scratch)/linux-build.sh"
    mkdir $"($scratch)/out"
    let arch = if ((^uname -m | str trim) == "arm64") { "linux/arm64" } else { "linux/amd64" }
    must docker run --rm --platform $arch -v $"($scratch)/src.tar:/src.tar:ro" -v $"($scratch)/linux-build.sh:/build.sh:ro" -v $"($scratch)/out:/out" node:24-bookworm bash /build.sh

    step "macOS and Windows prebuilds"
    must rustup target add aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc
    # Each into its own target dir, never target/release — the windowed
    # smoke round leaves a smoke-featured addon there.
    for t in [[target prebuild]; [aarch64-apple-darwin darwin-arm64] [x86_64-apple-darwin darwin-x64]] {
        must cargo build -p kui-node --release --target $t.target
        must $nu.current-exe scripts/collect-prebuild.nu $t.target $t.prebuild
    }
    must cargo xwin build -p kui-node --release --target x86_64-pc-windows-msvc
    must $nu.current-exe scripts/collect-prebuild.nu x86_64-pc-windows-msvc win32-x64
    cp -r $"($scratch)/out/linux-arm64" $"($scratch)/out/linux-x64" packages/kui/prebuilds/

    step "verify the bundled prebuilds"
    for p in $PREBUILDS {
        if not ($"packages/kui/prebuilds/($p)/kui_node.node" | path exists) {
            error make {msg: $"missing prebuild ($p)"}
        }
    }
    do {
        cd packages/kui
        must npm test
        must npm pack --dry-run
    }
    must cargo publish --dry-run ...(package-args $CRATES) --registry drydock9

    if $dry_run {
        print $"\ndry run: ($tag) built and verified, nothing published"
        return
    }
    if not $yes {
        let answer = (input $"\nPublish ($tag) to ($HOST)? Type the version to confirm: ")
        if $answer != $version { print "not published"; return }
    }

    step "cargo publish"
    let pending = ($CRATES | where {|c| not (crate-published $c $version) })
    if ($pending | is-empty) {
        print "every crate is already published"
    } else {
        print $"publishing: ($pending | str join ', ')"
        with-env {CARGO_REGISTRIES_DRYDOCK9_TOKEN: $"Bearer ($env.DRYDOCK9_TOKEN)"} {
            must cargo publish ...(package-args $pending) --registry drydock9
        }
    }

    step "npm publish"
    # A userconfig naming the variable, not holding the token: npm expands
    # ${VAR} in .npmrc, so the token stays in the environment.
    let npmrc = $"($scratch)/npmrc"
    let auth_key = ($NPM_REGISTRY | str replace "https:" "")
    $"($auth_key):_authToken=${DRYDOCK9_TOKEN}\n" | save -f $npmrc
    let dist_tag = if ($version | str contains "-") {
        $version | split row "-" | skip 1 | str join "-" | split row "." | first
    } else { "latest" }
    if (npm-published $version) {
        print $"@qxuken/kui@($version) is already published"
    } else {
        do {
            cd packages/kui
            must npm publish --registry $NPM_REGISTRY --tag $dist_tag --userconfig $npmrc
        }
    }
    # CI's guard: a prerelease also takes `latest` when it sorts highest,
    # since a registry without `latest` answers `npm view` with nothing.
    if $dist_tag != "latest" {
        let versions = (^npm view $"@qxuken/kui@($dist_tag)" versions --json --registry $NPM_REGISTRY)
        let highest = (with-env {VERSIONS: $versions} {
            ^node -e '
              const root = require("node:child_process").execFileSync("npm", ["root", "-g"], { encoding: "utf8" }).trim();
              const semver = require(require.resolve("semver", { paths: [root + "/npm/node_modules"] }));
              const l = JSON.parse(process.env.VERSIONS);
              process.stdout.write((Array.isArray(l) ? l : [l]).filter((v) => semver.valid(v)).sort(semver.rcompare)[0]);
            '
        })
        if $highest == $version {
            must npm dist-tag add $"@qxuken/kui@($version)" latest --registry $NPM_REGISTRY --userconfig $npmrc
        } else {
            print $"latest left alone: ($highest) sorts above ($version)"
        }
    }
    rm -f $npmrc

    step "registries"
    for c in $CRATES {
        print $"($c): (if (crate-published $c $version) { 'published' } else { 'MISSING' })"
    }
    print (^npm view @qxuken/kui dist-tags --registry $NPM_REGISTRY)
    rm -rf $scratch
}
