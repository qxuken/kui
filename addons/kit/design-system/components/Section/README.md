# Section

A heading that folds what is under it.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.section` and kui paints it.

## When to use

Use a section to group rows under a heading the user can fold: advanced settings, a note's checklist, a group of results. The heading is a button with a 12px chevron in muted, the title in the label style and an optional faint count; the body sits 18px in, under the title.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `title` | `title` | string | The heading. |
| `collapsed` | `collapsed` | boolean? | Folded: yours to keep. |
| `count` | `count` | integer? | A count beside the heading. |
| `indent` | `indent` | number? | The body's left inset in px; 18 when left out, under the title. 0 for rows with their own inset that line up with `leading`. |

| event | Node | carries | when |
|---|---|---|---|
| `on_toggle` | `onToggle` | `toggle` = { `collapsed`: boolean — What it asks to be. } | The heading was pressed. |

Children:

- the list part is what it folds.
- `leading` goes before the heading, beside it rather than in it: a box that ticks the whole section.
- `actions` goes after the heading, at the far side: a status, an action on the whole section.

Every component also takes `key` and `fresh`.

## Do

- Pass `collapsed` and keep it in your model when the fold should survive a restart; leave it out and Kit keeps it per instance.
- Put a box that ticks the whole section in `leading`, and a status or an action on the section in `actions`: they stay controls of their own.
- Pass `indent = 0` for rows that bring their own inset and line up with `leading`.

## Don't

- Fold the only thing on the page.
- Put a section's actions inside the title text.
