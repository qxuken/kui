# Shared web build policy: one Cargo profile, exact tools, and post-processing
# of shipping copies only. Cargo's named artifacts remain useful to twiggy.
const ROOT = (path self | path dirname | path dirname)
const PROFILE = "web-release"

# Ask Cargo so CARGO_TARGET_DIR and .cargo/config.toml agree with our paths.
export def target-dir [] {
    cd $ROOT
    ^cargo metadata --locked --offline --no-deps --format-version 1
        | from json | get target_directory
}

export def directory [target: string] {
    target-dir | path join $target $PROFILE
}

export def build [package: string, target: string, args: list<string>] {
    cd $ROOT
    ^cargo build --locked --profile $PROFILE -p $package --target $target ...$args
}

def executable [name: string] {
    if $nu.os-info.name == "windows" { $"($name).exe" } else { $name }
}

# Prefer our cargo-installed CLI, but accept an exactly matching one on PATH.
# A stale local CLI must not hide a matching system installation.
def bindgen-cli [local: path, version: string] {
    let candidates = ([$local] | append (which wasm-bindgen --all | get path) | uniq)
    for candidate in $candidates {
        if not ($candidate | path exists) { continue }
        let result = (^$candidate --version | complete)
        if $result.exit_code == 0 and ($result.stdout | str trim) == $"wasm-bindgen ($version)" {
            return $candidate
        }
    }
    let root = ($local | path dirname | path dirname)
    error make {msg: $"wasm-bindgen ($version) is required by Cargo.lock. Rerun this build with --install-tools to install it into ($root)."}
}

# Call before compiling or changing the output tree. Binaryen comes from the
# same checksum-locked mise pin on a workstation and on CI, never 'latest'.
export def tools [--install] {
    cd $ROOT
    let tools = (target-dir | path join "web-tools")
    let version = (open --raw ($ROOT | path join Cargo.lock) | from toml | get package
        | where name == "wasm-bindgen" | first | get version)
    if $install {
        ^mise install --locked binaryen
        ^rustup target add wasm32-unknown-unknown wasm32-wasip1
        ^cargo install --locked --root $tools --version $version wasm-bindgen-cli
    }
    let optimizer = (^mise which wasm-opt | complete)
    if $optimizer.exit_code != 0 {
        error make {msg: "Pinned Binaryen is missing. Run `mise install --locked binaryen` or rerun this build with --install-tools."}
    }
    let wasm_opt = ($optimizer.stdout | str trim)
    let pinned = (open ($ROOT | path join mise.toml) | get tools.binaryen.version)
    let actual = (^$wasm_opt --version | str trim)
    let parsed = ($actual | parse -r '^wasm-opt version (?<version>\d+)\b' | get version)
    if $parsed != [$pinned] {
        error make {msg: $"Expected Binaryen ($pinned) from mise.toml, found: ($actual). Run `mise install --locked binaryen`."}
    }
    {
        wasm_opt: $wasm_opt
        bindgen: (bindgen-cli ($tools | path join bin (executable wasm-bindgen)) $version)
    }
}

# wasm-bindgen replaces both JS and Wasm. Keep all its output private until
# optimization succeeds, so a failed optimizer cannot publish unoptimized
# Wasm or JS that no longer matches the previous module.
export def bindgen [cli: path, optimizer: path, input: path, directory: path, name: string] {
    let destination = ($directory | path expand)
    let parent = ($destination | path dirname)
    mkdir $parent
    let stage = (mktemp -d -p $parent ".wasm-bindgen.XXXXXXXX")
    try {
        ^$cli --target web --no-typescript --out-dir $stage --out-name $name $input
        let wasm = ($stage | path join $"($name)_bg.wasm")
        optimize $optimizer $wasm $wasm
        if not ($stage | path join $"($name).js" | path exists) {
            error make {msg: $"wasm-bindgen did not generate ($name).js"}
        }
        mkdir $destination
        let files = (ls $stage | get name)
        cp -r ...$files $destination
    } catch {|err|
        rm -rf $stage
        error make {msg: $"Generating ($name) failed: ($err.msg)"}
    }
    rm -r $stage
}

# Works for a new file or in-place (the *_bg.wasm emitted by wasm-bindgen).
# Keep the old destination intact on errors and clean up partial output. The
# sibling temporary directory keeps the final rename on the same filesystem.
export def optimize [optimizer: path, input: path, output: path] {
    let destination = ($output | path expand)
    let parent = ($destination | path dirname)
    mkdir $parent
    let stage = (mktemp -d -p $parent ".wasm-opt.XXXXXXXX")
    let temporary = ($stage | path join module.wasm)
    try {
        # wasm-opt validates input/output by default. Do not disable validation
        # or broaden target features; retain each input's feature contract.
        ^$optimizer $input -Oz --strip-debug --strip-producers -o $temporary
        mv -f $temporary $destination
    } catch {|err|
        rm -rf $stage
        error make {msg: $"Optimizing ($input) failed: ($err.msg)"}
    }
    rm -r $stage
}
