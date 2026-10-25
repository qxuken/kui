-- A self-contained kui extension: owns its state, describes its UI as data,
-- and receives its own click events. Loaded by the lua_panel example.
--
-- It provides the component "panel" (docs/adr/0067), drawn where the host
-- places it: the title the host passes is in `props`, and a toggle raises
-- the host's `on_toggle` by returning `emit("on_toggle", fields)` from
-- on_event -- the host hears its own message, the fields under `toggle`.
--
-- And, when one has been built, it hosts a plugin of its own: the same
-- panel again in C, loaded with env.add_extension and placed with
-- `component`, which makes this frame three languages deep -- a Rust host, a
-- Lua extension, and a C extension the Lua one put there. Build the plugin
-- with `cargo run -p kui-devtools --bin cbuild`; without it this file is
-- exactly what it was.

-- Where `cbuild` leaves it, dev before release. On Windows this is the
-- shape that names kui_ffi.dll as the module its imports come from, so it
-- goes into any host that ships that DLL; `panel-host.dll` beside it names
-- c_panel.exe and loads into that and nothing else. On the unixes a plugin
-- leaves its kui_* undefined and takes them from the executable that
-- loaded it, which this example's host exports for it (kui-lua/build.rs,
-- the same flag kui-ffi's examples get) -- so the message below means the
-- plugin has not been built, and nothing else.
-- examples/devtools/src/bin/cbuild.rs's header comment is the long version.
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
    -- Keep the first reason from a file that was there over a later
    -- "no such file": the debug .so's undefined kui_* is the message
    -- worth reading, not that the release one was never built.
    if plugin_why == nil or plugin_why:find("no such file", 1, true) then
      plugin_why = err
    end
  end
  return false
end

todos = { "ship the layout solver", "wire up wgpu", "write this panel" }
done = {}
filter = ""

local function panel(env, props)
  -- The palette the host derived from the OS (ADR 0019): every key is the
  -- 0xRRGGBBAA a colour prop takes, so a role goes straight into `bg`.
  -- Read, not set: a script is a guest in someone else's frame.
  local t = env.theme
  local title = props.title or "lua panel"

  local items = {}
  for i, todo in ipairs(todos) do
    if filter == "" or todo:find(filter, 1, true) then
      local checked = done[i] and "[x] " or "[ ] "
      local color = done[i] and t.faint or t.fg
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
    bg = t.surface,
    radius = 10,
    border = { w = 1, color = t.border },
    text({ title, { " · " .. remaining .. " left", color = t.muted } },
         { size = 12, color = t.muted }),
    -- `label` because nothing else names it: an edit with no text inside it
    -- and no label is what `control-without-name` is for. Its text is the
    -- script's `filter`, declared as `value` every frame and kept up to
    -- date from the `change` its on_change message arrives with.
    edit { key = "filter", label = "filter todos", value = filter, size = 14,
           width = "grow", pad = { l = 8, r = 8, t = 6, b = 6 },
           bg = t.sunken, radius = 6, on_change = { kind = "filter" } },
    column { height = "grow", scroll = true, table.unpack(items) },
    row {
      gap = 8,
      button { label = "add", on_click = { kind = "add" } },
      button { label = "clear done", on_click = { kind = "clear" } },
    },
  }

  -- The plugin's half. `component` is a position among these children,
  -- drawn then and there by whoever answers to "native/panel" -- the plugin
  -- this script loaded, under the namespace this script chose. The props
  -- are its title and the message this script wants back on a toggle,
  -- which is what the host does for this script one level up.
  local guest
  if load_plugin(env) then
    guest = component(plugin_ns .. "/panel", {
      title = "the same panel, in C · " .. native_toggles .. " heard",
      on_toggle = { kind = "native_toggle" },
    })
  else
    guest = column {
      width = 300,
      pad = 16,
      gap = 6,
      bg = t.surface,
      radius = 10,
      border = { w = 1, color = t.border },
      text("no native panel loaded", { size = 12, color = t.muted }),
      -- `wrap` is the text one: "word" / "glyph" / "none". The boolean is
      -- `wrap_children`, on a row, and passing it here is a runtime error
      -- that takes the whole view with it.
      text(plugin_why or "build examples/c first", { size = 11, color = t.faint, wrap = "word" }),
    }
  end

  return row { width = "grow", height = "grow", gap = 12, mine, guest }
end

-- What this script provides: the panel, a function of the env, the props
-- the host placed it with and the instance (`inst.key`, `inst.namespace`).
-- With no `components` the script's `view` would be its root, drawn after
-- the host's view.
components = {
  panel = function(env, props, inst) return panel(env, props or {}) end,
}

function on_event(ev, at)
  -- The C panel's on_toggle, raised by the plugin this script placed: this
  -- script's own message, `at.event` "component". Nothing is emitted, so
  -- the host never hears the C panel -- it is this script's guest, not the
  -- host's.
  if ev.kind == "native_toggle" and at.event == "component" then
    native_toggles = native_toggles + 1
    return
  end
  if ev.kind == "toggle" then
    done[ev.index] = not done[ev.index]
    -- Raise the host's on_toggle: the host hears the message it passed,
    -- these fields under `toggle` -- or nothing, when it passed none.
    -- Returned rather than sent: an emit is data.
    return emit("on_toggle", { index = ev.index, done = done[ev.index] or false })
  elseif ev.kind == "filter" then
    -- The field's whole text after the edit, on the event: the next
    -- view filters by it and declares it back as the field's value.
    filter = ev.change.text
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
