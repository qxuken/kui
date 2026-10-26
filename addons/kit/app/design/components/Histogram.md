# Histogram

How a column's values fall into bins.


## When to use

Use a histogram to show the spread of one measure across many rows: request times across 100,000 requests, a latency spread. Bars in the accent, 2px apart, stand on an ink hairline, with the lowest and highest value under its ends in faint. Bins are counted once per channel revision.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `data` | `data` | channel | The rows. |
| `field` | `field` | string | The column to bin. |
| `bins` | `bins` | integer? | How many; 24 when left out. |
| `height` | `height` | number? | In px; 120 when left out. |

Every component also takes `key` and `fresh`.

## Do

- Pick `bins` for the detail the reader needs (24 by default; the gallery's 30).
- Say the unit in the panel's subtitle: the axis shows only the two ends.

## Don't

- Use a histogram for categories: use a bar chart.
