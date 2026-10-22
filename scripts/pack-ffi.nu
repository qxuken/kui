#!/usr/bin/env nu
# A pack of the C library for every platform kui ships: libkui_ffi, dynamic
# and static, with the header - what a C, Odin or other non-Rust host links
# instead of building the workspace.
#
#   nu scripts/pack-ffi.nu                         every platform this machine can build
#   nu scripts/pack-ffi.nu --platforms linux-x64,darwin-arm64
#   nu scripts/pack-ffi.nu --accept-msvc-license   Windows too (see below)
#   nu scripts/pack-ffi.nu --no-runner             without the windowed runner (kui_run)
#   nu scripts/pack-ffi.nu --platforms linux-x64 --zig   this Linux machine's own, at glibc 2.28
#   nu scripts/pack-ffi.nu bundle [--partial]      the release's two tarballs, from the packs
#   nu scripts/pack-ffi.nu verify <tarball>        link and run a counter against one
#
# What lands in target/pack/, per platform:
#
#   kui-ffi-<version>-<platform>/
#     include/kui.h
#     lib/        libkui_ffi.so / .dylib / kui_ffi.dll + kui_ffi.dll.lib   dynamic
#                 libkui_ffi.a / kui_ffi.lib                               static
#                 windows.0.5x.0.lib (Windows): windows-targets' import
#                 libraries, which link.txt names and no SDK has
#     link.txt    what a static link needs besides the archive (rustc's
#                 native-static-libs: -lm, -framework Metal, ws2_32.lib, ...)
#     BUILD.txt   the target, the features, the toolchain it was built with
#   kui-ffi-<version>-<platform>.tar.gz, and SHA256SUMS over every tarball there
#
# Where each is built: the machine's own platform natively with cargo, and
# the rest in a Docker image this script builds once (pinned below):
# cargo-zigbuild, as the release workflow builds the Linux Node prebuilds,
# and cargo-xwin, which the release workflow no longer uses (it builds
# Windows on a Windows runner):
#
#   linux-x64, linux-arm64   cargo-zigbuild against glibc 2.28, the floor
#                            the npm prebuilds have; ALSA (the audio backend
#                            links it) from Debian's multiarch packages
#   win32-x64                cargo-xwin: clang-cl and lld-link, with the
#                            MSVC CRT and Windows SDK it downloads
#   darwin-arm64             natively on a Mac only: Apple's SDK is not
#                            redistributable, so no container builds it
#
# On a Linux machine its own arch is that native build, against the
# machine's glibc rather than 2.28; BUILD.txt says the floor each .so has.
# `--zig` builds it through cargo-zigbuild at 2.28 instead, natively —
# what the release workflow does on its Linux runners, each building its
# own platform (.github/workflows/release.yml).
#
# The release ships two tarballs over every platform's pack, which
# `bundle` writes to target/pack/release/ with their SHA256SUMS:
#
#   kui-c-<version>/      include/kui.h
#   kui-odin-<version>/   kui/, the Odin package (packages/odin/kui)
#     and in both:        lib/<platform>/shared/  libkui_ffi.so / .dylib, or
#                                                 kui_ffi.dll + kui_ffi.dll.lib
#                         lib/<platform>/static/  libkui_ffi.a, or kui_ffi.lib
#                                                 and windows-targets' .libs,
#                                                 link.txt
#                         lib/<platform>/BUILD.txt, README.md, LICENSE
#
# A kind to a directory, so `-L lib/<platform>/static -lkui_ffi` is the
# static link and the same line with `shared` the dynamic one: a linker
# handed a directory holding both takes the shared library. `verify`
# builds examples/c/apps/counter.c (from the C tarball) or
# examples/odin/apps/counter.odin (from the Odin one) against each kind
# for this machine's platform, as a user outside this repository would,
# and runs it `--headless`; Linux and macOS, since Windows has no `cc`.
#
# Windows asks for --accept-msvc-license: the CRT and SDK xwin downloads are
# Microsoft's, under Microsoft's license, and building the platform means
# accepting it. The flag is that acceptance, made by whoever runs this.
#
# Both kinds come from one compile (`cargo rustc --crate-type
# cdylib,staticlib`), release, with DWARF dropped (strip = debuginfo):
# symbols stay, so the dynamic library still exports kui_* and the static
# one still links. The static archive is large - every object of the
# dependency tree plus std: 75 MB on macOS, 156 MB on Linux, 113 MB on
# Windows at 0.1.0-alpha.41, of which debug info is under a tenth, so it is
# not stripped. A linker keeps what is used: the Odin counter linked
# statically is 35 MB on macOS and 62 MB on Linux.

const ROOT = path self | path dirname | path dirname
const OUT = $ROOT | path join target pack

# The first two pinned like the release workflow's
# (.github/workflows/release.yml); cargo-xwin only here.
const CARGO_ZIGBUILD_VERSION = "0.23.3"
const ZIGLANG_VERSION = "0.16.0"
const CARGO_XWIN_VERSION = "0.23.1"

const PLATFORMS = [
    [platform target builder];
    [linux-x64 x86_64-unknown-linux-gnu zigbuild]
    [linux-arm64 aarch64-unknown-linux-gnu zigbuild]
    [darwin-arm64 aarch64-apple-darwin mac]
    [win32-x64 x86_64-pc-windows-msvc xwin]
]

def version [] {
    open ($ROOT | path join Cargo.toml) | get workspace.package.version
}

# The platform this machine is, in the table's spelling.
def host-platform [] {
    let os = match $nu.os-info.name { "macos" => "darwin", "windows" => "win32", $o => $o }
    let arch = match $nu.os-info.arch { "aarch64" => "arm64", "x86_64" => "x64", $a => $a }
    $"($os)-($arch)"
}

def main [
    --platforms: string # which to build, comma-separated; default every one this machine can
    --accept-msvc-license # build win32-x64, accepting Microsoft's license for the CRT and SDK
    --no-runner # leave out the windowed runner (kui_run, kui_run_with): a library for a host with its own window
    --zig # build this Linux machine's own platform through cargo-zigbuild, at glibc 2.28
] {
    let host = host-platform
    # `try` as well as `complete`: a docker that cannot be spawned at all
    # (Docker Desktop's CLI mount in a WSL distro without its integration
    # fails with EIO) is an error `complete` does not catch.
    let has_docker = (which docker | is-not-empty) and (try { (do { docker info } | complete).exit_code == 0 } catch { false })
    let wanted = if $platforms == null { $PLATFORMS | get platform } else { $platforms | split row "," | str trim }
    for p in $wanted {
        if $p not-in ($PLATFORMS | get platform) {
            error make { msg: $"unknown platform ($p); there is: ($PLATFORMS | get platform | str join ', ')" }
        }
    }

    # What each platform is built by here, or why it is not.
    let plan = $PLATFORMS | where platform in $wanted | each {|row|
        let why = if $row.platform == $host {
            null
        } else if $row.builder == "mac" {
            "only a Mac builds it (Apple's SDK is not redistributable)"
        } else if $row.builder == "xwin" and not $accept_msvc_license {
            "needs --accept-msvc-license (the CRT and SDK cargo-xwin downloads are Microsoft's)"
        } else if not $has_docker {
            "needs Docker, which is not running"
        } else {
            null
        }
        $row | insert how (if $row.platform == $host { "native" } else { "docker" }) | insert skip $why
    }
    for row in ($plan | where skip != null) {
        print -e $"skip ($row.platform): ($row.skip)"
    }
    let todo = $plan | where skip == null
    if ($todo | is-empty) {
        error make { msg: "nothing to build" }
    }

    let features = if $no_runner { [--no-default-features] } else { [] }
    let feature_note = if $no_runner { "no default features (no runner)" } else { "default features (with the runner)" }
    mkdir $OUT
    if ($todo | any {|r| $r.how == "docker" }) { build-image }

    let built = $todo | each {|row|
        print $"== ($row.platform) \(($row.target), ($row.how))"
        let note = if $row.how == "native" {
            build-native $row.target $features ($zig and ($row.builder == "zigbuild"))
        } else {
            build-docker $row.target $row.builder $features
        }
        stage ($row | update how {|r| if $r.how == "native" and $zig and $r.builder == "zigbuild" { "native, cargo-zigbuild" } else { $r.how } }) $note $feature_note
    }

    # Over every tarball here, not only this run's: a run of one platform
    # leaves the others' packs, and their sums, where they were.
    cd $OUT
    let sums = glob "kui-ffi-*.tar.gz" | each {|t| sha256sum-of ($t | path basename) } | sort
    $sums | str join "\n" | save -f SHA256SUMS
    # A width of its own: off a terminal (a pipe, tee, a CI log) nu reads
    # the width as 1 and prints "Couldn't fit table" in place of it.
    print ($built | wrap tarball | insert size {|r| ls $r.tarball | get 0.size } | table --width 100)
    print $"in ($OUT)"
}

# The compile both kinds come from; rustc's note on what a static link
# needs is on stderr, and is kept.
def rustc-args [target: string, features: list<string>] {
    [-p kui-ffi --lib --release --locked --target $target ...$features --crate-type "cdylib,staticlib" -- --print native-static-libs]
}

def build-native [target: string, features: list<string>, zig: bool] {
    cd $ROOT
    with-env { CARGO_PROFILE_RELEASE_STRIP: debuginfo } {
        # The `.2.28` suffix is the glibc zig links against; the output
        # still lands in target/<triple>/release.
        let r = if $zig {
            do { cargo-zigbuild rustc ...(rustc-args $"($target).2.28" $features) } | complete
        } else {
            do { cargo rustc ...(rustc-args $target $features) } | complete
        }
        if $r.exit_code != 0 {
            print -e $r.stderr
            error make { msg: $"the ($target) build failed" }
        }
        let registry = $env.CARGO_HOME? | default ($nu.home-dir | path join .cargo) | path join registry src
        let extra = import-lib-dirs $target | each {|d| glob ($registry | path join "*" $d | str replace -a '\' '/') } | flatten
        # Where cargo put it: CARGO_TARGET_DIR when the environment sets one.
        let target_dir = $env.CARGO_TARGET_DIR? | default ($ROOT | path join target)
        { dir: ($target_dir | path join $target release), log: $r.stderr, toolchain: (rustc --version), extra: $extra }
    }
}

# Where the import libraries a static Windows link names besides the
# system's come from: windows-targets' `windows.0.53.0.lib` and its
# siblings, which ship in the windows_<arch>_msvc crates (one per version
# Cargo.lock pins), not with Windows or its SDK. As registry-relative
# directories; nothing for other targets.
def import-lib-dirs [target: string] {
    if not ($target | str ends-with "windows-msvc") { return [] }
    let krate = $"windows_($target | split row '-' | first)_msvc"
    open --raw ($ROOT | path join Cargo.lock) | from toml | get package | where name == $krate | get version
        | each {|v| $"($krate)-($v)/lib/*.lib" }
}

# Every tool the container legs need, built once into an image whose tag
# names the versions, so a changed pin builds a new one.
# The Rust is the host's stable, so a newer toolchain on the host is a new
# image too: an image built once kept alpha.41's cargo until alpha.60, which
# refused a manifest the host's own cargo had long taken.
def image-tag [] {
    let rust = rustc --version | split row " " | get 1
    $"kui-pack-ffi:zb($CARGO_ZIGBUILD_VERSION)-zig($ZIGLANG_VERSION)-xwin($CARGO_XWIN_VERSION)-rust($rust)"
}

def build-image [] {
    let tag = image-tag
    if (do { docker image inspect $tag } | complete).exit_code == 0 { return }
    print $"building the ($tag) image \(once)"
    # ALSA for both Linux arches through multiarch (one is the image's own,
    # the other foreign, whichever arch Docker runs here); clang, lld and
    # llvm for cargo-xwin's clang-cl and lld-link.
    let dockerfile = $"FROM rust:1-bookworm
RUN dpkg --add-architecture amd64 && dpkg --add-architecture arm64 \\
 && apt-get update \\
 && apt-get install -y --no-install-recommends python3-pip pkg-config clang lld llvm \\
      libasound2-dev:amd64 libasound2-dev:arm64 \\
 && rm -rf /var/lib/apt/lists/*
RUN rustup toolchain install ((rustc --version | split row ' ' | get 1)) --profile minimal && rustup default ((rustc --version | split row ' ' | get 1))
RUN pip3 install --break-system-packages cargo-zigbuild==($CARGO_ZIGBUILD_VERSION) ziglang==($ZIGLANG_VERSION)
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-pc-windows-msvc
RUN cargo install cargo-xwin --locked --version ($CARGO_XWIN_VERSION)
"
    $dockerfile | docker build --pull -t $tag -
}

# A leg in the container: the checkout read-only, the target dir and the
# registry in named volumes (so a second run is incremental), and xwin's
# download of the SDK in a third.
def build-docker [target: string, builder: string, features: list<string>] {
    let arch_dir = match $target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu"
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu"
        _ => ""
    }
    # pkg-config reads the target arch's ALSA, not the image's.
    let pkg = if $arch_dir != "" {
        [-e PKG_CONFIG_ALLOW_CROSS=1 -e $"PKG_CONFIG_PATH=/usr/lib/($arch_dir)/pkgconfig" -e $"PKG_CONFIG_LIBDIR=/usr/lib/($arch_dir)/pkgconfig"]
    } else { [] }
    let cmd = match $builder {
        # The `.2.28` suffix is the glibc zig links against; the output
        # still lands in target/<triple>/release.
        "zigbuild" => [cargo-zigbuild rustc ...(rustc-args $"($target).2.28" $features)]
        "xwin" => [cargo xwin rustc ...(rustc-args $target $features)]
    }
    let r = do {
        (docker run --rm
            -v $"($ROOT):/src:ro"
            -v kui-pack-target:/target
            -v kui-pack-registry:/usr/local/cargo/registry
            -v kui-pack-xwin:/xwin
            -e CARGO_TARGET_DIR=/target -e CARGO_PROFILE_RELEASE_STRIP=debuginfo
            -e XWIN_CACHE_DIR=/xwin -e CARGO_BUILD_JOBS=4
            ...$pkg -w /src (image-tag) ...$cmd)
    } | complete
    if $r.exit_code != 0 {
        print -e $r.stderr
        error make { msg: $"the ($target) build failed in the container" }
    }
    # Out of the volume and into target/pack.
    let staging = $OUT | path join $".build-($target)"
    rm -rf $staging
    mkdir $staging
    let files = libraries $target | each {|f| $"/target/($target)/release/($f)" }
    let imports = import-lib-dirs $target | each {|d| $"/usr/local/cargo/registry/src/*/($d)" }
    (docker run --rm -v kui-pack-target:/target -v kui-pack-registry:/usr/local/cargo/registry
        -v $"($staging):/out" (image-tag)
        sh -c $"cp ($files | str join ' ') /out/ && mkdir /out/imports ($imports | each {|i| $' && cp ($i) /out/imports/' } | str join '')")
    let toolchain = docker run --rm (image-tag) rustc --version
    let extra = if ($imports | is-empty) { [] } else { glob ($staging | path join imports "*.lib" | str replace -a '\' '/') }
    { dir: $staging, log: $r.stderr, toolchain: $toolchain, extra: $extra }
}

# The files a build leaves for a target: the dynamic library (with its
# import library on Windows) and the static one.
def libraries [target: string] {
    if ($target | str contains "windows") {
        [kui_ffi.dll kui_ffi.dll.lib kui_ffi.lib]
    } else if ($target | str contains "apple") {
        [libkui_ffi.dylib libkui_ffi.a]
    } else {
        [libkui_ffi.so libkui_ffi.a]
    }
}

# target/pack/kui-ffi-<version>-<platform>/ and its tarball.
def stage [row: record, built: record, feature_note: string] {
    let name = $"kui-ffi-(version)-($row.platform)"
    let dir = $OUT | path join $name
    rm -rf $dir
    mkdir ($dir | path join include) ($dir | path join lib)
    # With LF line ends whatever the checkout has: a CRLF one (Git for
    # Windows' autocrlf) would otherwise ship a header that differs by host.
    open --raw ($ROOT | path join crates kui-ffi include kui.h) | str replace -a "\r\n" "\n" | save -f --raw ($dir | path join include kui.h)
    for f in (libraries $row.target) {
        let src = $built.dir | path join $f
        if not ($src | path exists) {
            error make { msg: $"the ($row.target) build left no ($f) in ($built.dir)" }
        }
        cp $src ($dir | path join lib)
    }
    # rustc's own line: "note: native-static-libs: -lgcc_s -lutil ...".
    let link = $built.log | lines | where {|l| $l | str contains "native-static-libs:" } | each {|l| $l | split row "native-static-libs:" | last | str trim } | get -o 0 | default ""
    let static = libraries $row.target | last
    # A library the link line names that no system has (windows-targets'
    # windows.0.53.0.lib) goes in lib/ beside the archive.
    let bundled = $link | split row " " | where {|l| $l =~ '^windows\.[\d.]+\.lib$' } | uniq
    for name in $bundled {
        let src = $built.extra | where {|f| ($f | path basename) == $name } | get -o 0
        if $src == null {
            error make { msg: $"the static link needs ($name), and no windows_*_msvc crate Cargo.lock pins has it" }
        }
        cp $src ($dir | path join lib)
    }
    let note = if ($bundled | is-empty) { "" } else { $" \(($bundled | str join ', ') are in lib/)" }
    $"# What linking lib/($static) needs besides the archive, as rustc's native-static-libs says it($note).\n($link)\n" | save -f ($dir | path join link.txt)
    [
        $"kui-ffi (version), ($row.platform)"
        $"target: ($row.target)"
        $"built: ($row.how)(if $row.how == 'native' { ', cargo' } else if $row.how == 'docker' { $', ($row.builder)' } else { '' })"
        $"features: ($feature_note)"
        $"toolchain: ($built.toolchain)"
        $"ABI: (open ($ROOT | path join crates kui-ffi include kui.h) | parse -r '#define KUI_ABI_VERSION (?<v>\d+)u' | get 0.v); a host checks kui_abi_version against it"
        ...(glibc-floor ($dir | path join lib libkui_ffi.so))
    ] | str join "\n" | save -f ($dir | path join BUILD.txt)
    if ($built.dir | str ends-with $".build-($row.target)") { rm -rf $built.dir }
    cd $OUT
    tar-gz $name
    $"($name).tar.gz"
}

# The newest glibc symbol version a Linux library needs, as a BUILD.txt
# line: 2.28 from zig, the build machine's own from a native build. Nothing
# for other platforms, or without an objdump to read it.
def glibc-floor [so: string] {
    if not ($so | path exists) or (which objdump | is-empty) { return [] }
    let versions = objdump -T $so | parse -r 'GLIBC_(?<v>[\d.]+)' | get v | uniq
    if ($versions | is-empty) { return [] }
    let newest = $versions | sort-by {|v| $v | split row "." | each { into int } } | last
    [$"glibc: ($newest) or newer \(the newest GLIBC_ symbol version libkui_ffi.so needs)"]
}

def sha256sum-of [file: string] {
    let sum = if (which sha256sum | is-not-empty) {
        sha256sum $file | split row " " | first
    } else {
        shasum -a 256 $file | split row " " | first
    }
    $"($sum)  ($file)"
}

# The release's two tarballs (see the top of this file), from the
# per-platform packs `main` left in target/pack/ — in the release
# workflow, the four its build legs uploaded.
def "main bundle" [
    --partial # bundle the platforms that are packed; without it all four must be
] {
    let v = version
    let all = $PLATFORMS | get platform
    let have = $all | where {|p| $OUT | path join $"kui-ffi-($v)-($p)" | path exists }
    let missing = $all | where {|p| $p not-in $have }
    if ($have | is-empty) {
        error make { msg: $"no kui-ffi-($v)-<platform> pack in ($OUT); run pack-ffi.nu first" }
    }
    if ($missing | is-not-empty) {
        if not $partial {
            error make { msg: $"no pack for ($missing | str join ', ') in ($OUT); pack them, or pass --partial" }
        }
        print -e $"bundling without ($missing | str join ', ')"
    }
    let rel = $OUT | path join release
    rm -rf $rel
    mkdir $rel
    let tarballs = [c odin] | each {|kind|
        let name = $"kui-($kind)-($v)"
        let dir = $rel | path join $name
        for p in $have {
            lay-out-libs ($OUT | path join $"kui-ffi-($v)-($p)") ($dir | path join lib $p)
        }
        if $kind == "c" {
            mkdir ($dir | path join include)
            cp ($OUT | path join $"kui-ffi-($v)-($have | first)" include kui.h) ($dir | path join include)
        } else {
            cp -r ($ROOT | path join packages odin kui) $dir
        }
        cp ($ROOT | path join LICENSE) $dir
        readme $kind $v $have | save -f ($dir | path join README.md)
        cd $rel
        tar-gz $name
        rm -rf $name
        $"($name).tar.gz"
    }
    cd $rel
    $tarballs | each {|t| sha256sum-of $t } | str join "\n" | save -f SHA256SUMS
    print ($tarballs | wrap tarball | insert size {|r| ls $r.tarball | get 0.size } | table --width 100)
    print $"in ($rel)"
}

# One platform's pack, split by kind: shared/ the library a program loads
# (with its import library on Windows), static/ the archive with
# link.txt and what link.txt names that no system has.
def lay-out-libs [pack: string, dst: string] {
    let shared = $dst | path join shared
    let static = $dst | path join static
    mkdir $shared $static
    for f in (ls ($pack | path join lib) | get name) {
        let base = $f | path basename
        let to = if $base in [libkui_ffi.so libkui_ffi.dylib kui_ffi.dll kui_ffi.dll.lib] { $shared } else { $static }
        cp $f $to
    }
    cp ($pack | path join link.txt) $static
    cp ($pack | path join BUILD.txt) $dst
}

# README.md of a release tarball: what is in it and the link lines, for
# the platforms it holds.
def readme [kind: string, v: string, platforms: list<string>] {
    let what = if $kind == "c" {
        "the C library: `include/kui.h`, and `libkui_ffi` for each platform"
    } else {
        "the Odin binding: the `kui` package, and `libkui_ffi` for each platform it links"
    }
    let use = if $kind == "c" {
        $"```sh
# dynamic: found at run time through the rpath \(on Windows, kui_ffi.dll beside the program)
cc -std=c11 -I include app.c -L lib/<platform>/shared -lkui_ffi -Wl,-rpath,'$ORIGIN/lib/<platform>/shared' -o app
# static: link.txt's last line is what the archive needs besides itself
cc -std=c11 -I include app.c -L lib/<platform>/static -lkui_ffi $\(tail -1 lib/<platform>/static/link.txt) -o app
```

On macOS the rpath is `@executable_path/...` rather than `$ORIGIN/...`. With
MSVC: `cl /std:c11 /I include app.c /link /LIBPATH:lib\\win32-x64\\shared kui_ffi.dll.lib`,
and for the static one `kui_ffi.lib`, link.txt's last line and `/NODEFAULTLIB:libcmt`
from `lib\\win32-x64\\static`."
    } else {
        $"Import the package as a collection \(`-collection:kui=<this directory>` and
`import \"kui:kui\"`; Odin takes no absolute import path), or by a path
relative to your source, and point the linker at a platform's kind:

```sh
# dynamic
odin build app -extra-linker-flags:\"-Llib/<platform>/shared -Wl,-rpath,<absolute>/lib/<platform>/shared\"
# static: link.txt's last line is what the archive needs besides itself
odin build app -extra-linker-flags:\"-Llib/<platform>/static $\(tail -1 lib/<platform>/static/link.txt)\"
```

On Windows the dynamic link is the default \(`kui_ffi.dll.lib`; copy
`kui_ffi.dll` beside the program); the static one is
`-define:KUI_LIB=system:kui_ffi.lib`, link.txt's last line and
`/NODEFAULTLIB:libcmt`, from `lib\\win32-x64\\static`. The package's own
documentation is `kui/README.md` in the repository, packages/odin/README.md."
    }
    $"# kui ($kind) ($v)

This is ($what), as kui's release builds it:
default features, so `kui_run` \(the windowed runner) is in. Each
platform's `BUILD.txt` names the target, the toolchain and, on Linux, the
glibc it needs \(2.28 or newer).

Platforms: ($platforms | str join ', ').

($use)

kui.h's `KUI_ABI_VERSION` is what this library answers from
`kui_abi_version\(\)`; a program built against another header refuses it.
"
}

# Links the counter against a release tarball for one platform, the
# shared kind and then the static one, and runs each `--headless`: the
# check that what the release ships links and works outside this
# repository. C needs `cc` (or $env.CC); Odin needs odin on PATH or ODIN.
def "main verify" [
    tarball: string # a kui-c-<version>.tar.gz or kui-odin-<version>.tar.gz
    --platform: string # whose libraries to link; default: this machine's
] {
    let p = $platform | default (host-platform)
    if ($p | str starts-with "win32") {
        error make { msg: "verify links with cc or odin's own linker line, on Linux and macOS; Windows is not checked" }
    }
    let tarball = $tarball | path expand
    let name = $tarball | path basename | str replace ".tar.gz" ""
    let kind = if ($name | str starts-with "kui-c-") { "c" } else if ($name | str starts-with "kui-odin-") { "odin" } else {
        error make { msg: $"($name) is neither kui-c-<version> nor kui-odin-<version>" }
    }
    let work = $OUT | path join $"verify-($kind)-($p)"
    rm -rf $work
    mkdir $work
    tar -xzf $tarball -C $work
    let root = $work | path join $name
    let lib = $root | path join lib $p
    if not ($lib | path exists) {
        error make { msg: $"($name) has no libraries for ($p)" }
    }
    let link_line = open ($lib | path join static link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default "" | split row " " | where {|a| $a != "" }
    let rpath = $lib | path join shared
    let results = [shared static] | each {|how|
        let exe = $work | path join $"counter-($how)"
        if $kind == "c" {
            mkdir ($work | path join src apps)
            cp ($ROOT | path join examples c common.h) ($work | path join src)
            cp ($ROOT | path join examples c apps counter.c) ($work | path join src apps)
            let cc = $env.CC? | default "cc"
            let extra = if $how == "shared" { [$"-Wl,-rpath,($rpath)"] } else { $link_line }
            run-or-fail $"cc, ($how)" $cc -std=c11 -I ($root | path join include) ($work | path join src apps counter.c) -L ($lib | path join $how) -lkui_ffi ...$extra -o $exe
        } else {
            let src = $work | path join src counter.odin
            mkdir ($work | path join src)
            open --raw ($ROOT | path join examples odin apps counter.odin)
                | str replace 'import kui "../../../packages/odin/kui"' 'import kui "kuipack:kui"'
                | save -f --raw $src
            let odin = $env.ODIN? | default "odin"
            let flags = if $how == "shared" { [$"-L($rpath)" $"-Wl,-rpath,($rpath)"] } else { [$"-L($lib | path join static)" ...$link_line] }
            # As a collection, which is how the tarball's README says to
            # import it: Odin takes no absolute import path.
            run-or-fail $"odin, ($how)" $odin build $src -file $"-collection:kuipack=($root)" $"-out:($exe)" $"-extra-linker-flags:($flags | str join ' ')"
        }
        let r = do { ^$exe --headless } | complete
        if $r.exit_code != 0 {
            print -e $r.stdout $r.stderr
            error make { msg: $"the ($kind) counter linked ($how) exited ($r.exit_code)" }
        }
        { kind: $kind, platform: $p, link: $how, size: (ls $exe | get 0.size), run: ($r.stdout | lines | last) }
    }
    print ($results | table --width 100)
}

def --wrapped run-or-fail [what: string, cmd: string, ...args] {
    let r = do { ^$cmd ...$args } | complete
    if $r.exit_code != 0 {
        print -e $r.stdout $r.stderr
        error make { msg: $"($what) failed: ($cmd) ($args | str join ' ')" }
    }
}

# <dir>.tar.gz, with nothing in it but the files: macOS's tar adds its
# extended attributes (com.apple.provenance on every file a download
# touched) and AppleDouble `._` files, and GNU tar warns about each one it
# meets on the way out. `--no-xattrs` is bsdtar's and GNU tar's both.
def tar-gz [dir: string] {
    with-env { COPYFILE_DISABLE: "1" } {
        tar --no-xattrs -czf $"($dir).tar.gz" $dir
    }
}
