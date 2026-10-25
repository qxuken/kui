// Resolves the kui-node addon (process.dlopen does not care about the .node
// extension, so the cargo cdylib loads as is). KUI_NODE_LIB=/path/to/lib is
// an explicit override that replaces the search; otherwise the candidates,
// in this order (ADR 0068):
//   - @qxuken/kui-<platform>/kui_node.node   the platform package npm installs
//                                            beside this one: an exact-version
//                                            optionalDependency per platform,
//                                            each declaring os/cpu (and libc on
//                                            Linux), so npm fetches only the
//                                            one this machine is
//   - ../../target/{release,debug}/          an in-repo cargo build, the newer
//                                            of the two first, so a fresh
//                                            `cargo build -p kui-node` is never
//                                            shadowed by a stale release one
//   - prebuilds/<platform>-<arch>/kui_node.node   a hand-staged package
//                                            (scripts/collect-prebuild.nu); the
//                                            published package has carried no
//                                            prebuilds/ since the platform
//                                            packages took them
// <platform> is Node's process.platform-process.arch, which for the four
// shipped platforms (linux-x64, linux-arm64, darwin-arm64, win32-x64) is
// the release's own spelling.
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

// The platforms a package is published for, in Node's spelling, which is
// the package's suffix as it stands.
const PLATFORMS = ['linux-x64', 'linux-arm64', 'darwin-arm64', 'win32-x64'];
const platform = `${process.platform}-${process.arch}`;
const platformPackage = PLATFORMS.includes(platform) ? `@qxuken/kui-${platform}` : null;
const version = (() => {
  try {
    return require('./package.json').version;
  } catch {
    return undefined;
  }
})();

/** The platform package's addon, or null when npm did not install it. */
function fromPlatformPackage() {
  if (!platformPackage) return null;
  try {
    return require.resolve(`${platformPackage}/kui_node.node`);
  } catch {
    return null;
  }
}

const mtime = (p) => statSync(p).mtimeMs;
const cargoBuilds = names
  ? ['release', 'debug'].map((profile) => path.join(__dirname, '..', '..', 'target', profile, names))
  : [];
const inRepo = cargoBuilds.filter(existsSync).sort((a, b) => mtime(b) - mtime(a));
const staged = path.join(__dirname, 'prebuilds', platform, 'kui_node.node');
const packaged = fromPlatformPackage();

// The override replaces the search rather than joining it, so a path that is
// not there is reported as the missing thing it is.
const override = process.env.KUI_NODE_LIB;
const looked = override
  ? [override]
  : [
      packaged ?? (platformPackage ? `${platformPackage}/kui_node.node (not installed)` : null),
      ...cargoBuilds,
      staged,
    ].filter(Boolean);
const found = override
  ? [override].filter(existsSync)
  : [packaged, ...inRepo, staged].filter((p) => p && existsSync(p));

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

/** What to run when this machine has no package: the addon from source. */
const fromSource =
  `build the addon from source: in a checkout of https://github.com/qxuken/kui` +
  (version ? ` at v${version}` : '') +
  ', `cargo build -p kui-node --release`, then point KUI_NODE_LIB at ' +
  `target/release/${names ?? 'libkui_node.<so|dylib|dll>'}.`;

/** Whether this Linux has a glibc (the platform packages declare it). */
function linuxWithoutGlibc() {
  if (process.platform !== 'linux') return false;
  try {
    return !process.report.getReport().header.glibcVersionRuntime;
  } catch {
    return false;
  }
}

if (native) {
  // Loaded, but something newer was unusable — say so, because the usual
  // reason is that the file the developer just built is the broken one.
  for (const s of skipped) {
    process.emitWarning(`kui: skipped an unloadable native library:\n  ${s}`, 'KuiNativeWarning');
  }
} else if (skipped.length) {
  throw new Error(
    `kui native library found but not loadable for ${platform}:\n  ` +
      skipped.join('\n  ') +
      (process.platform === 'linux' ? '\nThe published addon needs glibc 2.28 or newer.' : '') +
      '\nRebuild it with `cargo build -p kui-node --release` in the kui repo, or point ' +
      'KUI_NODE_LIB at a working library.',
  );
} else if (override) {
  throw new Error(
    `kui native library not found for ${platform}: KUI_NODE_LIB names ${override}, which is not there.`,
  );
} else if (platformPackage && !linuxWithoutGlibc()) {
  const pin = version ? `${platformPackage}@${version}` : platformPackage;
  throw new Error(
    `kui native library not found for ${platform}: it ships in ${platformPackage}, ` +
      'an optional dependency of @qxuken/kui that npm installs only on the platform it is ' +
      'for, and it is not installed. The usual causes:\n' +
      '  - the install left optional dependencies out (`--omit=optional`, `--no-optional`, ' +
      '`omit=optional` in .npmrc, or NODE_ENV=production with an old npm): install again ' +
      'without it;\n' +
      '  - the lockfile was written on another OS or CPU, and npm skips a platform package ' +
      'the lockfile did not record (https://github.com/npm/cli/issues/4828): delete ' +
      'node_modules and package-lock.json and run `npm install` again, or add it by name ' +
      `with \`npm install ${pin}\`;\n` +
      '  - node_modules was copied from another machine: run `npm install` (or `npm ci`) here.\n' +
      `Or ${fromSource}\nLooked in:\n  ` +
      looked.join('\n  '),
  );
} else {
  // An Intel Mac had a prebuild until alpha.40 (as did an x64 Node under
  // Rosetta on Apple silicon, which reads as the same platform), so it is
  // told that, not left to guess whether the install went wrong.
  const dropped =
    process.platform === 'darwin' && process.arch === 'x64'
      ? ' (Intel Macs, and an x64 Node under Rosetta, had one until 0.1.0-alpha.40 and build it from source since)'
      : linuxWithoutGlibc()
        ? ' on a Linux without glibc (musl, as Alpine is: the published addon links glibc 2.28)'
        : '';
  throw new Error(
    `kui native library not found for ${platform}${dropped} - ` +
      'packages are published for linux-x64, linux-arm64, darwin-arm64 and win32-x64 ' +
      `(glibc on Linux); here, ${fromSource}\nLooked in:\n  ` +
      looked.join('\n  '),
  );
}
module.exports = native;
