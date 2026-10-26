# Menu

A menu of items floating under what opened it.


## When to use

Use a menu for a set of actions on one thing that do not fit as buttons: copy, rename, archive, delete. It floats under its opener on the float surface, at least 180px wide, 4px padding; items are `$kitRow` tall in body text with the chord in faint at the far end. A destructive item is drawn in danger; a disabled one at half opacity. The first enabled item takes the keyboard.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `items` | `items` | {Item} | The rows, in order. |

| event | Node | carries | when |
|---|---|---|---|
| `on_choose` | `onChoose` | `choose` = { `value`: string — Its value. } | A row was chosen. |
| `on_dismiss` | `onDismiss` | — | Escape, or a press outside. |

Every component also takes `key` and `fresh`.

## Do

- Write items as verbs; end with an ellipsis when the item asks more (Rename…).
- Put the destructive item last, with `danger`.
- Close it on `choose` and on `dismiss`.

## Don't

- Use a menu to pick a value: use a select.
- Hide an action that is unavailable now: show it `disabled`.
