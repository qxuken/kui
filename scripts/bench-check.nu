#!/usr/bin/env nu
# Benches HEAD against a base ref (default: the latest v* tag reachable from
# HEAD) on this machine, back to back, and fails when one of the guarded
# frame benches got slower than the noise floor allows. This is the guard
# C15 asked for; it is not in CI on purpose (the docker runner is too weak
# and a check that false-fails gets disabled), so it sits on the pre-tag run
# list instead - see docs/BACKLOG.md.
#
#   nu scripts/bench-check.nu [base-ref] [divan filter...]
#   KUI_BENCH=stream nu scripts/bench-check.nu [base-ref] [divan filter...]
#
# or `scripts/bench-check.nu ...` on macOS and Linux, through the shebang.
# A leading `--` stands for the default base, so `-- frame_1k` filters
# without naming one; `--help` prints this usage.
#
# It is Nushell, where it was bash with a Node script inside, so that the
# same file runs on macOS, Linux and Windows with nothing but `nu`, git and
# cargo on PATH: no Git for Windows shell, no `ps`/`awk`/`getconf`, and no
# Node (which the bash one needed on PATH just to read divan's table).
#
# KUI_BENCH names the bench file (default `frame`, the one with guarded
# rows; `stream`, `long_line`, `cells`, `editing`, `highlight` are the
# others - every row of those is reported and none judged, since the
# guard list is per bench and only `frame` has one). The README table at
# the end is filled in for whichever rows that bench has.
#
# The base ref is checked out into a worktree under target/bench-base/ (kept
# between runs, so its build cache survives; `git worktree remove
# target/bench-base` drops it). Both sides are built, then run interleaved
# base/HEAD/base/HEAD; the second run of each side is the one read, the
# first is warm-up, and the spread between the two is printed as the noise
# this machine showed. Medians are compared per row. The guarded rows fail
# the script (exit 1) when HEAD is more than $KUI_BENCH_TOLERANCE percent
# slower (default 10: measured back-to-back noise on the M3 Pro is ~3-5%, so
# twice its top); every other row is reported and not judged. Rows only one
# side has are listed, not compared.
#
# Exit codes: 0 no guarded row regressed, 1 one did (or is missing at HEAD),
# 2 the run cannot say. Readability is judged per row, not per run: a row
# whose own two runs disagree by more than the tolerance cannot resolve a
# difference that size, so it reports as unreadable instead of as either
# answer, and the run is INCONCLUSIVE only when no readable row failed. The
# false-fail this avoids is the same false-fail that kept the check out of
# CI. It also warns before starting when other processes are busy, since the
# benching takes minutes a loaded machine will waste. A base ref git cannot
# resolve, or no v* tag to default to, is also 2: the run has nothing to say
# about it. A command the script leans on failing (git, the build) ends it
# with that command's own code, as `set -e` did.
#
# Two tables come out. The comparison, and then the README's table with
# HEAD's medians filled in against the descriptions the README already
# carries, so the run that guards the tag is the run that refreshes the
# numbers.
#
# The trap this also watches for: the bench *builder* can change between
# refs (f6eec64 unified the grid), and then a row's median is not comparable
# even though the row's name is. When crates/kui-core/benches/frame.rs
# differs, the script says which rows still build from unchanged source -
# the bench function and every helper it reaches - and which are touched,
# and by what. A touched row is not a failure; it is a row to read the diff
# for. For the grid rows the rule is: a plain grid has every `Grid` switch
# off, so a builder change that only adds a switch leaves it comparable,
# while a new switch that is on in the row does not.

const ROOT = (path self | path dirname | path dirname)

# The guarded rows, per bench file. Only `frame` has any: the other
# benches are read, not judged.
const GUARDED_BY_BENCH = {
    frame: [
        frame_10k_rects
        frame_1k_typical
        frame_10k_rects_with_text_and_hits
        deep_nesting_64_levels
        # Ten thousand leaf floats: what the float stack costs per float (ADR
        # 0023), which the four above cannot see - C29 found +12% here while
        # they read flat.
        frame_10k_segments
        # The access tree over 10k nodes: the row 18cf953's cache is justified
        # by, and the virtual list: the row the README's "a list costs a
        # screenful" rests on. Neither was guarded until backlog AR47, and
        # the class C29 found regressing was exactly the unguarded rows.
        frame_10k_rects_with_access_tree
        list_10k_rows_virtual
        # The segment emitter's inner loop: C41 found it +10% for three
        # alpha tags, a register spill in `emit_node` that the drop-zone
        # commit caused without touching the loop, while every row above
        # read flat.
        frame_1k_curves
    ]
}

const UNIT = {ns: 1.0, "µs": 1e3, us: 1e3, ms: 1e6, s: 1e9}

const USAGE = "usage: nu scripts/bench-check.nu [base-ref] [divan filter...]
       KUI_BENCH=stream nu scripts/bench-check.nu [base-ref] [divan filter...]

Benches HEAD against base-ref (default: the latest v* tag before HEAD) and
fails when a guarded row is more than KUI_BENCH_TOLERANCE percent (default
10) slower. Exit 0 ok, 1 regressed, 2 cannot say. See the file's header."

# Runs an external command, and when it fails ends the script with that
# command's exit code - what the bash version got from `set -e`. It also
# names the command, which `set -e` did not: a command that never ran (not
# on PATH) otherwise ends the run with 1 and not a word.
def --wrapped must [cmd: string, ...args: string] {
    try { ^$cmd ...$args } catch {|e|
        let code = $env.LAST_EXIT_CODE
        say-err $"`($cmd) ($args | str join ' ')` failed: ($e.msg)"
        exit (if $code == 0 { 1 } else { $code })
    }
}

# git's trimmed stdout, or null when git fails.
def --wrapped git-out [...args: string] {
    let r = (do { ^git ...$args } | complete)
    if $r.exit_code == 0 { $r.stdout | str trim } else { null }
}

def say-err [msg: string] { print -e $"bench-check: ($msg)" }

def read-text [path: string] { open --raw $path | decode utf-8 }

# One cargo bench run of one side, its output (both streams, interleaved)
# into the log the analysis reads. A failed run prints its log and ends the
# script with 1, as the bash did.
def run-bench [bench: string, logs: string, side: string, dir: string, run: int, filters: list<string>] {
    print $"bench-check: ($side), run ($run)"
    let log = ($logs | path join $"($side)-($run).txt")
    let ok = (try {
        do {
            cd $dir
            ^cargo bench --quiet -p kui-core --bench $bench -- --color never ...$filters o+e> $log
        }
        true
    } catch { false })
    if not $ok {
        print (read-text $log)
        say-err $"($side) run ($run) failed"
        exit 1
    }
}

# divan's table: "├─ name  fastest │ slowest │ median │ mean │ samples │ iters"
def parse-divan [path: string] {
    mut rows = {}
    for line in (read-text $path | lines) {
        if not ($line =~ '^[├╰]─ ') { continue }
        let cols = ($line | split row "│" | str trim)
        let name = ($cols.0 | str replace --regex '^[├╰]─\s+' '' | split row --regex '\s+' | first)
        let m = ($cols.2? | default "" | parse --regex '^(?<v>[\d.]+)\s*(?<u>ns|µs|us|ms|s)$')
        if ($name | is-empty) or ($m | is-empty) {
            say-err $"cannot read a median from: ($line)"
            exit 2
        }
        $rows = ($rows | upsert $name (($m.0.v | into float) * ($UNIT | get $m.0.u)))
    }
    if ($rows | is-empty) {
        say-err $"no bench rows in ($path)"
        exit 2
    }
    $rows
}

# JavaScript's `toFixed`, which the README's numbers were written with:
# it rounds the double's exact value, an exact half goes up, and a negative
# that rounds to zero keeps its sign ("-0.0"). Rust's `{:.1}` behind `into
# string --decimals` is exact too but sends a half to even, which would put
# 80.25 µs down as 80.2 where every earlier table said 80.3. Scaling and
# `math round` is no answer either: 2.675 is stored as 2.67499999..., which
# toFixed reads as 2.67, but times 100 it rounds to 267.5 and then up. So
# Rust rounds, and only an exact half - the exact expansion ending in a 5 at
# the next place, seen 30 places out - is taken one up by hand.
def fixed [v: float, digits: int] {
    let mag = ($v | math abs)
    let exact = ($mag | into string --decimals ($digits + 30))
    let cut = ($exact | str length) - 30
    let tie = ($exact | str substring $cut.. | str trim --right --char '0') == "5"
    let text = if not $tie { $mag | into string --decimals $digits } else {
        let n = ($exact | str substring 0..<$cut | str replace '.' '' | into int) + 1
        let s = ($n | into string | fill --alignment right --character '0' --width ($digits + 1))
        let int_len = ($s | str length) - $digits
        if $digits == 0 { $s } else { $"($s | str substring 0..<$int_len).($s | str substring $int_len..)" }
    }
    if $v < 0 { $"-($text)" } else { $text }
}

# The README's style: ~3 significant digits in the unit that keeps the
# number under 1000.
def fmt [ns: float] {
    let vu = if $ns >= 1e9 { [($ns / 1e9) s] } else if $ns >= 1e6 { [($ns / 1e6) ms] } else if $ns >= 1e3 { [($ns / 1e3) "µs"] } else { [$ns ns] }
    let v = $vu.0
    let digits = if $v >= 100 { 0 } else if $v >= 10 { 1 } else { 2 }
    $"(fixed $v $digits) ($vu.1)"
}

def pct [from: float, to: float] { ($to - $from) / $from * 100 }

def signed [p: float] { $"(if $p >= 0 { '+' } else { '' })(fixed $p 1)%" }

def ticked [names: list<string>] { $names | each {|n| $"`($n)`" } | str join ", " }

# The order the comparison lists rows in: the bash version sorted with
# Node's `localeCompare`, which puts `_` before digits and letters where a
# plain string sort puts it between them. The names are lowercase
# identifiers, so swapping `_` for a space (below both) reproduces it.
def locale-key [name: string] { $name | str lowercase | str replace --all '_' ' ' }

# -- Which rows still build from the same source ---------------------------
# Top-level items of the bench file (fn / struct / impl / const), with
# comments and attributes stripped so a doc edit is not a change. A row is
# "identical" when its own function and every item it reaches, transitively,
# read the same at both refs. A struct and its impl share a name, and their
# bodies are joined under it, in file order.
def bench-items [src: string] {
    mut out = {}
    mut name = ""
    mut body = []
    for raw in ($src | lines) {
        let line = ($raw | str replace --regex '//.*$' '' | str trim --right)
        let m = ($raw | parse --regex '^(?:pub\s+)?(?:fn|struct|impl|const)\s+(?<n>[A-Za-z_][A-Za-z0-9_]*)')
        if ($m | is-not-empty) {
            if $name != "" {
                $out = ($out | upsert $name (($out | get -o $name | default "") + ($body | str join "\n")))
            }
            $name = $m.0.n
            $body = []
        }
        if $name == "" or ($line =~ '^\s*#\[') or $line == "" { continue }
        $body = ($body | append $line)
    }
    if $name != "" {
        $out = ($out | upsert $name (($out | get -o $name | default "") + ($body | str join "\n")))
    }
    $out
}

# Every item `start` reaches through the identifiers in its body, itself
# included. `refs` maps an item to the identifiers its body names.
def reaches [refs: record, start: string] {
    mut seen = [$start]
    mut queue = [$start]
    while ($queue | is-not-empty) {
        let current = ($queue | last)
        $queue = ($queue | drop)
        for id in ($refs | get -o $current | default []) {
            if ($id in $refs) and not ($id in $seen) {
                $seen = ($seen | append $id)
                $queue = ($queue | append $id)
            }
        }
    }
    $seen
}

def --wrapped main [...args: string] {
    if ($args.0? | default "") in ["--help" "-h"] {
        print $USAGE
        exit 0
    }
    cd $ROOT

    # `:-` in bash: set-but-empty counts as unset.
    let tolerance_text = ($env.KUI_BENCH_TOLERANCE? | default "" | str trim | if ($in | is-empty) { "10" } else { $in })
    let tolerance = (try { $tolerance_text | into float } catch {
        say-err $"KUI_BENCH_TOLERANCE is not a number: ($tolerance_text)"
        exit 2
    })
    let bench = ($env.KUI_BENCH? | default "" | if ($in | is-empty) { "frame" } else { $in })
    let bench_file = $"crates/kui-core/benches/($bench).rs"
    let base_dir = "target/bench-base"
    let out_dir = "target/bench-check"
    let logs = ($ROOT | path join $out_dir)
    if not ($bench_file | path exists) {
        say-err $"no such bench: ($bench_file) \(KUI_BENCH names a file under crates/kui-core/benches/\)"
        exit 2
    }

    let head_sha = (git-out rev-parse HEAD)
    mut filters = $args
    mut base = ""
    if ($filters | is-not-empty) and ($filters.0 != "--") {
        $base = $filters.0
        $filters = ($filters | skip 1)
    } else {
        if ($filters | is-not-empty) { $filters = ($filters | skip 1) } # drop the leading "--"
        let latest = (git-out describe --tags --match 'v*' --abbrev=0 HEAD)
        if $latest == null {
            say-err "no v* tag reachable from HEAD to default to; name a base ref"
            exit 2
        }
        $base = $latest
        # At a tag, "the previous tag" is the one before it, not itself.
        if (git-out rev-parse $"($base)^{commit}") == $head_sha {
            let previous = (git-out describe --tags --match 'v*' --abbrev=0 $"($base)^")
            if $previous == null {
                say-err $"HEAD is ($base) and no v* tag comes before it; name a base ref"
                exit 2
            }
            $base = $previous
        }
    }
    let base = $base
    let filters = $filters
    let base_sha = (git-out rev-parse --verify --quiet $"($base)^{commit}")
    if $base_sha == null {
        say-err $"no such ref: ($base) \(the first argument is the base ref; `--` before divan filters keeps the default\)"
        exit 2
    }
    if $base_sha == $head_sha {
        say-err $"base ($base) is HEAD; nothing to compare"
        exit 2
    }
    if (do { ^git diff --quiet HEAD -- crates/kui-core } | complete).exit_code != 0 {
        say-err "note: crates/kui-core has uncommitted changes; HEAD's side benches the working tree"
    }

    print $"bench-check: ($bench): HEAD ($head_sha | str substring 0..6) against ($base) \(($base_sha | str substring 0..6)\), tolerance ($tolerance_text)% on the guarded rows"

    # Said before the benching rather than after it, because a loaded machine
    # will spend the benching minutes earning nothing. This sums the CPU of
    # what is running *now* rather than reading the load average, which is a
    # one-minute decay: a box that has just gone quiet still reports a load
    # in the tens, so the average both cries wolf and misses a lull that is
    # long enough to bench in. Advisory either way - what actually decides a
    # run is the two-runs-per-side spread the table reports. The bash read
    # `ps -Ao %cpu`, which Git for Windows' `ps` does not have, and under
    # `set -eo pipefail` that failure ended the run before it benched (the
    # alpha.12 Windows round), so it needed an `|| true`; nu's own `ps` and
    # `sys cpu` are the same on every platform, and the `try` only keeps an
    # advisory advisory. Nu's `ps` samples over a moment, so "now" is closer
    # to now than `ps`'s decaying %cpu was, but on macOS it cannot see other
    # users' processes (WindowServer, root daemons) - the builds and apps
    # this warns about are the user's own.
    let cpus = (try { sys cpu | length } catch { 8 })
    let busy = (try { ps | where cpu > 15 | get cpu | append 0.0 | math sum | math round } catch { null })
    if $busy != null and $busy > $cpus * 25 {
        say-err $"WARNING: other processes are using ~($busy)% CPU of ($cpus)00% available. These medians will be noise. Close what else is running first."
    }

    mkdir $out_dir
    if ($base_dir | path join .git | path exists) {
        # Reuse it: the point of keeping the worktree is keeping its build
        # cache, which is most of a run's wall clock.
        must git -C $base_dir checkout --quiet --detach --force $base_sha
    } else {
        # It is gone or half-made - most likely `cargo clean` took it, since
        # it lives under target/. Drop what is left and remake it. `prune` is
        # repo-wide, but it only forgets worktrees whose directory no longer
        # exists, so a sibling worktree that is still on disk is not touched.
        # `--permanent`: a config with `rm.always_trash` would otherwise put
        # a whole build cache in the trash.
        if ($base_dir | path exists) { rm --recursive --force --permanent $base_dir }
        must git worktree prune
        must git worktree add --quiet --detach $base_dir $base_sha
    }

    print "bench-check: building both sides"
    do {
        cd $base_dir
        must cargo bench --quiet -p kui-core --bench $bench --no-run
    }
    must cargo bench --quiet -p kui-core --bench $bench --no-run

    run-bench $bench $logs base $base_dir 1 $filters
    run-bench $bench $logs head . 1 $filters
    run-bench $bench $logs base $base_dir 2 $filters
    run-bench $bench $logs head . 2 $filters

    let base_src = (do { ^git show $"($base_sha):($bench_file)" } | complete)
    if $base_src.exit_code != 0 {
        print -e $base_src.stderr
        exit $base_src.exit_code
    }
    $base_src.stdout | save --force ($logs | path join $"($bench).rs.base")
    let differs = (do { ^git diff --quiet $base_sha HEAD -- $bench_file } | complete).exit_code != 0

    # Machine line, in the README's shape. `uname -sr` and `sw_vers` were
    # the bash's; nu's `sys host` says "macOS 27.0.1" by itself, and on
    # Windows names Windows where Git for Windows' `uname` said MINGW64_NT.
    let host = (sys host)
    let un = (uname)
    let os = match $nu.os-info.name {
        "macos" => $"macOS ($host.os_version)"
        "windows" => $host.long_os_version
        _ => $"($un.kernel-name) ($un.kernel-release)"
    }
    let rustc = (^rustc -V | str trim | split row ' ' | first 2 | str join ' ')
    let machine_line = $"measured (date now | format date '%F') on ($un.machine), ($os), ($rustc), release, steady-state warm caches"

    let guarded = ($GUARDED_BY_BENCH | get -o $bench | default [])
    let base1 = (parse-divan ($logs | path join base-1.txt))
    let base2 = (parse-divan ($logs | path join base-2.txt))
    let head1 = (parse-divan ($logs | path join head-1.txt))
    let head2 = (parse-divan ($logs | path join head-2.txt))

    # -- Which rows still build from the same source -----------------------
    let head_items = (bench-items (read-text $bench_file))
    let base_items = (bench-items (read-text ($logs | path join $"($bench).rs.base")))
    let changed_items = (
        ($head_items | items {|n, body| if ($base_items | get -o $n) != $body { $n } } | compact)
        | append ($base_items | columns | where {|n| not ($n in $head_items) })
        | uniq
    )
    # Each item's identifiers, read once rather than once per row reached.
    let refs = ($head_items | items {|n, body|
        {name: $n, ids: ($body | parse --regex '(?<id>[A-Za-z_][A-Za-z0-9_]*)' | get -o id | default [] | uniq)}
    } | reduce --fold {} {|it, acc| $acc | upsert $it.name $it.ids })
    let touched_by = {|row| reaches $refs $row | where {|n| $n in $changed_items } | sort }

    # -- Comparison --------------------------------------------------------
    let head_names = ($head2 | columns)
    let base_names = ($base2 | columns)
    let both = ($head_names | where {|n| $n in $base2 })
    let head_only = ($head_names | where {|n| not ($n in $base2) } | sort)
    let base_only = ($base_names | where {|n| not ($n in $head2) } | sort)
    let ordered = (
        ($both | where {|n| $n in $guarded } | sort-by {|n| locale-key $n })
        | append ($both | where {|n| not ($n in $guarded) } | sort-by {|n| locale-key $n })
    )

    print ""
    print $"| bench | ($base) | HEAD | change | run-to-run | source |"
    print "|---|---|---|---|---|---|"
    mut failures = []
    mut unreadable = []
    mut noise = 0.0
    for name in $ordered {
        let b = ($base2 | get $name)
        let h = ($head2 | get $name)
        let change = (pct $b $h)
        # Warm-up run against read run, per side: this machine disagreeing
        # with itself. A row the warm-up run does not have cannot say, and
        # must not silently read as zero noise.
        let spreads = (
            [[($base1 | get -o $name) $b] [($head1 | get -o $name) $h]]
            | where {|pair| $pair.0 != null }
            | each {|pair| pct $pair.0 $pair.1 | math abs }
        )
        let spread = if ($spreads | length) == 2 { $spreads | math max } else { null }
        let touched = if $differs { do $touched_by $name } else { [] }
        let is_guarded = ($name in $guarded)
        if $is_guarded and $spread != null { $noise = ([$noise $spread] | math max) }
        # Judged per row, not per run. A row whose own two runs agree to
        # within the tolerance can carry a verdict even if a neighbouring row
        # was hiccuped by the scheduler; a row that disagrees with itself by
        # more than the difference we are looking for cannot resolve that
        # difference, so it reports as unreadable rather than as either
        # answer. The cheap rows are the jittery ones -
        # `deep_nesting_64_levels` is ~80 µs, where one preemption is 20% -
        # and letting them veto the whole table would throw away three good
        # verdicts to buy nothing.
        mut verdict = ""
        if $is_guarded {
            if $spread == null or $spread > $tolerance {
                $unreadable = ($unreadable | append $name)
                $verdict = " unreadable"
            } else if $change > $tolerance {
                $failures = ($failures | append $name)
                $verdict = " **slower**"
            } else {
                $verdict = " ok"
            }
        }
        let source = if ($touched | is-not-empty) { $"touched: (ticked $touched)" } else { "same" }
        let mark = if $is_guarded { " (guarded)" } else { "" }
        let spread_text = if $spread == null { "—" } else { $"±(fixed $spread 1)%" }
        print $"| `($name)`($mark) | (fmt $b) | (fmt $h) | (signed $change)($verdict) | ($spread_text) | ($source) |"
    }
    let failures = $failures
    let unreadable = $unreadable
    print ""
    if ($head_only | is-not-empty) { print $"Only at HEAD, not compared: (ticked $head_only)." }
    if ($base_only | is-not-empty) { print $"Only at ($base), not compared: (ticked $base_only)." }
    print $"Worst run-to-run spread on a guarded row: (fixed $noise 1)%. A row over the ($tolerance_text)% tolerance there reads \"unreadable\" and carries no verdict."

    if $differs {
        let changed_text = ($changed_items | sort | ticked $in | if ($in | is-empty) { "none beyond comments" } else { $in })
        print ""
        print $"**($bench_file) differs between ($base) and HEAD.** Changed items: ($changed_text)."
        print $"A row marked \"same\" above builds from source that reads the same at both refs. A \"touched\" row does not, and its change is only a regression if the tree it builds is still the same tree: for a grid row that means every `Grid` switch the row turns on existed at ($base) and the builder emits the same nodes when they are off. Read `git diff ($base) HEAD -- ($bench_file)` for those rows before believing their column."
    }

    # -- README table -------------------------------------------------------
    # A CRLF checkout (Windows, autocrlf) left every row unmatched against
    # the regex's `$` in the alpha.12 Windows round, and printed "(not in
    # the README yet)" for all of them; the bash's Node split on `\r?\n` for
    # it. Nu's `lines` drops the `\r` itself.
    mut described = {}
    for line in (read-text README.md | lines) {
        let m = ($line | parse --regex '^\| `(?<name>[A-Za-z0-9_]+)` \| (?<what>.*) \| ~[^|]+ \|$')
        if ($m | is-not-empty) { $described = ($described | upsert $m.0.name $m.0.what) }
    }
    let described = $described
    print ""
    print $"README table at HEAD \(`cargo bench -p kui-core`, ($machine_line)\):"
    print ""
    print "| bench | what it holds | median |"
    print "|---|---|---|"
    let readme_order = ($described | columns | where {|n| $n in $head2 })
    let undescribed = ($head_names | where {|n| not ($n in $described) } | sort)
    for name in ($readme_order | append $undescribed) {
        let what = ($described | get -o $name | default "(not in the README yet)")
        print $"| `($name)` | ($what) | ~(fmt ($head2 | get $name)) |"
    }
    let stale = ($described | columns | where {|n| not ($n in $head2) })
    if ($stale | is-not-empty) { print $"\nIn the README but not in this run: (ticked $stale)." }

    print ""
    let missing = ($guarded | where {|n| not ($n in $head2) })
    if ($missing | is-not-empty) {
        print $"bench-check: FAIL - guarded rows missing at HEAD: ($missing | str join ', ')"
        exit 1
    }
    let unjudged = ($guarded | where {|n| not ($n in $base2) })
    if ($unjudged | is-not-empty) {
        print $"bench-check: note: guarded rows missing at ($base), so not judged: ($unjudged | str join ', ')"
    }
    # A row that cannot agree with itself cannot answer, and does not
    # pretend to. Unreadable rows exit 2, not 1: that is neither a pass nor
    # a regression, and calling it a failure is exactly the false-fail that
    # kept this check out of CI. A real regression still wins over noise
    # elsewhere, so failures are reported first.
    let n_guarded = ($guarded | length)
    let n_unreadable = ($unreadable | length)
    if ($failures | is-empty) and ($unreadable | is-not-empty) {
        let clean = $n_guarded - $n_unreadable
        let others = if $clean != 0 { $"The other ($clean) read clean and did not regress." } else { "" }
        print $"bench-check: INCONCLUSIVE - ($n_unreadable) of ($n_guarded) guarded rows disagreed with themselves by more than the ($tolerance_text)% a regression has to clear, so they cannot resolve one: ($unreadable | str join ', '). Something else is using the CPU. Close it and rerun. ($others) \(logs in ($out_dir)/\)"
        exit 2
    }
    if ($failures | is-not-empty) {
        let noisy = if ($unreadable | is-not-empty) { $". ($unreadable | str join ', ') was too noisy to read either way" } else { "" }
        print $"bench-check: FAIL - more than ($tolerance_text)% slower than ($base), on rows steady enough to say so: ($failures | str join ', ')($noisy) \(logs in ($out_dir)/\)"
        exit 1
    }
    if $n_guarded == 0 {
        print $"bench-check: read - `($bench)` has no guarded rows, so nothing was judged; the table above is the comparison \(logs in ($out_dir)/\)"
    } else {
        print $"bench-check: ok - none of the ($n_guarded) guarded rows is more than ($tolerance_text)% slower than ($base) \(logs in ($out_dir)/\)"
    }
}
