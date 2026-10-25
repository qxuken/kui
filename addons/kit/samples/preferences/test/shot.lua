-- A picture of the preferences window, a state staged.
--
--   kui run test/shot.lua --shot shots/preferences.png --after 1000 --size 1040x760 -- [skin] [state]
--
-- `skin` is ledger, neutral or phosphor; `state` one of general, sync,
-- network, invalid, notifications, advanced, search, nothing, reset.
package.path = kui.dir .. "/../?.lua;" .. package.path
local skin, state = kui.args[1] or "ledger", kui.args[2] or "general"
-- a file that is never written: the shots never save
kui.args = { "--config", (kui.env.HOME or "/tmp") .. "/.config/kui-samples/tidewater-shot.toml" }
require("main")
model = init()
model.skin = skin

local function set(key, v) model.draft[key] = v end

if state == "sync" then
  update({ kind = "section", section = "sync" })
  set("desktop", true); set("versions", 60); set("mode", "upload")
elseif state == "network" then
  update({ kind = "section", section = "network" })
  set("upload_limit", 2500); set("proxy", "manual"); set("proxy_host", "proxy.larder.internal"); set("proxy_port", "8080")
elseif state == "invalid" then
  update({ kind = "section", section = "network" })
  set("proxy", "manual"); set("proxy_host", "proxy larder"); set("proxy_port", "80800")
elseif state == "notifications" then
  update({ kind = "section", section = "notifications" })
  set("notify_done", true); set("quiet_to", "7.30")
elseif state == "advanced" then
  update({ kind = "section", section = "advanced" })
  update({ kind = "show_toml" })
elseif state == "search" then
  update({ kind = "query", change = { text = "proxy" } })
elseif state == "nothing" then
  update({ kind = "query", change = { text = "bluetooth" } })
elseif state == "reset" then
  update({ kind = "section", section = "advanced" })
  set("transfers", 12); set("verbose", true); set("language", "de-DE")
  update({ kind = "reset" })
else
  set("device_name", "Maren’s MacBook Pro (work)")
  set("appearance", "dark")
end
