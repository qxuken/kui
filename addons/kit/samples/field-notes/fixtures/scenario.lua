-- App-specific assertions; the runner owns startup, time and cleanup.
return function()

local path=kui.args[2]

local store = require("store")


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

  assert(wait(function() return not model.loading end), "the notebook opened")
  assert(#model.notes == 12 and store.count() == 12, "a new file is seeded with 12 notes")
  local t = titles()
  assert(t[1] == "Q4 planning — checkout reliability" and t[2] == "Groceries for Saturday", "pinned first, then the newest")
  assert(find(model, model.selected).title == t[1], "the first note is open")

  -- notebooks and search
  update({ kind = "nav", notebook = "home" })
  assert(#titles() == 3 and find(model, model.selected).notebook == "home", "Home: 3 notes, one of them open")
  update({ kind = "query", change = { text = "radiator" } })
  assert(#titles() == 1 and titles()[1] == "Flat: before winter", "the search reads checklist items")
  update({ kind = "query", change = { text = "" } })
  update({ kind = "nav", notebook = "pinned" })
  assert(#titles() == 2, "two pinned")
  update({ kind = "nav", notebook = "all" })
  update({ kind = "sort", change = { value = "title" } })
  assert(titles()[1] == "1:1 with Ada — 22 Oct" and titles()[12] == "Tomatoes that did well this year", "A–Z")
  update({ kind = "sort", change = { value = "recent" } })

  -- editing saves itself
  update({ kind = "pick", select = { row = 2 } })
  local note = find(model, model.selected)
  assert(note.title == "Postmortem draft: INC-2291 inventory sync", "picked the postmortem")
  update({ kind = "title", change = { text = "Postmortem: INC-2291 inventory sync" } })
  update({ kind = "body", change = { text = note.body .. "\n\nAction: probe a different pod when half-open." } })
  assert(saved(), "the edit was saved half a second later")
  local disk = { items = {} }
  for _, n in ipairs(store.all()) do if n.id == note.id then disk = n end end
  assert(disk and disk.title == "Postmortem: INC-2291 inventory sync" and disk.body:find("different pod", 1, true), "the file has the new title and body")
  assert(visible(model)[1] ~= note and visible(model)[3] == note, "pinned notes still lead; the edit is the newest of the rest")

  -- the checklist
  update({ kind = "item", toggle = { index = 1, checked = true } })
  update({ kind = "new_item", change = { text = "  Write the timeline up  " } })
  update({ kind = "add_item" })
  assert(#note.items == 3 and note.items[3].text == "Write the timeline up" and model.new_item == "", "an item added, trimmed, the field cleared")
  update({ kind = "add_item", submit = { text = "   " } })
  assert(#note.items == 3, "a blank item is not added")
  update({ kind = "item_remove", remove = { index = 0 } })
  assert(#note.items == 2 and note.items[1].done, "an item removed")
  assert(saved(), "the checklist saved")
  for _, n in ipairs(store.all()) do if n.id == note.id then disk = n end end
  assert(#disk.items == 2 and disk.items[2].text == "Write the timeline up", "the file has the checklist")

  -- pin, move
  update({ kind = "pin", toggle = { on = true } })
  update({ kind = "move", change = { value = "reading" } })
  assert(note.pinned and note.notebook == "reading" and visible(model)[1] == note, "pinned and moved: it leads now")
  assert(saved(), "the pin and the move saved")

  -- a new note lands in the notebook in view
  update({ kind = "nav", notebook = "garden" })
  update({ kind = "new" })
  assert(wait(function() return find(model, model.selected).title == "Untitled" end), "a new note opens")
  local fresh = find(model, model.selected)
  assert(wait(function() return model.fresh ~= nil end) and model.fresh == fresh.id and fresh.notebook == "garden", "in Garden, saved, its title asks for the keyboard")
  assert(store.count() == 13, "13 notes on disk")


  -- deleting asks first
  update({ kind = "delete" })
  assert(model.confirm == fresh.id, "delete asks")
  update({ kind = "cancel" })
  assert(model.confirm == nil and find(model, fresh.id), "kept")
  update({ kind = "delete" })
  update({ kind = "confirm_delete" })
  assert(not find(model, fresh.id) and model.toast and model.toast.message == "Deleted “Untitled”.", "deleted, said in a toast")
  assert(model.selected ~= nil and find(model, model.selected).notebook == "garden", "the next garden note opens")
  assert(wait(function() return store.count() == 12 end), "12 on disk again")

  -- the file again, as the next run would read it
  local _, again = store.open(path)
  local ok = false
  for _, n in ipairs(again) do ok = ok or (n.title == "Postmortem: INC-2291 inventory sync" and n.pinned and n.notebook == "reading") end
  if not ok then for _, n in ipairs(again) do print("  ", n.title, n.pinned, n.notebook) end end
  assert(#again == 12 and ok, "reopened: the edits are there, nothing seeded twice")

  for _, s in ipairs({ "neutral", "phosphor", "ledger" }) do update({ kind = "skin", change = { value = s } }) end
  kui.sleep(150)

end
