#!/usr/bin/env bash
# Copies the kui-node cdylib built for a cargo target into
# packages/kui/prebuilds/<platform>-<arch>/kui_node.node - the layout
# packages/kui/native.cjs looks up, named after Node's process.platform and
# process.arch (linux-x64, darwin-arm64, ...). The release workflow runs this
# on every build leg and merges the directories before `npm publish`.
set -euo pipefail
cd "$(dirname "$0")/.."
target="${1:?usage: scripts/collect-prebuild.sh <rust target> <platform-arch>}"
prebuild="${2:?usage: scripts/collect-prebuild.sh <rust target> <platform-arch>}"
dir="target/$target/release"
src=""
for name in libkui_node.so libkui_node.dylib kui_node.dll; do
  [ -f "$dir/$name" ] && src="$dir/$name"
done
if [ -z "$src" ]; then
  echo "no kui-node cdylib in $dir; run: cargo build -p kui-node --release --target $target" >&2
  exit 1
fi
mkdir -p "packages/kui/prebuilds/$prebuild"
cp "$src" "packages/kui/prebuilds/$prebuild/kui_node.node"
ls -l "packages/kui/prebuilds/$prebuild/kui_node.node"
