# Divider

A rule between groups, as the skin draws regions.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.divider` and kui paints it.

## When to use

Use a divider between two groups in one column or pane: a request bar and the panes under it, two runs of settings. It is a hairline (`$kitHair`) in the rule colour across its parent.

## What you provide

No props of its own; every component also takes `key` and `fresh`.

## Do

- Use it inside a region, between groups that share a frame.
- Let the gap around it come from its parent's gap.

## Don't

- Use dividers to frame a region: that is a panel.
- Stack two dividers, or put one under a toolbar or above a statusbar: they draw their own hairlines.
