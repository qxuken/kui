---
status: proposed
date: 2026-10-26
---

# A grid, and composites that reach past their window

> **Proposed, and built as proposed (2026-10-26); backlog C89 and C90.**
> Both came from Kit's fifth sample round: its list and table are key
> sinks with index-based selection and `reveal`, because a `list`
> composite's arrows stop at a virtualized list's window, and a table is
> rows × cells, which ADR 0007's one-dimensional composites cannot walk.
> The open questions are at the end; none blocks what is built.

## Context

- ADR 0007 derives a composite from a container role whose items are
  focusable — `radioGroup`/`radio`, `tabList`/`tab`, `menu`/`menuItem`,
  `list`/`listItem` — makes it one Tab stop, and moves focus inside it
  with the arrows, Home / End and type-ahead. It walks the items **in the
  tree**. Its decision 8 already names the limit: "a list can be
  windowed. With 4,000 rows and 40 in the tree, the last item is not the
  last row." Its follow-ups name the grid: "a `grid` / `row` / `cell`
  vocabulary".
- What a windowed list already tells the core: each built row carries
  its data `index` (`Ui::open_indexed`, the `index` prop, `Tree::indexed`)
  and the list says how many rows it has, built or not (`rowCount`,
  `Tree::row_counts`, which Select All in a virtual list already reads).
  `onRange` (ADR 0065) tells the app which rows are in view; the app
  builds the rows its scroll offset shows (Kit's `body`, `uniform_list`).
- So "asking for the row" needs no new event. Scrolling the list *is*
  asking: the next view builds the rows the scroller shows, and `onRange`
  tells a pager which rows to fetch.

## Decision

1. **A `grid` is a composite of `cell`s in `row`s.** Three roles are
   appended at the tail (ADR 0006): `grid`, `row`, `cell` — AccessKit
   `Grid`, `Row`, `GridCell`; C `KUI_ROLE_GRID`, `KUI_ROLE_ROW`,
   `KUI_ROLE_CELL`. The pair is `(grid, cell)` in `composite::PAIRS`, so
   everything ADR 0007 derives holds: a grid with a focusable cell is one
   Tab stop, entered on the focused cell, else `initial_focus`, else
   `selected`, else `checked`, else the first; a focusable node inside a
   cell warns `focusable-inside-item`. A cell's row is the nearest `row`
   between it and the grid; a cell in no row is in the grid's one row.
   A cell is focusable where it says `focusable`, as a `listItem` is; a
   `row` never is. A table that selects whole rows stays a `list` of
   `listItem`s — the grid is for moving between cells.
2. **The grid's keys.** Left / Right move along the row, Up / Down to the
   row before or after keeping the column (onto the live cell nearest
   it, the earlier on a tie, so a disabled cell keeps its column and is
   stepped past), Home / End to the row's ends, and with Ctrl (or the
   primary) held to the grid's first and last cell. Page Up / Page Down
   move by the rows the scroller around them shows. A grid clamps at
   every edge (it is a list of rows, windowed like one), and moving never
   activates a cell; Enter and Space do, as on any item. Type-ahead
   searches its cells' names, as ADR 0007's does for any composite.
3. **A `list` pages too.** Page Up / Page Down in a `list` composite move
   by the rows its scroller shows (at least one); the choice composites
   (`radioGroup`, `tabList`, `menu`) have no pages.
4. **A composite whose rows carry their data index under a `rowCount` is
   windowed.** A list's unit is its item; a grid's is its row. When the
   unit has an `index` and a `rowCount` is declared from it up to the
   container (on the list, or on the scroller between, where a virtual
   list's helper puts it), a step is computed by index over
   `0..rowCount` rather than over the built items: End is row 3,999, not
   row 40.
5. **A step to a row the frame has not built reaches for it.** The focus
   shows, the scroller moves toward where the row would be — estimated
   from the pitch the built rows are laid at, which is exact for the
   uniform rows a virtual list draws — and a frame is owed. When that
   frame finishes, the row with that index (for a grid, its cell nearest
   the kept column) takes focus, as a landed arrow would. A row still not
   built (non-uniform rows, the estimate short) is reached for again from
   the rows that frame built, three times at most, then let go. Focus
   moving any other way — a click, a Tab, a landed step — lets the reach
   go. A key pressed again before the frame steps from the index being
   reached for, so a held arrow keeps going.
6. **No new event, no new prop.** Scrolling is the ask: the view builds
   what the scroller shows, and an app that pages its data hears
   `onRange` as it always did. The windowed composite needs only what a
   virtual list already declares.

## Considered options

- **An event asking the app for the row** (`{need: index}`). Rejected:
  the app would answer by scrolling — the only way a virtual list
  changes what it builds — so the core doing the scroll is the same
  thing with one round trip fewer, and every binding's app would have to
  write the answer. `onRange` still tells a pager what to fetch.
- **Moving focus to a key the frame has not built** (the row's
  `Key::index` is known before it exists). Rejected: a focus on a node
  the tree lacks is a state every other part of focus treats as "gone"
  (regions fall back, a modal restores past it), and the ring, the
  access tree and the edit store would all have to learn an exception.
- **`aria-activedescendant`-style virtual focus.** Rejected for the
  reason ADR 0007 gave: kui moves real focus.
- **A grid whose items are its rows when no cell is focusable.**
  Rejected: that is a `list`, which already exists and is what Kit's
  row-selecting table should be.
- **Wrapping in a grid** (Right at a row's end to the next row's first).
  Rejected: ARIA's grid does not wrap by default, and a windowed grid
  has no last row in the tree to wrap to (ADR 0007, decision 8).

## Consequences

- Kit can drop its key-sink list and table: a `list` of focusable
  `listItem`s with `selected = is_focused(row)` (or the `on_focus` of a
  row moving its selection), under the `rowCount` its `body` already
  declares, walks all of its rows; a table that moves between cells is
  a `grid` of `row`s of focusable `cell`s. Its `reveal` and `hand_on`
  (a sink owning Tab) go with the sink.
- `Role::ALL` is 30, `ROLES` gains three names, C three constants; Odin,
  Lua, Node and `docs/props.md` pick them up from the schema.
- Per keystroke: one walk of the composite's items (as ADR 0007), plus
  for a grid a pass grouping cells by row. Per frame: one check of a
  pending reach, which is nothing when there is none.

## Open questions

1. **Row and column for assistive technology.** A cell reports no
   position: "7 of 12" across rows would be wrong, and the access node
   has no row / column index yet. AccessKit has `row_index`,
   `column_index`, `row_count` and `column_count`; wiring them (and the
   windowed `rowCount` as the grid's row count, a row's `index` as its
   row index) is the next step, and the same fields would let a windowed
   `list` say "row 120 of 4,000" without a `label`.
2. **Type-ahead in a windowed list** searches the built rows only. The
   core cannot see the names of rows it has not built; a search event to
   the app is possible and not built.
3. **The entry of a windowed composite** is its focused, `initial_focus`,
   `selected` or first *built* item. A `selected` row scrolled out of the
   window is not entered on; Tab lands on the first row in view.
4. **Non-uniform rows** converge in up to three frames and can miss on
   a very uneven list. A row-height hint, or the app answering a reach,
   would close it; nothing in the repository needs it yet.
5. **Status.** Proposed rather than accepted because it was written and
   built in one sitting without a grilling round; the user decides.
