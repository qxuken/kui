//! Source-only Lua loading for the desktop, embedded and browser runners.

use mlua::{Function, Lua, Table, chunk::ChunkMode};
use std::path::Path;

/// Load a Lua file. Only the desktop runner's own packed bytecode may opt out
/// of source-only mode.
pub fn load_file(
    lua: &Lua,
    path: &Path,
    env: Option<Table>,
    allow_bytecode: bool,
) -> (Option<Function>, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => return (None, Some(format!("cannot open {}: {err}", path.display()))),
    };
    let mut chunk = lua.load(bytes).set_name(format!("@{}", path.display()));
    if !allow_bytecode {
        chunk = chunk.set_mode(ChunkMode::Text);
    }
    if let Some(env) = env {
        chunk = chunk.set_environment(env);
    }
    match chunk.into_function() {
        Ok(f) => (Some(f), None),
        Err(err) => (None, Some(err.to_string())),
    }
}

/// Wrap Lua's dynamic loader and file searcher. Native embedded apps expose
/// file I/O through kui.fs, so only their module searcher needs the file loader.
pub fn install(lua: &Lua, loadfile: Function, expose_files: bool) -> mlua::Result<()> {
    lua.load(LOADER_LUA)
        .set_name("@kui:loader")
        .call::<()>((loadfile, expose_files))
}

const LOADER_LUA: &str = r##"
local loadfile_, expose_files = ...
local load_ = load
-- An `env` given, even nil, is the chunk's `_ENV`; one left out is the
-- globals. So the arguments are passed on as many as they came.
function load(...)
  local chunk, name, _, env = ...
  if select("#", ...) >= 4 then return load_(chunk, name, "t", env) end
  return load_(chunk, name, "t")
end
if expose_files then
  function loadfile(path, _mode, env)
    return loadfile_(path, env)
  end
  function dofile(path)
    local f, err = loadfile_(path)
    if not f then error(err, 2) end
    return f()
  end
else
  loadfile, dofile = nil, nil
end
-- `require`'s searcher for Lua files, the same as Lua's own but through
-- the loader above.
package.searchers[2] = function(name)
  local path, err = package.searchpath(name, package.path)
  if not path then return "\n\t" .. err end
  local f, lerr = loadfile_(path)
  if not f then error(("error loading module '%s' from file '%s':\n\t%s"):format(name, path, lerr), 2) end
  return f, path
end
"##;
