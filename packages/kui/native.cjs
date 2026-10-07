// Resolves the kui-node addon (process.dlopen does not care about the .node
// extension, so the cargo cdylib loads as is). KUI_NODE_LIB=/path/to/lib is
// an explicit override; otherwise the newest of these wins, so an in-repo
// `cargo build -p kui-node` is never shadowed by a stale local prebuild:
//   - prebuilds/<platform>-<arch>/kui_node.node   bundled in the published
//                                                 package (scripts/collect-prebuild.nu)
//   - ../../target/{release,debug}/               an in-repo cargo build
//
// A candidate can exist and still not load, and one way to get there is
// ordinary enough to name: `cargo build --all-targets` (or `--tests`, or
// `cargo clippy --all-targets`) builds kui-node's test target, which pulls in
// its dev-dependency on `napi/noop`, and cargo unifies features across one
// build of a dependency — so the cdylib written to target/debug/ that run is
// compiled without `napi_register_module_v1` and Node refuses it with
// "Module did not self-register". Newest-wins then picks exactly that file.
// See the dev-dependency comment in crates/kui-node/Cargo.toml.
//
// So a candidate that fails to load is skipped rather than fatal, with a
// warning naming it; if nothing loads at all, every failure is reported with
// what it looked like. Skipping keeps newest-wins honest: the only thing that
// overrides a newer artifact is that newer artifact being unusable.
'use strict';
const { existsSync, statSync } = require('node:fs');
const os = require('node:os');
const path = require('node:path');

// The addon is what provides `kui_*` to a C extension loaded later
// (`ctx.addExtension`, ADR 0014): a plugin leaves the whole API undefined and
// the dynamic linker resolves it against what is already in the process. On
// glibc "already in the process" means the global scope, and Node's default
// for an addon is RTLD_LOCAL, so every `kui_*` the plugin imports is
// unresolvable however plainly the addon exports it — `undefined symbol:
// kui_slot_params`, which is what CI on Linux found. This is the same fact
// `crates/kui-ffi/build.rs` passes `--export-dynamic` for, one loader down:
// being in the image is not the problem, being reachable from the plugin is.
//
// Only RTLD_GLOBAL is added; RTLD_LAZY is Node's own default and staying with
// it keeps the change to one bit. Darwin resolves such a plugin either way
// and Windows ignores the flags, so both are unaffected — and the constants
// are read defensively, because a platform that offers none should load the
// addon the way it always did rather than throw here.
const DLOPEN_FLAGS = (() => {
  const c = os.constants.dlopen;
  return c && c.RTLD_LAZY !== undefined && c.RTLD_GLOBAL !== undefined
    ? c.RTLD_LAZY | c.RTLD_GLOBAL
    : undefined;
})();

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
// The override replaces the search rather than joining it, so a path that is
// not there is reported as the missing thing it is.
const looked = process.env.KUI_NODE_LIB ? [process.env.KUI_NODE_LIB] : candidates;
const found = looked.filter(existsSync).sort((a, b) => mtime(b) - mtime(a));

/** Why `p` did not load, naming the cause where the cause is known. */
function diagnose(p, err) {
  const why = /did not self-register/i.test(err.message)
    ? [
        'was built without N-API module registration. A cargo build that also builds',
        "kui-node's tests (`--all-targets`, `--tests`, `cargo clippy --all-targets`)",
        'unifies its `napi/noop` dev-dependency feature into the cdylib and leaves',
        'this behind. Rebuild the addon on its own: `cargo build -p kui-node`.',
      ]
    : [err.message];
  return [p, ...why.map((line) => `    ${line}`)].join('\n');
}

let native;
const skipped = [];
for (const p of found) {
  const mod = { exports: {} };
  try {
    if (DLOPEN_FLAGS === undefined) process.dlopen(mod, p);
    else process.dlopen(mod, p, DLOPEN_FLAGS);
  } catch (err) {
    skipped.push(diagnose(p, err));
    continue;
  }
  native = mod.exports;
  break;
}
if (native) {
  // Loaded, but something newer was unusable — say so, because the usual
  // reason is that the file the developer just built is the broken one.
  for (const s of skipped) {
    process.emitWarning(`kui: skipped an unloadable native library:\n  ${s}`, 'KuiNativeWarning');
  }
} else if (skipped.length) {
  throw new Error(
    `kui native library found but not loadable for ${process.platform}-${process.arch}:\n  ` +
      skipped.join('\n  ') +
      '\nRebuild it with `cargo build -p kui-node --release` in the kui repo, or point ' +
      'KUI_NODE_LIB at a working library.',
  );
} else {
  // An Intel Mac had a prebuild until alpha.40, so it is told that, not
  // left to guess whether the install went wrong.
  const dropped =
    process.platform === 'darwin' && process.arch === 'x64'
      ? ' (Intel Macs had one until 0.1.0-alpha.40 and build it from source since)'
      : '';
  throw new Error(
    `kui native library not found for ${process.platform}-${process.arch}${dropped} - ` +
      'this package ships prebuilds for linux-x64, linux-arm64, darwin-arm64 ' +
      'and win32-x64; elsewhere run `cargo build -p kui-node --release` in the kui ' +
      'repo or point KUI_NODE_LIB at a built library.\nLooked in:\n  ' +
      looked.join('\n  '),
  );
}
module.exports = native;
