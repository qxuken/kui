import type { AppMsg, KuiNode, TextProps } from './jsx-runtime.js';

export type { KuiNode, KuiElement, Msg, KuiMsg, AppMsg } from './jsx-runtime.js';

// -- the messages the core itself sends ------------------------------------
// Payload shapes from `EVENTS` in crates/kui-core/src/schema.rs (the table
// docs/props.md is generated from). `tag` is the payload declared on the
// node (`onDrag` / `onHover` / `onKey`), left off when the node declared
// none.

/** A pointer-captured drag on an `onDrag` node; `parent` is the container
 *  rect, so fractions need no geometry query. */
export type DragMsg<T = AppMsg> = {
  kind: 'drag';
  phase: 'start' | 'move' | 'end';
  x: number;
  y: number;
  dx: number;
  dy: number;
  parent: { x: number; y: number; w: number; h: number };
  tag?: T;
};

/** A key press on the focused `onKey` sink; `code` is a character or a name
 *  ("left", "f5"), `text` what the press would insert (null for chords). */
export type KeyMsg<T = AppMsg> = {
  kind: 'key';
  code: string;
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  super: boolean;
  text: string | null;
  repeat: boolean;
  tag?: T;
};

/** A secondary-button (right) press on an `onContextMenu` node — on the
 *  press, not the release. `x`/`y` are logical viewport coordinates: where
 *  the menu goes. The core opens nothing; declare the menu as a `modal`
 *  float and stop declaring it on `dismiss`. */
export type ContextMenuMsg<T = AppMsg> = {
  kind: 'contextmenu';
  x: number;
  y: number;
  tag?: T;
};

/** The pointer entered or left an `onHover` node — also when a new frame
 *  moved it under a still cursor. */
export type HoverMsg<T = AppMsg> = {
  kind: 'hover';
  phase: 'enter' | 'leave';
  tag?: T;
};

/** The rect layout gave an `onLayout` node — logical px, viewport
 *  coordinates, after scrolling and position easing — on its first frame
 *  and whenever it changes, never on a frame that left it alone (a
 *  transition that moves the node reports every frame it moves). `parent`
 *  is the container rect, as on drags. Layout's numbers, so the view never
 *  re-derives them by hand. */
export type LayoutMsg<T = AppMsg> = {
  kind: 'layout';
  x: number;
  y: number;
  w: number;
  h: number;
  parent: { x: number; y: number; w: number; h: number };
  tag?: T;
};

/** The frame's `modal` node was asked to go away: Escape, or a press that
 *  landed outside it. The core closes nothing — stop declaring the node
 *  (or ask first). Only the modal in effect gets one. */
export type DismissMsg<T = AppMsg> = {
  kind: 'dismiss';
  reason: 'escape' | 'outside';
  tag?: T;
};

/** The viewport changed size or DPI (logical px, delivered on the root);
 *  `win.size()` queries the same numbers. */
export type ResizeMsg = {
  kind: 'resize';
  width: number;
  height: number;
  scale: number;
};

/** The physical modifier state changed (delivered on the root). */
export type ModifiersMsg = {
  kind: 'modifiers';
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  super: boolean;
};

/** An editor's text changed / Enter in a single-line editor; the editor's
 *  key is on the event, so `editText(ev.key)` reads it back. */
export type EditMsg = { kind: 'changed' } | { kind: 'submit' };

/** A tagged playback (`play(id, { tag })` or `<audio tag>`) finished on its
 *  own — never when something stopped it. On an `<audio>` node's key, or the
 *  root for `play`. */
export type SoundMsg<T = AppMsg> = {
  kind: 'sound';
  phase: 'ended';
  playback: number;
  tag: T;
};

/** Everything the core sends on its own. Put it in the app's union —
 *  `type Msg = MyMsg | CoreMsg` — and `update` switches over one flat
 *  discriminated union, no casts and no narrowing preamble. */
export type CoreMsg =
  | DragMsg
  | KeyMsg
  | ContextMenuMsg
  | HoverMsg
  | LayoutMsg
  | DismissMsg
  | ResizeMsg
  | ModifiersMsg
  | EditMsg
  | SoundMsg
  | AccessMsg;

/** Assistive technology nudged a `slider` role. `tag` is the node's
 *  `onClick` payload (or its `onDrag` / `onKey` tag). Activation, focus,
 *  text and scrolling requests resolve in the core and arrive as the
 *  messages a pointer would have produced. */
export interface AccessMsg {
  kind: 'access';
  /** `increment` / `decrement` on a `slider` role; `setValue`,
   *  `replaceSelectedText` (with `text`) and `setTextSelection` (with
   *  `anchor` / `focus`) on a custom editor — an `onKey` sink declared
   *  `role="multilineTextInput"` that draws its own `role="line"` rows. */
  action: 'increment' | 'decrement' | 'setValue' | 'replaceSelectedText' | 'setTextSelection';
  text?: string;
  /** Line ordinals among the drawn `role="line"` rows and byte offsets
   *  into their text. */
  anchor?: { line: number; offset: number };
  focus?: { line: number; offset: number };
  tag?: unknown;
}

/** A position in an editor's text: one of its `runs` and a character
 *  index into it (the run's character count is its end). */
export interface TextPos {
  run: string;
  character: number;
}

/** One laid-out line (or a piece of one) of an editor's text, with every
 *  character placed — what a screen reader reads by character and word.
 *  A line that continues ends with its `"\n"`, a character of no width. */
export interface AccessRun {
  key: string;
  /** The line it belongs to and its byte range in that line's text. */
  line: number;
  start: number;
  end: number;
  text: string;
  rect: { x: number; y: number; w: number; h: number };
  charLengths: number[];
  /** Each character's x relative to `rect.x`, and its width. */
  charPositions: number[];
  charWidths: number[];
  /** Character indices where words start. */
  wordStarts: number[];
  rtl: boolean;
}

/** The argument of `access(key, 'setTextSelection', …)`: the end that
 *  stays (`anchor`) and the caret (`focus`); `text` rides along for the
 *  text actions when given as an object. */
export interface AccessArg {
  anchor?: TextPos;
  focus?: TextPos;
  text?: string;
}

/** What a node is to assistive technology. The first group can be declared
 *  with the `role` prop; the rest the core derives (an `onClick` box is a
 *  button, an editor a text input, a scrolling box a scroll view, the root
 *  the window). */
export type AccessRole =
  | 'none' | 'button' | 'checkbox' | 'radio' | 'switch' | 'slider' | 'tab'
  | 'tabList' | 'link' | 'heading' | 'list' | 'listItem' | 'image' | 'dialog'
  | 'group' | 'textInput' | 'multilineTextInput' | 'line'
  | 'window' | 'titleBar' | 'staticText' | 'scrollView';

/** What assistive technology can ask of a node (`access(key, action)`). */
export type AccessAction =
  | 'click' | 'focus' | 'blur' | 'setValue' | 'increment' | 'decrement'
  | 'scrollIntoView' | 'scrollUp' | 'scrollDown' | 'scrollLeft' | 'scrollRight'
  | 'setTextSelection' | 'replaceSelectedText';

/** One semantic node of a frame. Plain boxes are elided, so `parent` is
 *  the nearest semantic ancestor. */
export interface AccessNode {
  key: string;
  parent: string | null;
  origin: number;
  role: AccessRole;
  /** `label`, else the node's own text, else (for buttons, links, tabs,
   *  headings) the text inside it, else the window title for the root. */
  name: string | null;
  /** What the `tooltip` prop sets. */
  description: string | null;
  /** Logical px, viewport coordinates. */
  rect: { x: number; y: number; w: number; h: number };
  /** An editor's text, with its caret and non-empty selection as byte offsets. */
  value: string | null;
  caret: number | null;
  selection: [number, number] | null;
  /** An editor's laid-out text, run by run; empty for anything else. */
  runs: AccessRun[];
  /** The caret (`focus`) and the selection's other end (`anchor`, equal
   *  to `focus` without a selection) as run positions. */
  anchor: TextPos | null;
  focus: TextPos | null;
  /** `checked` for checkbox / radio / switch roles. */
  checked: boolean | null;
  /** `valueNow` / `valueMin` / `valueMax` for a slider. */
  valueNow: number | null;
  valueMin: number | null;
  valueMax: number | null;
  /** Holds keyboard focus (`focused()` names the same node). */
  focused: boolean;
  /** Declared `disabled`: inert, and not a Tab stop. */
  disabled: boolean;
  /** The frame's `modal` surface (`aria-modal`): focus and input are
   *  confined to its subtree, everything else is inert. Only the modal in
   *  effect — the last one declared — carries it. */
  modal: boolean;
  scroll: { x: number; y: number; maxX: number; maxY: number } | null;
  /** The requests this node accepts. */
  actions: AccessAction[];
}

/** The semantic nodes of a frame in tree order (root first) — what a
 *  screen reader sees, as data. Assert on it in tests. */
export interface AccessTree {
  nodes: AccessNode[];
  /** The node holding keyboard focus. */
  focus: string | null;
  /** Changes when anything above does. */
  hash: string;
}

/** What text measures (`measureText`): logical px at the scale of the
 *  current or last frame; `lines` after wrapping. The same numbers layout
 *  gives a `<text>` with that content and style. */
export interface TextMetrics {
  width: number;
  height: number;
  lines: number;
}

/** A silent misconfiguration the core noticed while finishing a frame —
 *  the kind that otherwise looks like "the feature is broken". Each
 *  distinct (code, node) pair is raised once. */
export interface Warning {
  /** Stable: match on it. `grow-weight-ignored` — a `{ grow: n }` with
   *  nothing to split against (the only grow child, or across the parent's
   *  main axis); `transition-auto-key` — a node's child count changed while
   *  an unkeyed child carries a `transition`, so the shifted children
   *  snapped (give list items a `key`); `duplicate-key` — two nodes share
   *  a key in one frame. */
  code:
    | 'grow-weight-ignored'
    | 'transition-auto-key'
    | 'duplicate-key'
    /** An `<image>` with no `label` (decorative ones take `role="none"`). */
    | 'image-without-label'
    /** A button, link, tab, checkbox, slider or editor with no `label` and
     *  no text inside it: a screen reader announces an unnamed control. */
    | 'control-without-name'
    | (string & {});
  /** The node it is about (hex, like event keys). */
  key: string;
  message: string;
}

/** Options for `play`. Volumes are linear amplitude (0..1), durations ms. */
export interface PlayOptions {
  volume?: number;
  loop?: boolean;
  fadeIn?: number;
  /** Asks for a `SoundMsg` when the playback ends on its own. */
  tag?: AppMsg;
}

/** What a driver plays; `Ctx.audioCommands()` drains them (headless), a
 *  `KuiWindow` plays them itself. */
export type AudioCommand =
  | { kind: 'play'; playback: number; sound: string; volume: number; loop: boolean; fadeIn: number }
  | { kind: 'stop'; playback: number; fade: number }
  | { kind: 'setVolume'; playback: number; volume: number; tween: number }
  | { kind: 'pause'; playback: number; fade: number }
  | { kind: 'resume'; playback: number; fade: number }
  | { kind: 'masterVolume'; volume: number; tween: number }
  | { kind: 'unload'; sound: string };

// --------------------------------------------------------------------------

/** One event out of the loop. `A` is the app's message union; it defaults to
 *  the registered `AppMsg` plus the core's own messages. */
export interface UiEvent<A = AppMsg | CoreMsg> {
  origin: number;
  /** Node key as a hex string; pass back to editText()/isFocused()/... */
  key: string;
  payload: A;
}

/** Window inner size in logical px plus the device pixel ratio. */
export interface WindowSize {
  width: number;
  height: number;
  scale: number;
}

export interface FrameStats {
  quadCount: number;
  viewportW: number;
  viewportH: number;
  scale: number;
  atlasSize: number;
}

/** One frame's cost in ms, split by phase (what the latency HUD draws). */
export interface FrameSample {
  /** Input routing and edits since the previous frame. */
  inputMs: number;
  /** The view (tree lowering). */
  viewMs: number;
  /** Layout, text measurement and display-list emission. */
  layoutMs: number;
  /** GPU encode and present. */
  renderMs: number;
  /** Blocked on a swapchain image (vsync pacing, not work). */
  waitMs: number;
  totalMs: number;
  /** Everything but `waitMs`. */
  workMs: number;
}

/** The window's frame-timing ring: the last 120 frames. */
export interface FrameTiming {
  frames: number;
  /** Null before the first frame. */
  last: FrameSample | null;
  avgTotalMs: number;
  maxTotalMs: number;
  avgWorkMs: number;
  maxWorkMs: number;
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

export type MouseButtonName = 'primary' | 'secondary' | 'middle';

export type EditKeyName =
  | 'left' | 'right' | 'up' | 'down'
  | 'home' | 'end' | 'pageup' | 'pagedown'
  | 'backspace' | 'delete' | 'enter' | 'tab'
  | 'selectall' | 'escape' | 'undo' | 'redo';

/** A headless kui core: build frames from JSX trees, feed input, poll events. */
/** A scroll container's retained offset, in logical px: positive means the
 *  content has moved up / left inside it. */
export interface ScrollOffset {
  x: number;
  y: number;
}

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
  /** A button press or release. `button` defaults to 'primary'; only that
   *  one presses, drags, places the caret and clicks. 'secondary' asks the
   *  node under the pointer for a context menu and moves nothing else;
   *  nothing routes 'middle' yet. */
  mouse(down: boolean, clicks?: number, button?: MouseButtonName): void;
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
  /** Registers a font from file bytes (TTF/OTF/TTC); returns its id for the
   *  `font` prop on `<text>` / `<edit>`. Throws when no usable face is found. */
  addFont(data: Buffer): string;
  /** The id for a font family by name — installed, or loaded with
   *  `loadFontsDir` / `loadFontFile`; null when none matches. The same
   *  family always gets the same id. */
  addSystemFont(name: string): string | null;
  /** Registers a font file by path (memory-mapped); throws when it cannot be
   *  read or holds no usable face. */
  loadFontFile(path: string): string;
  /** Loads every font file under a folder (recursively) so its families can
   *  be picked by name with `addSystemFont`; returns the face count. */
  loadFontsDir(dir: string): number;
  removeFont(id: string): void;
  /** Family names of every font the core can see (sorted). */
  systemFontFamilies(): string[];
  /** Registers a sound from its encoded bytes (wav/ogg/mp3/flac); returns
   *  its id for `<audio src>`, `clickSound` / `hoverSound`, and `play`. */
  addSound(data: Buffer): string;
  removeSound(id: string): void;
  /** Starts a playback; returns its id for stop/setVolume/pause/resume.
   *  Headless, nothing plays: the command queues for `audioCommands()`. */
  play(sound: string, opts?: PlayOptions): number;
  stop(playback: number, fadeMs?: number): void;
  setVolume(playback: number, volume: number, tweenMs?: number): void;
  pause(playback: number, fadeMs?: number): void;
  resume(playback: number, fadeMs?: number): void;
  setMasterVolume(volume: number, tweenMs?: number): void;
  /** Drains the audio commands the core queued (tests, custom drivers). */
  audioCommands(): AudioCommand[];
  /** A custom driver reports a playback finished on its own; a tagged one
   *  becomes a `SoundMsg` in `pollEvents`. */
  audioEnded(playback: number): void;
  /** Events since the last poll. `A` types their payloads — the app's own
   *  union, or one core message type when only that is being watched. */
  pollEvents<A = AppMsg | CoreMsg>(): UiEvent<A>[];
  /** Measures text the way layout would, without adding a node: `content`
   *  is whatever `<text>` takes (a string, or children with `<span>`s),
   *  `style` its props (`size`, `font`, `wrap`, `maxLines`, `ellipsis`, …),
   *  `maxWidth` the width to wrap at. Works before the first frame. Size a
   *  column to its widest label, or pick the tier that fits, from these
   *  numbers instead of constants found by screenshot. */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics;
  /** Drains the warnings the core raised since the last call; `createApp`
   *  collects them on `app.warnings` for you. */
  warnings(): Warning[];
  /** Whether the core runs the checks behind `warnings`. A bare `Ctx` has
   *  them on; `createApp` / `runWindowed` turn them off under
   *  `NODE_ENV=production`. */
  setDiagnostics(on: boolean): void;
  /** The window title the last frame declared (a root `<box title>`), or
   *  null when it declared none. */
  windowTitle(): string | null;
  /** What assistive technology sees of the last frame (see `AccessTree`). */
  accessTree(): AccessTree;
  /** A request from assistive technology on a node: an action it
   *  advertises, with `value` the new text for `setValue`. Resolved like
   *  its pointer/keyboard equivalent, so the events land in `pollEvents`. */
  access(key: string, action: AccessAction, value?: string | AccessArg): void;
  isHovered(key: string): boolean;
  isPressed(key: string): boolean;
  /** Whether a node holds keyboard focus — any node: an editor, an `onKey`
   *  sink, a button Tab landed on. */
  isFocused(key: string): boolean;
  /** The node holding keyboard focus, or null. Tab / Shift-Tab (`key('tab')`)
   *  walk every control in tree order; Enter and Space press the focused
   *  one; the arrows nudge a focused slider. */
  focused(): string | null;
  /** Whether focus got where it is by keyboard or assistive technology
   *  rather than a click — when the focus ring (or `focusBg`) shows. */
  focusVisible(): boolean;
  /** Moves keyboard focus to a node now; `<box keyFocus>` is the
   *  declarative form (it takes focus when it starts being declared). */
  focus(key: string): void;
  blur(): void;
  /** What Tab / Shift-Tab do, as calls — for an `onKey` sink that binds
   *  Tab itself and wants to hand the keyboard on. */
  focusNext(): void;
  focusPrev(): void;
  /** Scrolls whatever contains a node so it shows — "scroll to the selected
   *  row", which needs container geometry only the core has. Resolved
   *  against the *next* frame's layout (one is requested), so a row the
   *  view is about to declare for the first time reveals fine; a key that
   *  frame does not declare, or one with nothing scrollable above it, is a
   *  no-op and is not kept for a later frame. Last reveal before a frame
   *  wins. */
  reveal(key: string): void;
  /** A scroll container's retained offset as the last layout clamped it
   *  (positive = content moved up / left) — stash it in a model and hand it
   *  back to `setScroll`. `{x: 0, y: 0}` for a node that never scrolled. */
  scrollOffset(key: string): ScrollOffset;
  /** Sets that offset the way the wheel would; the next layout clamps it,
   *  so `(0, 0)` jumps to the top and a huge `y` to the end without knowing
   *  the content height. */
  setScroll(key: string, x: number, y: number): void;
  editText(key: string): string | null;
  setEditText(key: string, text: string): void;
  stats(): FrameStats;
  quads(): Buffer;
}

export declare function quadStride(): number;

export interface WindowOptions {
  width?: number;
  height?: number;
  /** Smallest inner size the user may resize the window to (logical px).
   *  The OS enforces it; `width`/`height` are clamped up into it. Either
   *  axis may stand alone — the other stays unbounded. */
  minWidth?: number;
  minHeight?: number;
  /** Largest inner size the user may resize the window to (logical px).
   *  A bound below the matching minimum loses to it. */
  maxWidth?: number;
  maxHeight?: number;
  chrome?: 'native' | 'custom' | 'borderless';
  /** Frame transport: 'binary' (default, fastest) or 'json' (readable, for
   *  debugging encoder suspicions). */
  transport?: 'binary' | 'json';
  /** `false` stops the loop printing the core's warnings (see `Warning`);
   *  `win.warnings()` still drains them. */
  warnings?: boolean;
  /** Whether the core runs the checks at all. Default: on unless
   *  `NODE_ENV` is `production`, so a shipped app pays and prints nothing. */
  diagnostics?: boolean;
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
  /** The window's inner size (logical px) and scale factor. Readable before
   *  the first frame (in `setup`); changes to it also arrive through
   *  `pollEvents` as a `ResizeMsg`. */
  size(): WindowSize;
  /** True when the last frame left a transition mid-flight. The window
   *  schedules its own redraws for that; this is for tests and drivers that
   *  want to know when motion has settled. */
  animating(): boolean;
  /** Summary of the last frame's display list (same shape as `Ctx.stats`). */
  stats(): FrameStats;
  /** Frame timing measured by the window's runner — the latency HUD as
   *  data. `Ctx` has no clock of its own, so this lives on the window. */
  frameStats(): FrameTiming;
  pollEvents<A = AppMsg | CoreMsg>(): UiEvent<A>[];
  close(): void;
  editText(key: string): string | null;
  setEditText(key: string, text: string): void;
  isFocused(key: string): boolean;
  /** Keyboard focus as data, as on `Ctx`: the focused node, whether the
   *  focus shows, and moving it (a move requests a redraw). */
  focused(): string | null;
  focusVisible(): boolean;
  focus(key: string): void;
  blur(): void;
  focusNext(): void;
  focusPrev(): void;
  /** Scrolling as data, as on `Ctx`: reveal a node, or read and write a
   *  container's retained offset (a write requests a redraw). `reveal`
   *  resolves against the next frame's layout — a key that frame does not
   *  declare is a no-op. */
  reveal(key: string): void;
  scrollOffset(key: string): ScrollOffset;
  setScroll(key: string, x: number, y: number): void;
  /** Hover state as of the last frame; keys come from events (an `onHover`
   *  enter, a click). For plain hover styling prefer the `hoverBg` /
   *  `pressedBg` props — the core resolves those without a JS round trip. */
  isHovered(key: string): boolean;
  isPressed(key: string): boolean;
  /** Registers a w×h RGBA image; returns its id for `<image src={id}>`. */
  addImage(width: number, height: number, rgba: Buffer): string;
  removeImage(id: string): void;
  /** Registers a font from file bytes; see `Ctx.addFont`. */
  addFont(data: Buffer): string;
  /** The id for a font family by name; see `Ctx.addSystemFont`. */
  addSystemFont(name: string): string | null;
  loadFontFile(path: string): string;
  loadFontsDir(dir: string): number;
  removeFont(id: string): void;
  systemFontFamilies(): string[];
  /** Registers a sound; see `Ctx.addSound`. */
  addSound(data: Buffer): string;
  removeSound(id: string): void;
  /** Starts a playback on the window's audio device at once; see `Ctx.play`.
   *  A `tag` comes back through `pollEvents` as a `SoundMsg`. */
  play(sound: string, opts?: PlayOptions): number;
  stop(playback: number, fadeMs?: number): void;
  setVolume(playback: number, volume: number, tweenMs?: number): void;
  pause(playback: number, fadeMs?: number): void;
  resume(playback: number, fadeMs?: number): void;
  setMasterVolume(volume: number, tweenMs?: number): void;
  /** Measures text the way layout would; see `Ctx.measureText`. At the
   *  window's scale once a frame has run. */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics;
  /** Drains the core's warnings; `runWindowed` prints them itself unless
   *  opened with `warnings: false`. */
  warnings(): Warning[];
  setDiagnostics(on: boolean): void;
  /** The last frame's access tree; the window hands it to the platform
   *  (AccessKit) by itself — this is for tests and tooling. */
  accessTree(): AccessTree;
  /** See `Ctx.access`; a real screen reader's requests arrive on their own. */
  access(key: string, action: AccessAction, value?: string | AccessArg): void;
}

export interface WindowedConfig<M, A = AppMsg | CoreMsg> {
  init: M | (() => M);
  /** Same contract as AppConfig, plus the window for editText etc. */
  update: (model: M, msg: A, event: UiEvent<A>, win: KuiWindow) => M | undefined | void;
  view: (model: M) => KuiNode;
  /** A clock: every `every` ms the loop feeds `msg` (or `msg(now)`, with
   *  `Date.now()`) to `update`. Ticks are frequent, so unlike UI events they
   *  re-render only when `update` returns a new model — a countdown that
   *  returns undefined until the displayed second changes costs nothing in
   *  between. Read backwards, that is the trap: a tick handler that mutates
   *  the model in place and returns undefined never reaches the screen.
   *  Return the model (any non-undefined return renders) on the ticks that
   *  should draw. */
  tick?: { every: number; msg: A | ((now: number) => A) };
}

/** Opens a window and runs the Elm loop; resolves with the final model on close. */
export declare function runWindowed<M, A = AppMsg | CoreMsg>(
  config: WindowedConfig<M, A>,
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
  /** Corner radii (physical px) clockwise from the top-left: tl, tr, br, bl. */
  radii: [number, number, number, number];
  /** The top-left radius — the uniform value for boxes rounded with `radius`. */
  radius: number;
  borderW: number;
  /** 0 solid, 1 mask glyph, 2 color glyph, 3 image, 4 subpixel glyph. */
  kind: number;
  uv: [number, number, number, number];
  clip: [number, number, number, number];
}

export declare function decodeQuads(buffer: Buffer): Quad[];

/** `M` is the model, `A` every message `update` can see. Annotate `update`
 *  with the app's own union (`type Msg = MyMsg | CoreMsg`) and `A` is
 *  inferred from it; leave it and `A` is the registered `AppMsg` plus the
 *  core's messages. */
export interface AppConfig<M, A = AppMsg | CoreMsg> {
  init: M | (() => M);
  /** Returns the next model; returning undefined keeps the current one. */
  update: (model: M, msg: A, event: UiEvent<A>) => M | undefined | void;
  view: (model: M) => KuiNode;
}

export interface App<M, A = AppMsg | CoreMsg> {
  ctx: Ctx;
  readonly model: M;
  /** Every warning the core raised while rendering, in order; each is also
   *  printed unless created with `warnings: false`. A test asserts it is
   *  empty, or that a specific code showed up. */
  readonly warnings: Warning[];
  dispatch(msg: A, event?: UiEvent<A>): void;
  render(): FrameStats;
  settle(): void;
  click(x: number, y: number, clicks?: number): void;
  /** A secondary-button press and release at `(x, y)`: an `onContextMenu`
   *  node under it gets a `contextmenu` message, and nothing else moves. */
  rightClick(x: number, y: number): void;
  type(text: string): void;
  key(name: EditKeyName, mods?: KeyMods): void;
  /** What assistive technology sees of the last render. */
  accessTree(): AccessTree;
  /** Drives the app the way a screen reader would — `access(key, 'click')`
   *  activates a node, `access(key, 'setValue', text)` types into an
   *  editor — and settles the events that follow through `update`. */
  access(key: string, action: AccessAction, value?: string | AccessArg): void;
}

export declare function createApp<M, A = AppMsg | CoreMsg>(
  config: AppConfig<M, A>,
  opts?: {
    width?: number;
    height?: number;
    scale?: number;
    /** Frame transport, as for `runWindowed`. */
    transport?: 'binary' | 'json';
    /** `false` keeps the core's warnings off the console; they still
     *  collect on `app.warnings`. */
    warnings?: boolean;
    /** Whether the core runs the checks at all; default on unless
     *  `NODE_ENV` is `production`. */
    diagnostics?: boolean;
  },
): App<M, A>;
