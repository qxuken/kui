#!/usr/bin/env bash
# Writes one release version everywhere it is spelled out: the workspace
# version, the workspace's kui-* dependency requirements (cargo publish needs
# a version next to each path dependency - matched by pattern rather than by a
# list of names, because the list was `kui`, `kui-core` and `kui-wgpu` and
# `kui-ffi` was added to the workspace after it, so alpha.10's bump left that
# one requirement at alpha.9 and check-version.sh below is what caught it), packages/kui/package.json, the
# changelog's top heading, and the examples' npm lockfile, which mirrors the
# linked package's version. The release workflow refuses a tag that does not
# match all five (scripts/check-version.sh), so bump, commit, then tag
# `v<version>`.
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:?usage: scripts/set-version.sh 0.1.0-alpha.2}"
sed -i.bak -E \
  -e "s/^version = \"[^\"]+\"/version = \"$ver\"/" \
  -e "s/^(kui(-[a-z]+)? = \{ path = \"[^\"]+\", version = )\"[^\"]+\"/\1\"$ver\"/" \
  Cargo.toml
rm Cargo.toml.bak
# The changelog's top heading is the fourth place the version is spelled out
# and the only one a human writes, so it is the one that ships stale: date the
# open section here. The range stops at the first `## `, so no released
# section below it can be touched. Only the heading line - what the release
# adds and what you can delete stay a person's to write.
today="$(date +%F)"
sed -i.bak -E \
  -e "1,/^## /s/^## .+ \(unreleased\)\$/## $ver ($today)/" \
  CHANGELOG.md
rm CHANGELOG.md.bak
# The book's setup page spells out the dependency line a reader copies into
# their own Cargo.toml. Nothing wrote it, so the alpha.28 book asked for
# alpha.27; the book is published from the release tag, so the line has to
# name the version that tag publishes.
sed -i.bak -E \
  -e "s/^(kui-native = \{ version = )\"[^\"]+\"/\1\"$ver\"/" \
  docs/book/src/setup.md
rm docs/book/src/setup.md.bak
# Dating that heading moves its anchor, and docs/howto.md links the open
# section by it while the release is being written - `#010-alpha10-unreleased`
# for `## 0.1.0-alpha.10 (unreleased)`. Left alone, three links stop landing
# between the last commit and the tag: the drift backlog F33 was filed for,
# arriving through a link rather than a sentence, and produced by the release
# round rather than by whoever wrote the page. So the same script that moves
# the anchor moves what points at it. The guard is
# `crates/kui-core/tests/docs.rs`, which fails the workspace run on a howto
# anchor no heading answers - run the tests after this script, not before.
slug="$(printf '%s' "$ver" | tr -d '.')"
sed -i.bak -E -e "s/#$slug-unreleased/#$slug-$today/g" docs/howto.md
rm docs/howto.md.bak
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
