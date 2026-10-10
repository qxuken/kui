#!/usr/bin/env nu
# The fuzz targets in fuzz/ (cargo-fuzz, libFuzzer), run the way the
# `fuzz` workflow runs them. Nightly Rust and cargo-fuzz:
#
#   rustup toolchain install nightly --profile minimal
#   cargo install cargo-fuzz --locked
#
#   nu scripts/fuzz.nu list                     the targets
#   nu scripts/fuzz.nu run scenes               one target until a crash or Ctrl-C
#   nu scripts/fuzz.nu run scenes --time 600    ... for ten minutes at most
#   nu scripts/fuzz.nu all --time 300           every target, five minutes each
#   nu scripts/fuzz.nu keep scenes fuzz/artifacts/scenes/crash-<hash> f170
#                                               minimize a crash into regressions/
#
# Linux (WSL on a Windows machine) and macOS. libFuzzer and AddressSanitizer
# on Windows MSVC build, but a run there is not one to trust, and nothing in
# a target is platform-specific: the core is the same core on every OS.
#
# Each target reads three corpus dirs: fuzz/corpus/<target> (what runs grow,
# not checked in, where libFuzzer writes), fuzz/seeds/<target> (the starting
# inputs) and fuzz/regressions/<target> (what crashed once, minimized). The
# last two are checked in and replayed on stable by `cargo test -p
# kui-fuzz`, which `cargo test --workspace` includes, so a fixed crash stays
# fixed without a fuzzer in CI's `check`.
#
# A crash lands in fuzz/artifacts/<target>/. `keep` minimizes it (`cargo
# fuzz tmin`) and copies the result to fuzz/regressions/<target>/<name>;
# name it after the backlog item that fixes it.
#
# Exit codes: 0 no target crashed, 1 one did (its artifact is printed),
# 2 a build or a tool failed.

const TARGETS = [parsers values path scenes decode]

# libFuzzer's flags for every run. RSS: a decoder may allocate what an
# image's header asks for, and the image crate's own cap is 512 MB, so the
# limit sits above it.
const LIBFUZZER = [-rss_limit_mb=2560 -print_final_stats=1]

# How long an input may grow, and run. libFuzzer starts short and
# lengthens inputs as coverage stalls; at the 50-100 runs a second
# `scenes` manages (every action is a frame), ten minutes never got past
# 17 bytes, a handful of actions. So `scenes` starts at its cap: 512 bytes
# is some fifty actions, a second of frames under ASan at worst. A timeout
# of 10 s an input is far past what a frame takes, and an input that runs
# longer is a hang worth a look - but for `decode`, where a frame just
# under the 512 MiB ceiling is a legitimate image that takes that long to
# decode under ASan.
def len-flags [target: string] {
    match $target {
        "scenes" => [-max_len=512 -len_control=0 -timeout=10]
        "decode" => [-max_len=65536 -timeout=60]
        _ => [-max_len=65536 -timeout=10]
    }
}

def fuzz-dir [] { $env.FILE_PWD | path dirname | path join fuzz }

# The nightly cargo-fuzz builds with: `KUI_FUZZ_TOOLCHAIN` (the `fuzz`
# workflow pins one), or whatever `nightly` is here.
def toolchain [] { $env.KUI_FUZZ_TOOLCHAIN? | default "nightly" }

def check-tools [] {
    if (which cargo-fuzz | is-empty) {
        print -e "cargo-fuzz is not installed: cargo install cargo-fuzz --locked"
        exit 2
    }
}

def check-target [target: string] {
    if $target not-in $TARGETS {
        print -e $"no target ($target); there are: ($TARGETS | str join ', ')"
        exit 2
    }
}

# Runs one target; true when it ended without a crash.
def run-one [target: string, time: int, jobs: int]: nothing -> bool {
    let dir = (fuzz-dir)
    let corpus = ($dir | path join corpus $target)
    mkdir $corpus
    let dirs = ([seeds regressions]
        | each {|k| $dir | path join $k $target }
        | where {|d| $d | path exists })
    let budget = if $time > 0 { [$"-max_total_time=($time)"] } else { [] }
    let par = if $jobs > 1 { [$"-fork=($jobs)" -ignore_crashes=0] } else { [] }
    print $"== ($target)"
    cd ($dir | path dirname)
    let start = (date now)
    # Streamed, not captured: a run is minutes of libFuzzer's status lines.
    let code = try {
        ^cargo $"+(toolchain)" fuzz run --fuzz-dir $dir --features libfuzzer $target $corpus ...$dirs -- ...$LIBFUZZER ...(len-flags $target) ...$budget ...$par
        0
    } catch {
        $env.LAST_EXIT_CODE
    }
    if $code == 0 {
        return true
    }
    # A crash is a file libFuzzer wrote under artifacts/; anything else
    # that failed is the build or the tool. Named again here, last, where a
    # CI log is read.
    let artifacts = ($dir | path join artifacts $target)
    let found = if ($artifacts | path exists) {
        ls $artifacts | where modified >= $start | get name
    } else {
        []
    }
    if ($found | is-empty) {
        print -e $"($target): exited ($code) with no artifact - a build or tool failure"
        exit 2
    }
    $found | each {|f| print -e $"($target): crashed, input at ($f)" } | ignore
    false
}

# Lists the targets.
def "main list" [] {
    $TARGETS | each {|t| print $t } | ignore
}

# Runs one target: until it crashes, or for --time seconds.
def "main run" [
    target: string
    --time: int = 0     # seconds; 0 runs until a crash or Ctrl-C
    --jobs (-j): int = 1  # libFuzzer workers (-fork)
] {
    check-tools
    check-target $target
    if not (run-one $target $time $jobs) { exit 1 }
}

# Runs every target in turn, --time seconds each.
def "main all" [
    --time: int = 300
    --jobs (-j): int = 1
] {
    check-tools
    let crashed = ($TARGETS | where {|t| not (run-one $t $time $jobs) })
    if ($crashed | is-not-empty) {
        print -e $"crashed: ($crashed | str join ', ')"
        exit 1
    }
}

# Minimizes a crash and keeps it as a regression.
def "main keep" [
    target: string
    artifact: path   # the crash file libFuzzer wrote
    name: string     # the file's name under regressions/<target>, e.g. the backlog item
] {
    check-tools
    check-target $target
    let artifact = ($artifact | path expand)
    let dir = (fuzz-dir)
    cd ($dir | path dirname)
    let r = (do {
        ^cargo $"+(toolchain)" fuzz tmin --fuzz-dir $dir --features libfuzzer $target $artifact
    } | complete)
    # tmin writes `minimized-from-<hash>` beside the crash and names it
    # (after "Minimized artifact:", and again in its "Reproduce with").
    let found = ($r.stdout + $r.stderr | parse -r '(?<path>\S*minimized-from-[0-9a-f]+)' | get path | uniq)
    let small = if ($found | is-empty) {
        print -e "tmin did not minimize it; keeping the artifact as found"
        $artifact
    } else {
        $found | last
    }
    let out = ($dir | path join regressions $target)
    mkdir $out
    let kept = ($out | path join $name)
    cp $small $kept
    print $"kept ($kept) \((ls $kept | get 0.size)\)"
}

def main [] {
    print "nu scripts/fuzz.nu list | run <target> [--time s] | all [--time s] | keep <target> <artifact> <name>"
}
