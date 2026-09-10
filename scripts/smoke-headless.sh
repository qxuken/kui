#!/usr/bin/env bash
# The headless smoke round: every example that declares a self-check, run
# with `--headless`, judged by its exit code (docs/adr/0021, decision 4).
#
#   scripts/smoke-headless.sh          # print the round
#   scripts/smoke-headless.sh --run    # run it (what CI does)
#
# Who is in it is read, not written: each crate's Cargo.toml lists its
# examples with a drive under `[package.metadata.kui] headless = [...]`
# (a test in examples/devtools pins every name there to an `[[example]]`),
# and examples/node/package.json's `smoke` script is the Node half. An
# example listed without a drive exits 2 with "no headless drive", so a
# name added here before its drive goes red rather than passing quietly.
#
# The C round is examples/c/build.sh --run's, and stays there: it builds
# what it runs.

set -u
cd "$(dirname "$0")/.."

run=0
case "${1:-}" in
    --run) run=1 ;;
    "") ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown flag $1" >&2; exit 2 ;;
esac

# `crate example` pairs, from the manifests.
pairs=$(cargo metadata --format-version 1 --no-deps | python3 -c '
import json, sys
meta = json.load(sys.stdin)
for p in meta["packages"]:
    names = (p.get("metadata") or {}).get("kui", {}).get("headless", [])
    for n in names:
        print(p["name"], n)
')

echo "the headless round:"
echo "$pairs" | while read -r crate name; do
    echo "  cargo run -p $crate --example $name -- --headless"
done
echo "  (cd examples/node && npm run smoke)"
if [ "$run" = 0 ]; then
    echo
    echo "  # pass --run to have this script run them"
    exit 0
fi

echo
failed=""
# Build first, so a compile error is one message and not one per example.
crates=$(echo "$pairs" | awk '{print $1}' | sort -u)
for crate in $crates; do
    cargo build -p "$crate" --examples || exit 1
done
while read -r crate name; do
    printf '  %-12s %-14s ' "$crate" "$name"
    out=$(cargo run -q -p "$crate" --example "$name" -- --headless 2>&1)
    code=$?
    if [ "$code" -eq 0 ]; then
        echo "ok"
    else
        echo "FAILED (exit $code)"
        echo "$out" | tail -8 | sed 's/^/                             /'
        failed="$failed $crate/$name"
    fi
done <<EOF
$pairs
EOF

printf '  %-12s %-14s ' node smoke
if (cd examples/node && npm run smoke >/tmp/kui-node-smoke.log 2>&1); then
    echo "ok"
else
    echo "FAILED"
    tail -12 /tmp/kui-node-smoke.log | sed 's/^/                             /'
    failed="$failed node/smoke"
fi

echo
if [ -n "$failed" ]; then
    echo "FAILED:$failed"
    exit 1
fi
echo "the headless round passed"
