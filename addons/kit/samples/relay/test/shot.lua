-- A picture of Relay at work: the history seeded, a response open.
--
--   kui run test/shot.lua --shot shots/relay.png --after 1500 --size 1240x800 -- [skin] [state]
--
-- `skin` is ledger, neutral or phosphor; `state` a response tab (body,
-- headers, timing) or one of post, badjson, sending, error, empty.
package.path = kui.dir .. "/../?.lua;" .. package.path
local skin, state = kui.args[1] or "ledger", kui.args[2] or "body"
kui.args = { "--port", "0" }
require("main")
model = init()
model.skin = skin

kui.spawn(function()
  while #model.history == 0 do kui.sleep(10) end
  if state == "empty" then
    model.history, model.response, model.selected = {}, nil, nil
    model.method, model.url, model.params = "GET", "{{base}}/api/products", { { on = true, key = "q", value = "butter" } }
  elseif state == "error" then
    update({ kind = "env", change = { value = "staging" } })
    update({ kind = "send" })
  elseif state == "sending" then
    -- staged, not sent: the picture must not race the answer
    update({ kind = "pick", select = { row = 6 } })
    model.sending = true
  elseif state == "post" or state == "badjson" then
    update({ kind = "pick", select = { row = 3 } })
    update({ kind = "req_tab", change = { value = "body" } })
    if state == "badjson" then
      update({ kind = "body", change = { text = '{\n  "customer": "cus_2210",\n  "items": [\n    { "sku": "VEG-0112", "qty": 2 },\n  ]\n' } })
    end
  else
    update({ kind = "pick", select = { row = 1 } })
    update({ kind = "res_tab", change = { value = state } })
    if state == "timing" then update({ kind = "req_tab", change = { value = "headers" } }) end
  end
end)
