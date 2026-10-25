---
status: accepted
date: 2026-10-25
---

# How kui reaches a machine

> **Accepted and built (2026-10-25); backlog C88.** Settled by a grilling round
> with the user. How a developer gets the `kui` runner and the Node
> addon, and how an app written on the runner reaches the people who use
> it — packed for any platform from one machine, shipped through GitHub,
> a company's own S3, or its npm registry.

## Context

- The `kui` runner (ADR 0064) is one self-contained binary: Lua 5.5,
  Kit's Lua and Teal's compiler are compiled in. `kui-cli` is
  `publish = false` and ships nowhere; ADR 0064 reserved `kui pack`.
- `release.yml` builds natively on four legs — linux-x64 and linux-arm64
  (glibc 2.28, via zigbuild), darwin-arm64, win32-x64 — and attaches the
  C and Odin packs to the GitHub release. There is no Intel Mac leg, and
  none is added; musl is not targeted.
- `@qxuken/kui` ships all four Node addons in one package: 89 MB
  unpacked, about 22 MB per platform, every install downloading all four.
- A runner app is Lua, the same bytes on every platform; only the runner
  is per platform, and it is already built per platform.
- Two audiences: developers who write apps on the runner, and people who
  receive an app and should not need to know kui exists. A company may
  host everything itself — artifacts on S3, npm through its Artifactory
  — and its users may be frontend developers who already have Node
  (Cookie Mapping).
- The book's site runs on a home server; it is not a place to host
  anything an install depends on.

## Decision

### The runner and the addon

1. **Runner binaries on the GitHub release.** The four legs also build
   `kui-cli --release`; the release carries `kui-<version>-<platform>.tar.gz`
   (`.zip` on Windows) and `SHA256SUMS`.
2. **Install scripts**, as release assets beside the binaries:
   `install.sh` (macOS, Linux) and `install.ps1` (Windows) pick the
   platform, check the sum, install to `~/.kui/bin` and print the PATH
   line; a version can be asked for, and `KUI_MIRROR` points them at a
   mirror with the release's layout.
3. **`kui upgrade [version]`** replaces the running binary from the
   release, and refuses with the right command when npm or another
   manager installed it.
4. **Per-platform npm packages**, esbuild's shape. `@qxuken/kui` keeps the
   JS and types and takes `@qxuken/kui-<platform>` (the addon alone) as
   exact-version `optionalDependencies`, each declaring `os`, `cpu` and,
   on Linux, `libc: glibc`. The runner is `@qxuken/kui-cli`, a `bin` shim
   over `@qxuken/kui-cli-<platform>`. The publish job puts the platform
   packages up before the packages that name them. `native.cjs` resolves
   the platform package before `target/`, `KUI_NODE_LIB` still wins, and
   a platform with no package says so and how to build from source.
5. **`kui runners fetch <version> --to <dir>`** downloads every
   platform's runner with its sums in the release's layout,
   `v<version>/kui-<version>-<platform>.tar.gz`, for a company to sync to
   its own mirror.

### The runner's version and its apps

6. **`kui.config.lua` pins the runner**: `kui = "<version>"`, exact while
   kui is pre-1.0. `kui run` warns when the pin is outside its
   compatibility table; `kui pack` packs the pinned runner, not its own.
7. **A compatibility table** in the runner: the earlier versions whose
   apps it runs unchanged. A release adds itself and keeps the earlier
   entries unless its CHANGELOG lists a runner or Kit break, which
   release-prep checks.
8. **Identity**: `kui.config.lua` (or `.tl`) gains `id` (reverse DNS),
   `version` and `icon` — one square PNG, 1024 px recommended, made into
   `.icns`, `.ico` and the Linux icon at pack time, kui's own when absent.
   `id` is required by `kui pack`; `kui run` takes the folder's name.
9. **`kui.data_dir` and `kui.cache_dir`**: per-app folders
   (`~/Library/Application Support/<id>`, `$XDG_DATA_HOME/<id>`,
   `%APPDATA%\<id>`, and their cache siblings), the same under `kui run`
   and packed. The samples and the book's task list move to them.

### `kui pack`

10. **`kui pack [path] --target <t,…> --format <f>`** builds an app for
    any target on any machine: it takes the target's prebuilt runner
    and adds the app. Nothing is compiled; a C plugin would need a cross
    compiler and stays out of v1.
11. **The archive is a zip**, deflated: the app's folder under `app/`
    (without `.git`, `node_modules`, `test/`, `.kui/` and what
    `pack.exclude` names), its own `kui.config.lua` (or `.tl`) unchanged
    as the manifest, and a generated **`kui.pack.lua`** returning what
    packing learned — `{ runner = …, bytecode = …, hash = … }`, the hash
    over `app/`. An app reads it as `kui.packed` (`nil` under `kui run`).
    The archive is also a file of its own, `<name>.kui`, which `kui run`
    runs.
12. **Sources by default; bytecode opt-in.** `kui pack --bytecode`
    compiles each `.lua` and `.tl` to Lua 5.5 bytecode with its debug
    information (`--strip` drops it). Bytecode ties the archive to that
    runner's Lua build: a runner refuses a bytecode archive packed for
    another and names the `kui` it needs. The runner loads binary chunks
    only from its own archive; every other load stays text-only.
13. **Formats**:
    - `exe` — Windows: one file, the archive appended (a zip's index
      sits at its end, so `unzip -l app.exe` lists it), the PE subsystem
      GUI unless `pack = { console = true }`, the icon in its resources.
    - `app` — macOS: an `.app` with `Info.plist`, the `.icns`, and the
      archive as `Contents/Resources/app.zip`, so the runner's signature
      still holds; shipped zipped.
    - `bin` — Linux: one file, the archive appended, in a `.tar.gz`.
    - `npm` — a package of the app's sources (never bytecode) with a
      `bin` shim and an exact dependency on `@qxuken/kui-cli` at the
      pinned version; named `pack.npm.name`, else `id`'s last part.
      Publishing stays the caller's (`npm publish`).
    DMG, AppImage, MSI and `.deb` are later.
14. **Where the runner comes from**: the release, cached under the
    user's cache folder; `--runner-mirror` / `KUI_RUNNER_MIRROR` names a
    mirror with the release's layout, `--runner <file>` a file.
15. **An installer**: `kui pack --installer <base-url>` also writes an
    `install.sh` and `install.ps1` that fetch that app's artifact from
    the base URL, check its sum and install it in the user's own folders
    — `~/Applications/<Name>.app`; `%LOCALAPPDATA%\Programs\<Name>\` with
    a Start-menu shortcut; `~/.local/bin` with a `.desktop` file and icon
    — so nothing needs admin rights.
16. **No signing in v1.** The docs say how: `rcodesign` signs and
    notarizes a `.app` from Linux, `osslsigncode` an `.exe`.
17. **No self-update in v1.** A packed app updates the way it was
    shipped — npm, its installer again, a download. `kui upgrade` is the
    runner's own.

### A packed app at run time

18. **The payload**: the runner looks for `Resources/app.zip` beside it
    (macOS) or an archive at its own end, and runs the app from it.
    `require` and reads of the app's own files come from the archive, and
    so do the paths the core reads itself — an `svg`'s or an `image`'s
    `src`, a font — through one resolver kui-lua takes and the runner
    fills; other hosts keep reading from disk. A write to an app-relative
    path is an error naming `kui.data_dir`.
19. **Arguments**: a packed binary hands all of its arguments to the
    app, so `cookie-mapping --port 3001` reaches `kui.parse_args` with no
    `--`; the runner's own flags come from `KUI_FLAGS`. A packed `.app`
    names itself in the Dock, so the runner does not rename itself.
20. **Logs**: a packed GUI app also writes its log to
    `kui.data_dir/logs/<name>.log`, the last few runs kept, each capped;
    `KUI_LOG=<path>` (or `-`, stderr) overrides it. `kui run` writes none
    unless asked.
21. **Devtools**: in a runner app the panel opens at its Log tab. They
    stay reachable in a packed app — F12, and on macOS a View menu entry
    — with `pack = { devtools = false }` to turn them off and
    `KUI_FLAGS` to turn them on again.
22. **A default menu**: the runner gives every app a good macOS menu bar
    when the app declares none — the app menu, Edit, View with Toggle
    Developer Tools, Window, Help — over ADR 0030's standard menus.

## What the building changed

- **A packed app is extracted, once, and run from a folder** — under
  `<kui cache>/apps/<hash>/`, written into a temporary folder renamed
  into place — rather than read from the archive in place (decision 18).
  The core still opens an `svg`'s or an `image`'s `src` by path itself;
  until the core-purity plan's "core I/O asks" moves that, a folder is
  what every reader can use. What decision 18 keeps: `kui.fs` refuses to
  write into it, naming `kui.data_dir`, and bytecode loads from nowhere
  else.
- **Text-only loading is the runner's for every app**, not only packed
  ones: Lua's `load`, `loadfile`, `dofile` and `require` take bytecode
  by default, and mlua's safe mode does not stop them, so the runner
  wraps all four.
- **A bytecode archive's entry is a source stub** (`__kui_entry.lua`,
  named in `kui.pack.lua`) that requires the compiled entry, so the
  host's own load of the entry stays source.
- **A `.app` packed on a Mac is signed ad hoc** after it is assembled:
  the runner's own signature holds as a binary and fails strict
  validation once it is a bundle's executable. Packed elsewhere it is
  not signed; decision 16 stands for everything past ad hoc.
- **The Windows `exe`'s icon is not written yet**: writing a PE resource
  section is its own piece of work. The subsystem is set.
- **The devtools are on, hidden, in every runner app** (docked nowhere),
  which is what lets F12 bring them up: a core with the devtools off
  hears none of their chords. An app that binds F12 itself says
  `devtools_key = false` in its manifest (the core's Ctrl+Shift+I and
  the macOS menu still reach the panel), or names another chord.
- **`kui run app.kui`** runs an archive by itself, as decision 11 says;
  the archive's comment also names the runner and hash, so `unzip -l`
  shows them.

## Considered options

- **One npm package with every prebuild** (today): every install pays
  for four platforms.
- **One per-platform package for the addon and the runner together**:
  an app on the Node API would carry a runner it never runs.
- **A source build only** (`cargo install kui-cli`): Kit's Lua and
  Teal's compiler live outside the crate, and a build pulls in wgpu.
  `cargo binstall` over the release assets may follow once the crate is
  published.
- **Cross compiling apps**: unneeded; the runner is prebuilt per target.
- **sqlar for the archive**: SQLite is built in, but a database cannot be
  opened partway through a file, so the runner would read the whole
  archive into memory first, and a fused binary would not list with any
  stock tool.
- **Bytecode by default**: smaller and opaque, and every error loses
  readability and every archive its independence from one Lua build.
- **`kui unpack`**: a zip appended to a binary opens with `unzip`, as a
  fused LÖVE game does; Deno's and Bun's compiled binaries have no
  unpack either.
- **The pack facts in the zip comment**: readable without opening an
  entry; a generated Lua file is one format with the rest, and the app
  can read it.
- **Hosting on the book's site**: it runs on a home server.

## Consequences

- An npm install downloads one platform: about 25 MB instead of 89.
- One Linux CI machine packs an app for all three systems; a company
  mirrors the release and the npm packages and ships from its own S3 or
  Artifactory.
- A browser-downloaded unsigned `.app` meets Gatekeeper until the app is
  signed and notarized; the installer's `curl` path does not.
- The runner gains `kui pack`, `kui upgrade`, `kui runners fetch`,
  `kui.data_dir`, `kui.cache_dir`, `kui.packed` and a resolver hook in
  kui-lua; the release gains four binaries, two scripts and eight npm
  packages.

## Building order

1. Release binaries, the install scripts and `kui upgrade`.
2. The per-platform npm packages, the addon's and the runner's.
3. `kui.data_dir`, identity and the version pin with its table.
4. `kui pack`: the archive, the four formats, the resolver, the
   installer, logs and devtools in a packed app.
5. The runner's default menu.
