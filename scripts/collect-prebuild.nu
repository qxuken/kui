#!/usr/bin/env nu
# Copies the kui-node cdylib built for a cargo target into
# packages/kui/prebuilds/<platform>-<arch>/kui_node.node, named after Node's
# process.platform and process.arch (linux-x64, darwin-arm64, ...): the
# layout scripts/npm-platform-packages.nu reads the addons from, one
# @qxuken/kui-<platform> package each (ADR 0068), and the one
# packages/kui/native.cjs still falls back to for a hand-staged package.
# The release workflow does the same in its build legs' shell (and
# scripts/release-local.nu runs this).
#
#   nu scripts/collect-prebuild.nu <rust target> <platform-arch>

def main [target: string, prebuild: string] {
    cd ($env.FILE_PWD | path dirname)
    let dir = $"target/($target)/release"
    let src = ([libkui_node.so libkui_node.dylib kui_node.dll]
        | each {|name| $dir | path join $name }
        | where {|p| $p | path exists }
        | get -o 0)
    if $src == null {
        print -e $"no kui-node cdylib in ($dir); run: cargo build -p kui-node --release --target ($target)"
        exit 1
    }
    let out = $"packages/kui/prebuilds/($prebuild)"
    mkdir $out
    cp $src $"($out)/kui_node.node"
    ls -l $"($out)/kui_node.node" | select name size modified | print
}
