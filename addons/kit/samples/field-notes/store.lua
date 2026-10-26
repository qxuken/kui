-- Field Notes' storage: one sqlite file, notes and their checklist items.
-- Every call waits, so each runs in a task (`kui.spawn`); the app keeps
-- the notes in memory and writes through.

local M = {}

local SCHEMA = {
  [[CREATE TABLE IF NOT EXISTS notes (
      id INTEGER PRIMARY KEY,
      title TEXT NOT NULL,
      body TEXT NOT NULL DEFAULT '',
      notebook TEXT NOT NULL,
      pinned INTEGER NOT NULL DEFAULT 0,
      created REAL NOT NULL,
      updated REAL NOT NULL)]],
  [[CREATE TABLE IF NOT EXISTS items (
      id INTEGER PRIMARY KEY,
      note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
      text TEXT NOT NULL,
      done INTEGER NOT NULL DEFAULT 0,
      pos INTEGER NOT NULL)]],
  "CREATE INDEX IF NOT EXISTS items_note ON items(note, pos)",
  "PRAGMA foreign_keys = ON",
}

---@type kui.Db opened by M.open
local db = nil

-- Opens the file (made with its folder when new), creating the tables;
-- seeds it from `seed` when it holds no notes. Returns every note, each
-- with its `items`, most recently updated first.
function M.open(path, seed)
  if path ~= ":memory:" then
    local dir = path:match("^(.*)/[^/]+$")
    if dir and not kui.fs.exists(dir) then kui.fs.mkdir(dir) end
  end
  db = kui.sqlite.open(path)
  for _, sql in ipairs(SCHEMA) do db:exec(sql) end
  local n = db:first("SELECT count(*) AS n FROM notes").n
  if n == 0 and seed then
    db:transaction(function(d)
      for _, s in ipairs(seed) do
        local id = d:exec("INSERT INTO notes (title, body, notebook, pinned, created, updated) VALUES (:title, :body, :notebook, :pinned, :created, :updated)",
          { title = s.title, body = s.body, notebook = s.notebook, pinned = s.pinned and 1 or 0, created = s.created, updated = s.updated }).last_id
        for i, it in ipairs(s.items or {}) do
          d:exec("INSERT INTO items (note, text, done, pos) VALUES (:note, :text, :done, :pos)",
            { note = id, text = it[1], done = it[2] and 1 or 0, pos = i })
        end
      end
    end)
  end
  return M.all()
end

function M.all()
  local notes, by_id = {}, {}
  for _, r in ipairs(db:query("SELECT * FROM notes ORDER BY updated DESC")) do
    local note = { id = r.id, title = r.title, body = r.body, notebook = r.notebook, pinned = r.pinned == 1,
      created = r.created, updated = r.updated, items = {} }
    notes[#notes + 1] = note
    by_id[r.id] = note
  end
  for _, r in ipairs(db:query("SELECT * FROM items ORDER BY note, pos")) do
    local note = by_id[r.note]
    if note then note.items[#note.items + 1] = { id = r.id, text = r.text, done = r.done == 1 } end
  end
  return notes
end

function M.create(note)
  note.id = db:exec("INSERT INTO notes (title, body, notebook, pinned, created, updated) VALUES (:title, :body, :notebook, :pinned, :created, :updated)",
    { title = note.title, body = note.body, notebook = note.notebook, pinned = note.pinned and 1 or 0, created = note.created, updated = note.updated }).last_id
  return note.id
end

function M.save(note)
  db:exec("UPDATE notes SET title = :title, body = :body, notebook = :notebook, pinned = :pinned, updated = :updated WHERE id = :id",
    { id = note.id, title = note.title, body = note.body, notebook = note.notebook, pinned = note.pinned and 1 or 0, updated = note.updated })
  db:transaction(function(d)
    d:exec("DELETE FROM items WHERE note = :id", { id = note.id })
    for i, it in ipairs(note.items) do
      it.id = d:exec("INSERT INTO items (note, text, done, pos) VALUES (:note, :text, :done, :pos)",
        { note = note.id, text = it.text, done = it.done and 1 or 0, pos = i }).last_id
    end
  end)
end

function M.delete(id)
  db:exec("DELETE FROM items WHERE note = :id", { id = id })
  return db:exec("DELETE FROM notes WHERE id = :id", { id = id }).changes
end

function M.count()
  return db:first("SELECT count(*) AS n FROM notes").n
end

return M
