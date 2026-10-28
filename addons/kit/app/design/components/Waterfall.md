# Waterfall

A trace waterfall: spans as bars on a time axis, nested by depth, coloured by group, zoomed and panned by the wheel and a drag.


## When to use

Use a waterfall for a trace: frames and the work inside them, a request's phases, spans from a profiler. Lanes are 20px, one per depth; each span is a 16px bar in its group's pale fill (group-1 to group-6) with a hairline rule edge and its name in the data face when it is 48px wide or more; a span narrower than 2px collapses into a faint tick. The selected span takes the selection with an ink edge; marks are 1px accent rules down the lanes, those closer than a pixel one rule whose tooltip counts them. A frame costs its width, not its spans: the bars come from the channel's lanes (`bars:lanes`, read a window at a time in Rust), so a million spans pan and zoom as an 18-span request does. The head carries the window's range and span count, and zoom out, zoom in and fit.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `bars` | `bars` | channel | The spans: fields `name`, `start`, `len` (ms), `depth`, `group`, and `target` (a label). |
| `marks` | `marks` | channel? | Points in time, fields `at` and `label`: a navigation, a paint. |
| `start` | `start` | number? | The window's start in ms: yours to keep, from `zoom`. |
| `length` | `length` | number? | The window's length in ms; the whole trace when left out. |
| `selected` | `selected` | integer? | The selected span (its row). |
| `grow` | `grow` | boolean? | Takes the room its parent leaves. |

| event | Node | carries | when |
|---|---|---|---|
| `on_select` | `onSelect` | `select` = { `row`: integer — Its row. } | A span was clicked. |
| `on_zoom` | `onZoom` | `zoom` = { `start`: number — Its start in ms.; `length`: number — Its length in ms. } | The wheel or a drag asks for another window. |

Every component also takes `key` and `fresh`.

## Do

- Pass spans as a channel with `name`, `start`, `len` (ms), `depth`, `group` and `target`.
- Keep `start` and `length` in your model from `zoom` to own the window; leave them out and Kit keeps its own.
- Show the selected span's fields in an inspector beside it.
- Add `marks` for points in time that explain the spans: a navigation, a paint.

## Don't

- Use more than six groups: the fills repeat.
- Put a waterfall in a short box: give it `grow` in a pane, so the lanes scroll.
