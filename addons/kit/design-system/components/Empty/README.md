# Empty

What a view shows with nothing in it: a title, a sentence, and what to do.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/data.tl`. In an app, place `kit.empty` and kui paints it.

## When to use

Use an empty state wherever a list, a table or a pane can have nothing in it: no history yet, no search results, nothing selected, a file that would not open. It is centred with 24px of padding, the title in the title style, the detail in muted at body size, and the actions under it.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `title` | `title` | string | What is missing. |
| `detail` | `detail` | string? | Why, or what fills it. |

Children:

- the list part are its actions.

Every component also takes `key` and `fresh`.

## Do

- Name what is missing in the title: No requests yet, No notes with “risotto”, Nothing selected.
- Say in the detail why, or what fills it: "Start the server, then send one.", "Pick a service in the table to see it here."
- Offer the one action that fills it, as a primary button when it is the view's purpose.

## Don't

- Write "No data" or "Nothing here" alone.
- Put an illustration or emoji in it: Kit draws none.
