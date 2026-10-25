#!/usr/bin/env nu
# Puts the npm names ADR 0068 added on npmjs once, so the release workflow
# can publish them. npmjs attaches a trusted publisher only to a package it
# already holds, and the workflow publishes only through one, so a name it
# has never held cannot be published from CI at all: `release.yml`'s
# "npm packages exist on npmjs" step fails naming it. This publishes a
# placeholder of each missing name, attaches the workflow as its trusted
# publisher, and — after the first real release — deprecates the
# placeholders.
#
#   nu scripts/npm-bootstrap.nu                publish a placeholder of every name npmjs lacks
#   nu scripts/npm-bootstrap.nu --trust        and attach release.yml as each one's trusted publisher
#   nu scripts/npm-bootstrap.nu --dry-run      say what would happen, change nothing
#   nu scripts/npm-bootstrap.nu --deprecate    after the first real release: deprecate the placeholders
#
# A placeholder is `0.0.0-bootstrap.0`: a package.json and a README saying
# the real versions come from CI, nothing to run. npmjs points `latest` at
# a package's first version whatever tag it went out under, and the
# workflow's publish moves `latest` to the first real version (it sorts
# above `0.0.0`), so nothing here has to be undone by hand but the
# deprecation, which says what it was to anyone who looks at the version.
#
# The trusted publisher is GitHub Actions, `qxuken/kui`, `release.yml`, with
# publishing allowed (`npm trust github … --allow-publish`). The fields are
# case-sensitive and npmjs checks none of them when they are saved; a wrong
# one shows at publish time as ENEEDAUTH. A package holds one trust
# configuration: a name that has one already makes `npm trust github`
# fail, which this reports and goes on past. docs/releasing.md says a new
# trusted publisher expires unless it publishes within two days: run this
# close to the tag. Check each package's settings on npmjs.com once
# afterwards — the workflow also moves dist-tags, which the website's form
# calls "Allow npm dist-tag".
#
# Every call that needs the account runs with the terminal, because npm's
# 2FA is a browser prompt it opens (npmjs then offers to skip it for five
# minutes); only `npm view`, which needs no login, is read quietly. The
# registry is whatever npm is configured with, which must be npmjs for the
# `@qxuken` scope too: this checks, and says how to put it back.
#
# Safe to run again: a name npmjs holds gets no placeholder, and a name with
# a trusted publisher keeps the one it has. Needs npm 11.15 or newer (`npm
# trust`; `npm install -g npm@11`) and a login on npmjs with 2FA on the
# account (`npm login`).

# The names ADR 0068 added, platform packages first. `@qxuken/kui` is on
# npmjs and trusted since alpha.67.
const NEW = [
    "@qxuken/kui-linux-x64" "@qxuken/kui-linux-arm64" "@qxuken/kui-darwin-arm64" "@qxuken/kui-win32-x64"
    "@qxuken/kui-cli-linux-x64" "@qxuken/kui-cli-linux-arm64" "@qxuken/kui-cli-darwin-arm64" "@qxuken/kui-cli-win32-x64"
    "@qxuken/kui-cli"
]
const PLACEHOLDER = "0.0.0-bootstrap.0"
const REPO = "qxuken/kui"
const WORKFLOW = "release.yml"
const NPMJS = "https://registry.npmjs.org"

# Runs the command with the terminal (npm's 2FA prompt needs it); answers
# whether it succeeded.
def --wrapped attempt [...cmd: string] {
    run-external ...$cmd
    $env.LAST_EXIT_CODE == 0
}

# The same, failing loudly.
def --wrapped must [...cmd: string] {
    if not (attempt ...$cmd) {
        error make {msg: $"($cmd | str join ' ') failed \(exit ($env.LAST_EXIT_CODE)\)"}
    }
}

# Whether npmjs holds the package (or the version, for `pkg@version`).
def exists [spec: string] {
    let r = (^npm view $spec version | complete)
    $r.exit_code == 0 and ($r.stdout | str trim | is-not-empty)
}

# A registry URL without its trailing slash, for comparing.
def bare [url: string] {
    $url | str trim | str trim --right --char "/"
}

# npm must talk to npmjs, for the scope as well: an `@qxuken:registry`
# line (the Forgejo copy's, docs/releasing.md) would send every scoped
# call there.
def check-registry [] {
    let scoped = (^npm config get "@qxuken:registry" | str trim)
    let plain = (bare (^npm config get registry))
    if $scoped not-in ["" "undefined" "null"] and (bare $scoped) != $NPMJS {
        error make {msg: $"npm sends @qxuken to ($scoped); this publishes to npmjs: npm config delete @qxuken:registry, then run this again"}
    }
    if $plain != $NPMJS {
        error make {msg: $"npm's registry is ($plain); this publishes to npmjs: npm config set registry ($NPMJS)/, then run this again"}
    }
}

# A placeholder package for `pkg` in a folder of its own, for `npm publish`.
def placeholder [pkg: string, dir: path] {
    mkdir $dir
    {
        name: $pkg
        version: $PLACEHOLDER
        description: "Placeholder: the real versions are published by kui's release workflow (ADR 0068). Install a 0.1.x version."
        license: "MIT"
        repository: {type: "git", url: "git+https://github.com/qxuken/kui.git"}
        publishConfig: {access: "public"}
        files: ["README.md"]
    } | to json --indent 2 | save -f ($dir | path join "package.json")
    $"# ($pkg)\n\nA placeholder that let kui's release workflow publish this name through\nnpm's trusted publishing \(npmjs attaches a trusted publisher only to a\npackage it already holds\). It contains nothing. The real versions come\nfrom https://github.com/qxuken/kui — see ADR 0068.\n" | save -f ($dir | path join "README.md")
}

def npm-version [] {
    ^npm --version | str trim | split row "." | each {|p| $p | into int }
}

def main [
    --trust          # also attach release.yml as each name's trusted publisher
    --deprecate      # deprecate the placeholder versions (after the first real release)
    --dry-run        # say what would happen, change nothing
] {
    check-registry
    let v = (npm-version)
    if $trust and (($v.0 < 11) or ($v.0 == 11 and $v.1 < 15)) {
        error make {msg: $"npm trust needs npm 11.15 or newer; this is ($v | str join '.') — npm install -g npm@11"}
    }

    if $deprecate {
        for pkg in $NEW {
            let spec = $"($pkg)@($PLACEHOLDER)"
            if not (exists $spec) {
                print $"($pkg): no placeholder version — nothing to deprecate"
                continue
            }
            if $dry_run { print $"would deprecate ($spec)"; continue }
            must npm deprecate $spec "A placeholder that let CI publish this name; install a 0.1.x version."
            print $"deprecated ($spec)"
        }
        return
    }

    let missing = ($NEW | where {|p| not (exists $p) })
    for pkg in ($NEW | where {|p| $p not-in $missing }) {
        print $"($pkg): on npmjs already"
    }
    if ($missing | is-not-empty) and not $dry_run {
        print "npm opens the browser for 2FA; npmjs offers to skip it for the next five minutes."
    }
    let work = (mktemp -d -t kui-npm-bootstrap.XXXXXX)
    for pkg in $missing {
        if $dry_run { print $"would publish ($pkg)@($PLACEHOLDER)"; continue }
        let dir = ($work | path join ($pkg | str replace -a "/" "__" | str replace -a "@" ""))
        placeholder $pkg $dir
        must npm publish $dir --tag bootstrap --access public
        print $"published ($pkg)@($PLACEHOLDER)"
    }
    rm -rf $work

    if not $trust {
        print "\nnext: nu scripts/npm-bootstrap.nu --trust  (attaches release.yml to each)"
        return
    }
    print ""
    mut first = true
    mut kept = []
    for pkg in $NEW {
        if $dry_run {
            print $"would trust ($pkg): github ($REPO) ($WORKFLOW)"
            continue
        }
        if not $first { sleep 2sec }
        $first = false
        if (attempt npm trust github $pkg --file $WORKFLOW --repo $REPO --allow-publish --yes) {
            print $"trusted ($pkg): github ($REPO) ($WORKFLOW)"
        } else {
            $kept = ($kept | append $pkg)
        }
    }
    if ($kept | is-not-empty) {
        print $"\nnpm refused these — most likely they have a trusted publisher already \(see its message above; `npm trust list <name>` shows it\): ($kept | str join ', ')"
    }
    if not $dry_run {
        print "\ncheck each on npmjs.com (Settings → Trusted publishing, \"Allow npm dist-tag\"), then tag within two days"
    }
}
