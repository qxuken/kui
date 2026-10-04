# Releases

Tagged commits publish the library crates (`kui-derive`, `kui-core`,
`kui-wgpu`, `kui-native`, `kui-lua`, `kui-ffi`) to crates.io and to the
self-hosted Forgejo's cargo registry, and
[`packages/kui`](../packages/kui) to Forgejo's npm registry as `@qxuken/kui`, with the Node addon
prebuilt for linux-x64, linux-arm64, darwin-arm64, darwin-x64 and win32-x64 bundled
under `prebuilds/` (`native.cjs` picks the one matching the running Node;
`KUI_NODE_LIB` still overrides it, and an in-repo `cargo build` still wins
for development). A Rust project needs nothing but the dependency:

```toml
# Cargo.toml
kui-native = "0.1.0-alpha.35"  # `use kui_native::…`; `cargo add kui-native` writes it
```

crates.io has had them since 0.1.0-alpha.33; every earlier version is on
the Forgejo registry only. A project that already names that registry
(`[registries.drydock9]` with index
`sparse+https://drydock9.qxuken.dev/api/packages/qxuken/cargo/`, and
`registry = "drydock9"` on the dependency) still finds every version
there, but from 0.1.0-alpha.34 on their own `kui-*` dependencies name
crates.io: with one `kui-*` crate from drydock9 a build reads both and
works, with two (`kui-native` and `kui-core`) it holds two `kui_core`s
whose types do not meet. Dropping `registry = "drydock9"` is the move.
Node installs from npmjs; the Forgejo npm registry carries every version
too, scoped:

```bash
npm install @qxuken/kui@alpha    # prereleases publish under their identifier as the dist-tag
npm create @qxuken/kui-node my-app   # or scaffold an app from the template

# the Forgejo copy, scoped on purpose: Forgejo does not proxy npmjs, so only @qxuken/* goes there
npm config set @qxuken:registry https://drydock9.qxuken.dev/api/packages/qxuken/npm/
```

Every release so far is a prerelease, so `latest` and `alpha` point at the same
thing — the newest alpha — and `npm install @qxuken/kui`, `npm view @qxuken/kui
version` and `npm outdated` all answer with it. `npm view @qxuken/kui@alpha
version` is the query that answers even if `latest` is ever missing, which it is
on a package published before this was arranged: `npm view` defaults to `latest`
and against a package without one prints nothing and exits 0.

A range does not pin a prerelease. Both `^0.1.0-alpha.8` and `~0.1.0-alpha.8`
admit every later alpha of the same `0.1.0` — that is npm's own semver, not a
quirk of the two spellings — so a range is a floor, not a choice. An app that
wants the version it tested writes that version exactly (`"@qxuken/kui":
"0.1.0-alpha.8"`) and commits its lockfile; the lockfile is what holds either
way, and without one a range reinstalls as whatever is newest.

To cut a release: `nu scripts/set-version.nu 0.1.0-alpha.2` (workspace version,
the `kui-*` dependency requirements, package.json and the changelog's open
`(unreleased)` heading move together — registries refuse a version that already
exists), commit, `git tag v0.1.0-alpha.2`, then push the branch and the tag
in one go: `git push --atomic origin main v0.1.0-alpha.2`. That next
`## <version> (unreleased)` heading is opened by hand; the script only dates
the open one, and a tag whose top heading is missing, stale or still says
unreleased fails the release. The tag then runs two pipelines, one on each
host the repository lives on:

- **GitHub**, [release.yml](../.github/workflows/release.yml): `check`
  (fmt, clippy, the workspace tests), then one addon build per platform on
  the platform itself (`ubuntu-24.04` and `ubuntu-24.04-arm` through
  cargo-zigbuild for a glibc 2.28 floor, `macos-15` for both Mac
  architectures, `windows-2025`), then `publish`, which verifies the tag
  against the manifests and the changelog heading, runs the parity tests
  against the five shipped binaries, publishes the crates to crates.io in
  dependency order and stages the npm package on npmjs. It needs two
  repository secrets: `CRATES_IO_TOKEN` (a crates.io API token with publish
  rights on the `kui-*` crates) and `NPM_TOKEN` (an npmjs granular access
  token for the `@qxuken` scope with "Read and write (stage only)").
- **Forgejo**, [ci.yml](../.forgejo/workflows/ci.yml): `check` in full (the
  C round, the Node parity tests, the scene corpus, the book), then
  `publish`, which waits for crates.io to list the version (the Forgejo
  copies name crates.io for their `kui-*` dependencies, so their verify
  builds cannot run before it does; it gives up after two hours, and a
  re-run finishes the job) and publishes the crates to the Forgejo cargo
  registry. It needs one repository secret, `PACKAGES_TOKEN` (a Forgejo
  personal access token with `write:packages`), and the one docker runner.

Neither pipeline publishes the npm package to the Forgejo npm registry; that
is done by hand, below.

The npmjs half is not finished by the runner, on purpose. A stage-only token
can put a version on registry.npmjs.org only as a *staged* release, hidden
until a maintainer with 2FA approves it; the GitHub job's last step stages
`@qxuken/kui@<version>` under its dist-tag (`alpha` for a prerelease). One
script makes it live, logged in on npmjs (`npm login`) with npm 11.15 or
newer (`npm install -g npm@11`):

```bash
nu scripts/npm-approve.nu              # the version package.json names; prompts for the 2FA code
nu scripts/npm-approve.nu --dry-run    # say what it would do
```

It finds the stage, approves it, waits for the registry to list the version
and then applies CI's `latest` rule: an alpha also takes `latest` when it is
the highest version the registry holds, so a bare `npm install @qxuken/kui`
resolves, and a stable release's `latest` is never taken back by a later
alpha (`--no-latest` skips that half). By hand, the same is:

```bash
npm stage list @qxuken/kui                # the pending stage and its id
npm stage view <stage-id>                 # or `npm stage download <stage-id>` to inspect the tarball
npm stage approve <stage-id> --otp <code> # publishes it; `npm stage reject <stage-id>` discards it
npm dist-tag add @qxuken/kui@<version> latest   # if `npm dist-tag ls` shows no `latest`, or an older one
```

A staged version holds its semver slot, so a re-run of the job finds it
staged and stops; rejecting it frees the slot.

The Forgejo npm copy: once both pipelines are through, `nu
scripts/release-local.nu` from a Mac with the tag on HEAD builds the five
prebuilds (the Linux ones in Docker with CI's pinned zig, the macOS ones
natively, Windows through cargo-xwin), runs CI's verification (`npm test`
over the bundled prebuilds, `npm pack --dry-run`, `cargo publish
--dry-run`), asks for a typed confirmation, skips every crate the registries
already hold, publishes the npm package to the Forgejo registry and applies
the `latest` guard there. It reads the token from `$env.DRYDOCK9_TOKEN` and
takes `--dry-run` to stop before publishing. The same script is the
fallback when a runner cannot publish at all (alpha.17's tag job hung in
checkout): it publishes whatever crates are missing too. It does not stage
on npmjs; do that by hand from `packages/kui` with the prebuilds still in
place:

```bash
npm stage publish --registry https://registry.npmjs.org/ --tag alpha --access public
```

then `nu scripts/npm-approve.nu` as above.

No Mac or Windows machine runs anything in either pipeline, and that is the
limit of what CI proves. The Windows non-client
chrome ([windows_nc.rs](../crates/kui-native/src/windows_nc.rs)), the macOS traffic-light
inset in `widgets::titlebar_with` and the whole AccessKit bridge
([access_bridge.rs](../crates/kui-native/src/access_bridge.rs)) are compiled and linked by
the release build and never executed by it — and neither of those two files
carries a test, so the headless suite pins the data they hand the platform, not
the platform's acceptance of it. Two optional jobs, `smoke-macos` and
`smoke-windows`, build and run `cargo test --workspace` natively against the
real SDK; both are gated on the repository variables `SMOKE_MACOS` /
`SMOKE_WINDOWS` and skip unless a runner with the matching label is registered.
They live in their own workflow, [smoke.yml](../.forgejo/workflows/smoke.yml),
for the reason `audit` does: they share nothing with what makes `check`
expensive and are on no release path — and a job whose runner label matches
nothing does not fail fast in Forgejo, it queues, which in `ci.yml` would
leave a run without a result long after `check` and `publish` had finished.

`smoke-windows` does one thing more, and it is the only automated check in the
repo that opens a window: `cargo run -p kui-devtools --bin smoke`
([examples/devtools/src/bin/smoke.rs](../examples/devtools/src/bin/smoke.rs))
runs every windowed example on the runner's own GPU for 120 frames apiece, on
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

Pushing the tag does not check the commit twice. The push to main and the push
of the tag that names it share a concurrency group keyed by the commit, and
only the tag run may cancel — so it takes over a `check` already running for
that commit, and in the other order (the tag handled first) the main run is
stood down by ci.yml's `gate` job instead of running a second one. With a
single runner that is the difference between one `check` and two before
anything is published. It needs Forgejo v14 or newer for the `concurrency`
block; the trade is that a run on main is no longer cancelled when a newer
commit is pushed to main, since the group is the commit rather than the branch.

The two refs go in one `--atomic` push for the same reason. Either order ends
with one `check`, but pushing main first and the tag a minute later means the
run the tag cancels has spent that minute compiling; sent together, the loser
is cancelled — or stood down by `gate`, which answers in about three seconds —
before it has done any work. Not `--follow-tags`: it pushes annotated tags
only, and the tags here are lightweight, so it would push main and silently
leave the tag behind. Name both refs.

What that looks like afterwards: the tag's run is the one that checks, builds
and publishes, and the run for the same commit on main ends as **cancelled**,
or as a three-second `gate` with `check` skipped. Both are the success case.

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
auto-login and the grant already in place could run it inside `smoke-macos`;
nothing else can.

Advisories are checked separately, by
[audit.yml](../.forgejo/workflows/audit.yml): `cargo audit --deny warnings` over
`Cargo.lock`, on pushes that touch the lockfile and on a weekly schedule —
weekly because an advisory is published against code that has not moved, so a
commit-triggered check would only find it the next time someone happened to
push. `--deny warnings` means an unmaintained or unsound crate fails the same
as a vulnerability; the only way to accept one is an entry in
[.cargo/audit.toml](../.cargo/audit.toml) with a comment saying what pulls it in
and what would let the line be deleted. One entry stands today: `ttf-parser`
([RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)),
unmaintained, reached through cosmic-text's `fontdb`.

The job also passes `--no-yanked`. Checking for yanked crates costs one
sparse-index request per crate — 440 of them — and this runner's egress is
slow enough that most time out; a timed-out lookup is not a warning, so
`--deny warnings` cannot see it, and run 141 printed 175 `error:` lines and
reported success. That check is off on purpose rather than silently not
happening. A local `cargo audit --deny warnings` still runs it.

That job is deliberately not in `publish`'s `needs`. A release is cut from a
tag, and an advisory landing between the last green `main` and the tag would
otherwise block a release whose code nobody had touched — so read the audit
job before tagging rather than having it read for you.
