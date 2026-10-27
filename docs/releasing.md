# Releases

Tagged commits publish the library crates (`kui-derive`, `kui-core`,
`kui-wgpu`, `kui-native`, `kui-lua`, `kui-ffi`) to crates.io; ten npm
packages to npmjs
([ADR 0068](adr/0068-how-kui-reaches-a-machine.md), esbuild's shape):
[`packages/kui`](../packages/kui) as `@qxuken/kui`, the JS and types, and
[`packages/kui-cli`](../packages/kui-cli) as `@qxuken/kui-cli`, a `bin`
shim over the `kui` runner, each naming four platform packages —
`@qxuken/kui-<platform>` (the Node addon alone) and
`@qxuken/kui-cli-<platform>` (the runner binary alone), for linux-x64,
linux-arm64, darwin-arm64 and win32-x64 — as exact-version
`optionalDependencies`, so an install downloads its own platform's only
(`native.cjs` resolves the platform package first; `KUI_NODE_LIB` still
overrides it, and an in-repo `cargo build` is found when no platform
package is installed); and, on the tag's GitHub release, the runner as
`kui-<version>-<platform>.tar.gz` (`.zip` on Windows) with `install.sh`,
`install.ps1` and a `SHA256SUMS` over every asset. A Rust project needs
nothing but the dependency:

```toml
# Cargo.toml
kui-native = "0.1.0-alpha.67"  # `use kui_native::…`; `cargo add kui-native` writes it
```

crates.io has had them since 0.1.0-alpha.33; every earlier version is on
the Forgejo registry only, which holds every version up to 0.1.0-alpha.67
and takes no new ones. A project that already names that registry
(`[registries.drydock9]` with index
`sparse+https://drydock9.qxuken.dev/api/packages/qxuken/cargo/`, and
`registry = "drydock9"` on the dependency) still finds every version
there, but from 0.1.0-alpha.34 on their own `kui-*` dependencies name
crates.io: with one `kui-*` crate from drydock9 a build reads both and
works, with two (`kui-native` and `kui-core`) it holds two `kui_core`s
whose types do not meet. Dropping `registry = "drydock9"` is the move.
Node installs from npmjs, which has had the package since 0.1.0-alpha.35;
every earlier version is on the Forgejo npm registry only, which, like the
cargo one, stops at 0.1.0-alpha.67 (a project scoped to it, with an
`@qxuken:registry` line naming drydock9, drops that line):

```bash
npm install @qxuken/kui@alpha    # prereleases publish under their identifier as the dist-tag
npm create @qxuken/kui-node my-app   # or scaffold an app from the template
npm install -g @qxuken/kui-cli@alpha   # the runner, `kui`
```

Every release so far is a prerelease, so `latest` and `alpha` point at the same
thing — the newest alpha — and `npm install @qxuken/kui`, `npm view @qxuken/kui
version` and `npm outdated` all answer with it. `npm view @qxuken/kui@alpha
version` is the query that answers even if `latest` is ever missing, which it is
on a package published before this was arranged: `npm view` defaults to `latest`
and against a package without one prints nothing and exits 0.

The runner without npm, from the release:

```bash
curl -fsSL https://github.com/qxuken/kui/releases/download/v<version>/install.sh | sh   # macOS, Linux
irm https://github.com/qxuken/kui/releases/download/v<version>/install.ps1 | iex        # Windows
```

Either installs to `~/.kui/bin` (`%USERPROFILE%\.kui\bin`; `KUI_INSTALL_DIR`
moves it), checks the archive against the release's `SHA256SUMS` and
prints the PATH line for the user's shell without editing anything. With
no version (`KUI_VERSION` or `sh install.sh <version>` names one) they take
the newest release the GitHub API lists, prereleases included: GitHub's
own `latest` skips prereleases, so while every release is an alpha it
names an older one (alpha.66, which is not marked a prerelease, is
GitHub's `latest` while alpha.67 is out), and
`releases/latest/download/install.sh` is not a link to give out. `KUI_MIRROR=<base>` reads `<base>/v<version>/<asset>`
instead of GitHub, and `<base>/latest.txt` (the bare version) when no
version is named, so a mirror holds that file beside the release
directories (`kui runners fetch` lays one out).

A range does not pin a prerelease. Both `^0.1.0-alpha.8` and `~0.1.0-alpha.8`
admit every later alpha of the same `0.1.0` — that is npm's own semver, not a
quirk of the two spellings — so a range is a floor, not a choice. An app that
wants the version it tested writes that version exactly (`"@qxuken/kui":
"0.1.0-alpha.8"`) and commits its lockfile; the lockfile is what holds either
way, and without one a range reinstalls as whatever is newest.

To cut a release: `nu scripts/release-prep.nu 0.1.0-alpha.2`, on any of the
machines. It refuses a dirty tree, runs `cargo fmt` and every generator a
check diffs (`nu scripts/gen.nu`: the Lua type files under `packages/lua`,
`npm run gen` for the two `.d.ts` files and `docs/props.md`, `odin.nu gen`,
the book's Examples part) and stops, with the regenerated files left in
the tree to commit, if any of them changes anything.
alpha.59 is why: a `schema::DOORS` row went out without its
`docs/props.md` line, Forgejo's `check` on the tag failed on the diff,
and its `publish` never ran while GitHub's had already put the version
on crates.io. `--skip odin` leaves steps out by name (a machine with
no Odin), and the summary says so;
without a version it runs the checks alone. Then it runs
`scripts/set-version.nu` (workspace version, the `kui-*` dependency
requirements, package.json and the changelog's open `(unreleased)`
heading move together — registries refuse a version that already
exists), which it requires to read `## <version> (unreleased)` first,
and prints what is left. GitHub's [ci.yml](../.github/workflows/ci.yml)
checks every push to main (and every pull request), and the tag runs it
again as the release's first job, so the last run on main is the first
thing to read; the round on the release machines — the windowed smoke
round with a display, the accessibility audit, below — is what CI cannot
do. Then
commit, `git tag v0.1.0-alpha.2`, then push the branch and the tag:
`git push origin main v0.1.0-alpha.2`. `origin` is
Forgejo, and its push mirror carries both refs to GitHub — but do not
count on the mirror to start GitHub's pipeline. Push the tag there by
name as well: `git push github main v0.1.0-alpha.2`. The one time a tag
reached GitHub by the mirror alone (kui-node-template's v0.14.0, the
same day as alpha.35) the tag was there and no workflow ran for it,
while the next tag, pushed by name, started its run within seconds; and
the mirror syncs on its own schedule in any case. Forgejo is the source:
a ref `origin` does not hold is the mirror's to overwrite, so push the
same commit and tag to both and nothing else to `github`. The next release's
`## <version> (unreleased)` heading is opened by hand; the script only dates
the open one, and a tag whose top heading is missing, stale or still says
unreleased fails the release. The tag starts the release pipeline;
Forgejo is a mirror since alpha.68 and runs nothing:

- [release.yml](../.github/workflows/release.yml): `check` (ci.yml,
  called: the whole check a push to main gets), then one build per platform on
  the platform itself (`ubuntu-24.04` and `ubuntu-24.04-arm` through
  cargo-zigbuild for a glibc 2.28 floor, `macos-15` for arm64 — Intel
  Macs have had no prebuild since alpha.41 — and `windows-2025`) of the
  addon and of the runner (`kui-cli`, packed by `scripts/pack-runner.nu`
  into its release asset and asked `kui -V` on its own platform), then
  `publish`, which verifies the tag against the manifests and the
  changelog heading, runs the parity tests against the four shipped
  addons, checks that npmjs holds all ten package names, publishes the
  crates to crates.io in dependency order and publishes the ten npm
  packages on npmjs, the eight platform packages before `@qxuken/kui` and
  `@qxuken/kui-cli`. What it publishes is `npm-stage`'s: that job writes
  the ten from the legs' artifacts (`scripts/npm-platform-packages.nu`,
  into `dist/npm/`), packs them, and installs them on linux-x64 as a user
  would — the addon must load out of `@qxuken/kui-linux-x64` and `npx kui`
  run out of `@qxuken/kui-cli-linux-x64` — before `publish` starts. Each
  build leg also packs `libkui_ffi` for its platform (`scripts/pack-ffi.nu`, from
  the same target dir: the addon's tree is the library's but for 18
  crates) and, off Windows, links and runs the C and Odin counters
  against it shared and static; `packs`, beside `publish` and not in its
  way, bundles the four into `kui-c-<version>.tar.gz` and
  `kui-odin-<version>.tar.gz`, checks them again on linux-x64, adds the
  four runner assets and the two install scripts, writes one `SHA256SUMS`
  over all of it, installs the linux-x64 runner through `install.sh`
  from those files laid out as a mirror, and attaches everything to the
  tag's GitHub release, creating it (a prerelease for an alpha). A failed
  `packs` is re-run on its own; the release's assets are replaced.
  `workflow_dispatch` runs the same without `publish` and without the
  release, leaving the release's assets as the run's `release-packs`
  artifact and the npm tarballs as its `npm-packages` artifact. It needs one
  repository secret, `CRATES_IO_TOKEN` (a crates.io API token with publish
  rights on the `kui-*` crates); npmjs takes no secret (below). The runner
  is not on crates.io: `kui-cli` stays `publish = false` (ADR 0068).

The npmjs half is finished by the runner through trusted publishing, since
alpha.67: each package's settings on npmjs.com name GitHub Actions,
`qxuken/kui` and `release.yml` as its trusted publisher, with "Allow npm
publish" and "Allow npm dist-tag" ticked — all ten of them since ADR 0068:
`@qxuken/kui`, `@qxuken/kui-cli`, and `@qxuken/kui-<platform>` and
`@qxuken/kui-cli-<platform>` for linux-x64, linux-arm64, darwin-arm64 and
win32-x64. The `publish` job holds
`id-token: write`, npm trades GitHub's OIDC token for a publish token of
its own, and each `<package>@<version>` goes live under its dist-tag (`alpha`
for a prerelease) with provenance, then takes `latest` by the rule below.
No token is stored anywhere, and nobody approves. What that needs and how
it fails:

- npm 11.21 or newer (11.5.1 publishes; moving a dist-tag over OIDC came
  later) and Node 22.14 or newer; release.yml pins both.
- `repository.url` in packages/kui/package.json must name the GitHub
  repository, `https://github.com/qxuken/kui`.
- npmjs does not check the trusted publisher when it is saved. A wrong
  field shows only at publish time, as `ENEEDAUTH`; the fields cannot be
  edited, only deleted and created again. A new one expires unless it
  publishes within two days.
- `npm stage list`/`approve` take no OIDC, so the job cannot see a stage:
  a version staged by hand makes its publish fail until the stage is
  approved or rejected.
- A trusted publisher is attached to a package npmjs already holds (to be
  verified on npmjs.com the first time: as of ADR 0068 the settings page
  offers it only for an existing package), so a name npmjs has never held
  cannot be published by the workflow. `publish` checks all ten names
  before it publishes anything, crates included, and fails naming the
  missing ones, while the tag can still move. The first release with
  per-platform packages (0.1.0-alpha.68, or whichever carries ADR 0068)
  needs the eight new names (and `@qxuken/kui-cli`) put up once by hand
  beforehand, which is [`scripts/npm-bootstrap.nu`](../scripts/npm-bootstrap.nu):
  from a login on npmjs with 2FA (npm 11.15 or newer, `@qxuken` and the
  default registry both npmjs), `nu scripts/npm-bootstrap.nu --trust`
  publishes a `0.0.0-bootstrap.0` placeholder of every name npmjs lacks
  and gives each `release.yml` as its trusted publisher (`npm trust
  github`; npm opens the browser for 2FA and npmjs offers to skip it for
  five minutes). Then tag, within two days. The tag's run publishes the
  real versions with provenance and moves `latest` off the placeholders;
  after it, `nu scripts/npm-bootstrap.nu --deprecate` from a login on
  npmjs deprecates them and drops their `bootstrap` tag (by hand: it
  needs the account, not the workflow). Done 2026-10-25 for the
  nine names: placeholders published and trusted. Or by
  hand: release.yml's `npm-packages` artifact from a `workflow_dispatch`
  run, the nine new ones published from it platform packages first
  (`npm publish ./qxuken-kui-cli-linux-x64-<version>.tgz --tag alpha
  --access public`, …), then each given a trusted publisher as above; the
  tag's run finds those versions live and skips them, without provenance. What to check on npmjs the first time: that the
  scope allows the new names, that each package's Trusted Publisher form
  saves, and that `npm view @qxuken/kui-cli-linux-x64@<version>` answers
  after the run.

Until alpha.67 a stage-only `NPM_TOKEN` staged the version and a maintainer
with 2FA approved it. That is still how a version staged by hand (the
fallbacks below) goes live, logged in on npmjs (`npm login`) with npm 11.15
or newer (`npm install -g npm@11`):

```bash
nu scripts/npm-approve.nu              # the version package.json names; npmjs's approval opens in the browser
nu scripts/npm-approve.nu --dry-run    # say what it would do
```

It finds the stage, approves it and then applies CI's `latest` rule, without
waiting for `npm view` to list the version — the packument that reads is
cached and ran more than a minute behind alpha.37's approval, so what is
live is read off the dist-tags instead. The rule: an alpha also takes `latest` when it is
the highest version the registry holds, so a bare `npm install @qxuken/kui`
resolves, and a stable release's `latest` is never taken back by a later
alpha (`--no-latest` skips that half). By hand, the same is:

```bash
npm stage list @qxuken/kui                # the pending stage and its id
npm stage view <stage-id>                 # or `npm stage download <stage-id>` to inspect the tarball
npm stage approve <stage-id> --otp <code> # publishes it; `npm stage reject <stage-id>` discards it
npm dist-tag add @qxuken/kui@<version> latest   # if `npm dist-tag ls` shows no `latest`, or an older one
```

A staged version holds its semver slot; rejecting it frees the slot.

What alpha.35, the first release staged this way, found out about it:

- `npm stage approve` does not ask for a code in the terminal. It prints an
  npmjs URL and waits for the approval in the browser, so run the script
  where one can open. `--otp` is still taken.
- Staging a package npmjs has never held creates it with a placeholder
  version, `0.0.0-stage`, published and holding `latest`. The approved
  version sorts above it, so the script's `latest` step moves the tag;
  until then a bare `npm install @qxuken/kui` answers with the placeholder.
  It stays in the version history; `npm deprecate
  @qxuken/kui@0.0.0-stage "<why>"` is what marks it.
- The registry is the answer, not the website: `npm view @qxuken/kui
  dist-tags` showed `latest` moved while the package page on npmjs.com
  still showed the placeholder under it.
- The maintainer's own npm needs `npm stage` too. Node's bundled npm can
  be older than 11.15 (11.6.2 on the machine that approved alpha.35), and
  the script stops with a message saying so.

When the job's npm step fails, the crates are on crates.io already and
the tag cannot move, so a fix to the workflow does not help that release:
a re-run of the job runs the workflow file the tag points at. alpha.35's
did fail there — the runner's npm predated `npm stage`, and `npm install
-g` without `sudo` cannot write under `/usr/local`; release.yml has
installed it with `sudo` since. The step is finished
from any machine, with the run's own packages rather than a rebuild —
the `npm-packages` artifact holds the ten tarballs `publish` would have
put up:

```bash
gh run download <run-id> -R qxuken/kui -D dist/npm -n npm-packages
cd dist/npm
for p in kui-linux-x64 kui-cli-linux-x64 kui-linux-arm64 kui-cli-linux-arm64 kui-darwin-arm64 kui-cli-darwin-arm64 kui-win32-x64 kui-cli-win32-x64 kui kui-cli; do
  mkdir -p "$p" && tar -xzf "qxuken-$p-<version>.tgz" -C "$p" --strip-components=1
  (cd "$p" && npm stage publish --registry https://registry.npmjs.org/ --tag alpha --access public)
done
```

(A package already live is refused at that step; leave it out.) That
stages with the maintainer's login instead of the runner's OIDC, which
changes nothing about what is staged: the binaries are the ones the run
built and `npm-stage` installed (only the provenance is missing). Then
`nu scripts/npm-approve.nu`, which approves all ten in publish order, the
platform packages before the two that name them. Never publish
`packages/kui` or `packages/kui-cli` straight out of the repository: they
carry no `optionalDependencies` (those are written at staging time, so an
in-repo `npm install` never asks a registry for an unpublished platform
package), and their `prepublishOnly` refuses. To rebuild the staged copies
from a run's artifacts instead, `gh run download <run-id> -R qxuken/kui -D
packages/kui/prebuilds -n linux-x64 -n linux-arm64 -n darwin-arm64 -n
win32-x64`, `gh run download <run-id> -R qxuken/kui -D target/pack/runner
-p 'kui-runner-*'`, then `nu scripts/npm-platform-packages.nu --pack`.

When `packs` cannot finish (and a re-run does not help), the runner's
release assets are the run's four `kui-runner-<platform>` artifacts:
download them into one directory with `install.sh` and `install.ps1`
beside them, `nu scripts/pack-runner.nu sums <dir>` (it covers every
file there, so add the C and Odin tarballs first if they are to be in
it), and `gh release upload v<version> <dir>/* --clobber`. `gh run view
<run-id> -R qxuken/kui` lists where each job stands.

When the runner cannot publish the crates at all (alpha.17's tag job once
hung in checkout), `nu scripts/publish-crates.nu crates-io` from a machine
with the tag checked out and a crates.io token publishes whatever crates.io
is missing, in dependency order; the npm half is the run's `npm-packages`
artifact, staged by hand as above.

The release pipeline runs no test on a Mac or Windows machine — its macOS
and Windows legs compile the addon, the runner and the C library, and ask
the runner its version — and `check` runs on Linux. The Windows non-client
chrome ([windows_nc.rs](../crates/kui-native/src/windows_nc.rs)), the macOS traffic-light
inset in `widgets::titlebar_with` and the whole AccessKit bridge
([access_bridge.rs](../crates/kui-native/src/access_bridge.rs)) are compiled and linked by
the release build and never executed by it — and neither of those two files
carries a test, so the headless suite pins the data they hand the platform, not
the platform's acceptance of it. [smoke.yml](../.github/workflows/smoke.yml)
narrows the gap: on every push to main, on GitHub's `macos-15` and
`windows-2025`, it builds and runs `cargo test --workspace` natively against
the real SDK, then the C round (`cbuild --run`) on the platform's own
compiler. It is non-blocking (`continue-on-error`) and in no release's way,
for the reason `audit` is its own workflow: it shares nothing with what
makes `check` expensive.

It does one thing more, and it is the only automated check in the repo that
opens a window: `cargo run -p kui-devtools --bin smoke`
([examples/devtools/src/bin/smoke.rs](../examples/devtools/src/bin/smoke.rs))
runs every windowed example on the runner's own adapter (a hosted runner
has no GPU, so whatever software one the platform offers) for 120 frames apiece, on
both theme bases, and fails on a crash or a hang — one program for every
platform, so the round cannot drift between a unix host and the Windows
runner; `--node` adds the Node windows. `KUI_SMOKE_FRAMES=n` is what
makes an example self-terminating, and any dev build honours it —
`KUI_SMOKE_FRAMES=120 cargo run -p kui-native --example fragment` is the same check
by hand. A release build ignores it unless built with `--features smoke`, so
that an app you ship does not close its own window over a variable its author
never asked about. Run it before a tag, on Windows above all: the first round
found three crashes and a dead feature that the headless suite passes straight
through (backlog W3–W6). Which example is in which round — windowed,
headless, by hand — is enrolled from the manifests and the example itself
rather than a list somebody keeps ([ADR 0021](adr/0021-one-subject-per-example.md));
`smoke -- --headless` runs the headless round, which CI does, and
`--headless --list` prints it. Open the C hosts (`counter`, `host`) straight
after `cbuild`, or run `cbuild` again after the headless round: that round
rebuilds `kui_ffi.dll` as kui-lua's runner-less dependency over the one the
hosts were linked against, and they then fail to start with
`STATUS_ENTRYPOINT_NOT_FOUND` (backlog W16).

The fuzz targets ([fuzz/](../fuzz/README.md)) are on no release path
either, and nothing runs them unasked: the `fuzz` workflow
([fuzz.yml](../.github/workflows/fuzz.yml)) is started by hand, and
`nu scripts/fuzz.nu all` is the same run on a workstation. What they
once found is replayed by every test run (`fuzz/regressions/`, under
`cargo test --workspace`). Before a tag that touched a parser,
the image decoder or encoders, the Node wire, paths, text or the input
and edit paths, an hour of it is worth having: the first round found
seven defects the headless suite passed through, six of them a crash or
an allocation a host could not stop (backlog FZ1–FZ7). After the hour,
check what it grew on kept sessions (`KUI_FUZZ_CORPUS=1 cargo test
--release -p kui-fuzz --test reuse`, about five minutes for an hour's
corpus): an input that reads differently after others is a defect no
fuzzer reports, and the alpha.66 hour's was a cache every window of a
session shares (backlog FZ14).

Name both refs in the push. Not `--follow-tags`: it pushes annotated tags
only, and the tags here are lightweight, so it would push main and silently
leave the tag behind.

Before tagging, run the macOS accessibility audit by hand:

```bash
cargo build -p kui-native --example accessibility
swiftc -O -o target/ax-audit scripts/ax-audit.swift
./target/debug/examples/accessibility &
target/ax-audit $!
```

106 checks over roles, names, values, the text protocol, the actions and the
live regions, asked through the same API VoiceOver uses. Compile it once
rather than running `swift scripts/ax-audit.swift`: interpreted, every
attribute read waits on the app's run loop and the menu-focus checks fail on
timing alone (104/106 and 99/106 seen; 106/106 on every compiled run), and
run it once per launch — the first run leaves the fixture's toggles flipped.
It stays a manual step rather than a CI job: it needs a logged-in GUI session for the window to exist, a Metal device to
draw it, and Accessibility permission for the calling terminal (System Settings
→ Privacy & Security → Accessibility). That last one is a TCC grant — per
machine, given by hand, and not scriptable without disabling SIP — so an
ephemeral runner can never hold one. A permanently self-hosted Mac with
auto-login and the grant already in place could run it in CI; nothing
else can.

Advisories are checked separately, by
[audit.yml](../.github/workflows/audit.yml): `cargo audit --deny warnings` over
`Cargo.lock`, on pushes that touch the lockfile and on a weekly schedule —
weekly because an advisory is published against code that has not moved, so a
commit-triggered check would only find it the next time someone happened to
push. `--deny warnings` means an unmaintained or unsound crate fails the same
as a vulnerability; the only way to accept one is an entry in
[.cargo/audit.toml](../.cargo/audit.toml) with a comment saying what pulls it in
and what would let the line be deleted. Two decisions stand today:
`ttf-parser`
([RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)),
unmaintained, reached through cosmic-text's `fontdb`; and `zvariant`,
`zvariant_derive` and `zbus_names` (RUSTSEC-2026-0370 to 0372),
unmaintained since zbus 6 folded them in, reached on Linux only through
AccessKit's AT-SPI bridge, which still requires zbus 5.

Yanked crates fail it too. On Forgejo's runner the job passed `--no-yanked`,
because one sparse-index request per crate timed out there and a timed-out
lookup is not a warning (run 141 printed 175 `error:` lines and reported
success); GitHub's runners complete it, as a workstation does.

That job is deliberately not in release.yml's way. A release is cut from a
tag, and an advisory landing between the last green `main` and the tag would
otherwise block a release whose code nobody had touched — so read the audit
job before tagging rather than having it read for you.
