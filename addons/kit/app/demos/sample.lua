-- The sample data more than one card shows: a few endpoints as a small
-- channel, and the requests counted by status.

-- Source also: jobs/generate.lua

local M = {}

-- A few named things with a method and a tone, for a list and a table.
M.endpoints = channel({
  title = { "/api/items", "/api/users", "/api/search", "/api/orders", "/health", "/static/app.js" },
  detail = { "12 ms · 340 today", "40 ms · 88 today", "410 ms · 1,204 today", "1.2 s · 9 today", "2 ms · 8,640 today", "6 ms · 2,410 today" },
  method = { "GET", "POST", "GET", "PUT", "GET", "GET" },
  tone = { "ok", "accent", "warn", "danger", "ok", "neutral" },
})

-- Requests by status, counted over the first 5,000 of `rows` (the
-- worker's channel, main.lua's `model.rows`): a small channel, counted
-- again only when the rows are new.
local counted = nil
function M.by_status(rows)
  if counted and counted.src == rows then return counted.ch end
  local counts, order = {}, {}
  if rows then
    for i = 0, math.min(#rows, 5000) - 1 do
      local s = tostring(rows:get(i, "status"))
      if not counts[s] then counts[s] = 0; order[#order + 1] = s end
      counts[s] = counts[s] + 1
    end
  end
  table.sort(order)
  local n = {}
  for i, s in ipairs(order) do n[i] = counts[s] end
  local ch = channel({ status = order, n = n })
  counted = { src = rows, ch = ch }
  return ch
end

return M
