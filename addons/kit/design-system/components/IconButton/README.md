# IconButton

A square button holding an icon.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.icon_button` and kui paints it.

## When to use

Use an icon button where the icon says the action at a glance and room is short: a toolbar's reload, a panel's settings, a note's delete. It is the control height square, ghost by default, the icon 14px in the label's colour.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `icon` | `icon` | string | The icon's name (Kit's icon set). |
| `label` | `label` | string | What it does: its accessible name and its tooltip. |
| `variant` | `variant` | Variant? (default · primary · ghost · danger · success) | Ghost when left out. |
| `disabled` | `disabled` | boolean? | Unavailable. |
| `pressed` | `pressed` | boolean? | A toggle that is on: drawn selected and read as such. |
| `tooltip` | `tooltip` | string? | The hint under the pointer; the label when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_press` | `onPress` | — | Pressed. |

Every component also takes `key` and `fresh`.

## Do

- Always give `label`: it is the accessible name and the tooltip.
- Use the danger variant for a delete that sits alone (Field Notes' trash beside a note).
- Use `pressed` for a toggle (show or hide a pane).

## Don't

- Use an icon that Kit does not have: a missing name draws an empty square of the same size.
- Use an icon button for an action whose icon is ambiguous; use a button with a label.
