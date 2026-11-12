# kui-cosmic-text

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

The Rust library name remains `cosmic_text`. The distinct package name
makes downstream builds use the corrected implementation through ordinary
Cargo dependency resolution.

Updating this copy requires an explicit vendor refresh. Compare a new
published upstream release with the recorded base, merge its changes into
this crate, and resolve the local differences. The functional patches are
in `src/font/system.rs` and `src/font/fallback/mod.rs`, with regression
coverage in `tests/font_match_laziness.rs`. Preserve the package name,
workspace integration, licenses, and modification notices. Update the base
version and commit recorded here after the refresh.

Validate a refresh with upstream's shaping and rendering tests, the kui
workspace tests, native startup measurements, and portable-core checks.
Review the new release's dependencies and license notices as part of the
update. When upstream supplies equivalent lazy matching and fallback
behavior, remove the corresponding local patches. Once it covers both
changes, the dependency can return to upstream cosmic-text.
