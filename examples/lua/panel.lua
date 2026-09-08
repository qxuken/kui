-- A self-contained kui extension: owns its state, describes its UI as data,
-- and receives its own click events. Loaded by the lua_panel example.
--
-- It fills the slot the host names "panel" (docs/adr/0014), reads the title
-- and the reply template the host passes with it from `slot.params`, and
-- answers a toggle by returning the host's template from on_event.
--
-- And, when one has been built, it hosts a plugin of its own: the same
-- panel again in C, loaded with env.add_extension and placed with `fill`,
-- which makes this frame three languages deep -- a Rust host, a Lua
-- extension, and a C extension the Lua one put there. See the amendment
-- to docs/adr/0014. Build the plugin with examples/c/build.ps1 (or
-- build.sh); without it this file is exactly what it was.

slots = { "panel" }

-- Where the build scripts leave it, dev before release. On Windows this is
-- the shape that names kui_ffi.dll as the module its imports come from, so
-- it goes into any host that ships that DLL; `panel-host.dll` beside it
-- names c_panel.exe and loads into that and nothing else. On the unixes a
-- plugin leaves its kui_* undefined and takes them from the executable that
-- loaded it, and this example's host does not export them -- so there the
-- message below is the expected outcome, not a fault.
-- examples/c/build.ps1's header comment is the long version.
plugin_paths = {
  "target/debug/panel.dll", "target/debug/panel.so",
  "target/release/panel.dll", "target/release/panel.so",
}
plugin_ns = nil -- the namespace it went in under, once it is in
plugin_why = nil -- or why it is not
plugin_tried = false
native_toggles = 0

-- Once, on the first frame: loading is a call in view() because view() is
-- where a script knows what it wants, and add_extension is idempotent so
-- calling it every frame would be honest too -- this only avoids retrying
-- a file that is not there on every frame of every second.
local function load_plugin(env)
  if plugin_tried then return plugin_ns ~= nil end
  plugin_tried = true
  for _, path in ipairs(plugin_paths) do
    local ok, err = env.add_extension("native", path)
    if ok then
      plugin_ns = "native"
      return true
    end
    plugin_why = err
  end
  return false
end

todos = { "ship the layout solver", "wire up wgpu", "write this panel" }
done = {}
filter = ""
filter_key = nil -- set by a "changed" event; read back in view(env)
title = "lua panel"
on_toggle = nil -- the host's reply template, kept from view for on_event

function view(env, slot)
  local params = slot and slot.params or {}
  title = params.title or "lua panel"
  on_toggle = params.on_toggle

  if filter_key then
    filter = env.edit_text(filter_key) or ""
  end

  local items = {}
  for i, todo in ipairs(todos) do
    if filter == "" or todo:find(filter, 1, true) then
      local checked = done[i] and "[x] " or "[ ] "
      local color = done[i] and 0x5c6174ff or 0xe8e8eaff
      items[#items + 1] = row {
        gap = 8,
        cross_align = "center",
        on_click = { kind = "toggle", index = i },
        tooltip = done[i] and "click to reopen" or "click to finish",
        pad = { t = 4, b = 4 },
        text(checked .. todo, { size = 14, color = color }),
      }
    end
  end

  local remaining = #todos
  for i in pairs(done) do
    if done[i] then remaining = remaining - 1 end
  end

  local mine = column {
    width = 300,
    height = "grow",
    pad = 16,
    gap = 10,
    bg = 0x14161eff,
    radius = 10,
    border = { w = 1, color = 0x2a2d3aff },
    text({ title, { " · " .. remaining .. " left", color = "#8a8fa3" } },
         { size = 12, color = 0x8a8fa3ff }),
    edit { key = "filter", initial = "", size = 14, width = "grow",
           pad = { l = 8, r = 8, t = 6, b = 6 }, bg = 0x0e1016ff, radius = 6 },
    column { height = "grow", scroll = true, table.unpack(items) },
    row {
      gap = 8,
      button { label = "add", on_click = { kind = "add" } },
      button { label = "clear done", on_click = { kind = "clear" } },
    },
  }

  -- The plugin's half. `fill` is a position among these children, filled
  -- then and there by whoever answers to "native/panel" -- the plugin this
  -- script loaded, under the namespace this script chose. The params are
  -- its title and the shape of the reply this script wants back, which is
  -- what the host does for this script one level up.
  local guest
  if load_plugin(env) then
    guest = fill {
      name = plugin_ns .. "/panel",
      params = {
        title = "the same panel, in C · " .. native_toggles .. " heard",
        on_toggle = { kind = "native_toggle" },
      },
    }
  else
    guest = column {
      width = 300,
      pad = 16,
      gap = 6,
      bg = 0x14161eff,
      radius = 10,
      border = { w = 1, color = 0x2a2d3aff },
      text("no native panel loaded", { size = 12, color = 0x8a8fa3ff }),
      text(plugin_why or "build examples/c first", { size = 11, color = 0x5c6174ff, wrap = true }),
    }
  end

  return row { width = "grow", height = "grow", gap = 12, mine, guest }
end

function on_event(ev)
  -- A reply from a plugin this script loaded carries `from`: the namespace
  -- it was loaded under. This script's own events have none, which is what
  -- tells the two apart. Nothing is returned, so the host never hears the
  -- C panel -- it is this script's guest, not the host's.
  if ev.from then
    native_toggles = native_toggles + 1
    return
  end
  if ev.kind == "toggle" then
    done[ev.index] = not done[ev.index]
    -- The host asked to hear about this: its template, with the index
    -- filled in, is the reply. Returned rather than sent - a reply is data.
    if on_toggle then
      local reply = { index = ev.index, done = done[ev.index] or false }
      for k, v in pairs(on_toggle) do reply[k] = v end
      return reply
    end
  elseif ev.kind == "changed" then
    filter_key = ev.node_key
  elseif ev.kind == "add" then
    todos[#todos + 1] = "todo #" .. (#todos + 1)
  elseif ev.kind == "clear" then
    local kept, kept_done = {}, {}
    for i, todo in ipairs(todos) do
      if not done[i] then kept[#kept + 1] = todo end
    end
    todos, done = kept, kept_done
  end
end
