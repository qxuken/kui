#!/usr/bin/env nu
# Drops everything the workspace itself produced out of the target dir, and
# reports what is left. CI runs this just before it writes the cargo cache.
#
#   nu scripts/prune-target.nu
#
# Why it has to exist: the cache is restored through `restore-keys`, so the
# target dir a run starts from was built for some older Cargo.lock, and
# cargo never removes what an older build left behind. Every artifact of the
# workspace is named after a hash that moves with the crate version, the
# feature set and the toolchain, so every release, every new dev-dependency
# and every stable rustc puts a fresh set beside the last one - and the run
# after that carries both forward. Run 249 (2026-09-08) restored 9.0 GB and
# saved 10.1 GB of a dir that should hold about 3 GB; the tar of it cost 33
# of the job's 41 minutes.
#
# The first cut of this script matched the crate names (libkui_core-*,
# kui-core-*) and got the rlibs, the .fingerprint dirs and the 400 MB
# libkui_ffi.a. It missed the biggest part: `cargo test --workspace` and the
# smoke round's `cargo build --examples` link one executable per integration
# test (69 in kui-core alone), bench and example (31 in kui), each carrying
# the debug info of everything it links, and those are named after the
# *target* - deps/access-<hash>, examples/gallery-<hash> - not the crate. A
# week of that is what the restore step was paying for.
#
# So this goes by what cargo itself says the workspace builds: `cargo
# metadata --no-deps` lists every target of every member, and each kind
# lands in a known place. `cargo clean -p` would be the tool for this, and
# on cargo 1.98 it removes the .fingerprint dirs and the incremental state
# and leaves every executable and rlib in place (the artifact hash and the
# fingerprint hash stopped being the same value) - a dry run over this
# workspace names 0 files under deps/ or examples/ - so it is not.
#
# Only the workspace's artifacts go. The dependency builds - which are the
# whole point of the cache, and the expensive half - stay. Nothing here is
# a loss even when a file was current: every step in `check` recompiles the
# workspace from source anyway (`Compiling kui-core ...` on every run), so
# what this deletes was going to be rebuilt regardless.

# Megabytes under `path`, as the disk holds them.
def megabytes [path: string] {
    (du $path | get 0.physical) / 1MB | math round
}

def main [] {
    cd ($env.FILE_PWD | path dirname)
    if not ("target" | path exists) {
        print "no target dir"
        return
    }
    let before = (megabytes target)
    let meta = try {
        ^cargo metadata --no-deps --format-version 1 --offline | from json
    } catch {
        print -e "cargo metadata failed; pruning nothing"
        exit 1
    }
    # Two lists, underscored the way cargo spells artifacts:
    #   packages: every member, both spellings - cargo names artifacts with
    #             underscores (libkui_core-*.rlib) and .fingerprint and build
    #             dirs with the package name as written (kui-core-*);
    #   exes:     every bin, test and bench target - what deps/ holds as a
    #             bare `<name>-<hash>` executable, spelled as written (a bin
    #             keeps its hyphen) and underscored (a lib's own unit-test
    #             executable is deps/kui_core-<hash>, so libs are in here too).
    # Examples are not listed: they all live under examples/, which goes whole.
    let both = {|names| $names | each {|n| [$n ($n | str replace -a "-" "_")] } | flatten | uniq }
    let packages = (do $both ($meta.packages | get name))
    let exes = (do $both ($meta.packages
        | each {|p| $p.targets | where {|t| $t.kind | any {|k| $k in [bin test bench lib] } } | get name }
        | flatten))
    if ($packages | is-empty) {
        print -e "cargo metadata named no workspace packages; pruning nothing"
        exit 1
    }

    # A profile dir is wherever cargo put a .fingerprint dir: target/debug,
    # target/release, and target/<triple>/<profile> on a cross leg (and
    # target/bench-base/target/<profile> on a workstation that benched).
    let profiles = (glob --no-file --depth 4 "target/**/.fingerprint"
        | each {|fp| $fp | path dirname | path relative-to $env.PWD })
    for dir in $profiles {
        print $"pruning ($dir)"

        # Every example is the workspace's, and so is every file cargo
        # uplifted to the profile dir's top level (libkui_ffi.a, the devtools
        # bins, their .d files); incremental state never survives a run in CI
        # (CARGO_INCREMENTAL=0), but a target dir that was ever built on a
        # workstation carries it into the cache.
        rm -rf --permanent ($dir | path join examples) ($dir | path join incremental)
        # Each listing is collected before anything in it goes: `ls` streams,
        # and deleting under it hands it entries that are no longer there.
        ls -a $dir
            | collect
            | where {|f| $f.type == file and not (($f.name | path basename) starts-with ".") }
            | each {|f| rm --permanent $f.name }
            | ignore

        # By crate name: rlibs, rmeta, cdylibs, dep-info, the .fingerprint
        # and build-script dirs. The bash version found these anywhere under
        # the profile dir; they only ever sit at its top level and directly
        # in deps/, build/ and .fingerprint/, which is where this looks - a
        # walk of every build script's out/ tree for names that cannot be
        # there was most of its time.
        let named = {|entry|
            let base = ($entry | path basename)
            $packages | any {|n|
                (
                    $base == $n or ($base starts-with $"($n)-") or ($base starts-with $"($n).")
                    or ($base starts-with $"lib($n).") or ($base starts-with $"lib($n)-")
                )
            }
        }
        for sub in ["" deps build .fingerprint] {
            let where = ($dir | path join $sub)
            if not ($where | path exists) { continue }
            ls -a $where | get name | collect | where {|e| do $named $e } | each {|e| rm -rf --permanent $e } | ignore
        }

        # By target name: the executables. Only the bare `<name>-<hash>`
        # form, so a test that shares its name with a dependency crate
        # (`windows`, `image`) leaves that crate's libwindows-<hash>.rlib and
        # windows-<hash>.d alone. A Windows executable is `<name>-<hash>.exe`
        # (with its .pdb beside it), the one dot allowed.
        let deps = ($dir | path join deps)
        if not ($deps | path exists) { continue }
        ls $deps
            | collect
            | where type == file
            | get name
            | where {|f|
                let base = ($f | path basename)
                let stem = ($base | str replace -r '\.(exe|pdb)$' '')
                (not ($stem | str contains ".")) and ($exes | any {|n| $stem starts-with $"($n)-" })
            }
            | each {|f| rm --permanent $f }
            | ignore
    }

    print $"target: ($before) MB -> (megabytes target) MB"
    print "largest of what is left:"
    glob --depth 2 --no-file "target/*/*"
        | each {|p| {mb: (megabytes $p), path: ($p | path relative-to $env.PWD)} }
        | sort-by -r mb
        | first 12
        | each {|r| print $"($r.mb)\t($r.path)" }
        | ignore
}
