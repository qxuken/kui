#!/usr/bin/env bash
# Writes one release version everywhere it is spelled out: the workspace
# version, the workspace's kui-* dependency requirements (cargo publish needs
# a version next to each path dependency), packages/kui/package.json, the
# changelog's top heading, and the examples' npm lockfile, which mirrors the
# linked package's version. The release workflow refuses a tag that does not
# match all five (scripts/check-version.sh), so bump, commit, then tag
# `v<version>`.
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:?usage: scripts/set-version.sh 0.1.0-alpha.2}"
sed -i.bak -E \
  -e "s/^version = \"[^\"]+\"/version = \"$ver\"/" \
  -e "s/^(kui(-core|-wgpu)? = \{ path = \"[^\"]+\", version = )\"[^\"]+\"/\1\"$ver\"/" \
  Cargo.toml
rm Cargo.toml.bak
# The changelog's top heading is the fourth place the version is spelled out
# and the only one a human writes, so it is the one that ships stale: date the
# open section here. The range stops at the first `## `, so no released
# section below it can be touched. Only the heading line - what the release
# adds and what you can delete stay a person's to write.
sed -i.bak -E \
  -e "1,/^## /s/^## .+ \(unreleased\)\$/## $ver ($(date +%F))/" \
  CHANGELOG.md
rm CHANGELOG.md.bak
(cd packages/kui && npm version --no-git-tag-version --allow-same-version "$ver" >/dev/null)
# examples/node depends on packages/kui by `file:` path, so its lockfile
# carries a copy of that manifest's version in its `../../packages/kui`
# entry - which the line above has just made stale. `--package-lock-only`
# rewrites the lockfile and nothing else (no node_modules, no network), so
# npm stays the only thing that writes its own format. This one went from
# alpha.2 to alpha.5 unnoticed because nothing here wrote it and nothing
# checked it; check-version.sh now does the second half.
(cd examples/node && npm install --package-lock-only --offline --no-audit --no-fund --silent)
# Cargo.lock records the workspace crates' own versions.
cargo update --workspace --offline --quiet
scripts/check-version.sh "$ver"
