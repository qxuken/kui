// Where the `kui` runner binary for this machine is (ADR 0068): `path` is
// its absolute path, which is what a package `kui pack --format npm` writes
// reads — `require('@qxuken/kui-cli').path` — and what bin/kui.js runs.
//
// The binary ships in a platform package, @qxuken/kui-cli-<platform>, one
// of this package's exact-version optionalDependencies, each declaring
// os/cpu (and libc on Linux) so npm installs only the one this machine is
// — esbuild's shape. The candidates, in order:
//   - KUI_CLI_BINARY=/path/to/kui   an explicit override, replacing the search
//   - @qxuken/kui-cli-<platform>/bin/kui (bin/kui.exe on Windows)
//   - ../../target/{release,debug}/kui   an in-repo `cargo build -p kui-cli`,
//                                        the newer first
// <platform> is Node's process.platform-process.arch, which for the four
// published platforms (linux-x64, linux-arm64, darwin-arm64, win32-x64) is
// the release's own spelling.
'use strict';
const { existsSync, statSync } = require('node:fs');
const path = require('node:path');

const PLATFORMS = ['linux-x64', 'linux-arm64', 'darwin-arm64', 'win32-x64'];
const platform = `${process.platform}-${process.arch}`;
const exe = process.platform === 'win32' ? 'kui.exe' : 'kui';
const platformPackage = PLATFORMS.includes(platform) ? `@qxuken/kui-cli-${platform}` : null;
const version = (() => {
  try {
    return require('./package.json').version;
  } catch {
    return undefined;
  }
})();

function fromPlatformPackage() {
  if (!platformPackage) return null;
  try {
    return require.resolve(`${platformPackage}/bin/${exe}`);
  } catch {
    return null;
  }
}

/** Whether this Linux has a glibc (the platform packages declare it). */
function linuxWithoutGlibc() {
  if (process.platform !== 'linux') return false;
  try {
    return !process.report.getReport().header.glibcVersionRuntime;
  } catch {
    return false;
  }
}

function resolve() {
  const override = process.env.KUI_CLI_BINARY;
  if (override) {
    if (existsSync(override)) return path.resolve(override);
    throw new Error(`kui runner not found for ${platform}: KUI_CLI_BINARY names ${override}, which is not there.`);
  }
  const packaged = fromPlatformPackage();
  if (packaged) return packaged;
  const mtime = (p) => statSync(p).mtimeMs;
  const cargoBuilds = ['release', 'debug'].map((profile) => path.join(__dirname, '..', '..', 'target', profile, exe));
  const built = cargoBuilds.filter(existsSync).sort((a, b) => mtime(b) - mtime(a));
  if (built.length) return built[0];

  const looked = [
    ...(platformPackage ? [`${platformPackage}/bin/${exe} (not installed)`] : []),
    ...cargoBuilds,
  ].join('\n  ');
  const fromSource =
    'build the runner from source: in a checkout of https://github.com/qxuken/kui' +
    (version ? ` at v${version}` : '') +
    ', `cargo build -p kui-cli --release`, then point KUI_CLI_BINARY at ' +
    `target/release/${exe}.`;
  if (platformPackage && !linuxWithoutGlibc()) {
    const pin = version ? `${platformPackage}@${version}` : platformPackage;
    throw new Error(
      `kui runner not found for ${platform}: it ships in ${platformPackage}, an optional ` +
        'dependency of @qxuken/kui-cli that npm installs only on the platform it is for, ' +
        'and it is not installed. The usual causes:\n' +
        '  - the install left optional dependencies out (`--omit=optional`, `--no-optional`, ' +
        '`omit=optional` in .npmrc, or NODE_ENV=production with an old npm): install again ' +
        'without it;\n' +
        '  - the lockfile was written on another OS or CPU, and npm skips a platform package ' +
        'the lockfile did not record (https://github.com/npm/cli/issues/4828): delete ' +
        'node_modules and package-lock.json and run `npm install` again, or add it by name ' +
        `with \`npm install ${pin}\`;\n` +
        '  - node_modules was copied from another machine: run `npm install` (or `npm ci`) here.\n' +
        `The runner is also a download of its own: https://github.com/qxuken/kui/releases` +
        (version ? `/tag/v${version}` : '') +
        ` (install.sh / install.ps1). Or ${fromSource}\nLooked in:\n  ${looked}`,
    );
  }
  const why =
    process.platform === 'darwin' && process.arch === 'x64'
      ? ' (there is no Intel Mac build)'
      : linuxWithoutGlibc()
        ? ' on a Linux without glibc (musl, as Alpine is: the published runner links glibc 2.28)'
        : '';
  throw new Error(
    `kui runner not found for ${platform}${why} - packages are published for linux-x64, ` +
      `linux-arm64, darwin-arm64 and win32-x64 (glibc on Linux); here, ${fromSource}\n` +
      `Looked in:\n  ${looked}`,
  );
}

module.exports = { path: resolve() };
