// Types for kui's JSX runtime. Set in tsconfig:
//   "jsx": "react-jsx", "jsxImportSource": "kui"

/** Event message payloads: plain data, both directions (the Elm shape). */
export type Msg = null | boolean | number | string | Msg[] | { [key: string]: Msg };

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
  /** Center children on both axes. */
  center?: boolean;
  /** Child alignment across the main axis. */
  crossAlign?: 'start' | 'center' | 'end';
  /** Space between children along the main axis. */
  gap?: number;
  /** Cross-axis size: px | "fit" | "grow" | "N%". */
  height?: SizingProp;
  /** Hover-track without a click payload (for isHovered-driven styling). */
  hoverable?: boolean;
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
  /** Message emitted when clicked (data, not a callback). */
  onClick?: Msg;
  /** Drag tag: emits {kind:"drag", phase, x, y, dx, dy, parent, tag} events. */
  onDrag?: Msg;
  /** Key-sink tag: with key focus held, presses arrive as {kind:"key", ...} events. */
  onKey?: Msg;
  /** Corner radius (logical px). */
  radius?: number;
  /** Main-axis size: px | "fit" | "grow" | "N%". */
  width?: SizingProp;
  /** Window-chrome role: interactions become window commands, not events. */
  window?: 'drag' | 'close' | 'minimize' | 'maximize';
}

export interface GeneratedStyleProps {
  /** Text color; default foreground when omitted. */
  color?: ColorProp;
  /** Font family. */
  family?: 'sans' | 'serif' | 'mono';
  /** Line height (logical px); default size * 1.35. */
  lineHeight?: number;
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
  /** Grabs key focus declaratively (focused editors still win). */
  keyFocus?: boolean;
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
  onClick?: Msg;
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
  }
}
