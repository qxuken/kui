#!/usr/bin/env bash
# Drops everything the workspace itself produced out of the target dir, and
# reports what is left. CI runs this just before it writes the cargo cache.
#
# Why it has to exist: the cache is restored through `restore-keys`, so the
# target dir a run starts from was built for some older Cargo.lock, and
# cargo never removes what an older build left behind. Every artifact of the
# workspace is named after a hash that moves with the crate version, the
# feature set and the toolchain, so every release, every new dev-dependency
# and every stable rustc puts a fresh set beside the last one - and the run
# after that carries both forward. Run 249 (2026-09-08) restored 9.0 GB and
# saved 10.1 GB of a dir that should hold about 3 GB; the tar of it cost 33
# of the job's 41 minutes.
#
# The first cut of this script matched the crate names (libkui_core-*,
# kui-core-*) and got the rlibs, the .fingerprint dirs and the 400 MB
# libkui_ffi.a. It missed the biggest part: `cargo test --workspace` and the
# smoke round's `cargo build --examples` link one executable per integration
# test (69 in kui-core alone), bench and example (31 in kui), each carrying
# the debug info of everything it links, and those are named after the
# *target* - deps/access-<hash>, examples/gallery-<hash> - not the crate. A
# week of that is what the restore step was paying for.
#
# So this now goes by what cargo itself says the workspace builds:
# `cargo metadata --no-deps` lists every target of every member, and each
# kind lands in a known place. `cargo clean -p` would be the tool for this,
# and on cargo 1.98 it removes the .fingerprint dirs and the incremental
# state and leaves every executable and rlib in place (the artifact hash
# and the fingerprint hash stopped being the same value) - a dry run over
# this workspace names 0 files under deps/ or examples/ - so it is not.
#
# Only the workspace's artifacts go. The dependency builds - which are the
# whole point of the cache, and the expensive half - stay. Nothing here is
# a loss even when a file was current: every step in `check` recompiles the
# workspace from source anyway (`Compiling kui-core ...` on every run), so
# what this deletes was going to be rebuilt regardless.
set -euo pipefail
cd "$(dirname "$0")/.."

[ -d target ] || { echo "no target dir"; exit 0; }

before=$(du -sm target 2>/dev/null | cut -f1)

meta=$(cargo metadata --no-deps --format-version 1 --offline) || {
  echo "cargo metadata failed; pruning nothing" >&2
  exit 1
}

# Two lists, one line per name, underscored the way cargo spells artifacts:
#   packages: every member, both spellings - cargo names artifacts with
#             underscores (libkui_core-*.rlib) and .fingerprint and build
#             dirs with the package name as written (kui-core-*);
#   exes:     every bin, test and bench target - what deps/ holds as a
#             bare `<name>-<hash>` executable, spelled as written (a bin
#             keeps its hyphen) and underscored (a lib's own unit-test
#             executable is deps/kui_core-<hash>, so libs are in here too).
# Examples are not listed: they all live under examples/, which goes whole.
lists=$(printf '%s' "$meta" \
  | node -e '
      let s = "";
      process.stdin.on("data", d => (s += d)).on("end", () => {
        const packages = new Set(), exes = new Set();
        for (const p of JSON.parse(s).packages) {
          packages.add(p.name);
          packages.add(p.name.replace(/-/g, "_"));
          for (const t of p.targets) {
            if (t.kind.some(k => k === "bin" || k === "test" || k === "bench" || k === "lib")) {
              exes.add(t.name);
              exes.add(t.name.replace(/-/g, "_"));
            }
          }
        }
        console.log([...packages].join(" "));
        console.log([...exes].join(" "));
      });
    ')
packages=$(printf '%s\n' "$lists" | sed -n 1p)
exes=$(printf '%s\n' "$lists" | sed -n 2p)

if [ -z "$packages" ]; then
  echo "cargo metadata named no workspace packages; pruning nothing" >&2
  exit 1
fi

# A profile dir is wherever cargo put a .fingerprint dir: target/debug,
# target/release, and target/<triple>/<profile> on a cross leg.
find target -type d -name .fingerprint -prune -print0 | while IFS= read -r -d '' fp; do
  dir=${fp%/.fingerprint}
  echo "pruning $dir"

  # Every example is the workspace's, and so is every file cargo uplifted
  # to the profile dir's top level (libkui_ffi.a, the devtools bins, their
  # .d files); incremental state never survives a run in CI
  # (CARGO_INCREMENTAL=0), but a target dir that was ever built on a
  # workstation carries it into the cache.
  rm -rf "$dir/examples" "$dir/incremental"
  find "$dir" -maxdepth 1 -type f ! -name '.*' -delete

  # By crate name: rlibs, rmeta, cdylibs, dep-info, the .fingerprint and
  # build-script dirs.
  for name in $packages; do
    find "$dir" \
      \( -name "$name" \
      -o -name "$name-*" \
      -o -name "$name.*" \
      -o -name "lib$name.*" \
      -o -name "lib$name-*" \) \
      -prune -print0
  done | xargs -0r rm -rf

  # By target name: the executables. Only the bare `<name>-<hash>` form, so
  # a test that shares its name with a dependency crate (`windows`, `image`)
  # leaves that crate's libwindows-<hash>.rlib and windows-<hash>.d alone.
  [ -d "$dir/deps" ] || continue
  for name in $exes; do
    find "$dir/deps" -maxdepth 1 -type f -name "$name-*" ! -name '*.*' -print0
  done | xargs -0r rm -f
done

after=$(du -sm target 2>/dev/null | cut -f1)
echo "target: ${before} MB -> ${after} MB"
echo "largest of what is left:"
du -sm target/*/* 2>/dev/null | sort -rn | head -12
