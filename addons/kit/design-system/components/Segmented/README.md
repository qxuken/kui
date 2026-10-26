# Segmented

A row of joined buttons, one of them chosen: a view switch, a range.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.segmented` and kui paints it.

## When to use

Use a segmented control to switch a view or a range: Table / List / Chart, 5 min / 15 min / 1 hour, an environment. The track is the pressed wash, 2px padding, segments 2px apart; the chosen segment is the `chosen` role: printed in the ink with the page's colour on it in Ledger and Phosphor, a raised chip in Neutral, the brand's accent once settings give one.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `options` | `options` | {Choice} | The segments, in order. |
| `value` | `value` | string | The one chosen: yours to keep. |
| `label` | `label` | string? | The group's name, read and not drawn: what the choice is of. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `value`: string — Its value. } | A segment was chosen. |

Every component also takes `key` and `fresh`.

## Do

- Keep it to two to five short labels.
- Give a segment a `count` and mark bad news with `alert`: the count turns danger off the chosen segment and bold in its ink on it.
- Give `label` for what the choice is of (Environment, Time range); it is read, not drawn.

## Don't

- Use it for actions: each segment is a choice, not a command.
- Use it for a long list of options: use a select.
