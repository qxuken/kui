# List

A virtual list over a channel: a title line and a detail line a row.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/data.tl`. In an app, place `kit.list` and kui paints it.

## When to use

Use a list for records read by one name and a line of detail: history, endpoints, notes. Rows are 40px with a detail line (26 without, 22 compact), the title in fg and the detail in muted; a `badge` field puts a pill before the title in the row's tone.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `rows` | `rows` | channel | The rows. |
| `title` | `title` | string | The field each row's title reads. |
| `detail` | `detail` | string? | The field its second line reads. |
| `badge` | `badge` | string? | A field drawn as a pill before the title: a method, a state. |
| `tone` | `tone` | string? | A field naming each row's badge tone (`ok`, `warn`, `danger`, `accent`, `neutral`). |
| `badge_width` | `badgeWidth` | number? | The badge column's width in px, so the titles line up; each badge's own when left out. |
| `label` | `label` | string? | Its name, read and not drawn: what the list holds (History). |
| `selected` | `selected` | integer? | The selected row (0-based): yours to keep. |
| `grow` | `grow` | boolean? | Takes the room its parent leaves. |

| event | Node | carries | when |
|---|---|---|---|
| `on_select` | `onSelect` | `select` = { `row`: integer — Its index, 0-based. } | A row was clicked, or the keyboard landed on it (the arrows, Page keys, a type-ahead, Tab back in). |

Every component also takes `key` and `fresh`.

## Do

- Name the `title` and `detail` fields of your channel.
- Set `badge_width` so the titles line up when badges differ in width (48 fits GET, POST, PUT).
- Give `label` for what the list holds (History, Endpoints).
- Keep `selected` in your model; the keyboard landing on a row selects it.

## Don't

- Use a list where the user compares fields across rows: use a table.
- Put controls in a row: a row is one selectable item.
