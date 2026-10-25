#!/usr/bin/env nu
# The ten npm packages a release publishes (ADR 0068), esbuild's shape,
# staged from the release's own artifacts into dist/npm/:
#
#   kui-<platform>/       @qxuken/kui-<platform>       kui_node.node, the addon
#   kui-cli-<platform>/   @qxuken/kui-cli-<platform>   bin/kui (bin/kui.exe), the runner
#   kui/                  @qxuken/kui                  packages/kui, prepacked
#   kui-cli/              @qxuken/kui-cli              packages/kui-cli
#
# for <platform> in linux-x64, linux-arm64, darwin-arm64, win32-x64. Each
# platform package declares `os` and `cpu` (and `libc: glibc` on Linux), so
# npm installs only the one a machine is; the two main packages name all
# four of theirs as exact-version optionalDependencies, which is written
# here rather than kept in the repository's package.json, so that an
# in-repo `npm install` (examples/node links packages/kui) never asks a
# registry for a platform package of a version not yet published. Every
# version is the workspace's. The platform package directories are made,
# never committed; publish them before the two that name them.
#
#   nu scripts/npm-platform-packages.nu                  all four platforms
#   nu scripts/npm-platform-packages.nu --platforms darwin-arm64 --pack
#
# Inputs, both in the release workflow's artifact layout:
#   --addons   <dir>/<platform>/kui_node.node             (default packages/kui/prebuilds,
#              where scripts/collect-prebuild.nu writes)
#   --runners  <dir>/kui-<version>-<platform>.tar.gz|.zip (default target/pack/runner,
#              where scripts/pack-runner.nu writes), or one directory down
#              (`gh run download -p 'kui-runner-*'` gives each artifact its
#              own) - the runner's npm binary is the release asset's, byte
#              for byte
#
# `--pack` also runs `npm pack` in each, leaving the tarballs in <out>.
# The two main packages are copied out of the repository with their
# `prepack` already run (packages/kui's copies the docs it cites) and their
# scripts dropped, so the staged copy packs and publishes on its own;
# packages/kui and packages/kui-cli refuse `npm publish` (prepublishOnly)
# for want of the optionalDependencies, which only the staged copy has.

use pack-runner.nu [PLATFORMS asset-name]

const ROOT = path self | path dirname | path dirname

const NAMES = {
    linux-x64: { os: linux, cpu: x64, what: "Linux on x64 (glibc 2.28 or newer)" }
    linux-arm64: { os: linux, cpu: arm64, what: "Linux on arm64 (glibc 2.28 or newer)" }
    darwin-arm64: { os: darwin, cpu: arm64, what: "macOS on Apple silicon" }
    win32-x64: { os: win32, cpu: x64, what: "Windows on x64" }
}

# The runner asset for a platform: in `dir`, or in a directory of its own
# one down; null when neither has it.
def find-runner [dir: string, version: string, platform: string] {
    let name = asset-name $version $platform
    let top = $dir | path join $name
    if ($top | path exists) { return $top }
    ls $dir | where type == dir | get name | each {|d| $d | path join $name } | where {|p| $p | path exists } | get -o 0
}

def version [] {
    open ($ROOT | path join Cargo.toml) | get workspace.package.version
}

# What every package of ours says about itself, beside its own fields.
def common [version: string, dir: string] {
    {
        version: $version
        license: MIT
        repository: { type: git, url: "https://github.com/qxuken/kui", directory: $dir }
        publishConfig: { access: public, registry: "https://registry.npmjs.org/" }
    }
}

def platform-manifest [name: string, platform: string, version: string, dir: string, files: list<string>, what: string] {
    let n = $NAMES | get $platform
    let base = { name: $name, description: $"($what) for ($n.what); an optional dependency npm installs on that platform only" }
    let fields = {
        os: [$n.os]
        cpu: [$n.cpu]
    }
    let fields = if $n.os == "linux" { $fields | insert libc [glibc] } else { $fields }
    $base | merge (common $version $dir) | merge $fields | merge { files: $files, preferUnplugged: true }
}

def write-json [path: string, value: record] {
    $value | to json --indent 2 | $in + "\n" | save -f $path
}

def platform-readme [name: string, main: string, platform: string, what: string] {
    let n = $NAMES | get $platform
    $"# ($name)

($what) for ($n.what). Install [`($main)`]\(https://www.npmjs.com/package/($main)), not this:
this package is one of its optionalDependencies, which npm installs only on
the platform it is for \(ADR 0068 in https://github.com/qxuken/kui).
"
}

# A main package out of the repository: its `files`, README and LICENSE,
# with the version and the optionalDependencies set and the scripts gone.
def stage-main [src: string, dst: string, version: string, prefix: string] {
    let pkg = open ($src | path join package.json)
    rm -rf $dst
    mkdir $dst
    for f in ($pkg.files | append [README.md LICENSE]) {
        let from = $src | path join ($f | str trim --right --char "/")
        if not ($from | path exists) { error make { msg: $"($from) is in ($pkg.name)'s files and not there" } }
        let to = $dst | path join ($f | str trim --right --char "/")
        mkdir ($to | path dirname)
        cp -r $from $to
    }
    let optional = $PLATFORMS | get platform | reduce --fold {} {|p, acc| $acc | insert $"($prefix)-($p)" $version }
    let manifest = $pkg
        | update version $version
        | reject -o scripts devDependencies
        | upsert optionalDependencies $optional
    write-json ($dst | path join package.json) $manifest
}

def main [
    --addons: string # <dir>/<platform>/kui_node.node; default packages/kui/prebuilds
    --runners: string # <dir>/kui-<version>-<platform>.tar.gz|.zip; default target/pack/runner
    --out: string # default dist/npm
    --platforms: string # comma-separated; default all four (the main packages name all four either way)
    --pack # npm pack each into <out>
] {
    let v = version
    let addons = $addons | default ($ROOT | path join packages kui prebuilds) | path expand
    let runners = $runners | default ($ROOT | path join target pack runner) | path expand
    let out = $out | default ($ROOT | path join dist npm) | path expand
    let all = $PLATFORMS | get platform
    let wanted = if $platforms == null { $all } else { $platforms | split row "," | str trim }
    for p in $wanted {
        if $p not-in $all { error make { msg: $"unknown platform ($p); there is: ($all | str join ', ')" } }
    }
    # Every input first, so a missing one fails before anything is written.
    for p in $wanted {
        let addon = $addons | path join $p kui_node.node
        if not ($addon | path exists) { error make { msg: $"no addon for ($p) at ($addon)" } }
        if (find-runner $runners $v $p) == null {
            error make { msg: $"no runner for ($p): no ($runners | path join (asset-name $v $p)) \(scripts/pack-runner.nu makes it)" }
        }
    }
    mkdir $out

    for p in $wanted {
        let row = $PLATFORMS | where platform == $p | first

        let name = $"@qxuken/kui-($p)"
        let dir = $out | path join $"kui-($p)"
        rm -rf $dir
        mkdir $dir
        cp ($addons | path join $p kui_node.node) ($dir | path join kui_node.node)
        write-json ($dir | path join package.json) (platform-manifest $name $p $v packages/kui [kui_node.node] "The kui Node addon")
        platform-readme $name "@qxuken/kui" $p "The kui Node addon" | save -f ($dir | path join README.md)
        cp ($ROOT | path join LICENSE) $dir

        let name = $"@qxuken/kui-cli-($p)"
        let dir = $out | path join $"kui-cli-($p)"
        rm -rf $dir
        mkdir ($dir | path join bin)
        let asset = find-runner $runners $v $p
        if ($asset | str ends-with ".zip") {
            if (which unzip | is-not-empty) {
                ^unzip -q -o $asset $row.exe -d ($dir | path join bin)
            } else {
                ^tar -xf $asset -C ($dir | path join bin) $row.exe
            }
        } else {
            ^tar -xzf $asset -C ($dir | path join bin) $row.exe
        }
        let bin = $dir | path join bin $row.exe
        if not ($bin | path exists) { error make { msg: $"($asset) holds no ($row.exe)" } }
        if $nu.os-info.name != "windows" { ^chmod 755 $bin }
        write-json ($dir | path join package.json) (platform-manifest $name $p $v packages/kui-cli [$"bin/($row.exe)"] "The kui runner")
        platform-readme $name "@qxuken/kui-cli" $p "The kui runner" | save -f ($dir | path join README.md)
        cp ($ROOT | path join LICENSE) $dir
    }

    # packages/kui's prepack copies the docs its files list names.
    do {
        cd ($ROOT | path join packages kui)
        ^node prepack.mjs
    }
    stage-main ($ROOT | path join packages kui) ($out | path join kui) $v "@qxuken/kui"
    stage-main ($ROOT | path join packages kui-cli) ($out | path join kui-cli) $v "@qxuken/kui-cli"

    let dirs = $wanted | each {|p| [$"kui-($p)" $"kui-cli-($p)"] } | flatten | append [kui kui-cli]
    if $pack {
        for d in $dirs {
            do {
                cd ($out | path join $d)
                let r = do { ^npm pack --pack-destination $out --silent } | complete
                if $r.exit_code != 0 { print -e $r.stderr; error make { msg: $"npm pack failed in ($d)" } }
            }
        }
    }
    print $"staged ($v) in ($out), publish in this order:"
    print ($dirs | str join "\n")
}
