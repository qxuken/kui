// Loads the kui-node cdylib straight from the cargo target dir (no copy step;
// process.dlopen doesn't care about the .node extension). Override with
// KUI_NODE_LIB=/path/to/libkui_node.dylib for prebuilt binaries.
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
    'kui native library not found - run `cargo build -p kui-node --release` ' +
      'or point KUI_NODE_LIB at the built library.\nLooked in:\n  ' +
      candidates.join('\n  '),
  );
}
module.exports = native;
