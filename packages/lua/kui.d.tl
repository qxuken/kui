-- kui-lua's view DSL for Teal, generated from kui-core's schema by
-- `kui_lua::teal_decls()`. Regenerate it rather than editing it.
--
-- Save it as `kui.d.tl` where `tl` finds modules and name it the
-- global environment in `tlconfig.lua`:
--
--   return { include_dir = { "types" }, global_env_def = "kui" }
--
-- `tl check view.tl` types a view against it, and `tl gen view.tl`
-- writes the `view.lua` a host loads. `view(env: kui.Env, inst:
-- kui.Instance): kui.Node` and `on_event(ev: kui.Event, at: kui.Envelope)`
-- are the script's own globals; an app's message is a record that
-- extends `kui.Event` (and the `kui.With…` field its event fills),
-- cast from `ev`.

global type kui = record
  record SizeArgPct where self.pct ~= nil
    pct: number
  end

  record SizeArgPx where self.px ~= nil
    px: number
  end

  record SizeMin where self.min ~= nil
    min: {SizeArg}
  end

  record SizeMax where self.max ~= nil
    max: {SizeArg}
  end

  record SizeClamp where self.clamp ~= nil
    clamp: {SizeArg}
  end

  record SizingGrow where self.grow ~= nil
    grow: number
  end

  enum MainAlign
    "start" "center" "end" "spaceBetween" "spaceAround" "spaceEvenly" "baseline"
  end

  enum Scrollbar
    "visible" "hidden" "auto"
  end

  enum Overscroll
    "auto" "contain"
  end

  enum ScrollAxes
    "both" "x" "y"
  end

  enum Window
    "drag" "close" "minimize" "maximize"
  end

  enum Cursor
    "default" "text" "pointer" "grab" "grabbing" "notAllowed" "ewResize" "nsResize" "nwseResize" "neswResize"
  end

  enum Easing
    "easeOut" "linear" "easeIn" "easeInOut" "spring" "bouncy" "smooth" "snappy"
  end

  enum Direction
    "normal" "reverse" "alternate" "alternateReverse"
  end

  enum Wrap
    "word" "glyph" "none" "break-spaces"
  end

  enum UnderlineStyle
    "solid" "wavy" "dotted"
  end

  enum Role
    "none" "button" "checkbox" "radio" "switch" "slider" "tab" "tabList" "link" "heading" "list" "listItem" "image" "dialog" "group" "textInput" "multilineTextInput" "line" "radioGroup" "menu" "menuItem" "terminal" "table" "row" "cell" "columnHeader" "rowHeader"
  end

  enum Expanded
    "collapsed" "expanded"
  end

  enum Live
    "off" "polite" "assertive"
  end

  record Pad
    all: Length
    x: Length
    y: Length
    l: Length
    r: Length
    t: Length
    b: Length
  end

  record Border
    w: Length
    color: Color
  end

  enum FloatWord
    "parent" "viewport" "below" "above"
  end

  enum OptionAsAlt
    "none" "left" "right" "both"
  end

  enum EventKind
    "click" "drag" "key" "text" "preedit" "select" "selectionrange" "contextmenu" "menu" "forceclick" "button" "scroll" "focus" "hover" "drop" "files" "open" "layout" "resize" "window" "system" "fonts" "modifiers" "change" "submit" "cancel" "boundary" "sound" "capture" "dismiss" "component" "range" "query" "typeahead"
  end

  enum DragEventPhase
    "start" "move" "end"
  end

  enum KeyEventPhase
    "down" "up"
  end

  enum KeyEventLocation
    "standard" "left" "right" "numpad"
  end

  enum ButtonEventPhase
    "press" "move" "release"
  end

  enum ButtonEventButtonWord
    "secondary" "middle"
  end

  record ScrollEventMods
    shift: boolean
    ctrl: boolean
    alt: boolean
    super: boolean
  end

  enum FocusEventPhase
    "in" "out"
  end

  enum FocusEventBy
    "pointer" "keyboard" "assistive" "program"
  end

  enum HoverEventPhase
    "enter" "leave"
  end

  enum HoverEventBy
    "pointer" "content"
  end

  enum DropEventPhase
    "enter" "move" "leave" "drop"
  end

  enum SliderChangeEventPhase
    "move" "end"
  end

  enum BoundaryEventKey
    "up" "down" "left" "right" "backspace" "delete"
  end

  enum BoundaryEventEdge
    "start" "end"
  end

  enum SoundEventPhase
    "ended" "refused"
  end

  enum CaptureEventPhase
    "saved" "failed"
  end

  enum DismissEventReason
    "escape" "outside"
  end

  enum WindowEventPhase
    "opened" "closed" "focused" "blurred"
  end

  enum SystemEventAppearance
    "unknown" "light" "dark"
  end

  enum SystemEventMotion
    "unknown" "full" "reduced"
  end

  enum SystemEventAssistive
    "unknown" "none" "listening"
  end

  enum SystemEventContrast
    "unknown" "normal" "more"
  end

  enum WindowEntryKind
    "normal" "popup"
  end

  -- A colour: `0xRRGGBBAA`, `"#hex"`, or a `"$token"` the host declared.
  type Color = integer | string

  -- A length in logical px — any number where the slot is no length, such as `opacity` — or a `"$token"` (`"$disabled_opacity"` among the roles).
  type Length = number | string

  -- A size expression's part: px, a spelling (`"80%"`, `"min(…)"`), `{ pct = n }` or `{ px = n }`.
  type SizeArg = number | string | SizeArgPct | SizeArgPx

  -- A size expression (backlog F109), resolved against the parent's content box: px, `"Npx"`, `"N%"`, `"min(…)"`, `"max(…)"`, `"clamp(MIN, TARGET, MAX)"`, nested — or the same as data, which is never parsed: `{ pct = n }`, `{ px = n }`, `{ min = { … } }`, `{ max = { … } }`, `{ clamp = { MIN, TARGET, MAX } }` (a part a table again, or a spelling). The process keeps 65 536 distinct expressions and never lets one go: past that a new one leaves its prop at its default, with a `size-expressions-full` warning — declare one per layout, not one per frame (backlog RG93).
  type Size = number | string | SizeArgPct | SizeArgPx | SizeMin | SizeMax | SizeClamp

  -- A sizing: px, `"fit"`, `"grow"`, `"N%"`, `{ grow = n }`, `{ pct = n }` (n percent), a size expression (`kui.Size`), or a `"$length"`.
  type Sizing = SizingGrow | number | string | SizeArgPct | SizeArgPx | SizeMin | SizeMax | SizeClamp

  -- A lower clamp: px, `"fit"`, a size expression (`kui.Size`), or a `"$length"`.
  type Min = number | string | SizeArgPct | SizeArgPx | SizeMin | SizeMax | SizeClamp

  -- An upper clamp: px, a size expression (`kui.Size`), or a `"$length"`.
  type Max = number | string | SizeArgPct | SizeArgPx | SizeMin | SizeMax | SizeClamp

  -- A key a call takes: the integer an event carried, or the label a `key` prop declared.
  type KeyRef = integer | string

  -- Every prop a node table may carry; its children at 1, 2, ….
  interface Props is {Node | string}
    -- Horizontal size: px | "fit" | "grow" | "N%" | a size expression — `"clamp(400px, 80%, 1000px)"`, `"min(720px, 100%)"`, `"max(50%, 300)"`, nested — which layout resolves against the parent's content box, the box a percentage takes its cut of (backlog F109). An expression with no percentage in it is a length; a calc does not ease under `transition`. A percentage or an expression gives, with the fit children, when its parent overflows — two `"50%"` children and a gap fit their row (backlog F110) — where a px size keeps its own. A row holding one gives as CSS's flex items do: every child that can give gives in proportion to its size, and stops at its content — the widest thing in it that cannot wrap, a label's longest word — unless `minWidth` says otherwise (backlog RG92). The process keeps 65 536 distinct expressions and never lets one go: past that a new one leaves its prop at its default, with a `size-expressions-full` warning, so declare one per layout — a px size for the part that moves each frame, a splitter's drag — not one per frame (backlog RG93).
    width: Sizing
    -- Vertical size: px | "fit" | "grow" | "N%" | a size expression (see `width`).
    height: Sizing
    -- Lower width clamp: logical px, a size expression (see `width`; a percentage clamp is none until the parent's width is known, as in CSS), or "fit" for the node's own fit width. "fit" under `width="grow"` is a content floor — CSS's `flex: 1 0 auto` — which is what an i3-style tab bar is: tabs that split the bar evenly while they fit and sit at their label's width, scrolling, once they do not. Opt-in, because a fit width is the unwrapped one: a paragraph in a grow column would stop wrapping under it. Left out, a child giving in an overflowing row that holds a percentage or a size expression stops at its content, CSS's `min-width: auto`; `0` lets it go below, CSS's `min-width: 0` (backlog RG92). A fit node across a column is no wider than the column's box — CSS's `fit-content` — down to this floor, 0 left out, unless the column scrolls x, so a text one wrapper deep in a capped card wraps there; "fit" keeps its content's width and runs past (backlog F116).
    min_width: Min
    -- Upper width clamp: logical px or a size expression (see `width`); grow+maxWidth is the responsive-width pattern.
    max_width: Max
    -- Lower height clamp: logical px, a size expression, or "fit" for the node's own fit height. Undeclared, it is the node's content where its column overflows — CSS's `min-height: auto`, none for a node that scrolls or clips — so a row keeps the height of its text; `0` asks for the squeeze back (see `minWidth`).
    min_height: Min
    -- Upper height clamp: logical px or a size expression (see `width`).
    max_height: Max
    -- Space between children along the main axis.
    gap: Length
    -- Children that don't fit the main axis start a new line instead of overflowing or shrinking. Rows only (a column is ignored, with a warning), and never on a scrollX row.
    wrap_children: boolean
    -- Space between wrap lines, across the main axis (`gap` stays the space along it).
    cross_gap: Length
    -- Child alignment along the main axis. `start`, `center` and `end` put the children together; `spaceBetween` deals the free space out between them (none at the ends), `spaceAround` gives each child an equal share split to its two sides, and `spaceEvenly` makes every gap and both ends equal — CSS's `justify-content`. The spread is added to `gap`, and there is none when nothing is free: a `grow` child takes it all, and an overflowing run keeps its gaps. `baseline` means nothing here and lays out as `start`, with a warning.
    main_align: MainAlign
    -- Child alignment across the main axis. On a row, `baseline` lines up the first baselines of the children's text, so a label and a larger value read as one line; a child with no text aligns by its bottom edge, a `grow` or percent height fills the line from its top, and a fit-height row grows to hold the aligned children. A column lays `baseline` out as `start` (as CSS does), and the three spreads mean nothing across an axis — each with a warning.
    cross_align: MainAlign
    -- Width over height — `16/9`, `1` for a square — CSS's `aspect-ratio`. It sizes the axis left `fit`: a fit height is the final width over the ratio (so `width: grow` and a ratio is a box that keeps its shape as the window resizes), and a fit width under a fixed height is that height times it. With both axes declared, or a fit width under a `grow` or percent height, it has nothing it can set and warns. The derived axis is neither shrunk nor fitted to the children, which overflow it; `minHeight: 'fit'` floors it at them. On an image it wins over the pixels' own aspect.
    aspect_ratio: Length
    -- Background fill.
    bg: Color
    -- Corner radius for all four corners (logical px); the per-corner props override it when listed after it. On a node that also clips or scrolls it rounds the clip as well, so children stay inside the corners.
    radius: Length
    -- Group opacity 0..1 (default 1): fades this node and its whole subtree, drawn as one picture and faded once in a window (ADR 0056), so a label on its own fill and overlapping pieces fade as one; a renderer that draws the quads itself without groups fades each quad, and overlapping pieces show their seams. Layout, hit-testing and the access tree are untouched; eases with `transition`, and `enter: { opacity: 0 }` fades a panel in. A node's `tooltip` fades with the node's ancestors and not with the node, so a dimmed box keeps a hint that can be read (backlog F178).
    opacity: Length
    -- Paint this node's background, border, shadow and fragment with each edge on a whole physical pixel: `x` and `x + width` rounded on their own, from where layout put them, as a text's span backgrounds are. Off by default, and a box is drawn where layout put it, so a 1 px `gap` between boxes is there at any scale. On, boxes that share an edge in layout meet on one pixel line, where a join inside a pixel was drawn by halves and left a seam — rows of a band stacked at a pitch that is not whole pixels, or a box that continues a text's selection. Layout, hit-testing, the clip and the children are untouched. A snapped box can draw up to half a pixel from its layout edge and its size can differ by a pixel, so a snapped hairline is 1 or 2 px thick by where it sits.
    pixel_snap: boolean
    -- Drop-shadow color; nothing else about a shadow draws without it. On its own it is a hard shadow exactly behind the node — add `shadowBlur` / `shadowY` to lift it. Outer shadows only, and none under the node itself: a translucent or faded node shows what is under it, not its own shadow, as CSS's `box-shadow` does.
    shadow_color: Color
    -- Drop-shadow blur radius (logical px): the edge ramps over this distance and reaches this far past the shape. 0 = a hard edge.
    shadow_blur: Length
    -- Drop-shadow horizontal offset (logical px).
    shadow_x: Length
    -- Drop-shadow vertical offset (logical px); positive casts downward.
    shadow_y: Length
    -- Grows (or, negative, shrinks) the drop shadow's shape before blurring (logical px).
    shadow_spread: Length
    -- Top-left corner radius (logical px).
    radius_tl: Length
    -- Top-right corner radius (logical px).
    radius_tr: Length
    -- Bottom-right corner radius (logical px).
    radius_br: Length
    -- Bottom-left corner radius (logical px).
    radius_bl: Length
    -- Center children on both axes.
    center: boolean
    -- Hover-track without a click payload (for isHovered-driven styling).
    hoverable: boolean
    -- Ask for another frame after this one, every frame this node is declared. What a `fragment` that reads `time` needs, and what anything driving itself off the clock rather than off input needs. Opt-in like `exit`, and for the same reason: it takes the loop off input-driven and onto the display's cadence for as long as it is declared, so a still node must not carry it. One node asking is enough for the whole window.
    animate: boolean
    -- Paint this node's background in the OS accent colour — `env.system.accent` — keeping the declared `bg` on a host that cannot tell what it is. The one prop whose paint depends on the environment, which is why it is opt-in: the same tree is a different colour on two machines, and that is the point here and a surprise anywhere else. On the stock button it does the whole job — the hover and pressed shades are derived from the accent, and the label goes black or white by its luminance, so a yellow accent is still readable — which is what `<button accent>` is for.
    accent: boolean
    -- Makes this node a selection scope: the text of every node inside it is one selectable run, in tree order, and a press-drag across them selects the lot — as do Shift with Left / Right / Home / End on a focused node inside it, a character or a word at a time, a scope with nothing selected anchoring at its start (`docs/adr/0017-selection-as-a-scope.md`). Declared on the container and not on each label, because what a reader selects is a paragraph or a card rather than one run of it — three labels in a column under one `selectable` select as three lines of one text. The selection is the window's: starting one anywhere clears the last, an editor's included. Scopes do not nest; an outer one around an inner one is warned about (`nested-selection-scope`) and the innermost owns the text. Text scrolled out of view inside the scope is still part of it — selection and copy reach it, hit-testing does not. On a `cells` grid the scope selects in cells rather than in bytes: a drag takes lines (with a modifier, a rectangle), a double click the word under the pointer and a triple click the whole row, its ends are absolute lines so a scroll does not move them, and a copy trims each line's trailing blanks.
    selectable: boolean
    -- Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already.
    focusable: boolean
    -- A press on this node, or anywhere inside it, leaves keyboard focus where it was: a toolbar button, a tab or a divider that acts without taking the keyboard from the editor or key sink that had it. Without it a press on an `onClick` node focuses the node, and the app's keys stop reaching the sink until it takes focus back. The press also leaves a text or cell selection and the Tab ring where they were, so a Copy button copies what was selected. An `<edit>` inside still takes its caret and focus, as the keyboard's own owner. The click, drag and hover are unchanged, and Tab and assistive technology still reach the node.
    keep_focus: boolean
    -- Makes this node's subtree a focus region: a Tab ring of its own that the ring outside never enters and that never leaves — a devtools dock, an inspector beside the app (`docs/adr/0022-focus-regions.md`). Entered on purpose: `focusRegion(name)` (`Ui::focus_region`, `env.focus_region`, `kui_focus_region`) moves focus in — to the focus the region last held, else its `initialFocus`, else its first stop — and `focusRegion(null)` moves it back to the main ring the same way; a press inside the region, or an explicit focus on a node in it, enters it too. Tab then walks that ring alone, wrapping inside it; with nothing focused, Tab enters the ring of the region in effect (`region()`). A region that stops being declared hands focus back to what the main ring last held. Only the ring is scoped: keys still bubble through the boundary to the sink above (a region that wants its own keymap is an `onKey` sink), the pointer and assistive technology see a plain node, and a `modal` in effect is the ring wherever it sits. Nested regions are skipped by the outer ring the way the main ring skips them.
    focus_region: boolean
    -- When a scrolling node draws its bars: `visible` (the default — the stock overlay thumb, drawn while the content overflows), `hidden` (no thumb, no track to press; the wheel, the keyboard, `reveal` and the caret still scroll it — for a list that draws its own indicator, or a pane whose bar would sit on a border), or `auto` (shown while the scroll state is changing — the offset or the content's extent moved, the pointer is on the track, a thumb is dragged — and for a second after, then faded out over a quarter of one; a node first seen shows it the same second; what an overlay bar does on macOS). `auto` needs the driver's clock and is `visible` without one. The bars are overlays and take no layout space in any mode. From the last change until it has faded — a second and a quarter — an `auto` bar asks for frames the way a transition of that length would (nothing else could wake the core when the hold ends); while the pointer holds it, it asks for none.
    scrollbar: Scrollbar
    -- Scroll anchoring on a scrolling node (backlog C26, CSS's `overflow-anchor`): the first child in view keeps its place on screen when the content before it changes size — a chat that prepends history, a log that inserts rows above the viewport, a list whose row heights are corrected as they are measured — with no `setScroll` and no arithmetic in the view. The core remembers which child was first in view and where its edge was, and moves the offset by however far that edge moved in the next layout, before the offset is clamped; a wheel notch or a `setScroll` between the frames is kept and the correction added to it. The child is found by key, so give the rows stable keys (a `key` or an `index`); a child that is gone anchors nothing that frame. On the scroll axis that is the node's main axis only — `scrollY` on a column, `scrollX` on a row — and content appended *after* the anchor moves nothing — unless the last layout left the node at its end (with somewhere to scroll) and nothing has scrolled it since: then it is kept at its end in the same frame, whatever grew or shrank, its own box included, so a tailing log stays tailing without asking each frame. A wheel notch, a drag or a `setScroll` away from the end stops that until the node is at its end again.
    anchor: boolean
    -- The thumb's width at rest, logical px (default 4); under the pointer or dragged it is 2 px wider. The grabbable track grows to fit a wide thumb.
    scrollbar_width: Length
    -- The thumb at rest; the default is the theme's `scrollbar` role, a translucent wash over whatever it sits on.
    scrollbar_color: Color
    -- The thumb under the pointer or while dragged; the default is the theme's `scrollbar_active` role.
    scrollbar_active_color: Color
    -- What a scroll gesture that starts over this scroller does when it is already at its limit that way (backlog F107, CSS's `overscroll-behavior`): `auto` (the default) passes the gesture on to the scroller around it, `contain` keeps it here, moving nothing until it turns back. A gesture picks its target once, when it starts — the innermost scroller under the pointer that can still move the way it goes — and keeps it until it ends, wherever the pointer or the content has gone; one that reaches a limit midway stops there, whatever this says. Only on the axes the node scrolls: a `scrollY` list that contains still passes a sideways swipe to the strip around it. For a panel or a popup's list whose scrolling must never move what is behind it.
    overscroll: Overscroll
    -- Inert: no click, drag or key sink, no hover / pressed / focus background, skipped by Tab, reported disabled to assistive technology; hover tracking stays so a `tooltip` can say why.
    disabled: boolean
    -- Background while the pointer is over the node or anything inside it, a clickable child included (or while any node in its hoverGroup is); implies hover tracking, eases with `transition`.
    hover_bg: Color
    -- A gradient painted over the node's `bg` and under its border and its children (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`): `{ to: 'bottom', stops: [...] }` towards a side or a corner (`right`, `bottom left`, …; the default is `bottom`), `{ angle: 0.125, stops }` in turns clockwise from east, or `{ radial: true, at: [0.5, 0], stops }` out from a centre (fractions of the box, the middle by default) to its farthest corner. A stop is a colour — a `$token` too — or `[colour, position]` with the position 0 to 1; stops without one are spaced evenly between those with. Two stops at one position are a hard edge. The gradient is defined on the box's unit square and stretched to it, so a side or a corner is CSS's and any other `angle` runs corner to corner at an eighth of a turn whatever the box's aspect, where CSS's pixel-measured `45deg` does not. Stops mix in straight sRGB with the alpha premultiplied, as CSS's do. What it costs is one image quad: the core rasterizes each distinct gradient once into the glyph atlas — a 256-texel strip along an axis, a 128-texel square otherwise, within half an 8-bit level of the gradient computed per pixel for a linear one and 1.2 for a radial — keyed by the gradient and not the box, so a box that resizes and a thousand boxes that share one rasterize nothing, and a host that draws an image draws it; a gradient box costs about 55 ns over a flat one, so ten thousand of them are half a millisecond. A hard edge is as soft as the raster stretched to the box (a 256th of its length along a strip); stripes are boxes. It does not tween — `transition` eases the `bg` under it and `opacity` fades it — and `hoverBg` and the other state backgrounds replace `bg`, not the gradient; one that changes every frame is a raster a frame, and a shimmer is a `fragment`'s. Ignored on a `line`, a `polygon` and a `path`. Fewer than two stops are an error in JSX and Lua, as a malformed `keyframes` is, and draw nothing in Rust and C. A stop whose `$token` misses is not an error: it is raised as `unknown-token` and left out, as a miss leaves any slot unset, and the rest are spaced as if it had not been declared — so a gradient left with fewer than two stops, a two-stop one with a typo, draws nothing over its `bg` (backlog RG118).
    gradient: {any:any}
    -- Blur what was drawn beneath the node, inside its rounded box, by this radius in logical px — CSS's `backdrop-filter: blur()`, the radius its standard deviation (backlog F129). What blurs is everything painted before the node: its ancestors' backgrounds, the siblings under it, content scrolling beneath it, the window's `backdrop` where the window has one. The node's own `bg`, border and children paint over the blur, so a translucent `bg` (`#ffffff40`) makes frosted glass and an opaque one hides it. Clipped as the node is, faded by its `opacity`; 0 is none. The GPU renderer reads back only the box (and a margin of three radii around it) and blurs it at reduced resolution, so it costs a copy and three small passes per blurred node on a frame that has one and nothing on a frame that does not. A renderer that cannot read back what it drew — a host's own, or anything older — leaves the node over an unblurred backdrop; the display list carries it as a `backdrop` quad (`KUI_QUAD_BACKDROP` in C) either way.
    backdrop_blur: Length
    -- Turns this node and everything under it, in turns clockwise (0.25 is a quarter turn right), about its pivot — the centre unless `pivotX` / `pivotY` say — after layout (`docs/adr/0043-a-node-turns-about-its-pivot.md`). Paint-only: the node takes the room its upright self takes, nothing around it moves, `onLayout` reports the layout rect. Everything the subtree draws turns with it — backgrounds, borders, shadows, text, images, strokes, fragments — and so does what it clips: a child cut by a turned card's rounded corners stays inside them. Hit where drawn: a tilted card is grabbed on its tilted edge and a press in its box past its edge falls through; drag payloads stay in viewport px. The access rect is the bounding box. Nests by composition. Tweens with `transition` as one slot with `scale`, and an entrance, an exit or a keyframe stop may name it (`enter: { rotate: -0.02 }`, `keyframes: [{ rotate: 0 }, { rotate: 1 }]` spins a box). A float anchored to the parent turns with it; a viewport float does not. On a `path` this is the path's own turn (ADR 0041), which does not tween — wrap it in a box for one that does. Text under a turn leaves the pixel grid, as a turned mask does. `backdropBlur` under a turn blurs the upright box.
    rotate: Length
    -- Scales this node and everything under it by this factor, uniformly, about its pivot, after layout (`docs/adr/0043-a-node-turns-about-its-pivot.md`); 1 is none. 0 draws nothing in Rust, Node and Lua; in C and Odin, where a zeroed field is unset, 0 is 1. Paint-only, as `rotate` is: layout, the room taken and `onLayout` are the upright node's; what it draws, clips and hits scales. Tweens with `transition` as one slot with `rotate`; `enter: { scale: 0.8 }` settles a chip in, `keyframes: [{ scale: 1.05, at: 0.5 }]` pulses. The edge ramps scale with the box, so a box scaled far up reads soft.
    scale: Length
    -- Where across the box `rotate` and `scale` are about, as a fraction of its width: 0 the left edge, 0.5 (the default) the middle, 1 the right edge; outside 0..1 is a point past the box. C: `pivot_x` with `pivot_set`.
    pivot_x: Length
    -- Where down the box `rotate` and `scale` are about, as a fraction of its height: 0 the top, 0.5 (the default) the middle, 1 the bottom. C: `pivot_y` with `pivot_set`.
    pivot_y: Length
    -- On a table (`dir="table"`, ADR 0033): grid lines of this colour between its columns and between its rows (backlog DX21) — down the middle of each gap between the columns of its widest row, from the first row's top to the last row's bottom, and across the middle of each gap between rows, the content box wide. Drawn with the table's box, under its cells and on whole pixels, so give the table and its rows a `gap` at least `ruleWidth` for the lines to show between cells; the outer edge is the table's `border`. Ignored on anything but a table.
    rules: Color
    -- The width of a table's `rules` in logical px; 1 when unset.
    rule_width: Length
    -- Background while files dragged in from the OS are over this node (ADR 0031); wins over pressedBg, focusBg and hoverBg, clears when they leave, land or the drag is cancelled. Implies hover tracking, eases with `transition`.
    drop_bg: Color
    -- Background while pressed (or while its hoverGroup is); implies hover tracking.
    pressed_bg: Color
    -- Background while the node holds keyboard-visible focus (moved there by Tab or assistive technology, not a click); replaces the default focus ring. Pressed wins over focus wins over hover; eases with `transition`.
    focus_bg: Color
    -- The colour of the ring the core draws around a keyboard-visibly focused node — a control, a `focusable` box, an `onKey` sink — for this node and everything under it that declares none (backlog C93): the nearest declaration on the focused node or above it wins, else the theme's `focusRing`. A colour with no alpha draws no ring, turning it off without touching the background as `focusBg` would (in C, whose 0 is unset, any colour with no alpha but 0: `0xffffff00`). What an add-on that paints in its own palette sets once at the root of what it draws, `focusRing="$kitFocus"`.
    focus_ring: Color
    -- Props by condition (ADR 0069): a list of entries, each the conditions it tests and the props it sets while every one holds — `{ width: { below: 700, band: 16 }, dark: true, gap: 8 }`. Every entry that matches applies, in list order, over the node's own props, so a later entry wins a prop an earlier one set; `hoverBg`, `pressedBg` and `focusBg` are entries ahead of the list. The conditions: the node's own `width` / `height` (`{ below, above, band }`, each a length or a `$length` token; `below` holds under it, `above` at or over it, and a range that holds keeps holding `band` past its threshold, so a size resting on one does not flicker); `window: { width, height }`, the same against the window's content size; `dark`, `light`, `reducedMotion` (`reduced_motion`), `contrast: "more"`, `platform` (`"macos"`, `"windows"`, `"linux"` or a list), `scale: { above }`; and the node's state, `hovered`, `pressed`, `focused`, `focusVisible`, `disabled`, `selected`, each true or false — hovered as `hoverBg` reads it, over the node or anything inside it or its hover group. The core resolves them, so a view declares a responsive, themed, interactive box once and is not re-run for a resize, a theme switch or a hover: window, environment and state entries as the node opens, before its `transition` eases what they set (layout props jump unless `slide`; under reduced motion everything jumps); own-size entries inside layout, a width entry once the node's width is final and before its children are laid across it, a height entry with one more layout pass on the frame its match changes — no frame is drawn with a stale match. An entry over the node's own size sets the layout of its children only — `dir`, `gap`, `crossGap`, padding, `wrapChildren`, `mainAlign`, `crossAlign`, `minColumn`, and `hidden` as the list of the keyed children it hides (`hidden: ["chooser"]`) — and makes the node a container in that axis: a `fit` size there is refused (`query-on-fit`) and the node takes the room its parent offers. Other entries set any box prop but handlers, access props, keys and children. What an entry may not set is left out with `rule-prop-refused`, the rest of it standing. `onQuery` tells the view when its own-size and window matches change.
    when: {any:any}
    -- On a row: wraps it into equal columns, as many as fit at this width or more — `max(1, floor((w + gap) / (n + gap)))` — every child as wide as a column, the last line's children too, which do not stretch (ADR 0069, decision 17). CSS's `repeat(auto-fill, minmax(n, 1fr))`: a gallery's cards, a settings grid. The lines are `crossGap` apart. An own-size `when` entry may set it, and on a column it does nothing.
    min_column: Length
    -- Not laid out and not drawn (ADR 0069): the node and everything in it take no room in its parent — no gap either — paint nothing, take no input and are out of the access tree and the Tab ring, while their keys, focus history and retained state (scroll offsets, editor text) are kept as any declared node's are. What a `when` entry sets to drop a node at a setting or a window size; an entry over the node's own size names the children it hides instead (`hidden: ["key"]`).
    hidden: boolean
    -- Query handler (ADR 0069): when the set of this node's `when` entries over its own size and the window's that match changes — and after the frame that first lays it out — this message arrives with `query: {matched, width, height}` in it, `matched` the 0-based indices of every matching entry. On a lazy branch's node, its decision: `matched` is `[0]` while `render` is built. State and environment changes do not raise it. Nothing needs it to lay out; it is for a view that keeps the fact in its model.
    on_query: any
    -- Modal surface: the Tab ring becomes this node's subtree, everything outside it is inert to the pointer, the wheel and assistive technology, and Escape or a press outside arrives on it as this message with `dismiss: {reason: "escape" | "outside"}` in it (ADR 0047) — the app stops declaring the node. The last one declared in tree order is the one in effect (a confirm inside a dialog); a modal that must cover the app is a float. The access tree is not pruned to the modal: it keeps every node of the frame and marks the one in effect `modal` (`docs/adr/0003-modal-surfaces.md`, decision 7), which is what assistive technology acts on.
    modal: any
    -- Where focus lands when the enclosing `modal` scope is entered: the first node in the modal's Tab ring declaring it, so a destructive confirm opens on its Cancel rather than on whichever control is declared first. Read on entry only — a Tab press afterwards stands, and the scope re-entered (a nested confirm closing) leaves focus where it was. Declared on nothing, or only on nodes the ring skips (disabled, `role="none"`, not focusable), entry stays the ring's first node.
    initial_focus: boolean
    -- Nodes sharing a group name show hoverBg/pressedBg together (a split button, a multi-piece shape).
    hover_group: string
    -- Draw this node only while its anchor — the nearest ancestor that tracks the pointer: an `onClick` row, a `hoverBg` card, a `hoverable` group header — is hovered (the pointer over it or anything inside it), holds a press begun inside it, or holds keyboard-visible focus inside it; and while focus of any kind is inside the node itself (ADR 0051). A row's secondary actions, shown for the row under the pointer with no `onHover` and no redraw of the view: the core decides it each frame as it decides `hoverBg`. Hidden is `opacity` 0, eased by a `transition`: the node keeps its room and its hit regions, and stays in the access tree and the Tab ring, so a control in it is there for the pointer that reaches it, the keyboard that tabs to it and a screen reader, and shows for each. With no anchor above it, it is always drawn and `reveal-without-anchor` says so.
    reveal_on_hover: boolean
    -- Hover handler: the pointer entering or leaving arrives as this message with `hover: {phase: "enter" | "leave", by}` in it (ADR 0047).
    on_hover: any
    -- Drop-zone handler (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`): files dragged in from the OS over this node arrive as this message with `drop: {phase: "enter" | "move" | "leave" | "drop", paths, x, y}` in it (ADR 0047) — `paths` the OS paths as strings, `x`/`y` the pointer in logical viewport coordinates (absent on `leave`). The zone under the files is the topmost zone by paint order: a node inside a zone is the zone's (a button in it, a field in it), and a node that is no zone and has none enclosing it is looked past, so an overlay shown on `enter` cannot make the zone lose the files. No `leave` follows a `drop`; a drop off every zone is refused by the driver. Implies hover tracking. No access row — a screen-reader user's way in is a button beside the zone. On Windows and Linux the position is the OS cursor at enter and release only, so `move` never fires there.
    on_drop: any
    -- Layout handler: the node's laid-out rect arrives as this message with `layout: {x, y, w, h, parent, scale}` in it (ADR 0047) on its first frame and whenever it changes (needs a stable key).
    on_layout: any
    -- Range handler (ADR 0065): on a node whose children carry data indices — `index`, a virtual list's rows — the least and greatest index among its children inside its box arrive as this message with `range: {first, last}` in it, after the frame that first lays it out and whenever either changes; what a table pages an unbounded source by. Needs a stable key.
    on_range: any
    -- On a windowed `list` or `table` composite (ADR 0070): the data row (a whole number, its `index`) Tab enters it on — the selected one after a reload — scrolled to and landed on when the frame did not build it. Wins over the row the composite remembers focus last held there; without either, ADR 0007's entry (`initialFocus`, `selected`, the first row). A value that is not a whole number is no row.
    active_index: any
    -- Type-ahead handler, on a `list` or `table` composite (ADR 0070): a search its built rows did not answer arrives as this message with `typeahead: {text}` in it, the whole search so far. A windowed list's unbuilt rows have no names the core can see; the app searches its data and answers with `answerTypeahead(key, index)`, and the core reaches for that row as a step does.
    on_typeahead: any
    -- Message emitted when clicked (data, not a callback).
    on_click: any
    -- Drag handler: the drag arrives as this message with `drag: {phase, x, y, dx, dy, parent}` in it (ADR 0047), `dx`/`dy` measured from the press point in every phase. On a `cells` grid the events also carry `cell: {row, col}`; inside an `onKey` sink that draws `role="line"` rows they carry `line`, `byte` and `clicks` — see the events table. A press on a child that does nothing with a press itself drags the nearest enclosing `onDrag` node, stopping at a clickable, a key sink, window chrome, a slider, the modal boundary and the selection scope the press is in (backlog F176).
    on_drag: any
    -- Key sink: with key focus held, presses arrive as this message with `key: {phase: "down", code, …}` in it, and a composition, a commit and a reader's selection as `preedit`, `text` and `select` the same way (ADR 0047). Releases only with `keyUp` beside it.
    on_key: any
    -- With `onKey`: releases arrive too, as the same payload with phase:"up" (`text` null, `repeat` false) — for a held-key interaction (WASD, press-and-hold, a key that arms a mode while it is down). A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so nothing is left stuck down. Without it a sink hears presses only, which is what a keymap wants — one that heard both halves would run every binding twice.
    key_up: boolean
    -- With `onKey`: the modifier and lock keys arrive as keys of their own (backlog F108) — `code` "shift", "ctrl", "alt", "super", "capslock", "numlock", "scrolllock", which side in `location` ("left" / "right"), releases too with `keyUp`. Without it a modifier is only ever held — the next key's `shift`, `ctrl`, … and the `modifiers` event — so a keymap mid-sequence never reads a Shift as a key between two others. For a terminal speaking kitty's keyboard protocol, or a game that binds a lone Shift.
    modifier_keys: boolean
    -- Context-menu handler: a secondary-button (right) press arrives on the node as this message with `contextmenu: {x, y}` in it (ADR 0047), at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it. Asked of the topmost node under the pointer, and when that node offers no menu the press reaches the nearest enclosing node that does — a container declaring a menu for everything inside it is the common case — the way an unclaimed key reaches the enclosing sink (`docs/adr/0011`): the event carries the *owner's* key and message, a nested declaration wins over its ancestor's, a disabled node's own is skipped, and the walk stops at the modal boundary.
    on_context_menu: any
    -- Force-click handler: a press that deepens past the second stage of a Force Touch trackpad arrives on the node as this message with `forceclick: {x, y}` in it (ADR 0047), at the logical viewport point it happened at (`docs/adr/0017-selection-as-a-scope.md`). Routed as a secondary press is — no focus moved, no caret placed, no click — but asked of the topmost node only, with no walk to an enclosing declaration — and the ordinary click the press is still producing arrives afterwards, as it does on macOS. Text needs none of this: a force click over an `edit` or a `selectable` scope selects the word under it and asks the host for its Look Up panel. macOS-only in practice, and there the user can switch the gesture off, so nothing may declare itself the only way to reach something.
    on_force_click: any
    -- Button handler (backlog F105): a press of a non-primary button — middle, secondary, or one past those — arrives on the node as this message with `button: {phase: "press", button, x, y, clicks}` in it (ADR 0047), and the button is then captured by it: every pointer move while it is held arrives as phase:"move" and its release as phase:"release", on this node wherever the pointer is. `button` is `"secondary"`, `"middle"` or a further button's number (3 and up); `x`/`y` are logical viewport coordinates, and on a `cells` grid each event carries `cell: {row, col}` as a click does. Several buttons can be held at once, each its own capture, and a primary drag is untouched. `buttons` says which buttons it claims — all of them unless it narrows them. Asked of the topmost node under the pointer, and when that node claims no such button the press reaches the nearest enclosing node that does, the way a context menu's does: a disabled node's own is skipped and the walk stops at the modal boundary. A claimed secondary press is this event *instead of* a `contextmenu` event and the stock menu (a nearer `onContextMenu` still wins, being the nested declaration). Like every non-primary press it moves no focus, places no caret and touches no selection or scrollbar. For a terminal's middle-click paste, and the mouse reports a program in it asked for.
    on_button: any
    -- Which non-primary buttons `onButton` claims (backlog F105): `"secondary"`, `"middle"` and `"other"` (every button past those), separated by spaces or commas — `"middle"`, `"secondary middle"`. Unset, all three: a node that wants the middle button and leaves the secondary one to its context menu says `"middle"`. A word that is none of the three is skipped, so a string of none of them claims nothing, and a typo never takes the secondary button from a context menu. Meaningless without `onButton`.
    buttons: string
    -- Scroll handler: the wheel over this node arrives on it as this message with `scroll: {x, y, dx, dy, lines}` in it (ADR 0047) instead of scrolling anything — `dx`/`dy` the delta in logical px as the driver reported it (positive `dy` is the wheel rolling up, toward earlier content), `x`/`y` the pointer, and `lines` on a `cells` grid the whole lines the delta covers (positive = later history, the sign `originLine` grows in; the fraction is carried to the next notch so a trackpad's small steps add up) and null on any other node. The node takes the wheel on the axes `scrollAxes` names (both unless it narrows them): a gesture that starts over it is its own whether or not it has anywhere to go — except on an axis the node also scrolls (`scrollX`/`scrollY`, its offset the app's to set), where it is answered by its room as a container is, so at its edge a gesture that way passes to the scroller around it (backlog F118) — and stays its own until it ends, wherever the pointer goes (backlog F107); it reaches no scroll container above it, and a scroller inside it still takes the axes it scrolls while it can move that way, passing this node the rest — the other axis, and a gesture that begins with that scroller at its limit (`overscroll: "contain"` on the scroller keeps it there). The core moves nothing — a grid re-declares `originLine`, a canvas zooms. A drag-select held past a `cells` grid's top or bottom edge arrives here too, once a frame with the lines that frame scrolled by (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).
    on_scroll: any
    -- Which axes `onScroll` takes (backlog F107): `both` (the default), `x` or `y`. A scroll gesture on an axis the node does not take passes it by, to the scroller around it, and hears nothing here: a terminal that scrolls its history says `y`, and a sideways swipe that starts over it moves the strip it sits in. (A swipe that started elsewhere is not the node's either way: a gesture keeps the target it started with.) Meaningless without `onScroll`.
    scroll_axes: ScrollAxes
    -- The modifiers `onScroll` is for (backlog F122): `"shift"`, `"ctrl"`, `"alt"` and `"super"` (⌘, the Windows key), separated by spaces or commas — `"ctrl super"`. With any named, the node hears only a scroll gesture that began with one of them held, and hears it first: ahead of every scroll container and every `onScroll` that names none, wherever under the pointer the gesture began, the innermost such node winning — so a Ctrl-wheel zoom declared on the window's root is heard over a list, and the list does not scroll. A wheel with none of them held passes the node by, as if it had no `onScroll`: a node that scrolls as well (`overflow`) scrolls for it as any container does. Its `scroll` events carry `mods`, the modifiers held when the gesture began; the gesture stays the node's to the end of its glide, whatever is let go meanwhile, and one begun without them never becomes its. A word that is none of the four is skipped. Unset, a handler like any other. Meaningless without `onScroll`.
    scroll_mods: string
    -- Window-chrome role: interactions become window commands, not events.
    window: Window
    -- The pointer shape over this node. Unset, the pointer is `text` over an editor or a `selectable` scope and `default` over everything else — an `onClick`, `focusable` or `onDrag` node included, as a native button is — so a hand (`pointer`) over a control, a `grab` over a handle (and `grabbing` while its drag runs, which the view declares as its drag state changes), a splitter's `ewResize` / `nsResize` and a `disabled` control's `notAllowed` are all declared. The stock `button` declares `pointer` itself. A captured drag keeps the dragged node's shape wherever the pointer goes.
    cursor: Cursor
    -- Animate sizing/colors/radius changes over this many ms — and, on a scroll container, the offset a reveal or a set_scroll moves it to (needs a stable key).
    transition: Length
    -- Easing for `transition` (default easeOut). The springs — `smooth` (no overshoot), `snappy`, `spring` and `bouncy` (the most), each a `bounce` of its own — integrate with momentum, so a value retargeted mid-flight keeps moving the way it was; `transition` is then about how long one takes to get there. Between `keyframes` stops a spring is drawn as `easeOut` (see that row).
    easing: Easing
    -- How far a spring overshoots its target: 0 glides in with none, 0.5 bounces visibly, and values past 0.9 are held there (a spring at 1 would never settle). It replaces a spring `easing`'s own bounce (`smooth` 0, `snappy` 0.15, `spring` 0.25, `bouncy` 0.5), and on a timed easing makes the transition a spring — so `transition` plus `bounce` is a spring of that length and bounce. It does nothing between `keyframes` stops, which are sampled off the clock (see that row).
    bounce: Length
    -- With transition: also ease the node's position (reordered siblings slide). What eases is the node's place within its parent — the node it is laid against: its parent, a float's anchor, the window for a viewport float — so a move its ancestors make is followed on the frame it happens and only its own move within the parent animates (backlog C97): a switch's knob keeps to its track while the window resizes, a sliding child rides its sliding parent and adds only its own move, and a scroll moves it with the content. Put `slide` on the node that changes place within its parent: on a wrapper that holds it, the wrapper slides and it rides along. A keyed node that changes parents — a card taken to another column — carries on from where it was drawn into its place in the new one. While it eases, the node is drawn between where it was and where this frame put it — not at the declared `dx`/`dy`, or its slot in the row — so anything else positioned from those numbers drifts for the transition's length: a canvas of floats eases everything or nothing. Siblings that cross on their way paint in tree order, the one in the later place on top; `overSiblings` keeps one over the others.
    slide: boolean
    -- Paint this node and its subtree after its siblings, over any it overlaps: a tab that glides past its neighbour under `slide` passes over it whichever way it moved, so the selected tab declared `overSiblings` stays on top without the view tracking when it moves. Siblings otherwise paint in tree order, a later place over an earlier. Only the sibling order changes: the parent still paints under it, what follows the parent over it, it keeps the parent's clip, and a float still opens above it. The hit test reads the same order, so where it overlaps a sibling it takes the press. Two siblings that both declare it paint in tree order between themselves. Not a z-index: there is no number to outbid (`docs/adr/0046-a-node-over-its-siblings-paints-after-them.md`).
    over_siblings: boolean
    -- Paint the floats under this node in tree order, the last declared on top, worked out again every frame — the order its in-flow children paint in — instead of the order they opened: a drawing canvas whose items are floats stacks them as its file does, so an item that pans into view or is added takes its place in the order rather than going over the rest, and moving an item in the tree moves it in the stack on the next frame. Every float at any depth under the node takes part, and one nested in another is still over the float it is in. As a group they keep the place the stack gives them among the floats outside the node: a menu opened after them stays over all of them, a new item included. A menu declared under the node is one of them and paints by its place; declare it outside the node to have it open over everything. The hover hint the core floats for a `tooltip` is the exception, over them all. The hit test reads the same order. Not a z-index: the order is the tree's (`docs/adr/0053-floats-under-a-node-paint-in-tree-order.md`).
    floats_in_order: boolean
    -- CSS-style stops `[{ at?, dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }, …]`: the slots they name cycle through them over `transition` ms, for ever unless `iterations` says how many times, without the view redrawing; `at` is 0..1 and spreads evenly when omitted. `dx` / `dy` are logical px from where layout put the node (backlog F132), as an entrance's are: the node and its subtree are drawn and hit that far away at the stop, a lane a stop leaves out is 0, and the offset adds to a `slide`'s, so `[{ dy: 0 }, { dy: -6 }]` with `repeat: 'alternate'` bobs a box and a sparkle drifts up its stops. Paint, hit and access only: layout and the room the node takes are its own place's, and an `onLayout` node reports its layout rect, not the cycle, which would post an event every frame it runs. The `easing` applies to each step between two stops, as CSS applies its timing function per keyframe; a spring easing there is drawn as `easeOut` and `bounce` does nothing, since a cycle is sampled off the clock and a spring has to be integrated (backlog F141) — an overshoot in a cycle is a stop past the target, `[{ scale: 1 }, { scale: 1.15, at: 0.6 }, { scale: 1 }]`.
    keyframes: {any:any}
    -- Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in, `scale: 0.8` settles it in).
    enter: {any:any}
    -- Where the node ends the frame after the view stops declaring it `{ dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }` — an `enter` read the other way. It plays when the node itself is removed, its parent still declared; a node that goes because an ancestor went — a tab switched away, a panel closed around it — goes at once with it, unless that ancestor has an `exit` of its own, whose picture carries it (backlog DX19; React's `AnimatePresence` rule). With a `transition`, the departing subtree is copied out of the last frame that had it and replayed frozen, in its place (the pass it painted in, just under the node that painted after it — a panel under a HUD leaves under it) and inert (no clicks, no Tab stop, no access row) while those slots ease from where they were, then dropped; without one it vanishes at once as it always did. `width`/`height` resize the departing node's own box only — the subtree inside it is a picture and is not laid out again. The exit read is the one the last frame that had the node declared, unless `exit_with` named another for the frame it went in: a card a button throws left or right is aimed by the handler that removes it, with no frame drawn first to point it. Needs a stable key across frames.
    exit: {any:any}
    -- How `keyframes` cycle (CSS `animation-direction`, default normal). Lua: `direction`, since `repeat` is a keyword.
    direction: Direction
    -- Holds the `keyframes` cycle back by this many ms (CSS `animation-delay`); siblings with different delays run out of phase.
    delay: Length
    -- How many times the `keyframes` cycle runs (CSS `animation-iteration-count`, backlog F133); left out, for ever. A finite cycle plays from the first frame the node is declared with it — a node that leaves and comes back plays again — holds its first stop through its `delay`, and rests where its last iteration ended (CSS's fill `both`): `1` plays a burst or a shake once, `2` with `repeat: "alternate"` goes out and back and ends where it began, `0.5` stops halfway. Once it is over the node owes no frame, so `animating()` and `owed()` go quiet as a settled transition's do; `delay` plus `iterations: 1` staggers one-shots. A count that is not a positive number is for ever. C: 0 is for ever.
    iterations: Length
    -- A registered sound (addSound) played when the node is clicked; implies hover tracking.
    click_sound: integer
    -- A registered sound (addSound) played when the pointer enters the node; implies hover tracking.
    hover_sound: integer
    -- Line height (logical px); default size * 1.35.
    line_height: Length
    -- Text color; default foreground when omitted.
    color: Color
    -- Font family: `sans`, `serif` or `mono`, kui's own, or the name of an installed family or one loaded with `loadFontsDir` / `loadFontFile` — `"Berkeley Mono"` — drawn in its face in the frame that names it (ADR 0037). A name is matched as `addSystemFont` matches it and registered in the session on first sight, exactly as the font database spells it (`"menlo"` is not `"Menlo"`); the session's first registration of any font maps the installed font files once (~30 ms on a Mac, backlog DX24), which a family named in a view pays in that frame. `systemFonts()` lists the names there are. A name nothing matches shapes as sans and raises `unknown-family`. A `$name` is a family token (ADR 0049): the face its fallback list picked, the first of its names that resolves. It and `font` set the same thing, so declare one.
    family: string
    -- A registered font handle (addFont / addSystemFont); overrides `family`.
    font: integer
    -- Line breaking at the node's width: between words (default), anywhere, never (one line per paragraph, clipped to the node), or between words with whitespace taking its room (`break-spaces`: a space that does not fit starts the next row rather than hanging past the edge — an editor's wrapped line). On a single-line `edit` — a field, which otherwise takes one line and scrolls it — declaring it is what makes the field fold to its width like a document, by this mode, while Enter still submits (see `edit`).
    wrap: Wrap
    -- Lay out at most this many lines (0 = unlimited); with `ellipsis`, a line clamp.
    max_lines: Length
    -- End the last line with an ellipsis when the text is cut off: a single line unless `maxLines` says otherwise.
    ellipsis: boolean
    -- The first letter as an initial — a drop cap — this many lines tall (backlog F164), as CSS's `initial-letter`: its top level with the first line's capitals, its foot on the last one's baseline, the lines beside it moved in past it and the ones below at full width again. The letter is the text's first, with any opening quote or bracket before it; it keeps its span's face and colour and is sized to the drop. Still one text: a caret, a hit, a selection and the access tree read it where it is drawn, and the text is at least as tall as the lines it drops through. 0 and 1 are none (the default); a text with `maxLines` or `ellipsis`, a right-to-left one and an `edit` have none.
    initial_letter: Length
    -- A line under the text, where the face puts its underline and as thick as it says, in the text colour. Paint only. On a `<span>` it covers the span alone and follows it across a wrap, one rect per line. `underlineColor` gives it a colour of its own and `underlineStyle` a shape; either implies it.
    underline: boolean
    -- The underline's own colour — a diagnostic's red under keyword-coloured text (backlog K4). Implies `underline`. On a `<span>` the span's; a span with no colour of its own takes the text's.
    underline_color: Color
    -- The underline's shape (backlog K4): `solid` (the face's line), `wavy` (three strokes tall around the line, a six-stroke period — a diagnostic's squiggle, a terminal's undercurl) or `dotted` (dots two strokes across, four apart). Implies `underline`. A wave or dots are runs of the segment primitive a `line` draws, so no backend learns a kind; the cost is two quads per period.
    underline_style: UnderlineStyle
    -- A line through the text, where the face puts its strikeout. Paint only; on a `<span>` the span alone, per line.
    strikethrough: boolean
    -- The family's bold, on a whole text or an editor (backlog F150) — a heading, a table's header, a title field: its bold face, or its regular drawn bold where the family has none, as a `<span bold>` is. A span inside a bold text is bold too.
    bold: boolean
    -- OpenType features for the shaper, as `tag=value` pairs separated by spaces or commas — a bare `tag` is 1, `-tag` is 0: `"liga=0 calt=0"` keeps a coding font from joining `->` and `!=` (what a terminal built on runs needs to hold its grid), `"tnum"` lines figures up in a gutter, `"ss01"` picks a stylistic set. Unset, the font's own defaults apply. At most 8; part of what the text is shaped as, so two texts differing only here are shaped twice.
    features: string
    -- What the node is to assistive technology. Unset, the core derives one (an `onClick` node is a button, an editor a text input, a scrolling box a scroll view, a plain box nothing); `none` hides the node and its subtree from the access tree — the decorative door, for what a reader need not hear: an icon beside the text that says the same, or a caption under the button it repeats. A text takes no `role` (it has no box), so `none` goes on the box around it; `ambiguous-name` points at it when a caption and its control share a name (backlog F140). Where the caption is all a control says, a `label` on the control that says what it does is the better answer. A `radio` belongs inside a `radioGroup` and a `tab` inside a `tabList`, labelled with what the choice is: the pair is a composite (`docs/adr/0007-composite-keyboard-patterns.md`) — one Tab stop for the set, the arrows, Home and End moving the choice inside it (each step is the item's click, so the choice follows focus), and a screen reader reading "2 of 3". A `radio` or `tab` with no container above it is a Tab stop of its own that no arrow moves, and the core warns (`item-outside-container`). `menu` holds `menuItem`s and `list` holds `listItem`s the same way.
    role: Role
    -- The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image, an icon-only button and a `modal` dialog have none, and the core warns (`image-without-label`, `control-without-name`, `modal-without-name`). Not the key label a `key` prop or `with_keyed` declares, which `key_of` looks up and a reader never hears; `key_named` looks a node up by this name.
    label: string
    -- The accessible description: the extra sentence a reader says after the name, for what the name cannot say on its own — what a button will do, why a control is disabled, what format a field wants. `tooltip` is the shorthand that also draws the string and hover-tracks the node; this is the description alone, for a hint that is spoken and never drawn. Both write the one slot, so a node declaring both keeps whichever its binding applied last. It reads only on a node that reaches the access tree — a role, a label, a control — since a plain box is elided and takes its description with it.
    description: string
    -- The on state of a `checkbox` / `radio` / `switch` role.
    checked: boolean
    -- A `checkbox` that is neither on nor off — the select-all box over a list some of whose rows are selected (ADR 0034). Read as mixed by assistive technology whatever `checked` says, and drawn as a dash by the stock `<checkbox>`. Meaningful on the checkbox role alone.
    mixed: boolean
    -- The current one of a set: which `tab` a `tabList` shows, which `listItem` a list has picked, which `link` is the page you are on. A `tab` always carries the state — its siblings read as "not selected" — while a list row or a link carries it only where it is set, since an ordinary list or navigation bar is not a selection and a reader saying "not selected" on every row of it is noise.
    selected: boolean
    -- A disclosure's state: what a node that shows and hides something (a twisty, an accordion header, a menu button) reads as. Unset, the node does not expand at all — which is why this names its state instead of being a flag.
    expanded: Expanded
    -- Marks this node a live region: when the text inside it changes, a screen reader reads the change without being asked — `polite` at the next pause, `assertive` interrupting. Put it on the smallest node that holds the message, since everything inside a live node is live. For a one-off with no node behind it ("Saved") the binding's `announce` verb is the other half.
    live: Live
    -- A `slider` role's current value (the drawing stays yours; this is what assistive technology reads).
    value_now: Length
    -- A `slider` role's minimum.
    value_min: Length
    -- A `slider` role's maximum.
    value_max: Length
    -- What a `slider` role's position reads as (ARIA's `aria-valuetext`). Without one a reader has only `valueNow` and the range and says a percentage — 25 in [5..60] is "36 percent" — so a value whose unit carries the meaning says it here: "25 minutes". It replaces the number in the reading rather than joining it, and a nudge announces the new text. Meaningful on the slider role alone, like the three numbers; putting the reading in `label` instead renames the control on every nudge, which is the wrong attribute.
    value_text: string
    -- How far one arrow key moves a `slider` role, and the grid a value the pointer sets snaps to (ADR 0034). Unset, a hundredth of the range. PageUp / PageDown move ten steps. Read by the core only where the slider declares `onChange`; reported to assistive technology either way.
    value_step: Length
    -- Keyboard focus entering or leaving this node's subtree — the node itself, or anything focused inside it — arrives as this message with `focus: {phase: "in" | "out", by}` in it (backlog DX18, ADR 0047). `by` is what moved it: `pointer` (a press), `keyboard` (Tab, a key a control answered), `assistive` (a screen reader's request) or `program` (the view or the app — `keyFocus`, `setFocus`, a modal's entry). Reported once the move settles, after the input that made it or at the end of the frame that declared it, so an app hears a pane taking the keyboard instead of diffing the focused key every frame. Leaving is reported innermost first, entering outermost first. It makes nothing focusable or interactive.
    on_focus: any
    -- Changes, as this message with `change` in it (ADR 0047). On a `slider` role (ADR 0034) the core turns a press on the node into the value under the pointer, a drag into the value under it, the arrows and assistive technology's increment / decrement into one `valueStep`, PageUp / PageDown into ten, Home / End into the range's ends — clamped to `valueMin`..`valueMax` (0..100 unset) and snapped to the step — and proposes it as `change: {value, phase}`: `phase` is `"move"` while the pointer holds the slider and `"end"` when it lets go or a key moved it. The value is proposed and never applied; the slider moves when the view declares it as `valueNow`. A key that lands where the slider already is proposes nothing. The pointer reads the node's content box along its main axis, so a `dir="column"` slider runs bottom to top. Without it a slider does not move and a key or a reader's request raises `slider-without-handler`. On an editor — an `edit`, or a node with an editor role that draws its own text — every change to the text arrives as `change: {text}`, the whole text after it: typing, a paste, a composition's commit, assistive technology setting the value. Ignored on any other role.
    on_change: any
    -- Enter in a single-line editor, as this message — a map as declared, any other message under `tag`, none as `{}` (ADR 0047).
    on_submit: any
    -- Escape letting go of a focused editor, as this message — a map as declared, any other message under `tag`, none as `{}` (ADR 0047): the keyboard leaves it, as it does for a click elsewhere, and this says it was Escape — a draft abandoned, where `onSubmit` is one confirmed and a plain blur is neither. A field that commits on blur cancels here instead.
    on_cancel: any
    -- An editing key that met the edge of an editor's text and did nothing, as this message with `boundary: {key, edge, word, doc, ctrl}` in it (ADR 0047, backlog F145): ↑ on the first line, ← or Backspace at the start (`edge: "start"`), ↓ on the last line, → or Delete at the end (`"end"`) — from a caret with no selection and without Shift. What a block editor joins blocks and moves between fields by. A field has one line, so its ↑ and ↓ always report. `word` and `doc` are the editing modifiers the key carried.
    on_boundary: any
    -- A choice from a menu opened for this node — a `select`'s, or one `openMenu` named it the target of — as this message with `menu: {role, item}` in it (ADR 0047): `item` is the row's `id`, else its label. The stock right-click menu, whose roles the core performs itself, sends nothing.
    on_menu: any
    -- On a `selectable` scope: a copy reached rows a virtual list has not built, as this message with `selectionrange` in it (ADR 0047) — what a list answers by building them.
    on_selection_range: any
    -- On a `line` of a custom editor (a `textInput` / `multilineTextInput` role drawn by the app): the caret's byte offset into that line's text.
    caret: Length
    -- On a `line` of a custom editor: the byte offset where the selection's other end sits (the caret is `caret`, possibly on another line).
    selection_anchor: Length
    -- On a `line` declaring `caret`: the caret is solid — a block caret in a modal editor's normal mode — so the driver's blink clock is not armed on it and `caretVisible` stays true, while the offset still anchors the IME and reads to assistive technology. Without it a declared `caret` is a caret to blink, and the one thing that asks an idle app for a frame twice a second; an editor whose caret only blinks while typing declares this on every other mode's line.
    caret_solid: boolean
    -- Font size in logical px; the text style is constructed from it, so declare it for the other style props to apply at that size.
    size: Length
    -- Padding; a frontend reports the names it saw and `PadShorthand::resolve` turns them into four edges — an edge falls back to its axis, an axis to the all-round `pad`, and the specific one always wins.
    pad: number | string | Pad
    -- Border width and color (drawn inside the rect).
    border: Border
    -- Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. The wheel goes to the scroller under the pointer on the axes it scrolls, and the rest of the notch to the one around it: a `scrollY` list inside a `scrollX` strip moves the strip on a sideways swipe (backlog DX13). A scroll gesture — a swipe and its glide, a wheel spun without a pause — picks that scroller when it starts, skipping one already at its limit that way for the one around it (unless it says `overscroll: contain`), and keeps it until it ends, so content moving under a still pointer does not hand the rest of a swipe to what came under it (backlog F107). An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).
    clip: boolean
    -- Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. The wheel goes to the scroller under the pointer on the axes it scrolls, and the rest of the notch to the one around it: a `scrollY` list inside a `scrollX` strip moves the strip on a sideways swipe (backlog DX13). A scroll gesture — a swipe and its glide, a wheel spun without a pause — picks that scroller when it starts, skipping one already at its limit that way for the one around it (unless it says `overscroll: contain`), and keeps it until it ends, so content moving under a still pointer does not hand the rest of a swipe to what came under it (backlog F107). An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).
    scroll: boolean
    -- Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. The wheel goes to the scroller under the pointer on the axes it scrolls, and the rest of the notch to the one around it: a `scrollY` list inside a `scrollX` strip moves the strip on a sideways swipe (backlog DX13). A scroll gesture — a swipe and its glide, a wheel spun without a pause — picks that scroller when it starts, skipping one already at its limit that way for the one around it (unless it says `overscroll: contain`), and keeps it until it ends, so content moving under a still pointer does not hand the rest of a swipe to what came under it (backlog F107). An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).
    scroll_x: boolean
    -- Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. The wheel goes to the scroller under the pointer on the axes it scrolls, and the rest of the notch to the one around it: a `scrollY` list inside a `scrollX` strip moves the strip on a sideways swipe (backlog DX13). A scroll gesture — a swipe and its glide, a wheel spun without a pause — picks that scroller when it starts, skipping one already at its limit that way for the one around it (unless it says `overscroll: contain`), and keeps it until it ends, so content moving under a still pointer does not hand the rest of a swipe to what came under it (backlog F107). An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).
    scroll_y: boolean
    -- Out-of-flow positioning against the parent or the viewport; `fit` flips/clamps to stay on screen. A float escapes every ancestor's clip — a tooltip is not cut by the scroller it hangs from — unless it declares `clip` and is anchored to its parent (`parent`, `below`, `above`): then the parent's clip holds it as it holds a child, so a node on a `clip` canvas panned past the canvas's edge is cut there and cannot be hit past it; it still paints as a layer over its in-flow siblings. A `line`, `polygon` or `path` in its parent's box is always clipped this way. The four preset names resolve in `FloatConfig::preset`, and `anchor` takes any of them — an override left out keeps the preset's own value, so `{ anchor: "below", dx: 4 }` still hangs below with its 6px gap. A float is a layer of its own: above the in-flow tree and every float that opened before it, under every float that opened after, and hit-tested in the same order — so a tooltip that appears over an open menu is over it, and a popover over a scroller's bar takes the press there. A scroller's bars and the focus ring belong to the layer that owns them. There is no z-index; a float declared under a fresh key reopens on top (`docs/adr/0023-layers-stack-in-the-order-they-open.md`).
    float: FloatWord | {any:any}
    -- Focuses this node (an `onKey` sink, an editor, any focusable node) when it starts being declared: declared every frame it takes focus once, so a later Tab press is not clobbered. Declaring it on the frame a `modal` stops being declared is how a view says where focus lands on the way out — the edge stands, and the focus the modal displaced is not handed back over it (`docs/adr/0003-modal-surfaces.md`, decision 4). To move focus at any time call the binding's focus verb (`ctx.focus`, `kui_focus`, `env.set_focus`).
    key_focus: boolean
    -- Stable identity for retained state (scroll offsets, editors, transitions; keys are hashes of the path from the root). Retained state outlives the key's absence, under a budget on the states nobody declares (see `<edit>` and the overflow props).
    key: string
    -- Stable identity by *data* index rather than by name: the key auto-keying would have given this node as the `i`th child, given to it wherever it actually sits. What a virtualised list is for — a view that builds rows 900..930 of ten thousand opens each with its own row number, so the row keeps its hover, focus, edit buffer and tweens as the built range slides over it, and a list that builds every row agrees with one that builds a screenful. Wherever `key` names a node this numbers it (a box, a `line`, a `cells`, a `fragment`); declared beside `key` the index wins. Indices and names are separate namespaces, so a spacer keyed `"lead"` cannot collide with row 0 — but two rows on one index do, exactly as two on one name would.
    index: number
    -- How many `index`ed rows this node's virtual list has, built or not. `uniformList` / `uniform_list` / `widgets::uniform_list` and `widgets::list` declare it on their container; a list composed by hand says it beside `scrollY`. What it buys: Select All (Cmd/Ctrl-A, the menu's row) inside a `selectable` virtual list selects the *data*, rows `0..rowCount`, rather than the rows the frame built, and the copy is a `selectionrange` ask whose `to.byte` is past the last row's length when that row is not built — cut it to the row. Without it Select All is the built rows, which is all the core can see.
    row_count: number
    -- Declares the window title for this frame; the driver diffs and applies.
    window_title: string
    -- Declares that this frame wants the window kept above every other app's — a floating palette, a picture-in-picture player, a timer (backlog C30). Frame state the way `title` is, applied by the driver on change and free on the frames it does not change, but with a default of false rather than "leave as-is": a frame that stops declaring it lowers the window again, so a pin button is a toggle on the app's own state and nothing has to remember to undo it. Whether the platform has a level to set is `env.window.always_on_top`, which is what the pin button should draw its state from — Wayland has no call for it at all, so there the window never moves and the reading says so; it is the driver's record of what it set, not a query, so a level the OS dropped afterwards (a fullscreen space, a tiling manager) is not reported. A popup keeps its own level whatever its owner declares.
    always_on_top: boolean
    -- Declares that this frame wants the keyboard to this window kept from every other process while the window has it — macOS's Secure Keyboard Entry, what a terminal turns on at a password prompt (backlog F85). Frame state the way `alwaysOnTop` is, default false: declare it on every frame the prompt is up, and the frame that stops is what turns it off, so nothing has to remember to undo it. The runner owns the platform call and its balance: `EnableSecureEventInput` is process-wide and counted, and the runner holds one count while a window whose frame asked has the keyboard, giving it back when that window loses the keyboard, closes or stops asking, and at exit — Apple's rule, since while it is on no other process can read the keyboard at all (a launcher's hotkey, a text expander, an accessibility tool). Nothing on Windows or Linux, which have no such switch. A focused `secure` editor asks for it too, on any frame it has the keyboard (backlog F167), and the read-back says so. A C host with its own loop reads the ask with `kui_secure_input_get` and makes the call itself.
    secure_input: boolean
    -- Declares which Option keys act as Alt in this window on macOS (backlog F113). On a Mac, Option composes: ⌥m types `µ`, and ⌥u, ⌥e, ⌥i, ⌥n and ⌥` are dead keys that start an accent and wait for the next key, so the press never reaches the app as a key and a keymap binding `<A-u>` never hears it. An Option key named here is Alt instead: it composes nothing and types nothing, and a key under it arrives as a chord of the key the layout prints unmodified — a terminal's "Option as Meta", an editor's Alt bindings. `"left"` or `"right"` leaves the other side composing, so a user keeps `ü` on one Option; `"both"` takes both; `"none"`, the default, is the Mac's own behaviour. Frame state the way `alwaysOnTop` is: declare it on every frame, and the frame that stops gives the Option keys back to the layout; the runner applies it to the window on change, never per frame. A popup's keys arrive through its owner, so the owner's declaration is the one they are read under. Nothing on Windows or Linux, whose Alt composes nothing. A C host with its own loop reads the ask with `kui_option_as_alt_get` and applies it itself.
    option_as_alt: OptionAsAlt
    -- Declares that this window takes the keyboard as keys, with the platform's input method off (backlog F125): no composition and no candidate window, and on a Mac no dead key waiting for the next and no press-and-hold — an input method too, so a held letter repeats instead of opening the accent picker, whatever the user's `ApplePressAndHoldEnabled` says. A `key` event's `text` is still the layout's character; what goes is everything the OS would have composed from it. What a modal editor's normal mode wants — `jjjj` is how one moves, and an IME left on eats the keymap — while its insert mode stops declaring it and gets accents, dead keys and the IME back. Frame state the way `alwaysOnTop` is, default false: declare it on every frame the mode wants it, and the frame that stops gives the input method back; the runner applies it to the window on change, never per frame, and a composition in progress when it turns off ends without a commit, as an empty `preedit`. The window's, not a node's: a stock editor focused under it composes nothing either. A popup's keys arrive through its owner, so the owner's declaration is the one they are read under. On Windows and Linux the window's IME is disabled the same way, and only that: their dead keys are the layout's, and still compose. A focused `secure` editor asks for it too, as a password field takes keys and not compositions (backlog F167), and the read-back says so. A C host with its own loop reads the ask with `kui_ime_off_get` and applies it itself.
    ime_off: boolean
    -- Declares which windows exist this frame, by stable name (`docs/adr/0004-multi-window.md`). A window opens on the first frame any window's frame declares it — its config is read then and never again, since the user owns its geometry once it exists — and closes on the first frame none does. The driver drains the `Open` / `Close` that result, and the entry's `onWindow` message hears both edges as `window: {phase: "opened" | "closed", id}` (ADR 0048) — read on every declaration, unlike the config, and for a window that closed the last one declared. A window the user closed does not reopen while it is still declared: stop declaring it, then declare it again. `kind: "popup"` makes it a menu surface instead: borderless, off the taskbar, owned by the window that declared it and closed with it, placed in screen coordinates against `anchor` — the `{x, y, w, h}` an `onLayout` node reported — and non-activating unless `activates` says otherwise, so the field that opened it keeps the focus ring while the arrows walk the list. A press outside it or Escape arrives as the entry's `onDismiss` message with `dismiss: {reason, id}` and closes nothing, exactly as a `modal` node's does: stop declaring the window. Reach for a popup only for the placements a float cannot make — a list taller than the window, a menu with nowhere in-window to go, a panel beside the app; everything else stays `fit` plus a `modal` float, which costs one tree instead of an OS surface.
    windows: {string | WindowEntry}
    -- The messages the window's own events arrive as (ADR 0048), one boxed group on the root rather than six rows on every node: `onResize` hears `resize: {width, height, scale}`, `onFocus` this window gaining or losing the keyboard as `window: {phase: "focused" | "blurred"}`, `onSystemSettingsChange` `system: {appearance, accent, motion, locale, assistive, contrast}`, `onFonts` the system's fonts changing (the message as declared), `onModifiers` `modifiers: {shift, ctrl, alt, super}`, and `onOpen` the OS's documents as `open: {paths}` — read on the main window's root alone, since the OS opens documents to the app. Each arrives as its message with the data in the field named for the event, as a node's handler does. A root that declares no group, or a group that leaves a key out, hears none of those events; `env` reads every fact they report either way. Read from the last frame the window finished, so a resize the next frame begins with arrives as the message the frame before declared. A window's opening and closing are its `windows` entry's (`onWindow`), and the menu bar's choice is the `menuBar` element's (`onMenu`). Declared on any node but the root it is ignored, and `window-events-off-root` says so.
    window_events: WindowEvents
    -- Floats a hint below the node while the pointer is over it or over anything inside it — a child button, a field, a row with a `hoverBg` included, as a browser's `title` shows over a descendant; with tooltips nested, the innermost one under the pointer is the one shown (backlog F179). A descendant floated against the viewport or another node — a dialog, a toast — is placed somewhere else, and the hint does not show over it; one floated against its parent, a badge, still counts as inside (backlog RG160). All three effects — hover tracking, the accessible description, and the float itself — come from `PropsOut::apply_tooltip`, so no frontend can implement two of them; a Rust view has all three in `NodeSpec::tooltip` (`NodeSpec::apply_tooltip` is the spec half, for a caller that floats the hint itself). The `description` row is that middle effect on its own, for a hint that is spoken and never drawn. On a box or a `fragment` the float is the node's last child; a leaf holds no children — a `line`, `polygon`, `path`, `cells` grid, `image` or `edit` — and its hint floats beside it instead, anchored to it, and lands below its box the same way, out of every clip and flipping above near the window's bottom (backlog RG113; `PropsOut::for_leaf`). A leaf draws its description, which is the hint unless a `description` applied after it overwrote the slot. A `line`, `polygon` or `path` is hovered by its shape, so its hint shows while the pointer is on the stroke or inside the outline, not anywhere in its box. The hint is drawn at the opacity the node's parent has, not the node's own: a box or a leaf at `opacity: 0.4` has a hint that is not, and a panel fading around it takes the hint with it (backlog F178). A box at `opacity: 0` floats none, and neither does one leaving with an `exit` (backlog RG160).
    tooltip: string
  end

  -- A node: what a constructor returns and a view gives back.
  record Node is Props
    type: string
  end

  record text is Props
    -- Plain or rich text; spans shape as one paragraph, so wrapping crosses style boundaries. A text is content plus a style and no box of its own, so the rows it reads are the style rows (`size`, `lineHeight`, `color`, `family`, `font`, `wrap`, `maxLines`, `ellipsis`, `initialLetter`, `underline`, `strikethrough`, `features`) and nothing else: a container row, an access row (`label`, `role`, `live`), `key` or `onClick` on a text is dropped with an `unknown-prop` warning naming the rows it does take — put them on the box around it. `wrap`, `maxLines` and `ellipsis` control line breaking. A span's `bg` is a background behind its glyphs alone, one rect per line it spans, so it follows the span across a wrap the way a box around a run cannot. With `bgRadius` (`bg_radius` in Lua, `KuiSpan.bg_radius` in C, `Span::bg_radius` in Rust; logical px) the background is rounded and joined into one shape with every rounded background of the same colour and radius it meets: a piece whose edge touches it exactly on the line above or below and overlaps it sideways, or that meets it end to end on its own line, in this text or another. Its corners are then convex where a line reaches past its neighbour, a fillet where it falls short, and round where nothing meets it — a selection over many rows, or over the wrapped lines of a paragraph, is one rounded outline, joined after every text of the frame is laid out and painted, so it is never a frame behind. Nothing names the shape: two that touch are one; `underline` and `strikethrough` on a span or on the whole text are lines where the face puts them. A span takes a face and a size of its own: `family` (a stock name or an installed family's, as the text's) or `font` (a handle, which wins) — inline code in `mono` inside a sans paragraph — and `size` in logical px, its line height scaled at the paragraph's ratio (`Span::family` / `Span::mono` / `Span::size` in Rust, `KuiSpan.family` with `KUI_SPAN_FAMILY`, `.font` and `.size` in C). Its glyphs are shaped in that face, so a caret, a hit, a selection and the measurement read the same glyphs and byte positions stay exact across the change; a line is as tall as its tallest span, and a span's background is its own height around its glyphs. A paragraph with a sized span is shaped whole, never in chunks. A text with no line breaks that is 4096 bytes or longer (and no `maxLines` or `ellipsis`), plain or spans alike, is shaped in ~1 KB chunks as they come on screen, so a minified bundle or a log line with a blob in it costs the screenful it shows and a keystroke into it — or a span moving along it, an editor's caret — costs the chunk it lands in; wrapped, the rows are broken from the chunks' positions, so a 100k-character paragraph costs the rows it shows. Its size is estimated from the first chunk until the rest shape (exact under monospace), and the access tree carries its value without its runs.
    value: any
    -- Plain or rich text; spans shape as one paragraph, so wrapping crosses style boundaries. A text is content plus a style and no box of its own, so the rows it reads are the style rows (`size`, `lineHeight`, `color`, `family`, `font`, `wrap`, `maxLines`, `ellipsis`, `initialLetter`, `underline`, `strikethrough`, `features`) and nothing else: a container row, an access row (`label`, `role`, `live`), `key` or `onClick` on a text is dropped with an `unknown-prop` warning naming the rows it does take — put them on the box around it. `wrap`, `maxLines` and `ellipsis` control line breaking. A span's `bg` is a background behind its glyphs alone, one rect per line it spans, so it follows the span across a wrap the way a box around a run cannot. With `bgRadius` (`bg_radius` in Lua, `KuiSpan.bg_radius` in C, `Span::bg_radius` in Rust; logical px) the background is rounded and joined into one shape with every rounded background of the same colour and radius it meets: a piece whose edge touches it exactly on the line above or below and overlaps it sideways, or that meets it end to end on its own line, in this text or another. Its corners are then convex where a line reaches past its neighbour, a fillet where it falls short, and round where nothing meets it — a selection over many rows, or over the wrapped lines of a paragraph, is one rounded outline, joined after every text of the frame is laid out and painted, so it is never a frame behind. Nothing names the shape: two that touch are one; `underline` and `strikethrough` on a span or on the whole text are lines where the face puts them. A span takes a face and a size of its own: `family` (a stock name or an installed family's, as the text's) or `font` (a handle, which wins) — inline code in `mono` inside a sans paragraph — and `size` in logical px, its line height scaled at the paragraph's ratio (`Span::family` / `Span::mono` / `Span::size` in Rust, `KuiSpan.family` with `KUI_SPAN_FAMILY`, `.font` and `.size` in C). Its glyphs are shaped in that face, so a caret, a hit, a selection and the measurement read the same glyphs and byte positions stay exact across the change; a line is as tall as its tallest span, and a span's background is its own height around its glyphs. A paragraph with a sized span is shaped whole, never in chunks. A text with no line breaks that is 4096 bytes or longer (and no `maxLines` or `ellipsis`), plain or spans alike, is shaped in ~1 KB chunks as they come on screen, so a minified bundle or a log line with a blob in it costs the screenful it shows and a keystroke into it — or a span moving along it, an editor's caret — costs the chunk it lands in; wrapped, the rows are broken from the chunks' positions, so a 100k-character paragraph costs the rows it shows. Its size is estimated from the first chunk until the rest shape (exact under monospace), and the access tree carries its value without its runs.
    spans: any
  end

  record button is Props
    -- The stock button: `widgets::button_spec(&theme, &metrics)` — the theme's accent trio as its three backgrounds, declared on the node and resolved by the core — keyed by its text (`key` overrides). It paints from the palette like every stock widget (backlog AR41): the OS's accent where the host reports one, the app's where it set or pinned one, kui's blue otherwise; the label goes black or white by the background's luminance. Its look is its spec, so the layout and paint rows are closed — declared, they are dropped with an `unknown-prop` warning naming the rows it does read — and those are the access rows: `label` when the text is not the name, `description`, `tooltip`, and `disabled` (inert, and dimmed to half). The one paint row it takes is `accent`, which on a button changes nothing (it is the accent already) and is kept for the box's sake. In Lua `label` is the name and the text both unless `text` says otherwise; in C the rows ride a `KuiSpec` whose other fields `kui_button_with` ignores. A button that needs any other row is a box with `role="button"` and the same rows spelled out.
    text: any
  end

  record edit is Props
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    initial: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    multiline: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    autofocus: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    keep_tab: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    placeholder: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    secure: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    tab_moves_focus: any
    -- Retained editor state by key. Its edits arrive as its `onChange` message with `change: {text}` in it, the whole text after the edit; Enter as its `onSubmit` message and Escape as its `onCancel` one; an editor that declares none of them hears nothing (ADR 0047). `value` is the text declared every frame: where it differs from the editor's, the editor is brought to it as a splice that keeps the caret, and a correction made in the `update` the edit reached joins that edit's undo step — so the model is the editor's, with `onChange` storing the text (without it the field reverts every keystroke, `edit-value-without-handler`). `initial` seeds a new editor only, and is never read beside `value` (`edit-value-and-initial`) — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end and clears undo). Name it by the label its `key` prop declares — `setEditText('note', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap="word"` or `"glyph"`): it folds to its width the way a document does and keeps a field's keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width="fit"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first) — except a `secure` one, forgotten with its text zeroed on the first frame that does not declare it (backlog F177). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes an editor's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab. A `multiline` editor takes Tab, as a code editor does (backlog C99): Tab types four spaces and Shift-Tab takes up to four back from the start of the line, and the keyboard keeps three ways out — Escape then Tab moves focus (the Escape arms the next Tab only, and a second Escape lets go), the window's "Tab moves focus" toggle (Ctrl+M, Ctrl+Shift+M on macOS; `env.tab_moves_focus`) makes every such editor walk the ring, and Tab under Ctrl, Alt or the platform primary walks it always. One that declares `tabMovesFocus` (Lua `tab_moves_focus`) is left by Tab and Shift-Tab as a field is, as a browser's textarea is. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none. `secure` makes it a password field (backlog F167): the text is drawn as a bullet (•) per grapheme and measured, hit and selected as the bullets; a copy or a cut from it is refused and a paste taken; no input method composes into it, and it keeps no undo history; the access tree reads it as a `passwordInput` whose value is the bullets; and while it has the keyboard the window asks for secure keyboard entry and the input method off, as `secureInput` and `imeOff` would. `editText` still reads the text. Declare it off on a later frame — an app's show-password toggle — and the same editor draws its text again, the caret where it was; on again, it masks it. A secure editor is a field: `multiline` is ignored, and the word keys and a double click take the whole text, as there are no words to show. The text the app was handed (each `change`'s, `editText`'s answer) is the app's copy, and the app's to clear. In C it is `KUI_EDIT_SECURE`, in Odin `.Secure`.
    value: any
  end

  record select is Props
    -- The stock select (`widgets::select_items`, backlog F72): a field showing the choice in force that, clicked, opens the core's own menu of the options under it with the current one checked — the menu a right-click opens, drawn in the frame or the platform's where the host shows menus itself, dismissed by Escape or a press outside, its rows walked by the arrows and read as a menu. `label` is the key and the accessible name both; `options` is a list whose entries are strings (an option by its label, posting it) or menu-item objects `{ label, id, enabled }` (posting `id`), and a `{ role: "separator" }` is a separator; `current` is the index in force, counted from 0 in JSX and C and from 1 in Lua, or none — one past the options or on a separator is none, with a `select-current-ignored` warning on the field; an empty `options` is refused, and a key of an option object no row reads (`disabled`, where the key is `enabled`) is an `unknown-prop` warning. The app holds no open state: the choice arrives as the field's `onMenu` message with `menu: {role: "custom", item: <the option>}` in it, on the field's key — a field that declares no `onMenu` still reports, as `{menu: …}` alone (ADR 0047) — and drawing the field again with the new `current` is the whole loop. A reader hears a button named by the field, described by its choice, expanded while the menu is open. Its look is its spec, so it reads no other row: a layout, paint or access row on it is dropped with an `unknown-prop` warning. Lua spells it `dropdown`, since `select` is Lua's own.
    options: any
    -- The stock select (`widgets::select_items`, backlog F72): a field showing the choice in force that, clicked, opens the core's own menu of the options under it with the current one checked — the menu a right-click opens, drawn in the frame or the platform's where the host shows menus itself, dismissed by Escape or a press outside, its rows walked by the arrows and read as a menu. `label` is the key and the accessible name both; `options` is a list whose entries are strings (an option by its label, posting it) or menu-item objects `{ label, id, enabled }` (posting `id`), and a `{ role: "separator" }` is a separator; `current` is the index in force, counted from 0 in JSX and C and from 1 in Lua, or none — one past the options or on a separator is none, with a `select-current-ignored` warning on the field; an empty `options` is refused, and a key of an option object no row reads (`disabled`, where the key is `enabled`) is an `unknown-prop` warning. The app holds no open state: the choice arrives as the field's `onMenu` message with `menu: {role: "custom", item: <the option>}` in it, on the field's key — a field that declares no `onMenu` still reports, as `{menu: …}` alone (ADR 0047) — and drawing the field again with the new `current` is the whole loop. A reader hears a button named by the field, described by its choice, expanded while the menu is open. Its look is its spec, so it reads no other row: a layout, paint or access row on it is dropped with an `unknown-prop` warning. Lua spells it `dropdown`, since `select` is Lua's own.
    current: any
  end

  record checkbox is Props
    -- The stock checkbox (`widgets::toggle_with`, ADR 0034): a box drawn from the state the view declares — `checked`, or `mixed` for the select-all box over a list some of whose rows are selected, drawn as a dash and read as mixed — and its label beside it, keyed by its text (`key` overrides). The state is the app's: a press by the pointer, Space, Enter or assistive technology posts `onClick`, and the view flips its model and draws it again. Its look is its spec, so the layout and paint rows are closed and dropped with an `unknown-prop` warning; the rows it reads are its state, the access rows, and its label's line breaking (backlog F184): `ellipsis` keeps the label on one line, cut with an ellipsis where the checkbox is squeezed — a list of long names in a narrow column — and `maxLines` (with `ellipsis`, a clamp) and `wrap` are the text's own rows, read on the label; the label's size, colour and face stay the stock's (C: `kui_checkbox_with`'s `KuiTextStyle`, of which it reads those three). A cut label is still the checkbox: a press on it toggles, its name is the whole text, and while it is cut the whole text floats as its hint on hover, unless a `tooltip` says something else. In Lua `label` is the name and the text both unless `text` says otherwise. The box is the metrics' control text plus one (16 px comfortable), so `compact` and `scaled` move it with the stock button.
    text: any
  end

  record radio is Props
    -- The stock radio (`widgets::toggle_with`, ADR 0034): a circle drawn from `checked`, and its label, keyed by its text. Put radios in a `radioGroup`, which makes them one Tab stop whose arrows, Home and End move the choice and press the radio they land on (ADR 0007), so radios whose `onClick` each set the choice answer the keyboard with no more code. The state is the app's, as a checkbox's is; the rows are the checkbox's, `mixed` aside.
    text: any
  end

  record switch is Props
    -- The stock switch (`widgets::toggle_with`, ADR 0034): a track and a knob drawn from `checked`, the knob sliding across when it changes, and its label; read as a switch, on or off. The state is the app's, as a checkbox's is; the rows are the checkbox's, `mixed` aside.
    text: any
  end

  record image is Props
    -- A registered RGBA image. Sizing: `width="fit"` takes the pixel size, a fit height against a resolved width keeps the aspect, `radius` rounds it. Two rows say how the pixels meet the box (`docs/adr/0025-the-image-is-the-canvas.md`): `sampling` is `linear` (the default) or `nearest` — pixel art, an emulator, a data grid that must stay square under zoom; `fit` is `fill` (the default: the pixels stretch to the box), `contain` (the largest rect of the image's aspect that fits, centred, the rest of the box showing what is behind) or `cover` (the box filled and the pixels that do not fit cropped, centred). The box — its layout, its hit region, its access rect — is the same in every mode. The pixels come from the atlas, or from a texture of the image's own once `updateImage` has replaced them or when no atlas page could hold them; the node cannot tell and need not. Drawn at less than half its texels a pixel, a `linear` image is drawn from a level the core halved it to (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`): the deepest level with at least a texel a pixel, the 2×2 means taken in linear light, made once per session at the first draw that wants it and kept in the atlas in place of the whole image — so a photo registered as decoded and shown on a card is neither resampled by the app nor aliased on screen. The display's scale and any `scale` the node is drawn through count. `nearest` and an image ever updated (a stream) draw the whole image.
    id: any
    -- A registered RGBA image. Sizing: `width="fit"` takes the pixel size, a fit height against a resolved width keeps the aspect, `radius` rounds it. Two rows say how the pixels meet the box (`docs/adr/0025-the-image-is-the-canvas.md`): `sampling` is `linear` (the default) or `nearest` — pixel art, an emulator, a data grid that must stay square under zoom; `fit` is `fill` (the default: the pixels stretch to the box), `contain` (the largest rect of the image's aspect that fits, centred, the rest of the box showing what is behind) or `cover` (the box filled and the pixels that do not fit cropped, centred). The box — its layout, its hit region, its access rect — is the same in every mode. The pixels come from the atlas, or from a texture of the image's own once `updateImage` has replaced them or when no atlas page could hold them; the node cannot tell and need not. Drawn at less than half its texels a pixel, a `linear` image is drawn from a level the core halved it to (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`): the deepest level with at least a texel a pixel, the 2×2 means taken in linear light, made once per session at the first draw that wants it and kept in the atlas in place of the whole image — so a photo registered as decoded and shown on a card is neither resampled by the app nor aliased on screen. The display's scale and any `scale` the node is drawn through count. `nearest` and an image ever updated (a stream) draw the whole image.
    sampling: any
    -- A registered RGBA image. Sizing: `width="fit"` takes the pixel size, a fit height against a resolved width keeps the aspect, `radius` rounds it. Two rows say how the pixels meet the box (`docs/adr/0025-the-image-is-the-canvas.md`): `sampling` is `linear` (the default) or `nearest` — pixel art, an emulator, a data grid that must stay square under zoom; `fit` is `fill` (the default: the pixels stretch to the box), `contain` (the largest rect of the image's aspect that fits, centred, the rest of the box showing what is behind) or `cover` (the box filled and the pixels that do not fit cropped, centred). The box — its layout, its hit region, its access rect — is the same in every mode. The pixels come from the atlas, or from a texture of the image's own once `updateImage` has replaced them or when no atlas page could hold them; the node cannot tell and need not. Drawn at less than half its texels a pixel, a `linear` image is drawn from a level the core halved it to (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`): the deepest level with at least a texel a pixel, the 2×2 means taken in linear light, made once per session at the first draw that wants it and kept in the atlas in place of the whole image — so a photo registered as decoded and shown on a card is neither resampled by the app nor aliased on screen. The display's scale and any `scale` the node is drawn through count. `nearest` and an image ever updated (a stream) draw the whole image.
    fit: any
  end

  record polygon is Props
    -- A filled polygon through up to eight `points`, the fill in `bg` (`docs/adr/0025-the-image-is-the-canvas.md`, decision 6): an arrowhead, a pie slice, the area under a curve. Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box a pixel out on each side, so it takes no room in a row or column, and painted in the parent's layer at its place in the tree, over the siblings before it and under those after (backlog F123). A polygon in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` polygon escapes (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `transition` eases the fill and, with `slide`, its position. The outline may be concave; a self-intersecting one fills even-odd, its overlaps unfilled. Hit by its outline (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside the outline hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable wedge is a button), so name it. A ninth point and later are dropped with `polygon-points-truncated`; fewer than three draw nothing; no `bg`, no fill. On the wire it is one `fragment` quad painted by a WGSL function the core registers itself, so a host that draws the list gets its source from `kui_fragment_source` like any other; what it costs is that quad and one pipeline switch per run of polygons. A stroked outline is a closed `line` over it.
    points: any
  end

  record path is Props
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    d: any
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    ops: any
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    fill_rule: any
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    pivot: any
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    dash: any
    -- Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. A `d` that changes is rasterized whole, each time, in time linear in the mask's area and the outline's length (backlog C96): the area under a 300-point series over a 360 × 120 plot at scale 2 is about 0.2 ms, under a 1,400-vertex zigzag that crosses every scanline at every step about 6 ms, two thirds of it the bleed's outline. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; C says a turn that passes through 0 with `KUI_TURNED` OR'd into the fill rule (Odin `c.TURNED`), so a spinner keeps that box at 0, where JSX and Lua tell an absent `rotate` from 0 themselves; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.
    dash_offset: any
  end

  record svg is Props
    -- An SVG document drawn in a box (`docs/adr/0063-an-svg-element.md`): `data` is the markup itself, `src` a file read once and again when it changes (one of the two). `mode` is `auto` (the default) — kui paths when every shape is one a `path` draws exactly, crisp at any size and recoloured live, and one image rasterized at the drawn size and scale when it is not (a gradient, a clip, a mask, a filter, an embedded picture, a butt cap) — or `paths` (always, approximating what a path cannot draw, with `svg-approximated`) or `image` (always). `fit` places the picture's viewBox in the box: `contain` (the default: whole, centred), `cover` (filling it, the overflow clipped) or `fill` (stretched). `color` is what `currentColor` paints — the theme's foreground by default — so an icon drawn `stroke="currentColor"` takes the ink of the text beside it, or a token's. With no `width` or `height` the box is the document's own size; one given, the other keeps its aspect; `grow` or a percentage draws at the size the box was laid out at, a frame after it changes. The box is read as an image: `label` names it, `role="none"` makes it decoration (`image-without-label` otherwise). A document that does not parse draws an empty box and raises `svg-malformed`, a `src` that cannot be read `svg-unreadable`. Never reads the network or another file: an `<image>` the document names by path is left out.
    data: any
    -- An SVG document drawn in a box (`docs/adr/0063-an-svg-element.md`): `data` is the markup itself, `src` a file read once and again when it changes (one of the two). `mode` is `auto` (the default) — kui paths when every shape is one a `path` draws exactly, crisp at any size and recoloured live, and one image rasterized at the drawn size and scale when it is not (a gradient, a clip, a mask, a filter, an embedded picture, a butt cap) — or `paths` (always, approximating what a path cannot draw, with `svg-approximated`) or `image` (always). `fit` places the picture's viewBox in the box: `contain` (the default: whole, centred), `cover` (filling it, the overflow clipped) or `fill` (stretched). `color` is what `currentColor` paints — the theme's foreground by default — so an icon drawn `stroke="currentColor"` takes the ink of the text beside it, or a token's. With no `width` or `height` the box is the document's own size; one given, the other keeps its aspect; `grow` or a percentage draws at the size the box was laid out at, a frame after it changes. The box is read as an image: `label` names it, `role="none"` makes it decoration (`image-without-label` otherwise). A document that does not parse draws an empty box and raises `svg-malformed`, a `src` that cannot be read `svg-unreadable`. Never reads the network or another file: an `<image>` the document names by path is left out.
    src: any
    -- An SVG document drawn in a box (`docs/adr/0063-an-svg-element.md`): `data` is the markup itself, `src` a file read once and again when it changes (one of the two). `mode` is `auto` (the default) — kui paths when every shape is one a `path` draws exactly, crisp at any size and recoloured live, and one image rasterized at the drawn size and scale when it is not (a gradient, a clip, a mask, a filter, an embedded picture, a butt cap) — or `paths` (always, approximating what a path cannot draw, with `svg-approximated`) or `image` (always). `fit` places the picture's viewBox in the box: `contain` (the default: whole, centred), `cover` (filling it, the overflow clipped) or `fill` (stretched). `color` is what `currentColor` paints — the theme's foreground by default — so an icon drawn `stroke="currentColor"` takes the ink of the text beside it, or a token's. With no `width` or `height` the box is the document's own size; one given, the other keeps its aspect; `grow` or a percentage draws at the size the box was laid out at, a frame after it changes. The box is read as an image: `label` names it, `role="none"` makes it decoration (`image-without-label` otherwise). A document that does not parse draws an empty box and raises `svg-malformed`, a `src` that cannot be read `svg-unreadable`. Never reads the network or another file: an `<image>` the document names by path is left out.
    mode: any
    -- An SVG document drawn in a box (`docs/adr/0063-an-svg-element.md`): `data` is the markup itself, `src` a file read once and again when it changes (one of the two). `mode` is `auto` (the default) — kui paths when every shape is one a `path` draws exactly, crisp at any size and recoloured live, and one image rasterized at the drawn size and scale when it is not (a gradient, a clip, a mask, a filter, an embedded picture, a butt cap) — or `paths` (always, approximating what a path cannot draw, with `svg-approximated`) or `image` (always). `fit` places the picture's viewBox in the box: `contain` (the default: whole, centred), `cover` (filling it, the overflow clipped) or `fill` (stretched). `color` is what `currentColor` paints — the theme's foreground by default — so an icon drawn `stroke="currentColor"` takes the ink of the text beside it, or a token's. With no `width` or `height` the box is the document's own size; one given, the other keeps its aspect; `grow` or a percentage draws at the size the box was laid out at, a frame after it changes. The box is read as an image: `label` names it, `role="none"` makes it decoration (`image-without-label` otherwise). A document that does not parse draws an empty box and raises `svg-malformed`, a `src` that cannot be read `svg-unreadable`. Never reads the network or another file: an `<image>` the document names by path is left out.
    fit: any
  end

  record fragment is Props
    -- A box a registered WGSL function paints (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`): a conic or a moving gradient, rings, noise, shimmer — anything the paint vocabulary has no prop for. An ordinary node otherwise — it lays out, rounds, clips, fades, takes input and holds children, which paint over it — but with **no intrinsic size**, so give it a `width`/`height` or `fill` or it is zero by zero. `src` is a handle from `add_fragment`, which validates the source and warns rather than minting one that cannot compile. `params` is up to sixteen numbers the shader reads as four `vec4<f32>`; more are dropped with a warning. `image` is a registered image the function reads — `kui_sample(uv)` (bilinear) and `kui_sample_nearest(uv)` return its texels at `uv` in `[0,1]²`, and `in.image` is its texel rect, `zw` the size — which is what makes a replaced image a waveform, a heatmap, a 50k-point line or an image effect from one quad (`docs/adr/0025-the-image-is-the-canvas.md`, decision 7); the core binds the atlas or the image's own texture, whichever holds it, and a fragment whose image is not live draws nothing, as one whose `src` is not does. `animate` asks for a frame every frame, which is what a fragment that reads `time` needs and what a still one must not declare.
    id: any
    -- A box a registered WGSL function paints (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`): a conic or a moving gradient, rings, noise, shimmer — anything the paint vocabulary has no prop for. An ordinary node otherwise — it lays out, rounds, clips, fades, takes input and holds children, which paint over it — but with **no intrinsic size**, so give it a `width`/`height` or `fill` or it is zero by zero. `src` is a handle from `add_fragment`, which validates the source and warns rather than minting one that cannot compile. `params` is up to sixteen numbers the shader reads as four `vec4<f32>`; more are dropped with a warning. `image` is a registered image the function reads — `kui_sample(uv)` (bilinear) and `kui_sample_nearest(uv)` return its texels at `uv` in `[0,1]²`, and `in.image` is its texel rect, `zw` the size — which is what makes a replaced image a waveform, a heatmap, a 50k-point line or an image effect from one quad (`docs/adr/0025-the-image-is-the-canvas.md`, decision 7); the core binds the atlas or the image's own texture, whichever holds it, and a fragment whose image is not live draws nothing, as one whose `src` is not does. `animate` asks for a frame every frame, which is what a fragment that reads `time` needs and what a still one must not declare.
    image: any
    -- A box a registered WGSL function paints (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`): a conic or a moving gradient, rings, noise, shimmer — anything the paint vocabulary has no prop for. An ordinary node otherwise — it lays out, rounds, clips, fades, takes input and holds children, which paint over it — but with **no intrinsic size**, so give it a `width`/`height` or `fill` or it is zero by zero. `src` is a handle from `add_fragment`, which validates the source and warns rather than minting one that cannot compile. `params` is up to sixteen numbers the shader reads as four `vec4<f32>`; more are dropped with a warning. `image` is a registered image the function reads — `kui_sample(uv)` (bilinear) and `kui_sample_nearest(uv)` return its texels at `uv` in `[0,1]²`, and `in.image` is its texel rect, `zw` the size — which is what makes a replaced image a waveform, a heatmap, a 50k-point line or an image effect from one quad (`docs/adr/0025-the-image-is-the-canvas.md`, decision 7); the core binds the atlas or the image's own texture, whichever holds it, and a fragment whose image is not live draws nothing, as one whose `src` is not does. `animate` asks for a frame every frame, which is what a fragment that reads `time` needs and what a still one must not declare.
    params: any
  end

  record cells is Props
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    rows: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    cols: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    lines: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    runs: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    cursor_at: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    cursor_shape: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    cursor_color: any
    -- A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.
    origin_line: any
  end

  record line is Props
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    from: any
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    to: any
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    points: any
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    curve: any
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    dash: any
    -- A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float="viewport"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float="viewport"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.
    dash_offset: any
  end

  record titlebar is Props
    -- Adaptive titlebar for custom chrome: drag strip, native-control inset, window buttons.
    title: any
  end

  record menuBar is Props
    -- The application menu (`docs/adr/0018-a-menu-bar-the-app-declares.md`): `menu` is what it *is*, and where this element sits is where its titles go when they have to be drawn in the window. One call and not two, because declaring the menu and placing the strip are one decision. It draws **nothing** where the platform owns the bar — macOS, where the driver hands the same declaration to the OS — so the frame has still said what the app's menu is and the strip simply is not there; that is the contract `windowButtons` has under native decorations, and it is what makes one view portable. Its rows are the rows a context menu has: the same `role`s the core performs itself (`copy`, `paste`, `selectAll`, `cut`, `lookUp`), the same `id` payload, the same `accel` text, plus `checked` for a setting — and choosing one arrives as the bar's own `onMenu` message with `menu: {role, item}` in it (ADR 0048), the field a context menu's choice carries on its target's `onMenu`; a bar that declares no `onMenu` posts nothing. Declared every frame and diffed: an unchanged menu costs a comparison, and an empty list takes it away. An accelerator kui can parse is rewritten into the platform's spelling, so `"mod+s"` reads as `⌘S` on macOS and `Ctrl+S` elsewhere and binds that key in the platform's own bar. On macOS the first menu is the application menu, which the OS titles with the app's own name whatever the label says — unless the bar declares `standardMenus` (ADR 0052): then the platform's standard menus stay and these go beside them, so adding one menu is declaring that one menu. On macOS that is the application menu the process has, the standard Edit menu, these menus in order, and the standard Window menu with its tiling and full-screen rows; none of these menus is the application menu, a menu titled `Window` is only a menu, and an `accel` binds as any row's does. Where the bar is drawn in the window (Windows, Linux, a core drawing its own) there are no standard menus, and the bar is these menus alone. While a menu is open the bar is the frame's modal scope, so hovering across the titles moves the open menu, a press on the open title closes it, and Escape or a press in the app below closes it and reaches nothing else.
    menu: any
    -- The application menu (`docs/adr/0018-a-menu-bar-the-app-declares.md`): `menu` is what it *is*, and where this element sits is where its titles go when they have to be drawn in the window. One call and not two, because declaring the menu and placing the strip are one decision. It draws **nothing** where the platform owns the bar — macOS, where the driver hands the same declaration to the OS — so the frame has still said what the app's menu is and the strip simply is not there; that is the contract `windowButtons` has under native decorations, and it is what makes one view portable. Its rows are the rows a context menu has: the same `role`s the core performs itself (`copy`, `paste`, `selectAll`, `cut`, `lookUp`), the same `id` payload, the same `accel` text, plus `checked` for a setting — and choosing one arrives as the bar's own `onMenu` message with `menu: {role, item}` in it (ADR 0048), the field a context menu's choice carries on its target's `onMenu`; a bar that declares no `onMenu` posts nothing. Declared every frame and diffed: an unchanged menu costs a comparison, and an empty list takes it away. An accelerator kui can parse is rewritten into the platform's spelling, so `"mod+s"` reads as `⌘S` on macOS and `Ctrl+S` elsewhere and binds that key in the platform's own bar. On macOS the first menu is the application menu, which the OS titles with the app's own name whatever the label says — unless the bar declares `standardMenus` (ADR 0052): then the platform's standard menus stay and these go beside them, so adding one menu is declaring that one menu. On macOS that is the application menu the process has, the standard Edit menu, these menus in order, and the standard Window menu with its tiling and full-screen rows; none of these menus is the application menu, a menu titled `Window` is only a menu, and an `accel` binds as any row's does. Where the bar is drawn in the window (Windows, Linux, a core drawing its own) there are no standard menus, and the bar is these menus alone. While a menu is open the bar is the frame's modal scope, so hovering across the titles moves the open menu, a press on the open title closes it, and Escape or a press in the app below closes it and reaches nothing else.
    standard_menus: any
  end

  record tooltip is Props
    -- A float hanging below the parent; the node form always draws, the prop form is hover-gated.
    value: any
  end

  record latencyGraph is Props
    -- Per-phase frame timing (windowed drivers fill it; headless shows the chrome empty).
    at: any
  end

  record audio is Props
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    src: any
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    loop: any
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    volume: any
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    paused: any
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    finish: any
    -- A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings that message back with `sound: {phase: "ended", playback}` in it (ADR 0047). Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: "refused"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.
    tag: any
  end

  -- What `uniform_list` reads off its options; every other key is the container's.
  record UniformList is Props
    -- The container's key, which its scroll geometry is read back by.
    key: string
    -- One row's height: the whole stride, spacing included.
    row_h: number
    -- How many rows the list has; none when left out.
    rows: number
    -- Rows built past each end of the viewport; two when left out.
    overscan: number
    -- Each row's own node's props — its click, stripe, hover; its `index` and `height` stay the list's.
    row_props: function(i: integer): Props
  end

  -- What `list` reads off its options; every other key is the container's.
  record List is Props
    -- The container's key, which its scroll geometry is read back by.
    key: string
    -- The heights the list slices by, made once with `row_heights` and kept.
    heights: RowHeights
    -- Rows built past each end of the viewport; two when left out.
    overscan: number
  end

  -- Bulk data the core owns, read by row and column (ADR 0065); rows are 0-based.
  record Channel
    rev: integer
    id: integer
    names: {string}
    metamethod __len: function(self: Channel): integer
    get: function(self: Channel, row: number, col: string | integer): any
    row: function(self: Channel, row: number): {string:any}
    column: function(self: Channel, col: string | integer): {any}
    reduce: function(self: Channel, col: string | integer, from: number, to: number, n: number): ChannelReduction
    publish: function(self: Channel, columns: {string:{any}}): Channel
  end

  -- What `ch:reduce` answers (backlog C95): a list per field, a bucket per index; rows 0-based.
  record ChannelReduction
    min: {number}
    max: {number}
    first: {number}
    last: {number}
    min_row: {integer}
    max_row: {integer}
    first_row: {integer}
    last_row: {integer}
  end

  -- A variable-height list's row heights: measured where known, the mean of those elsewhere (rows are 0-based).
  record RowHeights
    len: function(self: RowHeights): integer
    set_len: function(self: RowHeights, rows: number)
    clear: function(self: RowHeights)
    set: function(self: RowHeights, i: number, h: number)
    measured: function(self: RowHeights, i: number): number
    get: function(self: RowHeights, i: number): number
    estimate: function(self: RowHeights): number
    total: function(self: RowHeights): number
    offset_of: function(self: RowHeights, i: number): number
    row_at: function(self: RowHeights, y: number): integer
  end

  -- An event `on_event` hears: the message its handler declared, the event's data in the
  -- field named for the event (`drag`, `change`, …), and the envelope beside them. An app's
  -- message extends it — `local record Pan is kui.Event, kui.WithDrag` — and is cast from `ev`.
  interface Event
    -- What happened: the event kind, beside the message rather than in it.
    event: EventKind
    -- The emitting node's key, what `env.edit_text` and the rest take.
    node_key: integer
    -- The window it came from, as `env.window.id` reads; a `window` event's data, which has the name.
    window: integer | WindowEvent
    -- The key of the component instance the node was drawn in.
    instance: integer
    -- That instance's full name, `ns/name`.
    component: string
    -- The namespace of the plugin that raised it.
    from: string
    -- The message's own `kind`: the core never writes one.
    kind: string
    -- A message that is not a table, kept under `tag`.
    tag: any
  end

  -- `on_event`'s second argument: the envelope alone, whatever fields the message has.
  record Envelope
    event: EventKind
    node_key: integer
    window: integer
    instance: integer
    component: string
    from: string
  end

  record Rect
    x: number
    y: number
    w: number
    h: number
  end

  record Cell
    row: integer
    col: integer
  end

  record LinePos
    line: integer
    offset: integer
  end

  record RangeEnd
    index: integer
    byte: integer
  end

  record Needs
    w: number
    h: number
  end

  record Lazy
    needs: Needs
    query: {any:any}
    render: function(): Node
    fallback: function(): Node
    on_query: any
    key: string
  end

  -- A `click` event's data.
  record ClickEvent
    cell: Cell
    line: integer
    byte: integer
    inside: boolean
    clicks: integer
  end

  -- A message that carries a `click` event: its `click` field alone.
  interface WithClick
    click: ClickEvent
  end

  -- A `drag` event's data.
  record DragEvent
    phase: DragEventPhase
    x: number
    y: number
    dx: number
    dy: number
    parent: Rect
    cell: Cell
    line: integer
    byte: integer
    inside: boolean
    clicks: integer
  end

  -- A message that carries a `drag` event: its `drag` field alone.
  interface WithDrag
    drag: DragEvent
  end

  -- A `key` event's data.
  record KeyEvent
    phase: KeyEventPhase
    code: string
    physical: string
    shift: boolean
    ctrl: boolean
    alt: boolean
    super: boolean
    text: string
    ["repeat"]: boolean
    location: KeyEventLocation
    caps_lock: boolean
    num_lock: boolean
  end

  -- A message that carries a `key` event: its `key` field alone.
  interface WithKey
    key: KeyEvent
  end

  -- A `text` event's data.
  record TextEvent
    text: string
    pasted: boolean
    concealed: boolean
    transient: boolean
  end

  -- A message that carries a `text` event: its `text` field alone.
  interface WithText
    text: TextEvent
  end

  -- A `preedit` event's data.
  record PreeditEvent
    text: string
    cursor: {integer}
  end

  -- A message that carries a `preedit` event: its `preedit` field alone.
  interface WithPreedit
    preedit: PreeditEvent
  end

  -- A `select` event's data.
  record SelectEvent
    anchor: LinePos
    focus: LinePos
  end

  -- A message that carries a `select` event: its `select` field alone.
  interface WithSelect
    select: SelectEvent
  end

  -- A `selectionrange` event's data.
  record SelectionRangeEvent
    from: RangeEnd
    to: RangeEnd
  end

  -- A message that carries a `selectionrange` event: its `selectionrange` field alone.
  interface WithSelectionRange
    selectionrange: SelectionRangeEvent
  end

  -- A `contextmenu` event's data.
  record ContextMenuEvent
    x: number
    y: number
  end

  -- A message that carries a `contextmenu` event: its `contextmenu` field alone.
  interface WithContextMenu
    contextmenu: ContextMenuEvent
  end

  -- A `menu` event's data.
  record MenuEvent
    role: string
    item: any
  end

  -- A message that carries a `menu` event: its `menu` field alone.
  interface WithMenu
    menu: MenuEvent
  end

  -- A `forceclick` event's data.
  record ForceClickEvent
    x: number
    y: number
  end

  -- A message that carries a `forceclick` event: its `forceclick` field alone.
  interface WithForceClick
    forceclick: ForceClickEvent
  end

  -- A `button` event's data.
  record ButtonEvent
    phase: ButtonEventPhase
    button: ButtonEventButtonWord | integer
    x: number
    y: number
    clicks: integer
    cell: Cell
    line: integer
    byte: integer
    inside: boolean
  end

  -- A message that carries a `button` event: its `button` field alone.
  interface WithButton
    button: ButtonEvent
  end

  -- A `scroll` event's data.
  record ScrollEvent
    x: number
    y: number
    dx: number
    dy: number
    lines: integer
    mods: ScrollEventMods
  end

  -- A message that carries a `scroll` event: its `scroll` field alone.
  interface WithScroll
    scroll: ScrollEvent
  end

  -- A `focus` event's data.
  record FocusEvent
    phase: FocusEventPhase
    by: FocusEventBy
  end

  -- A message that carries a `focus` event: its `focus` field alone.
  interface WithFocus
    focus: FocusEvent
  end

  -- A `hover` event's data.
  record HoverEvent
    phase: HoverEventPhase
    by: HoverEventBy
  end

  -- A message that carries a `hover` event: its `hover` field alone.
  interface WithHover
    hover: HoverEvent
  end

  -- A `drop` event's data.
  record DropEvent
    phase: DropEventPhase
    paths: {string}
    x: number
    y: number
  end

  -- A message that carries a `drop` event: its `drop` field alone.
  interface WithDrop
    drop: DropEvent
  end

  -- A `typeahead` event's data.
  record TypeaheadEvent
    text: string
  end

  -- A message that carries a `typeahead` event: its `typeahead` field alone.
  interface WithTypeahead
    typeahead: TypeaheadEvent
  end

  -- A `files` event's data.
  record FilesEvent
    paths: {string}
  end

  -- A message that carries a `files` event: its `files` field alone.
  interface WithFiles
    files: FilesEvent
  end

  -- A `range` event's data.
  record RangeEvent
    first: integer
    last: integer
  end

  -- A message that carries a `range` event: its `range` field alone.
  interface WithRange
    range: RangeEvent
  end

  -- A `query` event's data.
  record QueryEvent
    matched: {integer}
    width: number
    height: number
  end

  -- A message that carries a `query` event: its `query` field alone.
  interface WithQuery
    query: QueryEvent
  end

  -- A `layout` event's data.
  record LayoutEvent
    x: number
    y: number
    w: number
    h: number
    parent: Rect
    scale: number
  end

  -- A message that carries a `layout` event: its `layout` field alone.
  interface WithLayout
    layout: LayoutEvent
  end

  -- A `change` event's data.
  record SliderChangeEvent
    value: number
    phase: SliderChangeEventPhase
  end

  -- A message that carries a `change` event: its `change` field alone.
  interface WithSliderChange
    change: SliderChangeEvent
  end

  -- A `change` event's data.
  record EditChangeEvent
    text: string
  end

  -- A message that carries a `change` event: its `change` field alone.
  interface WithEditChange
    change: EditChangeEvent
  end

  -- A `boundary` event's data.
  record BoundaryEvent
    key: BoundaryEventKey
    edge: BoundaryEventEdge
    word: boolean
    doc: boolean
    ctrl: boolean
  end

  -- A message that carries a `boundary` event: its `boundary` field alone.
  interface WithBoundary
    boundary: BoundaryEvent
  end

  -- A `sound` event's data.
  record SoundEvent
    phase: SoundEventPhase
    playback: integer
  end

  -- A message that carries a `sound` event: its `sound` field alone.
  interface WithSound
    sound: SoundEvent
  end

  -- A `capture` event's data.
  record CaptureEvent
    phase: CaptureEventPhase
    path: string
    width: integer
    height: integer
    frames: integer
    seconds: number
    error: string
  end

  -- A message that carries a `capture` event: its `capture` field alone.
  interface WithCapture
    capture: CaptureEvent
  end

  -- A `dismiss` event's data.
  record DismissEvent
    reason: DismissEventReason
    id: integer
  end

  -- A message that carries a `dismiss` event: its `dismiss` field alone.
  interface WithDismiss
    dismiss: DismissEvent
  end

  -- A `resize` event's data.
  record ResizeEvent
    width: number
    height: number
    scale: number
  end

  -- A message that carries a `resize` event: its `resize` field alone.
  interface WithResize
    resize: ResizeEvent
  end

  -- A `window` event's data.
  record WindowEvent
    phase: WindowEventPhase
    id: integer
  end

  -- A message that carries a `window` event: its `window` field alone.
  interface WithWindow
    window: WindowEvent
  end

  -- A `system` event's data.
  record SystemEvent
    appearance: SystemEventAppearance
    accent: integer
    motion: SystemEventMotion
    locale: string
    assistive: SystemEventAssistive
    contrast: SystemEventContrast
  end

  -- A message that carries a `system` event: its `system` field alone.
  interface WithSystem
    system: SystemEvent
  end

  -- A `modifiers` event's data.
  record ModifiersEvent
    shift: boolean
    ctrl: boolean
    alt: boolean
    super: boolean
  end

  -- A message that carries a `modifiers` event: its `modifiers` field alone.
  interface WithModifiers
    modifiers: ModifiersEvent
  end

  -- A `open` event's data.
  record OpenEvent
    paths: {string}
  end

  -- A message that carries a `open` event: its `open` field alone.
  interface WithOpen
    open: OpenEvent
  end

  -- What an `on_key` sink hears: one of these fields, by `ev.event`.
  interface WithKeySink is WithKey, WithText, WithPreedit, WithSelect
  end

  -- The messages the window's own events arrive as (ADR 0048): `window_events` on the root table.
  -- A key left out is an event the window does not hear.
  record WindowEvents
    -- The window changed size or scale. It arrives with `kui.WithResize`.
    on_resize: any
    -- The window gained or lost the keyboard: `phase` is `"focused"` or `"blurred"`. It arrives with `kui.WithWindow`.
    on_focus: any
    -- An OS setting changed: the appearance, the accent, reduced motion, the language, or assistive technology listening. It arrives with `kui.WithSystem`.
    on_system_settings_change: any
    -- The system's installed fonts changed; the message arrives as declared.
    on_fonts: any
    -- The physical modifier state changed. It arrives with `kui.WithModifiers`.
    on_modifiers: any
    -- The OS asked the app to open documents; read on the main window's root alone. It arrives with `kui.WithOpen`.
    on_open: any
  end

  -- A `windows` entry as a table: the window's name and config, and the messages its opening and
  -- closing (`kui.WithWindow`) and a popup's dismissal (`kui.WithDismiss`) arrive as (ADR 0048).
  record WindowEntry
    name: string
    kind: WindowEntryKind
    width: number
    height: number
    activates: boolean
    anchor: Rect
    on_window: any
    on_dismiss: any
  end

  record Point
    x: number
    y: number
  end

  record TextHit
    byte: integer
    line: integer
  end

  record TextMetrics
    width: number
    height: number
    lines: integer
  end

  record ScrollGeometry
    x: number
    y: number
    w: number
    h: number
    content_w: number
    content_h: number
    offset: Point
    max_offset: Point
  end

  record SelectionEnds
    anchor: RangeEnd
    focus: RangeEnd
  end

  record CellEnd
    line: integer
    col: integer
  end

  record CellSelection
    node: integer
    anchor: CellEnd
    focus: CellEnd
    block: boolean
  end

  record EnvAudio
    -- What the driver's output device is doing: `"closed"` (the default, and a headless driver's answer), `"opening"` (the ~90 ms open, on its own thread), `"open"`, or `"failed"` (it refused, and commands are dropped) — `KUI_AUDIO_DEVICE_*` in C, closed 0. A fact and not a verb: nothing lets a view close it, the driver does that itself once it has been idle a while. Worth reading because an open stream is a real-time thread whether or not anything plays, which is the whole of an idle app's CPU once a session has held a sound.
    device: string
    -- Playbacks started and not yet ended, plus any waiting on the device to open. Zero with the device still `"open"` is the idle stream the row above is about. A play that arrives while the device is `"opening"` counts here from the frame it was asked, until the open answers: if the device refuses, the play is refused on the next apply — its message with `sound: {phase: "refused"}` for a tagged one — and leaves the count with it, so what a machine with no output device shows is `opening`/1 then `failed`/0 with the refusal between (backlog F63).
    live: integer
  end

  record EnvSystem
    -- The OS light/dark setting: `"light"`, `"dark"`, or `"unknown"` when the host has no way to ask (`KUI_APPEARANCE_*` in C, where unknown is 0). Unknown is a real answer and the default — a view picks its own palette for it rather than being handed a guess. The core acts on it in one way: the theme is derived from it (ADR 0019), so the stock widgets and a `<text>` with no colour follow the setting — and nothing of the app's own repaints, because only the view knows which of its colours is the background.
    appearance: string
    -- The OS accent/highlight colour as `0xRRGGBBAA`, ready to pass straight back as a `bg` or `color`. "The host cannot tell" is `null` in Node, an absent key in Lua, and 0 in C — a fully transparent accent is not a colour anyone was given, the way a refresh rate of zero is not a rate. Node's `setEnv` also takes the `"#rrggbb"` spelling a prop takes.
    accent: integer
    -- The OS reduce-motion setting: `"reduced"` when the user asked for less animation, `"full"` when they did not, `"unknown"` when nobody asked the OS (`KUI_MOTION_*` in C, unknown 0). Spelled as what the user wants rather than as a `reduceMotion` boolean, because the third reading has no place in a boolean. Nothing a view declared is shortened for it — a view that honours it does so where it declares one; the one animation the core starts on its own, a wheel notch's glide (ADR 0050), jumps instead.
    motion: string
    -- The UI language as a BCP-47 tag (`"en"`, `"en-US"`, `"zh-Hant-HK"`), for whatever the view formats dates and numbers with; kui does not parse it. Carried inline (31 ASCII bytes, `Locale`) so the reading stays `Copy`, and anything that does not fit reads back as unknown: `null` in Node, an absent key in Lua, an empty `KuiStr` in C.
    locale: string
    -- Whether assistive technology is listening: `"listening"` once an accessibility client has asked this window for its tree, `"none"` while the bridge is up and nobody has, `"unknown"` where there is no bridge — a headless `Ctx`, a runner built without `accesskit`, a C host that never called the setter (`KUI_ASSISTIVE_*`, unknown 0). The reading that changes what a view *says* rather than what it draws: an alert that announces when something is listening and blinks when nothing is. Reported through the `system` event when it changes, like the other four. Two limits are the platform's, not kui's. *Any* client counts — a probe, an accessibility inspector, a test driving the AX API and VoiceOver alike all ask for the tree, and nothing tells them apart — so it says something is listening, not that a person is. And it falls back to `"none"` only where the adapter reports deactivation, which in the pinned AccessKit is AT-SPI alone (the session's accessibility bus going away); on macOS and Windows nothing reports a client leaving, so once it has risen it stays `"listening"` for the window's life.
    assistive: string
    -- Whether the user asked for more contrast (ADR 0069): `"more"` under macOS's Increase Contrast or a Windows high contrast theme, `"normal"` when the OS says not, `"unknown"` where nobody asked (`KUI_CONTRAST_*`, unknown 0). What a `when` entry's `contrast = "more"` reads, so a view that thickens its borders for it declares the entry and is not re-run. Reported through the `system` event when it changes, like the rest of `system`.
    contrast: string
  end

  record EnvWindow
    -- Which window this frame draws, assigned by the driver: 0 for the window the app starts in. Every event from it carries the same number.
    id: integer
    -- The host asked the app to draw its own chrome, so there is no native titlebar to sit under. `<titlebar>` and `<windowButtons>` build nothing when this is false.
    custom_chrome: boolean
    -- The window is maximized — what picks the restore glyph over the maximize one.
    maximized: boolean
    -- The window is fullscreen.
    fullscreen: boolean
    -- The window is above every other app's: the level the driver set after the frame asked for it (`alwaysOnTop` / `always_on_top` / `kui_set_always_on_top`, backlog C30), on a platform that has one. On Wayland winit has no call for it, so a driver there reports false however often the app asks — which is why a pin button draws its state from this and not from the app's own flag. It is the driver's record of what it set and not a query (winit has no level getter), so a level the OS dropped afterwards — a fullscreen space, a tiling manager — is not seen here. A C host reports it through its own setter rather than an argument on `kui_env_set_window`, the way `kui_env_set_assistive` is, so an older host that never applies a level has nothing to recompile.
    always_on_top: boolean
    -- Area (logical px, window coordinates) covered by controls the OS still draws over our content — the macOS traffic lights under custom chrome. Keep out of it. Node hands back the `Rect` the core holds (`{x, y, w, h}`, or `null` for none); Lua and C flatten it to a width and height anchored at the window origin (absent in Lua, `0` in C, for none), which is the shape C's two numbers can express and where the one real instance sits.
    controls_w: number
    -- Area (logical px, window coordinates) covered by controls the OS still draws over our content — the macOS traffic lights under custom chrome. Keep out of it. Node hands back the `Rect` the core holds (`{x, y, w, h}`, or `null` for none); Lua and C flatten it to a width and height anchored at the window origin (absent in Lua, `0` in C, for none), which is the shape C's two numbers can express and where the one real instance sits.
    controls_h: number
    -- What is behind the window's transparent pixels, as the driver got it (backlog F126): `"opaque"` (the default, and every headless core's), `"transparent"` (the desktop as it is), `"blur"` (a live blur of what is behind the window) or `"tinted"` (the desktop's colour, not live) — `KUI_BACKDROP_*` in C, opaque 0. The app asks with `Launcher::backdrop` and decides which regions show it by painting them with alpha; this is the answer, which is less where the platform has less — a blur asked of GNOME reads `"tinted"`, the wallpaper kui draws itself, or `"opaque"` where none could be read — so a view paints its translucent regions opaque when this says so. A C host reports it through its own setter, as `always_on_top` is; a C app asks with `KuiRunConfig.backdrop` and its view reads the answer with `kui_ctx_backdrop`.
    backdrop: string
  end

  -- `env.theme`: the palette the core derived from `system`, one `0xRRGGBBAA` per role.
  record Theme
    -- The window behind everything.
    bg: integer
    -- A card, panel or list sitting on `bg`.
    surface: integer
    -- A surface floating above content: a menu, a tooltip, a popover. Under a light theme it is no lighter than `surface` — a float on a white page separates by its border.
    raised: integer
    -- A well cut into a surface: a text field, a code block, a track.
    sunken: integer
    -- The hairline between two surfaces.
    border: integer
    -- A border that has to be seen — a float's edge, a focused field.
    border_strong: integer
    -- Body text, and what a `color`-less text run resolves to.
    fg: integer
    -- Secondary text: captions, hints, an accelerator beside a label.
    muted: integer
    -- Text that is barely there: a placeholder, a gutter number.
    faint: integer
    -- The one saturated colour: the OS accent where the host reports one, the app's where it pinned one, kui's blue otherwise. The `accent` prop paints from this.
    accent: integer
    -- `accent` under a pointer.
    accent_hover: integer
    -- `accent` under a press.
    accent_pressed: integer
    -- Black or white — whichever a reader can see on `accent`. What a button's label is.
    on_accent: integer
    -- The accent as a translucent wash rather than a fill: a selected menu row, a chosen tab, a highlighted list item. Keeps `fg` readable over it on both bases, which a fill does not.
    accent_soft: integer
    -- What a text selection is painted under, in an editor and over a `selectable` scope alike.
    selection: integer
    -- The default keyboard focus ring (ADR 0002).
    focus_ring: integer
    -- A translucent wash over a hovered neutral control. An overlay, not a fill, so one value works on every surface.
    hover: integer
    -- The same over a pressed one, and the firmer of the two on both bases.
    pressed: integer
    -- A good outcome. Readable on `surface` on both bases, which is why it is not one colour for both.
    success: integer
    -- Something that wants attention.
    warning: integer
    -- A destructive action or a failure. The close button's hover, too.
    danger: integer
    -- The scrollbar thumb at rest.
    scrollbar: integer
    -- The thumb while hovered or dragged.
    scrollbar_active: integer
    -- The base the palette came from.
    appearance: string
    -- A multiplier, not a colour: what a disabled control's opacity is.
    disabled_opacity: number
  end

  -- `env.metrics`: the sizes the stock widgets are built from, in logical px.
  record Metrics
    -- A stock control's label: the button's text size.
    control_text: number
    -- The chrome's text: a menu row, a menu-bar title, the titlebar's title.
    chrome_text: number
    -- A tooltip's text.
    hint_text: number
    -- The corner of every stock surface: a button, a field, a menu, a tooltip.
    radius: number
    -- The corner of a row inside one: a menu row, a menu-bar title.
    radius_inner: number
    -- A button's horizontal padding.
    control_pad_x: number
    -- A button's vertical padding.
    control_pad_y: number
    -- A text field's horizontal padding.
    field_pad_x: number
    -- A text field's vertical padding.
    field_pad_y: number
    -- A tooltip's horizontal padding.
    hint_pad_x: number
    -- A tooltip's vertical padding.
    hint_pad_y: number
    -- A menu row's horizontal padding, and a menu-bar title's.
    menu_pad_x: number
    -- A menu row's vertical padding; a menu-bar title's is two px less.
    menu_pad_y: number
    -- A menu panel's width.
    menu_width: number
    -- The drawn menu bar's height.
    menu_bar_h: number
    -- The titlebar strip's height: the platform's caption height, 32 on Windows and 34 elsewhere, or the window's own where the runner knows it (backlog W22) — 40 and 52 off macOS for a launcher's `medium` or `tall` titlebar under custom chrome, and under macOS custom chrome the OS's own titlebar as `window.native_controls` measures it (32, 40 or 52 on macOS 27). The stock number stands for that window's height, so a set of the app's keeps it unless it names a number of its own. Under macOS custom chrome the strip is drawn at the measured height whatever this row says (`widgets::titlebar_height`).
    titlebar_h: number
  end

  -- `env.tokens`: the named tokens in force, the script's own over the host's.
  record Tokens
    colors: {string:integer}
    lengths: {string:number}
    families: {string:string}
  end

  -- `view`'s first argument: the host's facts for this frame, and the calls that answer about it.
  record Env
    -- Display refresh rate in Hz. "The host cannot tell" is `null` in Node (a stable shape to destructure, typed `number | null`), an absent key in Lua, and a rate at or below zero in C.
    refresh_hz: number
    -- One vsync interval at `refresh_hz`, or at 120 Hz when the host cannot tell: the per-frame time budget, and what the latency HUD draws its line at. Derived, and in the reading anyway, so no view restates the fallback. A C host is the one that knows the rate and computes its own.
    frame_budget_ms: number
    -- Whether the *window* has the keyboard at all. Not the focused node — that is the `focus` row.
    focused: boolean
    -- The logical width of the current (or last) frame's viewport — the window less the devtools' dock while the panel is docked (`docs/adr/0024`), the same number a `resize` reports and Node's `KuiWindow.size()` answers — the other host fact a view wants at the same moment, so it rides in the same reading. Zero before the first frame, since the frame establishes it (backlog F43: this row once read the window instead, so an app under `KUI_DEVTOOLS` sized itself to a viewport it did not have). Node's `viewport` is the `WindowSize` shape `runWindowed` already uses.
    viewport_w: number
    -- Its logical height.
    viewport_h: number
    -- The focused *node*'s key, as events carry it (absent for none). A value the host wrote before the view ran, so it lags a same-frame verb by one frame; `env.is_focused(key)` is the live query. Node spells it as the call `focused()` on the context, and C as `kui_focused`, rather than a key on `env`.
    focus: integer
    -- Whether focus shows — the keyboard or assistive technology put it where it is, or acted on it there; a click alone does not. Node: `focusVisible()` on the context.
    focus_visible: boolean
    -- The caret's blink phase — `true` draws it (backlog C35). The driver's clock sets it while there is a caret to blink: a focused `edit`'s, or the `caret` a `line` under a focused `onKey` sink declares; a custom editor draws its caret node on the on phase and skips it on the off, keeping the `caret` row on its `line` either way, so it blinks in step with the stock editor and, in a window without the keyboard, not at all. Always `true` headless. Node: `caretVisible()` on the context (`setCaretVisible` is the driver's half, for a test that drives the phase).
    caret_visible: boolean
    -- The frame clock in seconds (backlog F134): the driver's monotonic clock, any origin, the one `transition` and `keyframes` read this frame — 0 before a driver sets one. Read a deadline off it (a toast's expiry, a sequence's beats) rather than off a clock of the app's own, so a test that moves the frame clock moves both. Node: `now()` on the context.
    now: number
    -- The focus region in effect — the key of the `focusRegion` node whose ring Tab walks (absent for the main ring; `docs/adr/0022-focus-regions.md`). What a chord that toggles between a dock and the app reads to know which way it is going. Node spells it as the call `region()` on the context, and C as `kui_region`.
    region: integer
    -- The window's "Tab moves focus" toggle (backlog C99): while it is on, a multiline `edit` whose Tab would type indentation walks the focus ring instead, as one declared `tabMovesFocus` does. Ctrl+M flips it in such an editor (Ctrl+Shift+M on macOS, as Monaco binds it); what an app reads to show a status label. Node: `tabMovesFocus()` on the context.
    tab_moves_focus: boolean
    audio: EnvAudio
    system: EnvSystem
    window: EnvWindow
    theme: Theme
    metrics: Metrics
    tokens: Tokens
    -- Says something once with no node behind it (ADR 0001).
    announce: function(text: string, politeness?: string)
    -- Moves focus to a node now; an app's move stands over a modal's restore (backlog AR17). `keyFocus` is the declarative, edge-triggered form.
    set_focus: function(key: KeyRef)
    -- Drops focus.
    blur: function()
    -- Steps the Tab ring forward (ADR 0002).
    focus_next: function()
    -- Steps the Tab ring backward.
    focus_prev: function()
    -- Enters a `focusRegion`'s ring, or leaves it for the main one (ADR 0022).
    focus_region: function(key: KeyRef)
    -- Whether a node has focus.
    is_focused: function(key: KeyRef): boolean
    -- The app's answer to a `typeahead` event (ADR 0070): the data index of the row its search names, which the core reaches for in the composite that asked — landing on it, or scrolling to it and landing the frame that builds it. False when that composite asked nothing, or focus has left it.
    answer_typeahead: function(key: KeyRef, index: integer): boolean
    -- The label a key was opened under, in the frame being built and then the last one; none for an auto-keyed node. The reverse of `key_of`.
    label_of: function(key: KeyRef): string
    -- Whether the pointer is over a node.
    is_hovered: function(key: KeyRef): boolean
    -- Whether a press started on a node and the pointer is still over it.
    is_pressed: function(key: KeyRef): boolean
    -- Whether files dragged in from the OS are over a node (ADR 0031) — for drop-dependent layout; the colour is `drop_bg`.
    is_drop_target: function(key: KeyRef): boolean
    -- The drop zone the dragged files are over, if any — what a driver answers the OS with, and what a test reads to say a zone was found (ADR 0031, decision 5).
    drop_target: function(): integer
    -- Whether any member of a hover group is hovered: Rust takes the id (`NodeSpec::hover_group_id`), every binding the `hoverGroup` name. `hoverBg` on a member paints it without asking.
    is_group_hovered: function(group: string): boolean
    -- Whether any member of a hover group holds a press (backlog C92), by id in Rust and by name elsewhere: what a widget drawn from several pieces styles its pressed state from; `pressedBg` on a member paints it without asking.
    is_group_pressed: function(group: string): boolean
    -- Where layout put a node last frame (backlog C26).
    layout_of: function(key: KeyRef): Rect
    -- A scrolling node's offset.
    scroll_offset: function(key: KeyRef): Point
    -- A scrolling node's viewport and content sizes.
    scroll_geometry: function(key: KeyRef): ScrollGeometry
    -- Scrolls a node to an offset.
    set_scroll: function(key: KeyRef, x: number, y: number)
    -- Moves a node's scroll by content that moved under it, with no ease: a variable-height list's anchor (backlog C46).
    shift_scroll: function(key: KeyRef, drawn: number, target: number)
    -- Scrolls a node for a drag the app holds with the pointer in the edge zone inside a rect, by the edge scroll's feel — on an axis it cannot move, the container around it, as the wheel chains, unless one says `overscroll: contain`; returns what moved and asks for the next frame while it moves (ADR 0029).
    edge_scroll: function(key: KeyRef, x: number, y: number, rect: Rect): Point
    -- Scrolls whatever encloses a node until it is in view.
    reveal: function(key: KeyRef)
    -- The byte and line under a point in a node's text (backlog C18).
    text_hit: function(key: KeyRef, x: number, y: number): TextHit
    -- The caret rect for a byte offset in a node's text.
    caret_rect: function(key: KeyRef, byte: number): Rect
    -- Shapes text in a style at a width and answers its size and line count.
    measure_text: function(s: string | number | {any:any}, opts?: Props, max_w?: number): TextMetrics
    -- Where a byte lands in a text wrapped at a width, before any node drew it: a caret on the frame its text is new (backlog F171).
    caret_in_text: function(s: string | number | {any:any}, opts: Props, max_w: number, byte: number): Rect
    -- An editor's text, by key or by label.
    edit_text: function(key: KeyRef): string
    -- Replaces an editor's text, caret at the end.
    set_edit_text: function(key: KeyRef, text: string)
    -- The window's selected text — a scope's, a grid's or the focused editor's.
    selection_text: function(): string
    -- The same with the formatting the text declared.
    selection_html: function(): string
    -- A text selection's anchor and focus as row indices and bytes (ADR 0029).
    selection_ends: function(): SelectionEnds
    -- A `cells` grid's selection: its ends as absolute lines and columns, and whether it is a block (ADR 0017 §4).
    cell_selection: function(): CellSelection
    -- Select All, scoped to a `selectable` node or a grid.
    select_all_in: function(key: KeyRef): boolean
    -- Drops the window's selection.
    clear_selection: function(): boolean
    -- Asks for the selection as a copy, which may come back as a `selectionrange` question.
    request_copy: function(): string, boolean
    -- The app's answer to that question.
    answer_selection_range: function(text: string): boolean
    -- A key sink's own Ctrl-C: posts a clipboard action for the host (backlog C33).
    set_clipboard: function(text: string, html?: string)
    -- Posts a secret for the clipboard, which the host writes marked concealed and transient the way a password manager does, so no clipboard manager shows or keeps it (backlog F84).
    set_clipboard_secret: function(text: string)
    -- A key sink's own Ctrl-V: the clipboard comes back as a commit, marked `concealed` / `transient` when the pasteboard said so (backlog F84). One ask at a time — a second while one is unanswered is dropped.
    request_paste: function()
    -- Whether a paste asked for is still unanswered (backlog AR34).
    awaiting_paste: function(): boolean
    -- Asks for the platform's Open, Save or folder dialog; the answer is a `files` event to whoever asked. One at a time — a second while one is out is dropped (backlog C51).
    request_files: function(opts?: any): boolean
    -- Whether a file dialog asked for is still unanswered.
    awaiting_files: function(): boolean
    -- Opens a context menu on a node at a point.
    open_menu: function(key: KeyRef, x: number, y: number, items: {any}): boolean
    -- Closes it.
    close_menu: function(): boolean
    -- Asks the driver to resize a window.
    set_window_size: function(window: integer, w: number, h: number)
    -- Asks the driver to bring a window to the front.
    focus_window: function(window: integer)
    -- Declares the origin's colour and length tokens (ADR 0027).
    set_tokens: function(decl: any)
    -- Loads a DTCG token file or resolver (ADR 0062): colours with two halves, lengths, families; Node and Lua answer the table, C adds it.
    load_tokens: function(path: string, opts?: {string:any}): {string:any}
    -- Runs a function inside a span of that name, on the timeline the Perf tab and a trace file show, and returns what it returned.
    span: function(name: string, f: function(): any...): any...
    -- A span timed elsewhere — a parse on another thread, a job — of so many ms, ending now, on the track named for it.
    perf_measure: function(name: string, ms: number)
    -- Adds to this frame's counter of that name.
    perf_count: function(name: string, n: number)
    -- Sets a reading — a footprint, a buffer's bytes — held until set again.
    perf_gauge: function(name: string, value: number)
    -- Puts an instant on the timeline: a file opened, a command run.
    perf_mark: function(name: string)
    -- Whether anything is recorded — the Perf tab recording or on show, `KUI_PERF_TRACE`, a `tracing` subscriber — so an app skips reading clocks of its own for the recording's doors.
    perf_measuring: function(): boolean
    -- Asks for a frame and names why: `owed_by` lists it under the name, and a recording's causes read `request: why`.
    request_frame_for: function(why: string)
    -- Loads a plugin under a namespace; a `KuiWindow` takes its list at construction (`extensions`).
    add_extension: function(namespace: string, path: string): boolean, string
    -- The namespaces loaded.
    extension_namespaces: function(): {string}
    -- Names the exit a node leaves by if this frame stops declaring it, over the one it declared: a throw a button aims.
    exit_with: function(key: KeyRef, exit: any)
    -- Asks for a frame at a time on the frame clock — a toast's expiry, a sequence's beat — with nothing owed until then.
    request_frame_at: function(at: number)
  end

  -- `view`'s second argument and a component's third: which instance is drawn.
  record Instance
    -- The component in the script's own vocabulary; `root` for `view`.
    name: string
    -- What the host loaded the script under.
    namespace: string
    -- `view`'s props, nil when the host passed none; a component gets them as its own argument.
    params: {string:any}
    -- The integer key events and `set_focus` use.
    key: integer
    -- The contribution slot it was placed in (`slot(name, props)`), nil for a placement by name.
    slot: string
  end
end

global row: function(t?: kui.Props): kui.Node

global column: function(t?: kui.Props): kui.Node

-- grid { row { text("name"), text("value") }, row { ... } }: a table
-- (docs/adr/0033) -- a column whose rows' children line up in columns,
-- each column as wide as its widest cell, so a label column sits at its
-- longest label with no width picked by hand. A cell's `width` sizes its
-- column (fit or a number is content, "grow" grows the column, a percent
-- takes its cut), a bare text is a cell held to its column, and the rows
-- are rows -- give them width = "grow" for the columns to grow into, and
-- their own gap, pad, bg, hover_bg, on_click. Every other key is the
-- column's. Named `grid` here because `table` is Lua's own.
global grid: function(t?: kui.Props): kui.Node

-- text("plain", opts) or text({ "plain ", { "bold", bold = true, color = 0x.. } }, opts)
-- The options are copied, not written into: a style table a script hoists
-- (`local mono = { size = 14 }`) and hands to several texts is several
-- nodes, not one table that ends up holding the last string.
global text: function(s: string | number | {any:any}, opts?: kui.text): kui.Node

global button: function(t: kui.button): kui.Node

-- The stock toggles (docs/adr/0034): checkbox { label = "Mute", checked =
-- muted, on_click = "mute" }, radio { ... }, switch { ... }. The state is
-- the model's: a press posts on_click and the view flips it. A checkbox
-- also takes mixed = true, the select-all box over a partial selection.
-- `label` is the name and the text both unless `text` says otherwise.
global checkbox: function(t: kui.checkbox): kui.Node

global radio: function(t: kui.radio): kui.Node

global switch: function(t: kui.switch): kui.Node

-- radio_group { label = "Theme", radio { ... }, radio { ... } }: one Tab
-- stop whose arrows move the choice and press the radio they land on.
-- Takes every box row; a column by default.
global radio_group: function(t: kui.Props): kui.Node

-- slider { label = "Volume", value_now = v, value_min = 0, value_max = 100,
-- value_step = 5, on_change = { kind = "vol" } }: the core turns the pointer
-- and the keys into that message with `change = { value, phase }` in it
-- (ev.event == "change"); store `ev.change.value` and draw the slider at it.
-- Without on_change the slider does not move.
global slider: function(t: kui.Props): kui.Node

-- Single-line input with chrome: input { label = "name", value = name,
-- on_change = { kind = "name" }, on_submit = { kind = "go" }, on_cancel = ... }.
-- `value` is the text, declared every frame (or `initial`, a seed for a new
-- field); a change arrives as the on_change message with `change = { text }`
-- in it, Enter and Escape as on_submit and on_cancel as declared. A handler
-- left out still reports, as the field alone on its key.
global input: function(t: kui.edit): kui.Node

-- The stock select: dropdown { label = "language", options = { "English",
-- "Deutsch", { label = "Latin", id = "la", enabled = false } }, current = 2 }.
-- A field showing the choice in force that, clicked, opens the core's own
-- menu of the options under it; the choice arrives as the `on_menu` message
-- with `menu = { role, item }` in it (ev.event == "menu"), `item` the
-- option's label or id -- or, with no on_menu, as `{ menu = ... }` on the
-- field's key. `current` counts from 1, as a Lua list does; nil for none.
-- Named `dropdown` here because `select` is Lua's own.
global dropdown: function(t: kui.select): kui.Node

-- Bare editor: edit { key = "note", value = note, on_change = { kind = "note" },
-- multiline = true, autofocus = true, size = 14, ...spec props }. An edit
-- arrives as the on_change message with `change = { text }` in it, the whole
-- text after it; store it and declare it back as `value`, which the editor
-- is brought to wherever they differ. `initial` instead seeds a new editor
-- that then keeps its own text (env.edit_text(key) reads it). on_submit,
-- on_cancel and on_boundary report Enter, Escape and an edge; an editor
-- that declares none of them hears nothing.
global edit: function(t: kui.edit): kui.Node

-- image { id = handle, width = 16, sampling = "nearest", fit = "contain" }:
-- a registered image (the host registers it and passes the handle in as a
-- plain integer). `sampling` is "linear" (default) or "nearest"; `fit` is
-- "fill" (default), "contain" or "cover" -- how the pixels meet the box,
-- which is itself the same in every mode (docs/adr/0025).
global image: function(t: kui.image): kui.Node

-- fragment { id = handle, params = {0.1, 0.2, ...}, animate = true, ... }: a
-- box the registered WGSL `id` paints (the host registers it with
-- Core::add_fragment and passes the handle in as a plain integer). An
-- ordinary node otherwise -- it lays out, rounds, clips, fades, takes input
-- and may hold children, which paint over it -- but it has no intrinsic
-- size, so give it a width and height or `fill`. `params` is up to sixteen
-- numbers the shader reads as four vec4<f32>; `animate` asks for a frame
-- every frame, which a fragment reading `time` needs.
global fragment: function(t: kui.fragment): kui.Node

-- line { from = {x, y}, to = {x, y}, width = 2, color = 0x7f9cf5ff } or
-- line { points = {{x, y}, ...}, curve = true, ... }: a round-capped stroke
-- in the parent's box space, never in layout (it floats, sized to its own
-- bounding box). `width` is the stroke width, `color` the stroke colour;
-- `key`, `transition`, `opacity`, `on_layout` apply, and `on_click`,
-- `on_drag`, `on_hover`, `hoverable` hit by the stroke (docs/adr/0026).
-- `dash = {6, 4}` cuts it into 6 px marks with 4 px gaps, as seen (one
-- length is both, four are a dash-dot); a mark no longer than `width` is a
-- dot, and a gap the dots overlap closes; `dash_offset` starts that far
-- into the pattern.
global line: function(t: kui.line): kui.Node

-- polygon { points = {{x, y}, ...}, bg = 0xd8863bff }: a filled polygon
-- through up to eight points in the parent's box space, the fill in `bg`,
-- placed like a line (never in layout; floats, sized to its own bounding
-- box). The outline may be concave. `key`, `transition`, `opacity`,
-- `on_layout` apply, and `on_click`, `on_drag`, `on_hover`, `hoverable`
-- hit by the outline (docs/adr/0025 decision 6, docs/adr/0026).
global polygon: function(t: kui.polygon): kui.Node

-- path { d = "M60 60 L100 60 A40 40 0 0 1 60 100 Z", bg = 0xd8863bff,
-- fill_rule = "nonzero", width = 2, color = 0xffffffff }: any outline as SVG
-- path data (or `ops`, the flat op form), filled with `bg` by `fill_rule`
-- and stroked `width` wide in `color` when `width` is given, placed like a
-- line. Hit by its outline under the rule (docs/adr/0040). `rotate = 0.25`
-- turns it a quarter turn clockwise about `pivot = {x, y}` (the centre of
-- its box without one): the quad turns, the mask is drawn once (docs/adr/0041).
-- `dash` and `dash_offset` cut the stroke as a line's do.
global path: function(t: kui.path): kui.Node

-- component("kit/card", { title = "Deploys", key = "deploys", on_close = { close = true } },
-- { text("body"), footer = { button { label = "OK" } } }): a component an
-- extension provides (docs/adr/0060), placed here and keyed by where it is
-- (or by `key`); its events arrive as the messages in its `on_*` props.
-- The third argument's list is the host children the component puts where
-- it calls `children()`, a named group where it calls `children("footer")`.
-- An instance whose props did not change is replayed; `fresh = true` asks
-- every frame.
global component: function(name: any, props: any, kids: any): kui.Node

-- slot("app.toolbar", { on_run = { kind = "run" } }): a contribution slot
-- (docs/adr/0067) — every component the loaded extensions contribute to
-- the name (their `contributes`) placed here as siblings, in load order
-- and then `order`, each with these props; their events arrive as these
-- messages with `at.from` naming who raised them. An empty slot draws
-- nothing and says nothing.
global slot: function(name: any, props: any): kui.Node

-- lazy { needs = { w = 320 }, render = function() return chart() end,
-- fallback = function() return text("too narrow") end }: a branch built
-- only when it can be shown (docs/adr/0069) -- `render` when the parent
-- offers the room `needs` names (`w`, `h` or both), laid out at exactly
-- that size, or while `query` (`{ width = { below = 700 } }`, the
-- parent's size, the window's or the environment's) holds; `fallback`
-- otherwise, and neither before the first layout. A bare child table with
-- a `render` function is the same thing; this names it for Teal.
global lazy: function(t: kui.Lazy): kui.Node

-- In a component's function (`components.card = function(env, props,
-- inst) … end`): where the host's children of `group` go, the unnamed group
-- without one.
global children: function(group: any): kui.Node

-- In `on_event`: raise the component's event `prop` ("on_close", or
-- "onClose" — either spelling finds the host's) with `fields`, which the
-- host hears as the message it passed for that prop.
global emit: function(prop: any, fields: any): kui.Node

-- svg { data = "<svg …>", label = "Search", color = "$accent" } or
-- svg { src = "icons/search.svg", width = 16 }: an SVG document in a box
-- (docs/adr/0063). `mode` is "auto" (paths when it can, an image when it
-- cannot), "paths" or "image"; `fit` is "contain", "cover" or "fill";
-- `color` is what `currentColor` paints. A relative `src` is the
-- script's folder's.
global svg: function(t: kui.svg): kui.Node

-- cells { rows=, cols=, lines={"row text", ...}, runs={{row, col, len, fg,
-- bg, flags}, ...}, cursor_at={row, col}, cursor_shape="block", cursor_color=,
-- size=, family= }: a terminal's screen as one node (backlog C20). Rows and
-- columns are 0-based in runs and the cursor; a run's fg or bg of 0 keeps
-- the default.
global cells: function(t: kui.cells): kui.Node

-- audio { src = id, loop = true, volume = 0.5, paused = false, tag = {...},
-- key = "music" }: a playback retained by key while the view declares it
-- (present = playing, gone = stopped; volume/paused apply live). Draws
-- nothing. The host registers the sound and hands the id to the script.
-- `tag` is the playback's message: its end arrives as that message with
-- `sound = { phase, playback }` in it (ev.event == "sound").
global audio: function(t: kui.audio): kui.Node

-- titlebar { title = "app" } draws the standard title; with children it
-- hosts custom content between the drag inset and the window buttons.
global titlebar: function(t?: kui.titlebar): kui.Node

global window_buttons: function(): kui.Node

-- menu_bar { menu = { { label = "File", items = { ... } } } }: the
-- application menu (docs/adr/0018-a-menu-bar-the-app-declares.md). `menu`
-- is what it is; where this sits is where its titles go when they have to
-- be drawn in the window. Nothing is drawn where the platform owns the bar
-- (macOS), so a view writes it once and is portable. A choice from it
-- arrives as `on_menu = msg`, with `menu = { role, item }` in it (ADR
-- 0048); a bar with no `on_menu` posts nothing.
global menu_bar: function(t?: kui.menuBar): kui.Node

-- tooltip("hint") or tooltip { children }: a float hanging below the parent.
-- For the common hover-gated case use the `tooltip = "hint"` prop on a
-- row/column instead; the node form always draws.
global tooltip: function(s: string | number | {any:any}, t?: kui.tooltip): kui.Node

global latency_graph: function(): kui.Node

-- latency_hud { at = { "end", "end" } }
global latency_hud: function(t?: kui.latencyGraph): kui.Node

-- devtools_tab { name = "syntax", label = "Tree-sitter", component = "ts/panel",
--   props = { on_pick = { kind = "picked" } } }
-- devtools_tab { name = "syntax", label = "Tree-sitter", view = function()
--   return column { text("identifier 12:4") }
-- end }
-- 
-- A tab in the core's devtools panel (ADR 0032), beside facts, events,
-- tree and perf. The first form names a component a plugin provides: the
-- panel places it in the tab's body while the tab is on show, with `props`
-- and the panel's facts (ADR 0067); its events arrive as the messages in
-- `props`. The second is the script's
-- own content: `view` is called only while the tab is on show -- so a tab
-- nobody looks at costs its declaration and nothing else -- and the tree
-- it returns is drawn over the tab's body, the script's to hear as any of
-- its nodes. `view` takes no arguments; close over the `env` the script's
-- view received. Not a node: declare it anywhere in the tree, every frame.
global devtools_tab: function(t: kui.Props): kui.Node

-- uniform_list(env, { key = "log", rows = 10000, row_h = 28, ... }, function(i)
--   return column { fill = true, on_click = { kind = "pick", row = i }, text("line " .. i) }
-- end)
-- 
-- A scrolling column of `rows` uniform rows that declares only the visible
-- ones, plus the two spacers that hold the height of the rest -- so the
-- frame costs a screenful however long the list is. `env.scroll_geometry`
-- is what it slices by, so the container needs the `key` this reads it back
-- under. Every other key of `opts` is the container's own; `scroll_y` and
-- `gap` are the widget's (put a row's spacing inside `row_h`).
-- 
-- `row(i)` returns row i's *contents*: the widget owns the row's own node,
-- `row_h` tall and keyed by the row's data `index`, so a row keeps its
-- hover, focus, edit buffer and tweens as the built range slides over it,
-- and a virtualised list and a full one agree on identity. A clickable row
-- puts its on_click on a `fill = true` child, as above -- or on the row's
-- own node: `row_props = function(i) return { on_click = ..., bg = ... } end`
-- in `opts` gives each row's node its props (backlog DX22), its `index`
-- and `height` staying the widget's.
-- 
-- The geometry is the previous frame's, so the first frame -- before any
-- layout has resolved the container -- slices by the viewport, and a resize
-- is one frame late and covered by `overscan` (two rows each side).
-- 
-- To reach a row that is not built, `reveal_row(env, key, i, row_h)`
-- before the list scrolls it to the middle when it does not show.
global uniform_list: function(env: kui.Env, opts: kui.UniformList, row: function(i: integer): kui.Node): kui.Node

-- reveal_row(env, "log", i, 28)
-- 
-- Scrolls the `uniform_list` keyed "log" so row i shows, when it does
-- not: to the middle, so a jump lands with rows on both sides (backlog
-- DX22, Rust's `widgets::reveal_row`). Call it before the list, so the
-- frame that scrolls builds the rows it scrolled to; `env.reveal` finds
-- nothing for a row the list has not built. True when it scrolled; the
-- first frame, before the list has laid out, scrolls nothing. Assumes the
-- rows start at the list's content top, as they do without top padding.
global reveal_row: function(env: kui.Env, key: string | integer, i: integer, row_h: number): boolean

-- How many whole rows of `row_h` the list keyed `key` shows as of the last
-- layout -- a page's stride; 0 before it has laid out.
global rows_in_view: function(env: kui.Env, key: string | integer, row_h: number): integer

-- splitter(env, { key = "divider", dir = "row", thickness = 4, on_drag = { kind = "split" } })
-- 
-- A divider between two panes that the pointer drags (backlog DX22, Rust's
-- `widgets::splitter`): `thickness` px across (4 unless said) and growing
-- along the rest of its parent, in the theme's border colour and its accent
-- while hovered or held, with the resize arrows, and `keep_focus`, so a
-- press on it leaves the keyboard with the editor beside it. `dir` is the
-- parent's: "row" (the default) for panes side by side, "column" for panes
-- stacked. The split is the handler's: a `drag` event carries `x`, `y` and
-- `parent`, and `(x - parent.x) / parent.w` is a row's new fraction.
global splitter: function(env: kui.Env, t: kui.Props): kui.Node

-- local heights = row_heights(#lines, 20)
-- list(env, { key = "chat", heights = heights },
--   function(i, width) return env.measure_text(lines[i + 1], { size = 14 }, width).height + 8 end,
--   function(i) return text { pad = 4, lines[i + 1] } end)
-- 
-- A scrolling column of rows of *different* heights that declares only the
-- visible ones -- `uniform_list` where no single stride describes the list.
-- `heights` is a `row_heights(rows, estimate)` the script makes once and
-- keeps: the core's own prefix sums over the rows measured so far, with the
-- mean of those standing in for every other row. `set_len` it when rows are
-- appended, `clear` it when they change under the same indices.
-- 
-- `measure(i, width)` returns row i's height at that content width, and is
-- called only for rows the frame is about to build that `heights` has no
-- number for; `env.measure_text(s, style, width)` is layout's own answer for
-- a text row. What it returns is the height the row gets: the widget fixes
-- each row's node to it. `row(i)` returns row i's contents, as
-- `uniform_list`'s does, keyed by the row's data `index` (0-based).
-- 
-- Measuring moves the estimate, and with it every row above the window, so
-- the widget puts the row the window starts in back where it was
-- (`env.shift_scroll`) -- and, mid-glide on a container with a
-- `transition`, the row the glide is going to. The arithmetic is the core's
-- (`widgets::list` in Rust); this is the loop around the script's callback.
-- 
-- `env.set_scroll(key, 0, heights:offset_of(i))` puts row i at the top.
global list: function(env: kui.Env, opts: kui.List, measure: function(i: integer, width: number): number, row: function(i: integer): kui.Node): kui.Node

-- `rows` rows, none measured, each at `estimate` logical px (20 when left out) until measured.
global row_heights: function(rows: number, estimate?: number): kui.RowHeights
-- A channel of `columns`: each key a column's name, each value its rows (ADR 0065).
global channel: function(columns: {string:{any}}): kui.Channel
