# Inspector

Keys and their values, aligned: a request's headers, a record's fields.


## When to use

Use an inspector to show one record's fields as key and value: a selected row, a span, a response's headers, a service's facts. Keys are muted, in a column 140px wide by default; values are body text or the data face, and wrap. The text can be selected and copied.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `entries` | `entries` | {Entry} | The rows, in order. |
| `key_width` | `keyWidth` | number? | The keys' column in px; 140 when left out. |

Every component also takes `key` and `fresh`.

## Do

- Set `mono` on values that are ids, paths, versions, timings.
- Set `key_width` to fit your longest key (70 for a table's column titles, 170 for HTTP headers).

## Don't

- Use it for editable fields: use text fields in a form.
- Put long prose in a value; it wraps, but an inspector is for facts.
