# Badge

A short label in a pill: a status, a method, a count.


## When to use

Use a badge for a word or a number that states something: a status (healthy, degraded, down), an HTTP status, a count of unsaved changes, a version. It is 18px tall on the sunken well, square in Ledger and Phosphor and round in Neutral, its text bold where the skin's labels are, in the tone's colour; the accent tone fills it with the accent. It is written as given, never recased.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string | What it says. |
| `tone` | `tone` | Tone? (neutral · info · accent · ok · warn · danger) | Neutral when left out. |
| `dot` | `dot` | boolean? | A dot in the tone's colour before the label: a status. |
| `tooltip` | `tooltip` | string? | A hint under the pointer: what the status means. |
| `fade` | `fade` | boolean? | Fades in as it appears and out as it goes: a status that comes and goes with an edit (changed), not one of a table's. |

Every component also takes `key` and `fresh`.

## Do

- Add `dot` for a status, so the state reads before the word.
- Say what a status means in `tooltip`: "OK: the request succeeded".
- Use `fade` for a status that comes and goes with an edit (changed).
- Pick the tone by meaning: ok for healthy, warn for slow or degraded, danger for down or 5xx, accent for what is new or unsaved, info for a note.

## Don't

- Make a badge a button: it takes no press.
- Use colour alone: the word says it, the tone backs it up.
- Outline a badge: Kit draws none, so a row of badges beside fields is not one more set of edges.
