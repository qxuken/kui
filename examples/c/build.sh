#!/usr/bin/env bash
# Builds kui_ffi and the C examples against it, after checking that
# include/kui.h still describes the structs Rust actually lays out.
#
#   ./examples/c/build.sh
#   ./examples/c/build.sh --run       # and run the round it prints
#   ./examples/c/build.sh --release
#
# Everything lands in target/<profile>/, beside the libkui_ffi the hosts
# link and the c_panel a plugin is loaded by, so nothing is written into the
# source tree:
#
#   counter        C as the host, kui as a plain library
#   host           C as the host of a C *extension* (ADR 0014's C half)
#   panel.so       the panel, which either host loads
#   panel-noabi.so the panel with kui_ext_abi deleted; must be refused
#
# Windows is examples/c/build.ps1: the same artifacts in the same place and
# the same round, but an MSVC-ABI compiler, an import library at each link,
# no rpath - and, because a DLL may not leave an import unresolved, two
# panels rather than one. Its header says which of those are Windows' and
# which are kui's.
set -euo pipefail
cd "$(dirname "$0")/../.."

profile=dev
run=0
for arg in "$@"; do
    case "$arg" in
        --release) profile=release ;;
        --run) run=1 ;;
        *) echo "usage: $0 [--release] [--run]" >&2; exit 2 ;;
    esac
done
# Where cargo puts the `dev` profile is `debug`; the one mapping.
profile_dir=debug
[ "$profile" = release ] && profile_dir=release
bin="target/$profile_dir"

# `KUI_PROFILE_DIR` is what host.c builds its default plugin path out of, so
# a --release host looks beside itself rather than in target/debug.
cflags=(-I crates/kui-ffi/include -std=c11 "-DKUI_PROFILE_DIR=$profile_dir" -Wall -Wextra)
# No rpath spelling that both linkers take, so the absolute path: the hosts
# are beside the library either way, and this survives being run from
# anywhere.
link=(-L "$bin" -lkui_ffi -Wl,-rpath,"$(pwd)/$bin")

# One build for both: the library the C hosts link, and the Rust host that
# loads the same plugin they do.
cargo build --profile "$profile" -p kui-ffi --lib --example c_panel

# The header is hand-written, so nothing in Rust makes it match the repr(C)
# structs in crates/kui-ffi/src/types.rs: a field added there but missing from -
# or misordered in - kui.h shifts every field after it, silently, at runtime.
# The same goes for the enums the API reads as indices into a list the core
# owns (KUI_ROLE_* and the rest): a list that grew leaves the header without a
# name for the new member, and C alone unable to say it. The test below
# regenerates a translation unit of _Static_asserts from the Rust layout and
# those lists (see mod abi_parity); compiling it against the header settles
# the two. Nothing links - the asserts are checked in the front end.
abi=target/kui-abi-assert.c
rm -f "$abi"
cargo test -p kui-ffi --lib abi_parity
test -f "$abi" # the test filter matched nothing if this is missing
cc "$abi" "${cflags[@]}" -fsyntax-only
# Anchored, so the #define in the prelude is not counted as a row.
echo "kui.h matches Rust ($(grep -c '^KUI_FIELD' "$abi") fields, $(grep -c '^KUI_ENUM' "$abi") enum members)"

cc examples/c/counter.c "${cflags[@]}" "${link[@]}" -o "$bin/counter"
echo "built $bin/counter"

# C on both sides: a C host that loads the same panel, through
# kui_ctx_add_extension / kui_run_with (ADR 0014's C half, ABI 10). It links
# kui_ffi like counter.c does - a host is a host - and the plugin it loads is
# the same file panel.rs loads, byte for byte.
cc examples/c/host.c "${cflags[@]}" "${link[@]}" -o "$bin/host"
echo "built $bin/host"

# The other direction: C as an extension inside a host that already owns the
# window (examples/c/panel.rs). No -lkui_ffi and no rpath - the
# plugin leaves every kui_* symbol undefined and resolves it from the host
# executable at dlopen time, the way a Lua C module resolves lua_*. Apple's
# linker needs to be told to allow that; ELF leaves undefined symbols in a
# shared object alone.
undef=()
case "$(uname -s)" in
    Darwin) undef=(-Wl,-undefined,dynamic_lookup) ;;
esac

cc examples/c/panel.c "${cflags[@]}" \
    -shared -fPIC "${undef[@]+"${undef[@]}"}" \
    -o "$bin/panel.so"
echo "built $bin/panel.so"

# The same plugin with its kui_ext_abi deleted: a plugin built against a
# header from before ADR 0006 gave plugins a version, which is the one the
# host must refuse and used to load unchecked (backlog S1). Produced from
# panel.c by deleting the one line rather than kept as a second source, so
# the mutant cannot drift from the example. The round below loads it and
# requires the refusal; the grep here is what makes a sed that stopped
# matching fail at the mutation instead of at the load.
noabi=target/panel-noabi.c
sed '/^uint32_t kui_ext_abi(void)/d' examples/c/panel.c > "$noabi"
if grep -q kui_ext_abi "$noabi"; then
    echo "panel-noabi.c still defines kui_ext_abi; the mutation missed" >&2
    exit 1
fi
cc "$noabi" "${cflags[@]}" \
    -shared -fPIC "${undef[@]+"${undef[@]}"}" \
    -o "$bin/panel-noabi.so"
echo "built $bin/panel-noabi.so (kui_ext_abi deleted; must be refused)"

# --- the round --------------------------------------------------------------

# What the artifacts are for, as one list, so that --run and the printout
# cannot disagree and CI runs exactly what a reader is told to run. Each is
# headless and self-asserting: it exits 0 having checked something, or
# non-zero having said what.
round=(
    "$bin/counter --headless|C as the host: the FFI self-test"
    "$bin/host --headless|C on both sides: the slot filled, the click routed, the reply back"
    "$bin/examples/c_panel --headless|the Rust host, loading the same plugin from the other side"
)
# Written out rather than in the list: it is the one that has to fail, and
# a non-zero exit is not enough on its own since every other way of failing
# to load has one too. The message is the check.
mutant="$bin/examples/c_panel --headless $bin/panel-noabi.so"

echo
if [ "$run" = 0 ]; then
    echo "next:"
    for entry in "${round[@]}"; do
        echo "  ./${entry%%|*}"
        echo "      # ${entry#*|}"
    done
    echo "  ./$mutant"
    echo "      # kui_ext_abi deleted: must be refused, not loaded"
    echo "  # or pass --run to have this script run them"
    echo
    echo "  ./$bin/counter              # with a window"
    echo "  ./$bin/examples/c_panel     # with a window"
    exit 0
fi

for entry in "${round[@]}"; do
    cmd=${entry%%|*}
    echo "+ ./$cmd"
    # Unquoted on purpose: these are this script's own strings, and the
    # split into words is what makes them a command.
    # shellcheck disable=SC2086
    ./$cmd
done

echo "+ ./$mutant"
# shellcheck disable=SC2086
if out=$(./$mutant 2>&1); then
    echo "$out"
    echo "FAIL: a plugin with no kui_ext_abi loaded" >&2
    exit 1
fi
echo "$out" | grep -F 'plugin declares no ABI; this build is'

echo
echo "the C round passed (${#round[@]} checks and the refusal, $profile_dir profile)"
