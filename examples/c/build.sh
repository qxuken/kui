#!/usr/bin/env bash
# Builds libkui_ffi and the C counter example against it, after checking that
# include/kui.h still describes the structs Rust actually lays out.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -p kui-ffi

# The header is hand-written, so nothing in Rust makes it match the repr(C)
# structs in crates/kui-ffi/src/lib.rs: a field added there but missing from -
# or misordered in - kui.h shifts every field after it, silently, at runtime.
# The test below regenerates a translation unit of _Static_asserts from the
# Rust layout (see mod abi_parity); compiling it against the header settles
# the two. Nothing links - the asserts are checked in the front end.
abi=target/kui-abi-assert.c
rm -f "$abi"
cargo test -p kui-ffi --lib abi_parity
test -f "$abi" # the test filter matched nothing if this is missing
cc "$abi" -I crates/kui-ffi/include -std=c11 -Wall -Wextra -fsyntax-only
echo "kui.h matches the Rust struct layout ($(grep -c KUI_FIELD "$abi") fields)"

cc examples/c/counter.c \
    -I crates/kui-ffi/include \
    -L target/debug -lkui_ffi \
    -Wl,-rpath,"$(pwd)/target/debug" \
    -Wall -Wextra -o examples/c/counter

echo "built examples/c/counter"
