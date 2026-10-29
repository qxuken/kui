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
#   nu scripts/pack-ffi.nu bundle [--partial]      the release's C and Odin archives, from the packs
#   nu scripts/pack-ffi.nu verify <tarball>        link and run a counter against one
#
# What lands in target/pack/, per platform:
#
#   kui-ffi-<version>-<platform>/
#     include/    HEADERS below: kui.h, kui_glfw.h and kui_sdl3.h (header-only
#                 input helpers) and kui_wgpu.h (header-only, kui drawn on a
#                 WebGPU device)
#     lib/        libkui_ffi.so / .dylib / kui_ffi.dll + kui_ffi.dll.lib   dynamic
#                 libkui_ffi.a / kui_ffi.lib                               static
#                 windows.0.5x.0.lib (Windows): windows-targets' import
#                 libraries, which link.txt names and no SDK has
#     link.txt    what a static link needs besides the archive (rustc's
#                 native-static-libs: -lm, -framework Metal, ws2_32.lib, ...)
#     BUILD.txt   the target, the features, the toolchain it was built with
#   kui-ffi-<version>-<platform>.tar.gz, and SHA256SUMS over this version's
#   (`-no-runner` after the platform for a --no-runner pack)
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
# The release ships a C archive and an Odin one for each platform, and the
# two again over all four, which `bundle` writes to target/pack/release/
# with their SHA256SUMS:
#
#   kui-c-<version>-<platform>.tar.gz     one platform's, a .zip for
#   kui-odin-<version>-<platform>.tar.gz  win32-x64 as the runner's is
#   kui-c-<version>.tar.gz                all four platforms, for whoever
#   kui-odin-<version>.tar.gz             vendors every target
#
# One layout for both, the per-platform archive holding a single
# lib/<platform>/, so a path the README gives is the same in either:
#
#   kui-c-<version>[-<platform>]/
#                         include/ (the HEADERS), lib/<platform>/ (and
#                         core/ in it, the library without the runner, as
#                         its own shared/ static/ BUILD.txt; and pkgconfig/
#                         with kui, kui-static, kui-core, kui-core-static
#                         .pc files, but on Windows)
#   kui-odin-<version>[-<platform>]/
#                         kui/, the Odin package (packages/odin/kui), with
#                         the same libraries under kui/c/lib/<platform>/
#     where <platform>/ is: shared/  libkui_ffi.so / .dylib, or
#                                    kui_ffi.dll + kui_ffi.dll.lib
#                           static/  libkui_ffi.a, or kui_ffi.lib and
#                                    windows-targets' .libs, link.txt
#                           BUILD.txt
#     and in both:        README.md, LICENSE
#
# Until alpha.71 the release shipped the all-platform two alone: 265 MB
# each to download and 978 MB unpacked, for a host that links one 12 MB
# library (backlog F268). darwin-arm64's own is 55 MB, 175 MB unpacked,
# at alpha.71; Linux's are larger (their static archives are). The
# all-platform Odin archive is the one way to vendor the package for
# several targets, since each archive's kui_c.odin names only the
# platforms it carries; the C one is kept beside it for the same reader.
#
# The static archives stay in each platform's archive, though they are
# most of its size: kui_c.odin links static by default, so the Odin
# package's promise (an import, and nothing beside the program) is the
# static library, and an archive without it would build nothing until a
# second one were unpacked over it. A host beside another Rust library
# links shared and carries the static one unused: 42 MB of darwin-arm64's
# 55 at alpha.71, where it was some 200 MB of every platform's at once.
#
# A kind to a directory, so `-L lib/<platform>/static -lkui_ffi` is the
# static link and the same line with `shared` the dynamic one: a linker
# handed a directory holding both takes the shared library. The Odin
# pack's kui_c.odin names its own copy, static unless KUI_SHARED. `verify`
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
const ZIGLANG_VERSION = "0.17.0"
const CARGO_XWIN_VERSION = "0.23.1"

# The headers every pack carries in include/, from crates/kui-ffi/include/:
# kui.h, and kui_glfw.h and kui_sdl3.h, header-only, a window library's
# keys and pointer as kui's (backlogs F239, F265), and kui_wgpu.h, header-only, kui's frame
# drawn on a WebGPU device the host owns (backlog F263). `bundle` copies a
# pack's include/ whole, so a header added here reaches every archive.
const HEADERS = [kui.h kui_glfw.h kui_sdl3.h kui_wgpu.h]

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
        stage ($row | update how {|r| if $r.how == "native" and $zig and $r.builder == "zigbuild" { "native, cargo-zigbuild" } else { $r.how } }) $note $feature_note $no_runner
    }

    # Over every tarball of this version here, not only this run's: a run
    # of one platform leaves the others' packs, and their sums, where they
    # were. An older version's tarballs stay on disk and out of the sums.
    cd $OUT
    let sums = glob $"kui-ffi-(version)-*.tar.gz" | each {|t| sha256sum-of ($t | path basename) } | sort
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
# download of the SDK in a third. The target volume is the checkout's own
# (`target-volume`): cargo keys a path package by its path, and every
# checkout is /src in here, so a shared one let a worktree's pack find
# another's newer build fresh and ship its library beside this kui.h.
# The container target dir of this checkout, named after its path.
def target-volume [] {
    $"kui-pack-target-($ROOT | hash sha256 | str substring 0..<10)"
}

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
            -v $"(target-volume):/target"
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
    # The import libraries only where the build fetched them (the runner's
    # dependencies are what download windows-targets' crates): `stage`
    # says so if the link line wants one that is missing. And the copies
    # handed back to whoever runs this, since a rootful Docker writes them
    # as root and the next `rm -rf` of the staging dir would fail.
    let owner = if $nu.os-info.name == "windows" { "" } else { $" && chown -R (^id -u | str trim):(^id -g | str trim) /out" }
    (docker run --rm -v $"(target-volume):/target" -v kui-pack-registry:/usr/local/cargo/registry
        -v $"($staging):/out" (image-tag)
        sh -c $"cp ($files | str join ' ') /out/ && mkdir /out/imports ($imports | each {|i| $' && { cp ($i) /out/imports/ 2>/dev/null || true; }' } | str join '')($owner)")
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
def stage [row: record, built: record, feature_note: string, no_runner: bool] {
    # A pack without the runner is another library, so another name: it
    # used to overwrite the full pack of its platform.
    let name = $"kui-ffi-(version)-($row.platform)(if $no_runner { '-no-runner' } else { '' })"
    let dir = $OUT | path join $name
    rm -rf $dir
    mkdir ($dir | path join include) ($dir | path join lib)
    # With LF line ends whatever the checkout has: a CRLF one (Git for
    # Windows' autocrlf) would otherwise ship a header that differs by host.
    for h in $HEADERS {
        open --raw ($ROOT | path join crates kui-ffi include $h) | str replace -a "\r\n" "\n" | save -f --raw ($dir | path join include $h)
    }
    for f in (libraries $row.target) {
        let src = $built.dir | path join $f
        if not ($src | path exists) {
            error make { msg: $"the ($row.target) build left no ($f) in ($built.dir)" }
        }
        cp $src ($dir | path join lib)
    }
    # rustc's own line: "note: native-static-libs: -lgcc_s -lutil ...".
    # Coloured where cargo colours (the release workflow sets
    # CARGO_TERM_COLOR=always), so its last flag carried the reset code:
    # alpha.60's macOS verify could not find `-lm\e[0m`.
    let link = $built.log | ansi strip | lines | where {|l| $l | str contains "native-static-libs:" } | each {|l| $l | split row "native-static-libs:" | last | str trim } | get -o 0 | default ""
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

# The release's C and Odin archives (see the top of this file), from the
# per-platform packs `main` left in target/pack/ — in the release
# workflow, the four its build legs uploaded: one of each per platform,
# and the all-platform two once all four are packed.
def "main bundle" [
    --partial # bundle the platforms that are packed, each on its own; without it all four must be
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
    # The library without the runner, beside each platform's (backlog
    # F238): every platform's in a release, as the runner one is.
    let core = $have | where {|p| $OUT | path join $"kui-ffi-($v)-($p)-no-runner" | path exists }
    let core_missing = $have | where {|p| $p not-in $core }
    if ($core_missing | is-not-empty) {
        if not $partial {
            error make { msg: $"no kui-ffi-($v)-<platform>-no-runner pack for ($core_missing | str join ', ') in ($OUT); pack them with --no-runner, or pass --partial" }
        }
        print -e $"bundling without the no-runner library for ($core_missing | str join ', ')"
    }
    let rel = $OUT | path join release
    rm -rf $rel
    mkdir $rel
    # Each platform's own, then all four together: the all-platform name
    # means all four, so a --partial run with some missing writes none.
    let single = $have | each {|p|
        [c odin] | each {|kind| archive-one $rel $kind $v $"kui-($kind)-($v)-($p)" [$p] ($core | where {|c| $c == $p }) }
    } | flatten
    let whole = if ($missing | is-empty) {
        [c odin] | each {|kind| archive-one $rel $kind $v $"kui-($kind)-($v)" $have $core }
    } else {
        print -e "no all-platform archives: they want all four platforms packed"
        []
    }
    let archives = $single ++ $whole
    cd $rel
    $archives | each {|t| sha256sum-of $t } | str join "\n" | save -f SHA256SUMS
    print ($archives | wrap archive | insert size {|r| ls $r.archive | get 0.size } | table --width 100)
    print $"in ($rel)"
}

# The archive of one kind (c or odin) over `platforms`, named `name`, in
# `rel`: laid out in a directory of that name, then a .tar.gz of it, or a
# .zip for an archive of win32-x64 alone, as the runner's is
# (scripts/pack-runner.nu's asset-name). Its file name.
def archive-one [rel: string, kind: string, v: string, name: string, platforms: list<string>, core: list<string>] {
    let dir = $rel | path join $name
    let pack_of = {|p| $OUT | path join $"kui-ffi-($v)-($p)" }
    let core_of = {|p| $OUT | path join $"kui-ffi-($v)-($p)-no-runner" }
    if $kind == "c" {
        for p in $platforms {
            lay-out-libs (do $pack_of $p) ($dir | path join lib $p)
        }
        for p in $core {
            lay-out-libs (do $core_of $p) ($dir | path join lib $p core)
        }
        for p in ($platforms | where {|p| not ($p | str starts-with "win32") }) {
            write-pkg-config ($dir | path join lib $p) $v ($p in $core)
        }
        # The pack's include/ whole: every header it carries (HEADERS).
        cp -r (do $pack_of ($platforms | first) | path join include) $dir
    } else {
        # The libraries inside the package, beside the file that links
        # them, as vendor:raylib carries its own: `import "kui"` is the
        # whole of using the pack, and the package travels with them.
        mkdir $dir
        cp -r ($ROOT | path join packages odin kui) $dir
        for p in $platforms {
            lay-out-libs (do $pack_of $p) ($dir | path join kui c lib $p)
        }
        for p in $core {
            lay-out-libs (do $core_of $p) ($dir | path join kui c lib $p core)
        }
        link-the-pack ($dir | path join kui c) $platforms $core
    }
    cp ($ROOT | path join LICENSE) $dir
    readme $kind $v $platforms | save -f ($dir | path join README.md)
    cd $rel
    let file = if $platforms == [win32-x64] {
        zip-dir $name
        $"($name).zip"
    } else {
        tar-gz $name
        $"($name).tar.gz"
    }
    rm -rf $name
    $file
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

# What a static link of one library needs, as Odin's foreign import names
# it: rustc's line from its link.txt, `-framework X` a framework, `-lX` a
# system library, a `.lib` the pack carries by its path (`lib`, relative
# to kui_c.odin) and any other by name; a `/flag` rides in
# extra_linker_flags on Windows.
def odin-static-link [static_dir: string, lib: string, windows: bool] {
    let link = open ($static_dir | path join link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default "" | split row " " | where {|t| $t != "" }
    mut imports = []
    mut flags = ["/NODEFAULTLIB:libcmt"]
    mut i = 0
    while $i < ($link | length) {
        let t = $link | get $i
        if $t == "-framework" {
            $imports = ($imports | append $"system:($link | get ($i + 1)).framework")
            $i = $i + 2
            continue
        }
        if ($t | str starts-with "-l") {
            $imports = ($imports | append $"system:($t | str substring 2..)")
        } else if ($t | str ends-with ".lib") {
            if ($static_dir | path join $t | path exists) {
                $imports = ($imports | append $"($lib)/static/($t)")
            } else {
                $imports = ($imports | append $"system:($t)")
            }
        } else if $windows and ($t | str starts-with "/") {
            $flags = ($flags | append $t)
        }
        $i = $i + 1
    }
    let archive = $"($lib)/static/(if $windows { 'kui_ffi.lib' } else { 'libkui_ffi.a' })"
    { imports: ([$archive] | append ($imports | uniq)), flags: ($flags | uniq) }
}

# lib/<platform>/pkgconfig/ in the C pack: kui.pc and kui-static.pc, and
# kui-core.pc and kui-core-static.pc for the library without the runner,
# each finding its library from where the file is (${pcfiledir}), so the
# tarball works wherever it is unpacked. A shared one carries its rpath; a
# static one the platform's link.txt. Not for Windows, whose MSVC reads no
# pkg-config.
def write-pkg-config [lib_dir: string, v: string, core: bool] {
    let pc = $lib_dir | path join pkgconfig
    mkdir $pc
    let variants = [
        [name sub what];
        [kui "" "kui's C library, with the windowed runner"]
    ] | append (if $core { [[name sub what]; [kui-core "core/" "kui's C library without the runner (no kui_run, wgpu, winit or audio device)"]] } else { [] })
    for var in $variants {
        let link = open --raw ($lib_dir | path join $"($var.sub)static" link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default ""
        for kind in [shared static] {
            let name = if $kind == "shared" { $var.name } else { $"($var.name)-static" }
            let libs = if $kind == "shared" {
                "-L${libdir} -lkui_ffi -Wl,-rpath,${libdir}"
            } else {
                $"-L${libdir} -lkui_ffi ($link)" | str trim
            }
            $"prefix=${pcfiledir}/../../..
includedir=${prefix}/include
libdir=${pcfiledir}/../($var.sub)($kind)

Name: ($name)
Description: ($var.what), linked ($kind)
Version: ($v)
Cflags: -I${includedir}
Libs: ($libs)
" | save -f ($pc | path join $"($name).pc")
        }
    }
}

# The Odin pack's link block: kui_c.odin's own, between its markers,
# replaced by one that names the libraries under kui/c/lib/ for each
# platform packed, with what each platform's link.txt says a static link
# needs. Static unless -define:KUI_SHARED=true; the library without the
# runner (lib/<platform>/core/) with -define:KUI_CORE=true, for the
# platforms in `core`.
def link-the-pack [c_dir: string, platforms: list<string>, core: list<string>] {
    let file = $c_dir | path join kui_c.odin
    let text = open --raw $file
    let start = "// -- link:"
    let end = "// -- end link"
    let a = $text | str index-of $start
    let b = $text | str index-of $end
    if $a < 0 or $b < 0 {
        error make { msg: $"($file) has no link markers; regenerate it with odin.nu gen" }
    }
    let when_of = {
        "linux-x64": "ODIN_OS == .Linux && ODIN_ARCH == .amd64"
        "linux-arm64": "ODIN_OS == .Linux && ODIN_ARCH == .arm64"
        "darwin-arm64": "ODIN_OS == .Darwin && ODIN_ARCH == .arm64"
        "win32-x64": "ODIN_OS == .Windows && ODIN_ARCH == .amd64"
    }
    let quoted = {|xs: list<string>| $xs | each {|x| $"\t\t\t\"($x)\"," } | str join "\n" }
    let branches = $platforms | each {|p|
        let windows = $p | str starts-with "win32"
        let shared_of = {|lib: string| if $windows { [$"($lib)/shared/kui_ffi.dll.lib"] } else if ($p | str starts-with "darwin") { [$"($lib)/shared/libkui_ffi.dylib"] } else { [$"($lib)/shared/libkui_ffi.so"] } }
        let attr_of = {|flags: list<string>| if $windows { $"\t\t@\(extra_linker_flags=\"($flags | str join ' ')\")\n" } else { "" } }
        let lib = $"lib/($p)"
        let full = odin-static-link ($c_dir | path join lib $p static) $lib $windows
        # The core library's branches, or an assert that names the
        # platform this pack has none for.
        let core_branches = if $p in $core {
            let clib = $"lib/($p)/core"
            let cs = odin-static-link ($c_dir | path join lib $p core static) $clib $windows
            $"\t} else when KUI_CORE && KUI_SHARED {
\t\tforeign import lib {
(do $quoted (do $shared_of $clib))
\t\t}
\t} else when KUI_CORE {
(do $attr_of $cs.flags)\t\tforeign import lib {
(do $quoted $cs.imports)
\t\t}
"
        } else {
            $"\t} else when KUI_CORE {
\t\t#assert\(!KUI_CORE, \"this pack has no library without the runner for ($p)\")
\t\tforeign import lib {
(do $quoted (do $shared_of $lib))
\t\t}
"
        }
        # A plugin reaches a branch on Windows only, where it imports from
        # the host's kui_ffi.dll as a program does: the shared library,
        # never a private static copy of kui.
        $"} else when ($when_of | get $p) {
\twhen KUI_PLUGIN {
\t\tforeign import lib {
(do $quoted (do $shared_of $lib))
\t\t}
($core_branches)\t} else when KUI_SHARED {
\t\tforeign import lib {
(do $quoted (do $shared_of $lib))
\t\t}
\t} else {
(do $attr_of $full.flags)\t\tforeign import lib {
(do $quoted $full.imports)
\t\t}
\t}
"
    }
    let block = $"// -- link: this pack's own, written by scripts/pack-ffi.nu's bundle. It
// links libkui_ffi from lib/<platform>/ beside this file: the static
// library by default, so a program needs nothing beside it, or with
// -define:KUI_SHARED=true the shared one, which the program then finds
// at run time \(an rpath, or the library beside it); a Windows plugin
// \(-define:KUI_PLUGIN=true) always links the shared one. With
// -define:KUI_CORE=true either kind comes from lib/<platform>/core/, the
// library without the runner: no kui_run, no wgpu, no winit, no audio
// device, for a host that brings its own window. A platform the pack
// does not carry falls back to KUI_LIB, as the repository's file does.
KUI_SHARED :: #config\(KUI_SHARED, false)
KUI_CORE :: #config\(KUI_CORE, false)
KUI_LIB :: #config\(KUI_LIB, "system:kui_ffi.dll.lib" when ODIN_OS == .Windows else "system:kui_ffi")
KUI_PLUGIN :: #config\(KUI_PLUGIN, false)

when KUI_PLUGIN && ODIN_OS == .Darwin {
\tforeign import lib "system:System"
} else when KUI_PLUGIN && ODIN_OS != .Windows {
\tforeign import lib "system:c"
($branches | str join '')} else {
\tforeign import lib { KUI_LIB }
}
"
    let out = ($text | str substring 0..<$a) + $block + ($text | str substring $b..)
    $out | save -f --raw $file
}

# README.md of a release archive: what is in it and the link lines, for
# the platforms it holds; in one platform's, its paths spelled out.
def readme [kind: string, v: string, platforms: list<string>] {
    let text = readme-text $kind $v $platforms
    # Every path to the libraries is `lib/<platform>`; the archives' own
    # names keep their `<platform>`.
    if ($platforms | length) == 1 { $text | str replace -a "lib/<platform>" $"lib/($platforms | first)" } else { $text }
}

def readme-text [kind: string, v: string, platforms: list<string>] {
    let for_which = if ($platforms | length) == 1 { $"for ($platforms | first)" } else { "for each platform" }
    let what = if $kind == "c" {
        $"the C library: `include/kui.h`, and `libkui_ffi` ($for_which); `include/kui_glfw.h` and `include/kui_sdl3.h` forward GLFW's or SDL3's keys and pointer, and `include/kui_wgpu.h` draws kui's frame on a WebGPU device of your own, all header-only"
    } else {
        $"the Odin binding: the `kui` package, and `libkui_ffi` ($for_which) it links; `kui/glfw` and `kui/sdl3` forward vendor:glfw's or vendor:sdl3's keys and pointer, and `kui/wgpu` draws kui's frame on a vendor:wgpu device of your own"
    }
    let use = if $kind == "c" {
        $"With pkg-config \(not on Windows), point it at the platform's files and
take what it says:

```sh
export PKG_CONFIG_PATH=$PWD/lib/<platform>/pkgconfig
cc -std=c11 app.c $\(pkg-config --cflags --libs kui) -o app          # shared, with its rpath
cc -std=c11 app.c $\(pkg-config --cflags --libs kui-static) -o app   # static
# kui-core and kui-core-static: the library without the runner
```

By hand:

```sh
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
        $"Copy `kui/` into your project, or point a collection at this directory,
and import it — `import kui \"kui:kui\"` with `-collection:kui=<this
directory>`, or by a path relative to your source; Odin takes no absolute
import path. That is all: `kui/c/kui_c.odin` links libkui_ffi from
`kui/c/lib/<platform>/` itself, as vendor:raylib links its own, the
static library by default, so the program needs nothing beside it:

```sh
odin build app -collection:kui=<this directory>
# the shared library instead, found at run time through an rpath \(on
# Windows, copy kui/c/lib/win32-x64/shared/kui_ffi.dll beside the program)
odin build app -collection:kui=<this directory> -define:KUI_SHARED=true \\
  -extra-linker-flags:\"-Wl,-rpath,<absolute>/kui/c/lib/<platform>/shared\"
```

`-define:KUI_LIB=<name>` links a library of your own instead, on a
platform the pack does not carry. The package's own documentation is
packages/odin/README.md in the repository."
    }
    $"# kui ($kind) ($v)

This is ($what), as kui's release builds it:
default features, so `kui_run` \(the windowed runner) is in. Each
platform's `BUILD.txt` names the target, the toolchain and, on Linux, the
glibc it needs \(2.28 or newer).

Platforms: ($platforms | str join ', ').

Each release carries this archive for each platform on its own,
`kui-($kind)-($v)-<platform>.tar.gz` \(`.zip` for win32-x64), and all four in
`kui-($kind)-($v).tar.gz`, for a project that vendors every target. The
layout is one: each platform's files are under its own `lib/` directory
in either.

($use)

## Without the runner

`lib/<platform>/core/` \(`kui/c/lib/<platform>/core/` in the Odin pack)
holds the same library built without the runner: no `kui_run`, no wgpu,
no winit, no audio device - under half the size, for a host that brings
its own window and draws `kui_draw_data` itself. Its `shared/` and
`static/` link as the others do, with `core/` in the path; in Odin,
`-define:KUI_CORE=true` picks it. Everything of kui.h but the runner's
functions is in it.

## Beside another Rust library

Every Rust static library carries its own copy of Rust's standard
library, and this one also carries wgpu and winit for `kui_run`. Linked
into one program with another \(wgpu-native's `libwgpu_native.a`, say), the
linker stops at the symbols both define: `rust_eh_personality`, and on
macOS `RawWindowMetalLayer`'s classes. Link the shared library instead;
each keeps its copies to itself. The library without the runner leaves
wgpu and winit out, and with them `RawWindowMetalLayer`, but not the
standard library, so beside another Rust archive the shared kind is still
the one.

kui.h's `KUI_ABI_VERSION` is what this library answers from
`kui_abi_version\(\)`; a program built against another header refuses it.
"
}

# Links the counter against a release tarball for one platform, the
# shared kind and then the static one, and the host loop against the
# library without the runner, both kinds, and runs each `--headless`: the
# check that what the release ships links and works outside this
# repository. C needs `cc` (or $env.CC); Odin needs odin on PATH or ODIN.
def "main verify" [
    tarball: string # a kui-c-<version>[-<platform>].tar.gz or kui-odin-<version>[-<platform>].tar.gz
    --platform: string # whose libraries to link; default: this machine's
] {
    let p = $platform | default (host-platform)
    if ($p | str starts-with "win32") or ($tarball | str ends-with ".zip") {
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
    # The Odin pack carries its libraries inside the package.
    let lib = if $kind == "odin" { $root | path join kui c lib $p } else { $root | path join lib $p }
    if not ($lib | path exists) {
        error make { msg: $"($name) has no libraries for ($p)" }
    }
    # The counter against the runner's library, and the host loop - which
    # calls nothing of the runner - against the library without it
    # (lib/<platform>/core/, backlog F238).
    let kinds = [
        {how: shared, dir: ($lib | path join shared), core: false, prog: counter, sub: apps}
        {how: static, dir: ($lib | path join static), core: false, prog: counter, sub: apps}
        {how: "core shared", dir: ($lib | path join core shared), core: true, prog: host_loop, sub: tools}
        {how: "core static", dir: ($lib | path join core static), core: true, prog: host_loop, sub: tools}
    ]
    if not ($lib | path join core | path exists) {
        error make { msg: $"($name) has no library without the runner for ($p) \(lib/($p)/core)" }
    }
    let results = $kinds | each {|k|
        let exe = $work | path join $"($k.prog)-($k.how | str replace ' ' '-')"
        let shared = $k.how | str ends-with "shared"
        if $kind == "c" {
            mkdir ($work | path join src $k.sub)
            cp ($ROOT | path join examples c common.h) ($work | path join src)
            cp ($ROOT | path join examples c $k.sub $"($k.prog).c") ($work | path join src $k.sub)
            let cc = $env.CC? | default "cc"
            let src = $work | path join src $k.sub $"($k.prog).c"
            if (which pkg-config | is-not-empty) {
                # Through the pack's own .pc files, as its README says.
                let pc = $"(if $k.core { 'kui-core' } else { 'kui' })(if $shared { '' } else { '-static' })"
                let flags = with-env { PKG_CONFIG_PATH: ($lib | path join pkgconfig) } {
                    run-or-fail $"pkg-config ($pc)" pkg-config --cflags --libs $pc
                    ^pkg-config --cflags --libs $pc | str trim | split row " " | where {|a| $a != "" }
                }
                run-or-fail $"cc, ($k.how), pkg-config ($pc)" $cc -std=c11 $src ...$flags -o $exe
            } else {
                let extra = if $shared { [$"-Wl,-rpath,($k.dir)"] } else {
                    open --raw ($k.dir | path join link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default "" | split row " " | where {|a| $a != "" }
                }
                run-or-fail $"cc, ($k.how)" $cc -std=c11 -I ($root | path join include) $src -L $k.dir -lkui_ffi ...$extra -o $exe
            }
        } else {
            let src = $work | path join src $"($k.prog).odin"
            mkdir ($work | path join src)
            open --raw ($ROOT | path join examples odin $k.sub $"($k.prog).odin")
                | str replace 'import kui "../../../packages/odin/kui"' 'import kui "kuipack:kui"'
                | save -f --raw $src
            let odin = $env.ODIN? | default "odin"
            # Nothing but the import: the pack's kui_c.odin names its own
            # libraries, as vendor:raylib does. The shared build is told to
            # be one, and where to find the library when it runs. Imported
            # as a collection, as the tarball's README says: Odin takes no
            # absolute import path.
            let flags = (if $shared { ["-define:KUI_SHARED=true" $"-extra-linker-flags:-Wl,-rpath,($k.dir)"] } else { [] }) ++ (if $k.core { ["-define:KUI_CORE=true"] } else { [] })
            run-or-fail $"odin, ($k.how)" $odin build $src -file $"-collection:kuipack=($root)" $"-out:($exe)" ...$flags
        }
        let r = do { ^$exe --headless } | complete
        if $r.exit_code != 0 {
            print -e $r.stdout $r.stderr
            error make { msg: $"the ($kind) ($k.prog) linked ($k.how) exited ($r.exit_code)" }
        }
        { kind: $kind, platform: $p, link: $k.how, size: (ls $exe | get 0.size), run: ($r.stdout | lines | last) }
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

# <dir>.zip, the directory at its top, as Windows' Explorer opens it:
# Compress-Archive on Windows, zip(1) elsewhere (the release bundles on
# Linux). `-X` leaves out the extra attributes, as tar-gz does.
def zip-dir [dir: string] {
    rm -f $"($dir).zip"
    if $nu.os-info.name == "windows" {
        ^powershell -NoProfile -NonInteractive -Command $"Compress-Archive -Path '($dir)' -DestinationPath '($dir).zip' -CompressionLevel Optimal"
    } else {
        ^zip -q -r -9 -X $"($dir).zip" $dir
    }
}
