# Wasm size investigation

Measured 2026-11-13 at commit `167ed8d0eb87b59aae7dd5a748dbd4e906ee9fcd`
(0.1.0-alpha.82). The research measurements below describe that baseline;
the implementation follow-up at the end records the subsequently adopted build
pipeline and browser validation.
All MB values below are decimal; exact bytes are included for reproduction.

**Current decision after robustness review:** keep the web build profile and
post-processing, build-time validation of fixed shaders, and font-library
alignment. Embedded-font compression was reverted: its decoder/cache lifecycle
and synchronous startup cost were not worth the extra moving parts. Fonts are
again embedded as their original TTFs. Later compression/profiling sections
below are historical measurements, not the current shipping configuration.
See the [review outcome](#robustness-review-and-rollback-2026-11-13).

## Findings

There is substantial room to shrink the current artifacts without removing
features. An experimental build using fat LTO, one codegen unit,
`opt-level="s"`, and Binaryen `-Oz` reduced the counter by **39.4%**, the book
gallery by **38.9%**, and the Lua runner by **37.0%** in raw bytes. Their gzip
savings were **21.8%, 21.6%, and 21.1%**, respectively.

The main occupants are text/font machinery, kui's runtime, embedded fonts,
and function-name metadata. Devtools also retain a surprising amount of code:
enabling them on the counter adds **1,790,104 bytes**, including Naga's WGSL
parser/validator through the devtools' polygon icons. The packed WebGPU renderer
is comparatively small and does not duplicate the full text/runtime stack.

## What is currently shipped

[web-counter.nu](../scripts/web-counter.nu) copies the Canvas compiler output
directly, and runs wasm-bindgen for its Rust WebGPU variant.
[web-runner.nu](../scripts/web-runner.nu) ships a direct-copy WASI Lua module
plus a separate bindgen-processed packed renderer.
[book-build.nu](../scripts/book-build.nu) assembles these paths for the book.
None runs wasm-opt, and the root Cargo manifest has no release-profile override.

| Artifact / selected build | Raw bytes | gzip-9 bytes |
| --- | ---: | ---: |
| Counter, Canvas, no optional features | 5,491,586 | 1,755,064 |
| Counter, Canvas, `devtools` | 7,281,690 | 2,274,952 |
| Book gallery, Canvas | 9,577,809 | 3,080,793 |
| Book gallery, Rust WebGPU, **post-bindgen** | 9,906,788 | 3,305,977 |
| Packed Lua/WASI `runner.wasm` | 11,726,487 | 3,719,432 |
| Packed WebGPU `runner-gpu_bg.wasm`, **post-bindgen** | 555,294 | 151,518 |

The gallery explicitly enables devtools/SVG and image decoders; the Lua runner
includes devtools/SVG and the Lua bindings/interpreter. They are not minimum
library-cost estimates. The small counter already has devtools and SVG off.
See [kui-web's manifest](../crates/kui-web/Cargo.toml),
[the gallery manifest](../examples/book-web/Cargo.toml), and
[the runner manifest](../crates/kui-web-lua/Cargo.toml).

For WebGPU, measuring the compiler input would misrepresent the shipping stage;
wasm-bindgen removes intermediate machinery. Generated JS is separate:
`gallery-wgpu.js` measured 72,031 raw / 13,245 gzip bytes, and `runner-gpu.js`
74,261 / 13,669. [wasm-bindgen size guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/optimize-size.html)

These are module measurements, not a complete first-page network budget: HTML,
other JS, WASI shims, and application assets are excluded. The packed WebGPU
path loads both the Lua runtime and renderer, totaling 12,281,781 raw Wasm bytes
before optimization. Its Canvas path can avoid the GPU module; see
[packed-gpu.js](../crates/kui-web-lua/web/packed-gpu.js).

## Where the bytes go

### Counter: sections and function owners

Direct binary-section accounting of the unmodified counter:

| Section | Bytes | Share of module |
| --- | ---: | ---: |
| Function code | 3,837,752 | 69.9% |
| Initialized data | 1,048,164 | 19.1% |
| `name` custom section | 597,179 | 10.9% |
| Everything else | 8,491 | 0.2% |

The name section contains function names, not full DWARF debug information.
The other measured main modules also retain large name sections: gallery
1,059,187 bytes, post-bindgen gallery WebGPU 1,494,921, Lua runner 1,325,510.

Grouping `twiggy top --format csv` rows by the **first crate identifier in
each demangled name** gives these counter function-body costs:

| Group | Shallow bytes |
| --- | ---: |
| `kui_core`, including embedded cosmic-text | 1,169,383 |
| `read_fonts` + `skrifa` + `harfrust` + `swash` | 1,193,346 |
| `zeno` + `yazi` rasterization | 423,498 |
| `core` + `alloc` + `hashbrown` | 842,472 |
| `fontdb` + `ttf_parser` + Unicode segmentation/bidi/script + `font_types` | 69,301 |
| `kui_web` itself | 9,843 |
| Counter entrypoint/app crate | 2,988 |

This ownership grouping is approximate: generics are attributed to their first
crate, inlined callees contribute to callers, and shared code is not assigned
to a single feature. The text-related groups above total **1.686 MB**, before
the embedded fonts or text/layout code inside `kui_core`. These are shallow
bytes, not guaranteed savings from deleting a dependency. Retained sizes must
not be added across ancestors/descendants.
[twiggy shallow-size guide](https://alexene.dev/twiggy/usage/command-line-interface/top.html),
[retained-size definition](https://alexene.dev/twiggy/concepts/dominators-and-retained-size.html)

The largest individual counter functions include `Core::emit_frame` (33,929),
Skrifa's hinting `Engine::run` (31,965), and `Core::layout_frame` (31,754).
There is no single dominant function to remove. `twiggy monos` also identifies
many sorting and other generic instantiations, but its estimated bloat is a
lead for investigation, not a measured removable-byte budget.

### Fonts and duplicate font libraries

Exact full-font byte sequences were found in the baseline modules:

| Embedded font | Bytes | Counter | Gallery | Lua runner |
| --- | ---: | :---: | :---: | :---: |
| Noto Sans Regular | 569,208 | yes | yes | yes |
| Noto Sans Mono Regular | 512,836 | | yes | yes |
| Noto Sans Symbols2 Web subset | 6,784 | yes | yes | yes |
| KUI Book Emoji subset | 9,712 | | yes | |
| **Total** | | **575,992** | **1,098,540** | **1,088,828** |

Sources: [shared browser host](../examples/web/host.rs),
[gallery initialization](../examples/book-web/src/lib.rs), and
[runner initialization](../crates/kui-web-lua/src/lib.rs).
The full Bold font in the repository is **not** embedded by these paths.

`cargo tree --target wasm32-unknown-unknown -d` confirms two font-library
generations: harfrust/kui-core use `read-fonts 0.37` / `skrifa 0.40`, while
swash uses `read-fonts 0.41` / `skrifa 0.44`. Both generations have live code
in the counter. Aligning the stack is worth investigating, but the versions
are incompatible dependency requirements; changing a version number alone is
not a safe fix, and no deduplication saving was measured here.

Font subsetting can reduce actual bytes if the supported character coverage
allows it. Moving fonts to separately cached assets changes module size and
caching, but does not automatically reduce the total first-load bytes.

### Devtools retain shader compilation machinery

Same counter, same release settings, only `--features devtools` changed:
**+1,790,104 raw / +519,888 gzip bytes**. Naga owns 781,098 shallow function
bytes in this build; it has no attributed live functions in the minimal counter.
The retaining path was confirmed with twiggy:

```text
devtools panel → tree/icon drawing → polygon_node → polygon_with_key
→ add_fragment → fragment::validate → naga::front::wgsl::parse_str
```

The source agrees:
[icons.rs](../crates/kui-core/src/runtime/devtools/icons.rs),
[stock_polygon in builder.rs](../crates/kui-core/src/runtime/builder.rs), and
[add_fragment](../crates/kui-core/src/runtime/resources_api.rs).

Recommendation: investigate registering trusted built-in shaders without the
runtime validation path, validating those constants during development/build
checks instead. This could let the linker drop Naga in apps that only use
built-ins. **This is an inference, not a measured saving**; applications or Lua
APIs that still expose arbitrary shader registration can retain the validator.
The general shader API needs to preserve its validation behavior.

### Full gallery and Lua runner

Additional visible owners in the Canvas gallery include `naga` (780,021),
JPEG decoding (172,345), WebP (123,680), `image` (74,950), and PNG (57,688).
`kui_core` grows to 1,960,097 shallow bytes. Despite SVG being enabled in its
dependency graph, no function rows are first-attributed to usvg/resvg/tiny-skia
in this particular gallery binary: enabled dependencies are not proof of
linked byte cost.

The Lua runner has live SVG code: `tiny_skia` (308,486), `usvg` (214,171),
`resvg` (57,518), XML parsing (53,044), `tiny_skia_path` (47,421), and
`svgtypes` (46,949), totaling **727,589 shallow bytes** before shared helpers.
It also has `mlua` (523,004), `kui_lua` (213,431), Naga (780,020), and Lua's
compiled C functions, which are not captured by Rust crate-name grouping.
The interpreter/bindings are meaningful costs, but do not account for the
whole 11.73 MB runtime.

## Measured optimization experiments

The combined compiler experiment used environment overrides, leaving the
workspace release configuration unchanged:

```sh
CARGO_PROFILE_RELEASE_LTO=fat \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
CARGO_PROFILE_RELEASE_OPT_LEVEL=s \
cargo build --locked --release -p kui-web --example counter \
  --target wasm32-unknown-unknown
```

Then Binaryen 133:

```sh
wasm-opt input.wasm -Oz --strip-debug --strip-producers -o output.wasm
```

For bindgen builds this belongs **after** wasm-bindgen. Fat LTO, codegen units,
and `s` were tested as one combination; these results do not isolate each
setting's contribution or prove that this is the optimal combination. Cargo
documents the available settings and warns that size levels need measurement.
[Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
[Binaryen options](https://github.com/WebAssembly/binaryen/blob/version_133/src/tools/optimization-options.h)

| Counter experiment | Raw bytes | gzip-9 bytes |
| --- | ---: | ---: |
| Baseline | 5,491,586 | 1,755,064 |
| Remove only `name` and `producers` sections, all other bytes preserved | 4,894,328 | 1,664,575 |
| Binaryen strip pass / roundtrip, no `-O` level | 4,743,420 | 1,650,497 |
| Baseline + Binaryen `-Os` and stripping | 4,370,314 | 1,675,627 |
| Baseline + Binaryen `-Oz` and stripping | 4,336,199 | 1,673,402 |
| Fat LTO / one unit / `s`, before Binaryen | 4,109,151 | 1,461,510 |
| Same profile + Binaryen strip pass / roundtrip | 3,541,251 | 1,375,336 |
| Same profile + Binaryen `-Oz` and stripping | **3,330,351** | **1,373,136** |

Binaryen's strip-pass roundtrip also re-encodes code, so its full reduction
must not be attributed to metadata removal. The byte-preserving section-removal
experiment above isolates that saving.

| Other module / experiment | Raw bytes | gzip-9 bytes |
| --- | ---: | ---: |
| Gallery baseline + Binaryen `-Oz` | 7,558,655 | 2,936,534 |
| Gallery fat LTO / one unit / `s` + Binaryen strip roundtrip | 6,192,702 | 2,415,771 |
| Gallery same profile + Binaryen `-Oz` | **5,856,602** | **2,415,602** |
| Lua runner baseline + Binaryen `-Oz` | 9,172,360 | 3,499,068 |
| Lua runner fat LTO / one unit / `s` + Binaryen strip roundtrip | 7,775,637 | 2,931,328 |
| Lua runner same profile + Binaryen `-Oz` | **7,390,960** | **2,934,895** |
| Gallery WebGPU baseline + Binaryen `-Oz`, no compiler-profile experiment | 7,739,215 | 3,089,568 |
| Packed GPU baseline + Binaryen `-Oz`, no compiler-profile experiment | 295,285 | 118,186 |

Smaller raw modules do not always compress better. On default-profile counter
and gallery builds, the Binaryen strip-only roundtrip gives a smaller gzip
result than `-Oz`. With the size profile, the Lua runner's strip-only roundtrip
also beats `-Oz` slightly in gzip bytes. Choose against both decoded module
size and the actual delivery codec.

## Recommended order

1. Add a dedicated web release profile and pinned post-processing step, based
   on these measured gains. Update both profile selection and the scripts'
   hardcoded `release` paths together. Compare runtime performance before
   adoption; native builds need not inherit web-specific choices.
2. Keep named analysis artifacts separately and strip production metadata.
   Compare gzip/Brotli outputs in CI as well as raw module bytes. No production
   hosting headers or Brotli results were measured here; a compressed ZIP is
   not evidence of HTTP compression.
   [HTTP content coding](https://www.rfc-editor.org/rfc/rfc9110.html#name-content-encoding)
3. Investigate the devtools → built-in polygon → Naga retention path. Offer a
   slim runner build without devtools if that is a useful supported product.
   This does not help the minimal counter, where devtools are already off.
4. Investigate font subsetting and upstream font-stack version alignment.
5. Consider splitting/lazy-loading gallery demos and optional decoders if
   gallery startup is the priority. The full gallery's size is not the cost
   every application has to pay.

Do not count `panic="abort"` as a new saving for `wasm32-unknown-unknown`: it
already defaults to abort. Panic formatting can still occupy code/data.
[Rust target documentation](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html#unwinding)

## Reproduction and validation

Toolchain: Rust 1.99.1, LLVM 23.1.1, host aarch64-apple-darwin; target
`wasm32-unknown-unknown` or `wasm32-wasip1`; wasm-bindgen CLI **0.2.129**, matching
Cargo.lock; Binaryen **133**; twiggy **0.8.0**; WASI SDK **34.0 arm64-macos**;
Node **24.22.2**. The shell's preinstalled bindgen was a different version, so
the matching version was installed into the temporary analysis tools directory.

Baseline commands, run separately so feature selections do not unify:

```sh
cargo build --locked --release -p kui-web --example counter --target wasm32-unknown-unknown
cargo build --locked --release -p kui-web --example counter --target wasm32-unknown-unknown --features devtools
cargo build --locked --release -p kui-book-web --lib --target wasm32-unknown-unknown
cargo build --locked --release -p kui-book-web --lib --target wasm32-unknown-unknown --features rust-wgpu
wasm-bindgen --target web --no-typescript --out-dir OUT --out-name gallery-wgpu INPUT.wasm
WASI_SDK_PATH=SDK nu -c 'use scripts/wasi-build.nu; wasi-build build kui-web-lua'
cargo build --locked --release -p kui-web --example packed_renderer --target wasm32-unknown-unknown --features rust-wgpu,packed-gpu
wasm-bindgen --target web --no-typescript --out-dir OUT --out-name runner-gpu INPUT.wasm
```

Copy each artifact before the next build overwrites its output path. The
compiler-profile experiments used `CARGO_TARGET_DIR=target/wasm-size-profile`.
The Lua experiment used the same three profile overrides around
`wasi-build build kui-web-lua`; its existing helper supplies the SDK's C compiler,
archiver, sysroot, and library path. The Lua C interpreter was built through
the existing vendored dependency setup; no additional C LTO configuration
was introduced.

Inspection commands:

```sh
twiggy top input.wasm --format csv -o top.csv
twiggy dominators input.wasm -d 2 -r 25
twiggy paths counter-devtools.wasm --regex 'naga.*front.*wgsl.*parse_str'
twiggy monos counter.wasm -n 12
cargo tree --locked -p kui-web --target wasm32-unknown-unknown -e features -i kui-core
cargo tree --locked -p kui-book-web --target wasm32-unknown-unknown -d
```

gzip measurements use Python `gzip.compress(data, compresslevel=9, mtime=0)`
with no filename header. Section accounting reads Wasm section IDs and LEB128
lengths directly. Grouping sums CSV `ShallowSize` by the first match of
`([A-Za-z_][A-Za-z_0-9]*)\[[0-9a-f]+\]` in the demangled name; unmatched
C symbols/data/metadata remain separate. Font totals use exact byte-sequence
matches in the baseline binaries.

All **30** generated baseline/experimental modules passed Node's
`WebAssembly.validate`. All ten counter variants instantiated with stubbed
Canvas imports and passed two frame calls plus pointer movement. Each emitted
the same observed initial call counts: one background, atlas upload, clip
reset and present, four solid quads, and 78 glyphs. This checks execution and
basic output structure; it does not establish pixel equivalence or browser/GPU,
Lua application, or performance correctness. Those initial research checks were
followed by the browser validation below before adopting the build changes.

Intermediate binaries, CSVs, section measurements, and tools were retained for
this session under `/tmp/kui-wasm-analysis` and `/tmp/kui-wasm-tools`; temporary
paths are not permanent repository artifacts. This report contains the
reproduction procedure and measured results.

## Implementation follow-up (2026-11-13)

The dedicated `web-release` profile now uses the measured fat-LTO / one-unit /
`s` combination. [web-build.nu](../scripts/web-build.nu) centralizes Cargo build
and output paths, exact tool resolution, and `-Oz --strip-debug
--strip-producers`. All three shipping entrypoints use it, including the
book's Lua module and the runner's separate GPU module. Bindgen runs before
optimization. Native release settings are unchanged; Cargo analysis artifacts
retain their names.

Binaryen 133 is pinned in mise.toml and checksum-locked for Linux x64/arm64,
macOS arm64, and Windows x64. CLI selection requires the wasm-bindgen version
in Cargo.lock, preferring the project installation. `--install-tools` is
available on each entrypoint. Paths come from Cargo metadata, including custom
target directories. Optimizer output is staged beside the destination and
renamed only after success; failed optimization preserves the destination and
removes partial output. This file-level guarantee does not make an entire
multi-file site rebuild transactional.

End-to-end builds used output directories containing spaces. Final main-module
sizes matched the experiment: counter **3,330,351**, Canvas gallery **5,856,602**,
and Lua runner **7,390,960** bytes. The now-profiled packed GPU module measured
**251,154** bytes. All nine distinct distributed modules (four counter/notes,
three gallery, two runner) validated in Node and had no name/producer sections;
the packed archive was inspected as well as the staged sites.

Validation completed:

- `nu scripts/web-build-test.nu`: real Binaryen produces executable stripped
  output; in-place processing works; invalid inputs fail without replacing
  existing output or publishing a new file; temporary files are cleaned; paths
  with spaces and a custom Cargo target directory work. CI runs this test.
- Complete `web-counter.nu` and `book-build.nu` builds, including creation and
  consumption of the packed runner ZIP.
- Existing notes smoke suite on Chromium, Firefox, and WebKit Canvas; Chrome
  WebGPU; touch flows with both renderers; all four renderer-failure/recovery
  scenarios; mobile keyboard tests on Chromium and WebKit at phone/tablet sizes.
- Counter increment, decrement, reset, and context-menu dismissal on those
  three Canvas engines and Chrome WebGPU, with displayed-value image checks.
- Existing book smoke suite: **61 checks, zero failures**, including Lua,
  packed tasks, and the legacy tasks redirect.
- Packed Lua tasks add/complete/clear interactions on Chromium, Firefox,
  WebKit Canvas and Chrome WebGPU.
- Cargo formatting, workflow lint, and diff whitespace checks.

Browser checks used Playwright's Chromium 151, Firefox 153, WebKit 26.5, and
installed Chrome 157 for a real WebGPU adapter. This is functional validation,
not a frame-time benchmark. Setup and reproduction instructions for the new
pipeline are in the [web examples README](../examples/web/README.md).

## Built-in shader validation follow-up (2026-11-13)

The devtools retention path above is now removed. Both stock fragments
(`POLYGON` and `JOIN`) are checked by
[kui-core/build.rs](../crates/kui-core/build.rs) on the build host, including
cross builds. The build script and runtime compile the same
[shader module](../crates/kui-core/src/fragment/shader.rs): the original WGSL
strings, module assembly, parser, validator capabilities/flags, and error
reporting are shared. The four WGSL strings were preserved byte-for-byte.
Changes to that module rerun the build check; malformed built-ins fail the
build with the built-in's name and WGSL diagnostic.

Only a crate-private `Builtin` enum can use the new trusted registration path.
Polygon rendering and joined text backgrounds use it; the public
`Core::add_fragment(&str)` API still validates user WGSL at runtime and emits
`fragment-rejected` warnings. Registry deduplication, source retrieval,
session sharing, and removal/re-registration remain intact. No public
unchecked registration API or opt-out flag was added.

Controlled counter comparison, with `--profile web-release --features devtools`
and Binaryen 133 `-Oz --strip-debug --strip-producers` on both sides:

| Measurement | Before | After |
| --- | ---: | ---: |
| Cargo output, with names | 5,398,651 | 4,384,597 |
| Shipping Wasm bytes | 4,332,922 | 3,567,993 |
| Shipping gzip-9 bytes | 1,747,899 | 1,464,101 |
| Function rows attributed to Naga by twiggy | 851 | **0** |
| Naga shallow function bytes | 668,821 | **0** |

This saves a further **764,929 raw bytes (17.7%)** and **283,798 gzip bytes
(16.2%)** on the devtools counter. These savings are additional to the web
profile/post-processing gains above. Naga remains a Cargo runtime dependency
for applications using user-defined shaders, and a build dependency for
checking the fixed ones; dependency presence is distinct from linked size.
The full gallery and Lua API still need runtime WGSL validation.

Verification:

- Eleven existing fragment unit tests, five new registration/validation tests,
  four joined-background tests, fifteen session tests, and the Lua fragment
  registration test pass. Syntax errors, semantic type errors, and edited
  built-in source are still rejected through the user API.
- Throwaway crate probes using the actual build script and shared module
  reject an invalid polygon, invalid join, and broken shared prelude; restoring
  valid source allows the next build to succeed. Production sources were not
  mutated for these failure probes.
- Full optimized book build and browser smoke suite: **61 checks, zero
  failures**, including the user-defined fragment example and Lua examples.
- Notes flows pass on Chromium, Firefox, and WebKit Canvas and Chrome WebGPU.
  The polygon example and devtools tree were opened, rendered, and closed on
  those same four browser/renderer combinations without page errors; the GPU
  tree/icons were visually inspected.
- Core Clippy with warnings denied, formatting, and rustdoc with broken links
  denied pass.

The target-independent shared module keeps build validation independent of
the core's UI/resource types and prevents a copied build-time shader wrapper
from drifting away from what backends actually compile.

## Font-library alignment follow-up (2026-11-13)

The workspace now uses one font-library generation for shaping, metrics, and
rasterization:

| Package | Before | After |
| --- | --- | --- |
| HarfRust | 0.5.2 | 0.12.0 |
| Skrifa | 0.40.0 and 0.44.0 | 0.44.0 |
| read-fonts | 0.37.0 and 0.41.0 | 0.41.0 |
| font-types | 0.11.3 and 0.12.5 | 0.12.5 |
| Swash | 0.2.10 | 0.2.10 |

Skrifa 0.44 is the version Swash already used. HarfRust 0.12 accepts the same
read-fonts 0.41 types, so the shaping/metrics side can move forward while the
rasterizer and its Skrifa implementation stay at their existing versions.
HarfRust's release notes identify the read-fonts update and the new
`ShapeOptions` API. The embedded shaper's call was adapted to pass its existing
cached plan and per-run features through that API. Font matching, fallback,
direction-cache keys, normalized variation coordinates, and the rasterization
policy were preserved.
[HarfRust 0.12 changelog](https://github.com/harfbuzz/harfrust/blob/0.12.0/CHANGELOG.md)

This upgrades the shaping dependency, including upstream shaping fixes; it is
more than a lockfile-only deduplication. The recorded cosmic-text source base
remains 0.19.0, with the small API adaptation documented in its
[vendor README](../crates/kui-core/vendor/cosmic-text/README.md). The lockfile
update changes HarfRust and removes the three older font packages, without
upgrading unrelated dependencies or introducing a source patch/fork.

Controlled comparison after the built-in shader change, still using the
counter with devtools, `web-release`, and Binaryen 133 `-Oz` with stripping:

| Measurement | Before | After |
| --- | ---: | ---: |
| Shipping Wasm bytes | 3,567,993 | 3,522,198 |
| Shipping gzip-9 bytes | 1,464,101 | 1,448,399 |
| `read_fonts` shallow function bytes | 279,595 | 246,630 |

The saving is **45,795 raw bytes (1.28%)**, or **15,702 gzip bytes (1.07%)**.
Both Cargo.lock and target-specific reverse dependency trees now contain one
read-fonts/Skrifa/font-types version. The named Wasm's twiggy output also has
one crate identity for each, down from two. The saving is smaller than the
sum of their old attributed sizes: different consumers retain different font
table operations, and LTO already removes unused code.

Validation completed:

- The original private shaper suite passed before editing dependencies.
- `GENERATE_IMAGES=0 cargo test --locked -p kui-core -p kui-cosmic-text-tests
  --no-fail-fast`: **2,281 passed, zero failures**, with nine existing ignored
  tests/doctests. No image reference was regenerated or changed. Coverage
  includes Arabic/Hebrew/bidirectional rendering, ligatures and font features,
  variable-weight matching, fallback laziness, direction-sensitive caches,
  glyph hinting, extreme text sizes, and text editing/layout behavior.
- Optimized counter/notes and complete book builds, including Lua/WASI and the
  packaged GPU renderer, pass. The book smoke suite passes **61 checks**.
- Existing notes interaction flows pass on Chromium, Firefox, and WebKit
  Canvas and Chrome WebGPU.
- Latin ligatures/punctuation, Cyrillic, and Greek editor glyphs were compared
  against the pre-upgrade site on those same four browser/renderer combinations;
  their text image crops were pixel-identical. The comparison excludes animated
  widget chrome, whose focus rings and antialiased panel borders can vary with
  frame timing independently of text rendering.
- Clippy for both packages' complete target sets passes with warnings denied;
  formatting and diff checks pass. Swash's version and the libraries' licenses
  are unchanged.

The dependency change is in [Cargo.toml](../Cargo.toml) and Cargo.lock; the only
embedded source adaptation is the shaping invocation in
[shape.rs](../crates/kui-core/vendor/cosmic-text/src/shape.rs).

## Embedded-font follow-up (2026-11-13)

The browser examples, book, and Lua/WASI runner now store the two large TTFs
as lossless LZMA streams. The shared private host decoder restores the exact
original bytes before passing them to `Session::with_fonts`. There is no
glyph subsetting or font-table rewriting: existing character coverage,
ligatures, positioning, names, hints, and font matching are preserved.

| Font payload | Original bytes | Embedded bytes |
| --- | ---: | ---: |
| Noto Sans Regular, counter/notes | 569,208 | 216,956 |
| Regular + Mono, book/Lua | 1,082,044 | 373,187 |

The larger hosts compress both TTFs together to share repeated font data, then
split the decoded buffer into the two independent original faces. They embed
only the combined stream; the smaller counter/notes embed only the Sans
stream. This cuts the combined payload by **65.5%**. The original Regular face
still has
3,748 glyphs / 2,840 mapped codepoints; Mono has 3,787 / 3,367. These counts
describe the existing bundled faces, not universal language coverage.
The small Symbols2 and book emoji faces are unchanged, and registration order
stays Sans, Symbols2, Mono (where used), then book emoji (where used). In
particular, Mono's arrows, Symbols2's shortcut/symbol glyphs, and the book's
three emoji remain available through the same fallback paths. Bold remains
a native test fixture and is not shipped on the web.

The implementation is in [bundled_fonts.rs](../examples/web/bundled_fonts.rs).
The output buffer is fixed to the source TTF length; the decoder's dictionary
has a 1 MiB limit. Decoding happens once per app session, without a fetch or
browser-specific decompression API. The source TTFs remain unchanged and are
used only for their compile-time lengths in shipping hosts; the compiler does
not retain their complete bytes alongside the compressed versions.
The `lzma-rs` dependency is pinned by Cargo.lock, with no encoder linked into
Wasm. For the public browser-host crate it is only an example dependency.

[compress-fonts.py](../scripts/compress-fonts.py) uses Python 3's standard
library to regenerate caches and replaces each cache atomically after a
successful roundtrip. `python3 scripts/compress-fonts.py --check` verifies
decoded equality, completion, and absence of trailing data without rewriting
anything. It deliberately does not compare the compressed bitstream, so an
equivalent result from another liblzma version does not look stale. Web
builds run this check before Cargo or output staging; `scripts/gen.nu` also
includes it. The Rust tests exercise the actual shipping decoder independently
of Python.

Zlib/Zopfli and Brotli storage were also measured. Zlib reduced raw/gzip size
but made Brotli downloads larger; the Brotli decoder's additional code and
dictionary outweighed its smaller font stream. LZMA reduced raw/gzip size while
keeping the counter's Brotli size essentially unchanged, at a higher one-time
decoding cost. Compressing Sans and Mono together also preserves their
inter-font redundancy in the larger hosts.

Controlled comparison after font-library alignment, using the same
`web-release` profile and Binaryen 133 optimization/stripping:

| Counter with devtools | Before | After |
| --- | ---: | ---: |
| Shipping Wasm bytes | 3,522,198 | 3,209,203 |
| Shipping gzip-9 bytes | 1,448,399 | 1,393,525 |
| Shipping Brotli-11 bytes | 1,111,753 | 1,112,030 |

That is **312,995 raw bytes (8.89%)** and **54,874 gzip bytes (3.79%)** saved,
including the decoder's cost. Counter Brotli grows by 277 bytes (0.025%),
which is effectively unchanged. Gzip uses the same Python settings as the other
follow-ups. Brotli measurements use Python brotli 1.2.0 at quality 11; these
are file measurements, not evidence of production HTTP content coding.

| Larger host | Raw before | Raw after | gzip-9 before | gzip-9 after |
| --- | ---: | ---: | ---: | ---: |
| Book Canvas gallery | 5,810,502 | 5,135,252 | 2,401,828 | 2,242,300 |
| Book WebGPU gallery | 5,989,701 | 5,314,913 | 2,498,583 | 2,337,479 |
| Book Lua gallery | 7,367,600 | 6,687,826 | 2,938,624 | 2,781,283 |
| Packaged Lua/WASI runner | 7,344,778 | 6,665,850 | 2,918,455 | 2,759,995 |

The combined stream saves **675,250 raw / 159,528 gzip bytes** in the Canvas
gallery and **678,928 raw / 158,460 gzip bytes** in the packaged runner.
Their Brotli sizes also improve: **1,801,628 → 1,794,435** for the Canvas
gallery and **2,189,348 → 2,179,073** for the runner. Inspections of the
shipping modules confirm that each retains only its selected compressed
stream, without the original TTFs or the other cache.

In local arm64 Chrome testing, 20 alternating fresh counter instances after
four warmups measured about **13 ms extra session initialization** for Sans;
the first frame remained about 1.5 ms. Compilation, instantiation, and
network loading were excluded. This is a one-time cost on this machine,
not a cross-device startup guarantee; decoded fonts use the same persistent
font storage as before.

Validation completed:

- `cargo test --locked -p kui-devtools --test bundled_fonts`: three tests
  pass, covering exact restoration of both cache layouts, malformed/truncated
  streams, wrong output lengths, trailing data, and fallback rendering.
  The rendering comparison checks exact GPU instances and atlas pixels for
  Latin ligatures, combining marks, Greek, Cyrillic, monospace, arrows,
  shortcut symbols, and emoji on Mac/Windows configurations at 1x and 2x.
- Final optimized counter/notes, full book, Lua/WASI, and packaged WebGPU
  builds pass. The book smoke suite passes **61 checks**, including Lua
  presentations, packaged tasks, and the legacy redirect.
- Notes interaction flows pass on Chromium, Firefox, and WebKit Canvas and
  Chrome WebGPU. Multilingual editor text crops match the uncompressed-font
  baseline pixel for pixel on all four combinations. The comparison verifies
  the editor's actual value after pasting and excludes animated widget chrome.
- The build-script test verifies stale/corrupt caches stop the build before
  Cargo or staging, and preserve an existing site. The Python cache check,
  native test Clippy, changed Wasm hosts' Clippy (`--no-deps`), formatting,
  diff checks, and a local `cargo audit --no-fetch` pass.
- An additional dependency-inclusive Wasm Clippy run encounters three
  existing `arc_with_non_send_sync` diagnostics in the unchanged
  `crates/kui-wgpu/src/lib.rs`; those do not occur in the scoped host checks.

## Current profile and next candidates (2026-11-13)

Remeasured the working tree after all four follow-ups above. The Canvas
counter with devtools and gallery were rebuilt using the existing helper and
`web-release`; the runner was profiled from the corresponding current WASI
build. Regenerated shipping outputs match the staged final modules exactly.
Other renderer rows below are remeasurements of the staged outputs from that
same source state, not estimates from pre-bindgen compiler inputs.

| Artifact | Shipping Wasm bytes | gzip-9 bytes |
| --- | ---: | ---: |
| Counter, Canvas, no optional features | 2,971,509 | 1,302,516 |
| Counter, Canvas, devtools | 3,209,203 | 1,393,525 |
| Counter, WebGPU, devtools, post-bindgen | 3,389,673 | 1,470,919 |
| Book gallery, Canvas | 5,135,252 | 2,242,300 |
| Book gallery, WebGPU, post-bindgen | 5,314,913 | 2,337,479 |
| Book Lua gallery | 6,687,826 | 2,781,283 |
| Packaged Lua/WASI runner | 6,665,850 | 2,759,995 |
| Packed WebGPU renderer, post-bindgen | 251,154 | 103,830 |

Devtools now add **237,694 raw / 91,009 gzip bytes** to the counter, rather
than the initial 1.79 MB / 520 KB. Neither counter variant retains Naga.
Turning devtools off remains an existing opt-out, not a new reduction that
preserves every enabled feature.

### Code dominates the remaining size

| Shipping section | Counter with devtools | Canvas gallery | Lua runner |
| --- | ---: | ---: | ---: |
| Function code | 2,606,679 | 4,142,490 | 5,171,814 |
| Initialized data | 596,392 | 982,481 | 1,479,063 |
| All other sections | 6,132 | 10,281 | 14,973 |

Code accounts for **81.2%, 80.7%, and 77.6%** respectively. Names, debug
information, and producers metadata are absent from shipping modules; the
small `target_features` section remains. There is no remaining large debug
section to strip.

For function attribution, optimize the named compiler snapshots with
`wasm-opt -Oz -g`, then run `twiggy top --format csv`. Binaryen otherwise
discards function names by default, even without `--strip-debug`. The named
and shipping variants were checked to have identical code/data section sizes.
The `-g` files are analysis artifacts only, never shipping outputs.

| Function owners, shallow bytes | Counter with devtools | Canvas gallery | Lua runner |
| --- | ---: | ---: | ---: |
| `kui_core`, including embedded cosmic-text | 1,015,905 | 1,200,358 | 1,187,517 |
| HarfRust + Skrifa + read-fonts + Swash + Zeno + Yazi | 808,402 | 808,059 | 808,233 |
| `core` + `std` + `alloc` + `hashbrown` | 449,126 | 557,867 | 690,453 |
| Naga | 0 | 463,872 | 456,723 |
| SVG stack, grouped | 0 | 0 | 655,773 |
| `mlua` + `kui_lua` | 0 | 0 | 673,211 |
| Image codecs and helpers, grouped | 0 | 310,468 | — |

These are first-crate attributions, not exclusive feature budgets: inlining
can place font code inside a core/host function, and generic functions are
attributed to their first crate. Lua's C functions add at least another
61,498 named shallow bytes beyond the Rust bindings above. SVG grouping here
includes tiny-skia, tiny-skia-path, usvg, resvg, svgtypes, XML/CSS parsing,
kurbo, and data-URL helpers. The runner also has 17,593 attributed
flate2/miniz bytes, consistent with accepted SVGZ input, not the gallery's
JPEG/PNG/WebP/GIF decoders. Enabled Cargo features alone do not establish
linked code cost: the gallery still has no first-attributed live SVG stack.

The book's own example code contributes another 425,606 shallow bytes. Large
optimized functions include the counter's `frame` export (191,678), devtools
panel (about 126–130 KB across hosts), and fallback shaping (about 54 KB).
Those functions contain inlined work; their sizes do not imply equally large
removable features.

Embedded font payloads are now **223,740 bytes** in the counter, **389,683**
in the gallery, and **379,971** in the runner, including the small fallback
faces. That is only **7.0%, 7.6%, and 5.7%** of their shipping modules.
Remaining data also includes Unicode tables, strings, source, and panic
locations. A large 116,088-byte counter data segment begins with source
location records referencing `unicode-bidi`'s `prepare.rs`; it must not be
mistaken for another embedded font. `panic=immediate-abort` is not available
on the pinned stable compiler without nightly/unstable options, so it was
excluded as a robust build-policy recommendation.

### Measured next reduction: lossless Kit source storage

The runner contains **331,787 bytes of readable Lua source** across the
included Kit modules, adapter prelude, argument helper, and application
wrapper. An isolated WASI prototype changes only the browser runner's Kit
loader, leaving the native CLI loader and adapter prelude unchanged.

The 18 Kit sources total **291,242 bytes** and compress together to **60,182
bytes** using the font pipeline's LZMA settings. The prototype uses the
already-linked decoder, bounded output/dictionary sizes, a one-time source
cache, and generated slice ranges. It preserves each original UTF-8 source
byte, `package.preload`/`require` execution, and existing chunk names; it does
not minify or precompile Lua, so source line numbering is preserved.
[Current Kit loader](../crates/kui-cli/src/kit.rs),
[browser inclusion](../crates/kui-web-lua/src/app.rs)

| Packaged runner | Before | Kit prototype |
| --- | ---: | ---: |
| Shipping raw bytes | 6,665,850 | 6,435,930 |
| Shipping gzip-9 bytes | 2,759,995 | 2,751,662 |

The actual saving, including indexing/loading overhead, is **229,920 raw
bytes (3.45%)** and **8,333 gzip bytes (0.30%)**. This final comparison uses a
fresh, separate Cargo target directory and the original full shader
diagnostics, so it does not accidentally include the diagnostic experiment
below. Source roundtrips and all 18 slice ranges were checked byte for byte.
The prototype starts and adds a task in Chromium, Firefox, and WebKit Canvas
and Chrome WebGPU, without page errors.

This is the next largest measured low-risk reduction for the Lua runner.
Before adoption, integrate generated-cache verification and update handling
like the fonts, test loader/chunk-name behavior explicitly, and measure
initialization and memory. The decoded source cache lives as long as preload
closures; smaller Wasm does not imply less runtime memory. It gives no
corresponding saving to the Rust-only counter/gallery, which do not embed
these Kit sources.

### Other measured probes

| Isolated probe | Raw-byte change | gzip-byte change | Assessment |
| --- | ---: | ---: | --- |
| Compact WGSL diagnostics, gallery | −35,430 | −14,896 | Worth adopting with diagnostic tests |
| Compact WGSL diagnostics, runner | −36,373 | −14,451 | Same parser/validator retained |
| Binaryen `--converge`, counter with devtools | −4,274 | −1,852 | Small build-only opportunity |
| Binaryen `--converge`, gallery | −10,842 | −7,228 | Same pinned optimizer |
| Binaryen `--converge`, runner | −4,828 | −3,002 | Same pinned optimizer |
| Three font sorts changed to unstable, counter | +4,695 | +1,703 | Reject: larger output |

The diagnostic prototype keeps the assembled WGSL, parser, all validation
flags, and capabilities unchanged. It prints Naga's messages, every span and
note, and semantic cause chains as plain text with app-relative locations.
It removes terminal-style source excerpts, and columns become documented
UTF-8 byte positions. The existing eleven fragment unit tests and five
registration/validation tests pass. Gallery codespan-reporting function
attribution drops from 17,924 bytes to zero while Naga remains about 464 KB;
the rest of the saving comes from supporting formatting/data. User shaders
still receive runtime validation. This changes diagnostic presentation and
needs focused multi-span/Unicode/missing-span tests before integration.

`--converge` repeats Binaryen's optimization passes while binary size falls.
It preserves features and changes no application code, but adds optimization
work. These probes pass Binaryen's normal module validation; browser
regression checks and build-time measurement remain adoption checks.

The font-sort change is semantically defensible for these three totally
ordered lists, but measured output grows; twiggy's sort monomorphization
estimates are not a savings budget. Dropping hinting, Unicode/script support,
image formats, SVG/SVGZ, or user-shader validation would remove existing
behavior and is not recommended as the next safe step.

No production source or build-policy changes from these probes were adopted.
The snapshots, CSV profiles, section/group summaries, and experiment outputs
are under `target/wasm-size-current/`; temporary implementations were kept
outside the repository. See [candidate source research](wasm-size-candidates.md)
for API contracts, primary-source links, and further test requirements.

## Robustness review and rollback (2026-11-13)

Reviewed the full working diff against `167ed8d0`, including new files, against
the requirements in this chat. No further size optimization was adopted.

### Standards review

One confirmed failure was found and fixed. Previously `web-counter.nu` ran
wasm-bindgen directly in the output directory and only then optimized the
generated Wasm. An optimizer failure therefore left new, unoptimized output
in place of the previous bundle, despite the helper's preservation guarantee.

The shared [bindgen helper](../scripts/web-build.nu) now generates and
optimizes everything in a temporary directory before publishing it. Both
browser and packed-runner builds use it. The regression test uses real
wasm-bindgen and Binaryen, then forces a failing optimizer/CLI and checks that
the previous JS and Wasm bytes survive, fresh output stays absent, and staging
directories are removed. CI runs it after its existing book/tool setup.

The complete site still is not an atomic deployment transaction. HTML and
other assets can have been copied before a later build/copy fails. Stop on a
nonzero exit and rerun successfully before serving or deploying that directory.
Adding a directory-swap deployment system would be a separate requirement,
not a reason to build more machinery into this size change.

### Requirements review

No shader-validation or shaping requirement was found missing. Fixed shaders
use a closed private enum and the same source assembly and validator in the
build script. User-provided shaders retain runtime parsing, validation, and
diagnostics. The font dependency update preserves the cached plan, features,
coordinates, and fallback policy; it still includes upstream shaping changes
and therefore affects native consumers as well as the browser.

Font compression was technically guarded but had the weakest complexity
tradeoff. It added an LZMA decoder, two generated binary assets, a Python/liblzma
check/regeneration workflow, temporary decoding memory, and synchronous app
startup work. The measured counter download benefit was about 55 KB with gzip
and effectively zero with Brotli, with about 13 ms extra initialization on the
tested fast machine. This was reverted completely, including the generator,
preflight, tests/dependencies, host-loader changes, and binary caches. Original
TTFs and fallback order are restored. No Kit compression, compact diagnostic
formatter, unstable font sorts, or optimizer convergence changes were adopted.

### Remaining change surface and failure conditions

The review reduces **42 changed/new files to 27**: 20 existing files and seven
new files. Two are research reports. Most of the apparent shader diff is a
roughly 500-line module/test move so build and runtime can share one source,
not a shader rewrite.

| Area | Files involved | When it can fail | Protection / decision |
| --- | ---: | --- | --- |
| Web profile, tool pin, scripts, CI and documentation | 13 | Missing/wrong Binaryen or bindgen; unsupported host asset; Cargo/bindgen/optimizer failure; interrupted site publication | Exact tool versions/checksums, explicit setup errors, shared helper, failure-preservation tests; keep |
| Fixed-shader build validation and registration | 8 | Editing shader/wrapper to invalid WGSL; upgrading Naga; adding a trusted built-in without updating its build-validation list | Same source/validator, closed enum, compile-time failure for current built-ins, positive/negative registration tests; keep |
| Font-library alignment | 5 | Upstream shaping changes on unusual fonts/scripts; vendor refresh dropping the API adaptation; future dependency updates splitting versions again | Lockfile, documented adaptation, rendering-reference suite; keep |
| Research reports | 2 | Treating an old experiment as the adopted configuration | Historical/current status called out explicitly |

The per-area counts overlap in `Cargo.toml`, giving 27 unique files. Native
release optimization settings are unchanged. Native font behavior and core
build prerequisites can change because the library update and build-time Naga
validation are shared. Clean builds do additional host validation work; this
does not embed that host build dependency in the target Wasm.

The original TTFs increase Wasm size back to the font-alignment stage:
counter/devtools about **3.52 MB raw / 1.45 MB gzip**, Canvas gallery
**5.81 MB / 2.40 MB**, and Lua runner **7.34 MB / 2.92 MB**. The much larger
earlier savings remain. Local executable testing is on macOS arm64; other host
assets are checksum-pinned, but Windows/Linux ARM build execution is not
claimed as locally verified.

Post-review verification completed:

- Fresh builds of the full book (including Lua/WASI and packaged GPU runtime)
  and the devtools counter/notes pass with the staged bindgen helper.
- `GENERATE_IMAGES=0 cargo test --locked -p kui-core -p
  kui-cosmic-text-tests --no-fail-fast`: **2,281 passed, zero failures, nine
  existing ignored tests/doctests**. No rendering reference was regenerated.
- The book browser smoke suite passes **61 checks**. Notes interaction flows
  pass on Chromium, Firefox, and WebKit Canvas and Chrome WebGPU.
- The expanded `web-build-test.nu` passes success, invalid-input, in-place,
  custom-target-directory, bindgen/optimizer failure preservation, and cleanup
  cases. Core/shaper all-targets Clippy with warnings denied, formatting, and
  diff checks pass.
- Actual restored shipping sizes are counter/devtools **3,522,198 raw /
  1,448,399 gzip bytes**, Canvas gallery **5,810,502 / 2,401,828**, and packaged
  Lua runner **7,344,778 / 2,918,455**. They match the earlier font-alignment
  baseline; the rollback did not undo the larger build/shader improvements.
