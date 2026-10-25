-- Field Notes end to end, without a window: a fresh sqlite file in the
-- temp folder, seeded, edited through `update`, read back from disk.
--
--   kui run test --headless          (from the sample's folder)
--
-- The exit code is the number of failures.

package.path = kui.dir .. "/../?.lua;" .. package.path
local path = os.tmpname() .. "-field-notes.db"
kui.args = { "--db", path }
require("main")
model = init()
local store = require("store")

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

local function titles()
  local out = {}
  for i, n in ipairs(visible(model)) do out[i] = n.title end
  return out
end

local function saved()
  local before = model.saved_at
  return wait(function() return not model.saving and model.saved_at ~= before end, 3000)
end

kui.spawn(function()
  check(wait(function() return not model.loading end), "the notebook opened")
  check(#model.notes == 12 and store.count() == 12, "a new file is seeded with 12 notes")
  local t = titles()
  check(t[1] == "Q4 planning — checkout reliability" and t[2] == "Groceries for Saturday", "pinned first, then the newest")
  check(find(model, model.selected).title == t[1], "the first note is open")

  -- notebooks and search
  update({ kind = "nav", notebook = "home" })
  check(#titles() == 3 and find(model, model.selected).notebook == "home", "Home: 3 notes, one of them open")
  update({ kind = "query", change = { text = "radiator" } })
  check(#titles() == 1 and titles()[1] == "Flat: before winter", "the search reads checklist items")
  update({ kind = "query", change = { text = "" } })
  update({ kind = "nav", notebook = "pinned" })
  check(#titles() == 2, "two pinned")
  update({ kind = "nav", notebook = "all" })
  update({ kind = "sort", change = { value = "title" } })
  check(titles()[1] == "1:1 with Ada — 22 Oct" and titles()[12] == "Tomatoes that did well this year", "A–Z")
  update({ kind = "sort", change = { value = "recent" } })

  -- editing saves itself
  update({ kind = "pick", select = { row = 2 } })
  local note = find(model, model.selected)
  check(note.title == "Postmortem draft: INC-2291 inventory sync", "picked the postmortem")
  update({ kind = "title", change = { text = "Postmortem: INC-2291 inventory sync" } })
  update({ kind = "body", change = { text = note.body .. "\n\nAction: probe a different pod when half-open." } })
  check(saved(), "the edit was saved half a second later")
  local disk = nil
  for _, n in ipairs(store.all()) do if n.id == note.id then disk = n end end
  check(disk and disk.title == "Postmortem: INC-2291 inventory sync" and disk.body:find("different pod", 1, true), "the file has the new title and body")
  check(visible(model)[1] ~= note and visible(model)[3] == note, "pinned notes still lead; the edit is the newest of the rest")

  -- the checklist
  update({ kind = "item", i = 2, toggle = { checked = true } })
  update({ kind = "new_item", change = { text = "  Write the timeline up  " } })
  update({ kind = "add_item" })
  check(#note.items == 3 and note.items[3].text == "Write the timeline up" and model.new_item == "", "an item added, trimmed, the field cleared")
  update({ kind = "add_item", submit = { text = "   " } })
  check(#note.items == 3, "a blank item is not added")
  update({ kind = "item_remove", i = 1 })
  check(#note.items == 2 and note.items[1].done, "an item removed")
  check(saved(), "the checklist saved")
  for _, n in ipairs(store.all()) do if n.id == note.id then disk = n end end
  check(#disk.items == 2 and disk.items[2].text == "Write the timeline up", "the file has the checklist")

  -- pin, move
  update({ kind = "pin", toggle = { on = true } })
  update({ kind = "move", change = { value = "reading" } })
  check(note.pinned and note.notebook == "reading" and visible(model)[1] == note, "pinned and moved: it leads now")
  check(saved(), "the pin and the move saved")

  -- a new note lands in the notebook in view
  update({ kind = "nav", notebook = "garden" })
  update({ kind = "new" })
  check(wait(function() return find(model, model.selected).title == "Untitled" end), "a new note opens")
  local fresh = find(model, model.selected)
  check(wait(function() return model.focus_title == 1 end) and fresh.notebook == "garden" and fresh.id ~= nil, "in Garden, saved, its title focused")
  check(store.count() == 13, "13 notes on disk")


  -- deleting asks first
  update({ kind = "delete" })
  check(model.confirm == fresh.id, "delete asks")
  update({ kind = "cancel" })
  check(model.confirm == nil and find(model, fresh.id), "kept")
  update({ kind = "delete" })
  update({ kind = "confirm_delete" })
  check(not find(model, fresh.id) and model.toast and model.toast.message == "Deleted “Untitled”.", "deleted, said in a toast")
  check(model.selected ~= nil and find(model, model.selected).notebook == "garden", "the next garden note opens")
  check(wait(function() return store.count() == 12 end), "12 on disk again")

  -- the file again, as the next run would read it
  local again = store.open(path)
  local ok = false
  for _, n in ipairs(again) do ok = ok or (n.title == "Postmortem: INC-2291 inventory sync" and n.pinned and n.notebook == "reading") end
  if not ok then for _, n in ipairs(again) do print("  ", n.title, n.pinned, n.notebook) end end
  check(#again == 12 and ok, "reopened: the edits are there, nothing seeded twice")

  for _, s in ipairs({ "neutral", "phosphor", "ledger" }) do update({ kind = "skin", change = { value = s } }) end
  kui.sleep(150)
  os.remove(path)
  print(("%s: %d failure(s)"):format(failed == 0 and "PASS" or "FAIL", failed))
  kui.exit(failed)
end)
