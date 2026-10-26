# Toaster

Toasts stacked in the window's corner, the newest at the foot, each sliding into the room one that goes leaves.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/overlays.tl`. In an app, place `kit.toaster` and kui paints it.

## When to use

Use a toaster when more than one toast can be up at once: an app's own news from every page, a save then an undo. It floats 16px in from the window's bottom-right corner, its toasts 8px apart and aligned to the end; the newest is at the foot, and when one goes the others slide into its room. Place it only while it has toasts; it leaves as a toast does.

## What you provide


Children:

- the list part are the toasts, each placed with `inline`.

Every component also takes `key` and `fresh`.

## Do

- Keep the stack in your model and give each toast a stable `key` and `inline = true`.
- Drop a toast from the stack on its `dismiss`.

## Don't

- Place single toasts beside a toaster: they would float over one another.
- Let the stack grow without bound: the oldest go by themselves after their duration.
