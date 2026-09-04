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
  anchor?: 'parent' | 'viewport';
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
  /** Inert: no click, drag or key sink, no hover / pressed / focus background, skipped by Tab, reported disabled to assistive technology; hover tracking stays so a `tooltip` can say why. */
  disabled?: boolean;
  /** Easing for `transition` (default easeOut); spring/bouncy integrate with momentum. */
  easing?: 'easeOut' | 'linear' | 'easeIn' | 'easeInOut' | 'spring' | 'bouncy';
  /** Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in). */
  enter?: EnterProp;
  /** A disclosure's state: what a node that shows and hides something (a twisty, an accordion header, a menu button) reads as. Unset, the node does not expand at all — which is why this names its state instead of being a flag. */
  expanded?: 'collapsed' | 'expanded';
  /** Background while the node holds keyboard-visible focus (moved there by Tab or assistive technology, not a click); replaces the default focus ring. Pressed wins over focus wins over hover; eases with `transition`. */
  focusBg?: ColorProp;
  /** Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already. */
  focusable?: boolean;
  /** Space between children along the main axis. */
  gap?: number;
  /** Cross-axis size: px | "fit" | "grow" | "N%". */
  height?: SizingProp;
  /** Background while hovered (or while any node in its hoverGroup is); implies hover tracking, eases with `transition`. */
  hoverBg?: ColorProp;
  /** Nodes sharing a group name show hoverBg/pressedBg together (a split button, a multi-piece shape). */
  hoverGroup?: string;
  /** A registered sound (addSound) played when the pointer enters the node; implies hover tracking. */
  hoverSound?: string;
  /** Hover-track without a click payload (for isHovered-driven styling). */
  hoverable?: boolean;
  /** CSS-style stops `[{ at?, width?, height?, bg?, radius?, opacity? }, …]`: the slots they name cycle through them over `transition` ms, forever, without the view redrawing; `at` is 0..1 and spreads evenly when omitted. */
  keyframes?: KeyframeProp[];
  /** The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image or an icon-only button has none, and the core warns (`image-without-label`, `control-without-name`). */
  label?: string;
  /** Child alignment along the main axis. */
  mainAlign?: 'start' | 'center' | 'end';
  /** Upper height clamp (logical px). */
  maxHeight?: number;
  /** Upper width clamp; grow+maxWidth is the responsive-width pattern. */
  maxWidth?: number;
  /** Lower height clamp (logical px). */
  minHeight?: number;
  /** Lower width clamp (logical px). */
  minWidth?: number;
  /** Modal surface: the Tab ring becomes this node's subtree, everything outside it is inert to the pointer, the wheel and assistive technology, and Escape or a press outside emits {kind:"dismiss", reason:"escape"|"outside", tag} on it — the app stops declaring the node. The last one declared in tree order is the one in effect (a confirm inside a dialog); a modal that must cover the app is a float. */
  modal?: AppMsg | null;
  /** Message emitted when clicked (data, not a callback). */
  onClick?: AppMsg;
  /** Context-menu tag: a secondary-button (right) press emits {kind:"contextmenu", x, y, tag} on the node, at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it; the topmost node under the pointer is the one asked, as for a click. */
  onContextMenu?: AppMsg | null;
  /** Drag tag: emits {kind:"drag", phase, x, y, dx, dy, parent, tag} events. */
  onDrag?: AppMsg | null;
  /** Hover tag: the pointer entering/leaving emits {kind:"hover", phase:"enter"|"leave", tag} events. */
  onHover?: AppMsg | null;
  /** Key-sink tag: with key focus held, presses and releases arrive as {kind:"key", phase:"down"|"up", ...} events. */
  onKey?: AppMsg | null;
  /** Layout tag: the node's laid-out rect arrives as {kind:"layout", x, y, w, h, parent, tag} on its first frame and whenever it changes (needs a stable key). */
  onLayout?: AppMsg | null;
  /** Group opacity 0..1 (default 1): fades this node and its whole subtree. A per-quad alpha multiply rather than an offscreen composite, so overlapping pieces of one subtree show their seams through the fade. Layout, hit-testing and the access tree are untouched; eases with `transition`, and `enter: { opacity: 0 }` fades a panel in. */
  opacity?: number;
  /** Background while pressed (or while its hoverGroup is); implies hover tracking. */
  pressedBg?: ColorProp;
  /** Corner radius for all four corners (logical px); the per-corner props override it when listed after it. */
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
  role?: 'none' | 'button' | 'checkbox' | 'radio' | 'switch' | 'slider' | 'tab' | 'tabList' | 'link' | 'heading' | 'list' | 'listItem' | 'image' | 'dialog' | 'group' | 'textInput' | 'multilineTextInput' | 'line';
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
  /** With transition: also ease the node's position (reordered siblings slide). */
  slide?: boolean;
  /** Animate sizing/colors/radius changes over this many ms (needs a stable key). */
  transition?: number;
  /** A `slider` role's maximum. */
  valueMax?: number;
  /** A `slider` role's minimum. */
  valueMin?: number;
  /** A `slider` role's current value (the drawing stays yours; this is what assistive technology reads). */
  valueNow?: number;
  /** Main-axis size: px | "fit" | "grow" | "N%". */
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
  /** A registered font handle (addFont / addSystemFont); overrides `family`. */
  font?: string;
  /** Line height (logical px); default size * 1.35. */
  lineHeight?: number;
  /** Lay out at most this many lines (0 = unlimited); with `ellipsis`, a line clamp. */
  maxLines?: number;
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
  children?: KuiNode;
}

export interface ButtonProps extends Keyed {
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
     *  changed `src` restarts. `tag` comes back as a `SoundMsg` when it ends
     *  on its own. Draws nothing and takes no space. */
    audio: Keyed & { src: string; loop?: boolean; volume?: number; paused?: boolean; tag?: AppMsg };
  }
}
