---
status: proposed
date: 2026-09-25
---

# Stock controls over the roles: checkbox, radio group, switch, slider

> **Proposed 2026-09-25**, from the second bake-off against gpui and
> iced (backlog C45). Both rounds put the same line in kui's column of
> the batteries table: checkbox, radio, switch and slider are "roles
> only, you draw". iced ships all four; gpui ships them through
> `gpui-component`. Every app on kui draws them again: the accessibility
> example draws a radio group, a switch that reads "on"/"off" and two
> sliders; the drag example draws the one slider that follows the
> pointer; the focus example a switch drawn as a button; the devtools
> panel its icon toggles as radios. No two are drawn alike, and only one
> of the sliders can be dragged.

## Context

- **The semantics are already in the core.** `Role::Checkbox`, `Radio`,
  `Switch`, `Slider` and `RadioGroup` are declarable in four bindings
  (`access.rs:52`). `checked` carries a toggle's state and
  `valueNow` / `valueMin` / `valueMax` / `valueText` a slider's
  (`schema.rs`, `spec.rs:877-887`). The four are controls, so they are
  Tab stops with no extra prop and named from their content
  (`access.rs:174-211`). A slider out of its range warns
  (`slider-value-out-of-range`).
- **So is most of the keyboard.** Space and Enter press a focused
  control through its `on_click` payload (`dispatch.rs:318-325`,
  `:449-452`); a `radioGroup` is a composite whose arrows, Home and End
  move focus *and* click the radio they land on (ADR 0007 decisions 8
  and 11). A slider's arrows, and assistive technology's
  Increment / Decrement, reach the app as `{kind:"access",
  action:"increment"|"decrement", tag}` (`composites.rs:262-282`), and
  ADR 0007 decision 13 left the step to the app: "its arrows nudge as
  before, and the app clamps". Home, End and PageUp/PageDown do nothing
  on a slider.
- **What is missing is the drawing and one piece of arithmetic.** A
  toggle's look is a box, a check mark and a label; a switch a track and
  a knob; a slider a track, a fill and a thumb. All of it is quads the
  core already emits. The arithmetic is the slider's: every slider that
  follows the pointer turns `{kind:"drag", x, parent}` into a value, and
  every one that answers the keyboard turns a nudge into a step, clamps
  and snaps. `drag.rs` does the first, `accessibility.rs` the second,
  and no app does both.
- **The rule stands that the core keeps no app state** (ADR 0003): what
  is checked and where a slider sits are the app's model, re-declared
  every frame, exactly like `selected`. The select (F72/F73) kept to it —
  the field opens the core's menu and a choice is one `menu` event on
  the field — and it is the shape to copy.

## Decisions

1. **The four are compositions over the roles, with one definition per
   control in `widgets.rs`**, as the button (`button_spec` /
   `button_with`) and the select (`select_spec` / `select_with`) are:
   `checkbox`, `radio_group` (its radios inside), `switch` and
   `slider`, each with a `*_spec(theme, metrics)` an app can restyle
   and a `*_with` that takes it. They draw from `Theme` roles that exist
   (`sunken`, `border_strong`, `accent`, `on_accent`, `hover`,
   `pressed`, `focus_ring`) and sizes derived from `Metrics::control_text`
   — a box of about 1.15× the control text, a switch two boxes wide — so
   `compact` and `scaled` move them with the stock button. No new metric
   role until an app asks for one; a metric is a change in four bindings
   and in `KuiMetrics`.

2. **The core holds no state and toggles keep the click.** A checkbox,
   radio or switch is declared with its `checked` and an `on_click`
   payload; the app flips its model on the payload, as it does for a
   hand-drawn one today, and Space, Enter, an assistive-technology press
   and a radio group's arrows all arrive as that one event. Nothing new
   in the core for toggles.

3. **A checkbox can be mixed.** A new row, `mixed` (a flag beside
   `checked`), reports AccessKit's `Toggled::Mixed` and draws a dash.
   The select-all box over a list is the case every table with a
   selection column has; `checked` stays a bool so no binding's wire
   changes.

4. **A slider says its step, and the core does the arithmetic.** Two
   rows: `valueStep` (default 1% of the range) and `onChange`, a message
   tag. On a `slider` node with `onChange`, the core turns a press on
   the node into the value under the pointer, a drag into the value
   under it, the arrows and Increment / Decrement into ± one step,
   PageUp / PageDown into ± ten, Home / End into the range's ends —
   clamped to `valueMin..valueMax` and snapped to the step — and emits
   `{kind:"change", value, phase:"move"|"end", tag}`. The app stores
   `value` and re-declares `valueNow`; the core never writes it. A
   slider without `onChange` keeps the `access` nudge exactly as ADR
   0007 decision 13 left it, so no existing slider changes. Pointer
   capture, the drag slop and the parent rect are the drag machinery the
   core already has; the value is `min + (x - rect.x) / rect.w * (max -
   min)`, snapped, along the node's main axis (a column slider is
   vertical, its maximum at the top).

5. **One door per binding, as elements.** The button is an element in
   every binding lowering to `widgets::button_spec`, and the select an
   element lowering to `widgets::select_with`; these follow. JSX
   `<checkbox>`, `<radioGroup>` with `<radio>` children, `<switch>`,
   `<slider>`; Lua `checkbox{}`, `radio_group{}`, `switch{}`,
   `slider{}` in the prelude; C `kui_checkbox`, `kui_radio_group`,
   `kui_switch`, `kui_slider` beside `kui_select`. Four `ELEMENTS` rows,
   Node ops and the frame at v16 (or the next free number when this is
   built), the ABI moving only if a struct does.

6. **The corpus pins them.** One scene, `stock-controls`: the four at
   rest, a mixed checkbox, a checked radio, a switch on, a slider at
   30% — and a drive of the slider (a press at 70%, a drag, ArrowRight,
   End) whose `change` rows every adapter must produce. An example under
   `widgets/controls.rs` and its Node twin show them.

## Considered options

- **Leave them to apps, with a howto.** Two rounds of the same row in
  the comparison, and five hand-drawn variants in this repo, say the
  howto is not enough. The drawing is small; the slider's arithmetic is
  what apps get wrong (none here does both pointer and keyboard).
- **Keep the slider's nudge and add only the drawing.** Every stock
  slider would still need the app to know the step, clamp and snap, and
  the pointer half would still be the app's `drag` arithmetic. A stock
  slider that cannot be dragged without app code is not stock.
- **State in the core** (a retained checked / value per key). Breaks
  ADR 0003's rule for the one widget family where the model is the
  whole point; the select showed the stateless shape works.
- **A tri-state `checked` enum instead of `mixed`.** Changes the
  `checked` row's kind on every wire for a state one control uses.

## Consequences

- Apps delete their drawn toggles and their slider arithmetic; the
  accessibility, drag and focus examples and the devtools' icon toggles
  are the first callers to move.
- `onChange` is the first event the core raises with a *value* it
  computed from app-declared props. The rule it keeps is that the value
  is proposed, not applied: nothing moves until the app re-declares
  `valueNow`.
- Not in this ADR: a progress bar (a `progressIndicator` role appended
  per ADR 0006, and a stock bar over it — the closed archive declined it
  until a report asked, and none has), a spinner (a `fragment` draws
  one), a tabs widget (the `tabList` composite exists and the look is
  the app's), a number field (an `edit` plus a slider).
