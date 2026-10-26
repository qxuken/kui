-- Field Notes' window: notebooks on the left, the notes in the middle, the
-- open note on the right — its title, notebook, pin, body and checklist.

local kit = require("kit.ui")

local SKINS = {
  { value = "ledger", label = "Ledger" }, { value = "neutral", label = "Neutral" }, { value = "phosphor", label = "Phosphor" },
}

local function label(s, o)
  local t = { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", max_lines = 1, ellipsis = true }
  for k, v in pairs(o or {}) do t[k] = v end
  return text(s, t)
end

local function when(at)
  local now = kui.now()
  local day = os.date("%Y-%m-%d", math.floor(at / 1000))
  if day == os.date("%Y-%m-%d", math.floor(now / 1000)) then return "today " .. os.date("%H:%M", math.floor(at / 1000)) end
  if day == os.date("%Y-%m-%d", math.floor((now - 86400000) / 1000)) then return "yesterday" end
  return (tostring(os.date("%b %d", math.floor(at / 1000))):gsub(" 0", " "))
end

local function done_of(n)
  local d = 0
  for _, it in ipairs(n.items) do if it.done then d = d + 1 end end
  return d
end

-- ---------- notebooks ----------

local function nav_row(m, value, title, count)
  local here = m.notebook == value
  return row {
    key = "nav." .. value, role = "tab", label = title, selected = here,
    width = "grow", height = "$kitCtl", pad = { x = 10 }, gap = 8, cross_align = "center",
    radius = "$kitRadius", bg = here and "$kitSel" or nil, hover_bg = here and "$kitSel" or "$kitHover", cursor = "pointer",
    on_click = { kind = "nav", notebook = value },
    text(title, { size = "$kitSize", family = "$kitUi", bold = here, color = here and "$kitSelFg" or "$kitFg" }),
    row { width = "grow" },
    text(tostring(count), { size = "$kitSizeLabel", family = "$kitData", color = here and "$kitSelFg" or "$kitMuted" }),
  }
end

local function sidebar(m)
  local counts, pinned = {}, 0
  for _, n in ipairs(m.notes) do
    counts[n.notebook] = (counts[n.notebook] or 0) + 1
    if n.pinned then pinned = pinned + 1 end
  end
  local books = {}
  for _, b in ipairs(NOTEBOOKS) do books[#books + 1] = nav_row(m, b.value, b.label, counts[b.value] or 0) end
  return column { key = "sidebar", width = "grow", height = "grow", bg = "$kitSide", pad = 8, gap = 2,
    column { role = "tabList", width = "grow", gap = 2,
      nav_row(m, "all", "All notes", #m.notes),
      nav_row(m, "pinned", "Pinned", pinned),
    },
    row { width = "grow", pad = { x = 10, t = 14, b = 4 }, label("NOTEBOOKS", { size = 10, bold = true }) },
    column { role = "tabList", width = "grow", gap = 2, table.unpack(books) },
    column { width = "grow", height = "grow" },
    row { width = "grow", pad = { x = 10, y = 4 }, tooltip = boot.db,
      label(boot.db == ":memory:" and "in memory: nothing is kept" or boot.db:match("[^/]+$"), { family = "$kitData", size = 10 }) },
  }
end

-- ---------- the list ----------

local list_ch = nil
local function list_channel(notes)
  local title, detail = {}, {}
  for i, n in ipairs(notes) do
    title[i] = (n.pinned and "★ " or "") .. n.title
    local first = n.body:match("^%s*([^\n]+)")
    local d = when(n.updated)
    if #n.items > 0 then d = d .. string.format(" · %d/%d done", done_of(n), #n.items) end
    detail[i] = d .. (first and (" · " .. first) or "")
  end
  local cols = { title = title, detail = detail }
  list_ch = list_ch and list_ch:publish(cols) or channel(cols)
  return list_ch
end

local function list_pane(m)
  local notes = visible(m)
  local selected = nil
  for i, n in ipairs(notes) do if n.id == m.selected then selected = i - 1 end end
  local body
  if m.loading then
    body = column { width = "grow", pad = 24, kit.progress({ label = "Opening the notebook…" }) }
  elseif m.error then
    body = kit.empty({ title = "No notes", detail = "The notes file did not open." })
  elseif #notes == 0 and m.query ~= "" then
    body = kit.empty({ title = "No notes with “" .. m.query .. "”", detail = "The search reads titles, bodies and checklist items." },
      { kit.button({ label = "Clear the search", on_press = { kind = "query", change = { text = "" } } }) })
  elseif #notes == 0 then
    body = kit.empty({ title = m.notebook == "pinned" and "Nothing pinned" or "No notes here yet",
      detail = m.notebook == "pinned" and "Pin a note to keep it at the top." or "Notes you write in this notebook land here." },
      { m.notebook ~= "pinned" and kit.button({ label = "New note", icon = "plus", variant = "primary", on_press = { kind = "new" } }) or nil })
  else
    body = kit.list({ key = "notes", rows = list_channel(notes), title = "title", detail = "detail", selected = selected,
      grow = true, on_select = { kind = "pick" } })
  end
  return column { key = "list-pane", width = "grow", height = "grow",
    row { key = "list-head", width = "grow", height = 40, pad = { x = 10 }, gap = 8, cross_align = "center",
      label(m.loading and "…" or (#notes .. (#notes == 1 and " note" or " notes")), { color = "$kitFg" }),
      row { width = "grow" },
      kit.segmented({ key = "sort", label = "Sort the notes", value = m.sort, on_change = { kind = "sort" }, options = {
        { value = "recent", label = "Recent" }, { value = "title", label = "A–Z" } } }),
    },
    kit.divider({}),
    column { key = "list-body", width = "grow", height = "grow", body },
  }
end

-- ---------- the open note ----------

local function checklist(m, n)
  local done = done_of(n)
  -- One Tab stop: Up and Down walk the items, Space ticks, an × removes.
  local items = {}
  for i, it in ipairs(n.items) do items[i] = { label = it.text, checked = it.done } end
  -- Folded as the model says, and the heading asks to change it: it was
  -- handed `collapsed = false` and no toggle, so pressing it did nothing.
  return kit.section({ key = "checklist", title = "Checklist", count = #n.items, collapsed = m.checklist_folded,
    on_toggle = { kind = "fold_checklist" } }, {
    column { width = "grow", gap = 6, pad = { t = 4 },
      #n.items > 0 and kit.progress({ key = "done", value = done / #n.items, label = string.format("%d of %d done", done, #n.items) }) or nil,
      #items > 0 and kit.checklist({ key = "items", label = "Checklist", items = items, removable = true,
        on_toggle = { kind = "item" }, on_remove = { kind = "item_remove" } }) or nil,
      row { key = "add", width = "grow", gap = 8, cross_align = "end",
        kit.text_field({ key = "new-item", label = "Add an item", hide_label = true, placeholder = "Add an item, then Enter",
          value = m.new_item, on_change = { kind = "new_item" }, on_submit = { kind = "add_item" } }),
        kit.button({ key = "add-btn", label = "Add", icon = "plus", disabled = m.new_item:match("^%s*$") ~= nil, on_press = { kind = "add_item" } }),
      },
    },
  })
end

local function editor(m)
  local n = m.selected and find(m, m.selected)
  if m.error then
    return column { key = "editor", width = "grow", height = "grow", pad = 16, gap = 12,
      kit.notice({ tone = "danger", title = "Could not open the notes file",
        message = (m.error:match("^[^\n]*"):gsub("^runtime error: ", "")) .. ". Pick another file with --db, or try again." },
        { kit.button({ label = "Try again", icon = "refresh", on_press = { kind = "retry" } }) }),
      kit.inspector({ key_width = 90, entries = { { key = "File", value = boot.db, mono = true } } }),
    }
  end
  if not n then
    return column { key = "editor", width = "grow", height = "grow", main_align = "center",
      kit.empty({ title = m.loading and "Opening…" or "No note open", detail = not m.loading and "Pick a note, or start a new one." or nil },
        { not m.loading and kit.button({ label = "New note", icon = "plus", on_press = { kind = "new" } }) or nil }) }
  end
  return column { key = "editor." .. n.id, width = "grow", height = "grow", scroll_y = true, pad = 16, gap = 12,
    kit.text_field({ key = "title", label = "Title", hide_label = true, value = n.title, autofocus = n.id == m.fresh, placeholder = "Untitled",
      on_change = { kind = "title" } }),
    row { key = "meta", width = "grow", gap = 10, cross_align = "center",
      row { width = 140, kit.select({ key = "book", options = NOTEBOOKS, value = n.notebook, on_change = { kind = "move" } }) },
      kit.switch({ key = "pin", label = "Pinned", on = n.pinned, on_toggle = { kind = "pin" } }),
      row { width = "grow" },
      label("edited " .. when(n.updated) .. " · created " .. when(n.created)),
      kit.icon_button({ key = "delete", icon = "trash", label = "Delete this note", variant = "danger", on_press = { kind = "delete" } }),
    },
    kit.text_area({ key = "body", label = "Note", hide_label = true, value = n.body, placeholder = "Write something…", rows = 8,
      -- Prose: Tab moves on to the checklist, as from a field.
      tab_moves_focus = true,
      on_change = { kind = "body" } }),
    checklist(m, n),
  }
end

-- ---------- window ----------

return function(m)
  local doomed = m.confirm and find(m, m.confirm)
  local status
  if m.error then status = "not open"
  elseif m.saving then status = "saving…"
  elseif m.saved_at then status = "saved " .. os.date("%H:%M:%S", math.floor(m.saved_at / 1000))
  else status = "all changes saved" end
  return column { window_title = "Field Notes", width = "grow", height = "grow", bg = "$kitBg",
    kit.settings({ skin = m.skin }),
    kit.toolbar({ key = "top" }, {
      text("Field Notes", { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" }),
      row { key = "search", width = 280,
        kit.search({ key = "query", value = m.query, placeholder = "Search notes", label = "Search the notes", focus = m.focus_search,
          count = m.query ~= "" and (#visible(m) .. " found") or nil, on_change = { kind = "query" } }) },
      trailing = {
        kit.button({ key = "new", label = "New note", icon = "plus", variant = "primary", disabled = m.loading or m.error ~= nil,
          on_press = { kind = "new" } }),
        row { key = "skin", width = 120, kit.select({ key = "skin-select", options = SKINS, value = m.skin, on_change = { kind = "skin" } }) },
      },
    }),
    kit.split({ key = "outer", fraction = m.side, min = 160, on_resize = { kind = "side" } }, {
      first = { sidebar(m) },
      second = {
        kit.split({ key = "inner", fraction = m.mid, min = 240, on_resize = { kind = "mid" } }, {
          first = { list_pane(m) },
          second = { editor(m) },
        }),
      },
    }),
    kit.statusbar({ key = "status" }, {
      label(string.format("%d notes in %d notebooks", #m.notes, #NOTEBOOKS)),
      trailing = { label(status) },
    }),
    doomed and kit.dialog({ key = "delete-dialog", title = "Delete “" .. doomed.title .. "”?", on_dismiss = { kind = "cancel" } }, {
      text(#doomed.items > 0 and string.format("The note and its %d checklist items go for good; there is no trash.", #doomed.items)
        or "The note goes for good; there is no trash.", { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" }),
      actions = {
        kit.button({ label = "Keep it", on_press = { kind = "cancel" } }),
        kit.button({ label = "Delete", icon = "trash", variant = "danger", on_press = { kind = "confirm_delete" } }),
      },
    }) or nil,
    m.toast and kit.toast({ key = "toast", message = m.toast.message, tone = m.toast.tone, on_dismiss = { kind = "toast_off" } }) or nil,
  }
end
