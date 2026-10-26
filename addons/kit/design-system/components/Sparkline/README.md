# Sparkline

A line with no axes, for a trend inline.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/charts.tl`. In an app, place `kit.sparkline` and kui paints it.

## When to use

Use a sparkline beside a number or a name to show which way it has been going: a stat card's last fifteen minutes, a service's requests in its detail. It is a 1.5px line, 80 by 18 by default, in the accent unless given a colour; under the pointer the value reads out as a hint with a 6px dot on its sample.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `data` | `data` | channel | The values. |
| `field` | `field` | string | The column to draw. |
| `color` | `color` | color? | The accent when left out. |
| `width` | `width` | number? | In px; 80 when left out. |
| `height` | `height` | number? | In px; 18 when left out. |
| `unit` | `unit` | string? | After the value under the pointer: `ms`, `%`. |
| `readout` | `readout` | boolean? | The value under the pointer, as a hint and a dot on its sample; on when left out. |

Every component also takes `key` and `fresh`.

## Do

- Pair it with the number it trends: a stat above it, a label beside it.
- Colour it by what it measures: `$kitWarn` for a latency, `$kitDanger` for an error rate, as Watchtower does.
- Give `unit` so the readout says "412 ms", not "412".

## Don't

- Use a sparkline where the reader needs values or a scale: use a line chart.
- Rely on Ledger's default yellow line on light paper (1.41:1 on panel): pass a colour when the trend matters.
