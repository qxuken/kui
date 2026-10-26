# Popover

A panel floating beside what opened it; Escape and a press outside ask it to close.


## When to use

Use a popover for a small task or detail tied to the control that opened it: scale a service, show a stat's breakdown, pick from a few fields. It floats under its opener, its left edge on the opener's, flipped and clamped onto the screen; it is the float surface (raised, the ink edge, the strong shadow) with 10px padding and 8 between children.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string? | Its name, read and not drawn: what it holds (Scale the service). A modal surface with no name warns `modal-without-name`. |
| `width` | `width` | number? | Its width in px. |

| event | Node | carries | when |
|---|---|---|---|
| `on_dismiss` | `onDismiss` | — | Escape, or a press outside. |

Children:

- the list part is what it holds.

Every component also takes `key` and `fresh`.

## Do

- Give `label` for what it holds (Scale the service): a modal surface is announced by its name.
- Set `width` for its content; 260 holds a stat and a line.
- Keep its open state in your model; close it on `dismiss`.

## Don't

- Put a form that needs more than a few fields in it: use a dialog or a panel.
- Open a popover from hover.
