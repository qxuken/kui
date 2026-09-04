#!/usr/bin/env bash
# Builds libkui_ffi and the C counter example against it, after checking that
# include/kui.h still describes the structs Rust actually lays out.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -p kui-ffi

# The header is hand-written, so nothing in Rust makes it match the repr(C)
# structs in crates/kui-ffi/src/lib.rs: a field added there but missing from -
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
