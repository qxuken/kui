-- Relay end to end, without a window: the mock API on a free port, real
-- requests through `update`, the state checked after each.
--
--   kui run test --headless          (from the sample's folder)
--
-- The exit code is the number of failures.

package.path = kui.dir .. "/../?.lua;" .. package.path
kui.args = { "--port", "0" }
require("main")
---@type any
model = init()

local failed = 0
local function check(ok, what)
  if ok then print("ok    " .. what) else failed = failed + 1; print("FAIL  " .. what) end
end

local function wait(cond, ms)
  local t = kui.now()
  while not cond() do
    if kui.now() - t > (ms or 5000) then return false end
    kui.sleep(10)
  end
  return true
end

-- Sends what the model holds and waits for the answer.
local function send()
  update({ kind = "send" })
  check(model.sending, "a send shows as under way")
  return wait(function() return not model.sending end)
end

kui.spawn(function()
  -- the seeds: real requests against the mock, in their order
  check(wait(function() return #model.history == 7 end), "the history is seeded with 7 answered requests")
  local statuses = {}
  for i, e in ipairs(model.history) do statuses[i] = e.response and e.response.status or 0 end
  check(table.concat(statuses, ",") == "200,200,200,201,404,503,200", "the seeds' statuses: " .. table.concat(statuses, ","))
  check(model.response and model.response.status == 200 and model.selected == 0, "the newest seed is open")

  -- the URL as sent
  model.params = { { on = true, key = "q", value = "olive oil" }, { on = false, key = "skip", value = "1" }, { on = true, key = "", value = "x" } }
  model.url = "{{base}}/api/products"
  check(full_url(model) == BASES["local"] .. "/api/products?q=olive%20oil", "ticked, named params only, encoded: " .. full_url(model))
  check(send(), "products answered")
  local r = model.response or {}
  check(r.status == 200 and r.json.count == 1 and r.json.data[1].sku == "PAN-0455", "the search found the olive oil")
  local names = {}
  for _, s in ipairs(r.spans) do names[#names + 1] = s.name end
  check(table.concat(names, " "):find("connect + send server auth cache render", 1, true) ~= nil, "the timing's phases: " .. table.concat(names, " "))
  check(model.history[1].url == "{{base}}/api/products" and #model.history == 8, "the send went to the top of the history")

  -- without the token: 401
  update({ kind = "url", change = { text = "{{base}}/api/orders" } })
  model.params = {}
  update({ kind = "row_on", list = "headers", i = 2, toggle = { checked = false } })
  send()
  check(model.response.status == 401 and model.response.json.error == "unauthorized", "no bearer token, no orders")
  update({ kind = "row_on", list = "headers", i = 2, toggle = { checked = true } })

  -- a rejected and a refused order
  update({ kind = "method", change = { value = "POST" } })
  update({ kind = "body", change = { text = '{ "customer": "cus_1042", "items": [] }' } })
  send()
  check(model.response.status == 422 and model.response.json.fields.items == "must not be empty", "an empty order is unprocessable")
  update({ kind = "method", change = { value = "DELETE" } })
  update({ kind = "url", change = { text = "{{base}}/api/orders/ord_8F2W7" } })
  send()
  check(model.response.status == 409 and model.response.json.message:find("delivered", 1, true), "a delivered order cannot be cancelled")
  update({ kind = "url", change = { text = "{{base}}/api/orders/ord_8F3JX" } })
  send()
  check(model.response.status == 204 and model.response.body == "", "an open one can: 204, no body")

  -- an environment that is down
  update({ kind = "method", change = { value = "GET" } })
  update({ kind = "env", change = { value = "staging" } })
  send()
  check(model.error ~= nil and model.response == nil and model.error.message:lower():find("refused", 1, true), "staging is down: " .. tostring(model.error and model.error.message))
  update({ kind = "env", change = { value = "local" } })

  -- the history: filtered, and a pick loads a copy
  update({ kind = "query", change = { text = "customers" } })
  check(#visible_history(model) == 1, "the search leaves the customer lookup")
  update({ kind = "pick", select = { row = 0 } })
  check(model.url == "{{base}}/api/customers/cus_9999" and model.response.status == 404, "a pick loads the request and its answer")
  update({ kind = "row_add", list = "params" })
  update({ kind = "row_key", list = "params", i = 1, change = { text = "expand" } })
  check(#model.params == 1 and #visible_history(model)[1].params == 0, "editing a loaded request leaves the history as it was")
  update({ kind = "row_remove", list = "params", i = 1 })
  check(#model.params == 0, "a row removed")
  update({ kind = "query", change = { text = "" } })

  -- curl
  local c = curl(model)
  check(c:find("curl -X GET '" .. BASES["local"] .. "/api/customers/cus_9999'", 1, true) and c:find("-H 'Authorization: Bearer", 1, true)
    and not c:find("X-Debug-Trace", 1, true), "copy as curl: the URL and the ticked headers")

  update({ kind = "clear_history" })
  check(#model.history == 0 and model.selected == nil, "the history cleared")
  check(model.hits == 12, "the mock counted its requests: " .. model.hits)

  print(("%s: %d failure(s)"):format(failed == 0 and "PASS" or "FAIL", failed))
  kui.exit(failed)
end)
