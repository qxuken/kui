-- Field Notes: notes with checklists in notebooks, kept in sqlite, on Kit
-- and the kui runner. Search, pin, sort, a confirm before deleting, and a
-- save that follows the typing.
--
--   kui run addons/kit/samples/field-notes
--   kui run addons/kit/samples/field-notes -- --db /tmp/notes.db
--   kui run addons/kit/samples/field-notes -- --db :memory:      (a fresh, seeded book every run)

local store = require("store")
local seed = require("seed")
local view_of = require("view")

NOTEBOOKS = {
  { value = "work", label = "Work" },
  { value = "home", label = "Home" },
  { value = "garden", label = "Garden" },
  { value = "reading", label = "Reading" },
}

if not boot then
  local opts = kui.parse_args({
    name = "field-notes",
    about = "notes and checklists in notebooks, kept in sqlite",
    options = {
      db = { default = kui.data_dir .. "/field-notes.db", value = "path",
        help = "the notes file (:memory: for a fresh, seeded one each run)" },
    },
  })
  boot = { db = opts.db }
end

function init()
  return {
    skin = "ledger",
    loading = true,
    error = nil,
    notes = {},
    notebook = "all",
    query = "",
    sort = "recent",
    selected = nil,       -- the open note's id
    side = 0.17,
    mid = 0.36,
    focus_search = 0,
    checklist_folded = false,
    new_item = "",
    confirm = nil,        -- the id a delete waits on
    toast = nil,
    saving = false,
    saved_at = nil,
  }
end

-- ---------- loading and saving ----------

local function load()
  kui.spawn(function()
    local ok, notes = pcall(store.open, boot.db, seed(kui.now()))
    kui.dispatch(ok and { kind = "loaded", notes = notes } or { kind = "load_failed", error = tostring(notes) })
  end)
end
if not boot.loading then
  boot.loading = true
  load()
end

local dirty, flushing = {}, false

-- Saves what changed half a second after the last change.
local function touch(note)
  note.updated = kui.now()
  if not next(dirty) then kui.timer.after(500, function() kui.dispatch({ kind = "flush" }) end) end
  dirty[note.id] = note
end

local function flush()
  if flushing or not next(dirty) then return end
  local batch = dirty
  dirty, flushing = {}, true
  model.saving = true
  kui.spawn(function()
    local ok, err = pcall(function() for _, note in pairs(batch) do store.save(note) end end)
    flushing = false
    kui.dispatch({ kind = "saved", ok = ok, error = err and tostring(err) })
  end)
end

-- ---------- what the view shows ----------

function find(m, id)
  for i, n in ipairs(m.notes) do if n.id == id then return n, i end end
end

local function matches(n, q)
  if q == "" then return true end
  if n.title:lower():find(q, 1, true) or n.body:lower():find(q, 1, true) then return true end
  for _, it in ipairs(n.items) do if it.text:lower():find(q, 1, true) then return true end end
  return false
end

-- The notes in the chosen notebook that the search leaves, in order.
function visible(m)
  local q = m.query:lower():match("^%s*(.-)%s*$")
  local out = {}
  for _, n in ipairs(m.notes) do
    local here = m.notebook == "all" or (m.notebook == "pinned" and n.pinned) or n.notebook == m.notebook
    if here and matches(n, q) then out[#out + 1] = n end
  end
  if m.sort == "title" then
    table.sort(out, function(a, b) return a.title:lower() < b.title:lower() end)
  else
    table.sort(out, function(a, b)
      if a.pinned ~= b.pinned then return a.pinned end
      return a.updated > b.updated
    end)
  end
  return out
end

-- ---------- update ----------

local function select_first(m)
  local v = visible(m)
  m.selected = v[1] and v[1].id or nil
end

function update(msg)
  local m = model
  local k = msg.kind
  if k == "loaded" then
    m.loading, m.notes = false, msg.notes
    if not m.selected then select_first(m) end
  elseif k == "load_failed" then
    m.loading, m.error = false, msg.error
  elseif k == "retry" then
    m.loading, m.error = true, nil
    load()
  elseif k == "nav" then
    m.notebook = msg.notebook
    local open = find(m, m.selected)
    local still = false
    for _, n in ipairs(visible(m)) do still = still or n == open end
    if not still then select_first(m) end
  elseif k == "query" then
    m.query = msg.change.text
    select_first(m)
  elseif k == "sort" then m.sort = msg.change.value
  elseif k == "pick" then
    local n = visible(m)[msg.select.row + 1]
    if n then m.selected, m.new_item, m.fresh = n.id, "", nil end
  elseif k == "new" then
    local book = (m.notebook == "all" or m.notebook == "pinned") and "work" or m.notebook
    local now = kui.now()
    local note = { title = "Untitled", body = "", notebook = book, pinned = m.notebook == "pinned", created = now, updated = now, items = {} }
    table.insert(m.notes, 1, note)
    m.query = ""
    kui.spawn(function()
      store.create(note)
      kui.dispatch({ kind = "created", note = note })
    end)
  elseif k == "created" then
    -- The new note opens with its title taking the keyboard (`autofocus`).
    m.selected, m.fresh = msg.note.id, msg.note.id
  elseif k == "title" or k == "body" then
    local n = find(m, m.selected)
    if n then n[k] = msg.change.text; touch(n) end
  elseif k == "move" then
    local n = find(m, m.selected)
    if n then n.notebook = msg.change.value; touch(n) end
  elseif k == "pin" then
    local n = find(m, m.selected)
    if n then n.pinned = msg.toggle.on; touch(n) end
  elseif k == "item" then
    local n = find(m, m.selected)
    if n and n.items[msg.i] then n.items[msg.i].done = msg.toggle.checked; touch(n) end
  elseif k == "item_remove" then
    local n = find(m, m.selected)
    if n then table.remove(n.items, msg.i); touch(n) end
  elseif k == "new_item" then m.new_item = msg.change.text
  elseif k == "add_item" then
    local n = find(m, m.selected)
    local t = (msg.submit and msg.submit.text or m.new_item):match("^%s*(.-)%s*$")
    if n and t ~= "" then
      n.items[#n.items + 1] = { text = t, done = false }
      m.new_item = ""
      touch(n)
    end
  elseif k == "flush" then flush()
  elseif k == "saved" then
    m.saving = false
    if msg.ok then m.saved_at = kui.now() else m.toast = { message = "Could not save: " .. msg.error, tone = "danger" } end
    if next(dirty) then flush() end
  elseif k == "delete" then m.confirm = m.selected
  elseif k == "cancel" then m.confirm = nil
  elseif k == "confirm_delete" then
    local n, i = find(m, m.confirm)
    m.confirm = nil
    if n then
      local next_note = visible(m)
      table.remove(m.notes, i)
      dirty[n.id] = nil
      -- the neighbour below opens, else the one above
      local pos = 1
      for j, v in ipairs(next_note) do if v == n then pos = j end end
      local after = next_note[pos + 1] or next_note[pos - 1]
      m.selected = after and after.id or nil
      m.toast = { message = "Deleted “" .. n.title .. "”.", tone = "neutral" }
      kui.spawn(function() store.delete(n.id) end)
    end
  elseif k == "focus_search" then m.focus_search = m.focus_search + 1
  elseif k == "fold_checklist" then m.checklist_folded = msg.toggle.collapsed
  elseif k == "skin" then m.skin = msg.change.value
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "mid" then m.mid = msg.resize.fraction
  elseif k == "toast_off" then m.toast = nil
  end
end

function view(env)
  return view_of(model, env)
end

-- Unsaved edits are written before the window goes.
function on_exit()
  for _, note in pairs(dirty) do pcall(store.save, note) end
end
