# Password

A one-line password field in the skin's field style, masked by kui, with an eye that shows or hides the password.

## When to use

Use a password field for a password, a token or another secret the user types. Hidden, kui draws one bullet per grapheme, refuses copy and cut, accepts paste, keeps no undo history and reads to assistive technology as a password input. While it has the keyboard it asks for secure keyboard entry and turns input-method composition off. The eye reveals the same editor, keeping its text and caret; hiding it masks it again. Leaving the view also hides it: a revealed password does not come back into view when the field returns.

## What you provide

| Lua prop | Node prop | type | what it is |
|---|---|---|---|
| `value` | `value` | string | Its password: yours to keep, from `change`. |
| `label` | `label` | string? | Its label, above it; its name. |
| `hide_label` | `hideLabel` | boolean? | The label names the field but is not drawn: a field whose place says what it is. |
| `placeholder` | `placeholder` | string? | Shown while it is empty. |
| `disabled` | `disabled` | boolean? | Unavailable, including the show/hide eye. |
| `width` | `width` | number? | Its width in px; grows when left out. |
| `focus` | `focus` | integer? | Takes the keyboard each time this number changes while the field is on screen. |
| `autofocus` | `autofocus` | boolean? | Takes the keyboard as it first shows. |
| `error` | `error` | string? | What is wrong with the password, in the danger colour, its edge in danger too, and read after its name. It is shown whole: at the end of the field while there is room, and under it, wrapping, while there is not. An empty string marks the field only. |

| event | Node | carries | when |
|---|---|---|---|
| `on_change` | `onChange` | `change` = { `text`: string — The whole password. } | The password changed. |
| `on_submit` | `onSubmit` | `submit` = { `text`: string — The whole password. } | Enter. |

Every component also takes `key` and `fresh`.

## Do

- Keep the value only where the form needs it, replace it from `change`, and submit on `submit`.
- Name the field with `label`, even when `hide_label` keeps the label off the screen.
- Clear your copy of the password when the form goes away. kui forgets and zeroes a hidden secure editor's own draft when the field is no longer declared.
- Use the eye when the user needs to check what they typed; the field starts hidden.

## Don't

- Show the password elsewhere in the view, in logs or in error messages.
- Treat masking as encryption or as protection from screenshots; it is neither.
- Use an ordinary text field for a secret: it lacks the password editor's copy, history, accessibility and secure-keyboard behavior.
