#!/usr/bin/env bash
# Builds libkui_ffi and the C counter example against it.
set -euo pipefail
cd "$(dirname "$0")/../.."

cargo build -p kui-ffi

cc examples/c/counter.c \
    -I crates/kui-ffi/include \
    -L target/debug -lkui_ffi \
    -Wl,-rpath,"$(pwd)/target/debug" \
    -Wall -Wextra -o examples/c/counter

echo "built examples/c/counter"
