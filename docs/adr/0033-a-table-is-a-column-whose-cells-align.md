---
status: accepted
date: 2026-09-17
amended: 2026-09-20
---

# A table is a column whose cells align

> **Amended 2026-09-20** by the regression pass (backlog RG3, RG7, RG8,
> RG11): decisions 2, 4 and 5 gained the sentences marked *amended*,
> and decision 9 is new. A row of a table is a `Row` child of it and
> nothing else is (a `column` section under the table had its stacked
> children taken as cells); a `Fit` table's width is its columns'
> whatever its rows' sizing (a `Fit` table of `grow` rows — the howto's
> own snippet — laid out 0 wide); a `scrollX` table's rows are at least
> as wide as its columns (a `grow` row was the table's width, so the
> overflow the table kept was clipped and `scroll_max.x` was 0); and an
> image cell's height is its aspect at its own width (a 16 px icon in a
> 200 px column was a 200 × 200 box).

> **Accepted and built (2026-09-17), the same day it was proposed** —
> raised by kawoosh's devtools tabs and by the panel's own. Every
> key/value list in either was a column of rows whose first cell was a
> box of a width picked by hand: 70 px for the Facts rows, 90 for the
> tokens and the legend, 52 for the inspector's groups, 110 for
> kawoosh's Perf tab, 180 for its Settings tab — six numbers, each the
> longest label that tab had on the day it was written, each wrong the
> day a longer one arrived (`syntax rows` overflowed 110 at 11 px mono;
> a settings path is as long as the user's dotted key). A width picked
> by hand is a width a longer label breaks, and no view should have to
> measure its labels to line them up. The answer is **a container the
> layout sizes as a table** — the nth in-flow child of every row is
> column n, and a column is as wide as its widest cell — in the five passes the
> solver already has, so it reaches every binding as one value of `dir`
> and needs no widget, no measurement in the view and no width in it.

## Context

- **What the hand-picked widths were for.** A key/value row is a row of
  two texts. Left to flex, each row's value starts where its own key
  ends, and a column of them reads as a ragged list. The fix every tab
  wrote was a box of a fixed width around the key, which lines the
  values up — at whatever width the author guessed. The `measure_text`
  door (`Ui::measure_text`, "sizing a column to its widest label")
  exists for this and none of the six used it, because measuring every
  label every frame to pick a box width is more code than the box, and
  the box works until it does not.

- **What the solver does.** Five passes over the flat preorder tree
  (`layout.rs`): fit widths, children before parents; grow widths,
  parents before children; fit heights; grow heights; positions. A
  row's children are placed one after the other by their sizes, so two
  rows agree on where a column starts exactly when their children agree
  on their widths. Alignment is therefore a *sizing* question: make the
  nth in-flow child of every row the same width and the positions
  follow.

- **When the table knows enough.** A column's fit width is the max of
  its cells' fit widths, which pass 1 has computed by the time it
  reaches the table (reverse order: the cells, then the rows, then the
  table). A column's grown width depends on the row's width, which pass
  2 has by the time it reaches the table (forward order: the table,
  then its rows). So each pass has the table's inputs at the table's
  own index, and the column widths can be resolved there and written
  down into the cells before the passes reach the rows — no extra pass,
  no second walk.

- **What a text does in pass 3.** `fit_heights` wraps a text to its
  final width and takes the wrapped measurement for *both* axes,
  because a long unbroken word may exceed the width and the truth is
  worth more than the clamp. A text held to a column wider than itself
  would shrink back to its intrinsic width there, and every cell after
  it would close up in `positions`. So a text that is a cell keeps the
  width it was given.

- **What the bindings spell.** `dir` is a custom prop with two values:
  `dir="row" | "column"` in JSX, `row { }` / `column { }` in Lua,
  `KUI_ROW` / `KUI_COLUMN` in C. A third value of an existing word is
  the smallest door: no new prop id, no new element op on the Node
  frame, no new field in `KuiSpec` (the ABI stays at 18), no new
  function in `kui.h`. In Lua, `table` is the standard library's
  table; the constructor cannot be called that.

## Decisions

1. **`LayoutSpec.table` is a flag on a column.** `NodeSpec::table()` is
   `column()` with it set; `dir` stays `Column`, so every place that
   matches on `Dir` keeps its two arms and a table is a column
   everywhere it is not a table: `gap` is between rows, `scrollY`
   scrolls them, `padding` is its own, the rows are its Tab-order
   children, a float in it is a float.

2. **Rows are rows and cells are cells, and both are the app's.** The
   table's in-flow `Row` children are the rows; each row's in-flow
   children are its cells; the nth cell of every row is column n; a row
   with fewer cells fills the first columns. A row keeps everything a
   row has — its `gap` is the space between its cells, its padding, its
   background, `hover_bg`, `on_click`, `label` — which is what a
   clickable settings row or a hovered inspector row needs, with no
   row API to learn. A header is a row. A float in a row is not a cell.
   *Amended (RG7):* only a `Row` is a row. Anything else straight
   under the table — a heading text, a `column` section wrapping a
   heading over its own rows, a nested table — is a child with its own
   width and no cells, and its children are its own; before, any
   in-flow container was a row, and a `column` section's heading and
   inner row were taken as cells 0 and 1 of the table. The rule is not
   a warning: a section under a table is a natural shape for a grouped
   settings list, and it lays out as the author meant. And
   `LayoutSpec.table` is read on a `Dir::Column` only, through
   `LayoutSpec::is_table` — a `Row` carrying the flag, which only
   Rust's public fields can build, is the row it says it is, to the
   solver, the diagnostics, `NodeInfo` and the corpus alike.

3. **A column's sizing is what its cells declared.** The column's fit
   width is the max of its cells' fitted widths — a `Fixed` cell's
   number, a `Fit` cell's content. Any `Grow` cell makes the column
   grow, with the largest factor; else any `Percent` cell makes it a
   percent column, with the largest fraction; else it is a fit column,
   and fixed (never shrunk) if any cell was `Fixed`. `minWidth` and
   `maxWidth` on a cell clamp the column: the largest floor and the
   smallest ceiling among its cells. No column spec on the table: the
   header cell that says `width: grow` says it for the column, and a
   table read in any binding is the same table.

4. **Pass 1 aligns the fit; pass 2 resolves the columns once.** In
   `fit_widths`, at the table: every column at its fit, written into
   every cell, and each `Fit` row re-fitted to the aligned cells, so
   the table's own fit width — read next — is the aligned one. In
   `grow_widths`, at the table, after its `Grow` rows have their width:
   the columns resolved against the widest row's content — percent
   columns take their cut, fit and fixed columns sit at their fit, grow
   columns split what is left in the same freeze loop `distribute_run`
   uses (backlog F71: a clamped column is frozen and the rest
   re-share), and when the fits alone overflow the row and the table
   does not scroll x, the fit columns are compressed toward their
   floors largest first, as `shrink_axis` compresses a row's children
   — then written into every cell, and each `Fit` row sized to the
   columns. A row of a table then skips its main-axis distribution
   entirely: its cells are final, and it neither grows, cuts nor
   shrinks them. *Amended (RG11):* in pass 1 every row but a `Fixed`
   one is sized to the aligned columns — the `grow` and percent rows
   included, whose own pass-1 width is 0 by the rule since C10 that a
   `Fit` parent counts a `Grow` child as nothing — so the table's own
   fit width, read next, is its columns'. The rows are the table's, and
   their `grow` says how the columns share the table, not that the
   table is nothing: a `Fit` table (the constructor's default, and
   `<box dir="table">` with no width) over `grow` rows is the aligned
   key/value list the howto shows, as wide as its longest key and
   value, where before it was 0 wide with every text folded to a glyph
   a line; and a `min: fit` floor on a `grow` table reads the same
   number. Pass 2 sizes the `grow` rows for good, as before.
   *Amended (RG3):* in pass 2, when the table scrolls x, every row is
   widened to its columns if they overflow it. A `grow` row is exactly
   the table's width, and `positions` measures a scroll container's
   content from its children's boxes — so the overflow the table kept
   (the compression it skipped) was clipped and `scroll_max.x` was 0
   for the rows every example writes, and 20 only for a `Fit` row. A
   table that does not scroll leaves its rows' boxes alone: fixed
   columns that overflow it overflow the row, as fixed children
   overflow any row.

5. **A bare text is a cell, held to its column.** `fit_heights` keeps
   the width pass 2 gave a text whose parent is a table row and takes
   only the height from the wrap, so `ui.text` straight inside a row
   is a column and the cells after it stay put. A text that needs an
   alignment inside its column (a right-aligned number) is a row
   around the text with `main_align: end`, as it would be anywhere.
   *Amended (RG8):* an image straight in a row is a cell the same way
   — its box is the column wide, since `positions` advances by the
   cell's box and a narrower one would close the cells after it up —
   and its `Fit` height is its aspect at its *own* width: the number a
   `Fixed` image declared, the intrinsic one of a `Fit` image, the
   column's only for a `Grow` or percent image that asked for it.
   Before, the height followed the column, and a 16 px icon in a 200 px
   column was a 200 × 200 box. How the pixels meet the wider box is
   the image's own `fit` row (ADR 0025): `fill`, the default,
   stretches them across the column; `contain` draws them at their
   size, centred. An icon that must keep its width and sit at the
   column's start is a box around the image, which holds the column
   as any `Fit` box does with the icon at its own size inside. The
   painter is not taught which image is a cell: the box is the cell,
   as it is for every other leaf, and the hit region and access rect
   are the box.

6. **A row of a table never wraps.** Its children are the columns, one
   each; `wrap_children` on it lays out as if absent and raises
   `wrap-ignored` with the reason, as a column's does.

7. **One door per binding, all the `dir` one.** `dir="table"` in JSX
   (`<box dir="table">`), `grid { }` in Lua — `grid`, since `table` is
   Lua's own, as `dropdown` is the select — and `KUI_TABLE` as the
   third `dir` in C, all lowering to `NodeSpec::table()`. A `table`
   row in `schema::ELEMENTS` for the docs and the corpus, whose
   `observe` derives it from the flag. One corpus scene, `table`, in
   the four adapters.

8. **No stock widget.** Every consumer that asked for a table — the
   panel's four lists, kawoosh's two tabs — needed cells that are not
   strings: a swatch and a hex, a select, a clickable row, a value in
   the accent. A `widgets::table(headers, rows_of_strings)` would have
   served none of them, and the container serves all of them in fewer
   lines than the boxes it replaces. If a string table is asked for,
   it is a loop over rows in any binding's own code; the container is
   the widget.

9. **What is not a row and what is not a cell is left alone.** Under
   the table: a text, an image, a `column`, a nested table, a float —
   each a child with its own width, laid out as it would be under any
   column. In a row: a float. The table never warns about them; the
   one diagnostic a table raises is `wrap-ignored` on a row that asked
   to wrap. (Decision 2 says why a section wrapper is allowed rather
   than refused.)

10. **A percent column's basis is the room the columns are laid across:
    the widest row's content box less that row's gaps.** `table_resolve`
    lays every column across one number — the widest row's width less
    its padding and `gap × (columns − 1)` — and a percent column takes
    its cut of that, the same number the grow columns share what is
    left of, so two `50%` columns with a gap between them fill the row
    exactly, with the gap. A percent child of a *plain* row takes its
    cut of the row's content box with the gap on top (`distribute_axis`:
    `content * p`, as CSS resolves a flex item's percentage against the
    container's content box), so two `50%` children of a plain row with
    a gap overflow it by the gap — which is CSS's answer too, and a
    plain row is left as CSS lays it. The two differ on purpose
    (recorded under backlog RG14): the table's columns are one shared
    allocation whose parts must sum to the row, and a percent that
    could overflow the row would push its grow siblings to zero and
    the last fit column out; a plain row's percent child is its own
    box, the author's to size. `a_percent_column_takes_its_cut_of_the_row`
    in `tests/table_layout.rs` pins the table's number beside the plain
    row's.

    *Amended 2026-09-29 (backlog F110, from kawoosh's launcher-sizes
    review):* a plain row's percent child — and a size expression's —
    now gives in an overflowing row, compressed toward its floor with
    the `Fit` children, largest first (`shrink_axis`), so two `50%`
    children and a gap fill a plain row too, 90 and 90 in 200 with a gap
    of 20, as the table's columns do. "CSS's answer too" was a flex
    item's with `flex-shrink: 0`; its default is 1, and the gap
    overflowing was what the user met, in kawoosh's launcher. The table
    keeps its own basis — the row less its gaps, a percent column never
    shrunk — so the two agree on the number by different roads while
    the shares sum to 100% or less; past that the table overflows where
    the row squeezes. A `Fixed` child still keeps its size, and a
    scrolling row overflows on purpose.

    *Corrected 2026-09-30 (backlog RG87, the regression run over F110):*
    "`flex-shrink: 1`" above names when a share gives, not how. The
    compression is the `Fit` children's rule — largest first, down to
    the next largest, each to its `min` (0 unless declared) — where CSS
    takes from each in proportion to its size and stops at its content:
    60% and 40% of 500 beside a `Fixed` 100 are 200 and 200 here, 240
    and 160 in CSS, and a share holding a 120 px child can go under it.

    *Amended 2026-09-30 (backlog RG92):* a row holding a share now gives
    as CSS's does. Every shrinkable child of it — the shares and the
    `Fit` children beside them — gives in proportion to its size
    (`shrink_as_css`, css-flexbox §9.7 with `flex-shrink: 1`), down to
    its declared `min`, or where none is declared (`Min::AUTO`) to its
    min-content: the widest thing in it that cannot wrap, a text's
    longest word measured off its shaped glyphs, the fixed children and
    gaps of a row added up. `minWidth: 0` declares no floor, as
    `min-width: 0` does, and a child scrolling that axis has none. A row
    of `Fit` children alone keeps the largest-first rule, so no layout
    without a share moved; a `Fixed` child still keeps its size, where
    CSS's would give too; and the table's percent columns still never
    shrink.

## Considered options

- **A widget over `measure_text`.** `widgets::key_value(ui, rows)`
  measuring each key and emitting fixed boxes. Rust only, or a widget
  op per binding; strings only, or a callback per cell, which Lua and
  C cannot hand a Rust widget; and the measurement is what the solver
  already does in pass 1. Declined.

- **A column spec on the table** (`columns: [fit, grow, 80]`). One
  more `Vec` on `NodeSpec`, a JSON blob on the Node frame, a
  `KuiSizing` array in `kui.h`. Not needed while a cell can say it
  (decision 3), and a table whose columns are declared apart from its
  cells has two places to disagree. Deferred until a table wants a
  column no cell of it declares — a column of empty cells — which none
  has.

- **`Dir::Table`.** A third direction reads well in C and the wire,
  and that is how the bindings spell it; but in the solver every
  `match dir` would gain an arm that says "as a column", and the
  cross-axis code that a table shares with a column would have to be
  reached twice. A flag on a column touches the solver in five places
  (below) and nothing else.

- **A column that shares widths across rows of *any* column
  (`align_cells` on every row).** Opting rows in one at a time
  spreads the declaration over the rows and lets two of them disagree.
  The table is the unit that has the columns.

- **A grid** (CSS grid: explicit tracks, cells spanning). A different
  thing, with a different pass order (a cell in two rows sizes both).
  Not asked for; the name `grid` is taken by the Lua constructor
  because `table` cannot be, not because this is one.

## Consequences

- The panel's Facts, tokens, legend and inspector lists and kawoosh's
  Perf and Settings tabs are tables: six hand-picked widths gone, every
  label column at its longest label, the value columns growing.
  A longer label widens its column instead of overflowing it.

- Layout cost is gated on `Tree::any_table`: a frame with no table
  runs the passes as before, one predicted branch per node in the
  five places the flag is read — `wraps` (a table row never wraps),
  `fit_widths` (the table step of pass 1), `distribute_axis` twice (a
  table row's cells are final; the table step of pass 2) and
  `fit_heights` (a text cell keeps its column's width) — every one
  behind the `any_table` test first; `check_wrap` in `diag.rs` reads it
  too, off the solver's path. A frame with one walks the table's
  rows and cells twice more (pass 1 and pass 2) and allocates a
  `Vec<Col>` per table per pass, the size of its column count.

- A virtualised list of rows (`virtual_column`) whose column is a
  table sizes its columns from the rows built that frame, so a column's
  width can change as the list scrolls past a wider cell. That is what
  every virtualised table does; a list that wants stable columns gives
  the wide column a `Fixed` cell.

- A `Fit` row in a table whose columns grow is as wide as the columns,
  which is wider than its content: the row's width follows the
  columns, not the other way round. Rows are usually `grow`, and
  then this never shows.

- A `Fit` table is as wide as its columns whatever its rows' sizing —
  the one place a `Fit` parent's width counts a `Grow` child, because
  the child is the table's own row and the columns are what it holds
  (RG11). A `scrollX` table's `grow` rows are wider than the table
  when the columns overflow it, and the row's background spans the
  scrolled content (RG3). An image cell's box is column × own aspect,
  and `fit: fill` stretches the pixels across it (RG8).

- `KUI_ABI_VERSION` stays 18 and the Node frame at v15: a `dir` value
  is data both already carry.
