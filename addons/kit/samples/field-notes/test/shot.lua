-- A picture of Field Notes: a seeded book in memory, a state staged.
--
--   kui run test/shot.lua --shot shots/field-notes.png --after 1200 --size 1180x760 -- [skin] [state]
--
-- `skin` is ledger, neutral or phosphor; `state` one of note, checklist,
-- search, delete, empty, loading, error.
package.path = kui.dir .. "/../?.lua;" .. package.path
local skin, state = kui.args[1] or "ledger", kui.args[2] or "note"
kui.args = { "--db", state == "error" and "/dev/null/nope/notes.db" or ":memory:" }
if state == "loading" then boot = { db = ":memory:", loading = true } end
require("main")
model = init()
model.skin = skin

kui.spawn(function()
  if state == "loading" or state == "error" then return end
  while model.loading do kui.sleep(10) end
  if state == "checklist" then
    update({ kind = "nav", notebook = "home" })
    update({ kind = "new_item", change = { text = "Lemons" } })
  elseif state == "search" then
    update({ kind = "query", change = { text = "inc-2291" } })
    update({ kind = "pick", select = { row = 1 } })
  elseif state == "delete" then
    update({ kind = "nav", notebook = "garden" })
    update({ kind = "delete" })
  elseif state == "empty" then
    update({ kind = "query", change = { text = "sourdough" } })
  end
end)
