# How do I…

A task index. [`props.md`](props.md) is the reference — every prop, element
and event, sorted by name — and this page is the other door: the question a
developer arrives with, two sentences of answer, and the row, the release
entry or the ADR that says the rest.

It exists because both field-report apps filed a wish for something that had
already shipped and was documented in their own `node_modules`. The examples
are JSX; the Lua and C spellings of every row named here are in the same
`props.md` table, one column over.

- [Draw and animate](#draw-and-animate)
- [Interaction, focus and reading](#interaction-focus-and-reading)
- [Sound and effects](#sound-and-effects)
- [Test it](#test-it)
- [Ship and upgrade](#ship-and-upgrade)

## Draw and animate

### How do I animate a removal?

Give the node a stable `key` and a `transition`, then declare `exit` —
where it should end up — and the frame after the view stops declaring it the
subtree is copied out of the last frame that had it and replayed frozen in
its place, inert, while those slots ease. A frame that removes more than 512
nodes declaring `exit` animates none of them, because the removal is judged
whole rather than half-animated, and the `exit-budget` warning names the
frame's count.

[`exit` row](props.md#container-props) ·
[ADR 0012](adr/0012-the-exit-budget.md) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07)

### How do I draw a connector between two boxes?

`<line from={[x, y]} to={[x, y]} width color/>` is one round-capped stroke,
`<line points={[[x, y], …]} curve/>` a polyline or a smooth curve through
the points; a line is always a float in its parent's box space, sized to its
own bounding box, so it takes no room in a row or column. With `onClick`,
`onDrag` or `hoverable` it is hit by its stroke, at least 4 px wide
([ADR 0026](adr/0026-hit-testing-by-shape.md)). Budget its quads: one per segment, and a curve is flattened
in the core at one piece per 6 logical px of chord, at most 32 per span — so
a nine-point curve over ~50 px spans is ~60 quads.

[`line` element](props.md#elements) ·
[ADR 0010](adr/0010-a-segment-primitive.md) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I make a tab bar whose tabs stop shrinking at their labels?

Give every tab `width="grow"` and `minWidth="fit"`: they split the bar evenly
while they fit and sit at their own label's width once they do not, which is
CSS's `flex: 1 0 auto`. It is opt-in rather than the default because a fit
width is the *unwrapped* one — a paragraph in a grow column would stop
wrapping under it.

[`minWidth` row](props.md#container-props) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07)

### How do I push a toolbar's last button to the far end, or space items evenly?

`mainAlign="spaceBetween"` on the row: the free space goes between the
children and none at the ends, so the first sits at the start and the last
at the end. `spaceAround` gives each child an equal share split to its two
sides, and `spaceEvenly` makes every gap and both ends equal. The spread is
added to `gap`, and none is dealt when nothing is free — a `grow` child takes
the space, and a row that overflows keeps its gaps. To push only the last of
several, a `<box width="grow"/>` spacer before it is still the way.

### How do I set a label and a large number on one line?

`crossAlign="baseline"` on the row: the first baselines of the children's
text line up, so `Total` in 13 px and `1,284` in 32 px read as one line
instead of two tops or two bottoms. Each child's baseline is its first
text's, found down its first-child chain through any wrapper box; a child
with no text — an icon — aligns by its bottom edge, and a `grow` or percent
height fills the row from its top. A fit-height row grows to hold the
aligned children. On a column it is `start`, with a warning.

### How do I keep a box 16:9, or square, whatever width it gets?

`aspectRatio={16/9}` with the height left `fit`: the height is the final
width over the ratio, so `width="grow"` and a ratio keeps its shape as the
window resizes (cap it with `maxWidth`). Under a fixed height a fit width is
that height times the ratio, which is how `height={40} aspectRatio={1}`
makes a square. The derived axis is not fitted to the children, which
overflow it — `minHeight="fit"` floors it at them — and with both axes
declared the ratio has nothing to set and warns (`aspect-ignored`).
[`examples/rust/features/align.rs`](../examples/rust/features/align.rs)
shows all three.

### How do I line up the columns of a key/value list, or any table?

`<box dir="table">`, `grid { }` (Lua's `table` is its own), a `KuiSpec`
with `dir = KUI_TABLE`, `NodeSpec::table()`: a column whose rows'
children line up in columns — the nth in-flow child of every row is
column n (a float in a row is not a cell), and a column is as wide as
its widest cell — so a label column sits at
its longest label with no width picked by hand and nothing measured
([ADR 0033](adr/0033-a-table-is-a-column-whose-cells-align.md)). Put
the rows in as rows: `<box dir="row" width="grow" gap={8}><text>{k}
</text><text>{v}</text></box>`, a bare text a cell held to its column.
The table itself needs no width — its `fit` is its columns', whatever
the rows' sizing, so that snippet with no width on the table is the
aligned list, as wide as its longest key and its longest value. A
cell's `width` sizes its column — `fit` (the default) and a number are
content, `grow` makes the whole column grow with the table, a percent
takes its cut of the row — and its `minWidth` / `maxWidth` clamp the
column; a `grow` cell in a header row is enough to make its column the
one that stretches. The rows keep everything a row has: their own `gap`
between cells, padding, `bg`, `hoverBg`, `onClick` and `label` — so a
clickable settings row, a hovered inspector row and a header that sorts
are rows with those props — and a number right-aligns in its column
with a `mainAlign="end"` row around the text. Only a `row` straight
under the table is a row: a `column` there (a section heading over its
own rows), a text or a nested table is a child with its own width, and
its children are not cells. An image straight in a row is a cell held
to its column like a text is — its box the column wide and its own
aspect tall, the pixels meeting it by the image's `fit` — so an icon
that must keep its width sits in a box. Everything else is the
column's: `gap` is between rows, `scrollY` scrolls them. Fit columns
that overflow the row are compressed largest first, as a row's children
are, unless the table scrolls x, where the rows are at least as wide as
the columns and the table scrolls to them; a row of a table never
wraps. The devtools' Facts, tokens, legend and inspector lists are
tables.

[`table` element](props.md#elements) ·
[examples/rust/widgets/table.rs](../examples/rust/widgets/table.rs) ·
[examples/node/widgets/table.tsx](../examples/node/widgets/table.tsx)

### How do I clamp text to one line, or to three with an ellipsis?

`wrap="none"` breaks nowhere, `maxLines={3}` lays out at most three lines,
and `ellipsis` ends the last one with an ellipsis when the text is cut off.
All three are text props, so they read on `<text>` and `<edit>`, and a span's
styles do not interrupt them — spans shape as one paragraph.

[`wrap` / `maxLines` / `ellipsis` rows](props.md#text-props)

### How do I underline a diagnostic in red, with a squiggle?

`underlineColor` and `underlineStyle` on a `<span>` (or on a text's style
rows): `<span underlineColor="#ff0000" underlineStyle="wavy">value</span>`
draws a red wave under the span alone, following it across a wrap, and
leaves the glyphs their own colour; either row implies `underline`. The
shapes are `solid`, `wavy` and `dotted`. A terminal's undercurl is the
same on a cell — the wave bit (32) in its flags and the underline colour
as a fifth entry (SGR 4:3 and 58). No `line` float under the run, no rect
arithmetic; a wave is pieces of the segment primitive a `line` draws.

[`underlineColor` / `underlineStyle` rows](props.md#text-props) ·
[alpha.13](../CHANGELOG.md#010-alpha13-2026-09-15)

### How do I draw a selection over several lines as one rounded shape?

Give each selected span its background and a radius:
`<span bg="#3b5bd466" bgRadius={4}>…</span>` on every row the selection
covers. Rounded backgrounds of one colour and radius that meet are one
shape, whichever text drew them: an editor's rows, the wrapped lines of
one paragraph, a line and the cell after it for its newline. The corners
are convex where a line reaches past the one above or below, filleted
where it falls short, and round where nothing meets them. It is worked
out from the frame's own layout once every text is painted, so it is
never a frame behind a wrap or a scroll, and the view names no shape and
measures no line. Give a second selection another colour, or leave a
gap, to keep it apart.

[`text` element](props.md#elements) ·
[ADR 0035](adr/0035-a-rounded-background-is-joined-by-meeting.md) ·
[alpha.22 `### Added`](../CHANGELOG.md#010-alpha22-unreleased)

### How do I list the installed fonts, the monospaced ones first?

`ctx.systemFonts()` (Rust `Core::system_fonts()`, C `kui_system_fonts`)
lists every family the core can see, installed or loaded, sorted by
name, each as `{family, monospaced, weights, italic}`: `monospaced` when
every face says it is fixed-pitch, the weights its faces come in (400
regular, 700 bold) and whether one is italic. Filter or sort on
`monospaced` and hand the chosen `family` to `addSystemFont`. It is read
from what the font database recorded when it scanned each face, so six
hundred families answer at once; measuring an `i` against an `M` in each
instead loads and shapes every file — seconds — and calls a symbol font
monospaced because its glyphs share an advance. `systemFontFamilies()`
is the names alone.

[Doors](props.md#doors) ·
[alpha.21 `### Added`](../CHANGELOG.md#010-alpha21-2026-09-26)

### How do I show a 100k-character line, or a paragraph that long?

Hand it over as one `text` node, plain or as spans. A text of 4096 bytes or
more with no line breaks is shaped in ~1 KB chunks as they come on screen,
so it costs the screenful it shows and a keystroke into it costs the chunk
it lands in — and so does a span moving along it, which is what an editor's
caret row is (a background under one character, the syntax colours around
it); under `wrap: word` the rows are broken from the chunks' positions, so a
paragraph costs the rows it shows. Its size is an estimate until the chunks
shape (exact under monospace), so a scrollbar can move a little as they do.
Do not slice the line yourself to spare the toolkit: the chunk is the slice.

[`text` row](props.md#elements) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How do I show a list of ten thousand rows?

`uniformList` (JSX), `uniform_list` (Lua, Rust): declare the rows that
can be seen and two spacers holding the height of the rest, so the frame
costs a screenful however long the list is — about 16 µs against 3.9 ms
for ten thousand rows built whole. It takes the row count and one row's height;
the rows come from a callback it runs for the ones it slices, and it keys
each by its data `index`, so a row keeps its hover, focus, edit buffer and
tweens as the built range slides over it. `setScroll(key, 0, i * rowH)` is
"scroll to row `i`" — `reveal` finds nothing for a row nobody declared.
It slices by the frame before, and the core asks for the frame that
closes the lag when the container came out otherwise (taller after a
resize, scrolled elsewhere by a reveal), so nothing to request yourself.

In Node it also declares the node that makes `view` run again when the
container scrolls: the wheel raises no event and a window redraws by
re-lowering the tree it was handed, so nothing else would. Those events
never reach `update`.

```jsx
uniformList(ctx, { key: 'log', rows: lines.length, rowH: 28 }, (i) => (
  <box width="grow" height="grow" onClick={{ kind: 'pick', row: i }}>
    <text>{lines[i]}</text>
  </box>
))
```

[`index` row](props.md#composite-props-hand-written-per-binding) ·
[`virtual_list.tsx`](../examples/node/widgets/virtual_list.tsx)

### How do I do that when the rows are not all the same height?

`list` (JSX, Lua), `widgets::list` (Rust): the same list over prefix sums
instead of a stride. It calls a `measure(ui, i, width)` for the rows it is about to build
and nothing else — `ui.measure_text(text, &style, Some(width))` is what
layout would give that row, wrap included, through the same shaping cache
its draw will hit — and every row it has not measured stands at the mean of
the ones it has. What `measure` returns is the height the row *gets*, so the
spacers can never disagree with the layout.

The heights live in a `RowHeights` the app owns and hands back each frame
(`set_len` when the list grows, `clear` when the rows change under the same
indices). Measuring changes the height of every row still standing at the
mean, the ones above the window included, so the widget puts the row the
window starts in back where it was before it re-slices — the frame that
learns is drawn already corrected. To stay at the end of a growing log, ask
for it: one `set_scroll(key, huge)` after the widget, every frame.

In Node and Lua the heights are the core's own `RowHeights` too — `new
RowHeights(rows, estimate)`, `row_heights(rows, estimate)` — kept in the
model or a script global, and `measure` is `(i, width) => height` over
`ctx.measureText` / `env.measure_text`. The slicing and the anchor are the
core's arithmetic in every binding; the port is the loop around your
`measure`, and the correction is `shiftScroll` / `shift_scroll`, which a
list composed by hand (C's `kui_shift_scroll` included) calls the same way.

```jsx
const heights = new RowHeights(lines.length, 24); // once, kept in the model
list(ctx, { key: 'log', heights },
  (i, width) => ctx.measureText(lines[i], { size: 13 }, width - 12).height + 12,
  (i) => <box pad={6}><text size={13}>{lines[i]}</text></box>)
```

[`exit_budget.rs`](../examples/rust/features/exit_budget.rs) ·
[`virtual_list.rs`](../examples/rust/widgets/virtual_list.rs) ·
[`virtual_list.tsx`](../examples/node/widgets/virtual_list.tsx) (`--variable`)

### How do I draw a terminal's screen?

`cells`: rows × cols of `{ch, fg, bg, flags}` and a cursor, one node — a
200×50 screen costs ~60 µs a frame with every character new, against ~2 ms
as a text node per cell. A click or drag on it carries `cell: {row, col}`
in its payload, and the node reads as a `terminal` with the rows joined as
its value. Inverse, dim and a wide cell's blank spacer are the app's.

So is the scrollback: a grid is one screenful, and `originLine` says which.
Declare `onScroll` on it and the wheel arrives as `{kind:"scroll", lines,
…}` — the whole lines the notch covers, positive toward later history, the
fraction carried to the next notch — and you re-declare the grid with
`originLine + lines`. A drag-select held past the grid's top or bottom
edge arrives the same way, once a frame with the lines that frame scrolled
by, and the selection's ends are absolute lines, so they stay put through
the scroll you answer with. `examples/rust/widgets/cells.rs` does both.

[`cells` row](props.md#elements) ·
[`scroll` event](props.md#events) ·
[ADR 0029](adr/0029-a-selection-follows-the-pointer-past-the-edge.md) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How does a selection follow the pointer past the edge?

On its own. A press-drag in an `edit`, a `selectable` scope or a `cells`
grid keeps following a held pointer after it leaves the scroller: the core
scrolls the nearest scrolling ancestor toward the pointer at 10 px/s for
every px past the edge (capped 100 px out, so 1000 px/s), and places the
live end again whenever the layout under the pointer moves — that nudge, a
wheel notch under the held press, a virtual list re-slicing its rows. A
Shift-press keeps the anchor and moves the live end, in all three, by
characters. Nothing to declare; a headless test sees the nudge as
`scrollOffset` moving a frame at a time under `animating()` — ten px a
frame with no clock, the clock's own share with one (`app.advance(ms)` in
Node, `Drive::advance` in Rust). The wheel over any node that declares
`onScroll` is a message instead — `{kind:"scroll", x, y, dx, dy, lines,
tag}` — and the node takes it from the scroller above; a `cells` grid
gets its edge drag the same way, since its history is yours.

[`onScroll` row](props.md#container-props) ·
[`scroll` event](props.md#events) ·
[ADR 0029](adr/0029-a-selection-follows-the-pointer-past-the-edge.md) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How do I pan a canvas with a drag?

`onDrag` events carry `dx`/`dy` measured **from the press point** in every
phase, so a pan handler is `offset = offsetAtPress + dx` rather than a sum of
deltas, and `end` alone is enough to commit. Cards that carry `slide` and a
`transition` may keep them: a tween whose target moves every frame trails it
by about one transition's worth of travel instead of freezing, as of
alpha.7.

[`drag` payload](props.md#events) ·
[`slide` row](props.md#container-props) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I follow the OS's dark mode, accent or reduce-motion setting?

Read `env.system`: `appearance` (`"light"` / `"dark"` / `"unknown"`),
`accent` (`0xRRGGBBAA`, or null), `motion` (`"reduced"` when the user asked
for less animation) and `locale` (a BCP-47 tag). The Rust runner asks the
OS for all four on macOS and Windows, and re-asks when the app takes focus
back; on X11 and Wayland it answers the locale from `LANG` and the rest
read `"unknown"` — as they do in any driver that owns its own window until
it pushes what it knows. So branch on the setting you got and keep your own
default for the unknown, which is a reading and not a missing value. The core acts on
none of it: nothing repaints because the appearance changed and no animation
shortens itself, because only the view knows which of its colours is the
background.

The fifth row is not a setting: `assistive` reads `"listening"` once an
accessibility client has asked the window for its tree, `"none"` while the
bridge is up and nobody has, and `"unknown"` where there is no bridge (a
headless `Ctx`). It is the reading that changes what a view *says* rather
than what it draws — `announce` the alert when it is `"listening"`, blink
it otherwise — and it arrives through the same `system` event as the other
four when it changes. Any client counts, and on macOS and Windows nothing
reports a client leaving, so once risen it stays; only AT-SPI says
`"none"` again. Headless, `ctx.setEnv({ system: { assistive: 'listening' } })`
declares it, so a test can assert the announcement.

Mostly you do not have to. The **theme** is that branch, written once: the
appearance picks a base and the accent recolours it, and the palette comes
back as named roles — `ctx.theme()` in Node, `ui.theme()` in Rust,
`env.theme` in Lua, `kui_theme` in C. The stock widgets already read it, so
a button, the context menu, a tooltip, a field, the scrollbars, the focus
ring and a `<text>` with no `color` follow the OS with nothing written at
all. Paint your own boxes from the same roles and they agree:

```tsx
const t = ctx.theme();
<box bg={t.surface} radius={10} borderColor={t.border} borderW={1}>
  <text color={t.muted}>seventeen items</text>
</box>
```

`ctx.setAccent('#d2691e')` keeps following the OS's light/dark while
painting your own colour — what an app with a brand colour wants — and
`ctx.setTheme({ appearance: 'dark', surface: '#101014' })` pins a palette
that follows nothing. Every role, and its value on both bases, is in the
[Theme table](props.md#theme).

A stock button follows the accent on its own: `<button accent>` takes the
theme's accent, derives its hover and pressed shades from it, and picks a
black or white label by its luminance — falling back to the stock blue
where nobody, the OS included, has said what the accent is. Any other node
can carry `accent` too, which substitutes its `bg` and nothing else;
`Color::mix` and `Color::luminance` are there for a colour of your own that
is not a role.

The reading is there before your first view — a window fills it in as it
opens, not on its first frame — and a change to it arrives as a `system`
message on the root, carrying the whole of `env.system` as it now reads:

```ts
update(m, msg) {
  if (msg.kind === 'system') return { ...m, dark: msg.appearance === 'dark' };
  ...
}
```

Take the message rather than only reading `env`, because a driver's redraw
re-lowers the tree it was handed and does not re-run your `view`: an app
that paints only on input would otherwise hold the palette its first frame
picked for as long as the user leaves it alone. A Rust `App`, whose `view`
*is* what the runner calls every frame, can read `env` and ignore the
message.

[`system.*` rows](props.md#env) ·
[Theme table](props.md#theme) ·
[`system` event](props.md#events) ·
[`accent` row](props.md#container-props) ·
[ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md) ·
[alpha.10](../CHANGELOG.md#010-alpha10-2026-09-09)

## Interaction, focus and reading

### How do I open a popup, and when is a modal enough?

A `modal` node is one tree: the Tab ring becomes its subtree, everything
outside it is inert to the pointer, the wheel and assistive technology, and
Escape or an outside press emits `{kind:"dismiss"}` on the node, which the
app answers by not declaring it. Reach for a `kind: "popup"` window only for
the placements a float cannot make — a list taller than the window, a panel
beside the app — because a popup is an OS surface where a `fit` float plus a
`modal` costs one tree; it reports the same `dismiss` event, on the root.

[`modal` row](props.md#container-props) ·
[`windows` row](props.md#composite-props-hand-written-per-binding) ·
[ADR 0003](adr/0003-modal-surfaces.md) ·
[ADR 0004](adr/0004-multi-window.md)

### How do I offer a choice among a few options?

`<select label="language" options={["English", "Deutsch"]} current={i}/>`,
`dropdown { label = "language", options = { "English", "Deutsch" }, current = i }`
(Lua's `select` is its own, so the table is `dropdown`, and `current`
counts from 1), `kui_select(ctx, label, items, count, current)`,
`widgets::select(ui, "language", &["English", "Deutsch"], Some(i))`: a
field that shows the choice in force and, clicked, drops the core's own
menu of the options with the current one checked — the menu a right-click
opens, drawn in the frame or the platform's where the host shows menus
itself, dismissed by Escape or a press outside, walked by the arrows. You
hold no open state. The choice arrives as the `menu` event a menu row
posts, on the field's key — `{kind:"menu", role:"custom", item:"Deutsch"}`
— and drawing the field again with the new index is the whole loop. An
option may be a menu item instead of a string — `{ label: "18 pt", id: 18 }`,
`enabled: false` for a dead row — so a choice can post an `id` of its own
(a size in points, an enum's tag) instead of its label; in Rust that is
`select_items`, and `select_with` takes the field's spec and text style
for a dense panel. The field reads no other row; one that needs any is a
`role="button"` box and `openMenu`. What is checked: the options may not
be empty (refused where they are written), a `current` past them or on a
separator is none with a `select-current-ignored` warning on the field,
a key of an option object no row reads (`disabled` for `enabled: false`)
is an `unknown-prop` warning, and a dead row reported chosen through
`activateMenuItem` / `kui_activate_menu_item` is refused — false, nothing
posted, the menu still open — as the pointer never reaches it.

[`select` element](props.md#elements) ·
[examples/rust/widgets/select.rs](../examples/rust/widgets/select.rs) ·
[examples/node/widgets/select.tsx](../examples/node/widgets/select.tsx) ·
[the context menu](#how-do-i-open-a-popup-and-when-is-a-modal-enough)

### How do I add a checkbox, a radio group, a switch or a slider?

The stock controls ([ADR 0034](adr/0034-stock-controls-over-the-roles.md))
are drawn from your model and hold nothing of their own. A toggle —
`<checkbox checked={m.sync} onClick={{kind: 'sync'}}>Sync</checkbox>`,
`<radio>`, `<switch>`; `checkbox { label = "Sync", checked = …, on_click
= … }` in Lua; `kui_checkbox(ctx, text, spec, payload)` in C;
`widgets::checkbox(ui, "Sync", on, payload)` in Rust — posts its
`onClick` when the pointer, Space, Enter or a screen reader presses it,
and your `update` flips the model. A select-all box over a partial
selection says `mixed`. Put radios in a `<radioGroup label="Theme">`: it
is one Tab stop whose arrows move the choice and press the radio they
land on, so radios that each set the choice answer the keyboard with no
more code (`widgets::radio_group(ui, "Theme", &options, Some(i), |i|
payload)` in Rust).

A slider says its range and step and asks the core for its changes:
`<slider label="Volume" valueNow={v} valueMin={0} valueMax={100}
valueStep={5} onChange={{kind: 'volume'}}/>`. A press proposes the value
under the pointer, a drag each new step, the arrows one step, PageUp /
PageDown ten, Home / End the ends — clamped, snapped, and the decimal the
step names (`0.3`, not `0.30000001`) — as `{kind: 'change', value, phase:
'move' | 'end', tag}`. Store `value` and declare it as `valueNow`; nothing
moves until you do. A slider without `onChange` is the hand-drawn kind
and keeps the `access` nudge.

The controls' look is their spec, so they read only their own rows, and
a paint row on one is an `unknown-prop` warning; a control that needs a
look of its own is a box with the role and the same rows. Their size
follows the metrics: the box is the control text plus one, so `compact`
moves them with the stock button.

[the elements](props.md#elements) ·
[examples/rust/widgets/controls.rs](../examples/rust/widgets/controls.rs) ·
[examples/node/widgets/controls.tsx](../examples/node/widgets/controls.tsx)

### How do I match my app's messages as types, in Rust?

`#[derive(Message)]` on an enum: each variant becomes a `{kind, …fields}`
payload (the kind is the variant's name in snake_case, or `#[message(kind
= "…")]`), so `on_click(Msg::Save)` builds it, and `ev.message::<Msg>()`
reads it back into an exhaustive `match`. A click delivers the message as
its payload; a drag, a change, a scroll or a drop delivers it as the `tag`
inside the core's event, and `message` looks in both places. The event's
own fields stay on `ev.payload` — a drag's `phase`, a change's `value`.
Field types are the numbers, `bool`, `String`, `Option`, `Vec`, `Value`
and other messages. An enum of unit variants marked `#[message(string)]`
is a bare string where it is a field (`dir: SplitDir` as `"h"`).

The payload stays plain data, so Lua, C and JSX read it as they always did.
`MessageError` says what did not fit. The derive is `kui-native`'s default
`derive` feature; from kui-core alone it is `kui-core/derive` and
`#[message(crate = "kui_core")]`.

```rust
#[derive(Message, Clone, Debug)]
enum Msg { Focus { pane: u64 }, TabNew, Split { path: String, dir: SplitDir } }

ui.with(NodeSpec::column().on_click(Msg::Focus { pane: id }), |ui| { … });

fn on_event(&mut self, ev: UiEvent) {
    match ev.message::<Msg>() {
        Some(Msg::Focus { pane }) => self.focused = pane,
        Some(Msg::Split { path, dir }) => self.drag_divider(&ev, path, dir),
        Some(Msg::TabNew) => self.new_tab(),
        None => {} // a core event with no message of ours: a key, a resize
    }
}
```

[`splitmux.rs`](../examples/rust/apps/splitmux.rs) ·
[backlog C50](BACKLOG.md)

### How do I let the user pick a file, or where to save one?

Ask for the platform's dialog: `ctx.requestFiles({ mode: 'open', multiple:
true, filters: [{ name: 'Images', extensions: ['png', 'jpg'] }], tag })`
(`'save'` with a `fileName`, or `'folder'`). The answer is one `files`
message — `{kind: 'files', paths, tag}`, the paths a `drop` carries, none
when the user cancelled — so a list that takes dropped files takes picked
ones with the same code. An Open or folder answer carries only what
exists: a name typed into Windows' multi-select panel that is not there
is left out. One dialog at a time: asking again while one is
up does nothing. The window's runner shows it (a sheet on macOS, through
rfd); a headless test takes the ask with `takeFileRequests()` and answers
with `answerFiles(paths)`. Rust asks from its view with
`ui.request_files(FileDialog::open()…)`, Lua with `env.request_files{…}`,
C with `kui_request_files`.

[`files` event](props.md#events) ·
[`drop.tsx`](../examples/node/features/drop.tsx) ·
[`drop.rs`](../examples/rust/features/drop.rs)

### How do I give my app a menu bar?

One call, in the view, wherever the strip belongs: `<menuBar menu={[…]}/>`,
`menu_bar { menu = {…} }`, `kui_menu_bar(ctx, menus, count)`,
`widgets::menu_bar(ui, bar)`. The menu is a list of `{ label, items }`
whose items are the rows a context menu takes (`id`, `role`, `accel`,
`enabled`, `checked`). On macOS the call draws nothing and the driver hands
the same declaration to the OS; everywhere else it draws those menus in the
window. Choosing a row is one
`{kind:"menu", role, item}` event either way, and a standard `role` is
performed by the core — an Edit menu's Copy is the right-click Copy.
Declare it every frame: it is diffed, so an unchanged bar costs a
comparison, and an empty list takes it away.

Declare nothing and a macOS app still has the menus a Mac app is expected
to have: the application menu, an Edit menu whose rows are the ⌘ chords
the runner already performs (greyed when the chord would do nothing), and a Window menu with Minimize, Zoom, Enter
Full Screen and — added by AppKit because the menu is registered as the
platform's — Fill, Center and the tiling submenus, so fn+ctrl+F and fn+F
work. A declared bar is exactly what you declared; name a menu `Window`
and it is the platform's, with those rows and shortcuts in it. An `Edit`
menu, standard or declared, also carries the two rows AppKit appends
that work in a kui window — Emoji & Symbols and Dictation both type
into the focused editor or key sink, the palette's pick and a dictated
phrase arriving as the `text` a composition's commit is — and not the
two that cannot: Writing Tools finds no selection to read and AutoFill
wants a platform text field, so the runner removes them. ⌃⌘Space opens
the same palette from the keyboard and types the same way.

[`menuBar` element](props.md#elements) ·
[ADR 0018](adr/0018-a-menu-bar-the-app-declares.md) ·
[ADR 0030](adr/0030-the-standard-menus-the-runner-keeps.md) ·
[examples/rust/widgets/menu_bar.rs](../examples/rust/widgets/menu_bar.rs)

### How do I take files dropped from the Finder?

Declare `onDrop` (Rust and Lua `on_drop`, C `KuiSpec.on_drop`) on the box
that takes them, and it hears `{kind:"drop", phase, paths, x, y, tag}` in
four phases — `enter`, `move`, `leave`, `drop`, with the OS paths as
strings and the pointer in viewport coordinates. Add `dropBg` and the box
lights while the files hover, the way `hoverBg` does, with nothing kept in
your model. A button or field inside the box is the box's; the banner you
float over it in answer to `enter` is looked past, so the zone does not
flicker between `leave` and `enter` the way an HTML drop target does. A
release over no zone is refused by the runner — the icon slides home — and
no `leave` follows a `drop`. Headless, `dragFiles(paths, x, y)`,
`dropFiles` and `dragCancel` on the `Ctx` (or their C spellings) are the
drive, and `dropTarget()` is what a driver answers the OS with.

[ADR 0031](adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md) ·
[`onDrop` row](props.md#container-props) ·
[`drop` payload](props.md#events) ·
[`examples/rust/features/drop.rs`](../examples/rust/features/drop.rs)

### How do I have global shortcuts and a Tab ring at once?

Put the keymap on an `onKey` sink that encloses the controls: a focused
control claims only the keys the core acts on for it (Enter and Space where
there is something to click, the arrows on a slider), and everything else
bubbles to the nearest enclosing sink. Chords always bubble, Tab always stays
the ring's, and bubbling stops at a modal boundary so a dialog's shortcuts do
not leak to the app behind it.

[ADR 0011](adr/0011-keys-bubble-to-the-enclosing-sink.md) ·
[`onKey` / `keyUp` rows](props.md#container-props) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I make a scroll glide instead of jumping?

Declare a `transition` on the scroll container. A `reveal` or a
`set_scroll` then eases the offset over it — the ribbon a keyboard
walks, a list a shortcut jumps to — while the wheel, the scrollbar's
thumb and a drag past the edge keep landing whole, since none of them
may lag a finger; any of them interrupts a leg in flight and takes the
content where it stands. `scroll_offset` is where it is going and
`scroll_geometry`'s offset is where the content is, which is what a
virtual list slices by, so a long list glides without drawing the wrong
rows.

[`transition` row](props.md#container-props) ·
[alpha.17](../CHANGELOG.md#010-alpha17-2026-09-25)

### Why does a swipe down not move the strip sideways?

A trackpad swipe keeps to the axis it started on, so nothing is yours
to filter. A finger never moves straight: a swipe down carries a few
pixels sideways, and a `scrollY` list passes the x it does not scroll
to the `scrollX` strip around it, so every swipe used to nudge the
strip. The native runner locks a swipe to the larger of the two axes
once it has travelled 4 px and drops the other while the swipe and its
glide last; a pause of 200 ms ends it, and a hand that turns without
lifting — the other axis carrying three times the locked one's recent
travel — turns the lock with it. A diagonal swipe therefore moves one
axis at a time, as macOS's own scroll views do. A mouse wheel's notches
are not locked: they come one axis at a time already, and Shift turns
them sideways on purpose. An `onScroll` node hears the locked delta.

[`scroll` event](props.md#events) ·
[alpha.22](../CHANGELOG.md#010-alpha22-unreleased)

### Does a pane off the edge of a scroller still hear its keys?

Yes, since alpha.17. A key reaches a node by holding focus, not by being
somewhere a pointer could land, so a sink scrolled out of its container,
or drawn part-way to its place by `slide` or an `enter` offset, hears
every press with its tag as usual — you can give the keyboard to a
column on a ribbon and type into it before the reveal has brought it
back, and animate the ribbon while you do. A `disabled` sink and one a
modal shut out hear nothing, wherever they are drawn, and the pointer is
unchanged: a click past the clip finds nothing. Before alpha.17 the
delivery read the pointer's hit list, so those keys fell silently on the
floor.

[ADR 0011 decision 9](adr/0011-keys-bubble-to-the-enclosing-sink.md) ·
[alpha.17](../CHANGELOG.md#010-alpha17-2026-09-25)

### How do I get an IME into an editor I own?

An `onKey` sink drawing `line` rows with `caret` hears a composition as
data: `{kind:"preedit", text, cursor, tag}` while it is composed (an empty
`text` is the composition ending) and `{kind:"text", text, tag}` on the
commit. Plain typing is not a commit — the `key` event already carries what
the press would insert — so nothing arrives twice. The candidate window is
anchored at the `line` carrying `caret` for you.

[`text` and `preedit` events](props.md#events) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How does copy and paste work?

The clipboard is the host's — the core never reads it — so every copy is
someone working out *what* and handing the host the text, and every
paste is the host reading the clipboard and handing the text back as
input. There are four ways onto it, and they end in one queue:

1. **The chords.** `Cmd/Ctrl-C/X/V/A` are the windowed runner's own,
   performed while an `edit` or a `selectable` scope (a `cells` grid is
   one) has focus: Copy reads the selection — an editor's, or a scope's
   runs in reading order with the bold and italic beside as HTML — and
   writes the clipboard directly; Paste is typing into the editor. No
   queue, so a headless test sees nothing from a chord: it reads
   `requestCopy()` / `selectionText()` / `selectionHtml()` instead.
2. **The stock menus.** The context menu over an editor (Cut / Copy /
   Paste / Select All) or a scope (Copy / Select All), and `role: copy |
   cut | paste` rows in your own menus or the menu bar. The core performs
   the row and queues a `setClipboard` (with `text` and `html`) or a
   `paste`.
3. **A virtual list's ask.** A copy whose selection reaches rows no frame
   built cannot be answered by the core: `requestCopy()` says `asked`, a
   `{kind:"selectionrange", from:{index, byte}, to:{index, byte}}`
   message arrives on the scope, and `answerSelectionRange(text)` — the
   rows are yours — queues the `setClipboard`. Select All in such a list
   spans the whole of it — rows `0..rowCount`, which `uniformList` and
   its siblings declare for you (`rowCount` on a list you compose by
   hand) — and asks the same way, `to.byte` past the last row's length
   when that row was never built: cut it to the row.
4. **Your own.** `setClipboard(text, html?)` and `requestPaste()` (Rust
   `ui.set_clipboard` / `ui.request_paste`, Lua `env.set_clipboard` /
   `env.request_paste`, C `kui_set_clipboard` / `kui_request_paste`)
   queue the same two actions from anywhere — an `onKey` sink that hears
   `Cmd-C` raw and wants a yank register, an "export" button, a
   `withEffects` handler that calls `surface.setClipboard` the way it
   calls `surface.play`.

**The queue.** In a window the runner drains it after every input and
every frame and does the work: a `setClipboard` goes to the OS, a `paste`
reads the OS and comes back as a **commit** — typing into the focused
editor, or `{kind:"text", text, tag}` on the focused `onKey` sink (the
nearest one above a focused control, or the root sink with nothing
focused), the same message an IME's commit arrives on, so one arm handles
both. Headless, `takeMenuActions()` (`Core::take_menu_actions`,
`kui_take_menu_action`) is the queue and `commit(text)` the answer; a C
host driving its own window drains it the same way and reads the
clipboard itself. A paste with no editor and no sink to land on is
dropped.

`examples/rust/features/clipboard.rs` and `examples/node/features/clipboard.tsx`
do all four, with a headless drive that pins what each leaves in the
queue — and, on the way to the copy, the drag held past the log's edge,
the wheel under the held press and the Shift-click that extends
(ADR 0029).

[`selectionrange` and `text` events](props.md#events) ·
[ADR 0017](adr/0017-selection-as-a-scope.md) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How do I give the editor I own a mouse and a clipboard?

Put `onDrag` on the same `onKey` sink. Every `drag` event inside it then
carries `line` (which of the sink's `role="line"` rows the pointer is on —
the same numbering its `access` events use), `byte` (where on that line,
as `textHit` would answer) and `clicks` (the press's count), so the press
places the caret, a `move` extends the selection from where it pressed,
and a count of two takes the word — all arithmetic in the handler, with
no query and no frame of lag. A click's map payload gains the same three.

The clipboard is the host's, and the sink hears the raw `Ctrl-c` /
`Ctrl-v`: bind `y` to `setClipboard(text)` (Lua `env.set_clipboard`, C
`kui_set_clipboard`, Rust `ui.set_clipboard`) and `p` to `requestPaste()`
— the paste comes back as the same `{kind:"text", text}` event an IME's
commit does, so one arm inserts both. A window applies both after every
input and every frame; headless, `takeMenuActions()` hands them out and
`commit(text)` answers the paste.

[`drag` and `text` events](props.md#events) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How do I hear the middle button, or give a terminal's program the mouse?

Declare `onButton` on the pane. A middle press over it — or over anything
inside it that claims no button of its own — is `{kind:"button",
phase:"press", button:"middle", x, y, clicks, tag}` on the pane, and the
button is then the pane's until it comes up: every move while it is held
is `phase:"move"` and the release `phase:"release"`, on the pane wherever
the pointer went, so a drag that leaves the pane still ends there. On a
`cells` grid each carries `cell: {row, col}`, clamped to the grid, which
is what a terminal's mouse report needs; a paste is the middle press
answered with `requestPaste()`.

The secondary button is claimed too unless you narrow it: `buttons:
"middle"` leaves the right button to the pane's `onContextMenu` and the
stock menu, and `buttons: "secondary middle"` takes both, which is what a
terminal whose program turned mouse reporting on wants — declare the one
or the other each frame from that mode. A further button (back, forward)
is `"other"`, and its events carry its number. None of these presses
moves focus or the caret; the primary button presses, drags and clicks as
it always did.

[`onButton` and `buttons` rows](props.md#container-props) ·
[`button` event](props.md#events) ·
[alpha.22 `### Added`](../CHANGELOG.md#010-alpha22-unreleased)

### How do I keep a pasted password out of my editor's history, and copy one?

Read the markers on the paste. A password manager copies a secret with
the pasteboard marked concealed and transient, and the answer to
`requestPaste()` says so: the sink's `{kind:"text", text}` carries
`concealed: true` (a secret — show it to no one, log it nowhere) and
`transient: true` (keep it in no history), each only when it is set.
Insert the text as usual and leave it out of whatever you remember —
a register, an undo you persist, a paste history. The runner reads the
markers on macOS and Windows; on Linux, and from a host that answers
with a bare `commit`, a paste arrives unmarked.

To put a secret on the clipboard yourself, call
`setClipboardSecret(text)` (Rust `ui.set_clipboard_secret`, Lua
`env.set_clipboard_secret`, C `kui_set_clipboard_secret`) instead of
`setClipboard`: the runner writes it marked the same way, so clipboard
managers neither show nor keep it. Headless, it comes out of
`takeMenuActions()` as `{kind:"setClipboardSecret", text}`, and a test
answers a paste with `paste(text, {concealed, transient})`.

[`text` events](props.md#events) ·
[alpha.17](../CHANGELOG.md#010-alpha17-2026-09-25)

### How do I protect a password as it is typed?

Declare `secureInput` on the root (Rust `ui.secure_input(true)`, Lua
`secure_input = true` on the root table, C `kui_set_secure_input`) on
every frame the prompt is up — a terminal whose pty turned echo off, a
password field of your own. The runner turns on macOS's Secure Keyboard
Entry while that window has the keyboard, so no other process can read
the keys, and turns it off when the window loses the keyboard, closes,
or the frame stops declaring it; you never call the platform and never
balance anything. Keep it to the prompt: while it is on, no other app
sees the keyboard at all, a launcher's hotkey and a text expander
included. Other platforms have no such switch and ignore it.
`Ctx.secureInput()` / `kui_secure_input_get` read the ask back — what a
test asserts on, and what a C host driving its own window reads to
make the call itself.

[`secureInput`](props.md#composite-props-hand-written-per-binding) ·
[alpha.17](../CHANGELOG.md#010-alpha17-2026-09-25)

### How do I make the caret I draw blink?

Read `caretVisible()` (Rust `ui.caret_visible()`, Lua `env.caret_visible`,
C `kui_caret_visible`) in `view` and draw your caret node only when it is
true — keeping the `caret` row on the `line` either way, because that
row is what arms the clock (and anchors the IME). The window's runner
runs the clock while a focused `edit`, or a `caret` line under the
focused `onKey` sink, has a caret: half a second on, half off, re-armed
solid whenever the caret moves, and parked hidden while the window has
no keyboard — so the stock editor and yours blink in step and neither
blinks in the background. Headless the phase stays true;
`setCaretVisible(false)` / `kui_set_caret_visible` is how a test sees
the off phase drawn, and how a C host with its own window drives it
(`kui_has_caret`, `kui_caret_stamp` are the clock's inputs). Never use
`keyframes` for this: they ask for a frame every vsync and never stop.

A caret that does not blink — the block of a modal editor's normal mode —
declares `caretSolid` (Lua `caret_solid = true`, C `KUI_VALUE_CARET_SOLID`
in `value_set`) beside `caret` on the same line: the row still anchors
the IME and is still the caret assistive technology hears, but it is no
caret to blink, so the clock is not armed and an editor idling in normal
mode asks for no frame at all. Without it the clock runs for as long as
the `caret` row is declared, whether or not the view reads the phase —
the one thing that asks an idle app for a frame twice a second.

[`caret_visible`](props.md#env) ·
[`caretSolid`](props.md#container-props) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### Why does a held key not repeat on a Mac?

macOS's press-and-hold: holding a letter offers its accents (`e` → `é è
ê`) instead of repeating it, and a letter with no accents does nothing
at all — on by default, and off only on a machine whose owner turned it
off, which is why it works on one Mac and not the next. Every other
platform repeats. It is the **user's setting, not the app's**: the read
that decides is HIToolbox's, of the user's global preference by name,
and no per-process default reaches it — not the argument domain, not a
registered default, not the app's own domain (F69 built a door that
pinned one; RG15 found it inert and RG16 removed it — the backlog
archive has the measurements). What works is

```
defaults write -g ApplePressAndHoldEnabled -bool false
```

and a relaunch — what the owner of a modal editor, where `j` held is a
motion, has usually done already. An app whose keys are commands can say
so in its README; kui has nothing to offer it beyond that.

### How do I reset an editor's text?

`initial` seeds a *new* editor only — a key declared again keeps the draft
the user typed — and `setEditText(name, text)` is what resets one, leaving
the caret at the end. Name it by the label the editor's own `key` prop
declares, `setEditText('edit-n13', text)`, and it can be called from the
`update` that *opens* the editor: no frame has declared it yet, so the text
is held and the frame that draws it takes it, over `initial`. That is the
spelling to reach for, because the other one — the 16-digit hex key — comes
from an event the editor has not fired. It reaches an editor that is coming
back too: one retained while its key was off screen takes the text over the
draft left in it, so a second rename opens on the model's text and not on an
abandoned edit. A name nothing declares by the end of that frame drops its
text with an `edit-text-without-editor` warning.

[`edit` element](props.md#elements) ·
[`edit-text-without-editor`](props.md#warnings) ·
[alpha.10](../CHANGELOG.md#010-alpha10-2026-09-09)

### How do I make a rename field break where its label breaks?

Declare `wrap` on the field: `<edit width="fit" maxWidth={w} wrap="word">`.
A single-line editor is a field — one line whatever its box, scrolled under
the caret — until it says `wrap`, when it folds to its width like a document
and keeps a field's keyboard: Enter submits, no newline goes in, the caret
opens at the end. With `fit` and `maxWidth` the box hugs a short draft,
stops growing sideways at the clamp and grows down from there, on the frame
that lays out the keystroke — so no headroom to declare past the widest
glyph. `wrap="none"` is the plain field; a `multiline` editor wraps either
way.

[`edit` element](props.md#elements) · [`wrap`](props.md#text-props) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How do I keep a panel's controls out of my app's Tab ring?

Declare the panel `focusRegion`. Its subtree becomes a Tab ring of its own:
the app's ring never enters it, and inside it Tab wraps over the panel's
controls alone. Bind a chord to `focusRegion('panel')` to move the keyboard
in — it lands on what the panel last held, else its `initialFocus`, else its
first stop, and shows the ring — and to `focusRegion(null)` to come back; a
click on the panel enters it too. The call resolves on the next frame, so
the `update` that turns the panel on may enter it in the same turn, by the
label the `key` prop declares. Keys still bubble through the boundary to the
sink above, so the app's own shortcut layer keeps hearing the chord while
focus is in the panel. Do not reach for `role="none"`: it keeps the panel
out of the ring by making it invisible to every keyboard and screen-reader
user.

[ADR 0022](adr/0022-focus-regions.md) ·
[`focusRegion` row](props.md#container-props) ·
[`focus-region-without-node`](props.md#warnings)

### How do I say which control a dialog opens focused?

`initialFocus` on that control: it is where focus lands when the enclosing
`modal` scope is entered, so a destructive confirm opens on its Cancel rather
than on whichever control the view declares first. It is read on entry only —
a Tab press afterwards stands — and declaring it on nothing leaves entry at
the ring's first node.

[`initialFocus` row](props.md#container-props) ·
[ADR 0003](adr/0003-modal-surfaces.md)

### How do I give a hint that is spoken and never drawn?

`description` is the accessible description on its own — the extra sentence a
reader says after the name — and `tooltip` is that same sentence plus the
float drawn while hovered and the hover tracking it needs. Both write one
slot, so a node declaring both keeps whichever its binding applied last, and
neither reads on a plain box that the access tree elides.

[`description` row](props.md#container-props) ·
[`tooltip` row](props.md#composite-props-hand-written-per-binding) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07)

### How do I have a screen reader announce something?

Put `live="polite"` on the smallest node that holds the message and a reader
reads it when the text inside changes, without being asked; everything inside
a live node is live, which is why it goes on the smallest one. For a one-off
with no node behind it — "Saved" — the binding's `announce(text, live?)` verb
is the other half.

[ADR 0008](adr/0008-live-regions-and-announcements.md) ·
[`live` row](props.md#container-props) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I name a node from outside the view?

Every verb that takes a node takes either spelling: the label a `key` prop
declared, or the 16-digit hex key an event carried. `focus`, `isFocused`,
`reveal`, `access`, `editText`, `setEditText`, `setScroll`, `scrollOffset`,
`scrollGeometry`, `textHit`, `caretRect`, `isHovered` and `isPressed` all
resolve a label through the frame being built so far and then the last
finished one, so a node the user has never touched can be named. `keyOf(label)`
(Node) and `kui_key_of` (C) hand back that hex key when one is wanted to hold
on to, and null / 0 when no recent frame declared the label; Lua's focus,
scroll and editor verbs take the label itself, so a script needs neither. One verb goes further:
`setEditText` accepts a label *no* frame has declared, and holds the text for
the frame that does — that is how the `update` opening an editor names it.
A label declared by two nodes under different parents raises `ambiguous-key`
and picks the first in tree order: labels are unique among siblings, not
across a tree. The lookup is the asker's: a plugin filling a slot (a Lua
script, a C extension) is answered from the nodes it opened and no one
else's, so its `filter` and the host's never meet; the host sees its own
first, and everyone's when it declared none.

[`key` row](props.md#composite-props-hand-written-per-binding) ·
[`ambiguous-key`](props.md#warnings) ·
[alpha.10](../CHANGELOG.md#010-alpha10-2026-09-09)

## Sound and effects

### How do I play a sound when the model changes?

Register the bytes once (`ctx.addSound(buffer)` in `setup`, wav / ogg / mp3 /
flac) and declare `<audio key src>` for as long as it should play: present is
playing, gone is stopped, so a chime is one node under a key that changes
when the event does. Give it a `tag` and the playback's own end arrives as
`{kind:"sound", phase:"ended"}` — which is what to keep the node declared
until, rather than a guessed duration.

Guess it short and the sound is cut off mid-chime, which is what
`truncated-playback` is for: a windowed run warns when a one-shot's node goes
away while the device is still playing it. Either keep the node declared
until the `ended` event, or add `finish` — the removal then *releases* the
playback to play itself out, and a `tag` still reports when it gets there.
Headless there is no device and so no warning; the same fact from the test's
end is `audioCommands()` holding a `stop` for the node.

`finish` plays a one-shot out after its node goes, and a released playback
is not free: it holds one of the device's 128 voices until its file ends,
released or not, and the 129th play is refused. A refused play never starts
and so never ends — it arrives as `{kind:"sound", phase:"refused"}` on a
`tag`, and as a `playback-refused` warning either way, so nothing waits on
an `ended` that cannot come. In the app's units, voices held = sound length
× release rate: a 1.4 s chime released four times a second holds 6 of the
128 at any moment, a 10 s ambience released once a second holds 10, and every
playback still declared (a loop included) counts beside them — a `refused`
`sound` event is what arriving at 128 sounds like. Stop what the view no
longer needs instead of releasing it, and release short sounds.

[`audio` element](props.md#elements) ·
[sound resources](props.md#resources) ·
[`sound` event](props.md#events) ·
[`truncated-playback`](props.md#warnings) ·
[`playback-refused`](props.md#warnings)

### How do I do the thing my app defines — a file write, a request?

`update` returns `withEffects(model, ...effects)`: the effects are the app's
own data, the loop hands them to the `effects` handler it was built with, and
whatever that handler dispatches goes back through `update` like any other
message. kui's own effects stay where they are — a sound is `<audio>` or
`surface.play`, a window is `windows` — and a test reads `app.effects()`
whether or not a handler ran.

[ADR 0013](adr/0013-effects-as-data.md) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07)

### How do I show a video frame, a camera, or a plot I drew myself?

Make it an image and replace its pixels. Register once (`add_image` /
`addImage` / `kui_image_add`), then each time you have a new frame call
`update_image(id, w, h, rgba)` (`updateImage`, `kui_image_update`): the
handle is unchanged, so every `<image src={id}>` shows the new pixels next
frame with no view change, and from the first update on the image draws
from a texture of its own rather than the glyph atlas — as does one too big
for a page, which used to draw nothing. `sampling="nearest"` keeps an
emulator's or a pixel-art texel square; `fit="contain"` or `"cover"` meets
a box of another aspect without stretching. kui composes, clips, rounds,
fades and hit-tests the box; what is inside it is yours, rasterised with
whatever you like. In Rust, prefer `update_image_with(id, w, h, |px| …)`
and render straight into the slice it hands you. It is a buffer the core
recycles, so a stream stops allocating after its second frame and skips
your copy too. `updateImage` and `kui_image_update` copy your bytes into
that same recycled buffer. At 1080p that copy is ~275 µs, where a fresh
8 MB buffer a frame cost ~870 on Windows, whose heap faults every new
block in page by page (backlog W20). The upload to the GPU comes on top.

[`image` element](props.md#elements) ·
[ADR 0025](adr/0025-the-image-is-the-canvas.md) ·
`cargo run --example image`

### How many pixels should I render for this box?

`w × scale` by `h × scale`, from the `layout` event: put `onLayout` on the
box, and its payload carries the logical rect and `scale`, the physical px
per logical px at that node. Render that many pixels, `update_image`, and
the next frame shows them one texel per pixel — the loop the `image`
example runs. It is one frame late, which is the frame model: until the
first report the previous pixels show, stretched. `scale` is the frame's
today, and where a zoom would compose in if the core ever takes one.

[`layout` event](props.md#events) ·
[ADR 0025](adr/0025-the-image-is-the-canvas.md)

### How do I fill a shape — an arrowhead, a pie wedge, the area under a curve?

`<polygon points={[[x, y], …]} bg/>`: up to eight points in the parent's
box space, filled with `bg`, placed like a `line` (a float sized to its
own bounding box, taking no room and no input). Concave outlines fill
correctly; a shape with more than eight vertices is two polygons (a pie is
wedges, an area chart is a strip of quads); a stroked outline is a closed
`line` over it. `transition` eases the fill, and with `slide` the position.
It is hit by its outline ([ADR 0026](adr/0026-hit-testing-by-shape.md)):
give a wedge `onClick` or `hoverable` and a press inside it is its own, one
in its bounding box past the arc is the neighbour's — no hit boxes — and
a clickable wedge is a button to a screen reader, so give it a `label`. A
`line` is the same by its stroke, with at least 4 px of grab. On the wire
a polygon is one `fragment` quad painted by a source the core registers
itself, so it costs what a fragment costs and no shader of yours.

[`polygon` element](props.md#elements) ·
[ADR 0025](adr/0025-the-image-is-the-canvas.md) ·
`cargo run --example polygon`

### How do I draw a gradient, a ring, or anything the paint props cannot?

Write a fragment. `add_fragment(wgsl)` validates one WGSL function and hands
back a handle; `<fragment src={id} params={[…]} animate>` is a box that
function paints. The app writes only

```wgsl
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>
```

and reads `in.local`, `in.size`, `in.time` and up to sixteen `params`. kui
owns the rest — the node's rounded box, the clip, the group opacity, the
blend — so it lays out, clips, fades, takes input and holds children like
any box. `animate` is what a fragment reading `time` needs. It has no
intrinsic size, so give it one.

[ADR 0015](adr/0015-a-fragment-element-and-the-painter-it-is-not.md) ·
`cargo run --example fragment`

### How do I draw fifty thousand points, a spectrogram, a heatmap?

Put the data in an image and read it from a fragment. `add_image` once,
`update_image` with the new values whenever they change, and `<fragment
src={fn} image={id}>` — `ui.fragment(fn.with_image(id), ..)`,
`kui_fragment_with` — draws a box whose function reads the image:
`kui_sample_nearest(uv)` is the texel under `uv` (a cell, a sample, a
bin), `kui_sample(uv)` the bilinear blend, and `in.image.zw` how many
texels there are. One node, one quad, however many values; the "canvas"
is the image and the drawing is the function. The core binds the atlas or
the image's own texture, whichever holds it, and the function never knows
which. The same input is every image effect — read the pixels, return
different ones. A removed image under a live fragment draws nothing.

[`fragment` element](props.md#elements) ·
[ADR 0025](adr/0025-the-image-is-the-canvas.md) ·
`cargo run --example fragment`

### My chat prepends history and the view jumps — how do I keep the row I was reading still?

Declare `anchor` on the scroller: `NodeSpec::column().scroll_y().anchor()`,
`<box scrollY anchor>`, `scroll_y = true, anchor = true`,
`KuiSpec.anchor = 1`. The core remembers which child was first in view
and moves the offset by however far that child's edge moved when the
content before it changed size, before the clamp — so a page of history
prepended above the viewport, a row above it growing as its text wraps,
or a virtual list correcting an estimate leaves what you were reading
where it was. Give the rows stable keys (the anchor is found by key), and
know that content *after* the anchor moves nothing: a log that tails
still `set_scroll`s to the end itself. It is CSS's `overflow-anchor`, and
one frame of it is in the corpus.

[`anchor`](props.md#container-props) · `crates/kui-core/tests/anchor.rs`

### How do I make my own controls agree with the stock ones on size?

Read `ui.metrics()` — `env.metrics` in Lua, `ctx.metrics()` in Node,
`kui_metrics` in C — and build from it: `m.radius` is the corner every
stock surface has, `m.control_pad_x` / `m.control_pad_y` the button's
padding, `m.control_text` its label's size, and so on for the field, the
tooltip and the menus. The stock widgets are built from the same struct,
so a density change reaches both at once: `core.set_metrics(Metrics::compact())`
for a dense tool, `Metrics::default().scaled(1.25)` for a slider,
`ctx.setMetrics({ base: 'compact', radius: 3 })` for overrides in Node.
Nothing scales by itself — a metric is logical px before `env.scale`,
which is the renderer's — and nothing in the OS is followed; the stock
set is what the widgets always drew, so an app that sets nothing changes
nothing.

[Metrics](props.md#metrics) ·
`cargo run --example metrics`

### How do I let an extension draw inside my view?

Declare the place: `ui.slot("fs/panel")` is a position among the host's own
children, filled then and there by the extension the name addresses, with a
`Value` of parameters in and replies out. The host decides the namespace an
extension is loaded under (`extension_as("fs", ext)`), so the same plugin
loaded twice is two namespaces and two sets of slots. An extension whose
slots are not known when it loads — views registered at runtime, one slot
per pane — lists `"*"` and fills every name declared under its namespace;
and every event says which slot's fill drew its node (`ev.slot`), so a
host with one extension across many panes routes by pane without the
extension stamping its payloads.

[ADR 0014](adr/0014-slots-an-extension-fills-in-place.md) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08) ·
[alpha.13](../CHANGELOG.md#010-alpha13-2026-09-15)

### How do I redraw when a thread has new data?

Take the `Waker` the loop hands `App::setup` and clone it into the thread —
a PTY reader, a file watcher, a socket — and call `wake()` when what `view`
will show has changed; the loop draws, and nothing else ever wakes it. A
host that owns the loop blocks on the same three things with
`pump_until(deadline)`: an OS event, a wake, or the deadline.

[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How do I save something as the window goes?

`App::teardown(&mut self)`: called once as the main window goes for good
— its close button, Quit from the menu or the dock, `WindowCommand::Close`
on it, a pumped runner ended — before `run` returns or the process exits.
A `:q` of your own that already saved sets a flag and skips it. There is
no `Ui` by then and nothing draws. A `Drop` on the app is not the same
thing: on a Mac, Quit ends the process from `applicationWillTerminate`
and `run` never returns. A crash under `run` does not reach it; under a
pumped runner the drop retires the runner, so a panic unwinding through
the host does.

Node: `teardown(model)` in `runWindowed`'s config, run once with the
model from inside the pump that saw the window go — under ⌘Q the
promise never resolves and nothing after `await runWindowed(...)` runs,
not even `process.on('exit')`, so this is the only place. `createApp`
takes the same field and `app.teardown()` runs it for a headless
drive. C: `kui_on_teardown(fn)` before `kui_run`, called once with the
run's `user`; nothing after `kui_run` runs under ⌘Q either.

[alpha.16](../CHANGELOG.md#010-alpha16-2026-09-20)

## Test it

### How do I get the settled frame instead of frame 0 of a transition?

Headless, `app.runOut(maxMs?, stepMs?)` draws once, then advances in frame
steps until `animating()` is false, and returns the milliseconds it spent —
a frame that applies a change is frame 0 of its transitions, so a `render()`
straight after a dispatch is the *start* of the motion. A window runs on the
wall clock and cannot be advanced, so the loop `runWindowed` builds answers
the same question with two promises its own pump resolves: `await
app.settled(maxMs = 10_000)` is `runOut` for a window, and `await
app.frame()` is the next pump, whether or not it drew —
`win.frameStats().framesTotal` moving is the paint. The cap resolves rather than throws,
with `animating()` still true and the milliseconds it waited — a view
holding a `repeat` keyframe never settles, and that number is how a test
says so.

[alpha.8 `runOut` entry](../CHANGELOG.md#010-alpha8-2026-09-07) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How do I move time in a test?

`app.advance(ms)` is the window's timer by hand: every `tick` inside the span
fires, the frame clock behind `transition` moves with it, and the app
re-renders. `ctx.setTime` under a loop throws and names `advance` — the loop
owns the clock — and `startTime` in the options pins where that clock starts
so assertions on `tick.msg(now)` are exact.

[alpha.8 `**What breaks.**`](../CHANGELOG.md#010-alpha8-2026-09-07)

### How do I test the real window, not a headless core?

`win.quads()` hands back the frame the shipping driver actually painted
(`decodeQuads` turns the buffer into rects), because `quads()` lives on the
shared surface rather than on the headless `Ctx`. Drive that window with
`access(key, action)` — `access(key, 'click')` activates a node,
`access(key, 'setValue', text)` types into an editor — since `click`, `type`
and `key` are refused on a window, which the OS drives and which says so
rather than pretending.

To run one in CI it has to close itself. `setup` is handed both the window
and the loop, so a test counts pumps and then closes it —
`runWindowed`'s promise resolves with the final model, and the process
exits with a code:

```ts
await runWindowed(app, {
  setup: (win, loop) => (async () => {
    for (let i = 0; i < 5; i += 1) await loop.frame();
    assert.equal(win.env().system.motion, 'full');
    assert.ok(decodeQuads(win.quads()).length > 0);
    win.close();
  })(),
});
```

`loop.frame()` resolves after the next pump, whether or not it drew —
`win.frameStats().framesTotal` moving is the paint, so a test that needs one
loops on `frame()` until it moves; `loop.settled()` waits for a pump that
left nothing animating, and returns how long it waited.
Neither needs an environment variable and neither is a build of kui: the
app asks, in its own code, which is the only place that knows a window is
being opened to be looked at rather than used. (`KUI_SMOKE_FRAMES` is the
Rust runner's own version of this, live in a dev build — for the examples
in this repository, which have no test around them to do the asking.)

A test of the window's idle pace ("a stopped window paints about once a
second") reads `frameStats()` twice, a second apart. `framesTotal` and
`pumps` give the rates. `wokenPumps` says whether the desktop touched the
window in that second: a focus change, the pointer crossing, a resize.
Each of those resets the driver's backoff exactly as a regression would.
If `wokenPumps` did not move, every frame in the second was the app's
own. If it moved, measure another second instead of failing.

Run under `KUI_DEVTOOLS=1` and the frame holds the dock's quads too.
`win.hostArea()` is where the frame put the app, `{x, y, w, h}` in
logical px — right of the pane under a left dock — and the quads are
physical px, so a check that nothing of the app's overflows filters by
the rect times the scale. The one quad of the app's outside it is the
root's `bg`, which is the window's background too and fills the whole
window beneath the pane:

```ts
const r = win.hostArea();
const s = win.env().viewport.scale;
const mine = decodeQuads(win.quads()).filter((q) =>
  q.x >= r.x * s && q.y >= r.y * s && q.x + q.w <= (r.x + r.w) * s && q.y + q.h <= (r.y + r.h) * s);
```

[`access` event](props.md#events) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07) ·
[alpha.19 `### Added`](../CHANGELOG.md#010-alpha19-2026-09-26)

### How do I trade smoothness for latency, or the other way?

You mostly do not have to (backlog C47). A window keeps two frames queued
ahead of the one on screen, so every vsync gets a frame even when little
is drawn. On macOS 14+ the Rust and C runners start frames that run back to
back (an animation, a drag, a scroll) at the display's vsync, so the queued
slot is slack and not a delay: 17.5–19.2 ms from a frame's state to the
screen on an M3 Pro, as quick as the old single-frame queue. A frame drawn
from idle, such as a keystroke, is drawn at once either way.

A Node window turns its event loop from a timer, where the display cannot
start the frames, so there the queued frame costs a vsync (27.6 ms against
19.3). For a view where a drag must track the pointer as closely as
possible, such as a drawing canvas or a splitter, ask for one:
`frameLatency: 1` in `WindowOptions`, `kui_native::app("t").frame_latency(1)`, or
`frame_latency = 1` in `KuiRunConfig`. It loses the odd vsync at light load
(1–6% of them). `KUI_FRAME_LATENCY=1` or `2` and `KUI_FRAME_PACING=0` in the
environment compare the options on the same build.

On Windows a window keeps one frame queued, not two: there
one already delivers every vsync, light load or heavy, so the second would
only be a vsync of latency. `frame_latency(2)` asks for it anyway.

### How do I see what a window draws for a user who asked for less motion?

Pin it at the launcher. A window's `env.system` is the OS's — the runner
writes the real reading before every frame, which is why a window has no
`setEnv` and never will: anything pushed at its core is gone by the next
view. So the pin goes in where the window is opened, as the app asking in
its own code, and the runner merges it over the OS's reading inside that
per-frame write:

```ts
await runWindowed(app, {
  system: { motion: 'reduced' },
  setup: (win, loop) => (async () => {
    for (let i = 0; i < 5; i += 1) await loop.frame();
    assert.equal(win.env().system.motion, 'reduced');
    win.close();
  })(),
});
```

Rust is `kui_native::app("mine").system(SystemEnv { motion: MotionPref::Reduced,
..Default::default() })`; C calls `kui_env_set_system` on the context it
hands `kui_run_with`. The partial is the one `Ctx.setEnv` takes, so the
branch you assert headless and the window you then look at read one
spelling — and every field you leave out, `'unknown'` or null is *not
pinned*: the appearance, accent and locale stay the OS's, a change to one
of them still arrives as the `system` message, and that message carries
your pin with it, since it is the whole reading. The examples' harnesses
take `--motion reduced` for the same look at any example.

It is an option and not an environment variable on purpose, for the reason
`KUI_SMOKE_FRAMES` is kept out of a shipped build: an app you ship should
not change its motion because of a variable in the environment it was
launched from. If the user *did* ask for less motion, the OS says so and
nothing needs pinning; the pin is for looking, on a machine whose owner
did not.

[`system.*` rows](props.md#env) ·
[`system` event](props.md#events) ·
[alpha.12](../CHANGELOG.md#010-alpha12-2026-09-14)

### How do I see what the core thinks is misconfigured?

Every silent misconfiguration comes back as data — `{ code, key, message }`,
one per distinct (code, node) pair — and the loop drains the surface's
`warnings()` after each frame into `app.warnings`, which is the assertion
point (a driver of your own calls `ctx.warnings()` / `win.warnings()`
itself). A test that asserts it is empty catches a grow weight with nothing
to split, a transition on an unkeyed list item, two nodes sharing a key, a
prop an element does not read.

[Warnings table](props.md#warnings)

### How do I use one font in every headless core of a suite?

Register it in `setup` and read its id in `init(surface)` into the model, and
every core in the file gets the same treatment without a global: `setup` runs
against that surface before the first frame, `init` is handed that surface
after it, and `addSystemFont` is idempotent per family, so one `setup` shared
by every core registers `"Antonio"` once per session and hands back that
session's id — whether the family came from `loadFontsDir` or was already
installed. What does not work is a module variable set once and read by every
view: a font id belongs to the session that registered it, and a core from
another one shapes it as sans and raises `foreign-resource`. A helper that
re-points that global at each core before rendering it is the shape this
answer replaces — it leaves the cores rendered earlier holding an id from a
session they are not in.

It is also what makes a baseline portable. Headless text is shaped against
the machine's installed fonts — a text that names no font gets the first
family of a per-OS list the machine has — so the same label measures a
fraction of a pixel apart on two machines, and a width or a quad recorded on
one fails on the other. A suite that compares against numbers taken
elsewhere loads its font file (`loadFontFile`, or `loadFontsDir` and
`addSystemFont`) and names that font on every text.

[Resources](props.md#resources) ·
[`foreign-resource`](props.md#warnings) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I size something to its text before the frame exists?

`measureText(content, style, maxWidth)` on the surface returns what layout
would give the same `<text>`, so a breakpoint assertion is arithmetic rather
than a screenshot. It is the same object `view`'s third argument is, so a
test measures exactly what the view measured. (A column sized to its widest
label no longer needs it: that is
[a table](#how-do-i-line-up-the-columns-of-a-keyvalue-list-or-any-table).)

[`text` element](props.md#elements) ·
[alpha.5](../CHANGELOG.md#010-alpha5-2026-09-03)

## Ship and upgrade

### How do I get completion for the Lua views in my editor?

Write `kui_lua::luals_meta()` into a directory and put the directory on
lua-language-server's `workspace.library`. The file declares every
prelude constructor with its doc and a `kui.Props` class of every prop,
typed and documented from the schema, so `row {` completes its props
and `row`, `text` and the rest are no longer undefined globals. Write
it again after an upgrade; it is generated, never edited.

[`props.md`](props.md) ·
[alpha.17](../CHANGELOG.md#010-alpha17-2026-09-25)

### How do I give my windows the app's icon?

Tell the launcher, once, and every window it creates carries it:
`kui_native::app("t").icon(rgba, w, h)` with straight RGBA pixels, row by row
— something a taskbar shrinks cleanly, 64 to 256 px, rendered from your
drawing at build time or decoded from a PNG you ship — and, for a
Windows program, `.icon_resource(1)` too. A Windows program's icon is a
resource linked into its executable (a `1 ICON "app.ico"` line in its
`.rc`, compiled by `embed-resource` or the like in `build.rs`), which is
what Explorer and a shortcut draw; the window gets it only when told,
since winit registers its window class with none. The resource wins
there, and the title bar and the taskbar each load the `.ico`'s frame
for their size; X11 shows the pixels. Pass both and let each platform
take its own: macOS draws the bundle's `.icns` in the Dock and Wayland
the `.desktop` file's icon, and neither has a window icon, so both
calls are nothing there. Node: `icon: {rgba, width, height, resource}`
in the window's options — under `node.exe` the resource is Node's, so
only a packaged app has one of its own. C: `kui_set_icon(rgba, w, h,
resource)` before `kui_run`. Pixels that are not the size are refused
with the reason (`Launcher::try_icon` for pixels from outside the
program).

[alpha.18 `### Added`](../CHANGELOG.md#010-alpha18-2026-09-25)

### How do I stop the console window on Windows?

A Windows binary is built for one of two subsystems, and Rust picks
*console* unless told otherwise: the app gets a console the moment it
starts — the black window that pops up beside its own when it is opened
from the Explorer — made by the loader before a line of the app has run,
so nothing the app does at runtime can prevent it, only close it after
it has been seen. The one way not to have it is to say so in the binary
crate, at the top of `main.rs`:

```rust
#![cfg_attr(windows, windows_subsystem = "windows")]
```

(A C host says the same to its linker, `/SUBSYSTEM:WINDOWS`; a Node or
Lua app runs inside `node.exe` or `lua.exe`, whose console it is.) What
that used to cost is the terminal: a windows-subsystem process started
from a shell has no standard output at all, so `println!`, a panic and
the runner's own `kui:` lines went nowhere. The runner attaches such a
process to the console of the shell that launched it, once, when it
starts the shell — so what prints from then on lands there, after the
prompt, since a shell does not wait for a windowed process. Two things
it cannot bring back: a Rust app's own `println!` *before* it calls the
runner's `run` (there was no console yet), and a C host's own `printf`,
whose C runtime set its stdio up at startup with no handle and does
not look again — the runner's `kui:` lines reach the terminal, the
host's own lines need its own `AttachConsole` and `freopen`. A Ctrl+C typed at that prompt is
the shell's and the app ignores it; closing that terminal ends the app,
as it ends a console build — Windows terminates every process on a
console it closes, and nothing an app does prevents it — so a session
meant to outlive the terminal is opened from the Explorer or a launcher.
From the Explorer there is no parent console and nothing is shown;
`cargo run`, the smoke round and `> log.txt` hand the process its output
and it keeps what it was given.

[alpha.17 `### Added`](../CHANGELOG.md#010-alpha17-2026-09-25)

### How do I pin the version I tested?

Write the exact version: `^0.1.0-alpha.7` and `~0.1.0-alpha.7` both admit any
prerelease of the same `0.1.0` tuple, so without a lockfile both float to the
newest alpha — that is npm's semver, not a difference between the two
spellings. The template's `^` is a floor on purpose; an app that wants the
release it tested pins it exactly and commits its lockfile.

[alpha.9 `### Changed`](../CHANGELOG.md#010-alpha9-2026-09-08) ·
[every release](../CHANGELOG.md)

### How do I find out a release happened?

`npm view @qxuken/kui version` is the query, and `npm outdated` inside your
app is the same answer against what you have installed: every alpha takes the
`latest` dist-tag as well as `alpha`, so the default-tag commands every other
package answers work here too. `npm view @qxuken/kui@alpha version` is the
fallback if `latest` is ever missing — a package without one prints nothing
and exits 0, which reads like "no such release" and is not. The repository
README's *Releases* section is what the registries and the tags are written
down in.

[every release](../CHANGELOG.md)

### How do I upgrade to a new alpha?

Read the release's `**What breaks.**` list first — from alpha.9 on it opens
with one bullet per break, naming the symbol, so it can be grepped before the
paragraphs are read — and then its "what you can delete". That second list
names the *behaviour* the release removed the need for, not the code you
wrote around it: a workaround that accreted two purposes only sheds the one
the release addressed, and your own tests are what say which.

[Changelog](../CHANGELOG.md)
