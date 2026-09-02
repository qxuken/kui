// Resolves the kui-node addon (process.dlopen does not care about the .node
// extension, so the cargo cdylib loads as is). Lookup order:
//   1. KUI_NODE_LIB=/path/to/lib      explicit override
//   2. prebuilds/<platform>-<arch>/   binaries bundled in the published
//      kui_node.node                  package (scripts/collect-prebuild.sh)
//   3. ../../target/{release,debug}/  an in-repo `cargo build -p kui-node`
'use strict';
const { existsSync } = require('node:fs');
const path = require('node:path');

const names = {
  darwin: 'libkui_node.dylib',
  linux: 'libkui_node.so',
  win32: 'kui_node.dll',
}[process.platform];

const candidates = [];
if (process.env.KUI_NODE_LIB) candidates.push(process.env.KUI_NODE_LIB);
candidates.push(
  path.join(__dirname, 'prebuilds', `${process.platform}-${process.arch}`, 'kui_node.node'),
);
if (names) {
  for (const profile of ['release', 'debug']) {
    candidates.push(path.join(__dirname, '..', '..', 'target', profile, names));
  }
}

let native;
for (const p of candidates) {
  if (!existsSync(p)) continue;
  const mod = { exports: {} };
  process.dlopen(mod, p);
  native = mod.exports;
  break;
}
if (!native) {
  throw new Error(
    `kui native library not found for ${process.platform}-${process.arch} - ` +
      'this package ships prebuilds for linux-x64, linux-arm64, darwin-arm64, ' +
      'darwin-x64 and win32-x64; elsewhere run `cargo build -p kui-node --release` in the kui ' +
      'repo or point KUI_NODE_LIB at a built library.\nLooked in:\n  ' +
      candidates.join('\n  '),
  );
}
module.exports = native;
