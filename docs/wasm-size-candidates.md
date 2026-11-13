# Further Wasm size candidates

**Research only; optimization work has stopped.** The later robustness review
reverted embedded-font compression and kept the earlier build/shader/dependency
changes. None of the prototypes in this note was adopted. Size projections
below use the pre-rollback baseline; see the
[review decision](wasm-size.md#robustness-review-and-rollback-2026-11-13).

Investigated 2026-11-13 against the working tree after the web release profile,
Binaryen 133, built-in shader validation, font-library alignment, and lossless
font storage changes recorded in [wasm-size.md](wasm-size.md). This is source
research, not a second set of size measurements: no builds or production-source
changes were made for this note. Cargo queries used `--locked` and explicit
package/target selections.

The remaining graph does not expose another obvious large, unused default
feature switch. Subsequent controlled measurements are recorded in the
[current profile](wasm-size.md#current-profile-and-next-candidates-2026-11-13).
They reject the stable-sort experiment below: the counter grows by 4,695 raw
and 1,703 gzip bytes. Compact WGSL diagnostics save about 35–36 KB raw.
The larger opportunity is lossless storage of the Lua runner's Kit sources,
which saves 229,920 raw / 8,333 gzip bytes in an isolated fresh-target build.
Several tempting dependency removals would either save only build work or cut
existing behavior.

## Candidates that preserve supported features

### Lossless Kit source storage in the browser runner

The WASI runner includes the CLI's [Kit loader](../crates/kui-cli/src/kit.rs)
through [app.rs](../crates/kui-web-lua/src/app.rs). Its 18 source inclusions
occupy 291,242 bytes, all found byte-for-byte in the baseline shipping module.
The full Lua-source census, also including the adapter prelude, argument
helpers, and browser application wrapper, is 331,787 bytes. The probe keeps
those latter inclusions unchanged and compresses only Kit's 18 sources.

Concatenating those sources and using the font pipeline's LZMA settings gives
60,182 bytes. The isolated WASI prototype restores exact bytes into a bounded
buffer, stores it once, and hands the loader slices with the same chunk names.
It leaves the native CLI loader and public adapter API unchanged. This uses
the decoder already linked for browser fonts; no new runtime codec is needed.
The prototype's actual module saving is smaller than the payload saving,
because source indexing and loading add code/data.
[Existing font decoder](../examples/web/bundled_fonts.rs),
[existing lazy preload and chunk naming](../crates/kui-cli/src/kit.rs)

Adoption should retain `package.preload`/`require` behavior, original UTF-8,
all whitespace and source line numbers, and `set_name` values. Use generated
offset/length metadata, exact decoded-equality checks in generation and CI,
bounded decoding, and a source-update preflight like the font cache check.
Measure initialization cost and memory: decoded source must remain valid for
the lifetime of preload closures. This is an artifact-size reduction, not a
claim that decoded source needs less memory. Browser smoke outcomes and exact
artifact measurements are in the current-profile report.

### Use unstable sorting for three totally ordered font lists

Measured outcome: reject this as a size optimization. The isolated counter
build grows by 4,695 raw / 1,703 gzip bytes after Binaryen, despite the value
ordering argument below. The generic-sort family totals are not removable
byte estimates.

Three calls in the embedded
[font system](../crates/kui-core/vendor/cosmic-text/src/font/system.rs) sort
plain font IDs or complete `FontMatchKey` values with `sort()`:

- `monospace_font_ids` is a `Vec<fontdb::ID>`, used for binary search.
- `get_monospace_ids_for_scripts` collects IDs, sorts, then deduplicates.
- `get_font_matches` sorts complete `FontMatchKey` structs using their derived
  `Ord` before applying the existing explicit `db.query` preference.

Switching these three calls to `sort_unstable()` preserves the resulting value
sequences. Equal IDs are indistinguishable scalar values. `FontMatchKey`
derives `Ord` and equality over all of its fields, including the unique face
ID; comparator-equal keys contain the same values in every field. Reordering
equal entries therefore cannot change fallback selection. There is no
user-provided comparison callback with side effects. This is a narrow safety
argument for these calls, not for sorts based on partial paint/layout keys.
[KUI's complete key and call sites](../crates/kui-core/vendor/cosmic-text/src/font/system.rs),
[derived Ord semantics](https://doc.rust-lang.org/std/cmp/trait.Ord.html#derivable),
[fontdb ID definition](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/src/lib.rs)

Rust documents `sort_unstable` as an in-place sort without allocation that may
reorder equal elements. Its implementation need not retain the same stable
merge-sort families. The linked saving is still an experiment: other stable
sort callers can retain shared helpers, and monomorphization/shallow-size
estimates cannot establish removable bytes. Compare multilingual fallback,
equal-style family matching, variable-weight matching, glyph atlas output,
and the private shaper suite when adopting the change.
[Rust slice sorting contract](https://doc.rust-lang.org/std/primitive.slice.html#method.sort_unstable),
[existing text validation](wasm-size.md)

### Compact runtime WGSL diagnostics

[The shared validator](../crates/kui-core/src/fragment/shader.rs) calls Naga's
`emit_to_string` for both parse and semantic errors. Naga 30 implements these
calls through `codespan_reporting`, diagnostic labels, source-file handling,
and terminal-style rendering. This can retain substantially more presentation
machinery than a plain message and source location, even though no terminal is
present. It is a promising ablation, not a quantified saving here.
[Naga parse error source](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/src/front/wgsl/error.rs),
[Naga spanned error source](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/src/span.rs)

The public APIs expose the information needed without the pretty-printer:

- Parse errors: `message()`, every `(Span, message)` from `labels()`, and
  every string from `notes()`.
- Semantic errors: `as_inner()` for the principal error, every span/context
  from `spans()`, and the complete `std::error::Error::source` chain starting
  at the inner error.
- Each defined span has `location(full_source)` and `to_range()`.

These APIs allow the same Naga messages, labels, and causes to be presented in
plain text. Calling only `to_string()` and `location()` loses parse notes,
secondary spans, and semantic error causes; that would be a diagnostic
regression. [Naga parse error API implementation](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/src/front/wgsl/error.rs),
[Naga span API implementation](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/src/span.rs)

The supported contract is rejection before registration, the
`fragment-rejected` warning, Naga's message, and line numbers in the app's
source. It does not require codespan typography. Existing tests assert an
app-relative `wgsl:2:` or `wgsl:3:`, a missing-function message containing
`fragment`, and rejection of invalid syntax/signatures/semantics. This makes a
plain formatter a reasonable feature-preserving option, with an explicit
change to diagnostic presentation.
[ADR 0015](adr/0015-a-fragment-element-and-the-painter-it-is-not.md),
[registration contract](../crates/kui-core/src/runtime/resources_api.rs),
[shader tests](../crates/kui-core/src/fragment/shader.rs),
[registration tests](../crates/kui-core/tests/fragments.rs)

Keep the same assembled source, `ValidationFlags::all()`, and
`Capabilities::empty()`. Convert line numbers with the existing prelude offset
and clamp, including spans in the epilogue. Naga locations use UTF-8 byte
columns; do not silently describe those as visual columns. If source snippets
are included, preserve control-character sanitization. Include every note and
error-chain entry, and cover multiline errors, multiple labels, Unicode,
missing spans, and errors in the wrapper in focused tests. Since the module is
shared with `build.rs`, its formatter also determines built-in build-failure
diagnostics. No runtime-validator feature can be dropped from general Lua
hosts, which expose arbitrary fragment registration.
[Naga location definition and diagnostic construction](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/src/span.rs),
[shared validator](../crates/kui-core/src/fragment/shader.rs),
[Lua registration](../crates/kui-lua/src/lib.rs)

Measure this first on the Canvas gallery and WASI runner. The minimal counter
already drops Naga, so it should have no comparable saving. A dependency can
remain in Cargo.lock after its live pretty-printing functions disappear.
Naga has no default features and its mandatory codespan dependency is not a
separately disableable diagnostic feature.
[Naga manifest](https://github.com/gfx-rs/wgpu/blob/v30.0.1/naga/Cargo.toml),
[previous linked-size findings](wasm-size.md)

### Specialize image decoding without removing formats

Only the browser book gallery enables `image` in the examined shipping graphs;
the WASI packed runner does not have it. The gallery selects exactly
`gif,jpeg,png,webp`, with defaults disabled. It already avoids AVIF, EXR, TIFF,
Rayon, and the other default formats. Removing those absent features is not a
new opportunity.
[gallery manifest](../examples/book-web/Cargo.toml),
[runner manifest](../crates/kui-web-lua/Cargo.toml),
[image 0.25.10 features](https://github.com/image-rs/image/blob/v0.25.10/Cargo.toml)

[The shared decoder](../crates/kui-native/src/decode.rs) first creates a generic
`ImageReader` to read dimensions, then decodes into `DynamicImage`, then converts
to RGBA8. A throwaway experiment could dispatch the same four formats directly
to concrete decoders, preserve image limits, decode once, and convert only
output sample types that those codecs actually support. DynamicImage contains
8-bit, 16-bit, and floating-point variants; its general conversion functions
handle all of them. Whether LTO already prunes those paths must be measured.
[image DynamicImage source](https://github.com/image-rs/image/blob/v0.25.10/src/images/dynimage.rs)

This is not automatically safe: retain 16-bit PNG handling, alpha and grayscale
conversion, animation compositing/timing/loop semantics, the current memory
ceilings, and useful malformed-input errors. Directly requesting RGBA from an
underlying JPEG decoder can also alter conversions. Require byte-for-byte
decoded-pixel comparisons over diverse fixtures, alongside malformed/oversized
input coverage. Treat a codec deletion as a separate feature decision, not as
an implementation of this candidate.
[current decoding and limit behavior](../crates/kui-native/src/decode.rs)

### Reduce formatting duplication only after retained-path evidence

There are many formatting calls in devtools and core, but their count is not a
size budget. A conservative change can replace temporary `format!` strings
followed immediately by `push_str` with `write!` into the same destination;
this preserves formatting while reducing allocations. It may have little or
negative effect on Wasm size once common formatting code is shared.
[devtools value formatter](../crates/kui-core/src/runtime/devtools/mod.rs),
[devtools panel formatting](../crates/kui-core/src/runtime/devtools/panel.rs)

Float formatting is real behavior here. Sliders intentionally round-trip f32
values through their shortest decimal spelling and use precision-controlled
decimal output to prevent values such as `0.30000001192092896` appearing on the
wire. Replacing this with casts or multiplication/round/division is not a
proven behavior-preserving reduction. Devtools additionally formats fractions
and timing values. Use final, named, optimized Wasm retained paths to identify
which formatter is exclusive to which caller before rewriting any numeric
behavior. [slider arithmetic contract](../crates/kui-core/src/slider.rs),
[devtools formatting](../crates/kui-core/src/runtime/devtools/panel.rs)

## Feature audit: attractive switches already absent or not free

| Area | Observed resolved features / source | Assessment |
| --- | --- | --- |
| Swash 0.2.10 | `default,render,scale,std` | These defaults are exactly the scaler, rasterizer, and std support used by KUI. There is no separate default shaping engine to disable. |
| Skrifa 0.44.0 | `std` | `autohint_shaping`, `traversal`, `spec_next`, and `libm` are absent. Disabling defaults again cannot remove them. |
| read-fonts 0.41 | `experimental_font_api,std` | HarfRust deliberately requests the experimental API. One parser generation remains after alignment. |
| usvg/resvg 0.46 | no default features | SVG text, system-font discovery, and resvg raster-image decoding are already disabled. |
| tiny-skia 0.11.4 via resvg | `default,png,png-format,simd,std` | PNG helper feature is enabled transitively, but resvg's image decode module is cfg-disabled. Dependency presence alone is not evidence of live PNG functions. |
| fontdb 0.23 | `fs,memmap,memmap2,std` | Enabled intentionally by the workspace; `Source::SharedFile` is used throughout the catalog. Removing memmap is not a one-line manifest change. |

Sources: isolated `cargo tree` queries below; [Swash exact published revision](https://github.com/dfrg/swash/blob/7773843df0d63cd468db61a29c152b5e7a99d4ab/Cargo.toml),
[Skrifa manifest](https://github.com/googlefonts/fontations/blob/skrifa-v0.44.0/skrifa/Cargo.toml),
[HarfRust workspace dependencies](https://github.com/harfbuzz/harfrust/blob/0.12.0/Cargo.toml),
[resvg manifest](https://github.com/linebender/resvg/blob/v0.46.0/crates/resvg/Cargo.toml),
[resvg image rendering](https://github.com/linebender/resvg/blob/v0.46.0/crates/resvg/src/image.rs),
[fontdb features](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/Cargo.toml),
[catalog source handling](../crates/kui-core/src/text.rs).

For tiny-skia, a maintained upstream change making `png-format` conditional on
resvg's `raster-images` could reduce compilation/dependency work without
changing this application's behavior. It is low priority for shipping bytes
until an ablation shows live code. A direct `tiny-skia` dependency with
`default-features=false` cannot disable defaults enabled by resvg. Cargo unions
features across consumers. [Cargo feature unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification)

Fontdb's `SharedFile` enum variant is itself gated by `fs + memmap`. KUI creates
and accepts that variant in its public font-catalog plumbing. Target-specific
gating for `wasm32-unknown-unknown` could remove unsupported filesystem paths,
but requires source/API decisions and may save almost nothing after LTO.
Do not apply the same assumption to WASI, where packed SVG files are read from
the filesystem. [fontdb source](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/src/lib.rs),
[KUI catalog](../crates/kui-core/src/text.rs),
[packed runner file resolver](../crates/kui-web-lua/src/lib.rs),
[Rust unknown-unknown target behavior](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html)

Fontdb still parses metadata through ttf-parser while shaping/metrics use
read-fonts. Eliminating ttf-parser would require replacing fontdb or changing
its parser and reproducing family naming, collections, variation metadata,
and matching. This is a larger compatibility project, not another safe version
alignment. [fontdb parser dependency](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/Cargo.toml),
[fontdb parsing implementation](https://github.com/RazrFalcon/fontdb/blob/v0.23.0/src/lib.rs)

## Existing behavior that must not be counted as a free removal

- **SVGZ:** the core manifest comment currently says no svgz, but usvg's
  gzip support is unconditional. Both initial parse and recoloring call
  `Tree::from_data`, which recognizes gzip magic and decompresses it. The
  default data-URL resolver can also recursively parse SVG content through
  `from_data_nested`. Switching just the outer call to `from_str` both cuts
  accepted compressed input and may leave nested decompression live. This is
  not a default feature that can simply be switched off.
  [KUI SVG implementation](../crates/kui-core/src/svg.rs),
  [usvg parser](https://github.com/linebender/resvg/blob/v0.46.0/crates/usvg/src/parser/mod.rs),
  [usvg image resolver](https://github.com/linebender/resvg/blob/v0.46.0/crates/usvg/src/parser/image.rs)
- **SVG filters, gradients, masks, and nested SVG images:** these require the
  existing resvg/tiny-skia fallback when KUI cannot render an SVG directly as
  paths. Replacing all SVGs with paths, or removing resvg, cuts those cases.
  [SVG fallback implementation](../crates/kui-core/src/svg.rs)
- **Font hinting and color/bitmap glyphs:** KUI explicitly enables hinting
  when permitted and tries color outlines, color bitmaps, then ordinary
  outlines. Removing Skrifa's interpreter, Yazi, or those source kinds changes
  supported font rendering. A static bundled face is not proof that app-loaded
  fonts cannot use the other paths. [glyph rasterization](../crates/kui-core/src/text.rs)
- **JPEG, WebP, GIF, APNG:** all are named by the shared decoder contract;
  deleting a feature or retaining only the gallery's present GIF fixture is a
  restricted gallery build. Browser decoding would preserve format breadth
  only with additional asynchronous host integration and potential pixel/timing
  differences. [image decoder](../crates/kui-native/src/decode.rs)
- **Panic and error messages:** replacing formatted failures with traps or
  terse constants changes diagnostics. `panic="abort"` already defaults on
  `wasm32-unknown-unknown`; it is not a new saving.
  [Rust target panic policy](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html#unwinding)

## Cargo feature isolation and reproducible ablations

The workspace already uses resolver 3. Build/proc-macro dependencies remain
separate from target dependencies, so host-only Naga compilation is not a
second runtime copy. Dev dependencies can still affect example/test builds,
and features of simultaneously selected workspace packages union. Keep each
shipping entry point in a separate Cargo invocation; do not compare a
workspace-wide all-features build with a package-specific baseline.
[Cargo resolver feature behavior](https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2),
[resolver versions](https://doc.rust-lang.org/cargo/reference/resolver.html#resolver-versions),
[workspace manifest](../Cargo.toml)

Read-only commands used for this audit:

```sh
cargo tree --locked -p kui-web --target wasm32-unknown-unknown -e features -i fontdb
cargo tree --locked -p kui-book-web --target wasm32-unknown-unknown -e features -i skrifa
cargo tree --locked -p kui-book-web --target wasm32-unknown-unknown -e features -i tiny-skia
cargo tree --locked -p kui-book-web --target wasm32-unknown-unknown -f '{p} {f}'
cargo tree --locked -p kui-web-lua --target wasm32-wasip1 -d
cargo tree --locked -p kui-web-lua --target wasm32-wasip1 -e features -i image
```

The final command reports that `image` is not present. Duplicate rows for the
same Naga/log/bitflags versions can reflect host versus target compilation,
not multiple shipped identities. The book graph contains png 0.17 through
tiny-skia and png 0.18 through image, but the former's runtime decode caller is
disabled; do not estimate savings by adding their compiled sizes.

For each subsequent ablation, compare raw, gzip-9, and Brotli-11 output at the
same final shipping stage and inspect a separately retained named module.
Record whether the change preserves all supported inputs, changes diagnostic
presentation, or is an explicit product feature cut. The existing methodology
and accepted browser/interaction checks are in [wasm-size.md](wasm-size.md).
