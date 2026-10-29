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
#   nu scripts/pack-ffi.nu bundle [--partial]      the release's archives, from the packs
#   nu scripts/pack-ffi.nu verify <archive>        link and run the C and Odin programs against one
#
# What lands in target/pack/, per platform - the pack, a build leg's own
# output, which `bundle` lays out into the release's archives:
#
#   kui-pack-<version>-<platform>/
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
#   kui-pack-<version>-<platform>.tar.gz, and SHA256SUMS over this version's
#   (`-core` after the platform for a --no-runner pack: the library
#   without the runner)
#
# Until alpha.73 the packs were kui-ffi-<version>-<platform>, the name the
# release's archives have now.
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
# The release ships two archives for each platform (backlog F278), which
# `bundle` writes to target/pack/release/ with their SHA256SUMS, a .zip for
# win32-x64 as the runner's is:
#
#   kui-ffi-<version>-<platform>.tar.gz       the full library: the runner
#                                             (kui_run) in, shared and static
#   kui-ffi-core-<version>-<platform>.tar.gz  the slim one: no runner, no
#                                             wgpu, winit or audio device,
#                                             shared only
#
# Each carries the C headers and the Odin package, over one copy of its
# library:
#
#   kui-ffi[-core]-<version>-<platform>/
#     include/           the HEADERS
#     lib/<platform>/    shared/  libkui_ffi.so / .dylib, or
#                                 kui_ffi.dll + kui_ffi.dll.lib
#                        static/  libkui_ffi.a, or kui_ffi.lib and
#                                 windows-targets' .libs, link.txt
#                                 (the full archive's alone)
#                        BUILD.txt
#                        pkgconfig/  kui.pc and kui-static.pc, or kui-core.pc
#                                 (but on Windows)
#     kui/               the Odin package (packages/odin/kui); its
#                        kui/c/kui_c.odin links ../../lib/<platform>/, so
#                        the package stays beside lib/
#     README.md, LICENSE
#
# Until alpha.72 the release carried a C and an Odin archive per platform,
# each with the runner's library and the core one, shared and static, and
# the two again over all four platforms: 252 MB each of the all-platform
# ones at alpha.71, 55 MB for darwin-arm64's own, in which the Odin
# package carried a second copy of every library. The full archive is
# what a program that opens its window with kui links, and the Odin
# package's static default (an import, and nothing beside the program)
# needs its static library; the slim one is what a host with its own
# window and its own Rust beside kui links - shared, as such a host must
# (below) - and at a quarter of the size. A project that vendors every
# target unpacks the four of one kind into one directory: the layout is
# one, each platform's files under its own lib/<platform>/, but the Odin
# package's kui_c.odin, which each archive writes for its own platform.
#
# A kind to a directory, so `-L lib/<platform>/static -lkui_ffi` is the
# static link and the same line with `shared` the dynamic one: a linker
# handed a directory holding both takes the shared library. The Odin
# package's kui_c.odin names its own archive's library: in the full one,
# static unless KUI_SHARED; in the slim one, shared. `verify` builds
# examples/c/apps/counter.c and examples/odin/apps/counter.odin against
# a full archive, each linked shared and static, and the C and Odin host
# loops (examples/*/tools/host_loop) against a slim one, as a user outside
# this repository would, and runs each `--headless`; Linux and macOS,
# since Windows has no `cc`.
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

    # The core library keeps `access`, the accessibility bridge a host
    # attaches to its own window (kui_access_attach, backlog F269).
    let features = if $no_runner { [--no-default-features --features access] } else { [] }
    let feature_note = if $no_runner { "no default features but access (no runner)" } else { "default features (with the runner)" }
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
    let sums = glob $"kui-pack-(version)-*.tar.gz" | each {|t| sha256sum-of ($t | path basename) } | sort
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

# target/pack/kui-pack-<version>-<platform>[-core]/ and its tarball.
def stage [row: record, built: record, feature_note: string, no_runner: bool] {
    # A pack without the runner is another library, so another name: it
    # used to overwrite the full pack of its platform.
    let name = $"kui-pack-(version)-($row.platform)(if $no_runner { '-core' } else { '' })"
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
        $"kui-ffi (version), ($row.platform)(if $no_runner { ', without the runner' } else { '' })"
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

# The release's archives (see the top of this file), from the packs `main`
# left in target/pack/ — in the release workflow, the four platforms' its
# build legs uploaded: a full and a slim archive per platform.
def "main bundle" [
    --partial # bundle the platforms that are packed, each on its own; without it all four must be
] {
    let v = version
    let all = $PLATFORMS | get platform
    let pack_of = {|p| $OUT | path join $"kui-pack-($v)-($p)" }
    let core_of = {|p| $OUT | path join $"kui-pack-($v)-($p)-core" }
    let have = $all | where {|p| do $pack_of $p | path exists }
    let core = $all | where {|p| do $core_of $p | path exists }
    if ($have | is-empty) and ($core | is-empty) {
        error make { msg: $"no kui-pack-($v)-<platform> pack in ($OUT); run pack-ffi.nu first" }
    }
    let missing = $all | where {|p| $p not-in $have }
    let core_missing = $all | where {|p| $p not-in $core }
    if ($missing | is-not-empty) or ($core_missing | is-not-empty) {
        let what = [
            (if ($missing | is-empty) { null } else { $"the runner's library for ($missing | str join ', ')" })
            (if ($core_missing | is-empty) { null } else { $"the library without the runner for ($core_missing | str join ', ') \(--no-runner)" })
        ] | compact | str join "; "
        if not $partial {
            error make { msg: $"no pack of ($what) in ($OUT); pack them, or pass --partial" }
        }
        print -e $"bundling without ($what)"
    }
    let rel = $OUT | path join release
    rm -rf $rel
    mkdir $rel
    let archives = ($have | each {|p| archive-one $rel full $v $p (do $pack_of $p) }) ++ ($core | each {|p| archive-one $rel core $v $p (do $core_of $p) })
    cd $rel
    $archives | each {|t| sha256sum-of $t } | str join "\n" | save -f SHA256SUMS
    print ($archives | wrap archive | insert size {|r| ls $r.archive | get 0.size } | table --width 100)
    print $"in ($rel)"
}

# One platform's archive of one kind - `full` (the runner's library, shared
# and static) or `core` (the library without it, shared alone) - from
# `pack`, in `rel`: laid out in a directory of its name, then a .tar.gz of
# it, or a .zip for win32-x64, as the runner's is (scripts/pack-runner.nu's
# asset-name). Its file name.
def archive-one [rel: string, kind: string, v: string, platform: string, pack: string] {
    let name = if $kind == "core" { $"kui-ffi-core-($v)-($platform)" } else { $"kui-ffi-($v)-($platform)" }
    let dir = $rel | path join $name
    let lib = $dir | path join lib $platform
    lay-out-libs $pack $lib ($kind == "core")
    if not ($platform | str starts-with "win32") {
        write-pkg-config $lib $v $kind
    }
    # The pack's include/ whole: every header it carries (HEADERS).
    cp -r ($pack | path join include) $dir
    # The Odin package beside lib/, its link block naming the libraries
    # there: one copy of each, the C host's and the Odin package's.
    cp -r ($ROOT | path join packages odin kui) $dir
    link-the-pack ($dir | path join kui c) $platform $kind
    cp ($ROOT | path join LICENSE) $dir
    readme $kind $v $platform | save -f ($dir | path join README.md)
    cd $rel
    let file = if ($platform | str starts-with "win32") {
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
# (with its import library on Windows), static/ the archive with link.txt
# and what link.txt names that no system has - left out with
# `shared_only`, the slim archive's.
def lay-out-libs [pack: string, dst: string, shared_only: bool] {
    let shared = $dst | path join shared
    let static = $dst | path join static
    mkdir $shared
    if not $shared_only { mkdir $static }
    for f in (ls ($pack | path join lib) | get name) {
        let base = $f | path basename
        if $base in [libkui_ffi.so libkui_ffi.dylib kui_ffi.dll kui_ffi.dll.lib] {
            cp $f $shared
        } else if not $shared_only {
            cp $f $static
        }
    }
    if not $shared_only { cp ($pack | path join link.txt) $static }
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

# lib/<platform>/pkgconfig/: kui.pc and kui-static.pc in the full archive,
# kui-core.pc in the slim one, each finding its library from where the
# file is (${pcfiledir}), so the archive works wherever it is unpacked. A
# shared one carries its rpath; a static one the platform's link.txt. Not
# for Windows, whose MSVC reads no pkg-config.
def write-pkg-config [lib_dir: string, v: string, kind: string] {
    let pc = $lib_dir | path join pkgconfig
    mkdir $pc
    let rows = if $kind == "core" {
        [[name dir what]; [kui-core shared "kui's C library without the runner (no kui_run, wgpu, winit or audio device), linked shared"]]
    } else {
        [
            [name dir what];
            [kui shared "kui's C library, with the windowed runner, linked shared"]
            [kui-static static "kui's C library, with the windowed runner, linked static"]
        ]
    }
    for row in $rows {
        let libs = if $row.dir == "shared" {
            "-L${libdir} -lkui_ffi -Wl,-rpath,${libdir}"
        } else {
            let link = open --raw ($lib_dir | path join static link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default ""
            $"-L${libdir} -lkui_ffi ($link)" | str trim
        }
        $"prefix=${pcfiledir}/../../..
includedir=${prefix}/include
libdir=${pcfiledir}/../($row.dir)

Name: ($row.name)
Description: ($row.what)
Version: ($v)
Cflags: -I${includedir}
Libs: ($libs)
" | save -f ($pc | path join $"($row.name).pc")
    }
}

# The Odin package's link block: kui_c.odin's own, between its markers,
# replaced by one that names its archive's library, under ../../lib/
# (lib/<platform>/ beside the package), with what the platform's link.txt
# says a static link needs. The full archive links static unless
# -define:KUI_SHARED=true; the slim one links its shared library whatever
# is asked, since it carries no other.
def link-the-pack [c_dir: string, platform: string, kind: string] {
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
    let windows = $platform | str starts-with "win32"
    let lib = $"../../lib/($platform)"
    let shared = if $windows { [$"($lib)/shared/kui_ffi.dll.lib"] } else if ($platform | str starts-with "darwin") { [$"($lib)/shared/libkui_ffi.dylib"] } else { [$"($lib)/shared/libkui_ffi.so"] }
    let shared_import = $"\t\tforeign import lib {\n(do $quoted $shared)\n\t\t}\n"
    # A plugin reaches a branch on Windows only, where it imports from the
    # host's kui_ffi.dll as a program does: the shared library, never a
    # private static copy of kui.
    let branch = if $kind == "core" {
        $"} else when ($when_of | get $platform) {
\t// The library without the runner, shared: the one this archive holds.
\twhen true {
($shared_import)\t}
"
    } else {
        let full = odin-static-link ($c_dir | path join $lib static) $lib $windows
        let attr = if $windows { $"\t\t@\(extra_linker_flags=\"($full.flags | str join ' ')\")\n" } else { "" }
        $"} else when ($when_of | get $platform) {
\twhen KUI_CORE {
\t\t#assert\(!KUI_CORE, \"the kui-ffi archive holds the runner's library; the library without the runner is the kui-ffi-core archive's\")
($shared_import)\t} else when KUI_PLUGIN || KUI_SHARED {
($shared_import)\t} else {
($attr)\t\tforeign import lib {
(do $quoted $full.imports)
\t\t}
\t}
"
    }
    let what = if $kind == "core" {
        "the library without the runner (no kui_run, no wgpu, no winit, no
// audio device), shared - the program finds it at run time, through an
// rpath or beside it"
    } else {
        "the library with the runner, static by default, so a program needs
// nothing beside it, or with -define:KUI_SHARED=true shared, which the
// program then finds at run time (an rpath, or the library beside it); a
// Windows plugin (-define:KUI_PLUGIN=true) always links the shared one"
    }
    let block = $"// -- link: this archive's own, written by scripts/pack-ffi.nu's bundle
// \(backlog F278). It links libkui_ffi from lib/($platform)/ beside the
// package \(../../lib from this file, so kui/ stays beside lib/):
// ($what).
// A platform the archive does not carry falls back to KUI_LIB, as the
// repository's file does.
KUI_SHARED :: #config\(KUI_SHARED, ($kind == 'core'))
KUI_CORE :: #config\(KUI_CORE, ($kind == 'core'))
KUI_LIB :: #config\(KUI_LIB, \"system:kui_ffi.dll.lib\" when ODIN_OS == .Windows else \"system:kui_ffi\")
KUI_PLUGIN :: #config\(KUI_PLUGIN, false)

when KUI_PLUGIN && ODIN_OS == .Darwin {
\tforeign import lib \"system:System\"
} else when KUI_PLUGIN && ODIN_OS != .Windows {
\tforeign import lib \"system:c\"
($branch)} else {
\tforeign import lib { KUI_LIB }
}
"
    let out = ($text | str substring 0..<$a) + $block + ($text | str substring $b..)
    $out | save -f --raw $file
}

# README.md of a release archive: what is in it, and the link lines for C
# and for Odin, its platform's paths spelled out.
def readme [kind: string, v: string, platform: string] {
    readme-text $kind $v $platform | str replace -a "lib/<platform>" $"lib/($platform)"
}

def readme-text [kind: string, v: string, platform: string] {
    let core = $kind == "core"
    let name = if $core { $"kui-ffi-core-($v)-($platform)" } else { $"kui-ffi-($v)-($platform)" }
    let library = if $core {
        "built without the runner: no `kui_run`, no wgpu, no winit, no audio device, for a host that brings its own window and draws `kui_draw_data` itself (`include/kui_wgpu.h` draws it on a WebGPU device of its own). Shared only: it is the library to link beside another Rust library (below), and a quarter of the full archive's size"
    } else {
        "with default features, so `kui_run` (the windowed runner) is in, shared and static"
    }
    let c_use = if $core {
        "```sh
export PKG_CONFIG_PATH=$PWD/lib/<platform>/pkgconfig
cc -std=c11 app.c $(pkg-config --cflags --libs kui-core) -o app   # with its rpath
# by hand
cc -std=c11 -I include app.c -L lib/<platform>/shared -lkui_ffi -Wl,-rpath,'$ORIGIN/lib/<platform>/shared' -o app
```"
    } else {
        "```sh
export PKG_CONFIG_PATH=$PWD/lib/<platform>/pkgconfig
cc -std=c11 app.c $(pkg-config --cflags --libs kui) -o app          # shared, with its rpath
cc -std=c11 app.c $(pkg-config --cflags --libs kui-static) -o app   # static
# by hand: the dynamic library, found at run time through the rpath
cc -std=c11 -I include app.c -L lib/<platform>/shared -lkui_ffi -Wl,-rpath,'$ORIGIN/lib/<platform>/shared' -o app
# static: link.txt's last line is what the archive needs besides itself
cc -std=c11 -I include app.c -L lib/<platform>/static -lkui_ffi $(tail -1 lib/<platform>/static/link.txt) -o app
```"
    }
    let msvc = if $core {
        "With MSVC: `cl /std:c11 /I include app.c /link /LIBPATH:lib\\win32-x64\\shared kui_ffi.dll.lib`, with `kui_ffi.dll` beside the program."
    } else {
        "With MSVC: `cl /std:c11 /I include app.c /link /LIBPATH:lib\\win32-x64\\shared kui_ffi.dll.lib`, with `kui_ffi.dll` beside the program; for the static one `kui_ffi.lib`, link.txt's last line and `/NODEFAULTLIB:libcmt` from `lib\\win32-x64\\static`."
    }
    let odin_use = if $core {
        "`kui/c/kui_c.odin` links the shared library from `lib/<platform>/shared/`,
which the program finds at run time:

```sh
odin build app -collection:kui=<this directory> \\
  -extra-linker-flags:\"-Wl,-rpath,<absolute>/lib/<platform>/shared\"
# on Windows, copy lib/win32-x64/shared/kui_ffi.dll beside the program
```"
    } else {
        "`kui/c/kui_c.odin` links libkui_ffi from `lib/<platform>/` itself, as
vendor:raylib links its own, the static library by default, so the
program needs nothing beside it:

```sh
odin build app -collection:kui=<this directory>
# the shared library instead, found at run time through an rpath (on
# Windows, copy lib/win32-x64/shared/kui_ffi.dll beside the program)
odin build app -collection:kui=<this directory> -define:KUI_SHARED=true \\
  -extra-linker-flags:\"-Wl,-rpath,<absolute>/lib/<platform>/shared\"
```"
    }
    let other = if $core {
        $"The full library - the runner in, shared and static - is
`kui-ffi-($v)-<platform>`."
    } else {
        $"The library without the runner - no kui_run, wgpu, winit or audio
device, shared only, for a host that brings its own window - is
`kui-ffi-core-($v)-<platform>`, a quarter of this archive's size."
    }
    $"# ($name)

kui's library for C and Odin on ($platform): `libkui_ffi`, ($library).
`BUILD.txt` names the target, the toolchain and, on Linux, the glibc it
needs \(2.28 or newer). `include/` holds the C headers: `kui.h`, and,
header-only, `kui_glfw.h` and `kui_sdl3.h` \(GLFW's or SDL3's keys and
pointer) and `kui_wgpu.h` \(kui's frame on a WebGPU device of your own).
`kui/` is the Odin package, with `kui/glfw`, `kui/sdl3` and `kui/wgpu`
the same three.

($other)

## C

($c_use)

On macOS the rpath is `@executable_path/...` rather than `$ORIGIN/...`.
($msvc)

## Odin

Point a collection at this directory and import the package - `import kui
\"kui:kui\"` with `-collection:kui=<this directory>` - or copy `kui/` and
`lib/` into your project side by side: the package links from `../../lib`
beside it, so the two travel together. ($odin_use)

`-define:KUI_LIB=<name>` links a library of your own instead. The
package's own documentation is packages/odin/README.md in the repository.

## Beside another Rust library

Every Rust static library carries its own copy of Rust's standard
library, and the full one also carries wgpu and winit for `kui_run`.
Linked into one program with another \(wgpu-native's `libwgpu_native.a`,
say), the linker stops at the symbols both define: `rust_eh_personality`,
and on macOS `RawWindowMetalLayer`'s classes. Link the shared library
instead; each keeps its copies to itself.

kui.h's `KUI_ABI_VERSION` is what this library answers from
`kui_abi_version\(\)`; a program built against another header refuses it.
"
}

# Links the C and Odin programs against one platform's release archive
# and runs each `--headless`: the counter, shared and static, against a
# full archive, and the host loop - which calls nothing of the runner -
# against a slim one: the check that what the release ships links and
# works outside this repository. C needs `cc` (or $env.CC); Odin needs odin
# on PATH or ODIN, and is left out with --no-odin.
def "main verify" [
    archive: string # a kui-ffi[-core]-<version>-<platform>.tar.gz
    --platform: string # whose libraries to link; default: this machine's
    --no-odin # the C programs alone
] {
    let p = $platform | default (host-platform)
    if ($p | str starts-with "win32") or ($archive | str ends-with ".zip") {
        error make { msg: "verify links with cc or odin's own linker line, on Linux and macOS; Windows is not checked" }
    }
    let archive = $archive | path expand
    let name = $archive | path basename | str replace ".tar.gz" ""
    if not ($name | str starts-with "kui-ffi-") {
        error make { msg: $"($name) is not a kui-ffi[-core]-<version>-<platform> archive" }
    }
    let core = $name | str starts-with "kui-ffi-core-"
    let work = $OUT | path join $"verify-($name)"
    rm -rf $work
    mkdir $work
    tar -xzf $archive -C $work
    let root = $work | path join $name
    let lib = $root | path join lib $p
    if not ($lib | path exists) {
        error make { msg: $"($name) has no libraries for ($p)" }
    }
    let links = if $core { [shared] } else { [shared static] }
    let langs = if $no_odin { [c] } else { [c odin] }
    let prog = if $core { {name: host_loop, sub: tools} } else { {name: counter, sub: apps} }
    let results = $langs | each {|lang| $links | each {|how|
        let exe = $work | path join $"($lang)-($prog.name)-($how)"
        let dir = $lib | path join $how
        if $lang == "c" {
            mkdir ($work | path join src $prog.sub)
            cp ($ROOT | path join examples c common.h) ($work | path join src)
            cp ($ROOT | path join examples c $prog.sub $"($prog.name).c") ($work | path join src $prog.sub)
            let cc = $env.CC? | default "cc"
            let src = $work | path join src $prog.sub $"($prog.name).c"
            if (which pkg-config | is-not-empty) {
                # Through the archive's own .pc files, as its README says.
                let pc = if $core { "kui-core" } else if $how == "static" { "kui-static" } else { "kui" }
                let flags = with-env { PKG_CONFIG_PATH: ($lib | path join pkgconfig) } {
                    run-or-fail $"pkg-config ($pc)" pkg-config --cflags --libs $pc
                    ^pkg-config --cflags --libs $pc | str trim | split row " " | where {|a| $a != "" }
                }
                run-or-fail $"cc, ($how), pkg-config ($pc)" $cc -std=c11 $src ...$flags -o $exe
            } else {
                let extra = if $how == "shared" { [$"-Wl,-rpath,($dir)"] } else {
                    open --raw ($dir | path join link.txt) | lines | where {|l| not ($l | str starts-with "#") and ($l | str trim) != "" } | get -o 0 | default "" | split row " " | where {|a| $a != "" }
                }
                run-or-fail $"cc, ($how)" $cc -std=c11 -I ($root | path join include) $src -L $dir -lkui_ffi ...$extra -o $exe
            }
        } else {
            let src = $work | path join src $"($prog.name).odin"
            mkdir ($work | path join src)
            open --raw ($ROOT | path join examples odin $prog.sub $"($prog.name).odin")
                | str replace 'import kui "../../../packages/odin/kui"' 'import kui "kuipack:kui"'
                | save -f --raw $src
            let odin = $env.ODIN? | default "odin"
            # Nothing but the import: the package's kui_c.odin names its
            # archive's library, as vendor:raylib does. A shared build is
            # told where to find the library when it runs. Imported as a
            # collection, as the archive's README says: Odin takes no
            # absolute import path.
            let flags = if $how == "shared" { [$"-extra-linker-flags:-Wl,-rpath,($dir)"] ++ (if $core { [] } else { ["-define:KUI_SHARED=true"] }) } else { [] }
            run-or-fail $"odin, ($how)" $odin build $src -file $"-collection:kuipack=($root)" $"-out:($exe)" ...$flags
        }
        let r = do { ^$exe --headless } | complete
        if $r.exit_code != 0 {
            print -e $r.stdout $r.stderr
            error make { msg: $"the ($lang) ($prog.name) linked ($how) exited ($r.exit_code)" }
        }
        { lang: $lang, platform: $p, link: $how, program: $prog.name, size: (ls $exe | get 0.size), run: ($r.stdout | lines | last) }
    } } | flatten
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
