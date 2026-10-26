# Dialog

A modal dialog: a title, its body and its actions; Escape and a press outside ask it to close.


## When to use

Use a dialog to confirm what cannot be undone, or to ask for what an action needs before it runs: discard a draft, delete a note, restart a service, reset every setting. The window dims under the scrim; the dialog is 420px on raised, edged in the ink (the rule width in Ledger and Phosphor, a hairline in Neutral) with the skin's strong shadow. The title row is 40px, the body 16 by 4, the foot 12 with the actions at its end.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `title` | `title` | string | Its title; its name. |
| `width` | `width` | number? | Its width in px; 420 when left out. |

| event | Node | carries | when |
|---|---|---|---|
| `on_dismiss` | `onDismiss` | — | Escape, or a press outside. |

Children:

- the list part is the body.
- `actions` are its buttons, at the end of its foot.

Every component also takes `key` and `fresh`.

## Do

- Title it as the question, naming the thing: Delete “Groceries for Saturday”?, Restart checkout-api?
- Say in the body exactly what will happen: "The note and its 6 checklist items go for good; there is no trash."
- Put the safe way out first and the action last; label both as verbs: Keep it / Delete, Keep running / Restart checkout-api.
- Close it by no longer placing it when `dismiss` arrives.

## Don't

- Use OK and Cancel as labels.
- Use a dialog for news: use a toast or a notice.
- Open a dialog over a dialog.
