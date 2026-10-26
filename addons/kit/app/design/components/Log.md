# Log

A log over a channel: time, level and message a line, following the end unless scrolled away.


## When to use

Use a log for lines that arrive over time: events, a server's output, a build. It sits in the sunken well; each 18px line has the time in faint, the level in 44px of bold capitals (debug faint, info muted, warn and error in their tones) and the message, all at the label size in the data face. ANSI colours in a message are drawn in the skin's: red is danger, green ok, yellow warn.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `rows` | `rows` | channel | The lines: fields `time`, `level` and `message`; a message's ANSI colours (SGR) are drawn in the skin's. |
| `follow` | `follow` | boolean? | Kept at the end as lines arrive; on when left out. |
| `grow` | `grow` | boolean? | Takes the room its parent leaves. |
| `wrap` | `wrap` | boolean? | A line longer than the log wraps onto more rows, where it is cut. |
| `time` | `time` | boolean? | The time column; on when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_select` | `onSelect` | `select` = { `row`: integer — Its index. } | A line was clicked. |

Every component also takes `key` and `fresh`.

## Do

- Publish lines as a channel with `time`, `level` and `message` fields.
- Leave `follow` on: the log keeps to its end until the reader scrolls up, and a Latest button takes them back.
- Use `wrap` when messages are long and every word matters (stack traces).

## Don't

- Rebuild the whole channel per line from scratch: publish the next revision, and a log that drops its oldest lines keeps the reader's place.
- Use a log for records the user sorts or selects among: use a table.
