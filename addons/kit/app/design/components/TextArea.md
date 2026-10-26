# TextArea

A text field of several lines, in a well in every skin: a note, a request body.


## When to use

Use a text area for text that runs to several lines: a note, a comment, a JSON body. It is a sunken well with a hairline edge in every skin (an underline under a block of text reads as a rule), `rows` lines tall at the least (4 by default) and growing with its text up to `max_rows`. Lines are 17px; the padding is 8.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | string | Its text: yours to keep, from `change`. |
| `label` | `label` | string? | Its label, above it; its name. |
| `hide_label` | `hideLabel` | boolean? | The label names the field but is not drawn. |
| `placeholder` | `placeholder` | string? | Shown while it is empty. |
| `mono` | `mono` | boolean? | In the data face: code, JSON. |
| `disabled` | `disabled` | boolean? | Unavailable. |
| `width` | `width` | number? | Its width in px; grows when left out. |
| `rows` | `rows` | integer? | Lines tall at the least; 4 when left out. It grows with its text. |
| `max_rows` | `maxRows` | integer? | Lines tall at the most, scrolling past them; no limit when left out. |
| `grow` | `grow` | boolean? | Takes the room its parent leaves (a request body filling its tab), `rows` still its least. |
| `focus` | `focus` | integer? | Takes the keyboard each time this number changes while it is on screen. |
| `autofocus` | `autofocus` | boolean? | Takes the keyboard as it first shows. |
| `error` | `error` | string? | What is wrong with the text, under it in the danger colour. |
| `tab_moves_focus` | `tabMovesFocus` | boolean? | Tab and Shift+Tab move the keyboard to the next or the last control, as from a one-line field: a note, a comment. Left out, Tab types indentation as a code editor's does, and the ways out are Escape then Tab, or Ctrl+M (Ctrl+Shift+M on macOS). |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `text`: string — The whole text. } | The text changed. |

Every component also takes `key` and `fresh`.

## Do

- Use `mono` for code and JSON.
- Set `tab_moves_focus` for prose (a note, a comment), so Tab moves on as from a one-line field. Left out, Tab types indentation, and the ways out are Escape then Tab, or Ctrl+M.
- Use `grow` for a body that fills its tab, with `rows` still its least.
- Put the error under it; it says what is wrong and where.

## Don't

- Use a text area for one line.
- Cap `max_rows` so low that a normal entry scrolls.
