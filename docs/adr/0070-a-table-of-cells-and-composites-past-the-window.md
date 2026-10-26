---
status: accepted
date: 2026-10-26
---

# A table of cells, and composites that reach past their window

> **Accepted and built (2026-10-26); backlog C89 and C90.** Proposed and
> built the same morning, then settled by a grilling round with the user
> on 2026-10-26, which renamed the grid a table and added positions and
> headers for readers, type-ahead and re-entry for windowed composites,
> actionable cells and the roles a layout table implies; all of that is
> built too. What the building changed against the decisions is at the
> end, under [*What the building changed*](#what-the-building-changed).

## Context

- ADR 0007 derives a composite from a container role whose items are
  focusable — `radioGroup`/`radio`, `tabList`/`tab`, `menu`/`menuItem`,
  `list`/`listItem` — makes it one Tab stop, and moves focus inside it
  with the arrows, Home / End and type-ahead. It walks the items **in the
  tree**. Its decision 8 already names the limit: "a list can be
  windowed. With 4,000 rows and 40 in the tree, the last item is not the
  last row." Its follow-ups name the missing two-dimensional pattern.
- Kit's fifth sample round found both: its list and table are key sinks
  with index-based selection and `reveal`, because a `list` composite's
  arrows stop at a virtualized list's window, and a table is rows ×
  cells, which ADR 0007's one-dimensional composites cannot walk.
- What a windowed list already tells the core: each built row carries
  its data `index` (`Ui::open_indexed`, the `index` prop, `Tree::indexed`)
  and the list says how many rows it has, built or not (`rowCount`,
  `Tree::row_counts`). `onRange` (ADR 0065) tells the app which rows are
  in view, and the app builds the rows its scroll offset shows.
- kui already has a layout table (`NodeSpec::table`, `dir="table"`, ADR
  0033): a column whose rows' children line up in columns. It said
  nothing to a reader.

## Decision

1. **Two shapes, one word each.** A table whose arrows move whole rows
   stays a `list` of `listItem`s. The cell composite is for moving
   between cells, and it is a **`table`** of `row`s of `cell`s — "grid"
   is CSS layout, and avoided (GLOSSARY, *Table*). No row mode in this
   version.
2. **Roles.** `table`, `row`, `cell`, `columnHeader` and `rowHeader` are
   appended at the tail (ADR 0006): `Role::ALL` is 32, C
   `KUI_ROLE_TABLE`, `KUI_ROLE_ROW`, `KUI_ROLE_CELL`,
   `KUI_ROLE_COLUMN_HEADER`, `KUI_ROLE_ROW_HEADER`. A header is a cell to
   navigation (`composite::is_item`). The pair `(table, cell)` joins
   `composite::PAIRS`, so everything ADR 0007 derives holds: a table with
   a focusable cell is a composite — one Tab stop, entered by ADR 0007's
   precedence — and AccessKit's `Grid` with `GridCell`s; one with none is
   a static `Table` of `Cell`s, which still reports positions and
   headers. A cell's row is the nearest `row` under the table; a cell in
   no row is in the table's one row. A `row` is never focusable.
3. **The layout table implies the roles.** A `table()` that declares no
   role reads as a `table`, its in-flow `Row` children as `row`s and
   their in-flow children as `cell`s — a text or an image straight in a
   row too, named by its text — where nothing else made the node
   something: a clickable row is still a button, and holds no cells. It
   is derived where the access tree derives every role
   (`access::derived_role`, through `composite::implied_role`), and the
   composite walks read the same (`composite::role`), so a reader and
   the keyboard see one table.
4. **Selection is the app's.** The core moves focus only, as ADR 0007
   decided for every composite. A Shift+arrow range event is backlog
   C101.
5. **The keys.** Left / Right along the row; Up / Down to the row before
   or after, keeping the column (onto the live cell nearest it, the
   earlier on a tie); Home / End to the row's ends, with Ctrl (or the
   primary) the table's first and last cell; Page Up / Page Down by the
   rows the scroller shows. A table clamps at every edge, and moving
   never activates a cell. A `list` pages too.
6. **Columns by index**, no spans in this version (backlog C103,
   `colSpan`).
7. **Positions for readers.** The access node carries `row_index`,
   `column_index`, `row_count` and `column_count` (AccessKit's, zero-
   based): a cell's row and column, a row's index, the table's counts. A
   windowed composite's `rowCount` is the row count and a row's `index`
   its row index, so a windowed `list` reads "row 120 of 4,000" with no
   `label` — a `list` gets the rows half, and a windowed one's "n of m"
   (`pos_in_set` / `set_size`) is the data's. A header is linked to its
   cells by column index: AccessKit has no relation property for headers,
   and the platform adapters that read tables find a cell's column header
   that way.
8. **A composite whose rows carry their data index under a `rowCount` is
   windowed.** A list's unit is its item, a table's its row. When the
   unit has an `index` and a `rowCount` is declared from it up to the
   container, a step is by index over `0..rowCount`.
9. **A step to an unbuilt row reaches for it.** The focus shows, the
   scroller moves toward where the row would be — from the pitch of the
   built rows — and a frame is owed; when that frame finishes, the row
   with that index (a table's cell nearest the kept column) takes focus.
   A row still not built is reached for again from the rows that frame
   built, three tries in all, then let go: uneven rows are accepted as
   they are. Focus moving any other way lets a reach go. No new event:
   scrolling is the ask, and `onRange` pagers work unchanged.
10. **Type-ahead in a windowed composite.** The built rows are searched
    first. A search none of them answers goes to the composite as its
    `onTypeahead` message, `typeahead: {text}`; the app searches its data
    and answers with `answer_typeahead(key, index)` (a door in every
    binding, pinned in DOORS), and the core reaches for that row as a
    step does. The answer follows kui's answered-event shape — an event,
    then a door that takes the answer once (`answer_selection_range`):
    false when the composite asked nothing, or focus has left it. A
    channel-column shortcut (`typeaheadColumn`) is backlog C102.
11. **Re-entry.** A windowed composite remembers the row (and in a table
    the column) focus last held; Tab back in lands there, reaching for it
    when the frame has scrolled it away. An optional `activeIndex` prop
    on the composite names the row the app wants entered — the selected
    one after a reload — and wins over the remembered one.
12. **Actionable cells.** Enter or F2 on a cell with focusable
    descendants moves focus to the first; Tab and Shift-Tab cycle within
    the cell; Escape returns focus to the cell, where the arrows navigate
    again. A cell with exactly one control (not an editor) presses it on
    Enter or Space without entering; F2 enters it whatever it holds.
    `focusable-inside-item` no longer fires for a cell's controls; it
    still fires inside every other item.

## Considered options

- **An event asking the app for an unbuilt row** (`{need: index}`).
  Rejected: the app would answer by scrolling — the only way a virtual
  list changes what it builds — so the core doing the scroll is the same
  thing with one round trip fewer.
- **Moving focus to a key the frame has not built.** Rejected: a focus on
  a node the tree lacks is one every other part of focus treats as gone
  (regions fall back, a modal restores past it).
- **`aria-activedescendant`-style virtual focus.** Rejected for ADR
  0007's reason: kui moves real focus.
- **A row mode for the cell table.** Rejected: that is a `list`, which
  already exists and is what a row-selecting table should be.
- **Wrapping.** Rejected: ARIA's grid does not wrap, and a windowed table
  has no last row in the tree to wrap to (ADR 0007, decision 8).
- **Calling it `grid`.** Rejected in the grilling round: "grid" is CSS's
  layout word, and kui's layout table is already a table.
- **Header relations by key** (a cell naming its header). Rejected:
  AccessKit carries no such relation; the column index is what the
  adapters read.

## Consequences

- Kit can drop its key-sink list and table: a `list` of focusable
  `listItem`s under the `rowCount` its body already declares walks all
  of its rows, and a `cells = true` table becomes a `table` of focusable
  `cell`s. `reveal` and `hand_on` go with the sink; `onTypeahead` and
  `activeIndex` cover its search and its reload.
- A layout table in any app now has a table, rows and cells in its
  access tree where it had loose texts: the corpus's `table` scene says
  so in all five adapters.
- Per keystroke: one walk of the composite's items (as ADR 0007), plus a
  pass grouping a table's cells by row. Per frame: a pending reach's
  check, nothing without one; the access build's position pass, one walk
  per table or list.

## Open questions

None left open by the grilling round: positions and headers, type-ahead
and re-entry are built; uneven rows are accepted with the three-try
reach. The Shift range (C101), `typeaheadColumn` (C102) and `colSpan`
(C103) are backlog entries.

## What the building changed

- **A text or an image in a layout table's row is a cell.** Decision 3
  said a row's children become cells; the building makes that true of
  its text and image leaves too, which the layout counts as cells, and
  names such a cell by its text (a cell is not presentational, so a
  container cell's text stays a child of it).
- **Implied roles yield to what a node already is.** A row or cell with
  a click, a scroller, a modal or chrome keeps that role; a focusable or
  key-sink one becomes the cell (it would otherwise be a `group`).
- **`cells_focus`.** Which AccessKit role a table and its cells take is
  carried on the access node (`cells_focus`; C `KUI_ACCESS_CELLS_FOCUS`)
  rather than worked out by each bridge.
- **`activeIndex` is a message-kind prop** carrying a whole number, so an
  index past 2^24 stays exact and C passes `NULL` for none
  (`KuiSpec.active_index`).
- **A hidden node (ADR 0069) is no item**: not counted, not stepped onto.
