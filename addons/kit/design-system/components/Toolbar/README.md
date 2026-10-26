# Toolbar

A row of controls at the control height, its trailing group pushed to the far side.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.toolbar` and kui paints it.

## When to use

Use one toolbar across the top of a window: the window's name, the controls that change what the whole window shows (environment, time range, search), and at the far side the state and the one action the window is for. It sits on the side colour with a hairline under it.

## What you provide


Children:

- the list part starts it.
- `trailing` ends it, at the far side.

Every component also takes `key` and `fresh`.

## Do

- Put the window's name first, in the title style (`$kitSizeTitle`, bold, fg).
- Put live state, a skin or appearance picker and the primary action in `trailing`.
- Let it wrap: when the window is too narrow for both groups, the trailing group moves to a line of its own.

## Don't

- Put more than one primary button in it.
- Add your own spacer between the two groups: the unnamed group already takes the room; a spacer inside it pushes what follows to the group's end.
- Use it as a panel header: a panel has its own head with `actions`.
