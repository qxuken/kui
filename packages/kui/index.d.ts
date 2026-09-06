import type { AppMsg, GeneratedSpecProps, KuiNode, TextProps } from './jsx-runtime.js';

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

/** A surface was asked to go away: Escape, or a press that landed outside
 *  it. The core closes nothing — stop declaring the surface (or ask first).
 *
 *  On a node it is the frame's `modal`, carrying its `tag`, and only the
 *  modal in effect gets one. On the root it is a `kind: 'popup'` window,
 *  carrying that window's `name` and `id`; the driver reports it, because a
 *  press outside a window and a key sent to a non-activating one are facts
 *  only the OS has. The two are one event, so a dropdown that graduates
 *  from a modal float to a popup window changes its declaration and keeps
 *  its handler. */
export type DismissMsg<T = AppMsg> = {
  kind: 'dismiss';
  reason: 'escape' | 'outside';
  tag?: T;
  /** Popup windows only: which window, as `windows` named it and as its
   *  events carry it. */
  name?: string;
  id?: number;
};

/** The viewport changed size or DPI (logical px, delivered on the root);
 *  `win.size()` queries the same numbers. */
export type ResizeMsg = {
  kind: 'resize';
  width: number;
  height: number;
  scale: number;
};

/** A declared window opened, or closed — because nothing declares it any
 *  more, or because the user closed it. A window the user closed stays
 *  closed while it is still declared (a declaration reopens a window only
 *  when it *starts*): stop declaring `name`, then declare it again. `id` is
 *  what that window's events carry in `window`; the message itself arrives
 *  on the root of whichever window's frame noticed. */
export type WindowMsg = {
  kind: 'window';
  phase: 'opened' | 'closed';
  name: string;
  id: number;
};

/** One entry of `windows(model)` (or of the root box's `windows` prop): a
 *  window that should exist, by stable name. `width`/`height` are the
 *  initial inner size in logical px (640x480 when left out) and are read on
 *  the frame the window opens and never again — the user owns its geometry
 *  once it exists. `activates` is whether opening it takes OS focus
 *  (default true for a normal window, false for a popup). A bare string is
 *  a name at the defaults.
 *
 *  `kind: 'popup'` makes it a menu surface instead: borderless, off the
 *  taskbar, owned by the window that declared it and closed with it, placed
 *  in screen coordinates against `anchor` — the `{x, y, w, h}` a
 *  `LayoutMsg` already reports for the field or button the menu belongs to
 *  — and non-activating, so the field keeps its focus ring while the arrows
 *  walk the list. A press outside it or Escape arrives as a `DismissMsg`
 *  and closes nothing: stop declaring the window, the way you stop
 *  declaring a `modal` node. Reach for one only where an in-window float
 *  cannot go — a list taller than the window, a menu with nowhere in-window
 *  to sit, a panel beside the app; everything else is cheaper as a `float`
 *  with `fit`. See `docs/adr/0004-multi-window.md`, decision 9. */
export type WindowDecl =
  | string
  | {
      name: string;
      kind?: 'normal' | 'popup';
      width?: number;
      height?: number;
      activates?: boolean;
      /** `kind: 'popup'` only: what to place it against, in the declaring
       *  window's own logical coordinates — a `LayoutMsg`'s rect. */
      anchor?: { x?: number; y?: number; w?: number; h?: number };
    };

/** What `Ctx.windowCommands()` drains: what chrome nodes asked for
 *  (`startDrag` / `close` / `minimize` / `toggleMaximize`, about `window`)
 *  and what the declared window set decided (`open` with the new window's
 *  id and the config from its declaration, `close`). A `KuiWindow` applies
 *  these itself. */
export type WindowCommand =
  | { kind: 'startDrag' | 'close' | 'minimize' | 'toggleMaximize'; window: number }
  | { kind: 'setSize'; window: number; width: number; height: number }
  | { kind: 'focus'; window: number }
  | {
      kind: 'open';
      window: number;
      /** The window whose frame declared this one: a popup's owner, whose
       *  position its `anchor` is measured against and whose closing closes
       *  it. */
      owner: number;
      /** Whose declaration won: 0 is your app, 1+ an extension. */
      origin: number;
      config: {
        kind: 'normal' | 'popup';
        width: number;
        height: number;
        activates: boolean;
        anchor: { x: number; y: number; w: number; h: number };
      };
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
  | WindowMsg
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
  | 'radioGroup' | 'menu' | 'menuItem'
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
  /** "3 of 7", derived from the composite container holding this node: the
   *  zero-based ordinal on each item, the count on the container. */
  posInSet: number | null;
  setSize: number | null;
  /** How a composite container (`radioGroup`, `tabList`, `menu`, `list`)
   *  arranges its items, from its own `dir`. `null` for anything else. */
  orientation: 'horizontal' | 'vertical' | null;
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

/** What `cursorShape()` reports: the `cursor` prop's own vocabulary, so a
 *  test compares against `'text'` and the two cannot drift. */
export type CursorShape = NonNullable<GeneratedSpecProps['cursor']>;

/** What text measures (`measureText`): logical px at the scale of the
 *  current or last frame; `lines` after wrapping. The same numbers layout
 *  gives a `<text>` with that content and style. */
export interface TextMetrics {
  width: number;
  height: number;
  lines: number;
}

// -- generated from the core's warning codes; edit crates/kui-core/src/diag.rs, then `npm run gen` --
export type WarningCode =
  /** A grow weight other than 1 on the only grow child of its parent (the
   *  weight splits space between grow siblings, so alone it changes nothing) or
   *  across the parent's main axis (cross-axis grow fills the parent whatever
   *  its weight). */
  | 'grow-weight-ignored'
  /** The child count of a node changed while one of its children carries a
   *  transition under an auto-assigned key. Auto keys are sibling positions, so
   *  the children that shifted became new nodes and snapped instead of easing.
   *  Give list items a key. */
  | 'transition-auto-key'
  /** Two nodes in one frame share a key: everything retained per key
   *  (transitions, scroll offsets, editors, layout events, hover state) is
   *  mixed between them. Siblings need distinct keys. */
  | 'duplicate-key'
  /** An image with no `label`: assistive technology has nothing to say for it.
   *  Decorative images take `role="none"`. */
  | 'image-without-label'
  /** The frame's modal surface is not in a float, and content painted after it
   *  is drawn on top of it: everything the user can see over the modal is
   *  inert, which looks like inert-behind is broken. A modal that has to cover
   *  the app is a float (`float="viewport"`); see
   *  `docs/adr/0003-modal-surfaces.md`. */
  | 'modal-behind-content'
  /** A control (a button, link, tab, checkbox, slider, editor) with no
   *  computable name: no `label`, and no text inside it. Icon buttons and
   *  editors need a `label`. */
  | 'control-without-name'
  /** A focusable node inside a composite's *item* — a button inside a list row,
   *  a link inside a tab. The item is one roving stop of a composite
   *  (`docs/adr/0007-composite-keyboard-patterns.md`), so the Tab ring stops at
   *  the item and nothing reaches what is inside it: declared, and impossible,
   *  which is what `modal-behind-content` set the precedent for. A focusable
   *  node inside the *container* but outside every item — a "+" at the end of a
   *  tab bar — is reachable and is not reported. */
  | 'focusable-inside-item'
  /** A `modal` surface with no `label`. A dialog is not named by the text
   *  inside it (it is not one of ARIA's name-from-content roles), so a screen
   *  reader announces it as an unnamed dialog — the same silent defect
   *  `control-without-name` catches, on the node that just took the user's
   *  focus. Only the derived dialog role is checked: a modal that says what it
   *  is with an explicit `role` says it with a `label` too, or means something
   *  naming works differently for. */
  | 'modal-without-name'
  /** A node declares `live` but carries no `label` and holds no text, so
   *  nothing it ever does can be announced: every platform derives the spoken
   *  string from a name, and there is none to derive. The same silent defect
   *  `image-without-label` catches, on the node that was meant to speak (see
   *  `docs/adr/0008-live-regions-and-announcements.md`). */
  | 'live-region-without-name'
  /** The same announcement text was queued on two consecutive frames. That is
   *  what an unguarded `announce` in a frame builder looks like — a view runs
   *  every frame, so the message is said every frame — and it is never what an
   *  app means: a message genuinely repeated is repeated across frames the user
   *  did something in between. The announcement still goes through; this names
   *  the builder that is shouting. */
  | 'announcement-repeated'
  /** `wrapChildren` on a container that cannot break lines: a column, or a row
   *  whose main axis scrolls. Both lay out exactly as if the flag were absent,
   *  which reads as "wrapping is broken"; see `LayoutSpec::wrap` for why a
   *  column cannot have it. */
  | 'wrap-ignored'
  /** More nodes are departing at once than the exit store will hold (512,
   *  `depart::MAX_NODES`), so the subtrees past the budget vanished instead of
   *  animating out. Correct — that is what a node with no `exit` does — and
   *  invisible from the outside, which is the whole reason it is a line here: a
   *  list that drops a thousand rows wants `exit` on the list, not on every
   *  row. */
  | 'exit-budget'
  /** A prop name nothing claims: not a schema row, not a composite, not one of
   *  the element's own props (see `schema::known_prop`). The binding threw the
   *  declaration away — `hoverBg` in a Lua table, `onclick` in JSX — so unlike
   *  every other code here this one is raised by the frontend that saw it,
   *  through `Core::warn`: by the time a frame is a tree the name is gone. The
   *  message names the likely spelling. */
  | 'unknown-prop'
  /** One name declared with two different window configs on the frame it
   *  opened. The config is read on the opening edge only, and on that edge the
   *  lowest declaring window wins (the first declaration within one frame), so
   *  the pick is deterministic — but two places in the app disagree about what
   *  `"palette"` is, and only one of them is right. See
   *  `docs/adr/0004-multi-window.md`, decision 4. */
  | 'duplicate-window-config'
  /** A window the user closed is still declared, so it stays closed: a
   *  declaration reopens a window only when it *starts*, and this one never
   *  stopped. The first version of every multi-window app does this — it
   *  declares the window unconditionally — and from outside it looks like
   *  `windows` being ignored. Handle the `{kind:"window", phase:"closed"}`
   *  event, stop declaring the name, and declare it again to reopen. See
   *  `docs/adr/0004-multi-window.md`, decision 6. */
  | 'window-declared-while-closed'
  /** A window declared with a `KUI_WINDOW_KIND_*` this build does not have —
   *  `KUI_WINDOW_KIND_NORMAL` and `KUI_WINDOW_KIND_POPUP` are the two there
   *  are. Only a C host can reach this: JSX and Lua name a kind by string, so
   *  an unknown one is refused where it is written rather than reported a frame
   *  later. The window still opens, as a normal one, so a host built against a
   *  later header degrades to a window rather than to nothing; this line is
   *  what keeps that from being silent. See `docs/adr/0004-multi-window.md`,
   *  decision 9. */
  | 'unknown-window-kind'
  /** A `FontId` / `ImageId` / `SoundId` registered in one `Session` and used
   *  through a core of another. Handles are unique to the process, so it cannot
   *  resolve to somebody else's resource; it behaves as a removed handle does
   *  (draws nothing, shapes as sans-serif, plays nothing), which from outside
   *  looks like the resource never registered. Two `Core::new()`s are two
   *  sessions; windows that share resources are built with `Core::new_in`
   *  against one `Session`. */
  | 'foreign-resource';
// -- end generated --

/** A silent misconfiguration the core noticed while finishing a frame —
 *  the kind that otherwise looks like "the feature is broken". Each
 *  distinct (code, node) pair is raised once. */
export interface Warning {
  /** Stable: match on it. Every code, with what it means, is `WarningCode`
   *  (and the Warnings table in docs/props.md); the `string` arm keeps a
   *  newer addon's codes from being a type error. */
  code: WarningCode | (string & {});
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

/** How urgently a screen reader should read a change it was not asked to
 *  read (the `live` prop, and `announce`'s politeness). */
export type Live = 'off' | 'polite' | 'assertive';

/** One thing to say once, with no node behind it. `Ctx.announcements()`
 *  drains them (headless); a `KuiWindow` delivers them itself. See
 *  docs/adr/0008-live-regions-and-announcements.md. */
export interface Announcement {
  text: string;
  /** Never `'off'` — `announce` drops those. */
  live: Live;
}

// --------------------------------------------------------------------------

/** One event out of the loop. `A` is the app's message union; it defaults to
 *  the registered `AppMsg` plus the core's own messages. */
export interface UiEvent<A = AppMsg | CoreMsg> {
  /** Which frontend drew the node: 0 is your app, 1+ an extension. Not the
   *  window — an extension draws into every one of them. */
  origin: number;
  /** Which window it happened in, matching `env().window.id`. 0 while an
   *  app has one window, which is every app today. */
  window: number;
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

/** Host facts the frame driver pushed into the core, as `env()` reads them
 *  back: what the display and the window are doing right now. A window's
 *  runner refreshes all of it every frame; a headless `Ctx` shows the
 *  defaults until `setEnv` says otherwise. */
export interface Env {
  /** Display refresh rate in Hz, or null when the host cannot tell (which
   *  is when `frameBudgetMs` falls back to 120 Hz). */
  refreshHz: number | null;
  /** One vsync interval at `refreshHz` — the per-frame time budget, and
   *  what the latency HUD draws its line at. */
  frameBudgetMs: number;
  /** Whether the *window* has the keyboard at all. Not to be confused with
   *  `focused()`, which is the focused *node*'s key. */
  focused: boolean;
  /** The viewport the current or last frame was begun with. */
  viewport: WindowSize;
  window: WindowEnv;
}

/** The window chrome facts on `Env`. A view that draws its own titlebar
 *  reads these the way `<titlebar>` does: inset past `nativeControls`, pick
 *  the maximize or restore glyph from `maximized`, draw nothing at all
 *  unless `customChrome`. */
export interface WindowEnv {
  /** Which window this frame is drawing, assigned by the driver — the same
   *  number every `UiEvent` from it carries. 0 is the window the app starts
   *  in, and the only one there is today. */
  id: number;
  /** The host asked the app to draw its own chrome, so there is no native
   *  titlebar to sit under. `<titlebar>` and `<windowButtons>` build
   *  nothing when this is false. */
  customChrome: boolean;
  maximized: boolean;
  fullscreen: boolean;
  /** Area (logical px, window coordinates) covered by controls the OS still
   *  draws over our content — the macOS traffic lights under custom chrome.
   *  Keep out of it. Null means the OS draws nothing over us. */
  nativeControls: Rect | null;
}

/** A box in logical px: position and size. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** What `Ctx.setEnv` takes: the writable half of `Env`, every key optional.
 *  Keys you leave out keep their current values, so a test declares just the
 *  fact it is about. */
export interface EnvInput {
  /** Null for "the host cannot tell"; a rate at or below zero means the
   *  same. */
  refreshHz?: number | null;
  focused?: boolean;
  window?: {
    /** Which window a headless `Ctx` is standing in for; every `UiEvent` it
     *  hands out carries it. Real windows get theirs from their runner. */
    id?: number;
    customChrome?: boolean;
    maximized?: boolean;
    fullscreen?: boolean;
    /** `x` and `y` default to the window origin; a zero-sized rect and null
     *  both mean "nothing is drawn over us". */
    nativeControls?: Partial<Rect> | null;
  };
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
   * snap. `createApp`'s loop sets it before every frame it draws and
   * `advance` moves it, so only a bare `Ctx` snaps.
   */
  setTime(nowSecs: number): void
  /**
   * Declares host facts a real window would have pushed — what C spells
   * `kui_env_set` + `kui_env_set_window`, in one call shaped like what
   * `env()` reads back. Only the keys you pass move; the rest keep their
   * values, so `setEnv({window: {customChrome: true}})` is the whole of
   * "pretend this app draws its own titlebar" and `<titlebar>`,
   * `<windowButtons>` and `widgets::window_buttons` start building
   * something. `refreshHz: null` means "the host cannot tell" (the
   * default), and `nativeControls: null` means the OS draws nothing over
   * our content. `window.id` is the one fact here an app never chooses —
   * a driver assigns it — and setting it is how a headless test says
   * "these events came from that window"; it is 0 otherwise.
   *
   * Headless only, and on purpose: a `KuiWindow` has no such call because
   * its runner reports the real window every frame, and anything set here
   * would be overwritten before the next view ran.
   */
  setEnv(env: EnvInput): void
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
   *
   * `physical` is the US-QWERTY key at that *position*, spelled the same
   * way; omit it and it equals `code`. Passing both is how a driver
   * reports a non-US layout, and it is what makes the reported `code`
   * portable: a layout producing something outside ASCII would leave a
   * Latin keymap matching nothing, so the position's US letter stands in.
   */
  keyDown(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void
  /**
   * The release of a key, spelled the way `keyDown` spells it (`physical`
   * included): the sink hears `{kind:"key", phase:"up", ...}` with `text`
   * null. A release whose press the sink never got resolves nothing, and
   * moving focus while a key is held delivers the `up` first.
   */
  keyUp(code: string, mods?: KeySinkMods, physical?: string): void
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
   * Drains the announcements queued since the last drain, as plain
   * objects (`{text, live}`). `runWindowed` drains and delivers them
   * itself; a bare `Ctx` hands them back so a driver or a test can see
   * what a frame asked to say.
   */
  announcements(): Announcement[]
  /**
   * A custom driver reports a playback finished on its own; a tagged
   * one becomes a `sound` event in `pollEvents`.
   */
  audioEnded(playback: number): void
  /**
   * Drains the window commands the core queued, as plain objects: what
   * chrome nodes asked for (`{kind:"startDrag"|"close"|"minimize"|
   * "toggleMaximize", window}`) and what the declared window set decided
   * (`{kind:"open", window, owner, origin, config:{kind, width, height,
   * activates, anchor}}` / `{kind:"close", window}`).
   */
  windowCommands(): WindowCommand[]
  /**
   * A custom driver reports that the OS closed window `id`: it stays
   * closed while still declared, whatever only it declared closes with
   * it, and `{kind:"window", phase:"closed", name, id}` lands in
   * `pollEvents`. Nothing happens for the main window (0) or for a
   * window the diff already closed.
   */
  windowClosed(id: number): void
  /**
   * A custom driver reports that window `id` was asked to go away: a
   * press landed outside it (`"outside"`) or Escape reached it
   * (`"escape"`). `{kind:"dismiss", reason, name, id}` lands in
   * `pollEvents` and **nothing closes** — the app stops declaring the
   * window on the frame it decides to, exactly as it answers a `modal`
   * node's dismissal. Nothing happens for a window that is not open.
   */
  windowDismissed(id: number, reason: string): void
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
   * Says something once, with no node behind it: `announce("Saved")`,
   * `announce("3 results", "assertive")`. `"off"` and an empty string
   * are both no-ops. A region whose message is on screen is the `live`
   * prop instead
   * (`docs/adr/0008-live-regions-and-announcements.md`).
   *
   * Call it from an event handler. Called while building a frame it
   * fires every frame, which the core reports as
   * `announcement-repeated`.
   */
  announce(text: string, live?: Live): void
  /**
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
   * Events since the last poll: `[{origin, window, key, payload}]`,
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
   * Host facts the frame driver pushed in: what the window and the
   * display are doing, as of now (see `Env`). This is the same
   * surface Lua's `view(env)` reads and C's `kui_env_set*` writes —
   * a JSX app needs it to build its own titlebar (inset past the
   * macOS traffic lights, pick the maximize glyph), to dim its
   * chrome when the window loses focus, or to pace itself against
   * the real refresh rate.
   *
   * Note `env().focused` is the *window*'s keyboard focus, not the
   * focused node's key — that is `focused()`, one call up.
   */
  env(): Env
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
   * The prop names the encoder threw away while lowering a tree,
   * as `[element, name]` pairs, raised as `unknown-prop` warnings
   * (see `Warning`). A name outside the schema never reaches the
   * binary stream, so the encoder is the only side that sees it;
   * `frame` / `setView` report what they dropped through here.
   * Behind the same `setDiagnostics` gate, and once per name.
   */
  warnUnknownProps(props: [string, string][]): void
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
   * The pointer shape for where the pointer is now, in the `cursor`
   * prop's own vocabulary: derived from the topmost node under it
   * — an editor is `'text'`, an `onClick` or `focusable` node
   * `'pointer'`, an `onDrag` node `'grab'` (`'grabbing'` while it
   * drags), a plain box or no pointer at all `'default'` — or
   * whatever that node's `cursor` overrode it with. A window
   * applies it to the real cursor by itself and only touches it
   * when the answer changes; this is for tests and drivers.
   */
  cursorShape(): CursorShape
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
  /**
   * The names of every window open right now, `"main"` first,
   * then in the order they opened — what a view's root
   * `windows` declared and the diff has opened. `view(model,
   * window)` is called once per name.
   */
  windows(): Array<string>
  /**
   * The name of the window this core draws: `"main"`, or the
   * name the declaration that opened `env().window.id` used.
   */
  windowName(): string
  /**
   * Asks the driver to resize a window to `width`×`height` logical
   * px. A request and not a declaration: a window's `size` config
   * is read on the frame it opens and never again, because the user
   * owns a window's size once it exists, so this is the only way an
   * app moves a live one. Queued the way `reveal` is — a `KuiWindow`
   * applies it on its next pump, and the window answers with the
   * ordinary `resize` event carrying the size it actually became,
   * while a headless `Ctx` has no window and simply keeps the
   * request. `window` is the id events carry (`env().window.id`),
   * 0 for the main window.
   */
  setWindowSize(window: number, width: number, height: number): void
  /**
   * Asks the driver to give a window keyboard focus; queued the same
   * way. Advisory: whether the window manager agreed shows up as
   * `env().focused` on the frames that follow, not as a reply.
   */
  focusWindow(window: number): void
  editText(key: string): string | null
  setEditText(key: string, text: string): void
}

/** Byte stride of one quad in the `quads()` buffer. */
export declare function quadStride(): number

/**
 * A real kui window (winit + wgpu) driven from Node. The event loop is
 * pumped, not run: call `pump()` from a timer loop so winit and libuv share
 * the main thread — or prefer `runWindowed`, which does that for you, unless
 * you are building your own loop. One event loop per process (winit event
 * loops are not recreatable on every platform), any number of windows on
 * it: a view whose root declares `windows` opens more, `windows()` lists
 * them, and `setView` takes the name of the one a tree is for.
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
   * `window` names which window the tree is for — `"main"` when left
   * out; the names `windows()` lists otherwise.
   */
  setViewBinary(stream: Float64Array, strings: Uint8Array, window?: string | undefined | null): void
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
   * Says something once, with no node behind it: `announce("Saved")`,
   * `announce("3 results", "assertive")`. `"off"` and an empty string
   * are both no-ops. A region whose message is on screen is the `live`
   * prop instead
   * (`docs/adr/0008-live-regions-and-announcements.md`).
   *
   * Call it from an event handler. Called while building a frame it
   * fires every frame, which the core reports as
   * `announcement-repeated`.
   */
  announce(text: string, live?: Live): void
  /**
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
   * Events since the last poll: `[{origin, window, key, payload}]`,
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
   * Host facts the frame driver pushed in: what the window and the
   * display are doing, as of now (see `Env`). This is the same
   * surface Lua's `view(env)` reads and C's `kui_env_set*` writes —
   * a JSX app needs it to build its own titlebar (inset past the
   * macOS traffic lights, pick the maximize glyph), to dim its
   * chrome when the window loses focus, or to pace itself against
   * the real refresh rate.
   *
   * Note `env().focused` is the *window*'s keyboard focus, not the
   * focused node's key — that is `focused()`, one call up.
   */
  env(): Env
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
   * The prop names the encoder threw away while lowering a tree,
   * as `[element, name]` pairs, raised as `unknown-prop` warnings
   * (see `Warning`). A name outside the schema never reaches the
   * binary stream, so the encoder is the only side that sees it;
   * `frame` / `setView` report what they dropped through here.
   * Behind the same `setDiagnostics` gate, and once per name.
   */
  warnUnknownProps(props: [string, string][]): void
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
   * The pointer shape for where the pointer is now, in the `cursor`
   * prop's own vocabulary: derived from the topmost node under it
   * — an editor is `'text'`, an `onClick` or `focusable` node
   * `'pointer'`, an `onDrag` node `'grab'` (`'grabbing'` while it
   * drags), a plain box or no pointer at all `'default'` — or
   * whatever that node's `cursor` overrode it with. A window
   * applies it to the real cursor by itself and only touches it
   * when the answer changes; this is for tests and drivers.
   */
  cursorShape(): CursorShape
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
  /**
   * The names of every window open right now, `"main"` first,
   * then in the order they opened — what a view's root
   * `windows` declared and the diff has opened. `view(model,
   * window)` is called once per name.
   */
  windows(): Array<string>
  /**
   * The name of the window this core draws: `"main"`, or the
   * name the declaration that opened `env().window.id` used.
   */
  windowName(): string
  /**
   * Asks the driver to resize a window to `width`×`height` logical
   * px. A request and not a declaration: a window's `size` config
   * is read on the frame it opens and never again, because the user
   * owns a window's size once it exists, so this is the only way an
   * app moves a live one. Queued the way `reveal` is — a `KuiWindow`
   * applies it on its next pump, and the window answers with the
   * ordinary `resize` event carrying the size it actually became,
   * while a headless `Ctx` has no window and simply keeps the
   * request. `window` is the id events carry (`env().window.id`),
   * 0 for the main window.
   */
  setWindowSize(window: number, width: number, height: number): void
  /**
   * Asks the driver to give a window keyboard focus; queued the same
   * way. Advisory: whether the window manager agreed shows up as
   * `env().focused` on the frames that follow, not as a reply.
   */
  focusWindow(window: number): void
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
  /** Shows `tree` in one window: `'main'` when `window` is left out, else
   *  a name `windows()` lists. */
  setView(tree: KuiNode, window?: string): void;
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
  /** The tree for one window. Called once per open window per frame with
   *  its name — `'main'` for the one the app starts in, else a name
   *  `windows` declared — so a single-window app ignores the argument. */
  view: (model: M, window: string) => KuiNode;
  /** Which windows exist besides `main`, by name (see `WindowDecl`). A
   *  window opens on the first frame that lists it and closes on the first
   *  that does not; the `WindowMsg` says when. Leave it out for one window. */
  windows?: (model: M) => WindowDecl[];
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

/** Opens the main window and runs the Elm loop; resolves with the final
 *  model when that window closes. `config.windows` opens more. */
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
  /** Corner radii of the clip, same order as `radii`: a clipping node with a
   *  radius rounds what it clips. All zero = a plain rect clip. */
  clipRadii: [number, number, number, number];
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
