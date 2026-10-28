-- kui-perf end to end, without a window: the app opened on a recording,
-- then driven through `update` as a person drives it — keys, filters,
-- lanes, the window — and the recording grown under it with Live on and
-- off. The exit code is the number of failures.
--
--   kui run tools/kui-perf/test --headless -- RECORDING REST
--
-- RECORDING is the first part of a recording; REST the part Live brings,
-- appended to RECORDING by the test.

package.path = kui.dir .. "/../?.tl;" .. kui.dir .. "/../?.lua;" .. package.path
local path, rest = kui.args[1], kui.args[2]
kui.args = { path }
-- From no preferences, in a folder of the test's own: the app's is the
-- person's, and a run before this one saved its own.
kui.data_dir = (os.getenv("TMPDIR") or "/tmp"):gsub("/$", "") .. "/kui-perf-test-" .. tostring(os.time()) .. "-" .. tostring(math.random(1e9))
kui.fs.try_remove(kui.data_dir .. "/prefs.json")
require("main")
---@type any
model = init()

local failed = 0
local function check(ok, what)
  if ok then print("ok    " .. what) else failed = failed + 1; print("FAIL  " .. what) end
end
local function key(code, mods)
  local k = { phase = "down", code = code }
  for m in pairs(mods or {}) do k[m] = true end
  update({ kind = "keys", key = k })
end

local m = model
local st = m.ins
check(st ~= nil and m.error == nil, "the recording opened")
require("inspector").restat(st)
local frames = #st.rec.frames
check(frames > 100, "frames in the table: " .. frames)
check(#st.lanes == 2, "two graph lanes by default")
check(m.help == true, "the keys' card opens the first time")
key("l")
check(st.frame == -1, "a key goes to the card while it is open")
key("/", { shift = true })
check(not m.help, "? closes it")
key("/", { shift = true })
check(m.help, "and opens it again")
update({ kind = "help_close" })

-- The keys.
key("l")
check(st.frame == 0, "l selects the first frame")
key("right")
check(st.frame == 1, "→ the next")
key("h")
check(st.frame == 0, "h the one before")
key("]")
local v = st.frame >= 0 and st.rec.frames:get(st.frame, "verdict")
check(v == "slow", "] the next slow frame: " .. tostring(v))
key("}")
check(st.rec.frames:get(st.frame, "burn") == true, "} the next burn's first frame")
local f = st.rec:frame(st.frame)
check(f and f.verdict and f.phases, "its detail has a verdict and phases")
local v0, v1 = st.v0, st.v1
key("w")
check(st.v1 - st.v0 < (v1 - v0) * 0.6, "w zooms in")
key("s")
key("s")
key("a")
key("0")
local lo, hi = st.session.start, st.session.finish
check(st.v0 == lo and st.v1 == hi, "0 the whole recording")
key("f")
check(st.v1 - st.v0 < hi - lo, "f fits the frame")
-- The range on the axis.
local w0, w1 = st.v0, st.v1
local r0 = w0 + (w1 - w0) * 0.25
update({ kind = "ins_range", range = { start = r0, length = (w1 - w0) * 0.25 } })
check(st.r0 == r0, "a drag on the axis selects a range")
key("escape")
check(st.r0 == nil and st.frame >= 0, "Esc clears the range first")
update({ kind = "ins_range", range = { start = r0, length = (w1 - w0) * 0.25 } })
key("z")
check(st.v0 == r0 and math.abs(st.v1 - (w0 + (w1 - w0) * 0.5)) < 1e-9 and st.r0 == nil, "z zooms to the range and spends it")
update({ kind = "ins_range", range = { start = r0, length = 1 } })
update({ kind = "ins_zoom", zoom = { start = r0, length = 1 } })
check(st.r0 == nil, "the waterfall's zoom to the range spends it")
update({ kind = "ins_range", range = { start = r0, length = 1 } })
update({ kind = "ins_range", range = { start = 0, length = 0 } })
check(st.r0 == nil, "a click on the axis clears it")
key("escape")
check(st.frame == -1 and st.span == -1, "Esc clears the selection")

-- Up and down the lanes.
key("0")
key("j")
local top = st.span
check(top >= 0 and st.rec.spans:get(top, "depth") == 0, "j with nothing selected: the top lane's first span")
key("down")
check(st.span ~= top and st.rec.spans:get(st.span, "depth") == 1, "↓ its first child")
key("k")
check(st.span == top, "k its parent")
key("up")
check(st.span == top, "↑ at the top lane stays")
key("escape")
local before = st.search_focus
key("/")
check(st.search_focus == before + 1, "/ asks the search for the keyboard")

-- The tables over the window.
update({ kind = "ins_zoom", zoom = { start = lo, length = (hi - lo) / 10 } })
require("inspector").restat(st)
check(st.stats.summary.frames < frames, "the tables read the window: " .. st.stats.summary.frames)
update({ kind = "ins_tab", change = { value = "spans" } })
check(st.tab == "spans" and #st.tables.spans > 0, "the spans table")

-- The filter.
local spans = #st.rec.spans
update({ kind = "ins_name", change = { text = "fill" } })
check(#st.rec.spans < spans and #st.rec.spans > 0, "a name keeps its spans: " .. #st.rec.spans)
update({ kind = "ins_name", change = { text = "" } })
update({ kind = "ins_frames", change = { value = "slow" } })
check(#st.rec.frames < frames, "only slow frames: " .. #st.rec.frames)
update({ kind = "ins_frames", change = { value = "all" } })
update({ kind = "ins_origin", name = "host" })
check(#st.rec.spans < spans, "an origin hidden")
update({ kind = "ins_origin", name = "host" })
check(#st.rec.spans == spans, "and shown again")

-- The dock.
update({ kind = "ins_dock", resize = { fraction = 0.7 } })
check(st.dock == 0.7, "the dock's rule moves")

-- The lanes.
update({ kind = "ins_lane", name = st.lanes[1] })
check(#st.lanes == 1, "a lane hidden")

-- Live: the recording grows.
if rest then
  kui.fs.append(path, kui.fs.read(rest))
  update({ kind = "moved" })
  check(#st.rec.frames > frames, "Live brings the frames that came: " .. #st.rec.frames)
  m.live = false
  update({ kind = "moved" })
  check(m.waiting == 1, "off, they wait for a refresh")
  update({ kind = "refresh" })
  check(m.waiting == 0, "and refresh takes them in")
end

kui.fs.try_remove(kui.data_dir .. "/prefs.json")
kui.fs.try_remove(kui.data_dir)
print(failed == 0 and "all passed" or (failed .. " failed"))
kui.exit(failed)
