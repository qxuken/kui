#!/bin/sh
# Installs the kui runner on macOS or Linux (ADR 0068): picks this machine's
# release asset, checks it against the release's SHA256SUMS, and puts `kui`
# in ~/.kui/bin. It edits no shell startup file; it prints the PATH line
# for yours.
#
#   curl -fsSL https://github.com/qxuken/kui/releases/download/v<version>/install.sh | sh
#   sh install.sh                     the newest release
#   sh install.sh 0.1.0-alpha.68      that one (a leading `v` is fine)
#
# Environment:
#   KUI_VERSION      the version, when no argument names one
#   KUI_INSTALL_DIR  where `kui` goes (default ~/.kui/bin)
#   KUI_MIRROR       a mirror with the release's layout, used instead of
#                    GitHub: <mirror>/v<version>/<asset>, and
#                    <mirror>/latest.txt holding the newest version for a
#                    run that names none (`kui runners fetch` lays one out)
#
# The assets, per release: kui-<version>-<platform>.tar.gz, one file `kui`
# in it, for linux-x64, linux-arm64 (glibc 2.28 or newer) and darwin-arm64;
# Windows has install.ps1. "The newest release" is the newest the GitHub
# API lists, prereleases included: every kui release so far is an alpha,
# and GitHub's own `latest` skips prereleases.
#
# Everything is inside main, called on the last line, so a download cut
# short runs nothing.

set -eu

REPO="qxuken/kui"

say() { printf '%s\n' "$*"; }
die() { printf 'kui install: %s\n' "$*" >&2; exit 1; }

# Fetches $1 to the file $2, or to stdout when $2 is "-".
fetch() {
    if command -v curl >/dev/null 2>&1; then
        if [ "$2" = - ]; then curl -fsSL --retry 3 "$1"; else curl -fsSL --retry 3 -o "$2" "$1"; fi
    elif command -v wget >/dev/null 2>&1; then
        if [ "$2" = - ]; then wget -q -O - "$1"; else wget -q -O "$2" "$1"; fi
    else
        die "neither curl nor wget is installed"
    fi
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1
    else
        die "no sha256sum, shasum or openssl to check the download with"
    fi
}

# This machine in the release's spelling: linux-x64, linux-arm64, darwin-arm64.
detect_platform() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$arch" in
        x86_64 | amd64) arch=x64 ;;
        aarch64 | arm64) arch=arm64 ;;
        *) die "no kui build for the $arch CPU (there are x64 and arm64 ones); build it from source: cargo build -p kui-cli --release in https://github.com/$REPO" ;;
    esac
    case "$os" in
        Darwin)
            # A shell under Rosetta on Apple silicon reports x86_64; the
            # arm64 build is the one for the machine.
            if [ "$arch" = x64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
                arch=arm64
            fi
            [ "$arch" = arm64 ] || die "there is no Intel Mac build of kui; build it from source: cargo build -p kui-cli --release in https://github.com/$REPO"
            say darwin-arm64
            ;;
        Linux)
            if ldd --version 2>&1 | grep -qi musl || ls /lib/ld-musl-* >/dev/null 2>&1; then
                die "this Linux uses musl (Alpine?); the kui build links glibc 2.28 or newer. Build it from source: cargo build -p kui-cli --release in https://github.com/$REPO"
            fi
            say "linux-$arch"
            ;;
        MINGW* | MSYS* | CYGWIN* | Windows_NT)
            die "on Windows, run install.ps1 in PowerShell: irm https://github.com/$REPO/releases/download/v<version>/install.ps1 | iex"
            ;;
        *) die "no kui build for $os; build it from source: cargo build -p kui-cli --release in https://github.com/$REPO" ;;
    esac
}

# The newest version: the mirror's latest.txt, else the newest release the
# GitHub API lists (prereleases included; see the top of this file).
latest_version() {
    if [ -n "${KUI_MIRROR:-}" ]; then
        v=$(fetch "$KUI_MIRROR/latest.txt" -) || die "could not read $KUI_MIRROR/latest.txt (a mirror holds the newest version there; or name one: KUI_VERSION=<version>)"
        v=$(printf '%s' "$v" | tr -d ' \t\r\n')
    else
        v=$(fetch "https://api.github.com/repos/$REPO/releases?per_page=1" - |
            sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1) ||
            die "could not ask GitHub for the newest release; name one: sh install.sh <version>"
    fi
    [ -n "$v" ] || die "could not tell the newest kui version; name one: sh install.sh <version>"
    say "${v#v}"
}

# The line that puts $1 on PATH, for the user's shell.
path_hint() {
    dir=$1
    shown=$dir
    case "$dir" in "$HOME"/*) shown="\$HOME${dir#"$HOME"}" ;; esac
    case "$(basename "${SHELL:-sh}")" in
        zsh) say "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.zshrc" ;;
        bash)
            rc=~/.bashrc
            [ "$(uname -s)" = Darwin ] && rc=~/.bash_profile
            say "  echo 'export PATH=\"$shown:\$PATH\"' >> $rc"
            ;;
        fish) say "  fish_add_path $dir" ;;
        nu) say "  \$env.PATH = (\$env.PATH | prepend '$dir')    # in \$nu.config-path" ;;
        *) say "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.profile" ;;
    esac
}

main() {
    version=${1:-${KUI_VERSION:-}}
    version=${version#v}
    dir=${KUI_INSTALL_DIR:-$HOME/.kui/bin}
    platform=$(detect_platform)
    [ -n "$version" ] || version=$(latest_version)

    asset="kui-$version-$platform.tar.gz"
    if [ -n "${KUI_MIRROR:-}" ]; then
        base="${KUI_MIRROR%/}/v$version"
    else
        base="https://github.com/$REPO/releases/download/v$version"
    fi

    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t kui-install)
    trap 'rm -rf "$tmp"' EXIT
    trap 'exit 130' INT TERM

    say "kui $version for $platform from $base"
    fetch "$base/$asset" "$tmp/$asset" || die "could not download $base/$asset (is $version a kui release, and one with runner binaries?)"
    fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || die "could not download $base/SHA256SUMS"

    # `<hex>  <name>`, or `<hex> *<name>` as sha256sum -b writes it.
    want=$(awk -v n="$asset" '$2 == n || $2 == "*" n { print $1; exit }' "$tmp/SHA256SUMS")
    [ -n "$want" ] || die "SHA256SUMS has no line for $asset"
    got=$(sha256 "$tmp/$asset")
    want=$(printf '%s' "$want" | tr 'A-F' 'a-f')
    [ "$got" = "$want" ] || die "checksum mismatch for $asset: expected $want, got $got; nothing was installed"

    mkdir -p "$tmp/x"
    tar -xzf "$tmp/$asset" -C "$tmp/x" kui || die "$asset holds no kui"
    mkdir -p "$dir"
    # Next to the target, then renamed over it: a running kui keeps its
    # old file, and no reader ever sees half a binary.
    cp "$tmp/x/kui" "$dir/.kui.new.$$"
    chmod 755 "$dir/.kui.new.$$"
    mv -f "$dir/.kui.new.$$" "$dir/kui"
    # curl and wget set no quarantine attribute; this is in case the
    # script ran from a browser download.
    if [ "$(uname -s)" = Darwin ]; then
        xattr -d com.apple.quarantine "$dir/kui" 2>/dev/null || true
    fi

    say "installed $("$dir/kui" -V 2>/dev/null || say "kui $version") at $dir/kui"
    case ":$PATH:" in
        *":$dir:"*) ;;
        *)
            say ""
            say "$dir is not on your PATH. Add it with:"
            path_hint "$dir"
            say "and open a new shell."
            ;;
    esac
}

main "$@"
