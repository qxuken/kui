# Toast

A message in a window's corner: shown for `duration` (5 s) or until dismissed (`duration = 0`), in its tone — info, success, warning or error — as a bar down its edge and its icon; it slides out as it goes.


## When to use

Use a toast to say that something happened away from where the user is looking, or that an action finished: saved, deleted, a worker stopped, a request failed in the background. It floats 16px in from the window's bottom-right corner on the float surface, 320 to 480px wide, with a 4px bar down its leading edge in the tone (info in focus blue, neutral in faint), the tone's icon, the message, one optional action and a dismiss ×.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `message` | `message` | string | What happened. |
| `tone` | `tone` | Tone? (neutral · info · accent · ok · warn · danger) | info (the default, in the skin's focus blue), neutral (grey), accent, ok (success), warn (warning) or danger (error). |
| `action` | `action` | string? | One action on what happened, a button beside the message: `Undo`. |
| `action_name` | `actionName` | string? | The action's accessible name when it says more than its label (Undo → Undo the delete). |
| `duration` | `duration` | number? | How long it stays, in ms, counted again from when the pointer leaves it; 5000 when left out, 0 to keep it until dismissed. |
| `inline` | `inline` | boolean? | One of a `toaster`'s stack rather than floating in the corner on its own. |

| event | Node | carries | when |
|---|---|---|---|
| `on_dismiss` | `onDismiss` | — | Its close button, or its `duration` ran out. |
| `on_action` | `onAction` | — | Its action was pressed. |

Every component also takes `key` and `fresh`.

## Do

- Write the message as what happened, past tense, with the count: Saved to disk., Deleted 3 rows., Restored the 3 rows.
- Offer Undo as the `action` for anything destructive, and keep that toast longer (8000 ms).
- Keep an error until it is dismissed (`duration = 0`); let the others go after 5 s.
- Stack several in a `toaster`, each with `inline`.

## Don't

- Put a question in a toast: use a dialog.
- Toast what the user can already see happen in front of them.
- Give a toast more than one action.
