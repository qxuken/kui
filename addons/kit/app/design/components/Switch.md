# Switch

An on/off switch and its label.


## When to use

Use a switch for a setting that takes effect the moment it is flipped: Live, Pinned, Serve on :3050. The track is 32 by 18: sunken and edged when off, the accent when on. The knob is one mark that only moves: a raised disc in Neutral, a solid block of the ink in Ledger, and in Phosphor a muted block off and a page-coloured block edged in the ink on.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string | Its label; its name. |
| `on` | `on` | boolean | On: yours to keep. |
| `disabled` | `disabled` | boolean? | Unavailable. |

| event | Node | carries | when |
|---|---|---|---|
| `on_toggle` | `onToggle` | `toggle` = { `on`: boolean — What it asks to be. } | Pressed. |

Every component also takes `key` and `fresh`.

## Do

- Write the label as the thing that is on: Live feeds, Open Tidewater when I log in.
- Act on `toggle` at once; there is no Save for a switch.

## Don't

- Use a switch inside a form that is saved with a button: use a checkbox.
- Change the label with the state (On / Off): the track says it.
