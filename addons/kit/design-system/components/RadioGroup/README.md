# RadioGroup

One of a few options, each a radio and its label.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.radio_group` and kui paints it.

## When to use

Use a radio group for one choice of a few, where seeing every option helps: a log level, an appearance. Each radio is a 14px circle in every skin, edged in the ink, the chosen one edged in the accent with a 6px accent dot. Rows are `$kitRow` tall, 2px apart; the group is one Tab stop and the arrows choose as they move.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `options` | `options` | {Choice} | The options, in order. |
| `value` | `value` | string? | The one chosen: yours to keep. |
| `label` | `label` | string? | The group's name, drawn above it. |
| `hide_label` | `hideLabel` | boolean? | The name is read but not drawn: a group whose place says what it is. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `value`: string — Its value. } | An option was chosen. |

Every component also takes `key` and `fresh`.

## Do

- Give the group a `label`; use `hide_label` only where its place names it.
- Order the options as the user thinks of them: by level, by scale, system first.

## Don't

- Use radios for more than five or six options: use a select.
- Use radios as a view switch: use a segmented control.
