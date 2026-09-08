#!/usr/bin/env bash
# Builds libkui_ffi and the C counter example against it, after checking that
# include/kui.h still describes the structs Rust actually lays out.
#
# Windows is examples/c/build.ps1: same three artifacts and the same checks,
# but an MSVC-ABI compiler, an import library at each link, and no rpath. Its
# header says which of those are Windows' and which are kui's.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -p kui-ffi

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
cc "$abi" -I crates/kui-ffi/include -std=c11 -Wall -Wextra -fsyntax-only
# Anchored, so the #define in the prelude is not counted as a row.
echo "kui.h matches Rust ($(grep -c '^KUI_FIELD' "$abi") fields, $(grep -c '^KUI_ENUM' "$abi") enum members)"

cc examples/c/counter.c \
    -I crates/kui-ffi/include \
    -L target/debug -lkui_ffi \
    -Wl,-rpath,"$(pwd)/target/debug" \
    -Wall -Wextra -o examples/c/counter

echo "built examples/c/counter"

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

cc examples/c/panel.c \
    -I crates/kui-ffi/include \
    -shared -fPIC "${undef[@]+"${undef[@]}"}" \
    -Wall -Wextra -o examples/c/panel.so

echo "built examples/c/panel.so"

# The same plugin with its kui_ext_abi deleted: a plugin built against a
# header from before ADR 0006 gave plugins a version, which is the one the
# host must refuse and used to load unchecked (backlog S1). Produced from
# panel.c by deleting the one line rather than kept as a second source, so
# the mutant cannot drift from the example. CI loads it through
# `c_panel --headless` and requires the refusal; the grep below is what makes
# a sed that stopped matching fail here instead of there.
noabi=target/panel-noabi.c
sed '/^uint32_t kui_ext_abi(void)/d' examples/c/panel.c > "$noabi"
if grep -q kui_ext_abi "$noabi"; then
    echo "panel-noabi.c still defines kui_ext_abi; the mutation missed" >&2
    exit 1
fi
cc "$noabi" \
    -I crates/kui-ffi/include \
    -shared -fPIC "${undef[@]+"${undef[@]}"}" \
    -Wall -Wextra -o target/panel-noabi.so

echo "built target/panel-noabi.so (kui_ext_abi deleted; must be refused)"

# C on both sides: a C host that loads the same panel.so, through
# kui_ctx_add_extension / kui_run_with (ADR 0014's C half, ABI 10). It links
# kui_ffi like counter.c does - a host is a host - and the plugin it loads is
# the same file panel.rs loads, byte for byte.
cc examples/c/host.c \
    -I crates/kui-ffi/include \
    -L target/debug -lkui_ffi \
    -Wl,-rpath,"$(pwd)/target/debug" \
    -Wall -Wextra -o examples/c/host

echo "built examples/c/host"
