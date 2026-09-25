#!/usr/bin/env bash
# Fails unless the workspace version, every kui-* dependency requirement,
# packages/kui/package.json, the changelog's top heading and the examples' npm
# lockfile all name $1 - the version a `v*` tag names.
set -euo pipefail
cd "$(dirname "$0")/.."
want="${1:?usage: scripts/check-version.sh 0.1.0-alpha.2}"
cargo metadata --no-deps --offline --format-version 1 \
  | node -e '
    const want = process.argv[1];
    const meta = JSON.parse(require("fs").readFileSync(0, "utf8"));
    const bad = [];
    for (const p of meta.packages) {
      if (p.version !== want) bad.push(`${p.name} is ${p.version}`);
      for (const d of p.dependencies) {
        if (!d.name.startsWith("kui")) continue;
        // kui-core dev-depends on itself to turn on the `conformance`
        // feature for its own tests. A self dependency resolves to the crate
        // being built, never to the registry, so it carries a path and no
        // version - and cargo publish strips it. Requiring a version here
        // would demand kui-core 0.1.0-alpha.6 from the registry while
        // publishing kui-core 0.1.0-alpha.6. Every other dev-dependency is
        // still checked: kui-lua names two, both with versions.
        if (d.name === p.name) continue;
        // A dev-dependency that names a path and no version is dropped
        // from the manifest cargo publishes - `cargo package -p kui-native` ships
        // an empty [dev-dependencies] - so it can never be asked for at
        // the registry and has no version to agree with. That is
        // kui-devtools, the harness the examples run in (publish = false), which
        // every crate with an example dev-depends on since ADR 0021, and
        // which this check refused for alpha.11 as "requires kui-devtools *".
        if (d.kind === "dev" && d.req === "*" && d.path) continue;
        const req = d.req.replace(/^\^/, "");
        if (req !== want) bad.push(`${p.name} requires ${d.name} ${d.req}`);
      }
    }
    const npm = require("./packages/kui/package.json").version;
    if (npm !== want) bad.push(`packages/kui/package.json is ${npm}`);
    // examples/node links packages/kui by `file:` path, so its lockfile
    // mirrors the version in that manifest. Nothing wrote it and nothing
    // read it until now, which is how it sat at alpha.2 through two
    // releases.
    const lockPath = "./examples/node/package-lock.json";
    const linked = require(lockPath).packages?.["../../packages/kui"];
    if (!linked) {
      bad.push(`${lockPath} has no "../../packages/kui" entry`);
    } else if (linked.version !== want) {
      bad.push(`${lockPath} records ${linked.version}`);
    }
    // The changelog heading is written by hand, so it is the one that ships
    // saying "unreleased" the day the release goes out.
    const heading = require("fs").readFileSync("CHANGELOG.md", "utf8")
      .split("\n").find((l) => l.startsWith("## "));
    const names = heading &&
      new RegExp(`(^|\\s)${want.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(\\s|$)`)
        .test(heading.slice(3));
    if (!names || /unreleased/i.test(heading)) {
      bad.push(`CHANGELOG.md top heading is ${heading ? `"${heading}"` : "missing"}`);
    }
    if (bad.length) {
      console.error(`version mismatch, expected ${want}:\n  ` + bad.join("\n  ") +
        "\n(scripts/set-version.sh sets all of them)");
      process.exit(1);
    }
    console.log(`all manifests and the changelog at ${want}`);
  ' "$want"
