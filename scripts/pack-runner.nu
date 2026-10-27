#!/usr/bin/env nu
# The runner's release assets (ADR 0068): the `kui` binary cargo built for
# a platform, packed the way install.sh and install.ps1 take it out.
#
#   nu scripts/pack-runner.nu <platform>                 from target/<triple>/release
#   nu scripts/pack-runner.nu <platform> --bin <file>    from that file (a debug build, say)
#   nu scripts/pack-runner.nu sums <dir>                 SHA256SUMS over every asset in <dir>
#
# What lands in target/pack/runner/ (or --out):
#
#   kui-<version>-linux-x64.tar.gz      one file, `kui`, mode 0755
#   kui-<version>-linux-arm64.tar.gz
#   kui-<version>-darwin-arm64.tar.gz
#   kui-<version>-win32-x64.zip         one file, `kui.exe`
#
# <version> is the workspace's. The release workflow builds each on its
# own platform (cargo-zigbuild at glibc 2.28 on Linux, as the addon is),
# runs this on every build leg, and attaches the four with install.sh,
# install.ps1 and one SHA256SUMS to the tag's GitHub release
# (.github/workflows/release.yml). The npm packages of the runner,
# @qxuken/kui-cli-<platform>, are made from these same files
# (scripts/npm-platform-packages.nu), so npm and the release ship one
# binary.

const ROOT = path self | path dirname | path dirname

export const PLATFORMS = [
    [platform target exe];
    [linux-x64 x86_64-unknown-linux-gnu kui]
    [linux-arm64 aarch64-unknown-linux-gnu kui]
    [darwin-arm64 aarch64-apple-darwin kui]
    [win32-x64 x86_64-pc-windows-msvc kui.exe]
]

def version [] {
    open ($ROOT | path join Cargo.toml) | get workspace.package.version
}

# The asset's file name for a platform at a version.
export def asset-name [version: string, platform: string] {
    let ext = if $platform == "win32-x64" { "zip" } else { "tar.gz" }
    $"kui-($version)-($platform).($ext)"
}

def main [
    platform: string # linux-x64, linux-arm64, darwin-arm64 or win32-x64
    --bin: string # the binary to pack; default target/<triple>/release/kui[.exe]
    --out: string # where the asset goes; default target/pack/runner
] {
    let row = $PLATFORMS | where platform == $platform | get -o 0
    if $row == null {
        error make { msg: $"unknown platform ($platform); there is: ($PLATFORMS | get platform | str join ', ')" }
    }
    let target_dir = $env.CARGO_TARGET_DIR? | default ($ROOT | path join target)
    let bin = $bin | default ($target_dir | path join $row.target release $row.exe) | path expand
    if not ($bin | path exists) {
        error make { msg: $"no runner at ($bin); build it: cargo build -p kui-cli --release --target ($row.target)" }
    }
    let out = $out | default ($ROOT | path join target pack runner) | path expand
    mkdir $out
    let name = asset-name (version) $platform
    let asset = $out | path join $name
    rm -f $asset

    # One file at the top of the archive, whatever the binary was called.
    let stage = $out | path join $".stage-($platform)"
    rm -rf $stage
    mkdir $stage
    cp $bin ($stage | path join $row.exe)
    if $platform == "win32-x64" {
        if $nu.os-info.name == "windows" {
            ^powershell -NoProfile -NonInteractive -Command $"Compress-Archive -Path '($stage | path join $row.exe)' -DestinationPath '($asset)' -CompressionLevel Optimal"
        } else {
            # Off Windows (a cargo-xwin build on a workstation): zip(1).
            do { cd $stage; ^zip -q -9 $asset $row.exe }
        }
    } else {
        ^chmod 755 ($stage | path join $row.exe)
        with-env { COPYFILE_DISABLE: "1" } {
            ^tar --no-xattrs -czf $asset -C $stage $row.exe
        }
    }
    rm -rf $stage
    if not ($asset | path exists) { error make { msg: $"($asset) was not written" } }
    print $"($asset) \((ls $asset | get 0.size))"
}

# SHA256SUMS in <dir> over every other file in it: `<sha256>  <name>`, a
# line each, sorted by name, which `sha256sum -c` and the install scripts
# read. The release's one file of sums, over the runner's assets, the
# install scripts and the C and Odin packs alike.
def "main sums" [dir: string] {
    let dir = $dir | path expand
    let lines = ls $dir
        | where type == file
        | get name
        | each {|f| $f | path basename }
        | where {|n| $n != "SHA256SUMS" and not ($n | str starts-with ".") }
        | sort
        | each {|n| $"(open --raw ($dir | path join $n) | hash sha256)  ($n)" }
    if ($lines | is-empty) { error make { msg: $"nothing to sum in ($dir)" } }
    $lines | str join "\n" | $in + "\n" | save -f ($dir | path join SHA256SUMS)
    print ($lines | str join "\n")
}
