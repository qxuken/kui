# Select

A field that opens a menu of options.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.select` and kui paints it.

## When to use

Use a select to choose one of several options that need not all be visible: a mode, a language, an environment, a skin. Closed, it is a quiet button with the chosen label in body text and a 12px chevron; open, its list floats on raised with a hairline ink edge, the chosen option in the selection, lifted so the chosen option sits over the field as a native pop-up's does.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `options` | `options` | {Choice} | The options, in order. |
| `value` | `value` | string? | The one chosen: yours to keep. |
| `placeholder` | `placeholder` | string? | Shown with nothing chosen. |
| `label` | `label` | string? | Its label, above it. |
| `width` | `width` | number? | Its width in px. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `value`: string — Its value. } | An option was chosen. |

Every component also takes `key` and `fresh`.

## Do

- Keep the value in your model; a pick arrives as `change`.
- Give `placeholder` for an unchosen select; it is drawn faint.
- Mark an option that exists but cannot be chosen now with `disabled`.
- Use `width` as an upper bound; the select gives way in a narrow row and ellipsizes its choice.

## Don't

- Use a select for two or three options that fit side by side: use a segmented control or radios.
- Put actions in a select: that is a menu.
