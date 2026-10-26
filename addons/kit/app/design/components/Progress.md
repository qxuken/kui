# Progress

A bar that fills; with no value it moves to say work is going on.


## When to use

Use a progress bar for work the user waits on: an upload, a worker building rows, a checklist ticked off. The track is 4px in the rule colour, the bar the accent; a value eases to each new one on a short spring, and with no value a third of the track slides across it (held still at its start under reduced motion).

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | number? | 0 to 1; indeterminate when left out. |
| `label` | `label` | string? | What is under way. |

Every component also takes `key` and `fresh`.

## Do

- Give `label` for what is under way, as a phrase: Uploading, The worker is building 100,000 rows….
- Pass `value` from 0 to 1 when you know it; leave it out when you do not.

## Don't

- Show a progress bar for something that takes less than a moment.
- Use it as a meter for a level (disk use): it reads as work going on.
