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

### How do I clamp text to one line, or to three with an ellipsis?

`wrap="none"` breaks nowhere, `maxLines={3}` lays out at most three lines,
and `ellipsis` ends the last one with an ellipsis when the text is cut off.
All three are text props, so they read on `<text>` and `<edit>`, and a span's
styles do not interrupt them — spans shape as one paragraph.

[`wrap` / `maxLines` / `ellipsis` rows](props.md#text-props)

### How do I show a 100k-character line, or a paragraph that long?

Hand it over as one `text` node. A plain text of 4096 bytes or more with no
line breaks is shaped in ~1 KB chunks as they come on screen, so it costs
the screenful it shows and a keystroke into it costs the chunk it lands in;
under `wrap: word` the rows are broken from the chunks' positions, so a
paragraph costs the rows it shows. Its size is an estimate until the chunks
shape (exact under monospace), so a scrollbar can move a little as they do.

[`text` row](props.md#elements) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How do I show a list of ten thousand rows?

`virtualColumn` (JSX), `virtual_column` (Lua, Rust): declare the rows that
can be seen and two spacers holding the height of the rest, so the frame
costs a screenful however long the list is — about 16 µs against 3.9 ms
for ten thousand rows built whole. It takes the row count and one row's height;
the rows come from a callback it runs for the ones it slices, and it keys
each by its data `index`, so a row keeps its hover, focus, edit buffer and
tweens as the built range slides over it. `setScroll(key, 0, i * rowH)` is
"scroll to row `i`" — `reveal` finds nothing for a row nobody declared.

In Node it also declares the node that makes `view` run again when the
container scrolls: the wheel raises no event and a window redraws by
re-lowering the tree it was handed, so nothing else would. Those events
never reach `update`.

```jsx
virtualColumn(ctx, { key: 'log', rows: lines.length, rowH: 28 }, (i) => (
  <box width="grow" height="grow" onClick={{ kind: 'pick', row: i }}>
    <text>{lines[i]}</text>
  </box>
))
```

[`index` row](props.md#composite-props-hand-written-per-binding) ·
[`virtual_list.tsx`](../examples/node/widgets/virtual_list.tsx)

### How do I do that when the rows are not all the same height?

`widgets::virtual_rows` (Rust): the same list over prefix sums instead of a
stride. It calls a `measure(ui, i, width)` for the rows it is about to build
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

[`exit_budget.rs`](../examples/rust/features/exit_budget.rs) ·
[`virtual_list.rs`](../examples/rust/widgets/virtual_list.rs)

### How do I draw a terminal's screen?

`cells`: rows × cols of `{ch, fg, bg, flags}` and a cursor, one node — a
200×50 screen costs ~60 µs a frame with every character new, against ~2 ms
as a text node per cell. A click or drag on it carries `cell: {row, col}`
in its payload, and the node reads as a `terminal` with the rows joined as
its value. Inverse, dim and a wide cell's blank spacer are the app's.

[`cells` row](props.md#elements) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

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

[`menuBar` element](props.md#elements) ·
[ADR 0018](adr/0018-a-menu-bar-the-app-declares.md) ·
[examples/rust/widgets/menu_bar.rs](../examples/rust/widgets/menu_bar.rs)

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

### How do I get an IME into an editor I own?

An `onKey` sink drawing `line` rows with `caret` hears a composition as
data: `{kind:"preedit", text, cursor, tag}` while it is composed (an empty
`text` is the composition ending) and `{kind:"text", text, tag}` on the
commit. Plain typing is not a commit — the `key` event already carries what
the press would insert — so nothing arrives twice. The candidate window is
anchored at the `line` carrying `caret` for you.

[`text` and `preedit` events](props.md#events) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

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
across a tree.

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
an `ended` that cannot come. Stop what the view no longer needs instead of
releasing it, and release short sounds.

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
whatever you like. At 1080p the cost is your own 8 MB copy of the frame and
the upload; the core adds nothing measurable.

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
loaded twice is two namespaces and two sets of slots.

[ADR 0014](adr/0014-slots-an-extension-fills-in-place.md) ·
[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

### How do I redraw when a thread has new data?

Take the `Waker` the loop hands `App::setup` and clone it into the thread —
a PTY reader, a file watcher, a socket — and call `wake()` when what `view`
will show has changed; the loop draws, and nothing else ever wakes it. A
host that owns the loop blocks on the same three things with
`pump_until(deadline)`: an OS event, a wake, or the deadline.

[alpha.9](../CHANGELOG.md#010-alpha9-2026-09-08)

## Test it

### How do I get the settled frame instead of frame 0 of a transition?

Headless, `app.runOut(maxMs?, stepMs?)` draws once, then advances in frame
steps until `animating()` is false, and returns the milliseconds it spent —
a frame that applies a change is frame 0 of its transitions, so a `render()`
straight after a dispatch is the *start* of the motion. A window runs on the
wall clock and cannot be advanced, so the loop `runWindowed` builds answers
the same question with two promises its own pump resolves: `await
app.settled(maxMs = 10_000)` is `runOut` for a window, and `await
app.frame()` is the next painted frame. The cap resolves rather than throws,
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
and the loop, so a test counts presented frames and then closes it —
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

`loop.frame()` resolves on the next pump that painted; `loop.settled()`
waits for one that left nothing animating, and returns how long it waited.
Neither needs an environment variable and neither is a build of kui: the
app asks, in its own code, which is the only place that knows a window is
being opened to be looked at rather than used. (`KUI_SMOKE_FRAMES` is the
Rust runner's own version of this, live in a dev build — for the examples
in this repository, which have no test around them to do the asking.)

[`access` event](props.md#events) ·
[alpha.8](../CHANGELOG.md#010-alpha8-2026-09-07)

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

[Resources](props.md#resources) ·
[`foreign-resource`](props.md#warnings) ·
[alpha.7](../CHANGELOG.md#010-alpha7-2026-09-06)

### How do I size something to its text before the frame exists?

`measureText(content, style, maxWidth)` on the surface returns what layout
would give the same `<text>`, so a breakpoint assertion or a column sized to
its widest label is arithmetic rather than a screenshot. It is the same
object `view`'s third argument is, so a test measures exactly what the view
measured.

[`text` element](props.md#elements) ·
[alpha.5](../CHANGELOG.md#010-alpha5-2026-09-03)

## Ship and upgrade

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
