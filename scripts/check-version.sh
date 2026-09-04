#!/usr/bin/env bash
# Fails unless the workspace version, every kui-* dependency requirement,
# packages/kui/package.json and the changelog's top heading all name $1 - the
# version a `v*` tag names.
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
        const req = d.req.replace(/^\^/, "");
        if (req !== want) bad.push(`${p.name} requires ${d.name} ${d.req}`);
      }
    }
    const npm = require("./packages/kui/package.json").version;
    if (npm !== want) bad.push(`packages/kui/package.json is ${npm}`);
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
