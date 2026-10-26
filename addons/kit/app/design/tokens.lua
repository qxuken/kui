-- design/tokens.json's values, from Kit itself: every colour role of the
-- six themes (three skins, light and dark, default settings) and the
-- lengths that are one per skin, read live through `env.tokens` as Kit
-- declares them, written into the file beside its usage notes. The notes
-- are the file's own; the values are Kit's, so the web rendition of the
-- design system regenerates from what Kit draws.
--
--   kui run --headless addons/kit/app/design/tokens.lua -- --write
--   kui run --headless addons/kit/app/design/tokens.lua -- --check
--
-- `--check` writes nothing and fails, naming each difference, when the
-- file is not what `--write` would make it: a colour role Kit has that the
-- file lacks or the other way round, or a value Kit computes otherwise.

local kit = require("kit.ui")

local mode = kui.args[1] or "--check"
local PATH = "tokens.json"

local THEMES = {
  { id = "ledger-light", skin = "ledger", appearance = "light" },
  { id = "ledger-dark", skin = "ledger", appearance = "dark" },
  { id = "neutral-light", skin = "neutral", appearance = "light" },
  { id = "neutral-dark", skin = "neutral", appearance = "dark" },
  { id = "phosphor-light", skin = "phosphor", appearance = "light" },
  { id = "phosphor-dark", skin = "phosphor", appearance = "dark" },
}

-- `kitSelFg` ↔ `sel-fg`, `kitGroup1` ↔ `group-1`.
local function role_of(token)
  local s = token:gsub("^kit", "")
  return (s:gsub("(%l)(%u)", "%1-%2"):gsub("(%a)(%d)", "%1-%2"):lower())
end

local function hex(c)
  if (c & 255) == 255 then return string.format("#%06x", c >> 8) end
  return string.format("#%08x", c)
end

local function px(v) return v == 0 and "0" or string.format("%gpx", v) end

-- What Kit declared, theme by theme: one frame declares a theme's
-- settings, the next reads what Kit made of them.
local seen = {}
local at = 0

function init() return {} end
function update() end

function view(env)
  if at > 0 and at <= #THEMES then
    local colors, lengths = {}, {}
    for name, v in pairs(env.tokens and env.tokens.colors or {}) do
      if name:match("^kit") then colors[role_of(name)] = v end
    end
    for name, v in pairs(env.tokens and env.tokens.lengths or {}) do lengths[name] = v end
    seen[THEMES[at].id] = { colors = colors, lengths = lengths }
  end
  at = at + 1
  local t = THEMES[at]
  if t then return column { kit.settings({ skin = t.skin, appearance = t.appearance }) } end
  return column {}
end

local function finish()
  local text = kui.fs.read(PATH)
  local doc = kui.json.decode(text)
  local problems = {}
  -- The colour roles: the file's in its order, Kit's new ones after.
  local roles, named = {}, {}
  for _, t in ipairs(doc.color.tokens) do named[t.name] = t end
  local live = seen[THEMES[1].id].colors
  for _, t in ipairs(doc.color.tokens) do
    if live[t.name] == nil then
      problems[#problems + 1] = "the file names " .. t.name .. ", which Kit does not export"
    else
      roles[#roles + 1] = t
    end
  end
  local extra = {}
  for name in pairs(live) do if not named[name] then extra[#extra + 1] = name end end
  table.sort(extra)
  for _, name in ipairs(extra) do
    problems[#problems + 1] = "Kit exports " .. name .. ", which the file does not name (give it a usage note)"
    roles[#roles + 1] = { name = name, usage = "TODO: what " .. name .. " is for.", value = {} }
  end
  for _, t in ipairs(roles) do
    local value = {}
    for _, th in ipairs(THEMES) do
      local v = seen[th.id].colors[t.name]
      value[th.id] = v and hex(v) or nil
      if t.value and t.value[th.id] ~= value[th.id] then
        problems[#problems + 1] = string.format("%s in %s: the file says %s, Kit draws %s", t.name, th.id,
          tostring(t.value[th.id]), tostring(value[th.id]))
      end
    end
    t.value = value
  end
  doc.color.tokens = roles
  -- The lengths one per skin: the sizes and the radius, and the row and
  -- control heights at the default density.
  local function length(group, name, token, skin)
    for _, t in ipairs(doc[group] and doc[group].tokens or {}) do
      if t.name == name then
        local v = seen[skin .. "-light"].lengths[token]
        local want = v and px(v) or nil
        if t.value ~= want then
          problems[#problems + 1] = string.format("%s: the file says %s, Kit sets %s", name, tostring(t.value), tostring(want))
          t.value = want
        end
      end
    end
  end
  for _, skin in ipairs({ "ledger", "neutral", "phosphor" }) do
    length("fontSize", "kit-size-" .. skin, "kitSize", skin)
    length("fontSize", "kit-size-label-" .. skin, "kitSizeLabel", skin)
    length("fontSize", "kit-size-title-" .. skin, "kitSizeTitle", skin)
    length("radius", "kit-radius-" .. skin, "kitRadius", skin)
  end
  length("spacing", "kit-row", "kitRow", "ledger")
  length("spacing", "kit-ctl", "kitCtl", "ledger")
  local out = kui.json.encode(doc, { pretty = true }) .. "\n"
  if mode == "--write" then
    if out ~= text then kui.fs.write(PATH, out) end
    print("wrote " .. PATH .. " (" .. #problems .. " changes)")
    kui.exit(0)
  else
    for _, p in ipairs(problems) do print("tokens.json: " .. p) end
    if out ~= text and #problems == 0 then print("tokens.json: not in the form --write writes it") end
    print(out == text and "tokens.json matches Kit" or "tokens.json is stale: run addons/kit/app/design/tokens.lua -- --write")
    kui.exit(out == text and 0 or 1)
  end
end

kui.timer.every(40, function()
  if at > #THEMES then finish() else kui.dispatch({ kind = "tick" }) end
end)
