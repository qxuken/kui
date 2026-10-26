# Checkbox

A box and its label.


## When to use

Use a checkbox for an option that is on or off and takes effect with the rest of a form, or to tick an item. The box is 14px: the panel colour edged in the ink (rule width where controls are outlined, a hairline in Neutral), filled with the accent and a check in on-accent when ticked, a dash when mixed. Focus rings the box itself, 2px out, not the whole row.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string | Its label; its name. |
| `checked` | `checked` | boolean | Ticked: yours to keep. |
| `mixed` | `mixed` | boolean? | Some of what it stands for: drawn as a dash. |
| `disabled` | `disabled` | boolean? | Unavailable. |
| `hide_label` | `hideLabel` | boolean? | The label names it but is not drawn: its children draw it instead (a name with search hits marked). |
| `grow` | `grow` | boolean? | Takes its row's room, the whole height a target: a list row ticked anywhere. |

| event | Node | carries | when |
|---|---|---|---|
| `on_toggle` | `onToggle` | `toggle` = { `checked`: boolean — What it asks to be. } | Pressed. |

Children:

- the list part draws after the box, the label's place: read as part of it, so nothing in it is a control.

Every component also takes `key` and `fresh`.

## Do

- Write the label as the thing that is true when ticked: Follow the log, Keep history.
- Use `mixed` for a box that stands for several, some ticked.
- Use `grow` in a list row so the whole row is the target.
- Draw a label with search hits marked as children, with `hide_label`; the label still names it.

## Don't

- Use a checkbox for a setting that acts at once: that is a switch.
- Write a negative label (Don't sync): the tick should mean yes.
