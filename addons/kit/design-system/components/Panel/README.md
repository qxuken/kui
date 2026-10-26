# Panel

A titled region: a header with the title and its actions, the body, an optional footer.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/layout.tl`. In an app, place `kit.panel` and kui paints it.

## When to use

Use a panel for each region of a tool window that has a name: a chart, a list of services, a selected item's details, a group of settings. The frame is the skin's region rule (Ledger's 2px ink, a hairline in Neutral, Phosphor's 1px ink), and the title is labelled as the skin labels regions.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `title` | `title` | string? | Its title, as the skin labels regions. |
| `subtitle` | `subtitle` | string? | A line under the title. |
| `scroll` | `scroll` | boolean? | The body scrolls rather than grows. |
| `grow` | `grow` | boolean? | Takes the room its parent leaves. |
| `pad` | `pad` | number? | The body's padding in px; 10 when left out, 0 for a body that brings its own or is often empty. |

Children:

- the list part is the body.
- `actions` goes in the header, after the title.
- `footer` goes under the body.

Every component also takes `key` and `fresh`.

## Do

- Put a status badge or a small action in `actions`; a search or a segmented control fits too, since the head is a control's height plus 4px either side.
- Pass `pad = 0` when the body brings its own inset (a table, a list of rows) or is often empty.
- Give the panel `grow` when it should take the room its column leaves, and `scroll` when its body should scroll rather than push the window.
- Use `subtitle` for the unit or scope: "requests a second, by minutes ago".

## Don't

- Nest panels inside panels: a panel's frame is a region boundary, and two frames read as a cage.
- Write the title in capitals yourself: the skin decides the casing (Ledger and Phosphor capitalise, Neutral does not).
- Put primary actions in the footer of a panel that is not a form: they belong in the toolbar or the dialog's foot.
