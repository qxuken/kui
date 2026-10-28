-- The inspector (ADR 0075, decisions 7–10 and 13): one recording, drawn
-- from the channels `kui.trace` keeps in Rust — the frames as an
-- overview, the spans and marks as Kit's waterfall with counters and
-- gauges as graph lanes under it on the same window, the frame or span
-- selected, and the tables over the window — and what the keys, the
-- filter and the window do to it. The shell (main.tl) owns the file;
-- this owns everything about one recording, so a devtools tab can mount
-- it over the live recording later.

local kit = require("kit.ui")
local state = require("state")

local type Inspector = state.Inspector
local type Msg = state.Msg

local record inspector
  new: function(rec: kui.Recording, prefs: state.Prefs): Inspector
  update: function(st: Inspector, msg: Msg): boolean
  key: function(st: Inspector, key: {string:any}): boolean
  view: function(st: Inspector, env: kui.Env): kui.Node
  filtering: function(st: Inspector): boolean
  grew: function(st: Inspector)
  restat: function(st: Inspector)
end

local LANE_H = 52
-- The y labels' column of the graph lanes, and the empty one the
-- waterfall keeps beside its lanes, so both plots start at one x.
local GUTTER = 44
-- The waterfall's own padding, which the lanes under it match.
local PAD = 10
-- The least the timeline and the dock under it are, in px.
local DOCK_MIN = 140
local TABS = {
  { value = "frames", label = "Frames" },
  { value = "spans", label = "Spans" },
  { value = "origins", label = "Origins" },
  { value = "components", label = "Components" },
  { value = "counters", label = "Counters" },
  { value = "burns", label = "Burns" },
  { value = "gaps", label = "Gaps" },
}

local function ms(v: number): string
  if not v then return "—" end
  if v >= 1000 then return ("%.2f s"):format(v / 1000) end
  if v >= 10 then return ("%.1f ms"):format(v) end
  return ("%.2f ms"):format(v)
end

local function muted(s: string): kui.Node
  return text(s, { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", max_lines = 1, ellipsis = true } as kui.text)
end

local function extent(st: Inspector): number, number
  local s = st.session
  return (s.start or 0) as number, (s.finish or 1) as number
end

-- The first row of `ch` whose `field` is at least `t` (the column sorted).
local function first_at(ch: kui.Channel, field: string, t: number): integer
  local lo, hi = 0, #ch
  while lo < hi do
    local mid = (lo + hi) // 2
    if (ch:get(mid, field) as number) < t then lo = mid + 1 else hi = mid end
  end
  return lo
end

-- The timeline's frames the window holds, first and last; nil for none.
local function frames_in(st: Inspector): integer, integer
  local f = st.rec.frames
  if #f == 0 then return nil, nil end
  local a = first_at(f, "begin", st.v0)
  local b = first_at(f, "begin", st.v1) - 1
  if a > #f - 1 then a = #f - 1 end
  if b < a then b = a end
  return f:get(a, "index") as integer, f:get(b, "index") as integer
end

local function set_window(st: Inspector, v0: number, v1: number)
  local lo, hi = extent(st)
  local len = math.max(v1 - v0, 0.01)
  len = math.min(len, hi - lo)
  if v0 < lo then v0 = lo end
  if v0 + len > hi then v0 = hi - len end
  st.v0, st.v1 = v0, v0 + len
  st.stale = true
end

local function apply_filter(st: Inspector)
  local shown: {string} = nil
  local hidden = 0
  for _, off in pairs(st.origins) do if off then hidden = hidden + 1 end end
  if hidden > 0 then
    shown = {}
    for _, o in ipairs((st.stats and st.stats.origins or {}) as {{string:any}}) do
      local name = o.origin as string
      if not st.origins[name] then shown[#shown + 1] = name end
    end
  end
  st.rec:filter({
    origins = shown,
    name = st.name ~= "" and st.name or nil,
    hide_names = not st.devtools and { "devtools" } or nil,
    min_us = st.min_us > 0 and st.min_us or nil,
    verdicts = st.frames == "slow" and { "slow", "late", "blocked", "waited" } or nil,
    hide_unchanged = st.frames == "changed" or nil,
  })
  st.frame, st.span = -1, -1
  st.stale = true
end

-- The tables over the window: the readings `rec:stats` takes in Rust,
-- made into channels for Kit's tables.
local function tables_of(s: {string:any}): {string:kui.Channel}
  local function list(k: string): {{string:any}} return (s[k] or {}) as {{string:any}} end
  local frames = math.max(1, ((s.summary as {string:any}).frames or 1) as number)
  local t: {string:kui.Channel} = {}
  do
    local name, origin, calls, per, total, self, worst: {any}, {any}, {any}, {any}, {any}, {any}, {any} = {}, {}, {}, {}, {}, {}, {}
    for i, r in ipairs(list("spans")) do
      name[i], origin[i], calls[i] = r.name, r.origin, r.calls
      per[i], total[i], self[i], worst[i] = (r.total as number) / frames, r.total, r.self, r.worst
    end
    t.spans = channel({ name = name, origin = origin, calls = calls, per_frame = per, total = total, self = self, worst = worst })
  end
  do
    local origin, self, spans: {any}, {any}, {any} = {}, {}, {}
    for i, r in ipairs(list("origins")) do origin[i], self[i], spans[i] = r.origin, r.self, r.spans end
    t.origins = channel({ origin = origin, self = self, spans = spans })
  end
  do
    local comp, placed, replayed, why: {any}, {any}, {any}, {any} = {}, {}, {}, {}
    for i, r in ipairs(list("components")) do
      comp[i], placed[i] = r.component, r.placed
      replayed[i] = ("%d%%"):format(math.floor((r.replayed as number) * 100 / math.max(1, r.placed as number) + 0.5))
      local f = ((r.fresh or {}) as {{string:any}})[1]
      why[i] = f and (("%s %s ×%d"):format(f.answer as string, f.why as string, f.count as integer)) or "—"
    end
    t.components = channel({ component = comp, placed = placed, replayed = replayed, why = why })
  end
  do
    local name, kind, a, b, c: {any}, {any}, {any}, {any}, {any} = {}, {}, {}, {}, {}
    for _, r in ipairs(list("counters")) do
      local i = #name + 1
      name[i], kind[i], a[i], b[i], c[i] = r.name, "counter", ("avg %.2f"):format(r.avg as number), ("most %d"):format(r.max as integer), ""
    end
    for _, r in ipairs(list("gauges")) do
      local i = #name + 1
      name[i], kind[i], a[i], b[i], c[i] = r.name, "gauge", ("last %.0f"):format(r.last as number),
        ("least %.0f"):format(r.least as number), ("most %.0f"):format(r.most as number)
    end
    t.counters = channel({ name = name, kind = kind, a = a, b = b, c = c })
  end
  do
    local first, last, n: {any}, {any}, {any} = {}, {}, {}
    for i, r in ipairs(list("burns")) do first[i], last[i], n[i] = r.first, r.last, r.frames end
    t.burns = channel({ first = first, last = last, frames = n })
  end
  return t
end

-- Whether the filter hides what the user asked it to (the devtools' own
-- spans, hidden by default, are not that).
function inspector.filtering(st: Inspector): boolean
  for _, off in pairs(st.origins) do if off then return true end end
  return st.name ~= "" or st.min_us > 0 or st.frames ~= "all"
end

function inspector.restat(st: Inspector)
  local a, b = frames_in(st)
  local key = ("%s-%s-%s"):format(tostring(a), tostring(b), tostring(#st.rec.frames))
  if key == st.stats_for and st.stats then st.stale = false return end
  st.stats = st.rec:stats(a, b)
  st.stats_for = key
  st.tables = tables_of(st.stats)
  st.stale = false
end

function inspector.new(rec: kui.Recording, prefs: state.Prefs): Inspector
  local s = rec:session()
  local st: Inspector = {
    rec = rec, session = s, follow = false, frame = -1, span = -1,
    tab = "frames", pane = "detail", split = prefs.split or 0.42, dock = prefs.dock or 0.58,
    lanes = {}, series = {}, origins = {}, name = prefs.name or "", min_us = prefs.min_us or 0,
    frames = prefs.frames or "all", devtools = prefs.devtools == true, search_focus = 0,
  }
  local lo, hi = (s.start or 0) as number, (s.finish or 1) as number
  st.v0, st.v1 = lo, hi
  local app = (s.app or "") as string
  local wanted = prefs.lanes and prefs.lanes[app]
  local names = rec:series_names()
  if wanted then
    for _, n in ipairs(wanted) do
      for _, have in ipairs(names) do if have == n then st.lanes[#st.lanes + 1] = n end end
    end
  else
    for i, n in ipairs(names) do if i <= 2 then st.lanes[#st.lanes + 1] = n end end
  end
  -- No origin is hidden yet, so the filter needs no reading first; the
  -- tables are read after the first frame (`view`).
  apply_filter(st)
  return st
end

-- More came (Live): the window rides the end while it is followed.
function inspector.grew(st: Inspector)
  st.session = st.rec:session()
  st.series = {}
  if st.follow then
    local lo, hi = extent(st)
    local len = st.v1 - st.v0
    st.v0, st.v1 = math.max(lo, hi - len), hi
  end
  st.stale = true
end

-- The next (or previous) row of `rec.frames` from `from` whose column
-- `field` is `want`, the first of a run when `run`.
local function seek(st: Inspector, from: integer, step: integer, field: string, want: any, run: boolean): integer
  local f = st.rec.frames
  local i = from + step
  while i >= 0 and i < #f do
    if f:get(i, field) == want and (not run or i == 0 or f:get(i - 1, field) ~= want) then return i end
    i = i + step
  end
  return nil
end

-- Selects frame row `row`, the window moved to show it when it is out.
local function select_frame(st: Inspector, row: integer)
  local f = st.rec.frames
  if row < 0 or row >= #f then return end
  st.frame, st.span = row, -1
  local b, e = f:get(row, "begin") as number, f:get(row, "end") as number
  if b < st.v0 or e > st.v1 then
    local len = st.v1 - st.v0
    set_window(st, b - len / 2, b + len / 2)
    st.follow = false
  end
end

local function fit(st: Inspector)
  if st.r0 then
    set_window(st, st.r0, st.r1)
    st.r0, st.r1 = nil, nil
  elseif st.span >= 0 then
    local s = st.rec:span(st.span)
    if s then set_window(st, s.start - s.len * 0.1, s.start + s.len * 1.1) end
  elseif st.frame >= 0 then
    local f = st.rec.frames
    local b, e = f:get(st.frame, "begin") as number, f:get(st.frame, "end") as number
    local d = st.rec:frame(st.frame)
    local from = b - (d and d.gap or 0)
    set_window(st, from, e + (e - from) * 0.1)
  end
  st.follow = false
end

function inspector.key(st: Inspector, key: {string:any}): boolean
  if key.phase ~= "down" or key.ctrl or key.super or key.alt then return false end
  local c = key.code as string
  local len = st.v1 - st.v0
  local mid = (st.v0 + st.v1) / 2
  if c == "right" or c == "l" then select_frame(st, math.max(st.frame, -1) + 1)
  elseif c == "left" or c == "h" then select_frame(st, math.max(st.frame - 1, 0))
  elseif c == "down" or c == "j" then select_frame(st, math.max(st.frame, -1) + 1)
  elseif c == "up" or c == "k" then select_frame(st, math.max(st.frame - 1, 0))
  elseif c == "]" or c == "[" then
    local i = seek(st, st.frame, c == "]" and 1 or -1, "verdict", "slow", false)
    if i then select_frame(st, i) end
  elseif c == "}" or c == "{" then
    local i = seek(st, st.frame, c == "}" and 1 or -1, "burn", true, true)
    if i then select_frame(st, i) end
  elseif c == "w" then set_window(st, mid - len / 4, mid + len / 4); st.follow = false
  elseif c == "s" then set_window(st, mid - len, mid + len); st.follow = false
  elseif c == "a" then set_window(st, st.v0 - len / 5, st.v1 - len / 5); st.follow = false
  elseif c == "d" then set_window(st, st.v0 + len / 5, st.v1 + len / 5)
  elseif c == "f" or c == "z" then fit(st)
  elseif c == "0" then local lo, hi = extent(st); set_window(st, lo, hi)
  elseif c == "/" then st.search_focus = st.search_focus + 1
  elseif c == "escape" then
    if st.r0 then st.r0, st.r1 = nil, nil else st.frame, st.span = -1, -1 end
  else return false end
  return true
end

function inspector.update(st: Inspector, msg: Msg): boolean
  local k = msg.kind
  if k == "ins_restat" then
    st.restat_due = false
    if st.stale then inspector.restat(st) end
  elseif k == "ins_zoom" then
    -- Zooming to the range spends it.
    local z = msg.zoom
    if st.r0 and math.abs(z.start - st.r0) < 1e-9 and math.abs(z.start + z.length - st.r1) < 1e-9 then
      st.r0, st.r1 = nil, nil
    end
    set_window(st, z.start, z.start + z.length)
    st.follow = false
  elseif k == "ins_range" then
    local r = msg.range
    if r.length > 0 then st.r0, st.r1 = r.start, r.start + r.length else st.r0, st.r1 = nil, nil end
  elseif k == "ins_span" then
    st.span = msg.select.row
  elseif k == "ins_frame" then
    select_frame(st, msg.select.row)
  elseif k == "ins_burn" then
    local first = st.tables.burns:get(msg.select.row, "first") as integer
    -- A burn names the timeline's frame; the frames view's row for it.
    local f = st.rec.frames
    local row = first_at(f, "index", first)
    select_frame(st, row)
  elseif k == "ins_tab" then st.tab = msg.change.value as string
  elseif k == "ins_pane" then st.pane = msg.change.value as string
  elseif k == "ins_split" then st.split = msg.resize.fraction
  elseif k == "ins_dock" then st.dock = msg.resize.fraction
  elseif k == "ins_name" then st.name = msg.change.text as string; apply_filter(st)
  elseif k == "ins_frames" then st.frames = msg.change.value as string; apply_filter(st)
  elseif k == "ins_min" then st.min_us = tonumber(msg.change.value as string) or 0; apply_filter(st)
  elseif k == "ins_origin" then
    st.origins[msg.name] = not st.origins[msg.name]
    apply_filter(st)
  elseif k == "ins_devtools" then st.devtools = not st.devtools; apply_filter(st)
  elseif k == "ins_lane" then
    local found = false
    for i, n in ipairs(st.lanes) do
      if n == msg.name then table.remove(st.lanes, i); found = true break end
    end
    if not found then st.lanes[#st.lanes + 1] = msg.name end
  elseif k == "ins_fit" then local lo, hi = extent(st); set_window(st, lo, hi)
  elseif k == "ins_latest" then
    st.follow = true
    local lo, hi = extent(st)
    local len = st.v1 - st.v0
    set_window(st, math.max(lo, hi - len), hi)
  elseif k == "ins_overview" then
    -- A press or a drag on the overview moves the window's middle there.
    local r = msg.value as {string:number}
    if r and r.w and r.w > 0 then
      local lo, hi = extent(st)
      local at = lo + (hi - lo) * math.max(0, math.min(1, ((msg.drag.x or 0) - r.x) / r.w))
      local len = st.v1 - st.v0
      set_window(st, at - len / 2, at + len / 2)
      st.follow = false
    end
  else
    return false
  end
  return true
end

-- ---------- the parts ----------

local function overview(st: Inspector, env: kui.Env): kui.Node
  local r = env.layout_of("overview")
  local w = r and r.w or 600
  local lo, hi = extent(st)
  local x0 = (st.v0 - lo) / math.max(hi - lo, 1e-9) * w
  local x1 = (st.v1 - lo) / math.max(hi - lo, 1e-9) * w
  return column { key = "overview", width = "grow", height = 40, clip = true,
    on_layout = { kind = "ins_layout" },
    on_drag = { kind = "ins_overview", value = r and { x = r.x, w = r.w } or nil },
    label = "Overview: the work of every frame; drag to move the window",
    #st.rec.frames > 0 and kit.sparkline({ key = "ov-line", data = st.rec.frames, field = "work",
      width = w, height = 40, unit = "ms" }) or nil,
    column { key = "ov-window", float = { anchor = "parent", dx = x0, dy = 0 },
      width = math.max(2, x1 - x0), height = 40, bg = "$kitHover", border = { w = 1, color = "$kitAccent" } },
  }
end

local function lanes(st: Inspector): kui.Node
  -- The lanes scroll in at most two lanes' height, so the waterfall keeps
  -- its room whatever lanes are shown.
  -- Padded as the waterfall is, and their y labels as wide as its gutter,
  -- so a lane's plot runs under its lanes at the same x.
  local rows: kui.Props = { key = "lanes", width = "grow", gap = 6, max_height = 2 * (LANE_H + 60), scroll_y = true,
    pad = { x = PAD } }
  for _, name in ipairs(st.lanes) do
    local ch = st.series[name]
    if not ch then
      ch = st.rec:series(name)
      st.series[name] = ch
    end
    if ch then
      rows[#rows + 1] = column { key = "lane-" .. name, width = "grow", gap = 2,
        row { width = "grow", cross_align = "center", main_align = "end",
          kit.icon_button({ key = "hide-" .. name, icon = "x", label = "Hide the " .. name .. " lane",
            on_press = { kind = "ins_lane", name = name } }) },
        kit.line_chart({ key = "chart-" .. name, data = ch, x = "at",
          series = { { field = "value", name = name } }, height = LANE_H, x_min = st.v0, x_max = st.v1,
          x_unit = "ms", gutter = GUTTER }),
      }
    end
  end
  return column(rows)
end

local function lane_picker(st: Inspector): kui.Node
  local shown: {string:boolean} = {}
  for _, n in ipairs(st.lanes) do shown[n] = true end
  local items: kui.Props = { key = "lane-picker", gap = 4, cross_align = "center", wrap_children = true,
    muted("Lanes") }
  for _, n in ipairs(st.rec:series_names()) do
    items[#items + 1] = kit.button({ key = "lane-" .. n, label = n, pressed = shown[n] == true,
      variant = "ghost", on_press = { kind = "ins_lane", name = n } })
  end
  return row(items)
end

local function entry(k: string, v: string, mono?: boolean): {string:any}
  return { key = k, value = v, mono = mono }
end

local function detail(st: Inspector): kui.Node
  if st.span >= 0 then
    local s = st.rec:span(st.span)
    if s then
      return column { key = "detail", width = "grow", gap = 8,
        text(s.name, { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" } as kui.text),
        kit.inspector({ key = "span-entries", key_width = 110, entries = {
          entry("origin", s.origin), entry("start", ms(s.start), true), entry("length", ms(s.len), true),
          entry("self", ms(s.self), true), entry("cpu", s.cpu and ms(s.cpu) or "—", true),
          entry("track", s.track), entry("depth", tostring(s.depth), true),
          entry("node", s.node ~= "" and s.node or "—", true), entry("cut", s.cut and "yes" or "no"),
        } as {kit.Entry} }),
      }
    end
  end
  if st.frame < 0 then
    return kit.empty({ key = "no-frame", title = "Nothing selected",
      detail = "Pick a frame in the table (← → or h l walk them; [ ] the slow ones, { } the burns), or a span in the timeline." })
  end
  local f = st.rec:frame(st.frame)
  if not f then return kit.empty({ key = "gone", title = "That frame is filtered out" }) end
  local v = f.verdict
  local verdict = v.kind
  if v.kind == "slow" then verdict = ("slow — %s of work, most in %s%s"):format(ms(v.work), v.phase, v.span and (" (" .. v.span .. ")") or "")
  elseif v.kind == "late" then verdict = ("late — its input waited %s%s"):format(ms(v.wait), v.slept and ", the loop asleep" or "")
  elseif v.kind == "blocked" then verdict = ("blocked — %s busy outside kui"):format(ms(v.busy))
  elseif v.kind == "waited" then verdict = ("waited — %s wall, %s on a CPU"):format(ms(v.wall), ms(v.cpu))
  elseif v.kind == "wasted" then verdict = "wasted — drew what the frame before drew"
  elseif v.kind == "burn" then verdict = ("burn — frame %d of %d with no input"):format(v.k, v.of)
  elseif v.kind == "fine" then verdict = ("fine — %d%% of the budget"):format(math.floor(v.share * 100 + 0.5)) end
  local entries: {kit.Entry} = {
    entry("verdict", verdict) as kit.Entry, entry("work", ms(f.work), true) as kit.Entry,
    entry("gap", ms(f.gap), true) as kit.Entry, entry("nodes", tostring(f.nodes), true) as kit.Entry,
    entry("why", #f.causes > 0 and table.concat(f.causes, ", ") or "nobody here asked") as kit.Entry,
  }
  for _, p in ipairs({ "handler", "build", "devtools", "layout", "emit", "render", "vsync wait" }) do
    local x = f.phases[p]
    if x and x > 0 then entries[#entries + 1] = entry(p, ms(x), true) as kit.Entry end
  end
  if f.path then
    local legs: {string} = {}
    for _, l in ipairs(f.path.legs) do legs[#legs + 1] = ("%s %s"):format(l.what, ms(l.ms)) end
    entries[#entries + 1] = entry("input to screen", ("%s %s: %s"):format(f.path.input, ms(f.path.total), table.concat(legs, " · "))) as kit.Entry
  end
  for _, h in ipairs(f.holders) do entries[#entries + 1] = entry("held by", h.what .. " " .. h.name) as kit.Entry end
  for _, p in ipairs(f.fresh) do
    entries[#entries + 1] = entry("drawn fresh", ("%s %s%s"):format(p.component, p.answer, p.why ~= "" and (" · " .. p.why) or "")) as kit.Entry
  end
  for _, n in ipairs(f.notes) do entries[#entries + 1] = entry("note", n) as kit.Entry end
  for name, value in pairs(f.counters) do entries[#entries + 1] = entry(name, tostring(value), true) as kit.Entry end
  return column { key = "detail", width = "grow", gap = 8,
    text(("Frame %d"):format(f.frame), { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" } as kui.text),
    kit.inspector({ key = "frame-entries", key_width = 120, entries = entries }),
  }
end

local function col(field: string, title: string, width?: number, mono?: boolean): kit.Column
  return { field = field, title = title, width = width, mono = mono, align = mono and "end" or nil } as kit.Column
end

local function table_for(st: Inspector, tab: string): kui.Node
  if tab == "frames" then
    return kit.table({ key = "t-frames", rows = st.rec.frames, selected = st.frame >= 0 and st.frame or nil,
      on_select = { kind = "ins_frame" }, grow = true, columns = {
        col("frame", "frame", 70, true), col("work", "work", 70, true), col("build", "build", 70, true),
        col("layout", "layout", 70, true), col("render", "render", 70, true), col("verdict", "verdict", 80),
        col("causes", "why it ran"),
      } })
  end
  local t = st.tables or {}
  if tab == "gaps" then
    local g = ((st.stats or {}).gaps or {}) as {string:number}
    return kit.inspector({ key = "t-gaps", key_width = 140, entries = {
      entry("between frames", ms(g.total), true), entry("in handlers", ms(g.handlers), true),
      entry("asleep", ms(g.asleep), true), entry("busy outside kui", ms(g.busy), true),
    } as {kit.Entry} })
  end
  local ch = t[tab]
  if not ch then return kit.empty({ key = "t-none", title = "Counting…" }) end
  local cols: {kit.Column}
  local on_select: any = nil
  if tab == "spans" then
    cols = { col("name", "span"), col("origin", "origin", 90), col("calls", "calls", 70, true),
      col("per_frame", "per frame", 80, true), col("worst", "worst", 70, true), col("self", "self", 80, true) }
  elseif tab == "origins" then
    cols = { col("origin", "origin"), col("self", "own time", 100, true), col("spans", "spans", 80, true) }
  elseif tab == "components" then
    cols = { col("component", "component"), col("placed", "placed", 80, true), col("replayed", "replayed", 80, true),
      col("why", "most often fresh for") }
  elseif tab == "counters" then
    cols = { col("name", "name"), col("kind", "kind", 80), col("a", "", 100, true), col("b", "", 100, true), col("c", "", 100, true) }
  elseif tab == "burns" then
    cols = { col("first", "first frame", 100, true), col("last", "last frame", 100, true), col("frames", "frames", 80, true) }
    on_select = { kind = "ins_burn" }
  end
  return kit.table({ key = "t-" .. tab, rows = ch, columns = cols, grow = true, on_select = on_select })
end

local function tables(st: Inspector, with_detail: boolean): kui.Node
  local tabs: {kit.Tab} = {}
  if with_detail then tabs[1] = { value = "detail", label = "Detail" } end
  for _, t in ipairs(TABS) do tabs[#tabs + 1] = t as kit.Tab end
  local value = with_detail and st.pane or st.tab
  local body = value == "detail" and detail(st) or table_for(st, value)
  return kit.tabs({ key = with_detail and "pane" or "tables", value = value, tabs = tabs,
    on_change = { kind = with_detail and "ins_pane" or "ins_tab" } }, {
    column { key = "tab-body", width = "grow", height = "grow", pad = { t = 8 }, body },
  })
end

local function filters(st: Inspector): kui.Node
  local origins: kui.Props = { key = "origins", gap = 2, cross_align = "center", muted("Origins") }
  for _, o in ipairs((st.stats and st.stats.origins or {}) as {{string:any}}) do
    local name = o.origin as string
    origins[#origins + 1] = kit.button({ key = "origin-" .. name, label = name, pressed = not st.origins[name],
      variant = "ghost", on_press = { kind = "ins_origin", name = name } })
  end
  origins[#origins + 1] = kit.button({ key = "origin-devtools", label = "devtools", pressed = st.devtools,
    variant = "ghost", on_press = { kind = "ins_devtools" } })
  return row { key = "filters", width = "grow", gap = 8, cross_align = "center", wrap_children = true,
    row { key = "name-box", width = 220,
      kit.search({ key = "name", value = st.name, placeholder = "Span name", label = "Filter spans by name",
        focus = st.search_focus, on_change = { kind = "ins_name" } }) },
    kit.segmented({ key = "min", label = "Shortest span shown", value = tostring(math.floor(st.min_us)),
      on_change = { kind = "ins_min" }, options = {
        { value = "0", label = "all" }, { value = "50", label = "≥ 50 µs" },
        { value = "500", label = "≥ 0.5 ms" }, { value = "5000", label = "≥ 5 ms" } } }),
    kit.segmented({ key = "frames-filter", label = "Frames shown", value = st.frames,
      on_change = { kind = "ins_frames" }, options = {
        { value = "all", label = "all frames" }, { value = "slow", label = "only slow" },
        { value = "changed", label = "hide unchanged" } } }),
    row(origins),
  }
end

local function timeline(st: Inspector): kui.Node
  local span_count = #st.rec.spans
  return column { key = "timeline", width = "grow", height = "grow", gap = 6,
    column { key = "wf", width = "grow", height = "grow", min_height = 160,
      kit.waterfall({ key = "waterfall", bars = st.rec.spans, marks = st.rec.instants,
        start = st.v0, length = st.v1 - st.v0, selected = st.span >= 0 and st.span or nil,
        range_start = st.r0, range_length = st.r0 and (st.r1 - st.r0) or nil, gutter = GUTTER,
        grow = true, on_select = { kind = "ins_span" }, on_zoom = { kind = "ins_zoom" },
        on_range = { kind = "ins_range" } }) },
    #st.lanes > 0 and lanes(st) or nil,
    span_count == 0 and muted("No span is kept by the filter.") or nil,
  }
end

function inspector.view(st: Inspector, env: kui.Env): kui.Node
  -- The tables over the window are read once it has been still a moment:
  -- a pan or a zoom draws its frames without reading them each step.
  if st.stale and not st.restat_due then
    st.restat_due = true
    kui.timer.after(150, function() kui.dispatch({ kind = "ins_restat" }) end)
  end
  -- The bands (decision 9) by the body's own width, a container query
  -- (ADR 0069): wide, the detail beside the tables in the dock; narrower,
  -- one pane of tabs with the detail first, and under 700 px no lane
  -- picker. Each band's pane is built and the other hidden, so a resize
  -- lays out again and runs no view.
  return column { key = "inspector", width = "grow", height = "grow", gap = 8, pad = { x = 10, t = 8 },
    when = { { width = { below = 700, band = 24 }, hidden = { "lane-picker" } } },
    filters(st),
    overview(st, env),
    lane_picker(st),
    column { key = "bands", width = "grow", height = "grow", gap = 8,
      -- The timeline over the dock, the rule between them dragged.
      kit.split({ key = "dock-split", axis = "column", fraction = st.dock, min = DOCK_MIN,
        on_resize = { kind = "ins_dock" } }, {
        first = { column { key = "top", width = "grow", height = "grow", timeline(st) } },
        -- The dock's own width picks its pane: a `when` reaches only the
        -- nodes of the scope it is declared in, and the split's are Kit's.
        second = { column { key = "dock", width = "grow", height = "grow",
          when = {
            { width = { above = 1180, band = 24 }, hidden = { "pane-box" } },
            { width = { below = 1180, band = 24 }, hidden = { "bottom" } },
          },
          column { key = "bottom", width = "grow", height = "grow",
            kit.split({ key = "bottom-split", fraction = st.split, min = 280, on_resize = { kind = "ins_split" } }, {
              first = { column { key = "detail-pane", width = "grow", height = "grow", scroll_y = true, detail(st) } },
              second = { column { key = "tables-pane", width = "grow", height = "grow", tables(st, false) } },
            }) },
          column { key = "pane-box", width = "grow", height = "grow", tables(st, true) },
        } },
      }),
    },
  }
end

return inspector
