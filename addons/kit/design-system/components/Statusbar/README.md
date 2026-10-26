# Statusbar

A thin row at a window's foot for state: counts, the connection, the clock.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.statusbar` and kui paints it.

## When to use

Use a statusbar at the very bottom of a window for facts that are always true of the whole window: how many rows, which environment, whether the server is up, when it last saved. It is 22px tall on the side colour, with a hairline above, items 14px apart.

## What you provide


Children:

- the list part fills it, in order.
- `trailing` ends it, at the far side.

Every component also takes `key` and `fresh`.

## Do

- Write each item as a short fact in muted text: "12 notes in 4 notebooks", "all changes saved".
- Use a badge with a dot for a connection's state, in its tone.
- Put what refers to the whole app (the data source, a total) in `trailing`.

## Don't

- Put buttons that change data here; a switch for a mode the window is in (Serve on :3050) is the most it holds.
- Repeat what the toolbar already shows.
- Let it grow taller than one line.
