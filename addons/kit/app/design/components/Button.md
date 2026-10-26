# Button

A button. Space, Enter and assistive technology press it as a click does.


## When to use

Use a button for an action. Default is a quiet fill outlined in the ink where the skin outlines controls (Ledger, Phosphor) or a sunken chip in Neutral. Primary, in the accent with the skin's shadow, is the one thing the view is for. Ghost has no fill until hovered: secondary actions beside a primary (Revert beside Save). Danger fills with the danger colour for an action that destroys. Success, the ok colour on the quiet fill, confirms what just happened (Copied), briefly.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `label` | `label` | string | The text on it; its accessible name unless `name` says more. |
| `name` | `name` | string? | Its accessible name when that says more than the label; start it with the label's words (Clear → Clear the selection). |
| `variant` | `variant` | Variant? (default · primary · ghost · danger · success) | Default when left out. |
| `icon` | `icon` | string? | An icon's name before the label (Kit's icon set). |
| `shortcut` | `shortcut` | string? | The chord, shown faintly after the label: `⌘R`. |
| `disabled` | `disabled` | boolean? | Drawn and read as unavailable; presses nothing. |
| `pressed` | `pressed` | boolean? | A toggle that is on (a panel shown): drawn selected and read as such. |
| `tooltip` | `tooltip` | string? | A hint under the pointer — why it is disabled, say. |

| event | Node | carries | when |
|---|---|---|---|
| `on_press` | `onPress` | — | Pressed. |

Every component also takes `key` and `fresh`.

## Do

- Write the label as a verb, sentence case, short: Run, Save, Keep editing, Restart checkout-api. The skin capitalises it where it should.
- End a label with an ellipsis when the press opens something that asks more: Restart…, Scale…, Rename….
- Show the chord in `shortcut` (⌘S): faint beside a quiet label, the label's own ink on a fill.
- Use `pressed` for a toggle that shows a panel (Theme): it is drawn in the selection and read as selected.
- Say why a disabled button is disabled in `tooltip`.
- Pair a danger button with a quiet way out in a dialog: Keep it beside Delete.

## Don't

- Put two primary buttons in one view.
- Use danger for an action that can be undone with Undo; use a default button and a toast with Undo.
- Fade a primary button to show it is unavailable: a disabled button of any variant is drawn as default at half opacity.
- Put an icon after the label, or an icon of another colour than the label.
