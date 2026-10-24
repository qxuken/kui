# Teal's compiler, for the tests

`tl.lua` is the compiler of [Teal](https://github.com/teal-language/tl),
the typed Lua dialect, at the v0.24.8 release (2025-10-13), unmodified:
`https://raw.githubusercontent.com/teal-language/tl/v0.24.8/tl.lua`,
sha256 `ddb28d177f057ebe4face7aa69d92e9f8ad9bd17004fa88c53ec9370f1862c29`.
MIT, its `LICENSE` beside it.

`src/teal.rs`'s tests load it into the crate's own Lua 5.5 and check
`kui_lua::teal_decls()` with it, so the file is held to the release a
project installs. The directory is excluded from the published crate.
Moving to a newer release is replacing both files and running
`cargo test -p kui-lua --lib teal`.
