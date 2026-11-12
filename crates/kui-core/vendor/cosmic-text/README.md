# Embedded cosmic-text

Kui's text shaper is derived from cosmic-text 0.19.0. The upstream source
and font fixtures retain their original MIT, Apache-2.0 and font licenses.
[Upstream cosmic-text](https://github.com/pop-os/cosmic-text)

The published upstream base is 0.19.0, from commit
`c24886c2471e5606587c46090cd25dbbf209186b`.
[Upstream base commit](https://github.com/pop-os/cosmic-text/commit/c24886c2471e5606587c46090cd25dbbf209186b)

The local change defers variable-weight eligibility checks until fallback
selection reaches a candidate. Matching and candidate ordering use the
same metadata and rules. This lets the native font worker prepare the
files a text actually needs while keeping the complete font inventory.
Monospace fallback yields its preferred face before preparing the
remaining candidates and resumes their existing order if shaping needs them.
Additional edits satisfy the workspace's lint checks.

The shape-run cache also includes the run's direction. This fixes FZ14
and FZ18, where neutral text shaped right to left could be reused in a
left-to-right paragraph, including another window sharing the session.

The source is a private module of kui-core and ships inside its package.
The unpublished `kui-cosmic-text-tests` workspace package in
`crates/kui-cosmic-text-tests` compiles the same source for upstream tests and
direct shaper benchmarks. Its fonts and test images stay in that test
package.

Module integration qualifies upstream `crate::` paths through
`crate::cosmic_text::` and fixes the former feature predicates to kui's
configuration. `std`, `swash`, and `shape-run-cache` are enabled. Other
upstream feature predicates are disabled. These mechanical adaptations
apply to imported updates along with the functional fixes.
Desktop discovery constructors live in the private test harness. The
embedded runtime receives font metadata and byte sources from the host.
Standalone upstream doctest examples remain as reference text. The
private harness runs the upstream integration and rendering tests.

The embedded module inherits kui-core's compilation profile. The private
harness is optimized separately in development. Use Core for application
measurements and the bench profile for comparisons with the direct shaper
benchmark.

Updating this copy requires an explicit vendor refresh. Compare a new
published upstream release with the recorded base, merge its changes into
this directory, and resolve the local differences. The font-loading
patches are in `src/font/system.rs` and `src/font/fallback/mod.rs`.
The direction fix is in `src/shape_run_cache.rs` and `src/shape.rs`.
Regression coverage is in the private harness's `font_match_laziness`
tests and kui-core's `tests/shaped_direction.rs`. Preserve the private
module paths, workspace dependencies, licenses, and modification notices.
Update the base version and commit recorded here after the refresh.

Validate a refresh with upstream's shaping and rendering tests, the kui
workspace tests, native startup measurements, and portable-core checks.
Review the new release's dependencies and license notices as part of the
update. When upstream supplies equivalent lazy matching and fallback
behavior, remove the corresponding local patches. Once it covers both
changes and the direction key, the implementation can return to an
upstream cosmic-text dependency.
