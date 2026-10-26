# TextField

A one-line text field, the skin's field style (an underline in Ledger).


## When to use

Use a text field for one line of text the user types: a name, a version, a path. Ledger and Phosphor draw an underline: a faint hairline at rest, the region rule in the ink when focused, danger in error. Neutral draws a sunken box with a hairline edge that turns focus blue or danger. The label sits above it in muted bold, cased by the skin.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | string | Its text: yours to keep, from `change`. |
| `label` | `label` | string? | Its label, above it; its name. |
| `hide_label` | `hideLabel` | boolean? | The label names the field but is not drawn: a field whose place says what it is. |
| `placeholder` | `placeholder` | string? | Shown while it is empty. |
| `mono` | `mono` | boolean? | In the data face. |
| `disabled` | `disabled` | boolean? | Unavailable. |
| `width` | `width` | number? | Its width in px; grows when left out. |
| `focus` | `focus` | integer? | Takes the keyboard each time this number changes while the field is on screen: keep a counter in your model and bump it (⌘F). A field that shows after the bump takes its number as where it starts. |
| `autofocus` | `autofocus` | boolean? | Takes the keyboard as it first shows: a new item's name. |
| `error` | `error` | string? | What is wrong with the text: drawn at the end of the field in the danger colour (cut short past half its width, the whole message its tooltip), its edge in danger too, and read after its name. It takes no room of its own, so a field in a toolbar keeps its height. An empty string marks the field only, for fields that share one message. |
| `browse` | `browse` | Browse? (folder · open · save) | A Choose… button after the field that opens the system's folder, open or save dialog; the path picked arrives as `change`, as if typed. Cancelled, nothing changes. The dialog starts in the field's folder when its text is an absolute path. |
| `browse_label` | `browseLabel` | string? | The button's label; Choose… when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `text`: string — The whole text. } | The text changed, or a path was picked. |
| `on_submit` | `onSubmit` | `submit` = { `text`: string — The whole text. } | Enter. |

Every component also takes `key` and `fresh`.

## Do

- Keep the value in your model and replace it from `change`; submit on `submit`.
- Put what is wrong in `error`, said as how to fix it: "Three numbers: 1.4.0". It sits at the field's end and takes no room, so a field in a toolbar keeps its height.
- Use `mono` for versions, ids, paths and URLs.
- Use `browse` for a path: a Choose… button opens the system's dialog and the path picked arrives as `change`.
- Use `hide_label` only where the field's place says what it is; the label still names it.

## Don't

- Write a placeholder that repeats the label, or put instructions only in the placeholder.
- Validate on every keystroke with a scolding message; say what the text should be.
- Set a fixed `width` in a row that can be narrow: a width is an upper bound, the field gives way to 80px.
