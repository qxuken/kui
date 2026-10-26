# BarChart

Bars by label.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/charts.tl`. In an app, place `kit.bar_chart` and kui paints it.

## When to use

Use a bar chart to compare a measure across a few named things: requests by status, a 5xx rate by service. Standing bars rise in the accent from an ink baseline, 2px apart, against round steps ruled in the rule colour, each value over its bar and each name under it in muted (two lines before an ellipsis). The bar under the pointer, or the keyboard, turns the ink. With `horizontal`, bars lie along 18px rows, each name whole beside its bar: for long names or many bars.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `data` | `data` | channel | The rows. |
| `label` | `label` | string | The column naming each bar. |
| `value` | `value` | string | The column of their values. |
| `height` | `height` | number? | In px; 160 when left out (standing bars). |
| `unit` | `unit` | string? | After each value and axis number: `%`, `ms`. |
| `values` | `values` | boolean? | Each bar's value over it; on when left out. |
| `horizontal` | `horizontal` | boolean? | Bars lying along rows, each name whole beside its bar: for long names or many bars. |
| `label_width` | `labelWidth` | number? | Lying bars: the names' column in px; 120 when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_select` | `onSelect` | `select` = { `row`: integer — Its row. } | A bar was clicked. |

Every component also takes `key` and `fresh`.

## Do

- Give `unit` (req, %, ms): it follows every value and axis number.
- Use `horizontal` with `label_width` when names are long (service names) or bars are many.
- Handle `select` to let a bar be picked; the bars become one Tab stop walked by the arrows.

## Don't

- Use a bar chart for a series over time: use a line chart.
- Expect Ledger light's yellow bars to carry the comparison alone (1.41:1 on panel): the values over the bars and the ink baseline carry it, so keep `values` on.
