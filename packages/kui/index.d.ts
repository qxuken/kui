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

/** A key press or release on the focused `onKey` sink; `code` is a character
 *  or a name ("left", "f5"), `text` what the press would insert (null for a
 *  chord, and on every release), `repeat` set when the OS auto-repeated the
 *  press. A key only comes up where it went down: a release whose press the
 *  sink never got is dropped, and focus leaving while a key is held delivers
 *  the `up` first — so a held-key binding cannot be left stuck down. */
export type KeyMsg<T = AppMsg> = {
  kind: 'key';
  phase: 'down' | 'up';
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
  /** The current one of a set. Every `tab` carries it; a `listItem` or a
   *  `link` only where the view set `selected`. */
  selected: boolean | null;
  /** A disclosure's state, as declared. `null` = it does not expand. */
  expanded: boolean | null;
  /** "3 of 7", derived from the `list` / `tabList` holding this node: the
   *  zero-based ordinal on each item, the count on the container. */
  posInSet: number | null;
  setSize: number | null;
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
    /** A `modal` with no `label`: a dialog is not named by the text inside
     *  it, so a reader announces an unnamed dialog. */
    | 'modal-without-name'
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

/** A scroll container's retained offset, in logical px: positive means the
 *  content has moved up / left inside it. */
export interface ScrollOffset {
  x: number;
  y: number;
}

/** What the last layout resolved for a scroll container: its own box
 *  (`x`/`y`/`w`/`h`, logical px in viewport coordinates), its laid-out
 *  content size (padding included) and the offset it clamped. */
export interface ScrollGeometry {
  x: number;
  y: number;
  w: number;
  h: number;
  contentW: number;
  contentH: number;
  /** Where it is scrolled to — the retained offset clamped to `maxOffset`,
   *  so it is always a position within the content even right after a
   *  `setScroll` of "a huge number" meaning "the end". */
  offset: ScrollOffset;
  /** How far `offset` can travel; zero on an axis that does not scroll.
   *  `offset.y === maxOffset.y` is "at the bottom". */
  maxOffset: ScrollOffset;
}

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
  /** `false` stops the loop printing the core's warnings (see `Warning`);
   *  `win.warnings()` still drains them. */
  warnings?: boolean;
  /** Whether the core runs the checks at all. Default: on unless
   *  `NODE_ENV` is `production`, so a shipped app pays and prints nothing. */
  diagnostics?: boolean;
}

/** The binary-frame protocol tables the addon exports: the stream's version,
 *  its opcodes and its prop ids, which is everything `createEncoder` needs.
 *  (`protocol()` also carries the schema tables `npm run gen` reads; those
 *  are generator input, not app surface, so they are not typed here.) */
export interface Protocol {
  version: number;
  op: Record<string, number>;
  prop: Record<string, number>;
}

/** A reusable frame encoder for the binary IR path (drivers make their own). */
export declare function createEncoder(p: Protocol): {
  encode(tree: KuiNode): { stream: Float64Array; strings: Uint8Array };
};

// -- generated from the addon's `#[napi]` surface; edit crates/kui-node/src/lib.rs, then `npm run gen` --

/**
 * The binary-frame protocol tables (`{version, op, prop}`). The JS encoder
 * reads its opcodes and prop ids from here at module init, so the two sides
 * cannot drift.
 */
export declare function protocol(): Protocol

/**
 * A headless kui core: build frames from JSX trees, feed input, poll events.
 * Everything a window does except open one, so an app's behaviour is
 * testable without a display.
 */
export declare class Ctx {
  constructor()
  /**
   * `frame` from an already-encoded binary instruction stream, for
   * callers that own their encoder (`createEncoder(protocol())`) — the
   * fastest path, and what the JS drivers use. Buffers are read
   * zero-copy.
   */
  frameBinary(width: number, height: number, scale: number, stream: Float64Array, strings: Uint8Array): void
  /**
   * The frame clock for `transition` props: monotonic seconds, any
   * origin. Set before each frame; never setting it makes transitions
   * snap (the default for headless tests).
   */
  setTime(nowSecs: number): void
  cursor(x: number, y: number): void
  cursorLeft(): void
  /**
   * A button press or release. `clicks`: 1 single, 2 double (word
   * select), 3 triple (line select). `button` defaults to "primary", and
   * only that one presses, drags, places the caret and clicks;
   * "secondary" asks the node under the pointer for a context menu and
   * moves nothing else, and nothing routes "middle" yet.
   */
  mouse(down: boolean, clicks?: number, button?: MouseButtonName): void
  scroll(dx: number, dy: number): void
  /** Committed text input (typing, paste); routed to the focused editor. */
  text(text: string): void
  /**
   * Editing key by name ("left", "backspace", "enter", ...) with optional
   * modifiers `{shift, word, doc}`.
   */
  key(name: EditKeyName, mods?: KeyMods): void
  /**
   * Raw key press for `onKey` sinks (modal keymaps): a single character
   * (layout-resolved, e.g. "W" or "$"), a name ("left", "enter", "escape",
   * "f5", ...), with mods `{shift, ctrl, alt, super}`. Editing keys for
   * focused editors still go through `key()`. `repeat` marks a press the
   * OS auto-repeated. The sink hears `{kind:"key", phase:"down", ...}`.
   */
  keyDown(code: string, mods?: KeySinkMods, repeat?: boolean): void
  /**
   * The release of a key, spelled the way `keyDown` spells it: the sink
   * hears `{kind:"key", phase:"up", ...}` with `text` null. A release
   * whose press the sink never got resolves nothing, and moving focus
   * while a key is held delivers the `up` first.
   */
  keyUp(code: string, mods?: KeySinkMods): void
  /**
   * Physical modifier state changed: `{shift, ctrl, alt, super}`. The
   * host receives `{kind:"modifiers", ...}` when it differs from the
   * last report.
   */
  modifiers(mods?: KeySinkMods): void
  /**
   * Drains the audio commands the core queued, as plain objects
   * (`{kind:"play", playback, sound, volume, loop, fadeIn}`, ...) — what a
   * windowed driver would play. For tests and custom drivers.
   */
  audioCommands(): AudioCommand[]
  /**
   * A custom driver reports a playback finished on its own; a tagged
   * one becomes a `sound` event in `pollEvents`.
   */
  audioEnded(playback: number): void
  /**
   * The window title the last frame declared (a root `<box title>`), or
   * null when it declared none. `runWindowed` applies it to the real
   * window; a bare `Ctx` hands it back so a test can assert on it.
   */
  windowTitle(): string | null
  /**
   * Raw quads for the finished frame, `quadStride()` bytes each, laid out
   * as kui-ffi's KuiQuad (see include/kui.h). Copied into the Buffer.
   */
  quads(): Buffer
  /**
   * Registers a w×h RGBA image (pixels copied); returns its id for
   * `<image src={id}>`. Stable until `removeImage`.
   */
  addImage(width: number, height: number, rgba: Buffer): string
  removeImage(id: string): void
  /**
   * Registers a font from file bytes (TTF/OTF/TTC); returns its id
   * for the `font` prop on `<text>` / `<edit>`. Throws when the
   * data holds no usable face.
   */
  addFont(data: Buffer): string
  /**
   * Registers an installed font by family name; null when none
   * matches (see `systemFontFamilies`). Also finds families loaded
   * with `loadFontsDir` / `loadFontFile`; the same family gets the
   * same id.
   */
  addSystemFont(name: string): string | null
  /**
   * Registers a font file by path (memory-mapped); throws when it
   * cannot be read or holds no usable face.
   */
  loadFontFile(path: string): string
  /**
   * Loads every font file under a folder (recursively) so its
   * families can be picked by name with `addSystemFont`; returns
   * the face count.
   */
  loadFontsDir(dir: string): number
  removeFont(id: string): void
  /**
   * Family names of every font the core can see, installed or
   * loaded (sorted).
   */
  systemFontFamilies(): Array<string>
  /**
   * Registers a sound from its encoded bytes (wav/ogg/mp3/flac);
   * returns its id for `<audio src>`, the `clickSound` /
   * `hoverSound` props and `play`. Stable until `removeSound`.
   */
  addSound(data: Buffer): string
  removeSound(id: string): void
  /**
   * Starts a playback: `{volume, loop, fadeIn, tag}`; returns its
   * id for `stop` / `setVolume` / `pause` / `resume`. A `tag` comes
   * back as a `SoundMsg` when the playback finishes on its own.
   * A window plays it on its own device at once; headless nothing
   * sounds and the command queues for `audioCommands()`.
   */
  play(sound: string, opts?: PlayOptions): number
  stop(playback: number, fadeMs?: number): void
  setVolume(playback: number, volume: number, tweenMs?: number): void
  pause(playback: number, fadeMs?: number): void
  resume(playback: number, fadeMs?: number): void
  setMasterVolume(volume: number, tweenMs?: number): void
  /**
   * Events since the last poll: `[{origin, key, payload}]`,
   * payloads as plain data (your Elm messages come back out
   * here). `A` types them — the app's own union, or one core
   * message type when only that is being watched.
   */
  pollEvents<A = AppMsg | CoreMsg>(): UiEvent<A>[]
  /**
   * True when the last frame left a transition mid-flight. A window
   * schedules its own redraws for that; this is for tests and
   * drivers that want to know when motion has settled.
   */
  animating(): boolean
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Measures text the way layout would, without adding a node:
   * `{width, height, lines}` in logical px, wrapped to `maxWidth`
   * when given. `content` is whatever `<text>` takes (a string, or
   * children with `<span>`s); `style` the `<text>` props (`size`,
   * `font`, `wrap`, `maxLines`, `ellipsis`, ...). Works before the
   * first frame; a window answers at its own scale once a frame has
   * run. Size a column to its widest label, or pick the tier that
   * fits, from these numbers instead of constants found by
   * screenshot.
   */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics
  /**
   * Drains the warnings the core raised since the last call
   * (see `Warning`), each distinct (code, node) pair once.
   * `createApp` collects them on `app.warnings` for you, and
   * `runWindowed` prints them, unless either was told not to.
   */
  warnings(): Warning[]
  /**
   * Turns the per-frame diagnostic checks behind `warnings` on or
   * off. A bare `Ctx` has them on; `createApp` / `runWindowed`
   * turn them off under `NODE_ENV=production`.
   */
  setDiagnostics(on: boolean): void
  /**
   * What assistive technology sees of the last frame (see
   * `AccessTree`). A window hands it to the platform by itself
   * (AccessKit); this is for tests and tooling.
   */
  accessTree(): AccessTree
  /**
   * A request from assistive technology on a node (`key`, hex as in
   * events): an `AccessAction` name the node advertises, with
   * `value` the new text for `setValue`. Resolved like its
   * pointer/keyboard equivalent, so the resulting events come out of
   * `pollEvents`. A real screen reader's requests arrive through a
   * window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame (keys come from events, e.g.
   * an `onHover` enter). For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  isPressed(key: string): boolean
  /**
   * Whether a node holds keyboard focus — any node: an editor, an
   * `onKey` sink, a button Tab landed on (see `focused`).
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab (`key("tab")`) walk every control in tree order,
   * Enter and Space press the focused one, and the arrows nudge a
   * focused slider.
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   */
  focus(key: string): void
  blur(): void
  /**
   * What Tab does, as a call — for an `onKey` sink that binds Tab
   * itself and wants to hand the keyboard on: the next focusable
   * node in tree order, wrapping.
   */
  focusNext(): void
  /** What Shift-Tab does. */
  focusPrev(): void
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins.
   */
  reveal(key: string): void
  /**
   * A scroll container's retained offset `{x, y}` as the last layout
   * clamped it (positive = content moved up / left) — the number to
   * keep in a model and hand back to `setScroll`. Zero for a node
   * that never scrolled.
   */
  scrollOffset(key: string): ScrollOffset
  /**
   * Everything the last layout resolved for the scroll container
   * `key`: its box `{x, y, w, h}`, its content size `{contentW,
   * contentH}` and the clamped `offset` — `null` for a key no layout
   * has resolved as a container.
   *
   * This is what makes a long list affordable. The core builds every
   * child a view declares, so ten thousand rows cost ten thousand
   * rows; knowing `h` and `offset.y`, a view renders the rows that
   * fit plus two spacers holding the space of the rest. Read while
   * building, it describes the previous frame, so a resize slices one
   * frame late — render a row or two extra at each end.
   */
  scrollGeometry(key: string): ScrollGeometry | null
  /**
   * Sets that offset the way the wheel would; the next frame's
   * layout clamps it, so `(0, 0)` jumps to the top and a huge `y` to
   * the end without knowing the content height.
   */
  setScroll(key: string, x: number, y: number): void
  editText(key: string): string | null
  setEditText(key: string, text: string): void
}

/** Byte stride of one quad in the `quads()` buffer. */
export declare function quadStride(): number

/**
 * A real kui window (winit + wgpu) driven from Node. The event loop is
 * pumped, not run: call `pump()` from a timer loop so winit and libuv share
 * the main thread — or prefer `runWindowed`, which does that for you, unless
 * you are building your own loop. One window per process; winit event loops
 * are not recreatable on every platform.
 */
export declare class KuiWindow {
  /**
   * Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
   * chrome: "native" | "custom" | "borderless"}`. The min/max pairs bound
   * what the user can resize the window to; either half may stand alone.
   */
  constructor(title: string, options?: WindowOptions)
  /**
   * `setView` with a flat binary instruction stream (see `Ctx::frame_binary`).
   * Copied once so redraws (resize, hover) can re-lower it between pumps.
   */
  setViewBinary(stream: Float64Array, strings: Uint8Array): void
  /**
   * Processes pending OS events without blocking. Returns false once the
   * window has closed.
   */
  pump(): boolean
  /**
   * The window's inner size in logical px plus its scale factor:
   * `{width, height, scale}`. Readable before the first frame (in
   * `setup`), and re-reported as a `{kind:"resize", width, height,
   * scale}` event through `pollEvents` — a `ResizeMsg` — whenever the
   * window changes size or moves to a display with another DPI.
   */
  size(): WindowSize
  /**
   * Frame timing measured by the runner — what the latency HUD draws,
   * as data: `{frames, last: {inputMs, viewMs, layoutMs, renderMs,
   * waitMs, totalMs, workMs} | null, avgTotalMs, maxTotalMs, avgWorkMs,
   * maxWorkMs}` over the last 120 frames. `waitMs` is vsync
   * backpressure; `workMs` is everything else.
   */
  frameStats(): FrameTiming
  /** Asks the window to close; the next pump returns false. */
  close(): void
  /**
   * Registers a w×h RGBA image (pixels copied); returns its id for
   * `<image src={id}>`. Stable until `removeImage`.
   */
  addImage(width: number, height: number, rgba: Buffer): string
  removeImage(id: string): void
  /**
   * Registers a font from file bytes (TTF/OTF/TTC); returns its id
   * for the `font` prop on `<text>` / `<edit>`. Throws when the
   * data holds no usable face.
   */
  addFont(data: Buffer): string
  /**
   * Registers an installed font by family name; null when none
   * matches (see `systemFontFamilies`). Also finds families loaded
   * with `loadFontsDir` / `loadFontFile`; the same family gets the
   * same id.
   */
  addSystemFont(name: string): string | null
  /**
   * Registers a font file by path (memory-mapped); throws when it
   * cannot be read or holds no usable face.
   */
  loadFontFile(path: string): string
  /**
   * Loads every font file under a folder (recursively) so its
   * families can be picked by name with `addSystemFont`; returns
   * the face count.
   */
  loadFontsDir(dir: string): number
  removeFont(id: string): void
  /**
   * Family names of every font the core can see, installed or
   * loaded (sorted).
   */
  systemFontFamilies(): Array<string>
  /**
   * Registers a sound from its encoded bytes (wav/ogg/mp3/flac);
   * returns its id for `<audio src>`, the `clickSound` /
   * `hoverSound` props and `play`. Stable until `removeSound`.
   */
  addSound(data: Buffer): string
  removeSound(id: string): void
  /**
   * Starts a playback: `{volume, loop, fadeIn, tag}`; returns its
   * id for `stop` / `setVolume` / `pause` / `resume`. A `tag` comes
   * back as a `SoundMsg` when the playback finishes on its own.
   * A window plays it on its own device at once; headless nothing
   * sounds and the command queues for `audioCommands()`.
   */
  play(sound: string, opts?: PlayOptions): number
  stop(playback: number, fadeMs?: number): void
  setVolume(playback: number, volume: number, tweenMs?: number): void
  pause(playback: number, fadeMs?: number): void
  resume(playback: number, fadeMs?: number): void
  setMasterVolume(volume: number, tweenMs?: number): void
  /**
   * Events since the last poll: `[{origin, key, payload}]`,
   * payloads as plain data (your Elm messages come back out
   * here). `A` types them — the app's own union, or one core
   * message type when only that is being watched.
   */
  pollEvents<A = AppMsg | CoreMsg>(): UiEvent<A>[]
  /**
   * True when the last frame left a transition mid-flight. A window
   * schedules its own redraws for that; this is for tests and
   * drivers that want to know when motion has settled.
   */
  animating(): boolean
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Measures text the way layout would, without adding a node:
   * `{width, height, lines}` in logical px, wrapped to `maxWidth`
   * when given. `content` is whatever `<text>` takes (a string, or
   * children with `<span>`s); `style` the `<text>` props (`size`,
   * `font`, `wrap`, `maxLines`, `ellipsis`, ...). Works before the
   * first frame; a window answers at its own scale once a frame has
   * run. Size a column to its widest label, or pick the tier that
   * fits, from these numbers instead of constants found by
   * screenshot.
   */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics
  /**
   * Drains the warnings the core raised since the last call
   * (see `Warning`), each distinct (code, node) pair once.
   * `createApp` collects them on `app.warnings` for you, and
   * `runWindowed` prints them, unless either was told not to.
   */
  warnings(): Warning[]
  /**
   * Turns the per-frame diagnostic checks behind `warnings` on or
   * off. A bare `Ctx` has them on; `createApp` / `runWindowed`
   * turn them off under `NODE_ENV=production`.
   */
  setDiagnostics(on: boolean): void
  /**
   * What assistive technology sees of the last frame (see
   * `AccessTree`). A window hands it to the platform by itself
   * (AccessKit); this is for tests and tooling.
   */
  accessTree(): AccessTree
  /**
   * A request from assistive technology on a node (`key`, hex as in
   * events): an `AccessAction` name the node advertises, with
   * `value` the new text for `setValue`. Resolved like its
   * pointer/keyboard equivalent, so the resulting events come out of
   * `pollEvents`. A real screen reader's requests arrive through a
   * window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame (keys come from events, e.g.
   * an `onHover` enter). For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  isPressed(key: string): boolean
  /**
   * Whether a node holds keyboard focus — any node: an editor, an
   * `onKey` sink, a button Tab landed on (see `focused`).
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab (`key("tab")`) walk every control in tree order,
   * Enter and Space press the focused one, and the arrows nudge a
   * focused slider.
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   */
  focus(key: string): void
  blur(): void
  /**
   * What Tab does, as a call — for an `onKey` sink that binds Tab
   * itself and wants to hand the keyboard on: the next focusable
   * node in tree order, wrapping.
   */
  focusNext(): void
  /** What Shift-Tab does. */
  focusPrev(): void
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins.
   */
  reveal(key: string): void
  /**
   * A scroll container's retained offset `{x, y}` as the last layout
   * clamped it (positive = content moved up / left) — the number to
   * keep in a model and hand back to `setScroll`. Zero for a node
   * that never scrolled.
   */
  scrollOffset(key: string): ScrollOffset
  /**
   * Everything the last layout resolved for the scroll container
   * `key`: its box `{x, y, w, h}`, its content size `{contentW,
   * contentH}` and the clamped `offset` — `null` for a key no layout
   * has resolved as a container.
   *
   * This is what makes a long list affordable. The core builds every
   * child a view declares, so ten thousand rows cost ten thousand
   * rows; knowing `h` and `offset.y`, a view renders the rows that
   * fit plus two spacers holding the space of the rest. Read while
   * building, it describes the previous frame, so a resize slices one
   * frame late — render a row or two extra at each end.
   */
  scrollGeometry(key: string): ScrollGeometry | null
  /**
   * Sets that offset the way the wheel would; the next frame's
   * layout clamps it, so `(0, 0)` jumps to the top and a huge `y` to
   * the end without knowing the content height.
   */
  setScroll(key: string, x: number, y: number): void
  editText(key: string): string | null
  setEditText(key: string, text: string): void
}

// -- end generated --

// The two calls the JS package adds to those classes: a frame crosses the
// boundary already encoded, and `encoder.js` is the only thing that encodes
// one, so `frame` / `setView` live on the prototypes (see index.js) rather
// than in the addon. Declaration merging puts them on the classes above.

export interface Ctx {
  /** Lowers a JSX tree into one frame: encodes it to the flat binary IR
   *  stream, then one zero-copy boundary crossing lowers it. */
  frame(width: number, height: number, scale: number, tree: KuiNode): void;
}

export interface KuiWindow {
  /** Stores the tree future redraws lower, and schedules one. Encodes it to
   *  the binary IR stream first. */
  setView(tree: KuiNode): void;
}

/** What both drivers take. `S` is the surface the loop drives, and the
 *  fourth argument `update` gets: the headless `Ctx` under `createApp`, the
 *  `KuiWindow` under `runWindowed`. `M` is the model, `A` every message
 *  `update` can see. */
export interface LoopConfig<M, A, S> {
  init: M | (() => M);
  /** Returns the next model; returning undefined keeps the current one.
   *  `surface` is the thing being driven — for `editText`, `focus`,
   *  `play`, `scrollGeometry` and the rest. */
  update: (model: M, msg: A, event: UiEvent<A>, surface: S) => M | undefined | void;
  view: (model: M) => KuiNode;
  /** A clock: every `every` ms the loop feeds `msg` (or `msg(now)`, with the
   *  clock's own reading — `Date.now()` under a window, the loop's own
   *  milliseconds headless) to `update`. Ticks are frequent, so unlike UI
   *  events they re-render only when `update` returns a new model — a
   *  countdown that returns undefined until the displayed second changes
   *  costs nothing in between. Read backwards, that is the trap: a tick
   *  handler that mutates the model in place and returns undefined never
   *  reaches the screen. Return the model (any non-undefined return renders)
   *  on the ticks that should draw.
   *
   *  A window fires these off its own timer; headless, `app.advance(ms)`
   *  fires every tick inside the span, so a ticking app is testable. */
  tick?: { every: number; msg: A | ((now: number) => A) };
}

/** `runWindowed`'s config: `update` also gets the window. */
export type WindowedConfig<M, A = AppMsg | CoreMsg> = LoopConfig<M, A, KuiWindow>;

/** Opens a window and runs the Elm loop; resolves with the final model on close. */
export declare function runWindowed<M, A = AppMsg | CoreMsg>(
  config: WindowedConfig<M, A>,
  opts?: WindowOptions & {
    title?: string;
    pumpMs?: number;
    /** Runs after the window opens, before `init` and the first frame —
     *  register images, fonts and other resources here. `app` is the loop
     *  itself, so a test can hold on to it and drive a real window with the
     *  same helpers `createApp` gives (`settle`, `access`, ...). */
    setup?: (win: KuiWindow, app: WindowLoop<M, A>) => void;
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
export type AppConfig<M, A = AppMsg | CoreMsg> = LoopConfig<M, A, Ctx>;

/** The loop both drivers run, over the surface it was handed. Everything
 *  here is written once and works against either — which is what lets the
 *  test helpers drive a real window when you want to watch one. */
export interface Loop<M, A, S> {
  /** The surface this loop drives. */
  readonly surface: S;
  readonly model: M;
  /** Every warning the core raised while rendering, in order; each is also
   *  printed unless created with `warnings: false`. A test asserts it is
   *  empty, or that a specific code showed up. */
  readonly warnings: Warning[];
  dispatch(msg: A, event?: UiEvent<A>): void;
  /** Renders the current model and hands back the frame's display-list
   *  summary. */
  render(): FrameStats;
  /** One turn of the loop: whatever the surface queued, then the ticks the
   *  clock owes, then a frame if either changed the model. `runWindowed`
   *  runs one after every `win.pump()`; a custom driver can too. */
  step(): void;
  /** Drain events -> update -> re-render until no events remain. */
  settle(): void;
  /** Synthetic input. These need a surface that takes it — a headless `Ctx`
   *  does; a window is driven by the OS and says so rather than pretending. */
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
   *  editor — and settles the events that follow through `update`. Works
   *  against a real window too. */
  access(key: string, action: AccessAction, value?: string | AccessArg): void;
}

export interface App<M, A = AppMsg | CoreMsg> extends Loop<M, A, Ctx> {
  /** The surface, under the name headless tests reach for. */
  ctx: Ctx;
  /** Moves the loop's own clock `ms` forward: every tick inside the span
   *  fires, the frame clock behind `transition` follows it (`ctx.setTime`),
   *  and the app re-renders. This is the window's timer by hand — what makes
   *  a ticking app drivable by a test. */
  advance(ms: number): void;
}

/** The loop `runWindowed` builds, handed to `setup`. It has no `advance`:
 *  a window runs on the wall clock and ticks itself. */
export type WindowLoop<M, A = AppMsg | CoreMsg> = Loop<M, A, KuiWindow>;

export declare function createApp<M, A = AppMsg | CoreMsg>(
  config: AppConfig<M, A>,
  opts?: {
    width?: number;
    height?: number;
    scale?: number;
    /** `false` keeps the core's warnings off the console; they still
     *  collect on `app.warnings`. */
    warnings?: boolean;
    /** Whether the core runs the checks at all; default on unless
     *  `NODE_ENV` is `production`. */
    diagnostics?: boolean;
    /** What the loop's clock reads before the first `advance` — the origin
     *  `tick.msg(now)` counts from. Default `Date.now()`; pin it to make a
     *  ticking app's assertions exact. */
    startTime?: number;
    /** The surface to drive. Default: a fresh headless `Ctx`. */
    surface?: Ctx;
    /** The loop's time source, in milliseconds — what `runWindowed` fills
     *  with `Date.now`. Given one, the loop reads it (and ticks resync
     *  after falling behind, as a window's do) instead of holding its own
     *  hands, so `advance` no longer applies. */
    clock?: () => number;
    /** Runs before `init` and the first frame — register images and fonts
     *  here so `init` can name their ids. */
    setup?: (ctx: Ctx, app: App<M, A>) => void;
  },
): App<M, A>;
