# Checklist

A list of things to tick, one Tab stop: Up and Down walk the items, Home and End the ends, Space or a click ticks the one focused.


## When to use

Use a checklist for a list of tasks or items to tick: a release's steps, a shopping list in a note. Each item is a control-height row with a 14px box; a ticked item's label turns muted. With `removable`, an × in muted on each item removes it under the pointer.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `items` | `items` | {Check} | The items, in order: yours to keep. |
| `label` | `label` | string? | What it lists, as it is read. |
| `removable` | `removable` | boolean? | An × on each item, which the pointer presses to remove it. |

| event | Node | carries | when |
|---|---|---|---|
| `on_toggle` | `onToggle` | `toggle` = { `index`: integer — Its place, 0-based.; `checked`: boolean — What it asks to be. } | An item was ticked or unticked. |
| `on_remove` | `onRemove` | `remove` = { `index`: integer — Its place, 0-based. } | An item asked to go. |

Every component also takes `key` and `fresh`.

## Do

- Keep the items in your model; `toggle` and `remove` carry the item's 0-based index.
- Give `label` for what it lists (Release); it is read, not drawn.
- Show the count done beside it: "2 of 6 done", or a progress bar.

## Don't

- Use a checklist for settings: use checkboxes or switches.
- Expect Delete to remove an item: no key removes, only the ×.
