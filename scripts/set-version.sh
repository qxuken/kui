#!/usr/bin/env bash
# Writes one release version everywhere it is spelled out: the workspace
# version, the workspace's kui-* dependency requirements (cargo publish needs
# a version next to each path dependency) and packages/kui/package.json.
# The release workflow refuses a tag that does not match all three
# (scripts/check-version.sh), so bump, commit, then tag `v<version>`.
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:?usage: scripts/set-version.sh 0.1.0-alpha.2}"
sed -i.bak -E \
  -e "s/^version = \"[^\"]+\"/version = \"$ver\"/" \
  -e "s/^(kui(-core|-wgpu)? = \{ path = \"[^\"]+\", version = )\"[^\"]+\"/\1\"$ver\"/" \
  Cargo.toml
rm Cargo.toml.bak
(cd packages/kui && npm version --no-git-tag-version --allow-same-version "$ver" >/dev/null)
# Cargo.lock records the workspace crates' own versions.
cargo update --workspace --offline --quiet
scripts/check-version.sh "$ver"
