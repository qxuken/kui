# Status / next

What v0 does not do, by area, with the ADR or backlog entry each limit
belongs to.

**Paint.** Fill, border, four radii, group opacity and one outer drop shadow
per node ([ADR 0005](adr/0005-the-paint-vocabulary.md)), and one stroke
primitive: a round-capped segment, which the `line` element emits one of per
straight piece of a segment, a polyline or a curve flattened in the core
([ADR 0010](adr/0010-a-segment-primitive.md)), and one escape hatch:
the `fragment` element, a box a registered WGSL function paints
([ADR 0015](adr/0015-a-fragment-element-and-the-painter-it-is-not.md)),
which is where rings, noise and shimmer live. A box takes a `gradient`,
linear or radial, painted over its `bg`
([ADR 0042](adr/0042-a-gradient-is-an-image-the-core-paints.md)): defined
on the box's unit square, rasterized once into the atlas and drawn as an
image quad, so it does not tween, a hard stop is soft, and there is no
conic one and none on a `path`, a `line` or a text — those are a
fragment's. There are no inset or
multiple shadows, and the single shadow is not knocked out of the middle of
the shape, so a translucent background shows it through. A stroke takes a `dash` (marks and gaps as seen, the pattern kept along
the whole stroke; backlog V2) on a `line` and on a `path`. There are **no
arrowhead caps**: a line is segments and nothing else, a
translucent polyline double-blends where its caps overlap at a join, and its
width does not tween (its colour does). A fill of any shape is a `path`
([ADR 0040](adr/0040-a-path-is-a-mask-in-the-atlas.md)): SVG path data,
filled by either rule and stroked, rasterized once per shape and scale
into the glyph atlas and drawn as a mask quad — so a hover or a colour
tween is free and a shape that changes every frame pays a raster each,
from a texture of its own; one that only turns does not, since `rotate`
turns the quad and not the mask
([ADR 0041](adr/0041-a-mask-turns-about-its-centre.md)), though a
`rotate` does not tween. A `polygon` of at most eight points
([ADR 0025](adr/0025-the-image-is-the-canvas.md)) is the fill that costs
nothing per frame however it moves: concave is fine, more vertices is two
polygons, and two sharing an edge show a hairline. All three take input **by shape**
([ADR 0026](adr/0026-hit-testing-by-shape.md)): a press within a
stroke's width (at least 4 px of grab) or inside an outline hits it, one in
the bounding box off the shape falls through, and a stroke or fill with a
click is a button to a screen reader, so name it. A raster the app made — a frame of video, a plot, a
page — is an `image` whose pixels it replaces; there is no drawing-command
canvas and no callback over the GPU. Opacity is a per-quad
alpha multiply rather than an offscreen composite, so overlapping pieces of one
faded subtree show their seams. There is no z-index: floats stack in tree order.
A fragment is one draw call of its own, so a hundred of them is about 0.7% of
a 120 Hz frame and ten thousand is the wrong tool; it cannot read what is
behind it (no backdrop blur), sample anything but its own parameters, run a
second pass, or hit-test per pixel — its edge is its box, like everything
else here.
Transitions cover sizing, colors, radius, opacity, shadows, position (`slide`,
`enter`) and departure (`exit`) — a node the view stops declaring is copied out
of the last frame that had it and replayed frozen, in its place and inert
until its transition ends. `exit` is opt-in per node, capped at 4096 departing nodes at
once — a frame's removal past that animates whole or not at all (ADR 0012) — and a
ghost cannot be re-laid-out: `exit`'s `width`/`height` resize the
departing node's own box and nothing inside it moves.

A `radius` on a node that clips or scrolls rounds the clip too, so a rounded
card's children stay inside its corners. What that gives up is nesting (the
inherited clip is one rect and four radii, so a corner both clippers round
takes the tighter of the two, and a corner an ancestor's straight edge crosses
goes square). Hit-testing follows the corners since
[ADR 0026](adr/0026-hit-testing-by-shape.md): a click in the dead corner
of a rounded card reaches what is under it, as it looks like it should.

**Theme.** The colours a view paints with are named roles, derived from the
two facts the OS reports — the appearance picks a base, the accent recolours
the family that comes off it
([ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md), and
the Theme table in [docs/props.md](props.md) for every role and its value
on both bases). The core still acts on the appearance exactly as much as it
did before, which is not at all; the **widgets** do, so a button, a context
menu, a tooltip, a field, the scrollbars, the focus ring and a `<text>` with no
`color` all follow the OS without an app writing a line about it. `ui.theme()`
in Rust, `env.theme` in Lua, `ctx.theme()` in Node, `kui_theme` in C — the
same twenty-three roles, generated from one table. Three sources, defaulting
to the OS for both facts: follow it, follow its light/dark with an accent of
your own (`set_accent`), or pin a palette that follows nothing (`set_theme`).
An unknown appearance takes the dark base, which is exactly what kui painted
before there were themes — so a host that reports nothing sees no change. What
this is **not** is a cascade: there is no inherited colour, no `var()`, no
numbered ramp. A role is read off `ui` and put in a `bg`, and an app's own
non-role colours — a highlighter's keywords, a chart's series — stay the app's.

**Metrics.** The palette's other axis: the sizes the stock widgets are built
from — a button's padding and text, every stock surface's radius, a menu's
width, the titlebar's height — as sixteen roles in one `Metrics`, read as
`ui.metrics()` / `env.metrics` / `ctx.metrics()` / `kui_metrics` and set with
`set_metrics` (the [Metrics table](props.md#metrics) has every one with
its stock and compact value). An app's own control that reads `m.radius`
agrees with the stock button at every density. A metric never scales by
itself — it is logical px before `env.scale`, which is the renderer's — and
density is the app's to choose (`Metrics::compact()`, `scaled(f)`); the
default is the constants the widgets always had, which is what the corpus
pins.

**Tokens.** The app's *own* names, beside both — for the app whose palette
is the design and has nothing to do with `surface`
([ADR 0027](adr/0027-tokens-beside-the-theme.md)). A colour token has a
light and a dark half the core picks by the appearance in effect (or one
value for both), a length token is logical px; declared whole —
`ctx.setTokens({ colors: { peach: '#ffcc99', ink: { light, dark } },
lengths: { sideW: 132 } })`, `Core::set_tokens`, a `tokens` global in a Lua
script, `kui_tokens_set` in C — into a table **per origin**, so an
extension's names are its own and a guest cannot shadow its host. Then
**referenced by name in any colour or length prop**: `bg="$peach"`,
`width="$sideW"`, a `pad` edge, a border's width and colour, a text's `size`,
a span's `color`. The binding that lowers the node resolves the name through
the core's table, so the core's own path never sees one; on Node's wire the
reference costs zero bytes (a tag on the prop id). `defineTokens` returns the
names typed, so `T.peech` does not compile; the roles take the same spelling
(`'$surface'`, `'$radius'`), and a declared name a role owns is refused with
`reserved-token`; a name nothing declared raises `unknown-token` once and the
slot keeps its default. The devtools list the tokens with their swatches and
print a token's name after the value it painted. C declares and reads
(`kui_token_color` / `kui_token_length`); its props carry no reference.

**Layout.** Wrapping is rows only, for the pass-order reason in [design.md](design.md#layout): a
**column** that outgrows its height is still one line, so it shrinks its `Fit`
children toward their `min` (or overflows) rather than moving anything into a
second column (C12). `Dir` is `Row` or `Column` with no reverse. The main
axis takes start/center/end and the three spreads (`spaceBetween`,
`spaceAround`, `spaceEvenly`, CSS's `justify-content`), and a row's cross axis
`baseline` besides start/center/end (C13); a column's cross axis has no
baseline, and there is no `align-content` (a wrapping row's lines always share
the leftover cross space equally) and no per-child `align-self`. A baseline is
the first line of the first text down a child's first-child chain; a child with
no text aligns by its bottom edge. `aspectRatio` (C14) sizes one axis from the
other — a fit height from the final width, or a fit width from a fixed height —
and not a fit width from a `grow` or percent height, which is resolved only
after every width is.

**Input.** Pointer buttons: the secondary one is routed to `on_context_menu`
(C2); every non-primary button — secondary, middle, back, forward — reaches a
node that claims it with `on_button` as press, move and release events,
captured by that node until the release (F105), which is a terminal's
middle-click paste and its mouse reporting, a middle-click-to-close and a
right-drag. There is no per-button `on_click`: a click is the primary
button's. Touch and pen are not input modes
of their own: a finger on a touchscreen arrives as whatever the platform
synthesises as mouse input, so a tap presses and clicks and nothing past that
exists — no multi-touch, no pinch, rotate or two-finger gestures, no pressure
and no stylus tilt (X3). v0 is desktop-first.

Keys are layout-resolved characters and a closed list of names, with the
US-QWERTY position beside them as `physical`, so a keymap can bind the finger
rather than the label ([ADR 0002](adr/0002-keyboard-focus-as-data.md),
decision 11). A press says which of a key's twins it was — the left or
right modifier, the keypad's digit or the main block's — and what Caps Lock
and Num Lock held, and the modifier keys themselves reach a sink that asks
for them (`modifierKeys`; decisions 14–16). A key that neither the layout
nor the position names is still dropped rather than delivered as
`unknown`. Presses and
releases route to the key sink and no further (C9): the core keeps no "which
keys are down" query, since the app that asked for the pair already has one.

A key sink owns its keyboard: while it holds focus every press is its data,
Tab included, and it hands the ring on with `focusNext` when it wants to
(ADR 0002, decision 3). A focused control owns only the keys the core
presses it with — Enter and Space where there is something to activate, a
slider's arrows, a composite's arrows, Home, End and type-ahead — and every
other press, chords included, walks up to the nearest enclosing sink
([ADR 0011](adr/0011-keys-bubble-to-the-enclosing-sink.md)), which is
how an app shell keeps its shortcuts while the Tab ring works underneath
it.

The pointer shape is declared, not derived: the `cursor` prop says what the
pointer is over a node, the core resolves whichever declaration is under it
per frame, and a node that declared nothing is the arrow whatever it does — a
clickable or draggable box included, as a native control is. The one shape
the core implies is the I-beam over an editor or a `selectable` scope; the
stock `button` declares its own hand. The list is closed (`text`, `pointer`,
`grab`, `grabbing`, `notAllowed`, the four resize arrows and the default), so
there are no custom bitmap cursors and no hiding the pointer (C3).

**Focus and accessibility.** Accessibility, keyboard focus and modality are
data ([ADR 0001](adr/0001-accessibility-as-data.md),
[0002](adr/0002-keyboard-focus-as-data.md) and
[0003](adr/0003-modal-surfaces.md)), and composites are derived from the
roles rather than declared
([ADR 0007](adr/0007-composite-keyboard-patterns.md)): a tab list, radio
group, menu or picker list is one Tab stop with arrows, Home/End and type-ahead
inside it, and `initialFocus` says which control a modal opens on. The focus
ring is still not a prop — no node styles the default one, and the geometry is
fixed — but its *colour* is now `theme.focus_ring`
([ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md)
revisiting ADR 0002): a pale blue ring reads on a dark page and is invisible
on a light one, and a focus indicator nobody can see is not one. ADR 0007's own follow-ups are the rest: grid navigation
(Left/Right into a row, Up/Down between rows, which wants a `grid` / `row` /
`cell` vocabulary), a `radio-without-group` warning, and multi-select.
Submenus are built for the core's own menus (backlog F128) — the context
menu and the menu bar, drawn or the platform's — and not as a derived
relation an app's own `menu` composite gets; C's `KuiMenuItem` has no
submenu field.

**Text and editing.** Editing covers caret blink, double/triple-click
word/line select, scroll-caret-into-view, inline IME composition, Tab focus
traversal, and undo/redo (operational deltas with typing/delete coalescing —
the widget owns its buffer, so it owns its history; hosts with their own text
model take raw chords through `on_key` and bring their own). Layout queries
stop at the node: `measure_text` and `on_layout` give whole-string and
whole-node rects, not the boxes of lines or glyphs inside a paragraph.

**Audio.** One-shots, loops, volume, pause and a finished-playback event.
Sounds decode fully into memory, and synthesis, effects, positional audio and
disk streaming are out of scope.

**Windows.** A frame declares which windows exist, by name, and the runner
opens them; `SetSize` and `Focus` are commands an app queues, because the
user owns a window's geometry once it exists
([ADR 0004](adr/0004-multi-window.md)). A window is `kind: "normal"`
or `kind: "popup"` — a menu surface, borderless, off the taskbar, owned by
the window that declared it, placed in screen coordinates against an
`anchor` the app already has from `onLayout`, and non-activating, so the
field that opened it keeps its focus ring while the arrows walk the list. A
press outside it or Escape is the same `dismiss` a `modal` node gets and
closes nothing: the app stops declaring the window.

**A popup is the exception, not the default.** It costs an OS surface, a
swapchain, a `Core` and an accessibility adapter, where a float costs one
tree and one draw call — so a dropdown, a tooltip and a context menu stay
in-window (`FloatConfig::fit` flips across the anchor and clamps what still
overflows, a `modal` float takes the dismiss) until they provably do not
fit. The three cases that do not are what the kind exists for: a list
taller than the window, a menu near an edge with nowhere in-window to sit,
and a panel the user wants beside the app.

A window can be **translucent** (`Launcher::backdrop`, backlog F126): the
frame cleared to nothing over a material the OS draws behind the window —
an `NSVisualEffectView` on macOS, Mica, Mica Alt or Acrylic on Windows 11
22H2 and later — or over the bare desktop. It is the launcher's, for every
window of the app at once: not a per-window declaration, not changeable
while the app runs, and not on a popup or the devtools' window. Linux has
no material (the window is merely see-through where the compositor and the
Vulkan surface allow), a C host under `kui_run` cannot ask for one, and on
Windows the device then presents through DirectComposition.

There is no window **position** an app can declare or read, no app-modal
window (decision 10 keeps modality per window), and no native menu bar.
Non-activating is a request, not a guarantee: on macOS a window that is
ordered front becomes key, so the runner hands the keyboard straight back
to the owner — the owner's `focused` stays true, but the popup does hold
key status for a moment.

The glyph atlas and the shaped-text cache stay per window (ADR 0004 step 1,
which amended decision 2 to say so): a `CachedText` entry stamps the atlas
epoch it was packed against, so it is only valid for that window's page, and a
second window re-rasterizes the same glyphs. Fonts, images, sounds and the one
audio queue are the session's and are shared.
