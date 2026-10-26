# Code

Text in the data face in a sunken well: a payload, a command, a stack.


## When to use

Use a code block for text the user reads or copies verbatim: a response body, a config file, a command. It is a sunken well with 8px padding, the text in the data face at 16px lines, scrolling sideways unless it wraps. A label names it; a Copy button (ghost, turning to a success Copied for 1.5 s) puts it on the clipboard.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `text` | `text` | string | The text. |
| `wrap` | `wrap` | boolean? | Wraps rather than scrolls sideways. |
| `max_lines` | `maxLines` | integer? | At most this many lines high, the rest scrolled to. |
| `line_numbers` | `lineNumbers` | boolean? | Each line's number in a gutter beside it, outside the selection (not with `wrap`). |
| `copy` | `copy` | boolean? | A Copy button over the text that puts it on the clipboard and says Copied for a moment. |
| `label` | `label` | string? | A caption over the text (a file's name), and the block's accessible name. |

Every component also takes `key` and `fresh`.

## Do

- Give `label` for a file's name; it also names the block for assistive technology.
- Turn on `copy` for anything the user will paste elsewhere.
- Use `line_numbers` for a file, `max_lines` to cap a long payload.

## Don't

- Use a code block for text the user edits: use a text area with `mono`.
- Combine `line_numbers` with `wrap`: numbers are not drawn with wrapping.
