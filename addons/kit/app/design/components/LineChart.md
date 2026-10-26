# LineChart

Lines over time with axes and a legend; `live` keeps the newest window in view.


## When to use

Use a line chart for one or more measures over time or an index: CPU and memory over the last 30 s, p50, p95 and p99 latency over 15 minutes. The legend sits over it (10 by 2 swatches, names in muted); y labels in the data face at 10px, faint, right-aligned in a gutter; grid lines in the rule colour; series lines 1.5px in series-1 to series-6; `area` fills under the first in area-1. Under the pointer, a faint crosshair, a 7px dot on each series and a readout on raised.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `data` | `data` | channel | The rows: an x column and a column a series. |
| `x` | `x` | string | The x column (time, an index). |
| `series` | `series` | {Series} | The lines. |
| `height` | `height` | number? | In px; 160 when left out. |
| `area` | `area` | boolean? | Filled under the first line. |
| `window` | `window` | number? | With it, only the last `window` of x is shown: a live chart. |
| `x_format` | `xFormat` | XFormat? (number · ago · clock) | How the x ticks read; the number when left out. |
| `x_unit` | `xUnit` | string? | After each x tick's number: `min`, `s`. |
| `y_unit` | `yUnit` | string? | After each y tick's number: `ms`, `%`. |
| `readout` | `readout` | boolean? | Under the pointer, a crosshair on the nearest sample, a dot on each series there and a readout of the x and every series' value in its colour, following the pointer and gone when it leaves; on when left out. An idle chart pays one hover layer for it. |
| `focusable` | `focusable` | boolean? | A Tab stop: Left and Right move the readout a sample, Home and End to the ends, and the readout is a live region a screen reader announces. Left out, the chart is no Tab stop; the pointer's readout works either way. |

| event | Node | carries | when |
|---|---|---|---|
| `on_hover` | `onHover` | `hover` = { `row`: integer — The row under it. } | The pointer moved over a row. |

Every component also takes `key` and `fresh`.

## Do

- Pass `window` for a live chart: only the last `window` of x shows, and the range glides rather than jumps.
- Give `x_unit` and `y_unit` (s, %, ms); use `x_format = "ago"` for time counted back to now (15 min ago … now) or `"clock"` for Unix seconds.
- Name each series for the legend and the readout.
- Set `focusable` where the samples are worth walking from the keyboard — the one chart a page is about — so the readout can be walked with Left and Right and is announced. Left out, the chart is no Tab stop, and a page of charts does not put one in every Tab.

## Don't

- Draw more than six series: colours repeat after series-6.
- Colour a series with a skin's raw hex: pass a role token (`$kitWarn`) or leave it to the skin's series order.
