# kui-cli

The `kui` runner from npm: one binary that runs a Lua or Teal app with Kit
and a standard library ([ADR 0064](https://github.com/qxuken/kui/blob/main/docs/adr/0064-the-kui-runner.md)).

```
npm install -g @qxuken/kui-cli@alpha
kui new my-app && kui run my-app
```

The binary itself comes in a platform package, `@qxuken/kui-cli-<platform>`
(linux-x64, linux-arm64 — glibc 2.28 or newer — darwin-arm64, win32-x64),
which npm installs beside this one for the machine it runs on. `bin/kui.js`
runs it with your arguments; from code, `require('@qxuken/kui-cli').path` is
its absolute path, which is what a package `kui pack --format npm` writes
runs. `KUI_CLI_BINARY` points both at another build.

If npm left the platform package out (`--omit=optional`, or a lockfile
written on another OS, [npm/cli#4828](https://github.com/npm/cli/issues/4828)),
`kui` says so and how to fix it. Without npm, the release has the same
binaries and an install script:

```sh
curl -fsSL https://github.com/qxuken/kui/releases/latest/download/install.sh | sh
```

```powershell
irm https://github.com/qxuken/kui/releases/latest/download/install.ps1 | iex
```
