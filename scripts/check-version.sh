#!/usr/bin/env bash
# Fails unless the workspace version, every kui-* dependency requirement and
# packages/kui/package.json all equal $1 - the version a `v*` tag names.
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
    if (bad.length) {
      console.error(`version mismatch, expected ${want}:\n  ` + bad.join("\n  ") +
        "\n(scripts/set-version.sh sets all of them)");
      process.exit(1);
    }
    console.log(`all manifests at ${want}`);
  ' "$want"
