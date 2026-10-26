# Slider

A value on a track, dragged, clicked or stepped by the arrows.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.slider` and kui paints it.

## When to use

Use a slider for a value in a range where the rough position matters more than the digits: a zoom, a limit, a retention. The label sits above in muted bold with the value in the data face at the far side; the 4px fill is the accent, the rest the rule colour, and the 12px handle is the panel edged in the ink, lit with the accent under the pointer.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | number | The value: yours to keep. |
| `min` | `min` | number | The least. |
| `max` | `max` | number | The most. |
| `step` | `step` | number? | What an arrow moves it by; a hundredth of the range when left out. |
| `label` | `label` | string? | Its label; its name. |
| `value_label` | `valueLabel` | string? | The value as text, shown beside the label and read for it: `20 MB/s`, `30 days`. The number when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `value`: number — The value it asks for. } | It asks for another value. |

Every component also takes `key` and `fresh`.

## Do

- Pass `value_label` with the unit: "20 MB/s", "30 days", "40%"; it is read for the value too.
- Set `step` to what the arrows should move by.

## Don't

- Use a slider where the user needs an exact number: use a text field with `mono`.
- Leave the value unshown.
