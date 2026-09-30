# Examples

Every Rust example in the kui repository, one page each, with its whole
source. An example has one subject and shows it in every state it has.
Each runs inside the repository's devtools harness (`kui-devtools`,
which is not published): in your own project, take the view and the
`on_event` and launch them with `kui_native::app` as the book does.
The comment at the top of each file says what it shows and how the
repository runs it.

- [`apps/`](apps.md): How it composes: an app owning its state, keymap or pane tree, touching whatever it needs.
- [`widgets/`](widgets.md): One element or one stock widget, in every state it has.
- [`features/`](features.md): One cross-cutting behaviour, with exactly the widgets it touches.
- [`tools/`](tools.md): Registered as an example for want of a better slot, and not one: a corpus dump.
