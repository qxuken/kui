//! The type files committed under `packages/lua` — the `---@meta` file for
//! lua-language-server and the `kui.d.tl` for Teal — are what the crate
//! writes today, so a script's author who copies them, rather than calling
//! `luals_meta()` or `teal_decls()` from Rust, gets the DSL this version
//! parses. `nu scripts/gen.nu` writes them again.

use std::path::Path;

#[test]
fn the_committed_type_files_are_current() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/lua");
    let mut stale = Vec::new();
    for (name, want) in [
        ("kui.lua", kui_lua::luals_meta()),
        ("kui.d.tl", kui_lua::teal_decls()),
    ] {
        let got = std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        if got != want {
            stale.push(name);
        }
    }
    assert!(
        stale.is_empty(),
        "packages/lua/{} is stale: run `nu scripts/gen.nu`",
        stale.join(", packages/lua/")
    );
}
