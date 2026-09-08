#!/usr/bin/env bash
# Drops the workspace's own build artifacts out of the target dir, and
# reports what is left. CI runs this just before it writes the cargo cache.
#
# Why it has to exist: the cache is restored through `restore-keys`, so the
# target dir a run starts from was built for some older Cargo.lock, and
# cargo never removes what an older build left behind. Every commit changes
# the workspace crates, so every commit adds a fresh set of rlibs, test
# executables, cdylibs and one 400 MB libkui_ffi.a next to the previous
# commit's - and the run after that carries both forward. Run 249
# (2026-09-08) restored 9.0 GB and saved 10.1 GB of a dir that should hold
# about 3 GB; the tar of it cost 33 of the job's 41 minutes.
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

# Both spellings: cargo names artifacts with underscores (libkui_core-*.rlib)
# and .fingerprint dirs with the package name as written (kui-core-*).
meta=$(cargo metadata --no-deps --format-version 1 --offline) || {
  echo "cargo metadata failed; pruning nothing" >&2
  exit 1
}

names=$(printf '%s' "$meta" \
  | node -e '
      let s = "";
      process.stdin.on("data", d => (s += d)).on("end", () => {
        const out = new Set();
        for (const p of JSON.parse(s).packages) {
          out.add(p.name);
          out.add(p.name.replace(/-/g, "_"));
        }
        console.log([...out].join("\n"));
      });
    ')

if [ -z "$names" ]; then
  echo "cargo metadata named no workspace packages; pruning nothing" >&2
  exit 1
fi

for name in $names; do
  find target \
    \( -name "$name" \
    -o -name "$name-*" \
    -o -name "$name.*" \
    -o -name "lib$name.*" \
    -o -name "lib$name-*" \) \
    -prune -print0
done | xargs -0r rm -rf

# Neither survives a run in CI (CARGO_INCREMENTAL=0), but a target dir that
# was ever built on a workstation carries them into the cache.
rm -rf target/debug/incremental target/*/*/incremental

after=$(du -sm target 2>/dev/null | cut -f1)
echo "target: ${before} MB -> ${after} MB"
echo "largest of what is left:"
du -sm target/*/* 2>/dev/null | sort -rn | head -12
