#!/usr/bin/env nu
# The pins live in conventional version files and mise.toml. This is only
# the native-library wiring that tool installation cannot supply.
const ROOT = path self | path dirname | path dirname

def --wrapped checked [cmd: string, ...args: string] {
    let r = (^$cmd ...$args | complete)
    if $r.exit_code != 0 { error make {msg: $r.stderr} }
    $r.stdout | str trim
}

def --env bindings [] {
    cd $ROOT
    let wgpu = checked mise where wgpu-native
    let odin = checked mise where odin
    $env.WGPU_NATIVE_DIR = $wgpu
    $env.ODIN = ($odin | path join (if $nu.os-info.name == "windows" { "odin.exe" } else { "odin" }))
    $env.ODIN_ROOT = $odin
    # Odin ships its Windows and macOS libraries, but no Linux copy.
    if $nu.os-info.name == "linux" {
        let arch = if $nu.os-info.arch == "aarch64" { "aarch64" } else { "x86_64" }
        let lib = $env.ODIN_ROOT | path join vendor wgpu lib $"wgpu-linux-($arch)-release"
        mkdir $lib
        cp -r ($wgpu | path join lib) $lib
    }
}

def "main bindings" [--github-env: path] {
    bindings
    if $github_env != null {
        [ $"WGPU_NATIVE_DIR=($env.WGPU_NATIVE_DIR)" $"ODIN=($env.ODIN)" $"ODIN_ROOT=($env.ODIN_ROOT)" "" ]
        | str join "\n" | save --append $github_env
    }
    print $"wgpu-native: ($env.WGPU_NATIVE_DIR)"
    ^$env.ODIN version
}

def --wrapped "main test" [...args: string] {
    bindings
    ^$nu.current-exe ($ROOT | path join scripts test.nu) --node ...$args
    if $env.LAST_EXIT_CODE != 0 { exit $env.LAST_EXIT_CODE }
}

def "main fuzz" [--github-env: path] {
    let pin = open ($ROOT | path join fuzz rust-toolchain.toml) | get toolchain.channel
    ^rustup toolchain install $pin --profile minimal --no-self-update
    if $env.LAST_EXIT_CODE != 0 { exit $env.LAST_EXIT_CODE }
    if $github_env != null { $"KUI_FUZZ_TOOLCHAIN=($pin)\n" | save --append $github_env }
}

def main [] { print "nu scripts/toolchain.nu bindings [--github-env PATH] | test [args] | fuzz [--github-env PATH]" }
