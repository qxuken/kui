# Tabs

A strip of tabs over the body of the one chosen.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.tabs` and kui paints it.

## When to use

Use tabs to switch between views of the same thing that are never needed side by side: a request's params, headers and body; a service's events, errors and latency spread. The chosen tab's label is fg with the accent as a rule-width underline; the others are muted and wash on hover.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `tabs` | `tabs` | {Tab} | The tabs, in order. |
| `value` | `value` | string | The one shown: yours to keep. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `value`: string — Its value. } | Another tab was chosen. |

Children:

- the list part is the chosen tab's body.
- `trailing` ends the strip, at the far side: what acts on the chosen tab.

Every component also takes `key` and `fresh`.

## Do

- Give a tab a `count` when it holds a list: the count is faint, in the data face.
- Put what acts on the chosen tab (reload it, close it) in `trailing`: it is never squeezed, the labels give first.
- Keep the value in your model and change it from `change`.

## Don't

- Use tabs as a view switch for a chart or a list: that is a segmented control.
- Put more tabs than fit at the window's least width: the labels ellipsize.
