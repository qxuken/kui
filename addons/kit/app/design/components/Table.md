# Table

A virtual table over a channel: its columns, a sticky header, the rows in view only, a selection, sortable headers.


## When to use

Use a table for many records with the same fields: requests, services, files. It builds only the rows in view, so 100,000 rows cost a screenful. Rows are `$kitRow` tall with 10px sides and gaps; the selected row is the selection with sel-fg text; the header labels are muted captions, the sorted one fg with a chevron.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `columns` | `columns` | {Column} | The columns, in order. |
| `rows` | `rows` | channel | The rows: a channel whose columns the `field`s name. |
| `selected` | `selected` | integer? | The selected row (0-based): yours to keep. |
| `sort` | `sort` | string? | The field it is sorted by, marked in its header; the sorting is yours. |
| `descending` | `descending` | boolean? | Sorted from the top down. |
| `striped` | `striped` | boolean? | Every other row tinted. |
| `border` | `border` | boolean? | Framed in a hairline, the header ruled off inside it; left out, no frame and the header on a plane of its own (in a panel, which is framed already). |
| `cells` | `cells` | boolean? | A table of cells: the arrows walk cells (Left and Right along a row, Up and Down keeping the column), the header a row of column headers, landing on a row selecting it. Left out, a list of rows, the arrows moving whole rows. |
| `grow` | `grow` | boolean? | Takes the room its parent leaves. |

| event | Node | carries | when |
|---|---|---|---|
| `on_select` | `onSelect` | `select` = { `row`: integer — Its index, 0-based. } | A row was clicked, or the keyboard landed on it (the arrows, Page keys, a type-ahead, Tab back in). |
| `on_sort` | `onSort` | `sort` = { `field`: string — The column's field. } | A header was clicked. |
| `on_range` | `onRange` | `range` = { `first`: integer — The first row in view.; `last`: integer — The last row in view. } | The rows in view changed: page your source by it. |

Every component also takes `key` and `fresh`.

## Do

- Pass rows as a channel and name each column's `field`; sort and filter in your worker and hand the table the new channel.
- Right-align numbers (`align = "end"`) and set ids, timings, sizes and hashes in the data face (`mono`).
- Make a status column `badge = true, tone = "tone"`: a pill with a dot in the tone the row names.
- Use `border` for a table on its own; leave it out inside a panel, which is framed already, and the header sits on the sunken plane.
- Use `cells` when the user needs to walk cells with the arrows; leave it out for a list of rows.
- Keep `selected` and `sort` in your model; page your source by `range`.

## Don't

- Pass a plain Lua table of thousands of rows: use a channel.
- Colour whole rows by status: the badge column carries it, and the selection must stay readable.
- Give every column a fixed width: leave the one that should take the rest (a path, a name) without one.
