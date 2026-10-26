-- Kit's build: type-checks every module in src/ against kui-lua's DSL
-- (packages/lua/kui.d.tl) and writes the plain Lua a host loads into lua/.
-- Uses the Teal compiler kui-lua already vendors for its own tests, so the
-- only thing it needs is a Lua 5.4+ on PATH.
--
-- A `{{wgsl:name}}` in a module is replaced by shaders/name.wgsl's text.
--
-- Run from the repo root: lua addons/kit/build.lua
-- `--check` type-checks and fails on a stale lua/ without writing it.

local here = (arg[0]:match("^(.*)/[^/]*$") or ".") .. "/"
local repo = here .. "../../"
package.path = repo .. "crates/kui-lua/tests/tl/?.lua;" .. package.path
local tl = require("tl")
tl.path = repo .. "packages/lua/?.lua;" .. here .. "src/?.lua"

local check_only = arg[1] == "--check"

local function read(path)
  local f = io.open(path, "rb")
  if not f then return nil end
  local s = f:read("a")
  f:close()
  return s
end

local function modules()
  local list = {}
  local p = io.popen('ls "' .. here .. 'src"')
  for name in p:lines() do
    local m = name:match("^(.+)%.tl$")
    if m then list[#list + 1] = m end
  end
  p:close()
  table.sort(list)
  return list
end

local failed, stale = false, {}
for _, m in ipairs(modules()) do
  local env = assert(tl.new_env({
    defaults = { gen_target = "5.4", gen_compat = "off" },
    predefined_modules = { "kui" },
  }))
  local src_path = here .. "src/" .. m .. ".tl"
  -- `{{wgsl:name}}` is shaders/name.wgsl's text, so Kit carries its own
  -- shaders into every host that loads it and the .wgsl stays the source.
  local text = read(src_path):gsub("{{wgsl:([%w_]+)}}", function(n)
    return assert(read(here .. "shaders/" .. n .. ".wgsl"), "no shaders/" .. n .. ".wgsl")
  end)
  local res = tl.check_string(text, env, src_path)
  local errs = {}
  for _, list in ipairs({ res.syntax_errors or {}, res.type_errors or {} }) do
    for _, e in ipairs(list) do errs[#errs + 1] = e end
  end
  if #errs > 0 then
    failed = true
    for _, e in ipairs(errs) do
      io.stderr:write(string.format("%s:%d:%d: %s\n", src_path, e.y, e.x, e.msg))
    end
  else
    local code = assert(tl.generate(res.ast, "5.4"))
    code = "-- Generated from src/" .. m .. ".tl by addons/kit/build.lua. Edit the .tl.\n" .. code
    local out = here .. "lua/" .. m .. ".lua"
    if read(out) ~= code then
      if check_only then
        stale[#stale + 1] = out
      else
        local f = assert(io.open(out, "wb"))
        f:write(code)
        f:close()
        print("wrote " .. out)
      end
    end
  end
end

if #stale > 0 then
  for _, s in ipairs(stale) do io.stderr:write("stale: " .. s .. "\n") end
  failed = true
end
os.exit(failed and 1 or 0)
