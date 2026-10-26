# Search

A search field: a glass, the query, and a clear button while it has one.

*Static rendition:* the preview is plain HTML and CSS built on this system's tokens, recreating what Kit draws from `addons/kit/src/inputs.tl`. In an app, place `kit.search` and kui paints it.

## When to use

Use a search field to filter what a view shows: requests, notes, services, settings. A 13px glass in muted leads, a count in the data face follows the query, and an × clears it (so does Escape).

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | string | The query: yours to keep. |
| `placeholder` | `placeholder` | string? | Shown while it is empty; `Search` when left out. |
| `label` | `label` | string? | Its name, read and not drawn: what it searches (the placeholder when left out). |
| `count` | `count` | string? | A count after the query: `12 of 340`. |
| `focus` | `focus` | integer? | Takes the keyboard each time this number changes while it is on screen (⌘F). |
| `autofocus` | `autofocus` | boolean? | Takes the keyboard as it first shows. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `text`: string — The query. } | The query changed; cleared is the empty string. |
| `on_submit` | `onSubmit` | `submit` = { `text`: string — The query. } | Enter. |

Every component also takes `key` and `fresh`.

## Do

- Write the placeholder as what it searches: Search requests, Service or team, Filter by path or method.
- Show `count` while there is a query: "12 of 340", "3 found".
- Bump `focus` from ⌘F so the keyboard lands in it.
- Filter as the query changes; submit only for searches that cost a request.

## Don't

- Leave the placeholder as the bare word when the view searches something specific.
- Pair it with a Search button that does what typing already does.
