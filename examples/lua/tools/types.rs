//! Writes the Lua DSL's type files from kui-core's schema and the prelude,
//! for a script's editor and checker: `kui.lua`, the `---@meta` file for
//! lua-language-server (`kui_lua::luals_meta`), and `kui.d.tl`, the
//! declaration file for Teal (`kui_lua::teal_decls`).
//!
//!     cargo run -p kui-lua --example lua-types -- packages/lua
//!
//! `nu scripts/gen.nu` runs it with every other generator, and kui-lua's
//! `tests/types.rs` fails while the copies committed under `packages/lua`
//! are not what it writes.

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: lua-types <directory>"),
    );
    std::fs::create_dir_all(&dir)?;
    for (name, text) in [
        ("kui.lua", kui_lua::luals_meta()),
        ("kui.d.tl", kui_lua::teal_decls()),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, text)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
