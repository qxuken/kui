-- Relay's window: the history on the left; the request (method, URL,
-- params, headers, body) over the response (status, body, headers,
-- timing) on the right.

local kit = require("kit.ui")

local METHODS = {
  { value = "GET", label = "GET" }, { value = "POST", label = "POST" }, { value = "PUT", label = "PUT" },
  { value = "PATCH", label = "PATCH" }, { value = "DELETE", label = "DELETE" },
}

local SKINS = {
  { value = "ledger", label = "Ledger" }, { value = "neutral", label = "Neutral" }, { value = "phosphor", label = "Phosphor" },
}

local function muted(s, o)
  local t = { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", max_lines = 1, ellipsis = true }
  for k, v in pairs(o or {}) do t[k] = v end
  return text(s, t)
end

local function mono(s, o)
  local t = { size = "$kitSizeLabel", family = "$kitData", color = "$kitMuted", max_lines = 1, ellipsis = true }
  for k, v in pairs(o or {}) do t[k] = v end
  return text(s, t)
end

local function tone_of(status)
  if not status then return "danger" end
  if status < 300 then return "ok" end
  if status < 400 then return "neutral" end
  if status < 500 then return "warn" end
  return "danger"
end

local function size_of(n)
  if n < 1024 then return n .. " B" end
  return string.format("%.1f KB", n / 1024)
end

local function ms_of(ms)
  if ms >= 1000 then return string.format("%.2f s", ms / 1000) end
  return string.format("%.0f ms", ms)
end

local function age_of(at)
  local s = math.max(0, (kui.now() - at) / 1000)
  if s < 45 then return "just now" end
  if s < 3600 then return string.format("%d min ago", math.floor(s / 60 + 0.5)) end
  return os.date("%H:%M", math.floor(at / 1000))
end

local function path_of(url)
  return (url:gsub("^{{base}}", ""))
end

-- ---------- history ----------

local history_ch = nil
-- A method's colour in the history: reads green, writes amber, deletes red.
local METHOD_TONE = { GET = "ok", HEAD = "ok", POST = "warn", PUT = "warn", PATCH = "warn", DELETE = "danger" }

local function history_channel(m)
  local title, detail, method, tone = {}, {}, {}, {}
  for i, e in ipairs(visible_history(m)) do
    title[i] = path_of(e.url)
    method[i], tone[i] = e.method, METHOD_TONE[e.method] or "neutral"
    if e.response then
      detail[i] = string.format("%d %s · %s · %s", e.response.status, reason(e.response.status), ms_of(e.response.ms), age_of(e.at))
    else
      detail[i] = "failed · " .. age_of(e.at)
    end
  end
  local cols = { title = title, detail = detail, method = method, tone = tone }
  history_ch = history_ch and history_ch:publish(cols) or channel(cols)
  return history_ch
end

local function history_pane(m)
  local shown = visible_history(m)
  local body
  if #m.history == 0 then
    body = kit.empty({ title = "No history", detail = "Requests you send land here, newest first." })
  elseif #shown == 0 then
    body = kit.empty({ title = "Nothing matches “" .. m.query .. "”", detail = "The search reads methods and paths." },
      { kit.button({ label = "Clear the search", on_press = { kind = "query", change = { text = "" } } }) })
  else
    body = kit.list({ key = "history", label = "History", rows = history_channel(m), title = "title", detail = "detail",
      badge = "method", tone = "tone", badge_width = 58,
      selected = m.selected, grow = true, on_select = { kind = "pick" } })
  end
  return column { key = "history-pane", width = "grow", height = "grow", bg = "$kitSide",
    row { key = "history-head", width = "grow", pad = { x = 10, y = 8 }, gap = 6, cross_align = "center",
      column { width = "grow",
        kit.search({ key = "history-search", value = m.query, placeholder = "Filter history", label = "Filter the history",
          count = m.query ~= "" and string.format("%d of %d", #shown, #m.history) or nil, on_change = { kind = "query" } }) },
      kit.icon_button({ key = "clear-history", icon = "trash", label = "Clear the history", disabled = #m.history == 0,
        on_press = { kind = "clear_history" } }),
    },
    column { key = "history-body", width = "grow", height = "grow", body },
  }
end

-- ---------- request ----------

local function rows_editor(m, list, noun)
  local rows = m[list]
  local out = {}
  if #rows == 0 then
    return column { key = list, width = "grow", gap = 6, pad = { y = 4 },
      muted(noun == "param" and "No query params: the URL is sent as it is." or "No headers: only what the client adds itself."),
      row { kit.button({ label = "Add " .. noun, icon = "plus", variant = "ghost", on_press = { kind = "row_add", list = list } }) } }
  end
  out[#out + 1] = row { key = "head", width = "grow", gap = 8, pad = { x = 2 },
    row { width = 20 },
    row { width = 180, muted("Key") },
    row { width = "grow", muted("Value") },
    row { width = "$kitCtl" },
  }
  for i, r in ipairs(rows) do
    out[#out + 1] = row { key = list .. i, width = "grow", gap = 8, cross_align = "center",
      kit.checkbox({ key = "on", label = "Send " .. (r.key ~= "" and r.key or "this " .. noun), hide_label = true,
        checked = r.on, on_toggle = { kind = "row_on", list = list, i = i } }),
      kit.text_field({ key = "k", label = noun .. " " .. i .. " name", hide_label = true, mono = true, width = 180,
        placeholder = "name", value = r.key, on_change = { kind = "row_key", list = list, i = i } }),
      kit.text_field({ key = "v", label = noun .. " " .. i .. " value", hide_label = true, mono = true,
        placeholder = "value", value = r.value, on_change = { kind = "row_value", list = list, i = i } }),
      kit.icon_button({ key = "x", icon = "x", label = "Remove " .. (r.key ~= "" and r.key or "this " .. noun),
        on_press = { kind = "row_remove", list = list, i = i } }),
    }
  end
  out[#out + 1] = row { key = "add", width = "grow", pad = { y = 4 },
    kit.button({ label = "Add " .. noun, icon = "plus", variant = "ghost", on_press = { kind = "row_add", list = list } }) }
  return column { key = list, width = "grow", gap = 4, table.unpack(out) }
end

local function count_on(rows)
  local n = 0
  for _, r in ipairs(rows) do if r.on and r.key ~= "" then n = n + 1 end end
  return n
end

local function body_editor(m)
  if m.method == "GET" then
    return kit.notice({ key = "no-body", message = "GET requests carry no body. Switch the method to POST, PUT or PATCH to send one." })
  end
  return kit.text_area({ key = "body", label = "Request body", hide_label = true, mono = true, grow = true, value = m.body,
    error = m.body_error, placeholder = '{ "customer": "cus_1042", "items": [] }', on_change = { kind = "body" } })
end

local function request_pane(m)
  local tabs = {
    { value = "params", label = "Params", count = count_on(m.params) },
    { value = "headers", label = "Headers", count = count_on(m.headers) },
    { value = "body", label = "Body" },
  }
  local body
  if m.req_tab == "params" then body = rows_editor(m, "params", "param")
  elseif m.req_tab == "headers" then body = rows_editor(m, "headers", "header")
  else body = body_editor(m) end
  return kit.tabs({ key = "req-tabs", tabs = tabs, value = m.req_tab, on_change = { kind = "req_tab" } }, {
    trailing = { kit.button({ key = "curl", label = "Copy as curl", icon = "copy", variant = "ghost", on_press = { kind = "copy_curl" } }) },
    column { key = "req-body", width = "grow", height = "grow", scroll_y = m.req_tab ~= "body", pad = 10, body },
  })
end

-- ---------- response ----------

local function pretty(r)
  if r.json ~= nil then return kui.json.encode(r.json, { pretty = true }) end
  return r.body or ""
end

local function timing_view(r)
  local name, start, len, depth, group, target = {}, {}, {}, {}, {}, {}
  local entries = {}
  for i, s in ipairs(r.spans) do
    name[i], start[i], len[i], depth[i], group[i] = s.name, s.start, s.len, s.depth, s.group
    target[i] = string.format("%.1f ms", s.len)
    entries[#entries + 1] = { key = string.rep("  ", s.depth) .. s.name, value = string.format("%8.1f ms", s.len), mono = true }
  end
  local bars = channel({ name = name, start = start, len = len, depth = depth, group = group, target = target })
  return column { key = "timing", width = "grow", height = "grow", gap = 12,
    column { key = "wf", width = "grow", height = 150,
      kit.waterfall({ key = "waterfall", bars = bars, grow = true }) },
    kit.inspector({ key = "phases", entries = entries, key_width = 160 }),
  }
end

local function response_pane(m)
  if m.sending then
    return column { key = "waiting", width = "grow", height = "grow", main_align = "center", cross_align = "center",
      column { width = 320, kit.progress({ label = "Waiting for " .. full_url(m):match("^%a+://([^/]+)") .. "…" }) } }
  end
  if m.error then
    return column { key = "error", width = "grow", height = "grow", pad = 12, gap = 10,
      kit.notice({ key = "error-notice", tone = "danger", title = "Could not get a response",
        message = m.error.message .. " — after " .. ms_of(m.error.ms) .. "." }, {
        kit.button({ label = "Retry", icon = "refresh", on_press = { kind = "send" } }),
        m.env ~= "local" and kit.button({ label = "Use the local mock", on_press = { kind = "env", change = { value = "local" } } }) or nil,
      }),
      kit.inspector({ entries = { { key = "URL", value = m.error.url, mono = true }, { key = "Environment", value = m.env } }, key_width = 110 }),
    }
  end
  local r = m.response
  if not r then
    return column { key = "none", width = "grow", height = "grow", main_align = "center",
      kit.empty({ title = "No response yet", detail = "Send the request to see its status, body, headers and timing." },
        { kit.button({ label = "Send", variant = "primary", icon = "play", on_press = { kind = "send" } }) }) }
  end
  local headers = {}
  for k, v in pairs(r.headers or {}) do headers[#headers + 1] = { key = k, value = v, mono = true } end
  table.sort(headers, function(a, b) return a.key < b.key end)
  local tabs = {
    { value = "body", label = "Body" },
    { value = "headers", label = "Headers", count = #headers },
    { value = "timing", label = "Timing" },
  }
  local body
  if m.res_tab == "headers" then
    body = kit.inspector({ key = "res-headers", entries = headers, key_width = 170 })
  elseif m.res_tab == "timing" then
    body = timing_view(r)
  elseif (r.body or "") == "" then
    body = kit.empty({ title = "No body", detail = r.status .. " " .. reason(r.status) .. " answers with headers only." })
  else
    body = kit.code({ key = "res-body", text = pretty(r) })
  end
  return kit.tabs({ key = "res-tabs", tabs = tabs, value = m.res_tab, on_change = { kind = "res_tab" } }, {
    trailing = {
      kit.badge({ key = "status", label = r.status .. " " .. reason(r.status), tone = tone_of(r.status), dot = true }),
      mono(ms_of(r.ms) .. "  ·  " .. size_of(r.size), { color = "$kitFg" }),
      kit.icon_button({ key = "copy-body", icon = "copy", label = "Copy the body", disabled = (r.body or "") == "",
        on_press = { kind = "copy_body" } }),
    },
    column { key = "res-body-box", width = "grow", height = "grow", scroll_y = true, pad = 10, body },
  })
end

-- ---------- window ----------

local function request_bar(m)
  return column { key = "bar", width = "grow", pad = { x = 10, y = 8 }, gap = 4,
    row { key = "line", width = "grow", gap = 8, cross_align = "end",
      row { key = "method", width = 104,
        kit.select({ key = "method-select", options = METHODS, value = m.method, on_change = { kind = "method" } }) },
      column { key = "url", width = "grow",
        kit.text_field({ key = "url-field", label = "Request URL", hide_label = true, mono = true, value = m.url,
          placeholder = "{{base}}/api/…", on_change = { kind = "url" }, on_submit = { kind = "send" } }) },
      kit.button({ key = "send", label = m.sending and "Sending…" or "Send", variant = "primary", icon = "play",
        shortcut = "↵", disabled = m.sending, on_press = { kind = "send" } }),
    },
    mono("→ " .. full_url(m), { color = "$kitFaint" }),
  }
end

return function(m)
  local base = BASES[m.env]
  return column { window_title = "Relay — Larder API", width = "grow", height = "grow", bg = "$kitBg",
    kit.settings({ skin = m.skin }),
    kit.toolbar({ key = "top" }, {
      text("Relay", { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" }),
      row { key = "env", width = 170,
        kit.select({ key = "env-select", options = ENVS, value = m.env, on_change = { kind = "env" } }) },
      mono(base),
      trailing = {
        kit.segmented({ key = "skin", label = "Skin", value = m.skin, options = SKINS, on_change = { kind = "skin" } }),
      },
    }),
    kit.split({ key = "sides", fraction = m.side, min = 240, on_resize = { kind = "side" } }, {
      first = { history_pane(m) },
      second = {
        column { key = "main", width = "grow", height = "grow",
          request_bar(m),
          kit.divider({}),
          kit.split({ key = "req-res", axis = "column", fraction = m.vsplit, min = 120, on_resize = { kind = "vsplit" } }, {
            first = { request_pane(m) },
            second = { response_pane(m) },
          }),
        },
      },
    }),
    kit.statusbar({ key = "status" }, {
      kit.badge({ label = "mock API", tone = "ok", dot = true }),
      muted(string.format("Larder on 127.0.0.1:%d · %d requests served", boot.server.port, m.hits)),
      trailing = { muted(string.format("%d in history", #m.history)) },
    }),
    m.toast and kit.toast({ key = "toast", message = m.toast.message, tone = m.toast.tone, on_dismiss = { kind = "toast_off" } }) or nil,
  }
end
