# Notice

A message in the flow of the page: an error to act on, a warning, a note; its actions after it.


## When to use

Use a notice for something the user should know where they are looking: a request that failed, a service that is degraded, why a tab is empty. It sits in the sunken well with a hairline edge in its tone, a 14px icon (check for ok, info for neutral, info and accent, alert otherwise) and the message in the tone's colour (neutral in fg). A danger notice is read assertively; the others politely.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `message` | `message` | string | What happened, or what to know. |
| `tone` | `tone` | Tone? (neutral · info · accent · ok · warn · danger) | Danger, warn, ok, accent, info or neutral (the default). |
| `title` | `title` | string? | A bold first line. |

Children:

- the list part are its actions, at its end.

Every component also takes `key` and `fresh`.

## Do

- Use `title` for the short headline of an error (Could not save) and `message` for the cause (The disk is full.).
- Put the way out as a button child (Retry); actions wrap under the message when the row is narrow.
- Write the message as a fact, with the numbers: "p95 above 800 ms since the 13:47 deploy (3.18.2)".

## Don't

- Use a notice for news that passes: use a toast.
- Stack several notices of the same tone; merge them.
- Blame the user: "The disk is full", not "You filled the disk".
