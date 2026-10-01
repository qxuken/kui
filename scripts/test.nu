#!/usr/bin/env nu
# The local test run, in parallel: what `cargo test --workspace --features
# kui-core/conformance` runs, with every test binary started side by side
# instead of one after another. Nushell, so the one script runs on macOS,
# Linux and Windows alike.
#
#   nu scripts/test.nu                       every binary + the doctests
#   nu scripts/test.nu scroll                a filter, handed to each binary
#   nu scripts/test.nu -- --ignored          libtest's own flags after `--`
#   nu scripts/test.nu -p kui-core,kui-lua
#   nu scripts/test.nu --node                Node's `npm test` alongside
#   nu scripts/test.nu -j 4 --no-doc
#
# `cargo test` runs its ~130 binaries in turn, so a run is the sum of them
# and the slowest few (the wgpu coverage mirrors, the corpus) are waited on
# while the other cores idle. Here cargo only builds (`--no-run`); the
# binaries are then run -j at a time (default: the core count), the slowest
# first by the times this script recorded last run (target/test-times.json),
# so the long ones start while the short ones fill in around them. Each
# binary still runs its own tests on threads, in one process, as under
# `cargo test`.
#
# Why not nextest: it runs every test in a process of its own, and a kui
# test's first `Core::new` scans the system's fonts (once a process, see
# `text::new_font_system`), so ~1600 processes pay a scan each - slower
# here than this, measured, for the same parallelism.
#
# A binary runs where `cargo test` would run it - its package's directory,
# with CARGO_MANIFEST_DIR set and the toolchain's libraries on the loader's
# path (a proc-macro's unit tests link libstd dynamically) - and its output
# is kept in target/test-logs/<package>.<target>.log; the logs of whatever
# failed are printed at the end. CI is unchanged: it runs `cargo test
# --workspace`.
#
# Exit codes: 0 everything passed, 1 something failed, 2 the build failed.

const LOGS = "target/test-logs"
const TIMES = "target/test-times.json"

# Seconds since `start`, to the hundredth.
def secs-since [start: datetime] {
    ((date now) - $start) / 1sec | math round --precision 2
}

# The sum of the named capture `n` over every match of `pattern`; 0 for
# none (`math sum` refuses an empty list).
def count [text: string, pattern: string] {
    $text | parse -r $pattern | get n | into int | reduce -f 0 {|n, acc| $acc + $n }
}

# The loader's variable on this platform, with `dirs` in front of what it
# held. Windows finds a DLL on PATH, under whatever case nu spells it
# (`PATH` in 0.115, launched from PowerShell or bash alike): Windows reads
# the name either way, but a record key is exact, and a `Path` beside
# `PATH` is a second variable the child never looks at.
def loader-env [dirs: list<string>] {
    let name = match $nu.os-info.name {
        "macos" => "DYLD_FALLBACK_LIBRARY_PATH"
        "windows" => ($env | columns | where {|c| ($c | str uppercase) == "PATH" } | get -o 0 | default "Path")
        _ => "LD_LIBRARY_PATH"
    }
    let sep = if $nu.os-info.name == "windows" { ";" } else { ":" }
    let held = ($env | get -o $name | default [])
    let held = if ($held | describe | str starts-with "list") { $held } else { $held | split row $sep }
    {$name: ($dirs | append $held | where {|d| $d != "" } | str join $sep)}
}

# One job: runs it, keeps its output, prints its row, says how it went.
def run-job [job: record, loader: record] {
    let start = date now
    let r = with-env ($loader | merge {CARGO_MANIFEST_DIR: $job.cwd}) {
        cd $job.cwd
        ^$job.argv.0 ...($job.argv | skip 1) | complete
    }
    let secs = secs-since $start
    $"($r.stdout)($r.stderr)" | save -f $"($LOGS)/($job.key | str replace -a '::' '.').log"
    if $r.exit_code == 0 {
        print $"  ok      ($secs | fill -a r -w 6)s  ($job.key)"
    } else {
        print $"  FAILED  ($secs | fill -a r -w 6)s  ($job.key) \(exit ($r.exit_code)\)"
    }
    {key: $job.key, code: $r.exit_code, secs: $secs, log: $"($r.stdout)($r.stderr)"}
}

def main [
    --jobs (-j): int            # how many binaries at once (default: the core count)
    --package (-p): string      # packages to test, comma-separated (default: the workspace)
    --node                      # also run Node's `npm test` in packages/kui
    --no-doc                    # leave out the doctests
    ...args: string             # handed to every test binary (a filter; libtest flags after `--`)
] {
    let t0 = date now
    cd ($env.FILE_PWD | path dirname)
    let root = $env.PWD
    let jobs = $jobs | default (sys cpu | length)
    let packages = if $package == null { [] } else { $package | split row "," | str trim }
    let select = if ($packages | is-empty) {
        ["--workspace"]
    } else {
        $packages | each {|p| ["-p" $p] } | flatten
    }
    # A `-p` selection that does not reach kui-core cannot take its feature.
    let wants_core = ($packages | is-empty) or ((^cargo tree ...$select -e normal,dev -i kui-core | complete).exit_code == 0)
    let cargo_args = if $wants_core { $select | append ["--features" "kui-core/conformance"] } else { $select }

    # Build: the binaries' paths come out of cargo's JSON, one per test
    # target. Cargo's own progress and errors go to the terminal; the JSON
    # goes to a file, because nu 0.116 reads an external's stdout inside
    # `try { }` only once it has exited, and cargo blocks on a full pipe
    # (64 KB, a fraction of one workspace build's messages) long before.
    mkdir target
    let json = "target/test-artifacts.json"
    try {
        ^cargo test ...$cargo_args --no-run --message-format=json-render-diagnostics out> $json
    } catch {
        print -e "the build failed"
        exit 2
    }
    mut records = (open --raw $json
        | lines
        | each {|l| $l | from json }
        | where {|m| $m.reason == "compiler-artifact" and ($m.profile?.test? == true) and ($m.executable? != null) }
        | each {|m|
            let dir = ($m.manifest_path | path dirname)
            let kind = ($m.target.kind | first)
            let target = if $kind in [lib proc-macro] { "unittests" } else { $m.target.name }
            {key: $"($dir | path basename)::($target)", cwd: $dir, argv: ([$m.executable] | append $args)}
        })
    if not $no_doc {
        let filter = if ($args | is-empty) { [] } else { ["--"] | append $args }
        $records = ($records | append {
            key: "doctests", cwd: $root, argv: (["cargo" "test"] | append $cargo_args | append "--doc" | append $filter)
        })
    }
    if $node {
        # The addon `native.cjs` picks up (the newest build wins); built
        # here rather than in the pool, so it does not wait on a cargo lock.
        try { ^cargo build -q -p kui-node } catch { print -e "the Node addon failed to build"; exit 2 }
        let npx = if $nu.os-info.name == "windows" { "node.exe" } else { "node" }
        $records = ($records | append {key: "node", cwd: ($root | path join packages kui), argv: [$npx "--test" "test.mjs"]})
    }

    # Slowest first, by last run's times; a binary with none yet goes first.
    let last = if ($TIMES | path exists) { open $TIMES } else { {} }
    let ordered = ($records
        | insert last {|r| $last | get -o $r.key | default 1e9 }
        | sort-by -r last
        | reject last)

    rm -rf $LOGS
    mkdir $LOGS
    let sysroot = (^rustc --print sysroot | str trim)
    let host = (^rustc --print host-tuple | str trim)
    let profile_dir = ($root | path join target debug)
    let loader = (loader-env [
        ($sysroot | path join lib rustlib $host lib)
        ($sysroot | path join lib)
        ($sysroot | path join bin)
        ($profile_dir | path join deps)
        $profile_dir
    ])

    print $"running ($ordered | length) jobs, ($jobs) at a time"
    let results = ($ordered | par-each --threads $jobs {|job| run-job $job $loader })

    # This run's times over the last ones, so a `-p` or filtered run does
    # not forget the binaries it left out.
    let ran = ($results | reduce -f {} {|r, acc| $acc | upsert $r.key $r.secs })
    $last | merge $ran | save -f $TIMES

    let failed = ($results | where code != 0)
    let logs = ($results | get log | str join "\n")
    # Rust's `test result:` lines, Node's `ℹ pass` / `ℹ fail` / `ℹ skipped`.
    let suites = ($logs | parse -r 'test result: \w+\.' | length)
    let passed = (count $logs '(?<n>\d+) passed;') + (count $logs 'ℹ pass (?<n>\d+)')
    let fails = (count $logs '(?<n>\d+) failed;') + (count $logs 'ℹ fail (?<n>\d+)')
    let ignored = (count $logs '(?<n>\d+) ignored;') + (count $logs 'ℹ skipped (?<n>\d+)')

    for f in ($failed | sort-by key) {
        print ""
        print $"==== ($f.key) \(($LOGS)/($f.key | str replace -a '::' '.').log\)"
        # The failures section when there is one, else the end of the log.
        let lines = ($f.log | lines)
        let at = ($lines | enumerate | where item == "failures:" | get -o 0.index)
        if $at != null {
            print ($lines | skip $at | first 120 | str join "\n")
        } else {
            print ($lines | last 40 | str join "\n")
        }
    }
    print ""
    print "slowest:"
    for r in ($results | sort-by -r secs | first 5) {
        print $"  ($r.secs | fill -a r -w 6)s  ($r.key)"
    }
    print ""
    let also = if $node { " and Node's" } else { "" }
    print $"($passed) passed, ($fails) failed, ($ignored) ignored over ($suites) Rust suites($also); (secs-since $t0)s in all"
    if ($failed | is-not-empty) {
        print $"FAILED: ($failed | get key | str join ' ')"
        exit 1
    }
}
