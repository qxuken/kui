-- `kui.parse_args(spec)`: the app's command line (what followed `--`) read
-- against a spec, with `--help` written from it and a mistake said with
-- what to type instead. Part of the runner's runtime (ADR 0064).

local function dashed(name) return (name:gsub("_", "-")) end

local function type_of(o)
  if o.type then return o.type end
  local d = o.default
  if type(d) == "boolean" then return "boolean" end
  if math.type(d) == "integer" then return "integer" end
  if type(d) == "number" then return "number" end
  return "string"
end

-- The edit distance between two short words, for "did you mean".
local function distance(a, b)
  local prev = {}
  for j = 0, #b do prev[j] = j end
  for i = 1, #a do
    local cur = { [0] = i }
    for j = 1, #b do
      local cost = a:sub(i, i) == b:sub(j, j) and 0 or 1
      cur[j] = math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost)
    end
    prev = cur
  end
  return prev[#b]
end

local function sorted_names(options)
  local names = {}
  for k in pairs(options) do names[#names + 1] = k end
  table.sort(names)
  return names
end

local function help(spec, name)
  local options, args = spec.options or {}, spec.args or {}
  local out = {}
  local function add(s) out[#out + 1] = s end
  add(spec.about and (name .. " — " .. spec.about) or name)
  add("")
  local usage = { "usage: " .. name }
  if next(options) then usage[#usage + 1] = "[options]" end
  for _, a in ipairs(args) do
    local w = "<" .. a.name .. ">" .. (a.many and "..." or "")
    usage[#usage + 1] = a.required and w or ("[" .. w .. "]")
  end
  add(spec.usage or table.concat(usage, " "))
  local rows = {}
  for _, a in ipairs(args) do
    rows[#rows + 1] = { "  <" .. a.name .. ">" .. (a.many and "..." or ""), a.help or "", head = "arguments:" }
  end
  for _, key in ipairs(sorted_names(options)) do
    local o = options[key]
    local t = type_of(o)
    local flag = (o.short and ("-" .. o.short .. ", ") or "    ") .. "--" .. dashed(key)
    if t ~= "boolean" then flag = flag .. " <" .. (o.value or t) .. ">" end
    local text = o.help or ""
    local extra = {}
    if o.choices then extra[#extra + 1] = "one of " .. table.concat(o.choices, ", ") end
    if o.default ~= nil and t ~= "boolean" then extra[#extra + 1] = "default " .. tostring(o.default) end
    if o.env then extra[#extra + 1] = "env " .. o.env end
    if o.required then extra[#extra + 1] = "required" end
    if o.many then extra[#extra + 1] = "repeatable" end
    if #extra > 0 then text = text .. (text ~= "" and " " or "") .. "(" .. table.concat(extra, "; ") .. ")" end
    rows[#rows + 1] = { "  " .. flag, text, head = "options:" }
  end
  rows[#rows + 1] = { "  -h, --help", "show this help", head = "options:" }
  local width = 0
  for _, r in ipairs(rows) do width = math.max(width, #r[1]) end
  local head
  for _, r in ipairs(rows) do
    if r.head ~= head then
      add("")
      add(r.head)
      head = r.head
    end
    add(r[2] ~= "" and (r[1] .. string.rep(" ", width - #r[1] + 3) .. r[2]) or r[1])
  end
  if spec.epilog then
    add("")
    add(spec.epilog)
  end
  return table.concat(out, "\n")
end

-- Ends the app now: printed, the code set, and the script stopped by an
-- error the runner knows is no failure.
local function stop(code, text, to_err)
  if to_err then io.stderr:write(text, "\n") else io.stdout:write(text, "\n") end
  io.stdout:flush()
  kui.exit(code)
  error("kui: exit requested", 0)
end

local function convert(label, t, raw, o)
  local v
  if t == "integer" then
    v = math.tointeger(tonumber(raw))
    if not v then return nil, ("%s: expected a whole number, got %q"):format(label, raw) end
  elseif t == "number" then
    v = tonumber(raw)
    if not v then return nil, ("%s: expected a number, got %q"):format(label, raw) end
  elseif t == "boolean" then
    local l = raw:lower()
    if l == "true" or l == "1" or l == "yes" or l == "on" then v = true
    elseif l == "false" or l == "0" or l == "no" or l == "off" then v = false
    else return nil, ("%s: expected true or false, got %q"):format(label, raw) end
  else
    v = raw
  end
  if o and o.choices then
    local ok = false
    for _, c in ipairs(o.choices) do ok = ok or c == v end
    if not ok then return nil, ("%s: expected one of %s, got %q"):format(label, table.concat(o.choices, ", "), raw) end
  end
  return v
end

function kui.parse_args(spec, argv)
  spec = spec or {}
  argv = argv or kui.args
  local name = spec.name or kui.name or "app"
  local options = spec.options or {}
  local args = spec.args or {}
  local by_flag, by_short = {}, {}
  for key, o in pairs(options) do
    by_flag[dashed(key)] = key
    if o.short then by_short[o.short] = key end
  end
  local function fail(msg)
    stop(2, ("%s: %s\nrun with --help for the options"):format(name, msg), true)
  end
  local function unknown(flag)
    local best, best_d
    for f in pairs(by_flag) do
      local d = distance(flag, f)
      if d <= 2 and (not best_d or d < best_d) then best, best_d = f, d end
    end
    fail(("unknown option --%s%s"):format(flag, best and (" (did you mean --" .. best .. "?)") or ""))
  end

  local out, rest = {}, {}
  local function set(key, raw)
    local o = options[key]
    local t = type_of(o)
    local v, err = convert("--" .. dashed(key), t, raw, o)
    if err then fail(err) end
    if o.many then
      out[key] = out[key] or {}
      table.insert(out[key], v)
    else
      out[key] = v
    end
  end

  local i = 1
  local done = false
  while i <= #argv do
    local a = argv[i]
    if done or a == "-" or a:sub(1, 1) ~= "-" then
      rest[#rest + 1] = a
    elseif a == "--" then
      done = true
    elseif a == "-h" or a == "--help" then
      stop(0, help(spec, name))
    else
      local key, inline
      if a:sub(1, 2) == "--" then
        local flag, value = a:sub(3):match("^([^=]+)=(.*)$")
        flag = flag or a:sub(3)
        inline = value
        key = by_flag[flag]
        if not key and flag:sub(1, 3) == "no-" and by_flag[flag:sub(4)]
          and type_of(options[by_flag[flag:sub(4)]]) == "boolean" and inline == nil then
          out[by_flag[flag:sub(4)]] = false
          goto next
        end
        if not key then unknown(flag) end
      else
        local short, value = a:sub(2, 2), a:sub(3)
        key = by_short[short]
        if not key then fail(("unknown option -%s"):format(short)) end
        if value ~= "" then inline = value:gsub("^=", "") end
      end
      if type_of(options[key]) == "boolean" and inline == nil then
        out[key] = true
      else
        local raw = inline
        if raw == nil then
          i = i + 1
          raw = argv[i]
          if raw == nil then fail(("--%s needs a value"):format(dashed(key))) end
        end
        set(key, raw)
      end
    end
    ::next::
    i = i + 1
  end

  -- what the line left out: the environment, then the default
  for key, o in pairs(options) do
    if out[key] == nil and o.env and kui.env[o.env] and kui.env[o.env] ~= "" then
      set(key, kui.env[o.env])
    end
    if out[key] == nil then
      if o.required then fail(("--%s is required"):format(dashed(key))) end
      if o.many then
        out[key] = kui.json.array()
      elseif o.default == nil and type_of(o) == "boolean" then
        out[key] = false
      else
        out[key] = o.default
      end
    end
  end

  local at = 1
  for n, a in ipairs(args) do
    if a.many then
      local list = kui.json.array()
      while at <= #rest do
        local v, err = convert("<" .. a.name .. ">", a.type or "string", rest[at], a)
        if err then fail(err) end
        list[#list + 1] = v
        at = at + 1
      end
      if a.required and #list == 0 then fail(("<%s> is required"):format(a.name)) end
      out[a.name] = list
    elseif rest[at] ~= nil then
      local v, err = convert("<" .. a.name .. ">", a.type or "string", rest[at], a)
      if err then fail(err) end
      out[a.name] = v
      at = at + 1
    elseif a.required then
      fail(("<%s> is required"):format(a.name))
    else
      out[a.name] = a.default
    end
    if n == #args and at <= #rest and not spec.rest then
      fail(("unexpected argument %q"):format(rest[at]))
    end
  end
  if #args == 0 and #rest > 0 and not spec.rest then
    fail(("unexpected argument %q"):format(rest[1]))
  end
  if spec.rest then
    local extra = kui.json.array()
    for j = at, #rest do extra[#extra + 1] = rest[j] end
    out[spec.rest == true and "rest" or spec.rest] = extra
  end
  return out
end
