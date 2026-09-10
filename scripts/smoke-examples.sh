#!/usr/bin/env bash
# The windowed smoke round on a unix host: every example of the `kui` crate
# opened for real, on the machine's own GPU, for a number of frames, on
# both theme bases — the twin of scripts/smoke-windows.ps1, same contract.
#
#   scripts/smoke-examples.sh                 # every example, both bases
#   scripts/smoke-examples.sh --frames 300    # longer, for pacing bugs
#   scripts/smoke-examples.sh --only fragment,enter_exit
#   scripts/smoke-examples.sh --release       # the profile that ships
#   scripts/smoke-examples.sh --base light    # one base only
#   scripts/smoke-examples.sh --node          # the Node windowed examples too
#
# How it works. `KUI_SMOKE_FRAMES=n` (crates/kui/src/lib.rs) makes the
# runner quit once the main window has presented n frames, so an example is
# a self-terminating check: exit 0 means it drew n frames and shut down,
# and anything else — a wgpu validation panic, a device loss, a hang — is
# a failure with the stderr to read. A frame only counts once a present
# has succeeded, so an example that opens a window and never paints runs
# out the timeout rather than passing quietly.
#
# Every example runs inside the harness (examples/harness, ADR 0021), so
# `--light` and `--dark` pin the theme base without the example knowing:
# each one is opened twice, and a literal colour that reads on one base
# and not the other is opened on both, every run. The dock is on, in its
# default place, so what is smoked is what `cargo run --example` shows.
#
# The dev profile honours `KUI_SMOKE_FRAMES` unasked; a release build only
# with `--features smoke`, which `--release` passes — an app you ship should
# not close its own window over a variable in its environment. `--node`
# adds the Node windowed examples (examples/node) on the same contract: it
# builds the addon in release with kui-node's `smoke` feature, since that
# is the build `native.cjs` loads first, then opens each example the way
# `npm run <name>` does.
#
# What it does not cover: anything needing a human, and what was drawn.
# `cargo test --workspace` checks the pixels, through the conformance
# corpus; this checks that drawing them did not fail.
#
# Exit codes: 0 every example passed, 1 at least one failed.

set -u
cd "$(dirname "$0")/.."

frames=120
timeout_sec=60
only=""
profile=dev
bases="light dark"
node=0
while [ $# -gt 0 ]; do
    case "$1" in
        --frames) frames="$2"; shift 2 ;;
        --timeout) timeout_sec="$2"; shift 2 ;;
        --only) only="$2"; shift 2 ;;
        --release) profile=release; shift ;;
        --base) bases="$2"; shift 2 ;;
        --node) node=1; shift ;;
        -h|--help) sed -n '2,38p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown flag $1" >&2; exit 2 ;;
    esac
done

# Every example in crates/kui, read from its manifest so a new one is
# smoked the day it is added. The other crates' examples are the C and
# Lua panel hosts (their rounds are examples/c/build.sh's) and kui-core's
# corpus dump, which opens no window.
examples=$(cargo metadata --format-version 1 --no-deps \
    | python3 -c '
import json, sys
meta = json.load(sys.stdin)
for p in meta["packages"]:
    if p["name"] == "kui":
        for t in p["targets"]:
            if "example" in t["kind"]:
                print(t["name"])
' | sort)
if [ -n "$only" ]; then
    wanted=$(echo "$only" | tr ',' '\n')
    for w in $wanted; do
        echo "$examples" | grep -qx "$w" || { echo "no such example: $w" >&2; exit 2; }
    done
    examples="$wanted"
fi

# bash 3 (macOS's) has no empty-array expansion under `set -u`, so the
# feature flag is a string.
features=""
if [ "$profile" = release ]; then
    features="--features smoke"
    dir=target/release/examples
else
    dir=target/debug/examples
fi

echo "building $profile examples..."
# shellcheck disable=SC2086
cargo build --profile "$profile" -p kui --examples $features || exit 1

count=$(echo "$examples" | wc -l | tr -d ' ')
echo
echo "smoke: $count examples × ($bases), $frames frames each, $profile profile"
echo

failed=""
# One run: `smoke <name> <base> <command...>`. The command is started with
# KUI_SMOKE_FRAMES set and `--<base>` appended, and judged by its exit.
smoke() {
    local name=$1 base=$2
    shift 2
    label=$(printf '%-14s %-5s' "$name" "$base")
    err=$(mktemp)
    start=$(date +%s.%N 2>/dev/null || date +%s)
    # stdout is nobody's: what an example prints is not what is judged.
    KUI_SMOKE_FRAMES="$frames" "$@" "--$base" >/dev/null 2>"$err" &
    pid=$!
        # A hang is a failure too: wait up to the timeout, then kill.
        waited=0
        while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$((timeout_sec * 10))" ]; do
            sleep 0.1
            waited=$((waited + 1))
        done
        if kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null
            wait "$pid" 2>/dev/null
            echo "  $label HUNG     ${timeout_sec}s"
            failed="$failed $name/$base"
            rm -f "$err"
            return
        fi
        wait "$pid"
        code=$?
        end=$(date +%s.%N 2>/dev/null || date +%s)
        secs=$(python3 -c "print(f'{$end - $start:.2f}')" 2>/dev/null || echo "?")
        # kui prints one line per misconfiguration a frame noticed. Not a
        # crash, but an example of all things should not produce them.
        warnings=$(grep -c '^kui: warning' "$err" || true)
        if [ "$code" -ne 0 ]; then
            echo "  $label FAILED   exit $code"
            head -12 "$err" | sed 's/^/                       /'
            failed="$failed $name/$base"
        else
            note=""
            [ "$warnings" -gt 0 ] && note=" ($warnings warning(s))"
            echo "  $label ok       ${secs}s$note"
            grep '^kui: warning' "$err" | sed 's/^/                       /'
        fi
        rm -f "$err"
}

for name in $examples; do
    for base in $bases; do
        exe="$dir/$name"
        if [ ! -x "$exe" ]; then
            echo "  $(printf '%-14s %-5s' "$name" "$base") MISSING  $exe"
            failed="$failed $name/$base"
            continue
        fi
        smoke "$name" "$base" "$exe"
    done
done

if [ "$node" = 1 ]; then
    echo
    echo "node: building the addon with kui-node's smoke feature, then the examples..."
    cargo build -p kui-node --release --features smoke || exit 1
    (cd examples/node && npm run build >/dev/null 2>&1) || exit 1
    # The windowed Node examples, by the path `npm run <name>` opens; the
    # headless-only tool is not a window.
    for entry in apps/counter features/window features/slide widgets/virtual_list; do
        for base in $bases; do
            smoke "node:${entry#*/}" "$base" node "examples/node/dist/$entry.mjs"
        done
    done
fi

echo
if [ -n "$failed" ]; then
    echo "FAILED:$failed"
    exit 1
fi
echo "all $count examples drew $frames frames on each base and exited cleanly"
