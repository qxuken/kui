#!/usr/bin/env nu
# Makes a staged npm release live. The GitHub release workflow stages
# `@qxuken/kui@<version>` on npmjs with a stage-only token
# (.github/workflows/release.yml, "npm stage publish"); only a maintainer
# with 2FA can approve it, which is this script:
#
#   nu scripts/npm-approve.nu                  the version packages/kui/package.json names
#   nu scripts/npm-approve.nu 0.1.0-alpha.35   that version
#   nu scripts/npm-approve.nu --otp 123456     the 2FA code up front; otherwise npm asks
#   nu scripts/npm-approve.nu --dry-run        say what would happen, change nothing
#   nu scripts/npm-approve.nu --no-latest      approve, leave the dist-tags alone
#
# It finds the stage holding the version (`npm stage list`), approves it
# (`npm stage approve`), waits for the registry to list the version, then
# applies CI's `latest` rule: a prerelease publishes under its identifier
# as the dist-tag (`alpha`), and also takes `latest` when it sorts highest
# among the versions the registry holds - a package without `latest`
# answers a bare `npm install @qxuken/kui` with nothing - while a stable
# release's `latest` is never taken back by an alpha published after it.
# A plain version is staged as `latest` already and needs no second step.
#
# Safe to run again: a version already live is not approved twice, and a
# `latest` already in place is left as it is. Needs npm 11.15 or newer
# (`npm stage`; `npm install -g npm@11`) and a login on npmjs with 2FA
# (`npm login`); the approve and the dist-tag each prompt for a code
# unless --otp carries one that is still fresh.

const PACKAGE = "@qxuken/kui"
const REGISTRY = "https://registry.npmjs.org/"

# Runs the command with inherited stdio (npm's 2FA prompt needs the
# terminal) and fails loudly when it does.
def --wrapped must [...cmd: string] {
    run-external ...$cmd
    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: $"($cmd | str join ' ') failed \(exit ($env.LAST_EXIT_CODE)\)"}
    }
}

# The version the registry lists under `spec`, or null when it lists none
# (a package or version it does not have exits non-zero or prints nothing).
def live-version [spec: string] {
    let r = (^npm view $spec version --registry $REGISTRY | complete)
    if $r.exit_code != 0 { return null }
    let v = ($r.stdout | str trim)
    if ($v | is-empty) { null } else { $v }
}

# The highest of `versions` by npm's own semver, resolved out of npm's
# bundled copy so nothing has to be installed for it.
def highest-of [versions: list<string>] {
    with-env {VERSIONS: ($versions | to json)} {
        ^node -e '
          const root = require("node:child_process").execFileSync("npm", ["root", "-g"], { encoding: "utf8" }).trim();
          const semver = require(require.resolve("semver", { paths: [root + "/npm/node_modules"] }));
          const all = JSON.parse(process.env.VERSIONS).filter((v) => semver.valid(v));
          if (all.length === 0) throw new Error("no versions to compare");
          process.stdout.write(all.sort(semver.rcompare)[0]);
        '
    } | str trim
}

def main [
    version?: string   # the version to approve; packages/kui/package.json's when omitted
    --otp: string      # the 2FA code; npm prompts for one when this is missing or stale
    --dry-run          # report, change nothing
    --no-latest        # approve only; do not touch the dist-tags
] {
    cd ($env.FILE_PWD | path dirname)
    let ver = if $version == null { open packages/kui/package.json | get version } else { $version }
    if (^npm stage --help | complete).exit_code != 0 {
        error make {msg: "this npm has no `npm stage` (11.15 or newer; `npm install -g npm@11`)"}
    }
    let otp_args = if $otp == null { [] } else { ["--otp" $otp] }

    # 1. Approve, unless the version is live already.
    if (live-version $"($PACKAGE)@($ver)") == $ver {
        print $"($PACKAGE)@($ver) is live on npmjs already"
    } else {
        let listed = (^npm stage list $PACKAGE --json --registry $REGISTRY | complete)
        if $listed.exit_code != 0 {
            error make {msg: $"npm stage list failed \(logged in on npmjs? `npm whoami --registry ($REGISTRY)`\):\n($listed.stderr)"}
        }
        let stages = ($listed.stdout | from json | where version == $ver)
        if ($stages | is-empty) {
            error make {msg: $"nothing staged for ($PACKAGE)@($ver) and it is not live; has the release workflow's `npm stage publish` run?"}
        }
        let stage = ($stages | first)
        print $"stage ($stage.id): ($PACKAGE)@($stage.version), tag ($stage.tag), status ($stage.status), staged ($stage.createdAt) by ($stage.actor)"
        if $dry_run {
            print $"dry run: would approve ($stage.id)"
        } else {
            must npm stage approve $stage.id --registry $REGISTRY ...$otp_args
            # The registry lists an approved version a moment after the approval.
            mut tries = 0
            while (live-version $"($PACKAGE)@($ver)") != $ver {
                $tries += 1
                if $tries > 30 { error make {msg: $"approved, but npmjs does not list ($PACKAGE)@($ver) yet; run this again for the `latest` step"} }
                sleep 2sec
            }
            print $"($PACKAGE)@($ver) is live on npmjs"
        }
    }

    # 2. `latest`, by CI's rule.
    if $no_latest { return }
    let latest = (live-version $"($PACKAGE)@latest")
    if $latest == $ver {
        print $"latest is ($ver) already"
        return
    }
    let versions = (^npm view $PACKAGE versions --json --registry $REGISTRY | complete)
    if $versions.exit_code != 0 {
        error make {msg: $"npm view ($PACKAGE) versions failed:\n($versions.stderr)"}
    }
    let all = ($versions.stdout | from json)
    let all = if ($all | describe | str starts-with "list") { $all } else { [$all] }
    let highest = (highest-of ($all | append $ver | uniq))
    if $highest != $ver {
        print $"latest left alone: ($highest) sorts above ($ver) \(latest is (if $latest == null { 'unset' } else { $latest })\)"
        return
    }
    if $dry_run {
        print $"dry run: would run npm dist-tag add ($PACKAGE)@($ver) latest \(latest is (if $latest == null { 'unset' } else { $latest })\)"
        return
    }
    must npm dist-tag add $"($PACKAGE)@($ver)" latest --registry $REGISTRY ...$otp_args
    print (^npm dist-tag ls $PACKAGE --registry $REGISTRY)
}
