# Split

Two panes and a draggable rule between them.


## When to use

Use a split for a window whose two parts the user sizes: a sidebar and the page, a list and the item open in it, a request over its response (`axis = "column"`). The panes are told apart by a hairline in the rule colour; the 6px strip around it washes with hover where it can be grabbed.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `axis` | `axis` | Axis? (row · column) | Side by side (row, the default) or stacked. |
| `fraction` | `fraction` | number | How much of the room the first pane takes, 0 to 1: yours to keep and pass back. |
| `min` | `min` | number? | The least either pane is, in px; 80 when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_resize` | `onResize` | `resize` = { `fraction`: number — 0 to 1. } | The rule was dragged: the fraction it asks for. |

Children:

- `first` is the first pane.
- `second` is the second pane.

Every component also takes `key` and `fresh`.

## Do

- Keep `fraction` in your model and pass back what `resize` asks for.
- Set `min` to the least a pane can be read at (Watchtower keeps 380px, Field Notes 160px for its sidebar).
- Nest splits for three panes: an outer split whose second pane is another split.

## Don't

- Draw your own rule or border between the panes: the split draws it.
- Use a split where the sizes never need to change: a fixed column is a row with widths.
