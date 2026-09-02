// Resolves the kui-node addon (process.dlopen does not care about the .node
// extension, so the cargo cdylib loads as is). KUI_NODE_LIB=/path/to/lib is
// an explicit override; otherwise the newest of these wins, so an in-repo
// `cargo build -p kui-node` is never shadowed by a stale local prebuild:
//   - prebuilds/<platform>-<arch>/kui_node.node   bundled in the published
//                                                 package (scripts/collect-prebuild.sh)
//   - ../../target/{release,debug}/               an in-repo cargo build
'use strict';
const { existsSync, statSync } = require('node:fs');
const path = require('node:path');

const names = {
  darwin: 'libkui_node.dylib',
  linux: 'libkui_node.so',
  win32: 'kui_node.dll',
}[process.platform];

const candidates = [
  path.join(__dirname, 'prebuilds', `${process.platform}-${process.arch}`, 'kui_node.node'),
];
if (names) {
  for (const profile of ['release', 'debug']) {
    candidates.push(path.join(__dirname, '..', '..', 'target', profile, names));
  }
}
const mtime = (p) => statSync(p).mtimeMs;
const found = process.env.KUI_NODE_LIB
  ? [process.env.KUI_NODE_LIB]
  : candidates.filter(existsSync).sort((a, b) => mtime(b) - mtime(a));

let native;
for (const p of found) {
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
