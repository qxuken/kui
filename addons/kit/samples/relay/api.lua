-- Larder's API, mocked: a grocery-delivery backend Relay talks to, served
-- from the app itself with `kui.http.serve`. Every handler is a task, so
-- the simulated database time is a plain `kui.sleep`, and every answer
-- carries a `Server-Timing` header and the time the request arrived — what
-- Relay's timing tab draws.

local M = {}

local CUSTOMERS = {
  cus_1042 = { id = "cus_1042", name = "Maren Holt", email = "maren.holt@fastmail.com", city = "Leipzig", orders = 38, since = "2023-04-11" },
  cus_2210 = { id = "cus_2210", name = "Tomás Ibarra", email = "t.ibarra@posteo.de", city = "Berlin", orders = 7, since = "2025-01-29" },
  cus_0388 = { id = "cus_0388", name = "Ada Okafor", email = "ada@okafor.studio", city = "Hamburg", orders = 112, since = "2021-09-02" },
  cus_3175 = { id = "cus_3175", name = "Linnea Berg", email = "linnea.berg@gmail.com", city = "Dresden", orders = 3, since = "2026-08-17" },
}

local PRODUCTS = {
  { sku = "VEG-0112", name = "Heirloom tomatoes, 500 g", category = "produce", price = 3.49, stock = 84 },
  { sku = "VEG-0140", name = "Rainbow chard, bunch", category = "produce", price = 2.19, stock = 31 },
  { sku = "DAI-0203", name = "Cultured butter, 250 g", category = "dairy", price = 4.25, stock = 12 },
  { sku = "DAI-0217", name = "Whole milk, 1 l (glass)", category = "dairy", price = 1.69, stock = 140 },
  { sku = "BAK-0301", name = "Rye sourdough loaf", category = "bakery", price = 4.80, stock = 0 },
  { sku = "BAK-0322", name = "Cardamom buns, 4 pack", category = "bakery", price = 5.20, stock = 18 },
  { sku = "PAN-0410", name = "Arborio rice, 1 kg", category = "pantry", price = 3.95, stock = 57 },
  { sku = "PAN-0455", name = "Cold-pressed olive oil, 500 ml", category = "pantry", price = 11.90, stock = 23 },
  { sku = "BEV-0520", name = "Oat drink barista, 1 l", category = "drinks", price = 2.49, stock = 96 },
}

local ORDERS = {
  { id = "ord_8F3K2", customer = "cus_1042", status = "open", placed = "2026-10-25T08:14:03Z", slot = "10:00–12:00", total = 42.37,
    items = { { sku = "VEG-0112", qty = 2 }, { sku = "DAI-0203", qty = 1 }, { sku = "PAN-0455", qty = 2 }, { sku = "BEV-0520", qty = 3 } } },
  { id = "ord_8F3JX", customer = "cus_0388", status = "open", placed = "2026-10-25T07:52:41Z", slot = "12:00–14:00", total = 18.62,
    items = { { sku = "BAK-0322", qty = 2 }, { sku = "DAI-0217", qty = 4 }, { sku = "VEG-0140", qty = 1 } } },
  { id = "ord_8F3H1", customer = "cus_2210", status = "packing", placed = "2026-10-25T07:30:09Z", slot = "09:00–11:00", total = 27.05,
    items = { { sku = "PAN-0410", qty = 2 }, { sku = "VEG-0112", qty = 3 }, { sku = "DAI-0203", qty = 2 } } },
  { id = "ord_8F2ZQ", customer = "cus_3175", status = "out_for_delivery", placed = "2026-10-24T19:02:55Z", slot = "08:00–10:00", total = 9.87,
    items = { { sku = "BEV-0520", qty = 2 }, { sku = "DAI-0217", qty = 3 } } },
  { id = "ord_8F2W7", customer = "cus_1042", status = "delivered", placed = "2026-10-24T16:45:12Z", slot = "18:00–20:00", total = 61.40,
    items = { { sku = "PAN-0455", qty = 3 }, { sku = "BAK-0322", qty = 2 }, { sku = "VEG-0140", qty = 4 } } },
  { id = "ord_8F2TB", customer = "cus_0388", status = "cancelled", placed = "2026-10-24T11:20:37Z", slot = "14:00–16:00", total = 14.20,
    items = { { sku = "BAK-0301", qty = 2 }, { sku = "DAI-0203", qty = 1 } } },
}

local next_order = 0x41

local function by_sku(sku)
  for _, p in ipairs(PRODUCTS) do if p.sku == sku then return p end end
end

local function find_order(id)
  for _, o in ipairs(ORDERS) do if o.id == id then return o end end
end

local function summary(o)
  local c = CUSTOMERS[o.customer]
  return { id = o.id, status = o.status, placed = o.placed, slot = o.slot, total = o.total,
    customer = { id = c.id, name = c.name }, items = #o.items }
end

local function detail(o)
  local lines = {}
  for i, it in ipairs(o.items) do
    local p = by_sku(it.sku)
    lines[i] = { sku = it.sku, name = p.name, qty = it.qty, price = p.price, subtotal = math.floor(p.price * it.qty * 100 + 0.5) / 100 }
  end
  local s = summary(o)
  s.items = lines
  s.customer = CUSTOMERS[o.customer]
  return s
end

-- A handler's answer with its timing: `phases` are { name, ms } run in
-- order (the sleeps are the "database"), `arrived` when the request did.
local function answer(arrived, phases, res)
  local parts = {}
  for _, ph in ipairs(phases) do
    kui.sleep(ph[2])
    parts[#parts + 1] = string.format("%s;dur=%.1f", ph[1], ph[2])
  end
  res.headers = res.headers or {}
  res.headers["Server-Timing"] = table.concat(parts, ", ")
  res.headers["X-Arrived-At"] = string.format("%.3f", arrived)
  res.headers["X-Request-Id"] = string.format("req_%x", math.floor(arrived) % 0xffffff)
  res.headers["Cache-Control"] = "no-store"
  res.headers["Server"] = "larder-api/2.14.0"
  return res
end

local function jitter(ms) return ms * (0.8 + math.random() * 0.4) end

-- Every /api route but the token check wants a bearer token.
local function guard(req, arrived)
  local auth = req.headers["authorization"] or req.headers["Authorization"]
  if not auth or not auth:match("^Bearer %S+") then
    return answer(arrived, { { "auth", jitter(0.6) } }, { status = 401,
      json = { error = "unauthorized", message = "Missing or malformed bearer token. Send `Authorization: Bearer <token>`." } })
  end
end

-- Starts the API on `port` (0: any free port) and returns the server.
-- `on_hit(method, path, status)` hears every request.
function M.serve(port, on_hit)
  local s = kui.http.serve({ port = port, host = "127.0.0.1" })
  local function route(method, path, fn)
    s:route(method, path, function(req)
      local arrived = kui.now()
      local res = guard(req, arrived) or fn(req, arrived)
      if on_hit then on_hit(method, req.path, res.status or 200) end
      return res
    end)
  end

  route("GET", "/api/orders", function(req, arrived)
    local status, limit = req.query.status, tonumber(req.query.limit) or 50
    local out = {}
    for _, o in ipairs(ORDERS) do
      if (not status or status == "" or o.status == status) and #out < limit then out[#out + 1] = summary(o) end
    end
    return answer(arrived, { { "auth", jitter(0.8) }, { "db", jitter(34) }, { "render", jitter(3) } },
      { json = { data = kui.json.array(out), count = #out, has_more = false } })
  end)

  route("GET", "/api/orders/:id", function(req, arrived)
    local o = find_order(req.params.id)
    if not o then
      return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(12) } },
        { status = 404, json = { error = "not_found", message = "No order " .. req.params.id .. "." } })
    end
    return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(21) }, { "render", jitter(2) } }, { json = detail(o) })
  end)

  route("POST", "/api/orders", function(req, arrived)
    local body = req.json
    if type(body) ~= "table" or not CUSTOMERS[body.customer or ""] or type(body.items) ~= "table" or #body.items == 0 then
      return answer(arrived, { { "auth", jitter(0.7) }, { "validate", jitter(1.5) } }, { status = 422, json = {
        error = "invalid_order",
        message = "An order needs a known `customer` and at least one item.",
        fields = { customer = body and body.customer and not CUSTOMERS[body.customer] and "unknown customer" or nil,
          items = (not body or type(body.items) ~= "table" or #body.items == 0) and "must not be empty" or nil },
      } })
    end
    local total = 0
    for _, it in ipairs(body.items) do
      local p = by_sku(it.sku or "")
      if not p then
        return answer(arrived, { { "auth", jitter(0.7) }, { "validate", jitter(2) } },
          { status = 422, json = { error = "invalid_order", message = "Unknown SKU " .. tostring(it.sku) .. "." } })
      end
      total = total + p.price * (it.qty or 1)
    end
    local o = { id = string.format("ord_8F4%02X", next_order), customer = body.customer, status = "open",
      placed = os.date("!%Y-%m-%dT%H:%M:%SZ"), slot = body.slot or "16:00–18:00", total = math.floor(total * 100 + 0.5) / 100, items = body.items }
    next_order = next_order + 1
    table.insert(ORDERS, 1, o)
    return answer(arrived, { { "auth", jitter(0.8) }, { "validate", jitter(2) }, { "db", jitter(48) }, { "render", jitter(2.5) } },
      { status = 201, headers = { Location = "/api/orders/" .. o.id }, json = detail(o) })
  end)

  route("DELETE", "/api/orders/:id", function(req, arrived)
    local o = find_order(req.params.id)
    if not o then return answer(arrived, { { "db", jitter(10) } }, { status = 404, json = { error = "not_found" } }) end
    if o.status ~= "open" then
      return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(14) } }, { status = 409,
        json = { error = "conflict", message = "Order " .. o.id .. " is " .. o.status:gsub("_", " ") .. " and can no longer be cancelled." } })
    end
    o.status = "cancelled"
    return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(26) } }, { status = 204 })
  end)

  route("GET", "/api/products", function(req, arrived)
    local q = (req.query.q or ""):lower()
    local cat = req.query.category
    local out = {}
    for _, p in ipairs(PRODUCTS) do
      if (q == "" or p.name:lower():find(q, 1, true)) and (not cat or cat == "" or p.category == cat) then out[#out + 1] = p end
    end
    return answer(arrived, { { "auth", jitter(0.6) }, { "cache", jitter(1.2) }, { "render", jitter(1.5) } },
      { json = { data = kui.json.array(out), count = #out } })
  end)

  route("GET", "/api/customers/:id", function(req, arrived)
    local c = CUSTOMERS[req.params.id]
    if not c then return answer(arrived, { { "db", jitter(9) } }, { status = 404, json = { error = "not_found", message = "No customer " .. req.params.id .. "." } }) end
    return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(16) } }, { json = c })
  end)

  route("GET", "/api/reports/daily", function(req, arrived)
    return answer(arrived, { { "auth", jitter(0.7) }, { "db", jitter(380) }, { "aggregate", jitter(120) }, { "render", jitter(6) } },
      { json = { date = req.query.date or "2026-10-24", orders = 214, revenue = 6481.20, avg_basket = 30.29,
        late_deliveries = 9, top_sku = "BEV-0520", by_slot = { ["08:00–10:00"] = 31, ["10:00–12:00"] = 58, ["12:00–14:00"] = 44,
          ["14:00–16:00"] = 27, ["16:00–18:00"] = 33, ["18:00–20:00"] = 21 } } })
  end)

  route("POST", "/api/inventory/sync", function(req, arrived)
    return answer(arrived, { { "auth", jitter(0.7) }, { "upstream", 900 } }, { status = 503, headers = { ["Retry-After"] = "30" },
      json = { error = "upstream_timeout", message = "The warehouse system (wms-eu-2) did not answer within 900 ms.", retry_after = 30 } })
  end)

  s:get("/*", function(req)
    local res = { status = 404, json = { error = "not_found", message = "No route for GET " .. req.path .. "." } }
    if on_hit then on_hit("GET", req.path, 404) end
    return res
  end)
  return s
end

return M
