# kui-cosmic-text

Kui's text shaper is derived from cosmic-text 0.19.0. The upstream source
and font fixtures retain their original MIT, Apache-2.0 and font licenses.
[Upstream cosmic-text](https://github.com/pop-os/cosmic-text)

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
