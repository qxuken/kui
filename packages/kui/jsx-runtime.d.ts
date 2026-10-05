// Types for kui's JSX runtime. Set in tsconfig:
//   "jsx": "react-jsx", "jsxImportSource": "@qxuken/kui"

/** Event message payloads: plain data, both directions (the Elm shape).
 *  This is the wire type — anything JSON-shaped crosses; an app narrows it
 *  to its own union with `KuiMsg` below. */
export type Msg = null | boolean | number | string | Msg[] | { [key: string]: Msg };

/** Declare the app's message type once, and the payload prop (`onClick`),
 *  the tag props (`onDrag`, `onHover`, `onKey`, `onLayout`) and `createApp`
 *  / `runWindowed` take it instead of "any plain data":
 *
 *  ```ts
 *  type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' };
 *
 *  declare module '@qxuken/kui/jsx-runtime' {
 *    interface KuiMsg { msg: CounterMsg }
 *  }
 *  ```
 *
 *  Register the messages you wrote — not `CounterMsg | CoreMsg`. `CoreMsg`
 *  (what the core sends by itself) is typed in terms of this registration:
 *  its `tag` fields carry your messages. Naming it here makes the alias
 *  refer to itself, and TypeScript reports a circular type. Keep the full
 *  union for `update` (`type Msg = CounterMsg | CoreMsg`); the loop types
 *  infer it from there.
 *
 *  A tag prop also takes `null`: `<box onKey={null} keyFocus>` is a key
 *  sink whose events carry no `tag`, so a sink that only needs the node key
 *  costs no inert member in the union.
 *
 *  A payload typo then fails where it is written rather than in `update`.
 *  Left un-augmented, payloads stay `Msg` and nothing changes. (One app per
 *  tsconfig is the shape kui already has: one window, one event loop.) */
export interface KuiMsg {}

/** The app's message type: whatever `KuiMsg` was augmented with, else `Msg`. */
export type AppMsg = KuiMsg extends { msg: infer M } ? M : Msg;

export interface KuiElement {
  type: string;
  key?: string;
  props: Record<string, unknown>;
  children: KuiNode[];
}

export type KuiNode =
  | KuiElement
  | KuiNode[]
  | string
  | number
  | boolean
  | null
  | undefined;

/** A reference to a colour token by name — `'$peach'` — as `defineTokens`
 *  types it (`docs/adr/0027-tokens-beside-the-theme.md`). The string is
 *  what rides; the brand is what tells a colour's reference from a
 *  length's at the type level. Any colour prop takes one. */
export type ColorToken = `$${string}` & { readonly __kuiToken?: 'color' };
/** A reference to a length token by name — `'$sideW'` — for any length
 *  prop: a size, a pad edge, a radius, a border width, a fixed width or
 *  height. Resolved to logical px by the core's table. A colour token in a
 *  length slot is a type error here; a length token in a colour slot is
 *  caught at encode time instead (`unknown-token`), since `ColorProp`
 *  admits any string and narrowing it would refuse every helper that
 *  returns one. */
export type LengthToken = `$${string}` & { readonly __kuiToken?: 'length' };
/** Logical px, or a length token. */
export type LengthProp = number | LengthToken;

/** A size expression (backlog F109), resolved against the parent's content
 *  box — the box a percentage takes its cut of: spelled as CSS spells it
 *  (`"clamp(400px, 80%, 1000px)"`, `"min(720px, 100%)"`, nested), or as
 *  data, which crosses to the addon as numbers and is never parsed:
 *  `{ clamp: [400, "80%", 1000] }`, `{ min: [...] }`, `{ max: [...] }`,
 *  `{ percent: 80 }`, `{ px: 12 }`. An expression with no percentage in
 *  it is a length. */
export type SizeExpr =
  | number
  | `${number}%`
  | `${number}px`
  | `min(${string})`
  | `max(${string})`
  | `clamp(${string})`
  | { percent: number }
  | { px: number }
  | { min: SizeExpr[] }
  | { max: SizeExpr[] }
  | { clamp: [SizeExpr, SizeExpr, SizeExpr] };

/** number = fixed logical px; "N%" of parent (`{ percent: N }` is the same
 *  number); grow soaks up leftover space; a size expression (`SizeExpr`);
 *  a length token is a fixed px the core's table resolves. */
export type SizingProp =
  | 'fit'
  | 'grow'
  | { grow: number }
  | SizeExpr
  | LengthToken;

/** A lower clamp: logical px, a size expression, or "fit" for the node's
 *  own fit size on that axis — what lets a `grow` child keep a content
 *  floor (a tab never narrower than its label). */
export type MinProp = 'fit' | SizeExpr | LengthToken;

/** An upper clamp: logical px or a size expression (`"90%"`, a clamp). */
export type MaxProp = SizeExpr | LengthToken;

/** 0xRRGGBBAA number, "#rgb" / "#rrggbb" / "#rrggbbaa", or a colour
 *  token's reference (`'$peach'`, a theme role's `'$surface'`). */
export type ColorProp = number | string | ColorToken;

export type AlignProp = 'start' | 'center' | 'end';

/** One CSS-style keyframe stop for `keyframes`. `at` is 0..1 and spreads
 *  evenly when omitted (a lone stop sits at 1 and animates from the node's
 *  own value); a slot a stop leaves out is left to its neighbours. Sizings
 *  animate their amount only, in the form the prop itself declares. */
export interface KeyframeProp {
  at?: number;
  width?: SizingProp;
  height?: SizingProp;
  bg?: ColorProp;
  radius?: number;
  /** Group opacity, 0..1. */
  opacity?: number;
}

/** Where a node starts the first frame it is seen, for `enter`: the slots
 *  it names ease in from these values over `transition` ms instead of
 *  snapping. `dx`/`dy` are logical px the node slides in from; the rest
 *  take the forms the props themselves take. A node that leaves and comes
 *  back enters again. */
export interface EnterProp {
  dx?: number;
  dy?: number;
  width?: SizingProp;
  height?: SizingProp;
  bg?: ColorProp;
  radius?: number;
  /** Group opacity, 0..1: `{ opacity: 0 }` fades the whole subtree in. */
  opacity?: number;
}

/** A gradient painted over a box's `bg`, under its border and children
 *  (docs/adr/0042-a-gradient-is-an-image-the-core-paints.md). Linear
 *  `to` a side or a corner (the default is `'bottom'`) or along `angle`,
 *  or `radial` from `at`. Defined on the box's unit square and stretched
 *  to it: a side or a corner is CSS's, and any other `angle` runs corner
 *  to corner at an eighth of a turn whatever the box's aspect, where
 *  CSS's `45deg` does not. Rasterized once per distinct gradient and
 *  drawn as one image quad; it does not tween. */
export interface GradientProp {
  /** The side or corner a linear gradient runs to. */
  to?: 'right' | 'bottom right' | 'bottom' | 'bottom left' | 'left' | 'top left' | 'top' | 'top right';
  /** A linear gradient's direction in turns, clockwise from east: 0.25
   *  runs downwards. */
  angle?: number;
  /** Out from `at` to the box's farthest corner instead of along a line. */
  radial?: boolean;
  /** A radial gradient's centre as fractions of the box; the middle,
   *  `[0.5, 0.5]`, without one. */
  at?: [number, number];
  /** Two or more colours, each alone or as `[colour, position]` with the
   *  position 0 to 1. Stops without one are spaced evenly between those
   *  with; two at one position are a hard edge. */
  stops: (ColorProp | [ColorProp, number])[];
}

export interface FloatProp {
  /** The preset to start from; every key below overrides one of its
   *  values and leaving one out keeps the preset's own, so
   *  `{ anchor: 'below', dx: 4 }` still hangs below with its 6px gap. */
  anchor?: 'parent' | 'viewport' | 'below' | 'above';
  /** Attach point on the anchor, [x, y]. */
  at?: [AlignProp, AlignProp];
  /** Attach point on the floating node itself, [x, y]. */
  self?: [AlignProp, AlignProp];
  dx?: number;
  dy?: number;
  /** Flip across the anchor / clamp to stay inside the viewport. */
  fit?: boolean;
  /** Take the parent's clip instead of escaping it: a node on a `clip`
   *  canvas panned past the canvas's edge is cut there and cannot be hit
   *  past it. Read with the `parent` anchor (and `below` / `above`, which
   *  anchor to the parent) only; still drawn as a layer over its in-flow
   *  siblings. A `line`, `polygon` or `path` in its parent's box is always
   *  clipped this way. */
  clip?: boolean;
}

// -- generated from the core's menu roles; edit MenuRole::ALL in crates/kui-core/src/menu.rs, then `npm run gen` --
/** What a menu row is: the app's own (`custom`), a divider, or one of
 *  the standard rows the core performs itself. The same spelling a
 *  `menu` message reports back. */
export type MenuItemRole =
  | 'custom' | 'separator' | 'cut' | 'copy' | 'paste' | 'selectAll' | 'lookUp';
// -- end generated --

/** One row to put in a context menu (`Ctx.openMenu`). Everything but
 *  `label` is optional, and a standard `role` takes its own wording when
 *  `label` is empty — so `{ role: 'copy' }` is the platform's Copy.
 *  `id` is what the row posts when chosen (its label, when absent). */
export interface MenuItemInput {
  label?: string;
  role?: MenuItemRole;
  enabled?: boolean;
  /** Draws a checkmark beside the row (and sets the platform's own check
   *  state where a host renders the menu): a setting the row *is*, not a
   *  command it runs. */
  checked?: boolean;
  id?: unknown;
  /** Display only: the shortcut is the app's or the platform's — except in
   *  a menu bar the platform draws, where a spelling kui can parse
   *  (`'mod+s'`, `'⌘S'`, `'Ctrl+Shift+P'`) becomes the real key equivalent.
   *  A declaration kui can parse is rewritten into the platform's own
   *  spelling, so `'mod+s'` reads as `⌘S` on macOS and `Ctrl+S` elsewhere. */
  accel?: string;
}

/** One menu of the application menu bar (the `<menuBar menu={…}/>`
 *  element's prop): a title and the rows that drop out of it
 *  (`docs/adr/0018-a-menu-bar-the-app-declares.md`). Its rows are the same
 *  `MenuItemInput` a context menu takes, so a standard `role` is performed
 *  by the core here too — an Edit menu's `{ role: 'copy' }` is the
 *  right-click Copy.
 *
 *  On macOS the first menu is the application menu, which the OS titles
 *  with the app's own name whatever `label` says. */
export interface MenuInput {
  label: string;
  items: MenuItemInput[];
  /** A disabled menu is dimmed and opens nothing. */
  enabled?: boolean;
}

/** The whole bar, in order — the `<menuBar menu={…}/>` element's prop.
 *  `[]` takes the menu away; a frame that draws no `<menuBar/>` at all
 *  leaves the last declaration in force. */
export type MenuBarInput = MenuInput[];

interface Keyed {
  /** Stable identity for retained state (scroll offsets, editors). */
  key?: string | number;
}

// -- generated from the addon's prop schema; edit schema.rs, then `npm run gen` --
export interface GeneratedSpecProps {
  /** Paint this node's background in the OS accent colour — `env.system.accent` — keeping the declared `bg` on a host that cannot tell what it is. The one prop whose paint depends on the environment, which is why it is opt-in: the same tree is a different colour on two machines, and that is the point here and a surprise anywhere else. On the stock button it does the whole job — the hover and pressed shades are derived from the accent, and the label goes black or white by its luminance, so a yellow accent is still readable — which is what `<button accent>` is for. */
  accent?: boolean;
  /** Scroll anchoring on a scrolling node (backlog C26, CSS's `overflow-anchor`): the first child in view keeps its place on screen when the content before it changes size — a chat that prepends history, a log that inserts rows above the viewport, a list whose row heights are corrected as they are measured — with no `setScroll` and no arithmetic in the view. The core remembers which child was first in view and where its edge was, and moves the offset by however far that edge moved in the next layout, before the offset is clamped; a wheel notch or a `setScroll` between the frames is kept and the correction added to it. The child is found by key, so give the rows stable keys (a `key` or an `index`); a child that is gone anchors nothing that frame. On the scroll axis that is the node's main axis only — `scrollY` on a column, `scrollX` on a row — and content appended *after* the anchor moves nothing, so a log that is tailing still asks for the end itself. */
  anchor?: boolean;
  /** Ask for another frame after this one, every frame this node is declared. What a `fragment` that reads `time` needs, and what anything driving itself off the clock rather than off input needs. Opt-in like `exit`, and for the same reason: it takes the loop off input-driven and onto the display's cadence for as long as it is declared, so a still node must not carry it. One node asking is enough for the whole window. */
  animate?: boolean;
  /** Width over height — `16/9`, `1` for a square — CSS's `aspect-ratio`. It sizes the axis left `fit`: a fit height is the final width over the ratio (so `width: grow` and a ratio is a box that keeps its shape as the window resizes), and a fit width under a fixed height is that height times it. With both axes declared, or a fit width under a `grow` or percent height, it has nothing it can set and warns. The derived axis is neither shrunk nor fitted to the children, which overflow it; `minHeight: 'fit'` floors it at them. On an image it wins over the pixels' own aspect. */
  aspectRatio?: LengthProp;
  /** Background fill. */
  bg?: ColorProp;
  /** How far a spring overshoots its target: 0 glides in with none, 0.5 bounces visibly, and values past 0.9 are held there (a spring at 1 would never settle). It replaces a spring `easing`'s own bounce (`smooth` 0, `snappy` 0.15, `spring` 0.25, `bouncy` 0.5), and on a timed easing makes the transition a spring — so `transition` plus `bounce` is a spring of that length and bounce. */
  bounce?: LengthProp;
  /** Which non-primary buttons `onButton` claims (backlog F105): `"secondary"`, `"middle"` and `"other"` (every button past those), separated by spaces or commas — `"middle"`, `"secondary middle"`. Unset, all three: a node that wants the middle button and leaves the secondary one to its context menu says `"middle"`. A word that is none of the three is skipped, so a string of none of them claims nothing, and a typo never takes the secondary button from a context menu. Meaningless without `onButton`. */
  buttons?: string;
  /** On a `line` of a custom editor (a `textInput` / `multilineTextInput` role drawn by the app): the caret's byte offset into that line's text. */
  caret?: LengthProp;
  /** On a `line` declaring `caret`: the caret is solid — a block caret in a modal editor's normal mode — so the driver's blink clock is not armed on it and `caretVisible` stays true, while the offset still anchors the IME and reads to assistive technology. Without it a declared `caret` is a caret to blink, and the one thing that asks an idle app for a frame twice a second; an editor whose caret only blinks while typing declares this on every other mode's line. */
  caretSolid?: boolean;
  /** Center children on both axes. */
  center?: boolean;
  /** The on state of a `checkbox` / `radio` / `switch` role. */
  checked?: boolean;
  /** A registered sound (addSound) played when the node is clicked; implies hover tracking. */
  clickSound?: string;
  /** Child alignment across the main axis. On a row, `baseline` lines up the first baselines of the children's text, so a label and a larger value read as one line; a child with no text aligns by its bottom edge, a `grow` or percent height fills the line from its top, and a fit-height row grows to hold the aligned children. A column lays `baseline` out as `start` (as CSS does), and the three spreads mean nothing across an axis — each with a warning. */
  crossAlign?: 'start' | 'center' | 'end' | 'spaceBetween' | 'spaceAround' | 'spaceEvenly' | 'baseline';
  /** Space between wrap lines, across the main axis (`gap` stays the space along it). */
  crossGap?: LengthProp;
  /** The pointer shape over this node. Unset, the pointer is `text` over an editor or a `selectable` scope and `default` over everything else — an `onClick`, `focusable` or `onDrag` node included, as a native button is — so a hand (`pointer`) over a control, a `grab` over a handle (and `grabbing` while its drag runs, which the view declares as its drag state changes), a splitter's `ewResize` / `nsResize` and a `disabled` control's `notAllowed` are all declared. The stock `button` declares `pointer` itself. A captured drag keeps the dragged node's shape wherever the pointer goes. */
  cursor?: 'default' | 'text' | 'pointer' | 'grab' | 'grabbing' | 'notAllowed' | 'ewResize' | 'nsResize' | 'nwseResize' | 'neswResize';
  /** Holds the `keyframes` cycle back by this many ms (CSS `animation-delay`); siblings with different delays run out of phase. */
  delay?: LengthProp;
  /** The accessible description: the extra sentence a reader says after the name, for what the name cannot say on its own — what a button will do, why a control is disabled, what format a field wants. `tooltip` is the shorthand that also draws the string and hover-tracks the node; this is the description alone, for a hint that is spoken and never drawn. Both write the one slot, so a node declaring both keeps whichever its binding applied last. It reads only on a node that reaches the access tree — a role, a label, a control — since a plain box is elided and takes its description with it. */
  description?: string;
  /** Inert: no click, drag or key sink, no hover / pressed / focus background, skipped by Tab, reported disabled to assistive technology; hover tracking stays so a `tooltip` can say why. */
  disabled?: boolean;
  /** Background while files dragged in from the OS are over this node (ADR 0031); wins over pressedBg, focusBg and hoverBg, clears when they leave, land or the drag is cancelled. Implies hover tracking, eases with `transition`. */
  dropBg?: ColorProp;
  /** Easing for `transition` (default easeOut). The springs — `smooth` (no overshoot), `snappy`, `spring` and `bouncy` (the most), each a `bounce` of its own — integrate with momentum, so a value retargeted mid-flight keeps moving the way it was; `transition` is then about how long one takes to get there. */
  easing?: 'easeOut' | 'linear' | 'easeIn' | 'easeInOut' | 'spring' | 'bouncy' | 'smooth' | 'snappy';
  /** Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in). */
  enter?: EnterProp;
  /** Where the node ends the frame after the view stops declaring it `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }` — an `enter` read the other way. It plays when the node itself is removed, its parent still declared; a node that goes because an ancestor went — a tab switched away, a panel closed around it — goes at once with it, unless that ancestor has an `exit` of its own, whose picture carries it (backlog DX19; React's `AnimatePresence` rule). With a `transition`, the departing subtree is copied out of the last frame that had it and replayed frozen, in its place (the pass it painted in, just under the node that painted after it — a panel under a HUD leaves under it) and inert (no clicks, no Tab stop, no access row) while those slots ease from where they were, then dropped; without one it vanishes at once as it always did. `width`/`height` resize the departing node's own box only — the subtree inside it is a picture and is not laid out again. Needs a stable key across frames. */
  exit?: EnterProp;
  /** A disclosure's state: what a node that shows and hides something (a twisty, an accordion header, a menu button) reads as. Unset, the node does not expand at all — which is why this names its state instead of being a flag. */
  expanded?: 'collapsed' | 'expanded';
  /** Background while the node holds keyboard-visible focus (moved there by Tab or assistive technology, not a click); replaces the default focus ring. Pressed wins over focus wins over hover; eases with `transition`. */
  focusBg?: ColorProp;
  /** Makes this node's subtree a focus region: a Tab ring of its own that the ring outside never enters and that never leaves — a devtools dock, an inspector beside the app (`docs/adr/0022-focus-regions.md`). Entered on purpose: `focusRegion(name)` (`Ui::focus_region`, `env.focus_region`, `kui_focus_region`) moves focus in — to the focus the region last held, else its `initialFocus`, else its first stop — and `focusRegion(null)` moves it back to the main ring the same way; a press inside the region, or an explicit focus on a node in it, enters it too. Tab then walks that ring alone, wrapping inside it; with nothing focused, Tab enters the ring of the region in effect (`region()`). A region that stops being declared hands focus back to what the main ring last held. Only the ring is scoped: keys still bubble through the boundary to the sink above (a region that wants its own keymap is an `onKey` sink), the pointer and assistive technology see a plain node, and a `modal` in effect is the ring wherever it sits. Nested regions are skipped by the outer ring the way the main ring skips them. */
  focusRegion?: boolean;
  /** Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already. */
  focusable?: boolean;
  /** Space between children along the main axis. */
  gap?: LengthProp;
  /** A gradient painted over the node's `bg` and under its border and its children (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`): `{ to: 'bottom', stops: [...] }` towards a side or a corner (`right`, `bottom left`, …; the default is `bottom`), `{ angle: 0.125, stops }` in turns clockwise from east, or `{ radial: true, at: [0.5, 0], stops }` out from a centre (fractions of the box, the middle by default) to its farthest corner. A stop is a colour — a `$token` too — or `[colour, position]` with the position 0 to 1; stops without one are spaced evenly between those with. Two stops at one position are a hard edge. The gradient is defined on the box's unit square and stretched to it, so a side or a corner is CSS's and any other `angle` runs corner to corner at an eighth of a turn whatever the box's aspect, where CSS's pixel-measured `45deg` does not. Stops mix in straight sRGB with the alpha premultiplied, as CSS's do. What it costs is one image quad: the core rasterizes each distinct gradient once into the glyph atlas — a 256-texel strip along an axis, a 128-texel square otherwise — keyed by the gradient and not the box, so a box that resizes and a thousand boxes that share one rasterize nothing, and a host that draws an image draws it. A hard edge is as soft as the raster stretched to the box (a 256th of its length along a strip); stripes are boxes. It does not tween — `transition` eases the `bg` under it and `opacity` fades it — and `hoverBg` and the other state backgrounds replace `bg`, not the gradient; one that changes every frame is a raster a frame, and a shimmer is a `fragment`'s. Ignored on a `line`, a `polygon` and a `path`. Fewer than two stops draw nothing. */
  gradient?: GradientProp;
  /** Vertical size: px | "fit" | "grow" | "N%" | a size expression (see `width`). */
  height?: SizingProp;
  /** Background while hovered (or while any node in its hoverGroup is); implies hover tracking, eases with `transition`. */
  hoverBg?: ColorProp;
  /** Nodes sharing a group name show hoverBg/pressedBg together (a split button, a multi-piece shape). */
  hoverGroup?: string;
  /** A registered sound (addSound) played when the pointer enters the node; implies hover tracking. */
  hoverSound?: string;
  /** Hover-track without a click payload (for isHovered-driven styling). */
  hoverable?: boolean;
  /** Where focus lands when the enclosing `modal` scope is entered: the first node in the modal's Tab ring declaring it, so a destructive confirm opens on its Cancel rather than on whichever control is declared first. Read on entry only — a Tab press afterwards stands, and the scope re-entered (a nested confirm closing) leaves focus where it was. Declared on nothing, or only on nodes the ring skips (disabled, `role="none"`, not focusable), entry stays the ring's first node. */
  initialFocus?: boolean;
  /** A press on this node, or anywhere inside it, leaves keyboard focus where it was: a toolbar button, a tab or a divider that acts without taking the keyboard from the editor or key sink that had it. Without it a press on an `onClick` node focuses the node, and the app's keys stop reaching the sink until it takes focus back. The press also leaves a text or cell selection and the Tab ring where they were, so a Copy button copies what was selected. An `<edit>` inside still takes its caret and focus, as the keyboard's own owner. The click, drag and hover are unchanged, and Tab and assistive technology still reach the node. */
  keepFocus?: boolean;
  /** With `onKey`: releases arrive too, as the same payload with phase:"up" (`text` null, `repeat` false) — for a held-key interaction (WASD, press-and-hold, a key that arms a mode while it is down). A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so nothing is left stuck down. Without it a sink hears presses only, which is what a keymap wants — one that heard both halves would run every binding twice. */
  keyUp?: boolean;
  /** CSS-style stops `[{ at?, width?, height?, bg?, radius?, opacity? }, …]`: the slots they name cycle through them over `transition` ms, forever, without the view redrawing; `at` is 0..1 and spreads evenly when omitted. */
  keyframes?: KeyframeProp[];
  /** The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image, an icon-only button and a `modal` dialog have none, and the core warns (`image-without-label`, `control-without-name`, `modal-without-name`). */
  label?: string;
  /** Marks this node a live region: when the text inside it changes, a screen reader reads the change without being asked — `polite` at the next pause, `assertive` interrupting. Put it on the smallest node that holds the message, since everything inside a live node is live. For a one-off with no node behind it ("Saved") the binding's `announce` verb is the other half. */
  live?: 'off' | 'polite' | 'assertive';
  /** Child alignment along the main axis. `start`, `center` and `end` put the children together; `spaceBetween` deals the free space out between them (none at the ends), `spaceAround` gives each child an equal share split to its two sides, and `spaceEvenly` makes every gap and both ends equal — CSS's `justify-content`. The spread is added to `gap`, and there is none when nothing is free: a `grow` child takes it all, and an overflowing run keeps its gaps. `baseline` means nothing here and lays out as `start`, with a warning. */
  mainAlign?: 'start' | 'center' | 'end' | 'spaceBetween' | 'spaceAround' | 'spaceEvenly' | 'baseline';
  /** Upper height clamp: logical px or a size expression (see `width`). */
  maxHeight?: MaxProp;
  /** Upper width clamp: logical px or a size expression (see `width`); grow+maxWidth is the responsive-width pattern. */
  maxWidth?: MaxProp;
  /** Lower height clamp: logical px, a size expression, or "fit" for the node's own fit height. Undeclared, it is the node's content where its column overflows — CSS's `min-height: auto`, none for a node that scrolls or clips — so a row keeps the height of its text; `0` asks for the squeeze back (see `minWidth`). */
  minHeight?: MinProp;
  /** Lower width clamp: logical px, a size expression (see `width`; a percentage clamp is none until the parent's width is known, as in CSS), or "fit" for the node's own fit width. "fit" under `width="grow"` is a content floor — CSS's `flex: 1 0 auto` — which is what an i3-style tab bar is: tabs that split the bar evenly while they fit and sit at their label's width, scrolling, once they do not. Opt-in, because a fit width is the unwrapped one: a paragraph in a grow column would stop wrapping under it. Left out, a child giving in an overflowing row that holds a percentage or a size expression stops at its content, CSS's `min-width: auto`; `0` lets it go below, CSS's `min-width: 0` (backlog RG92). A fit node across a column is no wider than the column's box — CSS's `fit-content` — down to this floor, 0 left out, unless the column scrolls x, so a text one wrapper deep in a capped card wraps there; "fit" keeps its content's width and runs past (backlog F116). */
  minWidth?: MinProp;
  /** A `checkbox` that is neither on nor off — the select-all box over a list some of whose rows are selected (ADR 0034). Read as mixed by assistive technology whatever `checked` says, and drawn as a dash by the stock `<checkbox>`. Meaningful on the checkbox role alone. */
  mixed?: boolean;
  /** Modal surface: the Tab ring becomes this node's subtree, everything outside it is inert to the pointer, the wheel and assistive technology, and Escape or a press outside emits {kind:"dismiss", reason:"escape"|"outside", tag} on it — the app stops declaring the node. The last one declared in tree order is the one in effect (a confirm inside a dialog); a modal that must cover the app is a float. The access tree is not pruned to the modal: it keeps every node of the frame and marks the one in effect `modal` (`docs/adr/0003-modal-surfaces.md`, decision 7), which is what assistive technology acts on. */
  modal?: AppMsg | null;
  /** With `onKey`: the modifier and lock keys arrive as keys of their own (backlog F108) — `code` "shift", "ctrl", "alt", "super", "capslock", "numlock", "scrolllock", which side in `location` ("left" / "right"), releases too with `keyUp`. Without it a modifier is only ever held — the next key's `shift`, `ctrl`, … and the `modifiers` event — so a keymap mid-sequence never reads a Shift as a key between two others. For a terminal speaking kitty's keyboard protocol, or a game that binds a lone Shift. */
  modifierKeys?: boolean;
  /** Button tag (backlog F105): a press of a non-primary button — middle, secondary, or one past those — emits {kind:"button", phase:"press", button, x, y, clicks, tag} on the node, and the button is then captured by it: every pointer move while it is held arrives as phase:"move" and its release as phase:"release", on this node wherever the pointer is. `button` is `"secondary"`, `"middle"` or a further button's number (3 and up); `x`/`y` are logical viewport coordinates, and on a `cells` grid each event carries `cell: {row, col}` as a click does. Several buttons can be held at once, each its own capture, and a primary drag is untouched. `buttons` says which buttons it claims — all of them unless it narrows them. Asked of the topmost node under the pointer, and when that node claims no such button the press reaches the nearest enclosing node that does, the way a context menu's does: a disabled node's own is skipped and the walk stops at the modal boundary. A claimed secondary press is this event *instead of* a `contextmenu` event and the stock menu (a nearer `onContextMenu` still wins, being the nested declaration). Like every non-primary press it moves no focus, places no caret and touches no selection or scrollbar. For a terminal's middle-click paste, and the mouse reports a program in it asked for. */
  onButton?: AppMsg | null;
  /** A `slider` role's changes (ADR 0034): the core turns a press on the node into the value under the pointer, a drag into the value under it, the arrows and assistive technology's increment / decrement into one `valueStep`, PageUp / PageDown into ten, Home / End into the range's ends — clamped to `valueMin`..`valueMax` (0..100 unset) and snapped to the step — and emits `{kind:"change", value, phase, tag}`: `phase` is `"move"` while the pointer holds the slider and `"end"` when it lets go or a key moved it. The value is proposed and never applied; the slider moves when the view declares it as `valueNow`. A key that lands where the slider already is proposes nothing. The pointer reads the node's content box along its main axis, so a `dir="column"` slider runs bottom to top. Without it a slider's arrows reach the app as `{kind:"access", action}`. Ignored on any other role. */
  onChange?: AppMsg | null;
  /** Message emitted when clicked (data, not a callback). */
  onClick?: AppMsg;
  /** Context-menu tag: a secondary-button (right) press emits {kind:"contextmenu", x, y, tag} on the node, at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it. Asked of the topmost node under the pointer, and when that node offers no menu the press reaches the nearest enclosing node that does — a container declaring a menu for everything inside it is the common case — the way an unclaimed key reaches the enclosing sink (`docs/adr/0011`): the event carries the *owner's* key and tag, a nested declaration wins over its ancestor's, a disabled node's own is skipped, and the walk stops at the modal boundary. */
  onContextMenu?: AppMsg | null;
  /** Drag tag: emits {kind:"drag", phase, x, y, dx, dy, parent, tag} events, `dx`/`dy` measured from the press point in every phase. On a `cells` grid the events also carry `cell: {row, col}`; inside an `onKey` sink that draws `role="line"` rows they carry `line`, `byte` and `clicks` — see the events table. */
  onDrag?: AppMsg | null;
  /** Drop-zone tag (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`): files dragged in from the OS over this node emit {kind:"drop", phase:"enter"|"move"|"leave"|"drop", paths, x, y, tag} — `paths` the OS paths as strings, `x`/`y` the pointer in logical viewport coordinates (absent on `leave`). The zone under the files is the topmost zone by paint order: a node inside a zone is the zone's (a button in it, a field in it), and a node that is no zone and has none enclosing it is looked past, so an overlay shown on `enter` cannot make the zone lose the files. No `leave` follows a `drop`; a drop off every zone is refused by the driver. Implies hover tracking. No access row — a screen-reader user's way in is a button beside the zone. On Windows and Linux the position is the OS cursor at enter and release only, so `move` never fires there. */
  onDrop?: AppMsg | null;
  /** Keyboard focus entering or leaving this node's subtree — the node itself, or anything focused inside it — emits `{kind:"focus", phase:"in"|"out", by, tag}` (backlog DX18). `by` is what moved it: `pointer` (a press), `keyboard` (Tab, a key a control answered), `assistive` (a screen reader's request) or `program` (the view or the app — `keyFocus`, `setFocus`, a modal's entry). Reported once the move settles, after the input that made it or at the end of the frame that declared it, so an app hears a pane taking the keyboard instead of diffing the focused key every frame. Leaving is reported innermost first, entering outermost first. It makes nothing focusable or interactive. */
  onFocus?: AppMsg | null;
  /** Force-click tag: a press that deepens past the second stage of a Force Touch trackpad emits {kind:"forceclick", x, y, tag} on the node, at the logical viewport point it happened at (`docs/adr/0017-selection-as-a-scope.md`). Routed as a secondary press is — no focus moved, no caret placed, no click — but asked of the topmost node only, with no walk to an enclosing declaration — and the ordinary click the press is still producing arrives afterwards, as it does on macOS. Text needs none of this: a force click over an `edit` or a `selectable` scope selects the word under it and asks the host for its Look Up panel. macOS-only in practice, and there the user can switch the gesture off, so nothing may declare itself the only way to reach something. */
  onForceClick?: AppMsg | null;
  /** Hover tag: the pointer entering/leaving emits {kind:"hover", phase:"enter"|"leave", tag} events. */
  onHover?: AppMsg | null;
  /** Key-sink tag: with key focus held, presses arrive as {kind:"key", phase:"down", code, ...} events. Releases only with `keyUp` beside it. */
  onKey?: AppMsg | null;
  /** Layout tag: the node's laid-out rect arrives as {kind:"layout", x, y, w, h, parent, tag} on its first frame and whenever it changes (needs a stable key). */
  onLayout?: AppMsg | null;
  /** Scroll tag: the wheel over this node emits {kind:"scroll", x, y, dx, dy, lines, tag} on it instead of scrolling anything — `dx`/`dy` the delta in logical px as the driver reported it (positive `dy` is the wheel rolling up, toward earlier content), `x`/`y` the pointer, and `lines` on a `cells` grid the whole lines the delta covers (positive = later history, the sign `originLine` grows in; the fraction is carried to the next notch so a trackpad's small steps add up) and null on any other node. The node takes the wheel on the axes `scrollAxes` names (both unless it narrows them): a gesture that starts over it is its own whether or not it has anywhere to go — except on an axis the node also scrolls (`scrollX`/`scrollY`, its offset the app's to set), where it is answered by its room as a container is, so at its edge a gesture that way passes to the scroller around it (backlog F118) — and stays its own until it ends, wherever the pointer goes (backlog F107); it reaches no scroll container above it, and a scroller inside it still takes the axes it scrolls while it can move that way, passing this node the rest — the other axis, and a gesture that begins with that scroller at its limit (`overscroll: "contain"` on the scroller keeps it there). The core moves nothing — a grid re-declares `originLine`, a canvas zooms. A drag-select held past a `cells` grid's top or bottom edge arrives here too, once a frame with the lines that frame scrolled by (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`). */
  onScroll?: AppMsg | null;
  /** Group opacity 0..1 (default 1): fades this node and its whole subtree. A per-quad alpha multiply rather than an offscreen composite, so overlapping pieces of one subtree show their seams through the fade. Layout, hit-testing and the access tree are untouched; eases with `transition`, and `enter: { opacity: 0 }` fades a panel in. */
  opacity?: LengthProp;
  /** What a scroll gesture that starts over this scroller does when it is already at its limit that way (backlog F107, CSS's `overscroll-behavior`): `auto` (the default) passes the gesture on to the scroller around it, `contain` keeps it here, moving nothing until it turns back. A gesture picks its target once, when it starts — the innermost scroller under the pointer that can still move the way it goes — and keeps it until it ends, wherever the pointer or the content has gone; one that reaches a limit midway stops there, whatever this says. Only on the axes the node scrolls: a `scrollY` list that contains still passes a sideways swipe to the strip around it. For a panel or a popup's list whose scrolling must never move what is behind it. */
  overscroll?: 'auto' | 'contain';
  /** Paint this node's background, border, shadow and fragment with each edge on a whole physical pixel: `x` and `x + width` rounded on their own, from where layout put them, as a text's span backgrounds are. Off by default, and a box is drawn where layout put it, so a 1 px `gap` between boxes is there at any scale. On, boxes that share an edge in layout meet on one pixel line, where a join inside a pixel was drawn by halves and left a seam — rows of a band stacked at a pitch that is not whole pixels, or a box that continues a text's selection. Layout, hit-testing, the clip and the children are untouched. A snapped box can draw up to half a pixel from its layout edge and its size can differ by a pixel, so a snapped hairline is 1 or 2 px thick by where it sits. */
  pixelSnap?: boolean;
  /** Background while pressed (or while its hoverGroup is); implies hover tracking. */
  pressedBg?: ColorProp;
  /** Corner radius for all four corners (logical px); the per-corner props override it when listed after it. On a node that also clips or scrolls it rounds the clip as well, so children stay inside the corners. */
  radius?: LengthProp;
  /** Bottom-left corner radius (logical px). */
  radiusBL?: LengthProp;
  /** Bottom-right corner radius (logical px). */
  radiusBR?: LengthProp;
  /** Top-left corner radius (logical px). */
  radiusTL?: LengthProp;
  /** Top-right corner radius (logical px). */
  radiusTR?: LengthProp;
  /** How `keyframes` cycle (CSS `animation-direction`, default normal). Lua: `direction`, since `repeat` is a keyword. */
  repeat?: 'normal' | 'reverse' | 'alternate' | 'alternateReverse';
  /** What the node is to assistive technology. Unset, the core derives one (an `onClick` node is a button, an editor a text input, a scrolling box a scroll view, a plain box nothing); `none` hides the node and its subtree from the access tree. A `radio` belongs inside a `radioGroup` and a `tab` inside a `tabList`, labelled with what the choice is: the pair is a composite (`docs/adr/0007-composite-keyboard-patterns.md`) — one Tab stop for the set, the arrows, Home and End moving the choice inside it (each step is the item's click, so the choice follows focus), and a screen reader reading "2 of 3". A `radio` or `tab` with no container above it is a Tab stop of its own that no arrow moves, and the core warns (`item-outside-container`). `menu` holds `menuItem`s and `list` holds `listItem`s the same way. */
  role?: 'none' | 'button' | 'checkbox' | 'radio' | 'switch' | 'slider' | 'tab' | 'tabList' | 'link' | 'heading' | 'list' | 'listItem' | 'image' | 'dialog' | 'group' | 'textInput' | 'multilineTextInput' | 'line' | 'radioGroup' | 'menu' | 'menuItem' | 'terminal';
  /** The width of a table's `rules` in logical px; 1 when unset. */
  ruleWidth?: LengthProp;
  /** On a table (`dir="table"`, ADR 0033): grid lines of this colour between its columns and between its rows (backlog DX21) — down the middle of each gap between the columns of its widest row, from the first row's top to the last row's bottom, and across the middle of each gap between rows, the content box wide. Drawn with the table's box, under its cells and on whole pixels, so give the table and its rows a `gap` at least `ruleWidth` for the lines to show between cells; the outer edge is the table's `border`. Ignored on anything but a table. */
  rules?: ColorProp;
  /** Which axes `onScroll` takes (backlog F107): `both` (the default), `x` or `y`. A scroll gesture on an axis the node does not take passes it by, to the scroller around it, and hears nothing here: a terminal that scrolls its history says `y`, and a sideways swipe that starts over it moves the strip it sits in. (A swipe that started elsewhere is not the node's either way: a gesture keeps the target it started with.) Meaningless without `onScroll`. */
  scrollAxes?: 'both' | 'x' | 'y';
  /** When a scrolling node draws its bars: `visible` (the default — the stock overlay thumb, drawn while the content overflows), `hidden` (no thumb, no track to press; the wheel, the keyboard, `reveal` and the caret still scroll it — for a list that draws its own indicator, or a pane whose bar would sit on a border), or `auto` (shown while the scroll state is changing — the offset or the content's extent moved, the pointer is on the track, a thumb is dragged — and for a second after, then faded out over a quarter of one; a node first seen shows it the same second; what an overlay bar does on macOS). `auto` needs the driver's clock and is `visible` without one. The bars are overlays and take no layout space in any mode. From the last change until it has faded — a second and a quarter — an `auto` bar asks for frames the way a transition of that length would (nothing else could wake the core when the hold ends); while the pointer holds it, it asks for none. */
  scrollbar?: 'visible' | 'hidden' | 'auto';
  /** The thumb under the pointer or while dragged; the default is the theme's `scrollbar_active` role. */
  scrollbarActiveColor?: ColorProp;
  /** The thumb at rest; the default is the theme's `scrollbar` role, a translucent wash over whatever it sits on. */
  scrollbarColor?: ColorProp;
  /** The thumb's width at rest, logical px (default 4); under the pointer or dragged it is 2 px wider. The grabbable track grows to fit a wide thumb. */
  scrollbarWidth?: LengthProp;
  /** Makes this node a selection scope: the text of every node inside it is one selectable run, in tree order, and a press-drag across them selects the lot — as do Shift with Left / Right / Home / End on a focused node inside it, a character or a word at a time, a scope with nothing selected anchoring at its start (`docs/adr/0017-selection-as-a-scope.md`). Declared on the container and not on each label, because what a reader selects is a paragraph or a card rather than one run of it — three labels in a column under one `selectable` select as three lines of one text. The selection is the window's: starting one anywhere clears the last, an editor's included. Scopes do not nest; an outer one around an inner one is warned about (`nested-selection-scope`) and the innermost owns the text. Text scrolled out of view inside the scope is still part of it — selection and copy reach it, hit-testing does not. On a `cells` grid the scope selects in cells rather than in bytes: a drag takes lines (with a modifier, a rectangle), a double click the word under the pointer and a triple click the whole row, its ends are absolute lines so a scroll does not move them, and a copy trims each line's trailing blanks. */
  selectable?: boolean;
  /** The current one of a set: which `tab` a `tabList` shows, which `listItem` a list has picked, which `link` is the page you are on. A `tab` always carries the state — its siblings read as "not selected" — while a list row or a link carries it only where it is set, since an ordinary list or navigation bar is not a selection and a reader saying "not selected" on every row of it is noise. */
  selected?: boolean;
  /** On a `line` of a custom editor: the byte offset where the selection's other end sits (the caret is `caret`, possibly on another line). */
  selectionAnchor?: LengthProp;
  /** Drop-shadow blur radius (logical px): the edge ramps over this distance and reaches this far past the shape. 0 = a hard edge. */
  shadowBlur?: LengthProp;
  /** Drop-shadow color; nothing else about a shadow draws without it. On its own it is a hard shadow exactly behind the node — add `shadowBlur` / `shadowY` to lift it. Outer shadows only, and the shape is not knocked out of the middle, so a translucent background shows it through. */
  shadowColor?: ColorProp;
  /** Grows (or, negative, shrinks) the drop shadow's shape before blurring (logical px). */
  shadowSpread?: LengthProp;
  /** Drop-shadow horizontal offset (logical px). */
  shadowX?: LengthProp;
  /** Drop-shadow vertical offset (logical px); positive casts downward. */
  shadowY?: LengthProp;
  /** With transition: also ease the node's position (reordered siblings slide). While it eases, the node is drawn between where it was and where this frame put it — not at the declared `dx`/`dy`, or its slot in the row — so anything else positioned from those numbers drifts for the transition's length: a canvas of floats eases everything or nothing. */
  slide?: boolean;
  /** Animate sizing/colors/radius changes over this many ms — and, on a scroll container, the offset a reveal or a set_scroll moves it to (needs a stable key). */
  transition?: LengthProp;
  /** A `slider` role's maximum. */
  valueMax?: LengthProp;
  /** A `slider` role's minimum. */
  valueMin?: LengthProp;
  /** A `slider` role's current value (the drawing stays yours; this is what assistive technology reads). */
  valueNow?: LengthProp;
  /** How far one arrow key moves a `slider` role, and the grid a value the pointer sets snaps to (ADR 0034). Unset, a hundredth of the range. PageUp / PageDown move ten steps. Read by the core only where the slider declares `onChange`; reported to assistive technology either way. */
  valueStep?: LengthProp;
  /** What a `slider` role's position reads as (ARIA's `aria-valuetext`). Without one a reader has only `valueNow` and the range and says a percentage — 25 in [5..60] is "36 percent" — so a value whose unit carries the meaning says it here: "25 minutes". It replaces the number in the reading rather than joining it, and a nudge announces the new text. Meaningful on the slider role alone, like the three numbers; putting the reading in `label` instead renames the control on every nudge, which is the wrong attribute. */
  valueText?: string;
  /** Horizontal size: px | "fit" | "grow" | "N%" | a size expression — `"clamp(400px, 80%, 1000px)"`, `"min(720px, 100%)"`, `"max(50%, 300)"`, nested — which layout resolves against the parent's content box, the box a percentage takes its cut of (backlog F109). An expression with no percentage in it is a length; a calc does not ease under `transition`. A percentage or an expression gives, with the fit children, when its parent overflows — two `"50%"` children and a gap fit their row (backlog F110) — where a px size keeps its own. A row holding one gives as CSS's flex items do: every child that can give gives in proportion to its size, and stops at its content — the widest thing in it that cannot wrap, a label's longest word — unless `minWidth` says otherwise (backlog RG92). The process keeps 65 536 distinct expressions and never lets one go: past that a new one leaves its prop at its default, with a `size-expressions-full` warning, so declare one per layout — a px size for the part that moves each frame, a splitter's drag — not one per frame (backlog RG93). */
  width?: SizingProp;
  /** Window-chrome role: interactions become window commands, not events. */
  window?: 'drag' | 'close' | 'minimize' | 'maximize';
  /** Children that don't fit the main axis start a new line instead of overflowing or shrinking. Rows only (a column is ignored, with a warning), and never on a scrollX row. */
  wrapChildren?: boolean;
}

export interface GeneratedStyleProps {
  /** Text color; default foreground when omitted. */
  color?: ColorProp;
  /** End the last line with an ellipsis when the text is cut off: a single line unless `maxLines` says otherwise. */
  ellipsis?: boolean;
  /** Font family: `sans`, `serif` or `mono`, kui's own, or the name of an installed family or one loaded with `loadFontsDir` / `loadFontFile` — `"Berkeley Mono"` — drawn in its face in the frame that names it (ADR 0037). A name is matched as `addSystemFont` matches it and registered in the session on first sight, exactly as the font database spells it (`"menlo"` is not `"Menlo"`); the session's first registration of any font maps the installed font files once (~30 ms on a Mac, backlog DX24), which a family named in a view pays in that frame. `systemFonts()` lists the names there are. A name nothing matches shapes as sans and raises `unknown-family`. It and `font` set the same thing, so declare one. */
  family?: 'sans' | 'serif' | 'mono' | (string & {});
  /** OpenType features for the shaper, as `tag=value` pairs separated by spaces or commas — a bare `tag` is 1, `-tag` is 0: `"liga=0 calt=0"` keeps a coding font from joining `->` and `!=` (what a terminal built on runs needs to hold its grid), `"tnum"` lines figures up in a gutter, `"ss01"` picks a stylistic set. Unset, the font's own defaults apply. At most 8; part of what the text is shaped as, so two texts differing only here are shaped twice. */
  features?: string;
  /** A registered font handle (addFont / addSystemFont); overrides `family`. */
  font?: string;
  /** Line height (logical px); default size * 1.35. */
  lineHeight?: LengthProp;
  /** Lay out at most this many lines (0 = unlimited); with `ellipsis`, a line clamp. */
  maxLines?: LengthProp;
  /** A line through the text, where the face puts its strikeout. Paint only; on a `<span>` the span alone, per line. */
  strikethrough?: boolean;
  /** A line under the text, where the face puts its underline and as thick as it says, in the text colour. Paint only. On a `<span>` it covers the span alone and follows it across a wrap, one rect per line. `underlineColor` gives it a colour of its own and `underlineStyle` a shape; either implies it. */
  underline?: boolean;
  /** The underline's own colour — a diagnostic's red under keyword-coloured text (backlog K4). Implies `underline`. On a `<span>` the span's; a span with no colour of its own takes the text's. */
  underlineColor?: ColorProp;
  /** The underline's shape (backlog K4): `solid` (the face's line), `wavy` (three strokes tall around the line, a six-stroke period — a diagnostic's squiggle, a terminal's undercurl) or `dotted` (dots two strokes across, four apart). Implies `underline`. A wave or dots are runs of the segment primitive a `line` draws, so no backend learns a kind; the cost is two quads per period. */
  underlineStyle?: 'solid' | 'wavy' | 'dotted';
  /** Line breaking at the node's width: between words (default), anywhere, never (one line per paragraph, clipped to the node), or between words with whitespace taking its room (`break-spaces`: a space that does not fit starts the next row rather than hanging past the edge — an editor's wrapped line). On a single-line `edit` — a field, which otherwise takes one line and scrolls it — declaring it is what makes the field fold to its width like a document, by this mode, while Enter still submits (see `edit`). */
  wrap?: 'word' | 'glyph' | 'none' | 'break-spaces';
}
// -- end generated --

/** Composite/constructor props with hand-written handling on both sides of
 *  the boundary (everything else comes from the generated schema types). */
export interface CustomSpecProps {
  /** Main axis; `column` is the default. `table` is a column whose rows'
   *  children line up in columns (ADR 0033): the nth in-flow child of
   *  every row is column n (a float in a row is not a cell), and a
   *  column is as wide as its widest cell — a cell's
   *  `width` sizes its column (`fit` and a number are content, `grow`
   *  grows the column, a percent takes its cut), a bare `<text>` is a
   *  cell held to its column, and the rows are rows: give them
   *  `width="grow"` for the columns to grow into, with their own `gap`,
   *  padding, `bg`, `hoverBg` and `onClick`. */
  dir?: 'row' | 'column' | 'table';
  pad?: LengthProp;
  padX?: LengthProp;
  padY?: LengthProp;
  padL?: LengthProp;
  padR?: LengthProp;
  padT?: LengthProp;
  padB?: LengthProp;
  borderW?: LengthProp;
  borderColor?: ColorProp;
  clip?: boolean;
  scrollX?: boolean;
  scrollY?: boolean;
  float?: 'below' | 'above' | 'parent' | 'viewport' | FloatProp;
  /** Focuses this node (an `onKey` sink, an editor, any focusable node)
   *  when it starts being declared: declared every frame it takes focus
   *  once, so a later Tab press is not clobbered. `ctx.focus(key)` moves
   *  focus at any time. */
  keyFocus?: boolean;
  /** Hover hint: a tooltip floated below this box while it is hovered
   *  (implies hoverable). */
  tooltip?: string;
  /** Stable identity by *data* index rather than by name: the key
   *  auto-keying would have given this node as the `i`th child, given to
   *  it wherever it actually sits. What a virtualised list is for — a
   *  view that builds rows 900..930 opens each with its own row number,
   *  so a row keeps its hover, focus, edit buffer and tweens as the built
   *  range slides over it. Beside a `key`, the index wins. */
  index?: number;
  /** How many `index`ed rows this node's virtual list has, built or not.
   *  `uniformList` declares it on its container; a list composed by
   *  hand says it beside `scrollY`. Select All inside a `selectable`
   *  virtual list then selects the *data*, rows `0..rowCount`, and the
   *  copy is a `selectionrange` ask whose `to.byte` is past the last
   *  row's length when that row is not built — cut it to the row. */
  rowCount?: number;
}

export interface BoxProps extends Keyed, GeneratedSpecProps, CustomSpecProps {
  /** Root box only: declares this frame's window title. */
  title?: string;
  /** Root box only: asks for the window above every other app's this
   *  frame — a floating palette, a picture-in-picture player, a timer.
   *  Declare it every frame you want it; a frame that stops is what lowers
   *  the window again, so a pin button toggles by declaring or not. Whether
   *  the platform agreed is `env.window.alwaysOnTop`, which is what the
   *  button should draw from. */
  alwaysOnTop?: boolean;
  /** Root box only: asks for secure keyboard entry while this window has
   *  the keyboard — what a terminal turns on at a password prompt, so no
   *  other process can read the keys typed there (macOS; nothing
   *  elsewhere). Declare it every frame the prompt is up; the frame that
   *  stops is what turns it off. The runner enables it only while the
   *  window has the keyboard and keeps the platform's count balanced. */
  secureInput?: boolean;
  /** Root box only: which Option keys act as Alt in this window on macOS
   *  (backlog F113). Option composes on a Mac — ⌥u, ⌥e, ⌥i, ⌥n and ⌥`
   *  are dead keys that start an accent, so the press never arrives as a
   *  key — and a side named here is Alt instead: it types nothing and a
   *  key under it arrives as a chord of the layout's unmodified key, so a
   *  keymap's `<A-u>` fires. `"left"` or `"right"` leaves the other side
   *  composing; `"none"`, the default, is the Mac's own behaviour.
   *  Declare it every frame; the frame that stops gives the Option keys
   *  back to the layout. Nothing elsewhere. */
  optionAsAlt?: 'none' | 'left' | 'right' | 'both';
  /** Root box only: which windows exist besides the main one (see
   *  `WindowDecl` in `@qxuken/kui`). `runWindowed` / `createApp` write it
   *  from the loop config's `windows(model)`; a view driving a `Ctx` by
   *  hand declares it here. */
  windows?: (string | { name: string; width?: number; height?: number; activates?: boolean })[];
  children?: KuiNode;
}

export interface TextProps extends Keyed, GeneratedStyleProps {
  size?: LengthProp;
  children?: KuiNode;
}

export interface SpanProps extends Keyed {
  bold?: boolean;
  italic?: boolean;
  /** Overrides the paragraph color; nested spans inherit. */
  color?: ColorProp;
  /** A line under the span, where the face puts it; nested spans inherit. */
  underline?: boolean;
  /** The underline's own colour — a diagnostic's red under keyword-coloured
   *  text; implies `underline`, nested spans inherit. Without it the
   *  underline is the span's colour. */
  underlineColor?: ColorProp;
  /** The underline's shape: `solid` (the face's line), `wavy` (a squiggle,
   *  three strokes tall with a six-stroke period) or `dotted`; implies
   *  `underline`, nested spans inherit. */
  underlineStyle?: 'solid' | 'wavy' | 'dotted';
  /** A line through the span, where the face puts it; nested spans inherit. */
  strikethrough?: boolean;
  /** A background behind the span's glyphs alone — one rect per line it
   *  spans, so it follows the span across a wrap the way a box cannot. */
  bg?: ColorProp;
  /** Rounds `bg` (logical px), and joins it into one shape with every
   *  rounded background of the same colour and radius it meets — edge to
   *  edge on the line above or below, or end to end on its own line, in
   *  this text or another: convex corners where a line reaches past its
   *  neighbour, a fillet where it falls short, round where nothing meets.
   *  A selection over rows, or over a paragraph's wrapped lines, is one
   *  outline. Nested spans inherit; 0 (the default) is square. */
  bgRadius?: number;
  children?: KuiNode;
}

/** The stock button (`widgets::button_spec`: padding, colours, radius,
 *  hover and pressed backgrounds). Its look is its own, so the layout and
 *  paint rows are not here — declared anyway they are dropped with an
 *  `unknown-prop` warning — and what is here are the rows a reader hears:
 *  `label` when the text is not the name, `description` for what the
 *  button will do, `tooltip`, and `disabled` (inert, and dimmed to half).
 *  The one paint row it does take is `accent`, which is a question put to
 *  the OS rather than a colour: the three backgrounds come off
 *  `env.system.accent` and the label goes black or white by its luminance,
 *  and the stock blue stands on a host that never said what the accent is.
 *  A button that needs any other row is a `<box role="button">` with the
 *  same rows spelled out. Keyed by its text unless `key` says otherwise.
 *
 *  The list is `BUTTON_ROWS_JSX` in schema.rs — the rows the encoder admits
 *  — so what it extends is generated from there with the rest of this file:
 *  add a row in Rust and it is a prop here the same `npm run gen` later. */
export interface ButtonProps
  // -- generated from the addon's button rows; edit BUTTON_ROWS_JSX in schema.rs, then `npm run gen` --
  extends Keyed,
    Pick<GeneratedSpecProps, 'onClick' | 'label' | 'description' | 'disabled' | 'accent'>,
    Pick<CustomSpecProps, 'index' | 'tooltip'>
  // -- end generated --
{
  children?: KuiNode;
}

/** A stock toggle — `<checkbox>`, `<radio>`, `<switch>`
 *  (docs/adr/0034-stock-controls-over-the-roles.md): drawn from the state
 *  you declare, `checked` (and on a checkbox `mixed`, the select-all box
 *  over a partial selection), with its text beside it. A press by the
 *  pointer, Space, Enter or assistive technology posts `onClick`; your
 *  `update` flips the model and the view draws it again. Its look is its
 *  spec, so these are the only rows it reads (`TOGGLE_ROWS_JSX` in
 *  schema.rs): any other is dropped with an `unknown-prop` warning. Keyed
 *  by its text unless `key` says otherwise. */
export interface ToggleProps
  extends Keyed,
    Pick<GeneratedSpecProps, 'onClick' | 'label' | 'description' | 'disabled' | 'checked' | 'mixed'>,
    Pick<CustomSpecProps, 'tooltip'> {
  children?: KuiNode;
}

/** The stock slider (docs/adr/0034-stock-controls-over-the-roles.md): a
 *  track, a fill to `valueNow` and a thumb. With `onChange` the core turns
 *  a press, a drag, the arrows, PageUp / PageDown and Home / End into
 *  `{kind: 'change', value, phase: 'move' | 'end', tag}` — clamped to
 *  `valueMin`..`valueMax` (0..100 unset) and snapped to `valueStep` (a
 *  hundredth of the range unset); store `value` and declare it as
 *  `valueNow`, since nothing moves until you do. `label` is its name and,
 *  unless `key` says otherwise, its key. Its look is its spec: the rows it
 *  reads are these (`SLIDER_ROWS_JSX` in schema.rs). */
export interface SliderProps
  extends Keyed,
    Pick<
      GeneratedSpecProps,
      | 'label'
      | 'description'
      | 'disabled'
      | 'valueNow'
      | 'valueMin'
      | 'valueMax'
      | 'valueStep'
      | 'valueText'
      | 'onChange'
      | 'width'
      | 'minWidth'
      | 'maxWidth'
    >,
    Pick<CustomSpecProps, 'tooltip'> {
  label: string;
}

/** A container of `<radio>`s (docs/adr/0034-stock-controls-over-the-roles.md):
 *  one Tab stop whose arrows, Home and End move the choice and press the
 *  radio they land on, so radios whose `onClick` each set the choice
 *  answer the keyboard with nothing more. Every box row; the role and the
 *  name are its own, and without a `gap` it takes the stock one. */
export interface RadioGroupProps extends BoxProps {
  label: string;
}

export interface EditProps extends TextProps, GeneratedSpecProps, CustomSpecProps {
  /** Stable identity (state is retained by key); `key` works too. */
  id?: string;
  initial?: string;
  multiline?: boolean;
  /** Takes focus once, on the frame the flag starts being declared, and only
   *  while nothing holds focus (ADR 0022, decision 9); a blur afterwards
   *  stands. `focus(key)` moves it at any other time. */
  autofocus?: boolean;
}

type Component<P> = (props: P) => KuiNode;

export declare function jsx<P>(type: Component<P>, props: P, key?: string | number): KuiNode;
export declare function jsx(type: string, props: object, key?: string | number): KuiElement;
export declare const jsxs: typeof jsx;
export declare const jsxDEV: typeof jsx;
export declare const Fragment: unique symbol;

export declare namespace JSX {
  type Element = KuiNode;
  type ElementType = string | Component<any>;
  interface ElementChildrenAttribute {
    children: {};
  }
  interface IntrinsicAttributes {
    key?: string | number;
  }
  interface IntrinsicElements {
    box: BoxProps;
    text: TextProps;
    button: ButtonProps;
    checkbox: ToggleProps;
    radio: ToggleProps;
    switch: ToggleProps;
    radioGroup: RadioGroupProps;
    slider: SliderProps;
    edit: EditProps;
    /** The stock single-line field with its chrome (`widgets::text_input`):
     *  `label` is the key and the accessible name both, `initial` the seed
     *  (a new editor only), and nothing else is read — the same door as
     *  Lua's `input { label= }` and C's `kui_text_input`. Read it back
     *  with `editText(label)`; a field that needs any other row is an
     *  `<edit>` in a box of its own. */
    input: { key?: string | number; label?: string; initial?: string };
    /** The stock select (`widgets::select_items`): a field showing the
     *  choice in force that, clicked, opens the core's own menu of the
     *  options under it with the current one checked — drawn, or the
     *  platform's where the host shows menus itself; Escape or a press
     *  outside closes it. `label` is the key and the accessible name;
     *  `options` are strings (posting the label) or the menu items
     *  `openMenu` takes (posting `id`); `current` is the index in force,
     *  from 0. You hold no open state: the choice arrives as a `MenuMsg`
     *  on the field's key — `{kind: 'menu', item}` — and drawing the
     *  field again with the new `current` is the whole loop. Nothing else
     *  is read; a field that needs any other row is a `<box role="button">`
     *  and `openMenu`. Lua spells it `dropdown { }`, C `kui_select`. */
    select: { key?: string | number; label?: string; options: (string | MenuItemInput)[]; current?: number };
    /** The node form of a tooltip (`widgets::tooltip`): a float hanging
     *  below the parent, always drawn — where the `tooltip` prop is
     *  hover-gated — for a hint the view gates itself
     *  (`{hovered && <tooltip value="hint"/>}`). `value` alone is the
     *  text; children are the float's own content. */
    tooltip: Keyed & { value?: string; children?: KuiNode };
    /** Styled run inside a rich <text>: bold/italic/color, nestable. */
    span: SpanProps;
    /** A registered image (id from addImage). Fit sizing = pixel size as
     *  logical px; Fit height against a resolved width keeps the aspect.
     *  Two rows say how the pixels meet the box
     *  (docs/adr/0025-the-image-is-the-canvas.md): `sampling` is
     *  `linear` (default) or `nearest` — pixel art, an emulator, a data
     *  grid that must stay square under zoom; `fit` is `fill` (default:
     *  the pixels stretch to the box), `contain` (the largest rect of the
     *  image's aspect that fits, centred) or `cover` (the box filled and
     *  the rest cropped, centred). The box itself — layout, hit region,
     *  access rect — is the same in every mode. The pixels come from the
     *  atlas, or from a texture of the image's own once `updateImage` has
     *  replaced them; the node cannot tell and need not. */
    image: Omit<BoxProps, 'children'> & {
      src: string;
      sampling?: 'linear' | 'nearest';
      fit?: 'fill' | 'contain' | 'cover';
    };
    /** A box a registered WGSL function paints
     *  (docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md):
     *  gradients, rings, noise, shimmer — anything the paint vocabulary has
     *  no prop for. An ordinary node otherwise: it lays out, rounds, clips,
     *  fades, takes input and holds children, which paint over it. It has
     *  **no intrinsic size**, so give it a `width`/`height` or `fill`.
     *  `src` is an id from `addFragment`; `params` is up to sixteen numbers
     *  the shader reads as four `vec4<f32>`, and more are dropped with a
     *  warning; `animate` asks for a frame every frame, which a fragment
     *  reading `time` needs and a still one must not declare. `image` is an
     *  id from `addImage` the function reads through `kui_sample(uv)` /
     *  `kui_sample_nearest(uv)`, its texel rect in `in.image` — a waveform,
     *  a heatmap, an image effect from a texture the app replaces
     *  (docs/adr/0025-the-image-is-the-canvas.md, decision 7); an image
     *  that is not live draws nothing, as a dead `src` does. */
    fragment: BoxProps & { src: string; image?: string; params?: number[] };
    /** A position an extension fills, in place
     *  (docs/adr/0014-slots-an-extension-fills-in-place.md): whatever was
     *  loaded under the name's namespace draws here, as a child of this
     *  node, at this position among its siblings.
     *
     *  `name` is the full `namespace/slot` — the namespace you loaded the
     *  plugin under (`ctx.addExtension('todos', path)`, or a window's
     *  `extensions` option) and the slot in the plugin's own vocabulary.
     *  `params` is whatever the plugin should read this frame: plain data,
     *  declared every frame, retained by nobody, like `onClick`'s payload.
     *  `"ns/root"` is the fill that follows the view for a plugin naming no
     *  slots, and declaring it moves that fill here.
     *
     *  A position, not a box: it takes no other props, and with nothing
     *  loaded under that namespace it places an empty node so a view can
     *  declare its layout before it has a plugin to put in it. */
    slot: { name: string; params?: unknown };
    /** A tab in the core's devtools panel, beside facts, events and tree
     *  (docs/adr/0032-a-devtools-tab-mounts-a-slot.md). `name` is the tab's
     *  identity, `label` what the strip shows (the name when left out).
     *
     *  Two forms. `slot` names a slot an extension fills, the full
     *  `namespace/slot` a plugin you loaded names: while the tab is on show
     *  the panel declares that slot in the tab's body and the plugin draws
     *  there; otherwise the plugin is not asked, and its naming the slot
     *  raises no `unknown-slot`. A **function child** is the app's own
     *  content, `{() => <column>…</column>}`: called only while the tab is
     *  on show — `frame` / `setView` read which tab that is once before
     *  encoding — so a tab nobody looks at costs its declaration and
     *  nothing else. What it returns is the app's: its keys, its events
     *  reaching `update` untouched, laid out and painted as a layer over
     *  the panel's tab body, clipped to it, in the dock's focus region.
     *  Read the panel's facts through `devtoolsSelected()` and its
     *  siblings; drive its highlight with `setDevtoolsSelected(key)`.
     *
     *  Not a node: declare it anywhere in the tree, every frame. A slot and
     *  a child together, or a child that is not a function, is a throw. A
     *  name declared twice in a frame is one tab and a `duplicate-tab`
     *  warning; the first stands. Docked only for the function form: with
     *  the panel in its own window the body says so. */
    devtoolsTab: { name: string; label?: string; slot?: string; children?: () => unknown };
    /** A round-capped stroke (docs/adr/0010-a-segment-primitive.md): one
     *  segment from `from` to `to`, a polyline through `points`, or a smooth
     *  curve through them with `curve`. Points are in the parent's box space
     *  (`float="viewport"` for viewport space). Never in layout: it floats,
     *  sized to its own bounding box, so it takes no room in a row or
     *  column. A stroke in its parent's box space is held by the parent's
     *  clip as a child is, its hit region with it, so it is cut at a
     *  scroller's edge with the row it is drawn in; a declared float is
     *  held that way only when it declares `clip` with a parent anchor, and
     *  a `float="viewport"` stroke escapes (backlog F78). `width` is the stroke width in px (default 1), `color` the
     *  stroke colour (default the foreground); `transition` eases the colour,
     *  and with `slide` beside it the stroke's position too — the points ride
     *  its box, so a stroke whose ends all move together slides with them,
     *  while one whose ends move apart resizes at once. A canvas of floats
     *  eases everything or nothing, connectors included.
     *  Hit by its shape (docs/adr/0026-hit-testing-by-shape.md): with
     *  `onClick`, `onDrag`, `onHover` or `hoverable`, a press within half
     *  its width of any piece (at least 4 px of grab) hits it and one
     *  elsewhere in its box falls through; with none it takes no input and
     *  has no access row, and with input it is a control — name it. */
    line: Keyed &
      Pick<
        GeneratedSpecProps,
        | 'opacity' | 'transition' | 'slide' | 'enter' | 'exit' | 'onLayout' | 'label' | 'role'
        | 'onClick' | 'onDrag' | 'onHover' | 'hoverable' | 'cursor' | 'description'
      > & Pick<CustomSpecProps, 'tooltip'> & {
        from?: [number, number];
        to?: [number, number];
        points?: [number, number][];
        curve?: boolean;
        /** Cuts the stroke into marks and gaps (backlog V2): one length
         *  (marks and gaps alike), `[mark, gap]`, or `[mark, gap, mark,
         *  gap]` for a dash-dot, in px **as seen** — every mark is
         *  round-capped, so a mark no longer than the stroke is wide is a
         *  dot (SVG's `stroke-dasharray` measures the centre line; this is
         *  its `mark − width, gap + width`). The pattern runs along the
         *  whole stroke, corners and curves included. A pattern with no
         *  gap, or finer than a pixel, draws solid. */
        dash?: number | [number] | [number, number] | [number, number, number, number];
        /** How far into the pattern the stroke starts, in px: growing it
         *  moves the marks towards the first point — a marquee's marching
         *  ants. It wraps; it does not tween. */
        dashOffset?: number;
        width?: LengthProp;
        color?: ColorProp;
        float?: 'parent' | 'viewport';
      };
    /** A filled polygon through up to eight `points`, the fill in `bg`
     *  (docs/adr/0025-the-image-is-the-canvas.md, decision 6): an arrowhead,
     *  a pie slice, the area under a curve. Placed as a `line` is — always
     *  a float in its parent's box space (`float="viewport"` for viewport
     *  space), sized to its own bounding box a pixel out on each side, so
     *  it takes no room in a row or column. Like a `line`, it is held by its
     *  parent's clip, hit region included, unless it is
     *  `float="viewport"` (backlog F78). `transition` eases the fill and,
     *  with `slide`, its position. The outline may be concave; a
     *  self-intersecting one fills even-odd, its overlaps unfilled. Hit by
     *  its outline
     *  (docs/adr/0026-hit-testing-by-shape.md): with `onClick`, `onDrag`,
     *  `onHover` or `hoverable`, a press inside the outline hits it and one
     *  in its box past the outline falls through — a pie's wedges need no
     *  hit boxes; with none it takes no input and has no access row, and
     *  with input it is a button — name it. A ninth point and later are
     *  dropped with `polygon-points-truncated`; fewer than three draw
     *  nothing; no `bg`, no fill. One `fragment` quad on the wire, painted
     *  by a WGSL function the core registers itself. */
    polygon: Keyed &
      Pick<
        GeneratedSpecProps,
        | 'opacity' | 'transition' | 'slide' | 'enter' | 'exit' | 'onLayout' | 'label' | 'role'
        | 'onClick' | 'onDrag' | 'onHover' | 'hoverable' | 'hoverBg' | 'cursor' | 'description'
      > & Pick<CustomSpecProps, 'tooltip'> & {
        points: [number, number][];
        bg?: ColorProp;
        float?: 'parent' | 'viewport';
      };
    /** Any outline — SVG path data, a pie wedge with a round arc, a map's
     *  region, an icon — filled with `bg` by `fillRule` (`nonzero`, the
     *  default, or `evenodd`) and stroked `width` wide in `color` when
     *  `width` is given, the stroke over the fill
     *  (docs/adr/0040-a-path-is-a-mask-in-the-atlas.md). `d` is SVG path
     *  data (`M L H V C S Q T A Z`, absolute or relative), parsed by one
     *  parser in the core so every binding draws the same shape — one that
     *  does not parse raises `path-malformed` and draws nothing — or a flat
     *  number array of op codes and operands (0 M x y, 1 L x y, 2 Q cx cy x
     *  y, 3 C c1x c1y c2x c2y x y, 4 A rx ry rot large sweep x y, 5 Z).
     *  Placed as a `line` is: always a float in its parent's box space
     *  (`float="viewport"` for viewport space), sized to its own bounding
     *  box two pixels out on each side, so it takes no room in a row or
     *  column. `transition` eases the fill and, with `slide`, its position.
     *  Hit by its outline under the fill rule
     *  (docs/adr/0026-hit-testing-by-shape.md): with `onClick`, `onDrag`,
     *  `onHover` or `hoverable`, a press inside hits it and one in its box
     *  past the outline falls through — a pie's wedges need no hit boxes; a
     *  stroke with no fill is hit by its stroke, as a line is; with input it
     *  is a button — name it. On the wire it is one glyph-mask quad per
     *  paint, fill and stroke, from the glyph atlas: rasterized once per
     *  shape, scale and quarter-pixel position, re-tinted for free. The fill
     *  bleeds half a pixel, so two paths sharing an edge meet without the
     *  background showing through. A mask a quarter of the biggest atlas
     *  page or more, or a path whose `d` changes twice within a few frames, draws
     *  from a texture of its own; one past 8192 px on a side draws nothing,
     *  with `path-too-large`. */
    path: Keyed &
      Pick<
        GeneratedSpecProps,
        | 'opacity' | 'transition' | 'slide' | 'enter' | 'exit' | 'onLayout' | 'label' | 'role'
        | 'onClick' | 'onDrag' | 'onHover' | 'hoverable' | 'hoverBg' | 'cursor' | 'description'
      > & Pick<CustomSpecProps, 'tooltip'> & {
        d: string | number[];
        fillRule?: 'nonzero' | 'evenodd';
        /** Turns the path, in turns clockwise, about `pivot`: the quad
         *  turns and the mask is drawn once, so a path that only turns is
         *  never rasterized again
         *  (docs/adr/0041-a-mask-turns-about-its-centre.md). */
        rotate?: number;
        /** The point `rotate` turns about, in the path's own coordinates;
         *  the centre of its box without one. A path with `rotate` or
         *  `pivot` is boxed by the square the turn sweeps. */
        pivot?: [number, number];
        /** Cuts the stroke into marks and gaps (backlog V2): one length
         *  (marks and gaps alike), `[mark, gap]`, or `[mark, gap, mark,
         *  gap]` for a dash-dot, in px **as seen** — every mark is
         *  round-capped, so a mark no longer than the stroke is wide is a
         *  dot (SVG's `stroke-dasharray` measures the centre line; this is
         *  its `mark − width, gap + width`). The pattern runs along the
         *  outline, restarting at every subpath as SVG's does. A pattern with no
         *  gap, or finer than a pixel, draws solid. */
        dash?: number | [number] | [number, number] | [number, number, number, number];
        /** How far into the pattern the stroke starts, in px: growing it
         *  moves the marks towards the first point — a marquee's marching
         *  ants. It wraps; it does not tween. */
        dashOffset?: number;
        bg?: ColorProp;
        width?: LengthProp;
        color?: ColorProp;
        float?: 'parent' | 'viewport';
      };
    /** Adaptive titlebar (drag strip + window buttons per env facts).
     *  `title` alone draws the standard title; children host custom content. */
    titlebar: Keyed & { title?: string; children?: KuiNode };
    /** Just the min/max/close buttons, for fully custom titlebars. */
    windowButtons: Keyed;
    /** The application menu (`docs/adr/0018-a-menu-bar-the-app-declares.md`):
     *  `menu` is what it is, and where this element sits is where its
     *  titles go when they have to be drawn in the window. Draws nothing
     *  where the platform owns the bar (macOS, where the driver hands the
     *  same declaration to the OS), so one view is portable. `[]` takes
     *  the menu away. */
    menuBar: Keyed & { menu?: MenuBarInput };
    /** Per-phase frame-latency bars (input/view/layout/render) vs the
     *  display budget. Reads the runner's frame stats — renders empty
     *  when headless. */
    latencyGraph: Keyed;
    /** latencyGraph in a translucent panel floating in a viewport corner. */
    latencyHud: Keyed & { at?: [AlignProp, AlignProp] };
    /** A playback retained by node key (id from addSound): present = playing
     *  (once, or looped), gone = stopped; `volume` / `paused` apply live, a
     *  changed `src` restarts. `finish` changes what gone means — the
     *  removal releases the playback to play itself out, so a one-shot need
     *  not stay declared for a length the view would have to guess (a loop
     *  still stops). `tag` comes back as a `SoundMsg` when it ends on its
     *  own, a released playback included. A released playback holds one of
     *  the device's 128 voices until its file ends — voices held = sound
     *  length × release rate, so a 1.4 s chime released four times a second
     *  holds 6 — and the 129th play is refused (`phase: "refused"` on the
     *  `tag`). Draws nothing and takes no space. */
    audio: Keyed & { src: string; loop?: boolean; volume?: number; paused?: boolean; finish?: boolean; tag?: AppMsg };
    /** A terminal's screen as one node (backlog C20): `rows × cols` cells,
     *  four entries each in `cells` — codepoint, fg, bg (`0xRRGGBBAA`, 0 = no
     *  background), flags (1 bold, 2 italic, 4 underline, 8 strikethrough,
     *  16 wide) — row-major, in a `Uint32Array` an app keeps and mutates. A
     *  glyph is shaped once per character and placed at `col × cell_w` ever
     *  after, so a screen new every frame costs what a still one costs. The
     *  style rows size the cells (`size`, `family`/`font`, `lineHeight`);
     *  the node rows apply — `onKey` makes it the terminal's sink, `onClick`
     *  / `onDrag` events carry `cell: {row, col}`. `cursorAt` is a cell to
     *  paint under its glyph in `cursorColor`, as a block, bar or underline
     *  (`cursor` stays the pointer shape).
     *  Its access role is `terminal`, the rows joined as its value. */
    cells: Omit<TextProps, 'children'> &
      GeneratedSpecProps &
      CustomSpecProps & {
        rows: number;
        cols: number;
        /** Four entries a cell — codepoint, fg, bg, flags — or five, the
         *  fifth the underline's own colour (0 = fg), row-major. Flags: 1
         *  bold, 2 italic, 4 underline, 8 strikethrough, 16 wide, 32 the
         *  underline is a wave (undercurl), 64 dotted. */
        cells: Uint32Array | number[];
        cursorAt?: [number, number];
        cursorShape?: 'block' | 'bar' | 'underline';
        cursorColor?: ColorProp;
      };
  }
}
