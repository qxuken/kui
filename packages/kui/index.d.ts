import type { KuiNode, Msg } from './jsx-runtime.js';

export type { KuiNode, KuiElement, Msg } from './jsx-runtime.js';

export interface UiEvent {
  origin: number;
  /** Node key as a hex string; pass back to editText()/isFocused()/... */
  key: string;
  payload: Msg;
}

export interface FrameStats {
  quadCount: number;
  viewportW: number;
  viewportH: number;
  scale: number;
  atlasSize: number;
}

export interface KeyMods {
  shift?: boolean;
  /** Word-wise motion (alt). */
  word?: boolean;
  /** Document-wise motion (cmd/ctrl). */
  doc?: boolean;
}

export interface KeySinkMods {
  shift?: boolean;
  ctrl?: boolean;
  alt?: boolean;
  /** Command / Windows key. */
  super?: boolean;
}

export type EditKeyName =
  | 'left' | 'right' | 'up' | 'down'
  | 'home' | 'end' | 'pageup' | 'pagedown'
  | 'backspace' | 'delete' | 'enter' | 'tab'
  | 'selectall' | 'escape' | 'undo' | 'redo';

/** A headless kui core: build frames from JSX trees, feed input, poll events. */
export declare class Ctx {
  constructor();
  /** Lowers a JSX tree into one frame. Encodes it to the flat binary IR
   *  stream first (the fastest transport, one zero-copy boundary crossing). */
  frame(width: number, height: number, scale: number, tree: KuiNode): void;
  /** `frame` with the addon walking the JS object graph itself — the
   *  reference transport, ~10x slower (every property read is an N-API call). */
  frameObject(width: number, height: number, scale: number, tree: KuiNode): void;
  /** `frame` with a pre-stringified tree (readable on the wire; the
   *  debugging transport when the encoder is suspect). */
  frameJson(width: number, height: number, scale: number, tree: string): void;
  /** `frame` from an already-encoded binary stream, for callers that own
   *  their encoder (`createEncoder(protocol())`). */
  frameBinary(width: number, height: number, scale: number, stream: Float64Array, strings: Uint8Array): void;
  /** Frame clock for `transition` props (monotonic seconds, any origin).
   *  Set before each frame; never setting it makes transitions snap. */
  setTime(nowSecs: number): void;
  /** True when the last frame left a transition mid-flight. */
  animating(): boolean;
  cursor(x: number, y: number): void;
  cursorLeft(): void;
  mouse(down: boolean, clicks?: number): void;
  scroll(dx: number, dy: number): void;
  text(text: string): void;
  key(name: EditKeyName, mods?: KeyMods): void;
  /** Raw key press for onKey sinks: a single character or a name
   *  ("left", "enter", "f5", ...). */
  keyDown(code: string, mods?: KeySinkMods): void;
  /** Physical modifier state changed; the host gets a
   *  `{kind:"modifiers", shift, ctrl, alt, super}` message when it differs. */
  modifiers(mods?: KeySinkMods): void;
  /** Registers a w×h RGBA image; returns its id for `<image src={id}>`. */
  addImage(width: number, height: number, rgba: Buffer): string;
  removeImage(id: string): void;
  pollEvents(): UiEvent[];
  isHovered(key: string): boolean;
  isPressed(key: string): boolean;
  isFocused(key: string): boolean;
  editText(key: string): string | null;
  setEditText(key: string, text: string): void;
  stats(): FrameStats;
  quads(): Buffer;
}

export declare function quadStride(): number;

export interface WindowOptions {
  width?: number;
  height?: number;
  chrome?: 'native' | 'custom' | 'borderless';
  /** Frame transport: 'binary' (default, fastest) or 'json' (readable, for
   *  debugging encoder suspicions). */
  transport?: 'binary' | 'json';
}

/** The binary-frame protocol tables ({version, op, prop}) from the addon. */
export declare function protocol(): { version: number; op: Record<string, number>; prop: Record<string, number> };

/** A reusable frame encoder for the binary IR path (drivers make their own). */
export declare function createEncoder(p: ReturnType<typeof protocol>): {
  encode(tree: KuiNode): { stream: Float64Array; strings: Uint8Array };
};

/**
 * A real kui window (winit + wgpu) with a pumped event loop. Prefer
 * `runWindowed` unless you're building your own loop. One per process.
 */
export declare class KuiWindow {
  constructor(title: string, options?: WindowOptions);
  /** Stores the tree future redraws lower, and schedules one. Encodes it to
   *  the binary IR stream first (the fastest transport). */
  setView(tree: KuiNode): void;
  /** `setView` with the addon walking the JS object graph itself — the
   *  reference transport, ~10x slower. */
  setViewObject(tree: KuiNode): void;
  /** `setView` with a pre-stringified tree (readable debugging transport). */
  setViewJson(tree: string): void;
  /** `setView` from an already-encoded binary stream. */
  setViewBinary(stream: Float64Array, strings: Uint8Array): void;
  /** Processes pending OS events; false once the window has closed. */
  pump(): boolean;
  pollEvents(): UiEvent[];
  close(): void;
  editText(key: string): string | null;
  setEditText(key: string, text: string): void;
  isFocused(key: string): boolean;
  /** Registers a w×h RGBA image; returns its id for `<image src={id}>`. */
  addImage(width: number, height: number, rgba: Buffer): string;
  removeImage(id: string): void;
}

export interface WindowedConfig<M> {
  init: M | (() => M);
  /** Same contract as AppConfig, plus the window for editText etc. */
  update: (model: M, msg: Msg, event: UiEvent, win: KuiWindow) => M | undefined | void;
  view: (model: M) => KuiNode;
}

/** Opens a window and runs the Elm loop; resolves with the final model on close. */
export declare function runWindowed<M>(
  config: WindowedConfig<M>,
  opts?: WindowOptions & {
    title?: string;
    pumpMs?: number;
    /** Runs after the window opens, before the first frame — register
     *  images and other resources here. */
    setup?: (win: KuiWindow) => void;
  },
): Promise<M>;

export interface Quad {
  x: number; y: number; w: number; h: number;
  color: [number, number, number, number];
  borderColor: [number, number, number, number];
  radius: number;
  borderW: number;
  /** 0 solid, 1 mask glyph, 2 color glyph, 3 image. */
  /** 0 solid, 1 mask glyph, 2 color glyph, 3 image, 4 subpixel glyph. */
  kind: number;
  uv: [number, number, number, number];
  clip: [number, number, number, number];
}

export declare function decodeQuads(buffer: Buffer): Quad[];

export interface AppConfig<M> {
  init: M | (() => M);
  /** Returns the next model; returning undefined keeps the current one. */
  update: (model: M, msg: Msg, event: UiEvent) => M | undefined | void;
  view: (model: M) => KuiNode;
}

export interface App<M> {
  ctx: Ctx;
  readonly model: M;
  dispatch(msg: Msg, event?: UiEvent): void;
  render(): FrameStats;
  settle(): void;
  click(x: number, y: number, clicks?: number): void;
  type(text: string): void;
  key(name: EditKeyName, mods?: KeyMods): void;
}

export declare function createApp<M>(
  config: AppConfig<M>,
  opts?: { width?: number; height?: number; scale?: number },
): App<M>;
