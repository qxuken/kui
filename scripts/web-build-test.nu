#!/usr/bin/env nu
# Exercise the shipping-file contract with real Binaryen, including failures.
use std/assert
use web-build.nu

const ROOT = (path self | path dirname | path dirname)

def main [] {
    cd $ROOT
    let tools = (web-build tools)
    let optimizer = $tools.wasm_opt
    let work = (mktemp -d)
    try {
        let dir = ($work | path join "paths with spaces")
        mkdir $dir
        let input = ($dir | path join input.wasm)
        let output = ($dir | path join output.wasm)
        # Valid module exporting answer() -> 42, with name/producers sections.
        0x[0061736d01000000 0105016000017f 03020100
           070a0106616e737765720000 0a06010400412a0b
           0005046e616d65 000b0970726f64756365727300] | save --raw $input

        web-build optimize $optimizer $input $output
        web-build optimize $optimizer $output $output
        ^node -e '
            const assert = require("node:assert/strict");
            const fs = require("node:fs");
            const original = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
            const shipped = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
            assert.equal(WebAssembly.Module.customSections(original, "name").length, 1);
            assert.equal(WebAssembly.Module.customSections(original, "producers").length, 1);
            assert.equal(WebAssembly.Module.customSections(shipped, "name").length, 0);
            assert.equal(WebAssembly.Module.customSections(shipped, "producers").length, 0);
            assert.equal(new WebAssembly.Instance(shipped).exports.answer(), 42);
        ' $input $output

        let digest = (open --raw $output | hash sha256)
        let corrupt = ($dir | path join corrupt.wasm)
        "invalid Wasm" | save $corrupt
        let failed = (try {
            web-build optimize $optimizer $corrupt $output
            false
        } catch { true })
        assert $failed "invalid input must fail the build"
        assert equal (open --raw $output | hash sha256) $digest "failed optimization must preserve the destination"
        let fresh = ($dir | path join fresh.wasm)
        let failed_new = (try {
            web-build optimize $optimizer $corrupt $fresh
            false
        } catch { true })
        assert $failed_new
        assert (not ($fresh | path exists)) "failed optimization must not publish a new file"
        assert equal (glob --no-file ($dir | path join ".wasm-opt.*") | length) 0 "temporary directories must be removed"

        let target = ($dir | path join "custom cargo target")
        with-env {CARGO_TARGET_DIR: $target} {
            assert equal (web-build directory wasm32-unknown-unknown) ($target | path join wasm32-unknown-unknown web-release)
        }

        # Real bindgen output must be optimized before either JS or Wasm is
        # published. Force failures after generation to protect the old pair.
        let site = ($dir | path join site)
        web-build bindgen $tools.bindgen $optimizer $input $site app
        let js = ($site | path join app.js)
        let wasm = ($site | path join app_bg.wasm)
        assert ($js | path exists)
        ^node -e '
            const assert = require("node:assert/strict");
            const fs = require("node:fs");
            const module = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
            assert.equal(WebAssembly.Module.customSections(module, "producers").length, 0);
            assert.equal(new WebAssembly.Instance(module).exports.answer(), 42);
        ' $wasm
        "previous JavaScript" | save --force $js
        let js_digest = (open --raw $js | hash sha256)
        let wasm_digest = (open --raw $wasm | hash sha256)
        # Nu rejects the Wasm input as a script and exits nonzero, simulating
        # an optimizer failure without relying on an OS-specific fake tool.
        let rejected = (try {
            web-build bindgen $tools.bindgen $nu.current-exe $input $site app
            false
        } catch { true })
        assert $rejected
        assert equal (open --raw $js | hash sha256) $js_digest
        assert equal (open --raw $wasm | hash sha256) $wasm_digest
        let failed_site = ($dir | path join fresh-site)
        let rejected_new = (try {
            web-build bindgen $tools.bindgen $nu.current-exe $input $failed_site app
            false
        } catch { true })
        assert $rejected_new
        assert (not ($failed_site | path exists))
        let failed_bindgen = (try {
            web-build bindgen $nu.current-exe $optimizer $input $site app
            false
        } catch { true })
        assert $failed_bindgen
        assert equal (open --raw $js | hash sha256) $js_digest
        assert equal (open --raw $wasm | hash sha256) $wasm_digest
        assert equal (glob --no-file ($dir | path join ".wasm-bindgen.*") | length) 0
    } catch {|err|
        rm -rf $work
        error make {msg: $err.msg}
    }
    rm -r $work
    print "Web build: stripping, in-place optimization, failure preservation, cleanup, custom target directory, and staged bindgen passed"
}
