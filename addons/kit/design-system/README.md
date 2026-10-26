Kit is kui's UI kit for dense tool windows: HTTP clients, ops dashboards, settings windows, note keepers. Build every surface from Kit's components and paint anything of your own with Kit's role tokens, so a skin or appearance change reaches it. Treat Ledger light (`ledger-light`) as the default; every rule here holds in all six themes.

## The three skins

- **Ledger** (the default): paper and ink. Square corners (`kit-radius-ledger`, 0), 2px region rules in `ink` (`kit-rule-w-ledger`), bold capitals on controls and headers, a hard 3px offset `shadow` under the one primary action and every float, one yellow `accent` used as a highlighter. Faces: Space Grotesk for the UI, IBM Plex Mono for data.
- **Neutral**: quiet chrome. 6px corners (`kit-radius-neutral`), hairlines in `rule` instead of ink frames, sentence-case regular labels, controls sitting on `sunken` with no border, a soft dropped `shadow` on floats, one blue `accent`. Faces: Inter and JetBrains Mono.
- **Phosphor**: a terminal. One mono face (IBM Plex Mono) for everything, green `ink` on near-black in dark and on a pale screen in light, 1px rules, square corners, and a glow (`shadow`, 8px blur, no offset) on strong things only.

Pick the skin with `kit.settings`; never mix two skins' values in one window. Use the theme id's skin and appearance together: `ledger-light`, `ledger-dark`, `neutral-light`, `neutral-dark`, `phosphor-light`, `phosphor-dark`.

## Content fundamentals

Kit's voice is a careful colleague at a terminal: plain, specific, unexcited. Write it that way.

- **Labels are verbs, short, sentence case.** Run, Save, Revert, Keep editing, Restart checkout-api, Start the server. Never write capitals yourself: Ledger and Phosphor capitalise labels in the skin (`ledger-label`), Neutral does not.
- **An ellipsis means more is asked.** Restart…, Scale…, Rename…, Choose…. Use the single character `…`, never three dots. An action that runs at once has none.
- **Name the thing and the consequence.** Dialog titles are the question with the object in it: “Discard the draft?”, “Delete “Groceries for Saturday”?”, “Restart checkout-api?”, “Reset every setting?”. The body says exactly what happens: “Three edits since the last save will be lost.”, “The note goes for good; there is no trash.”, “All 4 instances are cycled one at a time; each drains its in-flight requests for up to 30 s first.”
- **The safe way out is a verb too.** Keep editing, Keep it, Keep running. Never OK and Cancel.
- **Errors say what is wrong or how to fix it, as a fact.** “Three numbers: 1.4.0”, “Could not save” over “The disk is full.”, “The request failed: 500.”, “It is down because its upstream is: a restart may not bring it back.” Never blame the reader, never apologise, never use an exclamation mark.
- **Toasts report what happened, past tense, with the count.** “Saved to disk.”, “Deleted 3 rows.”, “Restored the 3 rows.”, “A new version is ready.” Offer Undo as the one action on a destructive toast.
- **Empty states name what is missing, then what fills it.** “No requests yet” / “Start the server, then send one.”; “No history” / “Requests you send land here, newest first.”; “Nothing selected” / “Pick a service in the table to see it here.”
- **Numbers carry their unit and their comparison.** “325 ms”, “0.90%”, “2,106”, “+4% vs 15 min ago”, “steady vs 15 min ago”, “12 of 340”. Put a space before a word unit (410 ms, 20 MB/s) and none before `%`. Use thousands separators in prose and stats. Use 24-hour times (13:47:02).
- **Status lines are facts joined by a middle dot.** “12 services · 3 need attention”, “Larder on 127.0.0.1:3050 · 41 requests served”, “checkout team · on call m.okafor”. Use an en dash for a range: “0 ms – 50 ms”.
- **Hints address the reader plainly.** “Pick a note, or start a new one.”, “Click a row, or Tab to the table and use the arrows.”, “Try a word from its help, like “proxy” or “hotspot”.” Use curly quotes around quoted words.
- **No emoji, anywhere.** Status is a word plus a tone and, where it helps, a dot or Kit's own icon.
- **Badges are written as given** (`200`, `degraded`, `v0.1.0-alpha.70`): data is never recased.

## Visual foundations

### Colour

- Paint every surface from the role tokens: `bg` for the window, `side` for the toolbar, sidebar and statusbar, `panel` for regions and tables, `raised` for floats, `sunken` for wells (text areas, code, logs, notices, badges, Neutral's fields).
- Set body text in `fg`, secondary text in `muted`, counts, times, shortcuts and chart ticks in `faint`. Kit moves these three until each reads at 4.5:1 or more on `bg`, `side`, `panel`, `raised` and `sunken`; keep text on those five grounds.
- Use `accent` for the one loud thing in view: the primary button, a ticked box, an on switch, the chosen tab's underline, a progress or slider fill, bars. Put `on-accent` on it. When the accent must be text (an accent notice, its toast bar), use `accent-text`: Ledger light's yellow becomes an olive there.
- Show selection with `sel` and `sel-fg` (Ledger's selection is its yellow), the chosen segment with `chosen` and `chosen-fg`.
- Use the tones for meaning only: `ok`, `warn`, `danger`, and `focus` as the info tone. Always pair a tone with a word; a dot or an icon may back it up.
- Use `danger` as a fill only for the destructive button, with `on-danger`.
- Dim the window under a dialog with `scrim`.
- Charts: series in `series-1` to `series-6` in order, the area under the first line in `area-1`, waterfall spans in `group-1` to `group-6` with `fg` labels.
- Hover and press: the washes `hover` and `press` over quiet things; opaque shades (`accent-hover`, `accent-press`, `danger-hover`, `danger-press`, `panel-hover` and the rest) over fills, so nothing under a fill shows through.
- Kit's own pairs that miss a floor are kept exact and named in their tokens' notes: `on-accent` on `accent-hover`/`accent-press` in Neutral and Phosphor, `on-danger` on `danger-press`, some pill tones on `press`, Ledger light's yellow as a line or bar on `panel` (1.41:1), Neutral's grey `ink` as an edge (1.65:1), Neutral light's `series-5` (2.36:1). Do not build new meaning on those pairs.

### Type

- Set UI text in the skin's UI face (`ui`, `neutral-ui` or `phosphor`) and every id, timing, number, path, code and log line in the data face (`data`, `neutral-data` or `phosphor`).
- Use the skin's three sizes and nothing between: body at `$kitSize` (`ledger-body`, 12px), labels at `$kitSizeLabel` (`ledger-label`, 11px), titles at `$kitSizeTitle` (`ledger-title`, 13px bold). Line height is 1.35 times the size, rounded (16, 15, 18).
- Labels on controls, tabs and panel and table headers are bold capitals in Ledger and Phosphor (`ledger-label`, `phosphor-label`), regular sentence case in Neutral (`neutral-label`). A field's, slider's or stat's label is that caption in `muted`.
- Headline numbers are `stat`: 22px bold in the data face. Chart labels are `tick`: 10px data face in `faint`.
- Kit ships no font files. Space Grotesk, IBM Plex Sans, IBM Plex Mono, Inter and JetBrains Mono are on Google Fonts; each stack falls back to the system face.

### Spacing and density

- Size controls by `kit-ctl` (26px) and rows by `kit-row` (22px); compact density is `kit-ctl-compact` (22px) and `kit-row-compact` (20px). Nothing else moves with density.
- Pad regions by `pad-panel` (10px) with `gap-panel` (8px) between children; pad controls sideways by `pad-ctl` (10px), with `gap-ctl` (6px) from icon to label.
- Page padding and the gap between cards is `pad-page` (16px). The toolbar is at least `toolbar-h` (38px), padded 8 by 6; the statusbar is `statusbar-h` (22px), items `gap-status` (14px) apart.
- Copy Kit's numbers exactly (a badge is 18px with 6px sides, a menu item 22px with 8px sides); do not round them to a grid.

### Radii

- Use `$kitRadius` for every control, region, float and well: 0 in Ledger and Phosphor, `kit-radius-neutral` (6px) in Neutral.
- Marks take their own: radios are round in every skin (`radius-radio`); checkboxes (`radius-tick`), pills (`radius-pill`), switch tracks (`radius-track`) and slider handles (`radius-handle`) round only where the skin rounds.

### Rules and borders

- Frame a region (panel, outlined control, float) with the region rule: `$kitRuleW` in `ink` (Ledger 2px, Phosphor 1px), a `kit-hair` in `rule` in Neutral.
- Separate rows, headers, a toolbar's foot and a tab strip with a `kit-hair` in `rule`. Rules are decoration; never let a `rule` hairline be the only sign of a boundary that matters.
- Draw a control's edge over its box: it takes no room, so a thicker focused edge moves nothing.
- Underline fields in Ledger and Phosphor: `faint` hairline at rest, `ink` at `$kitRuleW` focused, `danger` in error. Box them in Neutral: `sunken` with a hairline in `rule`, `focus` or `danger`.

### Shadows

- Give `shadow` to the primary button and to floats (dialog, popover, menu, toast) only. Never shadow a panel or a card.
- Ledger's is hard: 3px down and right in `ink`, no blur. Under the pointer the face moves up and left by a pixel and the shadow grows to `shadow-lift`, its far edge fixed; held, the face drops onto the shadow's place and the shadow goes. The shades stay opaque so the shadow never shows through.
- Neutral's is soft (0 4px 16px, black at 18%); Phosphor's is a glow in `ink` with no offset, and only strong things glow (a select's list gets `shadow-quiet`, which is none in Phosphor).
- Turning shadows off in settings removes them all; keep every edge so nothing depends on a shadow to be seen.

### Motion

- Ease short and plain: a switch knob and a dialog's fade 140ms, a popover or menu 100ms, a toast 180ms (in from 12px below, out 32px to the side, the stack sliding into its room), a chart gutter's glide 160ms, a progress value 320ms on a snappy spring, the skeleton's shimmer one 1500ms sweep for every bar.
- Under reduced motion, everything Kit eases jumps, loops hold still and shaders that read `time` stop. Do the same for your own nodes.

### States and focus

- Hover: the `hover` wash on quiet things, the opaque `-hover` shade on fills. Press: `press` or the `-press` shade.
- Selected: `sel` with `sel-fg`; a pressed toggle button is drawn the same way.
- Disabled: the whole control at `disabled-opacity` (0.5) and, for any variant, drawn as default: never a faded accent.
- Focus: a `focus-ring-w` (2px) ring in `focus`, `focus-ring-gap` (2px) outside the control. Checkboxes, radios and switches ring their mark, not their row. `focus` reads at 4.5:1 on every surface, so the ring clears 3:1 everywhere.

### Layout

- Build a window as a `Toolbar` on top, a `Split` of a `side`-coloured sidebar and the page, `Panel`s on `bg` in the page, and a `Statusbar` at the foot. Floats (`Dialog`, `Popover`, `Menu`, `Toast`) sit above.
- Let rows decide their own columns: as many cards a line as fit at a least width (`min_column`), and a field and its label stacking under 360px.
- Keep one primary button per view, in the toolbar or a dialog's foot.

## Iconography

- Use Kit's own icon set (`addons/kit/src/icons.tl`): 40 icons drawn in a 24-unit frame as 2-unit strokes with round caps and joins, `fill="none"`, `stroke="currentColor"`. Media glyphs (`play`, `stop`) are solid so they never read as a box to tick.
- The set: `alert`, `bolt`, `check`, `chevron_down`, `chevron_left`, `chevron_right`, `chevron_up`, `circle`, `clock`, `code`, `copy`, `database`, `dot`, `download`, `external`, `eye`, `file`, `filter`, `fit`, `folder`, `globe`, `info`, `link`, `menu`, `minus`, `more`, `pause`, `play`, `plus`, `refresh`, `reset`, `search`, `sliders`, `stop`, `terminal`, `trash`, `upload`, `x`, `zoom_in`, `zoom_out`.
- Size icons at 14px by default, 13px beside a button label or in a field, 12px for chevrons, dismiss crosses and error marks, 11px inside a checkbox or a sort header.
- Draw an icon in the colour of the text beside it: a button's icon takes its label's ink, a chevron `muted`, an error mark `danger`. An icon is decoration: the control it sits in carries the name.
- Never substitute emoji or another icon family. A name Kit does not have draws an empty square of the same size.
- Kit has no logo or mark. Set the name “Kit” in plain type: the UI face, bold, in `fg`, as the Kit app's toolbar does.
