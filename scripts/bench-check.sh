#!/usr/bin/env bash
# Benches HEAD against a base ref (default: the latest v* tag reachable from
# HEAD) on this machine, back to back, and fails when one of the guarded
# frame benches got slower than the noise floor allows. This is the guard
# C15 asked for; it is not in CI on purpose (the docker runner is too weak
# and a check that false-fails gets disabled), so it sits on the pre-tag run
# list instead - see docs/BACKLOG.md.
#
#   scripts/bench-check.sh [base-ref] [divan filter...]
#   KUI_BENCH=stream scripts/bench-check.sh [base-ref] [divan filter...]
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
# benching takes minutes a loaded machine will waste.
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
set -euo pipefail
cd "$(dirname "$0")/.."

tolerance="${KUI_BENCH_TOLERANCE:-10}"
bench="${KUI_BENCH:-frame}"
bench_file=crates/kui-core/benches/$bench.rs
base_dir=target/bench-base
out_dir=target/bench-check
if [ ! -f "$bench_file" ]; then
  echo "bench-check: no such bench: $bench_file (KUI_BENCH names a file under crates/kui-core/benches/)" >&2
  exit 2
fi

head_sha=$(git rev-parse HEAD)
if [ $# -gt 0 ] && [ "$1" != "--" ]; then
  base="$1"; shift
else
  if [ $# -gt 0 ]; then shift; fi   # drop the leading "--"
  base=$(git describe --tags --match 'v*' --abbrev=0 HEAD)
  # At a tag, "the previous tag" is the one before it, not itself.
  if [ "$(git rev-parse "$base^{commit}")" = "$head_sha" ]; then
    base=$(git describe --tags --match 'v*' --abbrev=0 "$base^")
  fi
fi
base_sha=$(git rev-parse --verify "$base^{commit}")
if [ "$base_sha" = "$head_sha" ]; then
  echo "bench-check: base $base is HEAD; nothing to compare" >&2
  exit 2
fi
if ! git diff --quiet HEAD -- crates/kui-core; then
  echo "bench-check: note: crates/kui-core has uncommitted changes; HEAD's side benches the working tree" >&2
fi

echo "bench-check: $bench: HEAD ${head_sha:0:7} against $base (${base_sha:0:7}), tolerance ${tolerance}% on the guarded rows"

# Said before the benching rather than after it, because a loaded machine
# will spend the benching minutes earning nothing. This sums the CPU of what
# is running *now* rather than reading the load average, which is a
# one-minute decay: a box that has just gone quiet still reports a load in
# the tens, so the average both cries wolf and misses a lull that is long
# enough to bench in. Advisory either way - what actually decides a run is
# the two-runs-per-side spread the table reports. Git for Windows' `ps`
# has no `-o`, and under `set -eo pipefail` that failure ended the run
# before it benched (the alpha.12 Windows round): the `|| true` keeps an
# advisory advisory.
cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 8)
busy=$( (ps -Ao %cpu= 2>/dev/null || true) | awk '$1 > 15 { s += $1 } END { printf "%.0f", s + 0 }')
if [ -n "$busy" ] && awk -v b="$busy" -v c="$cpus" 'BEGIN { exit !(b > c * 25) }'; then
  echo "bench-check: WARNING: other processes are using ~${busy}% CPU of ${cpus}00% available. These medians will be noise. Close what else is running first." >&2
fi

mkdir -p "$out_dir"
if [ -e "$base_dir/.git" ]; then
  # Reuse it: the point of keeping the worktree is keeping its build cache,
  # which is most of a run's wall clock.
  git -C "$base_dir" checkout --quiet --detach --force "$base_sha"
else
  # It is gone or half-made - most likely `cargo clean` took it, since it
  # lives under target/. Drop what is left and remake it. `prune` is
  # repo-wide, but it only forgets worktrees whose directory no longer
  # exists, so a sibling worktree that is still on disk is not touched.
  rm -rf "$base_dir"
  git worktree prune
  git worktree add --quiet --detach "$base_dir" "$base_sha"
fi

bench() { # side dir run [divan filter...]
  local side=$1 dir=$2 run=$3
  shift 3
  echo "bench-check: $side, run $run"
  (cd "$dir" && cargo bench --quiet -p kui-core --bench "$bench" -- --color never "$@") \
    > "$out_dir/$side-$run.txt" 2>&1 \
    || { cat "$out_dir/$side-$run.txt"; echo "bench-check: $side run $run failed" >&2; exit 1; }
}

echo "bench-check: building both sides"
(cd "$base_dir" && cargo bench --quiet -p kui-core --bench "$bench" --no-run)
cargo bench --quiet -p kui-core --bench "$bench" --no-run

bench base "$base_dir" 1 "$@"
bench head . 1 "$@"
bench base "$base_dir" 2 "$@"
bench head . 2 "$@"

git show "$base_sha:$bench_file" > "$out_dir/$bench.rs.base"
if git diff --quiet "$base_sha" HEAD -- "$bench_file"; then
  bench_file_differs=0
else
  bench_file_differs=1
fi

# Machine line, in the README's shape.
os=$(uname -sr)
command -v sw_vers >/dev/null 2>&1 && os="macOS $(sw_vers -productVersion)"
machine_line="measured $(date +%F) on $(uname -m), $os, $(rustc -V | cut -d' ' -f1-2), release, steady-state warm caches"

node - "$out_dir" "$base" "$tolerance" "$bench_file_differs" "$bench_file" "$machine_line" "$bench" <<'EOF'
const fs = require("fs");
const [outDir, baseName, tolArg, differsArg, benchFile, machineLine, bench] = process.argv.slice(2);
const tolerance = Number(tolArg);
const differs = differsArg === "1";

// The guarded rows, per bench file. Only `frame` has any: the other
// benches are read, not judged.
const GUARDED_BY_BENCH = {
  frame: [
    "frame_10k_rects",
    "frame_1k_typical",
    "frame_10k_rects_with_text_and_hits",
    "deep_nesting_64_levels",
    // Ten thousand leaf floats: what the float stack costs per float (ADR
    // 0023), which the four above cannot see - C29 found +12% here while
    // they read flat.
    "frame_10k_segments",
    // The access tree over 10k nodes: the row 18cf953's cache is justified
    // by, and the virtual list: the row the README's "a list costs a
    // screenful" rests on. Neither was guarded until backlog AR47, and
    // the class C29 found regressing was exactly the unguarded rows.
    "frame_10k_rects_with_access_tree",
    "list_10k_rows_virtual",
  ],
};
const GUARDED = GUARDED_BY_BENCH[bench] || [];
const UNIT = { ns: 1, "µs": 1e3, us: 1e3, ms: 1e6, s: 1e9 };

// divan's table: "├─ name  fastest │ slowest │ median │ mean │ samples │ iters"
function parse(path) {
  const rows = new Map();
  for (const line of fs.readFileSync(path, "utf8").split("\n")) {
    if (!/^[├╰]─ /.test(line)) continue;
    const cols = line.split("│").map((c) => c.trim());
    const name = cols[0].replace(/^[├╰]─\s+/, "").split(/\s+/)[0];
    const m = /^([\d.]+)\s*(ns|µs|us|ms|s)$/.exec(cols[2] || "");
    if (!name || !m) {
      console.error(`bench-check: cannot read a median from: ${line}`);
      process.exit(2);
    }
    rows.set(name, Number(m[1]) * UNIT[m[2]]);
  }
  if (rows.size === 0) {
    console.error(`bench-check: no bench rows in ${path}`);
    process.exit(2);
  }
  return rows;
}
const base1 = parse(`${outDir}/base-1.txt`), base2 = parse(`${outDir}/base-2.txt`);
const head1 = parse(`${outDir}/head-1.txt`), head2 = parse(`${outDir}/head-2.txt`);

// The README's style: ~3 significant digits in the unit that keeps the
// number under 1000.
function fmt(ns) {
  const [v, u] = ns >= 1e9 ? [ns / 1e9, "s"] : ns >= 1e6 ? [ns / 1e6, "ms"] : ns >= 1e3 ? [ns / 1e3, "µs"] : [ns, "ns"];
  return `${v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v.toFixed(2)} ${u}`;
}
const pct = (from, to) => ((to - from) / from) * 100;
const signed = (p) => `${p >= 0 ? "+" : ""}${p.toFixed(1)}%`;

// -- Which rows still build from the same source ---------------------------
// Top-level items of the bench file (fn / struct / impl / const), with
// comments and attributes stripped so a doc edit is not a change. A row is
// "identical" when its own function and every item it reaches, transitively,
// read the same at both refs.
function items(src) {
  const out = new Map();
  let name = null, body = [];
  const flush = () => { if (name) out.set(name, (out.get(name) || "") + body.join("\n")); };
  for (const raw of src.split("\n")) {
    const line = raw.replace(/\/\/.*$/, "").trimEnd();
    const m = /^(?:pub\s+)?(?:fn|struct|impl|const)\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(raw);
    if (m) { flush(); name = m[1]; body = []; }
    if (!name || /^\s*#\[/.test(line) || line === "") continue;
    body.push(line);
  }
  flush();
  return out;
}
function reaches(all, start) {
  const seen = new Set([start]);
  const queue = [start];
  while (queue.length) {
    const body = all.get(queue.pop()) || "";
    for (const id of body.match(/[A-Za-z_][A-Za-z0-9_]*/g) || []) {
      if (all.has(id) && !seen.has(id)) { seen.add(id); queue.push(id); }
    }
  }
  return seen;
}
const headItems = items(fs.readFileSync(benchFile, "utf8"));
const baseItems = items(fs.readFileSync(`${outDir}/${bench}.rs.base`, "utf8"));
const changedItems = new Set();
for (const [n, body] of headItems) if (baseItems.get(n) !== body) changedItems.add(n);
for (const n of baseItems.keys()) if (!headItems.has(n)) changedItems.add(n);
const touchedBy = (row) => [...reaches(headItems, row)].filter((n) => changedItems.has(n)).sort();

// -- Comparison ------------------------------------------------------------
const both = [...head2.keys()].filter((n) => base2.has(n));
const headOnly = [...head2.keys()].filter((n) => !base2.has(n));
const baseOnly = [...base2.keys()].filter((n) => !head2.has(n));
const order = (a, b) => (GUARDED.includes(b) - GUARDED.includes(a)) || a.localeCompare(b);

console.log("");
console.log(`| bench | ${baseName} | HEAD | change | run-to-run | source |`);
console.log("|---|---|---|---|---|---|");
const failures = [];
const unreadable = [];
let noise = 0;
for (const name of both.sort(order)) {
  const b = base2.get(name), h = head2.get(name);
  const change = pct(b, h);
  // Warm-up run against read run, per side: this machine disagreeing with
  // itself. A row the warm-up run does not have cannot say, and must not
  // silently read as zero noise.
  const spreads = [[base1.get(name), b], [head1.get(name), h]]
    .filter(([first]) => first !== undefined)
    .map(([first, second]) => Math.abs(pct(first, second)));
  const spread = spreads.length === 2 ? Math.max(...spreads) : null;
  const touched = differs ? touchedBy(name) : [];
  const guarded = GUARDED.includes(name);
  if (guarded && spread !== null) noise = Math.max(noise, spread);
  // Judged per row, not per run. A row whose own two runs agree to within
  // the tolerance can carry a verdict even if a neighbouring row was
  // hiccuped by the scheduler; a row that disagrees with itself by more
  // than the difference we are looking for cannot resolve that difference,
  // so it reports as unreadable rather than as either answer. The cheap
  // rows are the jittery ones - `deep_nesting_64_levels` is ~80 µs, where
  // one preemption is 20% - and letting them veto the whole table would
  // throw away three good verdicts to buy nothing.
  let verdict = "";
  if (guarded) {
    if (spread === null || spread > tolerance) { unreadable.push(name); verdict = " unreadable"; }
    else if (change > tolerance) { failures.push(name); verdict = " **slower**"; }
    else verdict = " ok";
  }
  const source = touched.length ? `touched: ${touched.map((t) => `\`${t}\``).join(", ")}` : "same";
  console.log(`| \`${name}\`${guarded ? " (guarded)" : ""} | ${fmt(b)} | ${fmt(h)} | ${signed(change)}${verdict} | ${spread === null ? "—" : `±${spread.toFixed(1)}%`} | ${source} |`);
}
console.log("");
if (headOnly.length) console.log(`Only at HEAD, not compared: ${headOnly.sort().map((n) => `\`${n}\``).join(", ")}.`);
if (baseOnly.length) console.log(`Only at ${baseName}, not compared: ${baseOnly.sort().map((n) => `\`${n}\``).join(", ")}.`);
console.log(`Worst run-to-run spread on a guarded row: ${noise.toFixed(1)}%. A row over the ${tolerance}% tolerance there reads "unreadable" and carries no verdict.`);

if (differs) {
  console.log("");
  console.log(`**${benchFile} differs between ${baseName} and HEAD.** Changed items: ${[...changedItems].sort().map((n) => `\`${n}\``).join(", ") || "none beyond comments"}.`);
  console.log(`A row marked "same" above builds from source that reads the same at both refs. A "touched" row does not, and its change is only a regression if the tree it builds is still the same tree: for a grid row that means every \`Grid\` switch the row turns on existed at ${baseName} and the builder emits the same nodes when they are off. Read \`git diff ${baseName} HEAD -- ${benchFile}\` for those rows before believing their column.`);
}

// -- README table -----------------------------------------------------------
// `\r?`: a CRLF checkout (Windows, autocrlf) left every row unmatched
// against the `$` below and printed "(not in the README yet)" for all of
// them, in the alpha.12 Windows round.
const readme = fs.readFileSync("README.md", "utf8").split(/\r?\n/);
const described = new Map();
for (const line of readme) {
  const m = /^\| `([A-Za-z0-9_]+)` \| (.*) \| ~[^|]+ \|$/.exec(line);
  if (m) described.set(m[1], m[2]);
}
console.log("");
console.log(`README table at HEAD (\`cargo bench -p kui-core\`, ${machineLine}):`);
console.log("");
console.log("| bench | what it holds | median |");
console.log("|---|---|---|");
const readmeOrder = [...described.keys()].filter((n) => head2.has(n));
const undescribed = [...head2.keys()].filter((n) => !described.has(n)).sort();
for (const name of [...readmeOrder, ...undescribed]) {
  console.log(`| \`${name}\` | ${described.get(name) || "(not in the README yet)"} | ~${fmt(head2.get(name))} |`);
}
const stale = [...described.keys()].filter((n) => !head2.has(n));
if (stale.length) console.log(`\nIn the README but not in this run: ${stale.map((n) => `\`${n}\``).join(", ")}.`);

console.log("");
const missing = GUARDED.filter((n) => !head2.has(n));
if (missing.length) {
  console.log(`bench-check: FAIL - guarded rows missing at HEAD: ${missing.join(", ")}`);
  process.exit(1);
}
const unguarded = GUARDED.filter((n) => !base2.has(n));
if (unguarded.length) console.log(`bench-check: note: guarded rows missing at ${baseName}, so not judged: ${unguarded.join(", ")}`);
// A row that cannot agree with itself cannot answer, and does not pretend
// to. Unreadable rows exit 2, not 1: that is neither a pass nor a
// regression, and calling it a failure is exactly the false-fail that kept
// this check out of CI. A real regression still wins over noise elsewhere,
// so failures are reported first.
if (failures.length === 0 && unreadable.length) {
  console.log(`bench-check: INCONCLUSIVE - ${unreadable.length} of ${GUARDED.length} guarded rows disagreed with themselves by more than the ${tolerance}% a regression has to clear, so they cannot resolve one: ${unreadable.join(", ")}. Something else is using the CPU. Close it and rerun. ${GUARDED.length - unreadable.length ? `The other ${GUARDED.length - unreadable.length} read clean and did not regress.` : ""} (logs in ${outDir}/)`);
  process.exit(2);
}
if (failures.length) {
  console.log(`bench-check: FAIL - more than ${tolerance}% slower than ${baseName}, on rows steady enough to say so: ${failures.join(", ")}${unreadable.length ? `. ${unreadable.join(", ")} was too noisy to read either way` : ""} (logs in ${outDir}/)`);
  process.exit(1);
}
if (GUARDED.length === 0) {
  console.log(`bench-check: read - \`${bench}\` has no guarded rows, so nothing was judged; the table above is the comparison (logs in ${outDir}/)`);
} else {
  console.log(`bench-check: ok - none of the ${GUARDED.length} guarded rows is more than ${tolerance}% slower than ${baseName} (logs in ${outDir}/)`);
}
EOF
