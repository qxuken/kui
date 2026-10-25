-- Watchtower's window: the headline numbers across the top; the edge's
-- charts over the services table on the left; the selected service and
-- the events (log, errors, latency spread) on the right.

local fleet = require("fleet")
local kit = require("kit.ui")
local state = require("state")

local type Model = state.Model

local record view
  visible: function(m: Model): {fleet.Service}
  window: function(m: Model, env: kui.Env): kui.Node
end

local RANGES: {string:number} = { ["5m"] = 5, ["15m"] = 15, ["1h"] = 60 }

local STATUS: {string:{string, kit.Tone}} = {
  healthy = { "healthy", "ok" },
  degraded = { "degraded", "warn" },
  down = { "down", "danger" },
  restarting = { "restarting", "accent" },
}
local RANK: {string:integer} = { down = 1, degraded = 2, restarting = 3, healthy = 4 }

local function muted(s: string): kui.Node
  return text(s, { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", max_lines = 1, ellipsis = true } as kui.text)
end

local function thousands(n: number): string
  local s = tostring(math.floor(n + 0.5))
  local out = s:reverse():gsub("(%d%d%d)", "%1,"):reverse()
  return (out:gsub("^,", ""))
end

local function ms(n: number): string
  if n >= 1000 then return ("%.2f s"):format(n / 1000) end
  return ("%d ms"):format(math.floor(n + 0.5))
end

local function pct(n: number): string
  if n >= 10 then return ("%d%%"):format(math.floor(n + 0.5)) end
  return ("%.2f%%"):format(n)
end

-- ---------- services ----------

function view.visible(m: Model): {fleet.Service}
  local q = m.query:lower()
  local out: {fleet.Service} = {}
  for _, v in ipairs(fleet.services) do
    local keep = (q == "" or v.name:find(q, 1, true) ~= nil or v.team:find(q, 1, true) ~= nil)
      and (m.filter == "all" or (m.filter == "attention" and v.status ~= "healthy") or v.status as string == m.filter)
    if keep then out[#out + 1] = v end
  end
  local f = m.sort
  table.sort(out, function(a: fleet.Service, b: fleet.Service): boolean
    local x, y: any, any
    if f == "status" then x, y = RANK[a.status], RANK[b.status]
    elseif f == "name" then x, y = a.name, b.name
    else
      x, y = (a as {string:any})[f], (b as {string:any})[f]
    end
    if x == y then return a.name < b.name end
    if m.descending then return (x as number) > (y as number) end
    return (x as number) < (y as number)
  end)
  return out
end

local COLUMNS: {kit.Column} = {
  { field = "name", title = "Service" },
  { field = "status", title = "Status", width = 104, tone = "tone", badge = true },
  { field = "version", title = "Version", width = 92, mono = true },
  { field = "instances", title = "Inst", width = 48, align = "end", mono = true },
  { field = "rps", title = "Req/s", width = 70, align = "end", mono = true },
  { field = "p95", title = "p95", width = 76, align = "end", mono = true },
  { field = "errors", title = "Errors", width = 70, align = "end", mono = true },
  { field = "cpu", title = "CPU", width = 56, align = "end", mono = true },
}

local services_ch: kui.Channel = nil
local function services_channel(list: {fleet.Service}): kui.Channel
  local c: {string:{any}} = { name = {}, status = {}, tone = {}, version = {}, instances = {}, rps = {}, p95 = {}, errors = {}, cpu = {} }
  for i, v in ipairs(list) do
    c.name[i] = v.name
    c.status[i] = v.status
    c.tone[i] = STATUS[v.status][2]
    c.version[i] = v.version
    c.instances[i] = tostring(v.instances)
    c.rps[i] = v.status == "down" and "—" or thousands(v.rps)
    c.p95[i] = v.status == "down" and "—" or ms(v.p95)
    c.errors[i] = pct(v.errors)
    c.cpu[i] = ("%d%%"):format(math.floor(v.cpu + 0.5))
  end
  services_ch = services_ch and services_ch:publish(c) or channel(c)
  return services_ch
end

local function services_panel(m: Model): kui.Node
  local list = view.visible(m)
  local attention, down = 0, 0
  for _, v in ipairs(fleet.services) do
    if v.status ~= "healthy" then attention = attention + 1 end
    if v.status == "down" then down = down + 1 end
  end
  local selected: integer = nil
  for i, v in ipairs(list) do if v.name == m.selected then selected = i - 1 end end
  local body: kui.Node
  if #list == 0 then
    body = kit.empty({ title = "No services match", detail = m.query ~= "" and ("Nothing named or owned by “%s”."):format(m.query) or "Nothing in this state right now." },
      { kit.button({ label = "Show all", on_press = { kind = "reset_filters" } }) })
  else
    body = kit.table({ key = "services", columns = COLUMNS, rows = services_channel(list), selected = selected, sort = m.sort,
      descending = m.descending, grow = true, on_select = { kind = "select" }, on_sort = { kind = "sort" } })
  end
  return kit.panel({ key = "services-panel", title = "Services", grow = true, pad = 0 }, {
    actions = {
      row { key = "svc-search", width = 180,
        kit.search({ key = "svc-query", value = m.query, placeholder = "Service or team", label = "Filter the services",
          on_change = { kind = "query" } }) },
      kit.segmented({ key = "svc-filter", label = "Which services", value = m.filter, on_change = { kind = "filter" }, options = {
        { value = "all", label = "All", count = #fleet.services },
        { value = "attention", label = "Attention", count = attention, alert = attention > 0 },
        { value = "down", label = "Down", count = down, alert = down > 0 },
      } }),
    },
    column { key = "svc-body", width = "grow", height = "grow", body },
  })
end

-- ---------- the edge ----------

local function last(t: {number}): number return t[#t] end
local function ago(t: {number}, n: integer): number return t[math.max(1, #t - n)] end

local function delta(now: number, before: number, good_when_down: boolean): string, kit.Tone
  if before == 0 then return nil, nil end
  local d = (now - before) / before * 100
  local up = d >= 0
  local good = up ~= good_when_down
  if math.abs(d) < 1 then return "steady vs 15 min ago", "neutral" end
  if now / before >= 3 then return ("×%d vs 15 min ago"):format(math.floor(now / before + 0.5)), good and "ok" or "danger" end
  return ("%s%d%% vs 15 min ago"):format(up and "+" or "−", math.floor(math.abs(d) + 0.5)), good and "ok" or "danger"
end

local function card(key: string, label: string, value: string, d: string, tone: kit.Tone, spark: kui.Node): kui.Node
  return column { key = key, width = "grow", bg = "$kitPanel", radius = "$kitRadius", pad = { x = 14, y = 10 }, gap = 6,
    kit.stat({ label = label, value = value, delta = d, tone = tone }),
    spark,
  }
end

local function headline(m: Model): kui.Node
  local s = fleet.series
  local d1, t1 = delta(last(s.rps), ago(s.rps, 900), false)
  local d2, t2 = delta(last(s.p95), ago(s.p95, 900), true)
  local d3, t3 = delta(last(s.errors), ago(s.errors, 900), true)
  return row { key = "headline", width = "grow", gap = 12, pad = { x = 12, y = 12 },
    card("rps", "Edge requests / s", thousands(last(s.rps)), d1, t1,
      kit.sparkline({ data = m.chart, field = "rps", width = 220, height = 22 })),
    card("p95", "Edge p95 latency", ms(last(s.p95)), d2, t2,
      kit.sparkline({ data = m.chart, field = "p95", width = 220, height = 22, color = "$kitWarn" })),
    card("err", "5xx rate", pct(last(s.errors)), d3, t3,
      kit.sparkline({ data = m.chart, field = "errors", width = 220, height = 22, color = "$kitDanger" })),
    card("inc", "Open incidents", "2", "INC-2291 · INC-2292", "danger",
      row { width = "grow", gap = 6, kit.badge({ label = "inventory-sync", tone = "danger", dot = true }),
        kit.badge({ label = "checkout-api", tone = "warn", dot = true }) }),
  }
end

local function charts(m: Model): kui.Node
  local window = RANGES[m.range]
  return row { key = "charts", width = "grow", gap = 12,
    column { key = "lat", width = "grow",
      kit.panel({ key = "latency", title = "Edge latency", subtitle = "p50 · p95 · p99 in ms, by minutes ago" }, {
        kit.line_chart({ data = m.chart, x = "ago", window = window, height = 170, x_format = "ago", x_unit = "min", series = {
          { field = "p50", name = "p50" }, { field = "p95", name = "p95" }, { field = "p99", name = "p99" } } }),
      }) },
    column { key = "thr", width = "grow",
      kit.panel({ key = "throughput", title = "Throughput", subtitle = "requests a second, by minutes ago" }, {
        kit.line_chart({ data = m.chart, x = "ago", window = window, height = 170, x_format = "ago", x_unit = "min", area = true, series = {
          { field = "rps", name = "req/s" } } }),
      }) },
  }
end

-- ---------- the selected service ----------

local ONCALL: {string:string} = { platform = "s.lindqvist", checkout = "m.okafor", catalog = "r.hale", fulfilment = "j.weber", growth = "p.nair" }

local function detail(m: Model): kui.Node
  local v = fleet.find(m.selected)
  if not v then
    return kit.panel({ key = "detail", title = "Service" }, {
      kit.empty({ title = "Nothing selected", detail = "Pick a service in the table to see it here." }) })
  end
  local look = STATUS[v.status]
  local down = v.status == "down"
  return kit.panel({ key = "detail", title = v.name, subtitle = ("%s team · on call %s"):format(v.team, ONCALL[v.team] or "—") }, {
    actions = { kit.badge({ label = look[1], tone = look[2], dot = true }) },
    column { key = "detail-body", width = "grow", gap = 10,
      v.note and kit.notice({ key = "note", tone = look[2], message = v.note }) or nil,
      row { key = "numbers", width = "grow", gap = 16,
        kit.stat({ label = "Req/s", value = down and "—" or thousands(v.rps) }),
        kit.stat({ label = "p95", value = down and "—" or ms(v.p95), tone = v.p95 > 400 and "danger" or nil }),
        kit.stat({ label = "Errors", value = pct(v.errors), tone = v.errors > 1 and "danger" or nil }),
        kit.stat({ label = "CPU", value = ("%d%%"):format(math.floor(v.cpu + 0.5)), tone = v.cpu > 80 and "warn" or nil }),
      },
      row { key = "trend", width = "grow", gap = 8, cross_align = "center",
        muted("Req/s, last minute"),
        kit.sparkline({ data = channel({ rps = v.trend as {any} }), field = "rps", width = 200, height = 20 }),
      },
      kit.inspector({ key = "facts", key_width = 110, entries = {
        { key = "Version", value = v.version, mono = true },
        { key = "Deployed", value = v.deployed },
        { key = "Instances", value = ("%d running"):format(v.instances) },
      } }),
    },
    footer = {
      row { key = "detail-actions", width = "grow", gap = 8, pad = { x = 10, y = 8 },
        kit.button({ key = "restart", label = "Restart…", icon = "refresh", variant = "danger",
          disabled = v.status == "restarting", tooltip = v.status == "restarting" and "A restart is under way" or nil,
          on_press = { kind = "restart" } }),
        row { key = "scale-anchor",
          kit.button({ key = "scale", label = "Scale…", icon = "sliders", on_press = { kind = "scale" } }),
          m.scaling == v.name and kit.popover({ key = "scale-pop", label = "Scale " .. v.name, width = 260, on_dismiss = { kind = "scale_off" } }, {
            column { width = "grow", gap = 10,
              kit.slider({ key = "scale-slider", label = "Instances", value = m.scale_to, min = 1, max = 12, step = 1,
                value_label = ("%d instance%s"):format(m.scale_to, m.scale_to == 1 and "" or "s"),
                on_change = { kind = "scale_to" } }),
              text(("Now %d. Each instance takes about 40 s to warm up."):format(v.instances),
                { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", wrap = "word" } as kui.text),
              row { width = "grow", gap = 8, main_align = "end",
                kit.button({ label = "Cancel", variant = "ghost", on_press = { kind = "scale_off" } }),
                kit.button({ label = "Apply", variant = "primary", disabled = m.scale_to == v.instances, on_press = { kind = "apply_scale" } }),
              },
            },
          }) or nil,
        },
      },
    },
  })
end

local function events(m: Model): kui.Node
  local body: kui.Node
  if m.events == "errors" then
    body = column { key = "errors", width = "grow", pad = 10, gap = 8,
      muted("5xx rate by service, capped at 5%"),
      kit.bar_chart({ data = m.errors, label = "service", value = "rate", unit = "%", height = 200 }) }
  elseif m.events == "spread" then
    body = column { key = "spread", width = "grow", pad = 10, gap = 8,
      muted("Edge request latency, last 5 minutes (ms)"),
      kit.histogram({ data = m.latencies, field = "ms", bins = 30, height = 160 }) }
  else
    body = kit.log({ key = "events-log", rows = m.log, grow = true, wrap = true })
  end
  return kit.tabs({ key = "events", value = m.events, on_change = { kind = "events_tab" }, tabs = {
    { value = "log", label = "Events", count = #fleet.lines },
    { value = "errors", label = "Errors" },
    { value = "spread", label = "Latency spread" },
  } }, {
    column { key = "events-body", width = "grow", height = "grow", body },
  })
end

-- ---------- window ----------

local SKINS: {kit.Choice} = { { value = "ledger", label = "Ledger" }, { value = "neutral", label = "Neutral" }, { value = "phosphor", label = "Phosphor" } }

function view.window(m: Model, env: kui.Env): kui.Node
  local attention = 0
  for _, v in ipairs(fleet.services) do if v.status ~= "healthy" then attention = attention + 1 end end
  local target = m.confirm and fleet.find(m.confirm)
  return column { window_title = "Watchtower — Larder " .. m.env, width = "grow", height = "grow", bg = "$kitBg",
    kit.settings({ skin = m.skin as kit.Skin }),
    kit.toolbar({ key = "top" }, {
      text("Watchtower", { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" } as kui.text),
      kit.segmented({ key = "env", label = "Environment", value = m.env, on_change = { kind = "env" }, options = {
        { value = "prod-eu", label = "prod-eu" }, { value = "prod-us", label = "prod-us" }, { value = "staging", label = "staging" } } }),
      kit.segmented({ key = "range", label = "Time range", value = m.range, on_change = { kind = "range" }, options = {
        { value = "5m", label = "5 min" }, { value = "15m", label = "15 min" }, { value = "1h", label = "1 hour" } } }),
      trailing = {
        muted(m.live and ("live · " .. m.updated) or ("paused at " .. m.updated)),
        kit.switch({ key = "live", label = "Live", on = m.live, on_toggle = { kind = "live" } }),
        row { key = "skin", width = 120, kit.select({ key = "skin-select", options = SKINS, value = m.skin, on_change = { kind = "skin" } }) },
      },
    }),
    headline(m),
    kit.split({ key = "split", fraction = m.split, min = 380, on_resize = { kind = "split" } }, {
      first = { column { key = "left", width = "grow", height = "grow", gap = 12, pad = { x = 12, b = 12 },
        charts(m), services_panel(m) } },
      second = { column { key = "right", width = "grow", height = "grow", gap = 12, pad = { x = 12, b = 12 },
        detail(m), events(m) } },
    }),
    kit.statusbar({ key = "status" }, {
      kit.badge({ label = m.env, tone = m.env == "staging" and "neutral" or "accent" }),
      muted(("%d services · %d need attention"):format(#fleet.services, attention)),
      trailing = { muted("metrics: prometheus-eu-1 · scrape 1 s") },
    }),
    target and kit.dialog({ key = "restart-dialog", title = "Restart " .. target.name .. "?", width = 440, on_dismiss = { kind = "cancel" } }, {
      text(("All %d instances are cycled one at a time; each drains its in-flight requests for up to 30 s first."):format(target.instances),
        { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" } as kui.text),
      target.status == "down" and kit.notice({ tone = "warn", message = "It is down because its upstream is: a restart may not bring it back." }) or nil,
      actions = {
        kit.button({ label = "Keep running", on_press = { kind = "cancel" } }),
        kit.button({ label = "Restart " .. target.name, variant = "danger", icon = "refresh", on_press = { kind = "confirm_restart" } }),
      },
    }) or nil,
    m.toast and kit.toast({ key = "toast", message = m.toast, tone = m.toast_tone as kit.Tone, on_dismiss = { kind = "toast_off" } }) or nil,
  }
end


return view
