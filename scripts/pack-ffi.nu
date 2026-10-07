#!/usr/bin/env nu
# A pack of the C library for every platform kui ships: libkui_ffi, dynamic
# and static, with the header - what a C, Odin or other non-Rust host links
# instead of building the workspace.
#
#   nu scripts/pack-ffi.nu                         every platform this machine can build
#   nu scripts/pack-ffi.nu --platforms linux-x64,darwin-arm64
#   nu scripts/pack-ffi.nu --accept-msvc-license   Windows too (see below)
#   nu scripts/pack-ffi.nu --no-runner             without the windowed runner (kui_run)
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
# the rest in a Docker image this script builds once (pinned below), the
# same tools the release workflow uses for the Node prebuilds:
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

# Pinned like the release workflow's (.github/workflows/release.yml).
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
] {
    let host = host-platform
    let has_docker = (which docker | is-not-empty) and ((do { docker info } | complete).exit_code == 0)
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
            build-native $row.target $features
        } else {
            build-docker $row.target $row.builder $features
        }
        stage $row $note $feature_note
    }

    # Over every tarball here, not only this run's: a run of one platform
    # leaves the others' packs, and their sums, where they were.
    cd $OUT
    let sums = glob "kui-ffi-*.tar.gz" | each {|t| sha256sum-of ($t | path basename) } | sort
    $sums | str join "\n" | save -f SHA256SUMS
    print ($built | wrap tarball | insert size {|r| ls $r.tarball | get 0.size })
    print $"in ($OUT)"
}

# The compile both kinds come from; rustc's note on what a static link
# needs is on stderr, and is kept.
def rustc-args [target: string, features: list<string>] {
    [-p kui-ffi --lib --release --locked --target $target ...$features --crate-type "cdylib,staticlib" -- --print native-static-libs]
}

def build-native [target: string, features: list<string>] {
    cd $ROOT
    with-env { CARGO_PROFILE_RELEASE_STRIP: debuginfo } {
        let r = do { cargo rustc ...(rustc-args $target $features) } | complete
        if $r.exit_code != 0 {
            print -e $r.stderr
            error make { msg: $"the ($target) build failed" }
        }
        let registry = $env.CARGO_HOME? | default ($nu.home-dir | path join .cargo) | path join registry src
        let extra = import-lib-dirs $target | each {|d| glob ($registry | path join "*" $d | str replace -a '\' '/') } | flatten
        { dir: ($ROOT | path join target $target release), log: $r.stderr, toolchain: (rustc --version), extra: $extra }
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
def image-tag [] {
    $"kui-pack-ffi:zb($CARGO_ZIGBUILD_VERSION)-zig($ZIGLANG_VERSION)-xwin($CARGO_XWIN_VERSION)"
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
RUN pip3 install --break-system-packages cargo-zigbuild==($CARGO_ZIGBUILD_VERSION) ziglang==($ZIGLANG_VERSION)
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-pc-windows-msvc
RUN cargo install cargo-xwin --locked --version ($CARGO_XWIN_VERSION)
"
    $dockerfile | docker build -t $tag -
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
    cp ($ROOT | path join crates kui-ffi include kui.h) ($dir | path join include)
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
        $"built: ($row.how), (if $row.how == 'native' { 'cargo' } else { $row.builder })"
        $"features: ($feature_note)"
        $"toolchain: ($built.toolchain)"
        $"ABI: (open ($ROOT | path join crates kui-ffi include kui.h) | parse -r '#define KUI_ABI_VERSION (?<v>\d+)u' | get 0.v); a host checks kui_abi_version against it"
        ...(glibc-floor ($dir | path join lib libkui_ffi.so))
    ] | str join "\n" | save -f ($dir | path join BUILD.txt)
    if ($built.dir | str ends-with $".build-($row.target)") { rm -rf $built.dir }
    cd $OUT
    tar -czf $"($name).tar.gz" $name
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
