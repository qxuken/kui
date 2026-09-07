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

/** number = fixed logical px; "N%" of parent; grow soaks up leftover space. */
export type SizingProp =
  | number
  | 'fit'
  | 'grow'
  | `${number}%`
  | { grow: number }
  | { percent: number };

/** A lower clamp: logical px, or "fit" for the node's own fit size on that
 *  axis — what lets a `grow` child keep a content floor (a tab never
 *  narrower than its label). */
export type MinProp = number | 'fit';

/** 0xRRGGBBAA number, or "#rgb" / "#rrggbb" / "#rrggbbaa". */
export type ColorProp = number | string;

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
}

interface Keyed {
  /** Stable identity for retained state (scroll offsets, editors). */
  key?: string | number;
}

// -- generated from the addon's prop schema; edit schema.rs, then `npm run gen` --
export interface GeneratedSpecProps {
  /** Background fill. */
  bg?: ColorProp;
  /** On a `line` of a custom editor (a `textInput` / `multilineTextInput` role drawn by the app): the caret's byte offset into that line's text. */
  caret?: number;
  /** Center children on both axes. */
  center?: boolean;
  /** The on state of a `checkbox` / `radio` / `switch` role. */
  checked?: boolean;
  /** A registered sound (addSound) played when the node is clicked; implies hover tracking. */
  clickSound?: string;
  /** Child alignment across the main axis. */
  crossAlign?: 'start' | 'center' | 'end';
  /** Space between wrap lines, across the main axis (`gap` stays the space along it). */
  crossGap?: number;
  /** Overrides the pointer shape over this node. Unset, the core derives one from what the node does — an editor is `text`, an `onClick` or `focusable` node `pointer`, an `onDrag` node `grab` (`grabbing` while dragging), window chrome and a plain box `default` — so this is for what that cannot know: a splitter (`ewResize` / `nsResize`), a `disabled` control that says `notAllowed`. */
  cursor?: 'default' | 'text' | 'pointer' | 'grab' | 'grabbing' | 'notAllowed' | 'ewResize' | 'nsResize' | 'nwseResize' | 'neswResize';
  /** Holds the `keyframes` cycle back by this many ms (CSS `animation-delay`); siblings with different delays run out of phase. */
  delay?: number;
  /** The accessible description: the extra sentence a reader says after the name, for what the name cannot say on its own — what a button will do, why a control is disabled, what format a field wants. `tooltip` is the shorthand that also draws the string and hover-tracks the node; this is the description alone, for a hint that is spoken and never drawn. Both write the one slot, so a node declaring both keeps whichever its binding applied last. It reads only on a node that reaches the access tree — a role, a label, a control — since a plain box is elided and takes its description with it. */
  description?: string;
  /** Inert: no click, drag or key sink, no hover / pressed / focus background, skipped by Tab, reported disabled to assistive technology; hover tracking stays so a `tooltip` can say why. */
  disabled?: boolean;
  /** Easing for `transition` (default easeOut); spring/bouncy integrate with momentum. */
  easing?: 'easeOut' | 'linear' | 'easeIn' | 'easeInOut' | 'spring' | 'bouncy';
  /** Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in). */
  enter?: EnterProp;
  /** Where the node ends the frame after the view stops declaring it `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }` — an `enter` read the other way. With a `transition`, the departing subtree is copied out of the last frame that had it and replayed frozen, in its place (the pass it painted in, just under the node that painted after it — a panel under a HUD leaves under it) and inert (no clicks, no Tab stop, no access row) while those slots ease from where they were, then dropped; without one it vanishes at once as it always did. `width`/`height` resize the departing node's own box only — the subtree inside it is a picture and is not laid out again. Needs a stable key across frames. */
  exit?: EnterProp;
  /** A disclosure's state: what a node that shows and hides something (a twisty, an accordion header, a menu button) reads as. Unset, the node does not expand at all — which is why this names its state instead of being a flag. */
  expanded?: 'collapsed' | 'expanded';
  /** Background while the node holds keyboard-visible focus (moved there by Tab or assistive technology, not a click); replaces the default focus ring. Pressed wins over focus wins over hover; eases with `transition`. */
  focusBg?: ColorProp;
  /** Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already. */
  focusable?: boolean;
  /** Space between children along the main axis. */
  gap?: number;
  /** Vertical size: px | "fit" | "grow" | "N%". */
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
  /** With `onKey`: releases arrive too, as the same payload with phase:"up" (`text` null, `repeat` false) — for a held-key interaction (WASD, press-and-hold, a key that arms a mode while it is down). A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so nothing is left stuck down. Without it a sink hears presses only, which is what a keymap wants — one that heard both halves would run every binding twice. */
  keyUp?: boolean;
  /** CSS-style stops `[{ at?, width?, height?, bg?, radius?, opacity? }, …]`: the slots they name cycle through them over `transition` ms, forever, without the view redrawing; `at` is 0..1 and spreads evenly when omitted. */
  keyframes?: KeyframeProp[];
  /** The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image, an icon-only button and a `modal` dialog have none, and the core warns (`image-without-label`, `control-without-name`, `modal-without-name`). */
  label?: string;
  /** Marks this node a live region: when the text inside it changes, a screen reader reads the change without being asked — `polite` at the next pause, `assertive` interrupting. Put it on the smallest node that holds the message, since everything inside a live node is live. For a one-off with no node behind it ("Saved") the binding's `announce` verb is the other half. */
  live?: 'off' | 'polite' | 'assertive';
  /** Child alignment along the main axis. */
  mainAlign?: 'start' | 'center' | 'end';
  /** Upper height clamp (logical px). */
  maxHeight?: number;
  /** Upper width clamp; grow+maxWidth is the responsive-width pattern. */
  maxWidth?: number;
  /** Lower height clamp: logical px, or "fit" for the node's own fit height (see `minWidth`). */
  minHeight?: MinProp;
  /** Lower width clamp: logical px, or "fit" for the node's own fit width. "fit" under `width="grow"` is a content floor — CSS's `flex: 1 0 auto` — which is what an i3-style tab bar is: tabs that split the bar evenly while they fit and sit at their label's width, scrolling, once they do not. Opt-in, because a fit width is the unwrapped one: a paragraph in a grow column would stop wrapping under it. */
  minWidth?: MinProp;
  /** Modal surface: the Tab ring becomes this node's subtree, everything outside it is inert to the pointer, the wheel and assistive technology, and Escape or a press outside emits {kind:"dismiss", reason:"escape"|"outside", tag} on it — the app stops declaring the node. The last one declared in tree order is the one in effect (a confirm inside a dialog); a modal that must cover the app is a float. The access tree is not pruned to the modal: it keeps every node of the frame and marks the one in effect `modal` (`docs/adr/0003-modal-surfaces.md`, decision 7), which is what assistive technology acts on. */
  modal?: AppMsg | null;
  /** Message emitted when clicked (data, not a callback). */
  onClick?: AppMsg;
  /** Context-menu tag: a secondary-button (right) press emits {kind:"contextmenu", x, y, tag} on the node, at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it; the topmost node under the pointer is the one asked, as for a click. */
  onContextMenu?: AppMsg | null;
  /** Drag tag: emits {kind:"drag", phase, x, y, dx, dy, parent, tag} events, `dx`/`dy` measured from the press point in every phase. */
  onDrag?: AppMsg | null;
  /** Hover tag: the pointer entering/leaving emits {kind:"hover", phase:"enter"|"leave", tag} events. */
  onHover?: AppMsg | null;
  /** Key-sink tag: with key focus held, presses arrive as {kind:"key", phase:"down", code, ...} events. Releases only with `keyUp` beside it. */
  onKey?: AppMsg | null;
  /** Layout tag: the node's laid-out rect arrives as {kind:"layout", x, y, w, h, parent, tag} on its first frame and whenever it changes (needs a stable key). */
  onLayout?: AppMsg | null;
  /** Group opacity 0..1 (default 1): fades this node and its whole subtree. A per-quad alpha multiply rather than an offscreen composite, so overlapping pieces of one subtree show their seams through the fade. Layout, hit-testing and the access tree are untouched; eases with `transition`, and `enter: { opacity: 0 }` fades a panel in. */
  opacity?: number;
  /** Background while pressed (or while its hoverGroup is); implies hover tracking. */
  pressedBg?: ColorProp;
  /** Corner radius for all four corners (logical px); the per-corner props override it when listed after it. On a node that also clips or scrolls it rounds the clip as well, so children stay inside the corners. */
  radius?: number;
  /** Bottom-left corner radius (logical px). */
  radiusBL?: number;
  /** Bottom-right corner radius (logical px). */
  radiusBR?: number;
  /** Top-left corner radius (logical px). */
  radiusTL?: number;
  /** Top-right corner radius (logical px). */
  radiusTR?: number;
  /** How `keyframes` cycle (CSS `animation-direction`, default normal). Lua: `direction`, since `repeat` is a keyword. */
  repeat?: 'normal' | 'reverse' | 'alternate' | 'alternateReverse';
  /** What the node is to assistive technology. Unset, the core derives one (an `onClick` node is a button, an editor a text input, a scrolling box a scroll view, a plain box nothing); `none` hides the node and its subtree from the access tree. */
  role?: 'none' | 'button' | 'checkbox' | 'radio' | 'switch' | 'slider' | 'tab' | 'tabList' | 'link' | 'heading' | 'list' | 'listItem' | 'image' | 'dialog' | 'group' | 'textInput' | 'multilineTextInput' | 'line' | 'radioGroup' | 'menu' | 'menuItem' | 'terminal';
  /** The current one of a set: which `tab` a `tabList` shows, which `listItem` a list has picked, which `link` is the page you are on. A `tab` always carries the state — its siblings read as "not selected" — while a list row or a link carries it only where it is set, since an ordinary list or navigation bar is not a selection and a reader saying "not selected" on every row of it is noise. */
  selected?: boolean;
  /** On a `line` of a custom editor: the byte offset where the selection's other end sits (the caret is `caret`, possibly on another line). */
  selectionAnchor?: number;
  /** Drop-shadow blur radius (logical px): the edge ramps over this distance and reaches this far past the shape. 0 = a hard edge. */
  shadowBlur?: number;
  /** Drop-shadow color; nothing else about a shadow draws without it. On its own it is a hard shadow exactly behind the node — add `shadowBlur` / `shadowY` to lift it. Outer shadows only, and the shape is not knocked out of the middle, so a translucent background shows it through. */
  shadowColor?: ColorProp;
  /** Grows (or, negative, shrinks) the drop shadow's shape before blurring (logical px). */
  shadowSpread?: number;
  /** Drop-shadow horizontal offset (logical px). */
  shadowX?: number;
  /** Drop-shadow vertical offset (logical px); positive casts downward. */
  shadowY?: number;
  /** With transition: also ease the node's position (reordered siblings slide). While it eases, the node is drawn between where it was and where this frame put it — not at the declared `dx`/`dy`, or its slot in the row — so anything else positioned from those numbers drifts for the transition's length: a canvas of floats eases everything or nothing. */
  slide?: boolean;
  /** Animate sizing/colors/radius changes over this many ms (needs a stable key). */
  transition?: number;
  /** A `slider` role's maximum. */
  valueMax?: number;
  /** A `slider` role's minimum. */
  valueMin?: number;
  /** A `slider` role's current value (the drawing stays yours; this is what assistive technology reads). */
  valueNow?: number;
  /** What a `slider` role's position reads as (ARIA's `aria-valuetext`). Without one a reader has only `valueNow` and the range and says a percentage — 25 in [5..60] is "36 percent" — so a value whose unit carries the meaning says it here: "25 minutes". It replaces the number in the reading rather than joining it, and a nudge announces the new text. Meaningful on the slider role alone, like the three numbers; putting the reading in `label` instead renames the control on every nudge, which is the wrong attribute. */
  valueText?: string;
  /** Horizontal size: px | "fit" | "grow" | "N%". */
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
  /** Font family. */
  family?: 'sans' | 'serif' | 'mono';
  /** OpenType features for the shaper, as `tag=value` pairs separated by spaces or commas — a bare `tag` is 1, `-tag` is 0: `"liga=0 calt=0"` keeps a coding font from joining `->` and `!=` (what a terminal built on runs needs to hold its grid), `"tnum"` lines figures up in a gutter, `"ss01"` picks a stylistic set. Unset, the font's own defaults apply. At most 8; part of what the text is shaped as, so two texts differing only here are shaped twice. */
  features?: string;
  /** A registered font handle (addFont / addSystemFont); overrides `family`. */
  font?: string;
  /** Line height (logical px); default size * 1.35. */
  lineHeight?: number;
  /** Lay out at most this many lines (0 = unlimited); with `ellipsis`, a line clamp. */
  maxLines?: number;
  /** A line through the text, where the face puts its strikeout. Paint only; on a `<span>` the span alone, per line. */
  strikethrough?: boolean;
  /** A line under the text, where the face puts its underline and as thick as it says, in the text colour. Paint only. On a `<span>` it covers the span alone and follows it across a wrap, one rect per line. */
  underline?: boolean;
  /** Line breaking at the node's width: between words (default), anywhere, or never (one line per paragraph, clipped to the node). */
  wrap?: 'word' | 'glyph' | 'none';
}
// -- end generated --

/** Composite/constructor props with hand-written handling on both sides of
 *  the boundary (everything else comes from the generated schema types). */
export interface CustomSpecProps {
  dir?: 'row' | 'column';
  pad?: number;
  padX?: number;
  padY?: number;
  padL?: number;
  padR?: number;
  padT?: number;
  padB?: number;
  borderW?: number;
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
}

export interface BoxProps extends Keyed, GeneratedSpecProps, CustomSpecProps {
  /** Root box only: declares this frame's window title. */
  title?: string;
  /** Root box only: which windows exist besides the main one (see
   *  `WindowDecl` in `@qxuken/kui`). `runWindowed` / `createApp` write it
   *  from the loop config's `windows(model)`; a view driving a `Ctx` by
   *  hand declares it here. */
  windows?: (string | { name: string; width?: number; height?: number; activates?: boolean })[];
  children?: KuiNode;
}

export interface TextProps extends Keyed, GeneratedStyleProps {
  size?: number;
  children?: KuiNode;
}

export interface SpanProps extends Keyed {
  bold?: boolean;
  italic?: boolean;
  /** Overrides the paragraph color; nested spans inherit. */
  color?: ColorProp;
  /** A line under the span, where the face puts it; nested spans inherit. */
  underline?: boolean;
  /** A line through the span, where the face puts it; nested spans inherit. */
  strikethrough?: boolean;
  /** A background behind the span's glyphs alone — one rect per line it
   *  spans, so it follows the span across a wrap the way a box cannot. */
  bg?: ColorProp;
  children?: KuiNode;
}

/** The stock button (`widgets::button_spec`: padding, colours, radius,
 *  hover and pressed backgrounds). Its look is its own, so the layout and
 *  paint rows are not here — declared anyway they are dropped with an
 *  `unknown-prop` warning — and what is here are the rows a reader hears:
 *  `label` when the text is not the name, `description` for what the
 *  button will do, `tooltip`, and `disabled` (inert, and dimmed to half).
 *  A button that needs any other row is a `<box role="button">` with the
 *  same rows spelled out. Keyed by its text unless `key` says otherwise. */
export interface ButtonProps
  extends Keyed,
    Pick<GeneratedSpecProps, 'label' | 'description' | 'disabled'>,
    Pick<CustomSpecProps, 'tooltip'> {
  /** Message emitted on click. */
  onClick?: AppMsg;
  children?: KuiNode;
}

export interface EditProps extends TextProps, GeneratedSpecProps, CustomSpecProps {
  /** Stable identity (state is retained by key); `key` works too. */
  id?: string;
  initial?: string;
  multiline?: boolean;
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
    edit: EditProps;
    /** Styled run inside a rich <text>: bold/italic/color, nestable. */
    span: SpanProps;
    /** A registered image (id from addImage). Fit sizing = pixel size as
     *  logical px; Fit height against a resolved width keeps the aspect. */
    image: Omit<BoxProps, 'children'> & { src: string };
    /** A round-capped stroke (docs/adr/0010-a-segment-primitive.md): one
     *  segment from `from` to `to`, a polyline through `points`, or a smooth
     *  curve through them with `curve`. Points are in the parent's box space
     *  (`float="viewport"` for viewport space). Never in layout: it floats,
     *  sized to its own bounding box, so it takes no room in a row or
     *  column. `width` is the stroke width in px (default 1), `color` the
     *  stroke colour (default the foreground); `transition` eases the colour,
     *  and with `slide` beside it the stroke's position too — the points ride
     *  its box, so a stroke whose ends all move together slides with them,
     *  while one whose ends move apart resizes at once. A canvas of floats
     *  eases everything or nothing, connectors included.
     *  Takes no pointer input and has no access row. */
    line: Keyed &
      Pick<GeneratedSpecProps, 'opacity' | 'transition' | 'slide' | 'enter' | 'exit' | 'onLayout' | 'label' | 'role'> & {
        from?: [number, number];
        to?: [number, number];
        points?: [number, number][];
        curve?: boolean;
        width?: number;
        color?: ColorProp;
        float?: 'parent' | 'viewport';
      };
    /** Adaptive titlebar (drag strip + window buttons per env facts).
     *  `title` alone draws the standard title; children host custom content. */
    titlebar: Keyed & { title?: string; children?: KuiNode };
    /** Just the min/max/close buttons, for fully custom titlebars. */
    windowButtons: Keyed;
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
     *  own, a released playback included. Draws nothing and takes no space. */
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
        cells: Uint32Array | number[];
        cursorAt?: [number, number];
        cursorShape?: 'block' | 'bar' | 'underline';
        cursorColor?: ColorProp;
      };
  }
}
