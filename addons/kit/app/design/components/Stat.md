# Stat

A number with its label, and how it moved.


## When to use

Use a stat for a headline number: requests a second, a latency, an error rate, open incidents. The label is a muted bold caption, the number 22px bold in the data face, and the move under it at the label size in its tone (muted when no tone is given).

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string | What it counts. |
| `value` | `value` | string | The number, as text. |
| `delta` | `delta` | string? | How it moved: `+12%`. |
| `tone` | `tone` | Tone? (neutral · info · accent · ok · warn · danger) | Whether the move is good news. |

Every component also takes `key` and `fresh`.

## Do

- Pass the value as text, already formatted with its unit: "2,106", "325 ms", "0.90%".
- Say what the move is against: "+4% vs 15 min ago".
- Set `tone` by whether the move is good news, not by its sign: latency going up is danger.

## Don't

- Put a sentence in `value`.
- Tone a number that has not moved: say "steady" in muted.
