import type {
  AppMsg,
  BoxProps,
  GeneratedSpecProps,
  KuiNode,
  MenuItemInput,
  MenuItemRole,
  TextProps,
} from './jsx-runtime.js';

export type { KuiNode, KuiElement, Msg, KuiMsg, AppMsg } from './jsx-runtime.js';
// The menu inputs live beside the other prop types, because `<menuBar/>`
// takes one; they are re-exported here because `Ctx.openMenu` takes one
// too, and an app should not have to know which file a type sits in.
export type { MenuItemInput, MenuItemRole, MenuInput, MenuBarInput } from './jsx-runtime.js';
export type { ColorToken, LengthToken } from './jsx-runtime.js';

// -- the messages the core itself sends ------------------------------------
// Payload shapes from `EVENTS` in crates/kui-core/src/schema.rs (the table
// docs/props.md is generated from). `tag` is the payload declared on the
// node (`onDrag` / `onHover` / `onKey`), left off when the node declared
// none.

/** A pointer-captured drag on an `onDrag` node. `x`/`y` are where the
 *  pointer is; `dx`/`dy` are its displacement **from the press point**, in
 *  every phase — `start` carries zero, a `move` how far the pointer is from
 *  where it pressed, `end` the whole distance — so a handler sets
 *  `value = start + dx` rather than summing deltas, and can commit from
 *  `end` alone. Nothing is dropped under the click slop (3 px, measured
 *  from the press): the first `move` already carries the whole distance.
 *  `parent` is the container rect, so fractions need no geometry query. */
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

/** A key press on the focused `onKey` sink — and its release too, as
 *  `phase: 'up'`, when the sink also declares `keyUp`; without that flag a
 *  sink hears presses only, so a keymap runs each binding once. `code` is a
 *  character or a name ("left", "f5"), `text` what the press would insert
 *  (null for a chord, and on every release), `repeat` set when the OS
 *  auto-repeated the press. A key only comes up where it went down: a release
 *  whose press the sink never got is dropped, and focus leaving while a key is
 *  held delivers the `up` first — so a held-key binding cannot be left stuck
 *  down. */
export type KeyMsg<T = AppMsg> = {
  kind: 'key';
  phase: 'down' | 'up';
  code: string;
  /** The US-QWERTY key at that *position*, spelled as `code` is — bind
   *  it when you want the finger rather than the label (WASD stays a
   *  square on every layout). A shifted letter arrives as the upper-case
   *  letter in `code` with `shift` set, as the OS spells it, and as the
   *  lower-case one here. */
  physical: string;
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  super: boolean;
  text: string | null;
  repeat: boolean;
  tag?: T;
};

/** Text an IME committed at the end of a composition, on the focused
 *  `onKey` sink (or the nearest one above the focused control) — the one
 *  committed text the platform never reports as a key press carrying
 *  `text`, so insert it as you would a key's `text`. Plain typing does not
 *  arrive this way: the `key` event already carries it, and a sink hearing
 *  both would type every character twice. A focused `<edit>` takes the
 *  commit itself and reports `changed`. */
export type TextMsg<T = AppMsg> = {
  kind: 'text';
  text: string;
  tag?: T;
};

/** An in-progress IME composition on the focused `onKey` sink: `text` is
 *  the uncommitted string to show inline at the caret, `cursor` the byte
 *  range inside it the IME's own caret covers (null when it does not say),
 *  and an empty `text` means the composition ended without a commit. The
 *  OS candidate window is anchored for you: the `line` carrying `caret`
 *  says where (`imeRect()` headless). */
export type PreeditMsg<T = AppMsg> = {
  kind: 'preedit';
  text: string;
  cursor: [number, number] | null;
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

/** The wheel over an `onScroll` node, or a drag-select held past a
 *  `cells` grid's top or bottom edge
 *  (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).
 *  `dx`/`dy` are the delta in logical px as the driver reported it
 *  (positive `dy` is the wheel rolling up, toward earlier content),
 *  `x`/`y` the pointer, and `lines` the whole lines a `cells` grid's `dy`
 *  covers — positive is later history, the sign `originLine` grows in,
 *  the fraction carried to the next notch — and null on any other node.
 *  The core scrolls nothing for it: re-declare the grid's `originLine`,
 *  or zoom the canvas. From the edge drag it comes once a frame while the
 *  pointer is held past the edge. */
export type ScrollMsg<T = AppMsg> = {
  kind: 'scroll';
  x: number;
  y: number;
  dx: number;
  dy: number;
  lines: number | null;
  tag?: T;
};

/** A menu row was chosen — a context menu's (`Ctx.openMenu`, or the stock
 *  one a right-click opens) or the application menu bar's
 *  (`docs/adr/0018-a-menu-bar-the-app-declares.md`). One message for both,
 *  so an app that wires Save once has it in both places.
 *
 *  `item` is the row's `id`, or its label when it declared none. `role`
 *  says whether the core already carried the row out: a standard role is
 *  performed (and its clipboard work queued for the host) *and* posted, so
 *  an app can hear its editor being cut from and is free to ignore it. */
export type MenuMsg<T = AppMsg> = {
  kind: 'menu';
  role: MenuItemRole;
  item: T;
};

/** A copy reached rows of a `selectable` virtual list that no frame built
 *  (ADR 0017, tier 3): the core cannot read text it never laid out, so it
 *  asks the app, on the scope, for the range — `from`/`to` are row indices
 *  (`index`, the row's data index) and byte offsets into each row's text —
 *  and the app answers with `answerSelectionRange(text)`, which is what
 *  reaches the clipboard. Only while `requestCopy()` answered `asked`; a
 *  late answer changes nothing. A Select All over a list that declared
 *  `rowCount` asks for `0..rowCount - 1` with `to.byte` past the last
 *  row's length (the row was never built) — cut it to the row. */
export type SelectionRangeMsg = {
  kind: 'selectionrange';
  from: { index: number; byte: number };
  to: { index: number; byte: number };
};

/** The pointer entered or left an `onHover` node — also when a new frame
 *  moved it under a still cursor. */
export type HoverMsg<T = AppMsg> = {
  kind: 'hover';
  phase: 'enter' | 'leave';
  tag?: T;
};

/** Files dragged in from the OS over an `onDrop` node
 *  (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`):
 *  `enter` when they come over the zone, `move` while they move over it
 *  (never twice for one point), `leave` when they go to another zone, to
 *  no zone or out of the window, `drop` when they land — and no `leave`
 *  after a `drop`. `paths` are the OS paths; `x`/`y` the pointer in
 *  logical viewport coordinates, absent on `leave`. The zone is the
 *  topmost one under the pointer by paint order: a node inside it is its,
 *  and a node that is no zone is looked past, so an overlay shown on
 *  `enter` does not end the hover. */
export type DropMsg<T = AppMsg> = {
  kind: 'drop';
  phase: 'enter' | 'move' | 'leave' | 'drop';
  paths: string[];
  x?: number;
  y?: number;
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
  /** Physical px per logical px at the node — `w × scale` by `h × scale`
   *  is how many pixels to render for it before `updateImage`. */
  scale: number;
  tag?: T;
};

/** A press that deepened past the second stage of a Force Touch trackpad,
 *  on an `onForceClick` node, at the logical viewport point it happened
 *  at. Routed as a secondary press is — no focus moved, no click — and the
 *  ordinary click the press is still producing arrives afterwards. Text
 *  needs none of this: over an `edit` or a `selectable` scope the core
 *  selects the word and asks the host for its Look Up panel instead. */
export type ForceClickMsg<T = AppMsg> = {
  kind: 'forceclick';
  x: number;
  y: number;
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

/** The viewport changed size or DPI (logical px, delivered on the root) —
 *  the window's, or what the devtools' dock leaves of it while the panel
 *  is docked, so a dock coming, moving or being dragged is one too;
 *  `win.size()` queries the same numbers. */
export type ResizeMsg = {
  kind: 'resize';
  width: number;
  height: number;
  scale: number;
};

/** An OS setting changed while the app was open — the appearance, the
 *  accent colour, reduced motion or the UI language — or assistive
 *  technology started listening (delivered on the root, one per window
 *  that noticed). The payload is `env().system` as it now reads, so a
 *  handler keeps the whole reading or takes the one field it branches on.
 *
 *  A window's view runs when a message changes the model, so without this
 *  a palette picked from `env().system.appearance` is the one the first
 *  frame read and stays it: the driver's redraw re-lowers the tree it was
 *  handed, it does not re-run `view`. The first frame establishes the
 *  reading rather than reporting it, the way `ResizeMsg` does. */
export type SystemMsg = {
  kind: 'system';
  appearance: 'unknown' | 'light' | 'dark';
  /** `0xRRGGBBAA`, or null where the host cannot tell. */
  accent: number | null;
  motion: 'unknown' | 'full' | 'reduced';
  /** A BCP-47 tag, or null where the host cannot tell. */
  locale: string | null;
  assistive: 'unknown' | 'none' | 'listening';
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
 *  root for `play`. `'refused'` is the device declining to start it at all
 *  (its 128 voices are all held, or the sound did not decode): that playback
 *  never starts and so never ends, so this arrives in place of the `'ended'`
 *  a view would otherwise wait forever for. */
export type SoundMsg<T = AppMsg> = {
  kind: 'sound';
  phase: 'ended' | 'refused';
  playback: number;
  tag: T;
};

/** Everything the core sends on its own. Put it in the app's union —
 *  `type Msg = MyMsg | CoreMsg` — and `update` switches over one flat
 *  discriminated union, no casts and no narrowing preamble. */
export type CoreMsg =
  | DragMsg
  | KeyMsg
  | TextMsg
  | PreeditMsg
  | ContextMenuMsg
  | ScrollMsg
  | MenuMsg
  | SelectionRangeMsg
  | HoverMsg
  | DropMsg
  | LayoutMsg
  | ForceClickMsg
  | DismissMsg
  | ResizeMsg
  | SystemMsg
  | WindowMsg
  | ModifiersMsg
  | EditMsg
  | SoundMsg
  | AccessMsg;

/** Assistive technology nudged a `slider` role. `tag` is the node's
 *  `onClick` payload (or its `onDrag` / `onKey` tag), typed as the app's
 *  own union like every other core message, so a handler reads it with no
 *  cast. Activation, focus, text and scrolling requests resolve in the
 *  core and arrive as the messages a pointer would have produced. */
export interface AccessMsg<T = AppMsg> {
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
  tag?: T;
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

// -- generated from the core's access lists; edit Role::ALL / AccessAction::ALL in crates/kui-core/src/access.rs, then `npm run gen` --
/** Every role a node of the tree can report: the ones a view declares
 *  (`role`), the ones the core derives (a `cells` grid is a `terminal`,
 *  an editor a text input, a scrolling box a scroll view, the root the
 *  window). */
export type AccessRole =
  | 'none' | 'button' | 'checkbox' | 'radio' | 'switch' | 'slider' | 'tab'
  | 'tabList' | 'link' | 'heading' | 'list' | 'listItem' | 'image' | 'dialog'
  | 'group' | 'window' | 'titleBar' | 'staticText' | 'textInput'
  | 'multilineTextInput' | 'scrollView' | 'line' | 'radioGroup' | 'menu'
  | 'menuItem' | 'terminal';

/** What assistive technology can ask of a node (`access(key, action)`). */
export type AccessAction =
  | 'click' | 'focus' | 'blur' | 'setValue' | 'increment' | 'decrement'
  | 'scrollIntoView' | 'scrollUp' | 'scrollDown' | 'scrollLeft'
  | 'scrollRight' | 'setTextSelection' | 'replaceSelectedText';
// -- end generated --

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
  /** What the `description` prop sets, or the `tooltip` shorthand: both
   *  write this one slot, and the later declaration wins. */
  description: string | null;
  /** Logical px, viewport coordinates. */
  rect: { x: number; y: number; w: number; h: number };
  /** The node's one string value: an editor's text, with its caret and
   *  non-empty selection as byte offsets, or a slider's `valueText` — a
   *  slider that named its reading reads as that instead of its number. */
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
  /** `valueNow` / `valueMin` / `valueMax` for a slider. What the position
   *  reads as is `valueText`, which arrives in `value` above. */
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
  /** Declared `live`: when the text inside this node changes, a reader
   *  reads the change without being asked. `'off'` for every other node
   *  (`docs/adr/0008-live-regions-and-announcements.md`). */
  live: Live;
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

/** Where a point landed in the text a keyed node drew (`textHit`): a byte
 *  offset into that text — across the node's text runs in order, the way
 *  the access tree reads a `line` — and the visual (wrapped) line, 0-based.
 *  `byte` is a caret position: between two characters, past the last one
 *  at the end. */
export interface TextHit {
  byte: number;
  line: number;
}

/** The application menu as the core holds it (`Ctx.menuBar()`): what a
 *  host with a menu bar of its own reads after `setNativeMenuBar(true)`.
 *  `revision` changes only when the declaration does, so a host keeps the
 *  last one it built and rebuilds nothing until the number moves. */
export interface MenuBarState {
  revision: number;
  menus: {
    label: string;
    enabled: boolean;
    items: OpenMenuItem[];
  }[];
}

/** One row of a menu, as `Ctx.menu()` and `Ctx.menuBar()` both report it
 *  (`docs/adr/0017-selection-as-a-scope.md`): what the drawn menu would
 *  show. `label` is the row's own text or, for a standard role that
 *  declared none, the role's wording; a host rendering natively uses `role`
 *  to pick the platform's own wording instead, and draws `accel` beside
 *  it. */
export interface OpenMenuItem {
  label: string;
  role: MenuItemRole;
  /** Drawn with a checkmark: a setting the row *is* rather than a command
   *  it runs. Present on every row; a context menu's rows can carry it as
   *  a bar's can. */
  checked: boolean;
  /** A disabled row is drawn dimmed and cannot be chosen — Paste with an
   *  empty clipboard, Copy with no selection. Present rather than absent,
   *  so a menu's rows do not move under the pointer. */
  enabled: boolean;
  /** Display only: the shortcut is the app's or the platform's. The row's
   *  own, or its role's default (`⌘C` on a `copy` row that declared none);
   *  null where there is neither. */
  accel: string | null;
}

/** The menu a window has open (`Ctx.menu()`): where it opened, the node it
 *  is about, and its rows. Read it after `setNativeMenus(true)`, show it
 *  however the platform does, and answer with `activateMenuItem(i)` or
 *  `closeMenu()`. */
export interface OpenMenu {
  /** The node the menu is about; chosen rows post their event on it. */
  target: string;
  x: number;
  y: number;
  items: OpenMenuItem[];
}

/** What choosing a menu row left for the host (`Ctx.takeMenuActions()`).
 *  The clipboard is the host's in this library: the core works out *what*
 *  to copy, which is the half only it can do, and hands over the text.
 *
 *  `setClipboard` carries `text` (and `html` where the selection had
 *  formatting to carry — beside the text, never instead of it);
 *  `paste` asks for what is on the clipboard, delivered back with
 *  `Ctx.commit(...)` — a focused editor takes it as typing, a focused
 *  `onKey` sink hears it as `{kind:"text"}`; `lookUp` asks for the
 *  platform's definition panel for `text`, anchored at the baseline
 *  origin `x`, `y`. A view queues the first two itself with
 *  `setClipboard` / `requestPaste`. */
export type MenuAction =
  | { kind: 'setClipboard'; text: string; html: string | null }
  | { kind: 'paste' }
  | { kind: 'lookUp'; text: string; x: number; y: number };

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
  /** A label resolved by name (`focus("beta")` in Node, `env.set_focus("beta")`
   *  in Lua, `kui_key_of` in C) is declared by more than one node in the frame,
   *  under different parents, so they have distinct keys and the name picked
   *  the first in tree order. Labels are unique among siblings, not across a
   *  tree. An extension asking from inside its fill is answered from the nodes
   *  it opened and no one else's, and the host from its own first — so this is
   *  a clash among the asker's own. Give the node meant a label nothing else
   *  declares, or pass the hex key an event carried. Two nodes with the *same*
   *  key are `duplicate-key`. */
  | 'ambiguous-key'
  /** A `focusRegion(name)` (`Core::focus_region`, `env.focus_region`,
   *  `kui_focus_region`) named a node the frame after it did not declare as a
   *  `focusRegion` — no node under the label, or a node without the row — so
   *  nothing was entered and focus stayed where it was. The call is resolved
   *  against the frame it lands on, so an `update` that toggles a dock on and
   *  enters it in one go is fine; this is that call with the view half missing,
   *  with a name the view spells differently, or naming a node that is not a
   *  region (`docs/adr/0022-focus-regions.md`, decision 4). */
  | 'focus-region-without-node'
  /** A `selectable` node inside another `selectable` node. Selection scopes do
   *  not nest: the innermost one owns every run under it, so the outer scope
   *  selects only the text outside the inner one — and a drag that crosses the
   *  boundary stops there, which reads as a selection that will not extend.
   *  Declare the scope once, on the container whose text should select as one. */
  | 'nested-selection-scope'
  /** An image with no `label`: assistive technology has nothing to say for it.
   *  Decorative images take `role="none"`. */
  | 'image-without-label'
  /** A `slider` whose `valueNow` lies outside its own `valueMin` / `valueMax`,
   *  or whose `valueMin` is above its `valueMax`. The row is advertised
   *  verbatim, so a screen reader reads a value the range says is impossible;
   *  the app that clamps in its own `update` keeps the range in two places with
   *  nothing tying them, and this is the tie. Declare the range the value is
   *  really held to, or clamp where the view declares it. */
  | 'slider-value-out-of-range'
  /** `Core::add_fragment` was given WGSL that does not compile, so no handle
   *  was minted and nothing will draw. The message carries naga's own error
   *  with the line numbers moved into the app's source
   *  (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`, decision
   *  1). The source is rejected here rather than at the first frame that shows
   *  it, so a headless test sees it too. */
  | 'fragment-rejected'
  /** A `fragment` node declared more than sixteen `params`. The shader takes
   *  four `vec4<f32>` and no more, so the extra numbers were dropped; pass
   *  fewer, or pack what the fragment needs into the sixteen it has. */
  | 'fragment-params-truncated'
  /** A `polygon` declared more than eight points: the stock fragment takes
   *  eight vertices in the sixteen params it has, so the rest were dropped. Two
   *  polygons, or the path primitive kui does not have
   *  (`docs/adr/0025-the-image-is-the-canvas.md`, decision 6). */
  | 'polygon-points-truncated'
  /** The frame's modal surface is not in a float, and content painted after it
   *  is drawn on top of it: everything the user can see over the modal is
   *  inert, which looks like inert-behind is broken. A modal that has to cover
   *  the app is a float (`float="viewport"`); see
   *  `docs/adr/0003-modal-surfaces.md`. Also raised for a modal that *is* a
   *  float when another float from outside its scope stacks over it
   *  (`docs/adr/0023-layers-stack-in-the-order-they-open.md`): a HUD opened
   *  after the dialog is the same inert surface over it. */
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
  /** `wrapChildren` on a container that cannot break lines: a column, a row
   *  whose main axis scrolls, or a row of a table, whose children are the
   *  table's columns. All lay out exactly as if the flag were absent, which
   *  reads as "wrapping is broken"; see `LayoutSpec::wrap` for why a column
   *  cannot have it. */
  | 'wrap-ignored'
  /** A text node sits more than four levels below the `line` row above it,
   *  which is as far as a text's place remembers its ancestors — so `textHit` /
   *  `caretRect` asked by that row's key cannot find the run, and a press
   *  inside it reports `byte: 0`. Flatten the wrappers between the row and its
   *  text, or ask by a nearer key. */
  | 'text-beyond-line'
  /** One frame removed more nodes declaring `exit` than the exit store will
   *  hold (512, `depart::MAX_NODES`), so none of that frame's removal animated:
   *  every departing node of it vanished at once, as a node with no `exit`
   *  does, rather than some sliding out and the rest blinking
   *  (`docs/adr/0012-the-exit-budget.md`, decision 2). Correct, and invisible
   *  from the outside, which is the whole reason it is a line here: a list that
   *  drops a thousand rows wants `exit` on the list, not on every row. A
   *  removal that fits the budget but finds earlier exits still in flight
   *  evicts those, oldest first, and is not this warning. */
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
  /** A `setEditText` (`Core::set_edit_text`, `kui_edit_set_text`) named a key
   *  or a label, the text was held for the frame that would declare it, and the
   *  frame after the call declared no editor under that name — so nothing was
   *  ever seeded and the text is dropped. The call is meant to run from an
   *  `update` that also opens the editor, one frame ahead of the view that
   *  declares it; this is the same call with the view half missing, or with a
   *  name the view spells differently. Pass the label the editor's `key` prop
   *  declares — the spelling that needs nothing to exist yet — or the hex key
   *  an event carried. `keyOf`/`kui_key_of` turns a label into that key, but
   *  only for an editor some frame declared (Lua's verbs take the label
   *  itself). An editor that already exists takes the text where the call is
   *  made and never reaches this. */
  | 'edit-text-without-editor'
  /** A one-shot `audio` node went away — or changed its `src` — while the sound
   *  it started was still playing, so the user heard it cut off. Almost always
   *  a duration guessed short: the view keeps the node declared for a constant
   *  it picked, and the asset is longer. Ask for the sound instead of the guess
   *  — `finish` (`AudioSpec::finish`, `finish` in JSX and Lua) releases the
   *  playback on removal so it plays itself out, and a `tag` reports
   *  `{kind:"sound", phase:"ended"}` when it gets there. A view that means to
   *  cut the sound off says so by stopping what it started (`Core::stop`),
   *  which is not reported, and a `looped` playback never is: it has no end to
   *  be short of. Only a driver with a real device raises it, because only a
   *  device knows the sound was still running: the core queues the `stop` and
   *  the driver answers `Core::audio_truncated` for the ones its handle found
   *  still playing. A headless `Ctx` therefore never raises it — nothing plays
   *  — and the assertion point there is the other end of the same fact:
   *  `audioCommands()` holding a `stop` for the node, where the suite expected
   *  none. */
  | 'truncated-playback'
  /** A `FontId` / `ImageId` / `SoundId` registered in one `Session` and used
   *  through a core of another. Handles are unique to the process, so it cannot
   *  resolve to somebody else's resource; it behaves as a removed handle does
   *  (draws nothing, shapes as sans-serif, plays nothing), which from outside
   *  looks like the resource never registered. Two `Core::new()`s are two
   *  sessions; windows that share resources are built with `Core::new_in`
   *  against one `Session`. */
  | 'foreign-resource'
  /** An extension names a slot no host declared this frame, so it drew nothing.
   *  A slot is a position the host declares in its own view by full name,
   *  `ui.slot("ns/name")` — the namespace the host gave the extension, then the
   *  name the extension lists; one listing none fills `"ns/root"` after the
   *  host's view. Declare the slot, or drop the name from the extension's list.
   *  See `docs/adr/0014-slots-an-extension-fills-in-place.md`, decision 5. */
  | 'unknown-slot'
  /** A colour or length prop named a token — `bg = "$peach"` — that nothing
   *  declared and that is no theme or metrics role, or named one of the other
   *  kind (a length in a colour slot). The slot is left at the row's default,
   *  as if the prop had not been written — no `bg`, a fit width, the theme's
   *  foreground for a text's `color` — never an explicit transparent or zero,
   *  which would hide the node a typo was on; the same in every binding and in
   *  every place a `$name` can go, a keyframe stop and an entrance included
   *  (backlog AR14), and what `ui.token_color` / `token_length` answer `None`
   *  for in Rust. Raised by the binding that lowered the reference, through
   *  `Core::warn_unknown_token`, once per name, since the name is gone by the
   *  time the frame is a tree. Also raised at the declaration for a derived
   *  token whose source — the `from`, or the colour a `mix` or `readable` names
   *  — is no colour token declared before it and no theme role: that token is
   *  dropped, the message names both, and the rest of the table lands. See
   *  `docs/adr/0027-tokens-beside-the-theme.md`, decision 4, and
   *  `docs/adr/0028-derived-tokens.md`. */
  | 'unknown-token'
  /** A declared token took a theme or metrics role's name (`surface`, `radius`)
   *  and was dropped: the roles are the corpus's contract and `$surface` always
   *  means the theme's, so an app cannot shadow one. Rename the token. See
   *  `docs/adr/0027-tokens-beside-the-theme.md`, decision 6. */
  | 'reserved-token'
  /** A slot name declared twice in one frame. The second declaration was
   *  ignored: a fill is keyed by the slot's full name, so two fills of one name
   *  would share every key. Two places for one extension are two names. See ADR
   *  0014, decision 5. */
  | 'duplicate-slot'
  /** An extension returned from `view` with nodes still open. The core closed
   *  them at the depth the fill began, so the host's tree is what the host
   *  declared; outside the guard, the rest of the host's view would have landed
   *  inside the extension's last open node. The extension has an `open` without
   *  its `close`. See ADR 0014, decision 5. */
  | 'unbalanced-extension'
  /** An extension's `view` returned an error. The message is drawn in red where
   *  the fill would have been, and reported here once per extension and slot
   *  rather than once per frame. */
  | 'extension-view-error'
  /** An extension declared one of its *own* slots while it was drawing, so
   *  filling it would have meant calling it inside itself. The slot is left
   *  empty. An extension may host extensions (`Fill::add`), and may declare
   *  their slots — what it cannot do is be its own guest. See ADR 0014,
   *  decision 5. */
  | 'recursive-slot'
  /** The device refused a play: its voices are all held, or the sound did not
   *  decode. A released playback (`finish`) holds one of the device's 128
   *  voices until its file ends, so a view that releases faster than its sounds
   *  finish reaches the limit and the 129th play is refused. The refusal is not
   *  left silent because the playback never starts and so never ends: a
   *  `tag`ged node waiting for `ended` would wait forever. It gets
   *  `{kind:"sound", phase:"refused"}` instead, and this line says why. Stop
   *  what the view no longer needs rather than releasing it, or release shorter
   *  sounds. */
  | 'playback-refused'
  /** A devtools tab name declared twice in one frame (ADR 0032, decision 1):
   *  two `devtools_tab` / `devtools_tab_with` calls, a host form and an
   *  extension form of one name, or an extension declaring from its fill under
   *  a name the host took. The first declaration stands and the second is
   *  ignored; give the second tab its own name. */
  | 'duplicate-tab'
  /** A `devtoolsTab` declaration a binding could not read as either form (ADR
   *  0032, decision 1): a child that is not a function, both a `slot` and a
   *  child, or a `view` that is not a function in Lua. The tab was not
   *  declared. A tab names a slot for an extension to fill, or carries a
   *  function the binding calls only when the tab is shown. */
  | 'bad-devtools-tab';
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
  /** The slot whose fill drew the node, as its key (`keyOf` of the slot's
   *  full name answers the same), or null for a node your app drew. One
   *  extension fills many slots, so `origin` cannot say which; this is
   *  what routes an event by the slot it came from. */
  slot: string | null;
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

/** What the last frame left owed, by kind: `animating()` taken apart. The
 *  window redraws for any of them; a test reads `cycle` apart from the
 *  rest, since a keyframe `repeat` cycle never ends (backlog F64). */
export interface Owed {
  /** A finite transition — a leg or a spring — still mid-flight. */
  transition: boolean;
  /** A keyframe cycle running; it always is, while its node is drawn. */
  cycle: boolean;
  /** An exit animation (a ghost) still departing. */
  depart: boolean;
  /** A frame a view asked for: `requestFrame`, or an `animate` node. */
  requested: boolean;
  /** A held drag scrolling its container. */
  autoscroll: boolean;
}

/** The window's frame timing: the averages and maxima over the last 120
 *  frames, and two monotonic counts. */
export interface FrameTiming {
  /** How many frames the ring holds: climbs to 120 in the window's first
   *  two seconds and stays there. Not a count of frames — that is
   *  `framesTotal`. */
  frames: number;
  /** Every frame the window has painted, since it opened (backlog F62).
   *  Two readings a second apart are that second's frame rate. */
  framesTotal: number;
  /** Every `pump()` the window has taken, the one that opened it
   *  included — what the driver's backoff is measured in: a stopped app
   *  with a once-a-second tick takes ~30 pumps a second at the default
   *  `idlePumpMs`, ~60 in the half-second after a click. */
  pumps: number;
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
  /** What the user set in the OS. */
  system: SystemEnv;
  /**
   * The current or last frame's viewport: the window less the devtools'
   * dock while the panel is docked — what `KuiWindow.size()` answers and a
   * `resize` reports. 0×0 before the first frame.
   */
  viewport: WindowSize;
  window: WindowEnv;
  /** What the driver's audio output is doing. */
  audio: AudioEnv;
}

/** One node of the last finished frame, as `nodes()` reads it back (after
 *  `setInspect(true)`): what it is, the label it was opened under, where
 *  layout put it in logical viewport px, and the declarations that make
 *  it interactive or special. What a tree view and a node inspector are
 *  built from. */
export interface NodeInfo {
  key: string;
  parent: string | null;
  /** Nesting depth; the root is 0. */
  depth: number;
  kind: 'box' | 'text' | 'edit' | 'image' | 'line' | 'cells' | 'fragment' | 'polygon';
  label: string | null;
  rect: Rect;
  dir: 'row' | 'column';
  /** A table (ADR 0033): a column whose rows' cells line up in columns. */
  table: boolean;
  /** `fit`, `grow(n)`, `<n>px` or `<n>%`, as the spec spelled it. */
  width: string;
  height: string;
  /** `0xRRGGBBAA`; 0 alpha is none. */
  bg: number;
  float: boolean;
  role: string | null;
  /** A text node's content, cut to a line's worth. */
  text: string | null;
  /** `click`, `drag`, `key`, `hover`, `hoverable`, `context-menu`, `modal`,
   *  `selectable`, `focusable`, `disabled`, `scroll`, `clip`, `transition`. */
  flags: string[];
  /** The paint layer: 0 in flow, else the float layer's rank from the
   *  bottom (1 is the first layer over the flow). */
  layer: number;
  /** Who declared it: 0 the app, 1+ an extension, 65535 the devtools. */
  origin: number;
  children: number;
  padding: { t: number; r: number; b: number; l: number };
  gap: number;
  mainAlign: 'start' | 'center' | 'end';
  crossAlign: 'start' | 'center' | 'end';
  wrap: boolean;
  /** Size floors and ceilings in px; a floor is `null` for the fit floor,
   *  a ceiling `null` when unbounded. */
  minWidth: number | null;
  minHeight: number | null;
  maxWidth: number | null;
  maxHeight: number | null;
  /** Per corner: top-left, top-right, bottom-right, bottom-left. */
  radius: [number, number, number, number];
  borderWidth: number;
  /** `0xRRGGBBAA`. */
  borderColor: number;
  opacity: number;
  /** A scroller's offset; `null` for a node that does not scroll. */
  scroll: { x: number; y: number } | null;
  /** Every handler it declared with the payload it would post: `click`,
   *  `drag`, `key` (the sink's tag), `hover`, `context-menu`,
   *  `force-click`, `layout`, `modal`. */
  events: Record<string, unknown>;
}

/** Where the core's devtools panel sits (`setDevtoolsDock`): beside the
 *  app's tree in the main window (a docked pane's inner edge is a handle
 *  that resizes it), in a window of its own named `kui-devtools`, or
 *  hidden with its chords still live. `'side'` is accepted as `'right'`. */
export type DevtoolsDock = 'left' | 'right' | 'bottom' | 'window' | 'off';

/** The devtools panel's own tabs (`setDevtoolsTab`, `devtoolsCurrentTab`):
 *  the runtime's facts, the event stream, the node tree. A `<devtoolsTab>`
 *  the app declares is named by its `name` beside these. */
export type DevtoolsTab = 'facts' | 'events' | 'tree';

/** The name of the core's devtools window while the panel is popped out
 *  (`setDevtoolsDock('window')`): what `windows()` lists it as, and what
 *  `runWindowed` never asks the app's `view` to draw. */
export declare const DEVTOOLS_WINDOW: 'kui-devtools';

/** The output device's state and how many playbacks are live, as the
 *  driver reports them each frame. A fact and not a verb — nothing closes
 *  the device from here; the runner does that itself once it has been idle
 *  a while. Worth reading because an open stream is a real-time thread
 *  whether or not anything plays, which is the whole of an idle app's CPU
 *  once it has held a sound: `device: 'open'` with `live: 0` long after the
 *  last sound is a bug that otherwise only `top` can see. A headless `Ctx`
 *  reads `'closed'` and 0. */
export interface AudioEnv {
  /** `'closed'` (the default), `'opening'` (the ~90 ms open, off-thread),
   *  `'open'`, or `'failed'` (it refused; commands are dropped). */
  device: 'closed' | 'opening' | 'open' | 'failed';
  /** Playbacks started and not yet ended, plus any waiting on the open. A
   *  play that arrives while the device is `'opening'` counts here from
   *  the frame it was asked until the open answers; if the device
   *  refuses, the play is refused on the next apply — a `SoundMsg` with
   *  `phase: 'refused'` for a tagged one — and leaves the count with it,
   *  so a machine with no output device shows `opening`/1, then
   *  `failed`/0 with the refusal between (backlog F63). */
  live: number;
}

/** The palette a frame paints with: one `0xRRGGBBAA` number per role,
 *  derived from `env().system` — the OS's light/dark picks the base and the
 *  OS's accent recolours it — unless the app called `setAccent` or
 *  `setTheme`. Pass a role straight to a prop: `<box bg={theme.surface}>`.
 *
 *  The stock widgets already read it, so an app built out of `<button>`,
 *  `<text>` and the context menu follows the OS's appearance without
 *  touching this. Read it when painting something of your own. */
export interface Theme {
  /** Which base this came from. `'unknown'` is the dark base, without
   *  claiming the user chose it — what kui painted before themes. */
  appearance: 'unknown' | 'light' | 'dark';
  // -- generated from the core's theme roles; edit THEME_ROLES in crates/kui-core/src/schema.rs, then `npm run gen` --
  /** The window behind everything. */
  bg: number;
  /** A card, panel or list sitting on `bg`. */
  surface: number;
  /** A surface floating above content: a menu, a tooltip, a popover. Under a
   *  light theme it is no lighter than `surface` — a float on a white page
   *  separates by its border. */
  raised: number;
  /** A well cut into a surface: a text field, a code block, a track. */
  sunken: number;
  /** The hairline between two surfaces. */
  border: number;
  /** A border that has to be seen — a float's edge, a focused field. */
  borderStrong: number;
  /** Body text, and what a `color`-less text run resolves to. */
  fg: number;
  /** Secondary text: captions, hints, an accelerator beside a label. */
  muted: number;
  /** Text that is barely there: a placeholder, a gutter number. */
  faint: number;
  /** The one saturated colour: the OS accent where the host reports one, the
   *  app's where it pinned one, kui's blue otherwise. The `accent` prop
   *  paints from this. */
  accent: number;
  /** `accent` under a pointer. */
  accentHover: number;
  /** `accent` under a press. */
  accentPressed: number;
  /** Black or white — whichever a reader can see on `accent`. What a button's
   *  label is. */
  onAccent: number;
  /** The accent as a translucent wash rather than a fill: a selected menu
   *  row, a chosen tab, a highlighted list item. Keeps `fg` readable over it
   *  on both bases, which a fill does not. */
  accentSoft: number;
  /** What a text selection is painted under, in an editor and over a
   *  `selectable` scope alike. */
  selection: number;
  /** The default keyboard focus ring (ADR 0002). */
  focusRing: number;
  /** A translucent wash over a hovered neutral control. An overlay, not a
   *  fill, so one value works on every surface. */
  hover: number;
  /** The same over a pressed one, and the firmer of the two on both bases. */
  pressed: number;
  /** A good outcome. Readable on `surface` on both bases, which is why it is
   *  not one colour for both. */
  success: number;
  /** Something that wants attention. */
  warning: number;
  /** A destructive action or a failure. The close button's hover, too. */
  danger: number;
  /** The scrollbar thumb at rest. */
  scrollbar: number;
  /** The thumb while hovered or dragged. */
  scrollbarActive: number;
  // -- end generated --
  /** What a disabled control's opacity is multiplied by. */
  disabledOpacity: number;
}

/** `setTheme`'s argument: a base, then the roles to change on top of it.
 *  Every colour takes the two spellings a prop takes — `0xRRGGBBAA` or
 *  `'#rrggbb'`.
 *
 *  `accent` is not just one role: naming it also recomputes `accentHover`,
 *  `accentPressed`, `onAccent`, `accentSoft`, `selection` and `focusRing`
 *  from it, and any of those named explicitly then wins. An unknown key
 *  throws rather than doing nothing. */
export interface ThemeOverrides {
  /** Which base to start from; the OS's when absent. */
  appearance?: 'unknown' | 'light' | 'dark';
  // -- generated from the core's theme roles (overrides); edit THEME_ROLES in crates/kui-core/src/schema.rs, then `npm run gen` --
  bg?: number | string;
  surface?: number | string;
  raised?: number | string;
  sunken?: number | string;
  border?: number | string;
  borderStrong?: number | string;
  fg?: number | string;
  muted?: number | string;
  faint?: number | string;
  accent?: number | string;
  accentHover?: number | string;
  accentPressed?: number | string;
  onAccent?: number | string;
  accentSoft?: number | string;
  selection?: number | string;
  focusRing?: number | string;
  hover?: number | string;
  pressed?: number | string;
  success?: number | string;
  warning?: number | string;
  danger?: number | string;
  scrollbar?: number | string;
  scrollbarActive?: number | string;
  // -- end generated --
  disabledOpacity?: number;
}

/** The sizes the stock widgets are built from — the palette's other axis
 *  (backlog T2). Logical px, before the scale factor the renderer applies.
 *  Read it so a control of your own agrees with `<button>`, the field and
 *  the menus on a radius and a padding: `<box radius={metrics.radius}>`.
 *  The stock set is the constants the widgets always had; `setMetrics`
 *  changes it, and nothing in the OS is followed. */
export interface Metrics {
  // -- generated from the core's metric roles; edit METRIC_ROLES in crates/kui-core/src/schema.rs, then `npm run gen` --
  /** A stock control's label: the button's text size. */
  controlText: number;
  /** The chrome's text: a menu row, a menu-bar title, the titlebar's title. */
  chromeText: number;
  /** A tooltip's text. */
  hintText: number;
  /** The corner of every stock surface: a button, a field, a menu, a tooltip. */
  radius: number;
  /** The corner of a row inside one: a menu row, a menu-bar title. */
  radiusInner: number;
  /** A button's horizontal padding. */
  controlPadX: number;
  /** A button's vertical padding. */
  controlPadY: number;
  /** A text field's horizontal padding. */
  fieldPadX: number;
  /** A text field's vertical padding. */
  fieldPadY: number;
  /** A tooltip's horizontal padding. */
  hintPadX: number;
  /** A tooltip's vertical padding. */
  hintPadY: number;
  /** A menu row's horizontal padding, and a menu-bar title's. */
  menuPadX: number;
  /** A menu row's vertical padding; a menu-bar title's is two px less. */
  menuPadY: number;
  /** A menu panel's width. */
  menuWidth: number;
  /** The drawn menu bar's height. */
  menuBarH: number;
  /** The titlebar's height where the strip is the app's alone: the platform's
   *  caption height, 32 on Windows and 34 elsewhere. Under macOS custom
   *  chrome the strip is the OS's own titlebar, as tall as
   *  `window.native_controls` measures it (32 on macOS 27, 28 before), and
   *  this row is not read (`widgets::titlebar_height`). */
  titlebarH: number;
  // -- end generated --
}

/** `setMetrics`'s argument: overrides on top of the set in effect, or on
 *  top of `base` — `'comfortable'` is the stock set, `'compact'` a dense
 *  tool's (smaller text, shallower padding, sharper corners, the titlebar
 *  unchanged) — then `scale` multiplying every length: a density slider.
 *  An unknown key throws rather than doing nothing. */
export interface MetricsOverrides extends Partial<Metrics> {
  base?: 'comfortable' | 'compact';
  scale?: number;
}

/** The OS settings on `Env` — appearance, accent, motion, locale — as the
 *  host reported them, and beside them whether assistive technology is
 *  listening. Every one of them can be "the host cannot tell", and that is
 *  the default: a `KuiWindow` asks the OS for all four on macOS and
 *  Windows (and for the locale from `LANG` elsewhere) and hears from its
 *  accessibility bridge for the fifth, a headless `Ctx` knows only what
 *  `setEnv` told it. Treat an unknown as "use my own default" rather than
 *  as an answer. Nothing in kui acts on these; a view that honours them
 *  does so where it picks a colour, declares an animation or decides
 *  whether an alert announces. */
export interface SystemEnv {
  /** The OS light/dark setting. `'unknown'` where the platform has no
   *  answer (X11, Wayland without an override) or nobody pushed one. */
  appearance: 'unknown' | 'light' | 'dark';
  /** The OS accent colour as `0xRRGGBBAA` — pass it straight back as a
   *  `bg` or `color` — or null when the host cannot tell. */
  accent: number | null;
  /** Whether the user asked for reduced motion. `'reduced'` is the
   *  request, `'full'` is its absence, `'unknown'` is nobody having asked
   *  the OS — so test for `=== 'reduced'`, not for truthiness. */
  motion: 'unknown' | 'full' | 'reduced';
  /** The UI language as a BCP-47 tag (`'en-US'`), unparsed, or null when
   *  the host cannot tell. */
  locale: string | null;
  /** Whether assistive technology is listening: `'listening'` once an
   *  accessibility client asked this window for its tree, `'none'` while
   *  the bridge is up and nobody has, `'unknown'` where there is no bridge
   *  (a headless `Ctx`). The reading that changes what a view *says*
   *  rather than what it draws — an alert that announces when something
   *  is listening and blinks when nothing is — so test for
   *  `=== 'listening'`. Any client counts (a probe, an inspector, a screen
   *  reader alike), and on macOS and Windows nothing reports a client
   *  leaving, so once risen it stays for the window's life; only AT-SPI
   *  reports deactivation. Arrives as a `SystemMsg` when it changes. */
  assistive: 'unknown' | 'none' | 'listening';
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
  /** The window is above every other app's: what the runner applied after
   *  the frame asked (a root `<box alwaysOnTop>`), not what was asked — a
   *  platform can refuse or drop the level, and on Wayland winit has no
   *  call for it, so it reads false there however often the app asks. A
   *  pin button draws its state from this. */
  alwaysOnTop: boolean;
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
  /** What the user set in the OS, as a real host would have pushed it.
   *  `'unknown'`, and null for the two values, are the readings a host that
   *  cannot tell reports — they are settings, not absences, so a test can
   *  declare them. `accent` also takes the `'#rrggbb'` spelling a prop
   *  takes; a `locale` that is not an ASCII tag of at most 31 bytes throws
   *  rather than being stored truncated. */
  system?: {
    appearance?: 'unknown' | 'light' | 'dark';
    accent?: number | string | null;
    motion?: 'unknown' | 'full' | 'reduced';
    locale?: string | null;
    assistive?: 'unknown' | 'none' | 'listening';
  };
  window?: {
    /** Which window a headless `Ctx` is standing in for; every `UiEvent` it
     *  hands out carries it. Real windows get theirs from their runner. */
    id?: number;
    customChrome?: boolean;
    maximized?: boolean;
    fullscreen?: boolean;
    alwaysOnTop?: boolean;
    /** `x` and `y` default to the window origin; a zero-sized rect and null
     *  both mean "nothing is drawn over us". */
    nativeControls?: Partial<Rect> | null;
  };
  /** What a driver with a device would report; see `AudioEnv`. */
  audio?: {
    device?: 'closed' | 'opening' | 'open' | 'failed';
    live?: number;
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

// -- generated from the core's input lists; edit EditKey::ALL / MouseButton::NAMED in crates/kui-core/src/input.rs, then `npm run gen` --
/** The buttons `ctx.mouse` takes by name; anything else is a code. */
export type MouseButtonName =
  | 'primary' | 'secondary' | 'middle';

/** The editing keys `ctx.key` takes, the spelling the corpus steps use. */
export type EditKeyName =
  | 'left' | 'right' | 'up' | 'down' | 'home' | 'end' | 'pageup' | 'pagedown'
  | 'backspace' | 'delete' | 'enter' | 'tab' | 'selectall' | 'undo' | 'redo'
  | 'escape';
// -- end generated --

/** One end of the text selection as `selectionEnds()` reads it: the data
 *  index of the virtualised row it is in (null outside every virtualised
 *  row) and the byte inside that row's own text. */
export interface SelectionEnd {
  index: number | null;
  byte: number;
}

/** The text selection's two ends as the drag made them — the anchor where
 *  the press landed, the focus where the pointer is — so a Shift-click
 *  that kept the anchor reads as one (ADR 0029). */
export interface SelectionEnds {
  anchor: SelectionEnd;
  focus: SelectionEnd;
}

/** One end of a `cells` grid's selection: an absolute line (`originLine`
 *  plus the row, so a scroll does not move it) and a column. */
export interface CellEnd {
  line: number;
  col: number;
}

/** A `cells` grid's selection as `cellSelection()` reads it: the grid's
 *  key, the two ends as the drag made them, and whether it is a block
 *  (rectangular) rather than linewise (ADR 0017, decision 4). */
export interface CellSelection {
  node: string;
  anchor: CellEnd;
  focus: CellEnd;
  block: boolean;
}

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
  /** How outline glyphs are antialiased: `'auto'` (the default) is LCD
   *  subpixel coverage where the GPU blends per channel and grayscale
   *  otherwise. `KUI_TEXT_AA=gray|subpixel` in the environment still
   *  overrides, for an A/B by hand. */
  textAa?: 'auto' | 'gray' | 'subpixel';
  /** Pins part of `env.system` for the life of the window, over whatever
   *  the OS says: `{ motion: 'reduced' }` opens the window as a user who
   *  asked for less motion sees it, on a machine whose owner did not. The
   *  same partial `Ctx.setEnv` takes, and the same readings — but a field
   *  left out, `'unknown'` or null here is *not pinned* and keeps
   *  following the OS, whose changes to it still arrive as the `system`
   *  message, carrying the pinned fields with them. A window has no
   *  `setEnv` because its runner writes the real reading before every
   *  frame; this is merged inside that write, which is what makes it
   *  hold. It is an option rather than an environment variable so that a
   *  shipped app's motion is its own code's decision. */
  system?: EnvInput['system'];
  /** macOS: whether holding a letter key opens the accent picker (the
   *  platform's press-and-hold, on unless the user turned it off) or
   *  repeats the key, as every other platform does. With it on, a held `e`
   *  offers `é è ê` and a held `j` does nothing at all — so an app whose
   *  keys are commands (a modal editor, where `j` held is a motion) says
   *  `false`; one that is typed into leaves it, the picker being how its
   *  users write accents. This process alone, never written to the user's
   *  preferences; a no-op on every other platform. */
  pressAndHold?: boolean;
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
/** One binding's cell in the verb table: its spelling of the verb, the
 *  same thing in another form (a prop, a reading, a callback, a
 *  constructor option), or the reason there is none. */
export type DoorCell = { is: string } | { as: string } | { no: string };

/** One row of the verb table (`schema::DOORS`, backlog B1a): a verb by its
 *  Rust spelling and what each binding has for it. The suite pins `Ctx`
 *  and `KuiWindow` to the Node column both ways. */
export interface Door {
  rust: string;
  c: DoorCell;
  node: DoorCell;
  lua: DoorCell;
  doc: string;
}

export interface Protocol {
  version: number;
  op: Record<string, number>;
  prop: Record<string, number>;
  doors: Door[];
}

/** The `KuiWindow` constructor's options, picked out of `runWindowed`'s
 *  (which carry the loop's own beside them): for an app opening its window
 *  itself with the same set. */
export declare function windowOptions(opts?: WindowOptions & Record<string, unknown>): WindowOptions;

/** A reusable frame encoder for the binary IR path (drivers make their own). */
export declare function createEncoder(p: Protocol): {
  encode(tree: KuiNode, tokens?: Map<string, { kind: 'color' | 'length'; index: number }>): {
    stream: Float64Array;
    strings: Uint8Array;
    unknown: [string, string][];
    unknownTokens: [string, string][];
  };
};

// -- tokens (docs/adr/0027-tokens-beside-the-theme.md) ---------------------

/** One step of a derived colour token's recipe (ADR 0028), a tuple: the
 *  verb, then its operands — a colour token or role by name for `mix` and
 *  `readable`, then the number. `lift` / `darken` move toward white /
 *  black by `t`; `raise` toward the front of whichever base is in effect;
 *  `alpha` sets the alpha; `mix` moves toward the named colour by `t`;
 *  `readable` moves toward black or white — whichever reads on the named
 *  colour — until it clears the ratio on it. */
export type ColorOp =
  | readonly ['lift', number]
  | readonly ['darken', number]
  | readonly ['raise', number]
  | readonly ['alpha', number]
  | readonly ['mix', string, number]
  | readonly ['readable', string, number];

/** A colour token computed from another (ADR 0028): `from` names a colour
 *  token declared *before* this one, or a theme role (`'accent'`), and
 *  `ops` is the chain folded over it in order — a list of tuples, or one
 *  bare tuple, or nothing for an alias. Resolved by the core on read, so a
 *  themed source's recipe runs on the half in effect. A source that is
 *  not there is dropped with `unknown-token` at the declaration. */
export interface DerivedColor {
  from: string;
  ops?: ColorOp | readonly ColorOp[];
}

/** A colour token's value: one colour for both bases, a light and a dark
 *  half the core picks by the appearance in effect — each the way a colour
 *  prop spells one, `0xRRGGBBAA` or `'#hex'` — or a recipe over an earlier
 *  token (`DerivedColor`). */
export type ColorTokenValue =
  | number
  | string
  | { light: number | string; dark: number | string }
  | DerivedColor;

/** What `setTokens` / `defineTokens` take: the app's named colours and
 *  lengths, apart by kind — a colour and a length are both a number, so
 *  the kind cannot be read off the value. Declaration order is the wire
 *  index. A name a theme or metrics role owns (`surface`, `radius`) is
 *  dropped by the core with a `reserved-token` warning: `$surface` always
 *  means the role's. */
export interface TokenDeclaration<
  C extends Record<string, ColorTokenValue> = Record<string, ColorTokenValue>,
  L extends Record<string, number> = Record<string, number>,
> {
  colors?: C;
  lengths?: L;
}

/** The references a declaration's names become: `{ peach: '$peach',
 *  sideW: '$sideW' }`, each a literal type branded by kind. */
export type TokenRefs<D extends TokenDeclaration> = {
  readonly [K in keyof D['colors'] & string]: `$${K}` & { readonly __kuiToken?: 'color' };
} & {
  readonly [K in keyof D['lengths'] & string]: `$${K}` & { readonly __kuiToken?: 'length' };
};

/**
 * Types a token declaration's names as the references a prop takes
 * (ADR 0027, decision 5): given `{ colors: { peach: '#ffcc99' }, lengths:
 * { sideW: 132 } }` it returns `{ peach: '$peach', sideW: '$sideW' }`, so
 * `bg={T.peach}` and `width={T.sideW}` type-check, `width={T.peach}` does
 * not, and `T.peech` does not exist. Zero runtime: the object *is* the
 * references. Hand the same declaration to `setTokens`.
 */
export declare function defineTokens<const D extends TokenDeclaration>(decl: D): TokenRefs<D>;

/** The theme's and metrics' roles as references — `roles.surface` is
 *  `'$surface'`, `roles.radius` is `'$radius'` — the same spelling an app
 *  token takes, resolved to the role's value whatever the app declared. */
export declare const roles: {
  readonly [K in Exclude<keyof Theme, 'appearance' | 'disabledOpacity'> & string]: `$${K}` & { readonly __kuiToken?: 'color' };
} & {
  readonly [K in keyof Metrics & string]: `$${K}` & { readonly __kuiToken?: 'length' };
};

/** `tokens()`: the tokens a view here sees this frame, resolved — colours
 *  as `0xRRGGBBAA` for the appearance in effect, lengths in px. Roles are
 *  not listed; read them off `theme()` and `metrics()`. */
export interface ResolvedTokens {
  colors: Record<string, number>;
  lengths: Record<string, number>;
}

// -- generated from the addon's `#[napi]` surface; edit crates/kui-node/src/lib.rs, then `npm run gen` --

/**
 * A headless kui core: build frames from JSX trees, feed input, poll events.
 * Everything a window does except open one, so an app's behaviour is
 * testable without a display.
 */
export declare class Ctx {

}

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
   * Loads a C extension: a shared library exporting the seven
   * `kui_ext_*` entry points `crates/kui-ffi/include/kui.h` describes
   * (ADR 0014). `namespace` is the word that fronts every slot name it
   * fills — `<slot name="todos/panel"/>` for `addExtension('todos', …)`
   * — and an empty one takes the plugin's own `kui_ext_name`. Throws
   * with the reason if the library will not load, declares no
   * `kui_ext_abi` or one this build does not implement, has no
   * `kui_ext_view`, or wants a namespace another extension has.
   *
   * Load before the first frame: origins are positions in the list, so
   * one added later renumbers the ones after it. The context owns it and
   * unloads it when the context goes.
   *
   * **A plugin runs in this process**, on this thread, on this app's
   * frame — loading one is trusting it as much as linking it would be.
   * Only C shared libraries: there is no script-loads-script path here,
   * and a Lua extension is loaded by a Rust host or not at all.
   */
  addExtension(namespace: string, path: string): void
  /**
   * The namespaces of the loaded extensions, in origin order: index `i`
   * is origin `i + 1`, and origin 0 is the app's own nodes. What turns
   * the `origin` on an event into the name this app gave the plugin.
   */
  extensionNamespaces(): Array<string>
  /**
   * The frame clock for `transition` props: monotonic seconds, any
   * origin. Set before each frame; never setting it makes transitions
   * snap. A bare `Ctx` is the only place to call it: `createApp`'s loop
   * owns the clock, stamps it before every frame it draws, and replaces
   * this method on its surface with one that throws — `app.advance(ms)`
   * is what moves time there.
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
   * Files dragged in from the OS are over the window at (`x`, `y`) —
   * entering and moving alike (ADR 0031): the `onDrop` zone under the
   * point hears `{kind:"drop", phase:"enter"|"move", paths, x, y,
   * tag}`, a zone it left hears `leave`, a repeat at the same point is
   * nothing. `dropTarget()` afterwards is what a driver answers the OS
   * with.
   */
  dragFiles(paths: Array<string>, x: number, y: number): void
  /**
   * The dragged files released at (`x`, `y`): the zone there hears
   * `{kind:"drop", phase:"drop", paths, x, y, tag}` and no `leave`
   * after it; with no zone there, nothing but the lit zone's `leave`.
   */
  dropFiles(paths: Array<string>, x: number, y: number): void
  /**
   * The dragged files left the window, or the OS ended the drag
   * elsewhere: the lit zone hears its `leave`.
   */
  dragCancel(): void
  /**
   * A button press or release. `clicks`: 1 single, 2 double (word
   * select), 3 triple (line select) — the grain everywhere text can be
   * selected: an editor, a `selectable` scope, and a `cells` grid,
   * where it counts in cells. `button` defaults to "primary", and
   * only that one presses, drags, places the caret and clicks;
   * "secondary" asks the node under the pointer for a context menu and
   * moves nothing else, and nothing routes "middle" yet.
   */
  mouse(down: boolean, clicks?: number, button?: MouseButtonName): void
  scroll(dx: number, dy: number): void
  /** Committed text input (typing, paste); routed to the focused editor. */
  text(text: string): void
  /**
   * Text an IME committed at the end of a composition: a focused
   * `<edit>` takes it, otherwise the focused `onKey` sink hears it as
   * `{kind:"text", text, tag}` — the one committed text a `key` event
   * never carries. `type`/`press` stay the way to type.
   */
  commit(text: string): void
  /**
   * An in-progress IME composition: `text` is the uncommitted string
   * (empty ends the composition without a commit), `cursor` the byte
   * range inside it the IME's caret covers, or null. A focused `<edit>`
   * shows it inline; otherwise the focused `onKey` sink hears
   * `{kind:"preedit", text, cursor, tag}`.
   */
  preedit(text: string, cursor?: [number, number] | null): void
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
   * way; omit it and it is the position's US key: the lower-case letter
   * for a letter, `code` for everything else — the pair a window
   * reports for ⇧Z is `code: "Z", physical: "z"`, and so is this door's
   * (backlog F65). Passing both is how a driver reports a non-US
   * layout, and it is what makes the reported `code` portable: a
   * layout producing something outside ASCII would leave a Latin
   * keymap matching nothing, so the position's US letter stands in.
   */
  keyDown(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void
  /**
   * The release of a key, spelled the way `keyDown` spells it (`physical`
   * included): a sink that declared `keyUp` hears `{kind:"key",
   * phase:"up", ...}` with `text` null; one that did not hears nothing,
   * since presses only is the keymap default. A release whose press the
   * sink never got resolves nothing, and moving focus while a key is
   * held delivers the `up` first.
   */
  keyUp(code: string, mods?: KeySinkMods, physical?: string): void
  /**
   * A whole key going down, the way a window sends it: the raw press
   * to an `onKey` sink, and then what the core is asked to do with that
   * key — Escape dismisses a modal, Tab walks the focus ring, an arrow
   * nudges a focused slider, Space presses a focused control, a
   * printable character reaches the focused editor.
   *
   * **This is the one to reach for.** `keyDown` and `key` are its two
   * halves, kept for a test that means to drive one channel and not the
   * other; a test that means "the user pressed this key" wants both,
   * and `keyDown("escape")` leaving a modal open is what having to
   * choose used to cost (backlog F6).
   *
   * Spelled exactly as `keyDown`: a single character (layout-resolved,
   * e.g. "W" or "$") or a name ("left", "enter", "escape", "f5", ...),
   * with mods `{shift, ctrl, alt, super}`, `repeat` for an OS
   * auto-repeat, and `physical` for the US-QWERTY key at that position.
   * `release()` is the other end of the same key.
   *
   * Spell it the way the OS does, because nothing here re-spells it: a
   * shifted letter is the upper-case letter with `shift` set —
   * `press("Z", { shift: true, super: true })` is ⇧⌘Z — and
   * `press("z", { shift: true })` is a chord no keyboard produces, which
   * a handler switching on `"z"` hears headless and never from a user
   * (backlog F60: an app's redo was green for six releases over it).
   * Fold a one-character `code` to lower case under a chord if a keymap
   * binds letters.
   */
  press(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void
  /**
   * The same key coming up, spelled the way `press` spells it. One
   * channel, because only one has a second half: the editing keys act
   * on the way down, so this is `keyUp` under the name that pairs with
   * `press`. A sink that declared `keyUp` hears it; one that did not
   * hears nothing.
   */
  release(code: string, mods?: KeySinkMods, physical?: string): void
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
   * The same driver reports that a `stop` it drained landed on a
   * playback still running, `at` seconds in: a one-shot `<audio>` that
   * went away without `finish` is named in a `truncated-playback`
   * warning (`warnings()`); any other stop reports nothing.
   */
  audioTruncated(playback: number, at: number): void
  /**
   * The same driver reports that its device refused a `play` it
   * drained: a tagged playback becomes a `sound` event with phase
   * `refused` in `pollEvents`, and the node that asked is named in a
   * `playback-refused` warning either way.
   */
  audioRefused(playback: number): void
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
   * Whether the last frame asked for the window above every other
   * app's (a root `<box alwaysOnTop>`, backlog C30); false when it did
   * not. `runWindowed` applies it to the real window on change and
   * reports what the platform did as `env().window.alwaysOnTop`; a
   * bare `Ctx` hands the ask back so a test can assert on it.
   */
  alwaysOnTop(): boolean
  constructor()
  /**
   * `frame` from an already-encoded binary instruction stream, for
   * callers that own their encoder (`createEncoder(protocol())`) — the
   * fastest path, and what the JS drivers use. Buffers are read
   * zero-copy.
   */
  frameBinary(width: number, height: number, scale: number, stream: Float64Array, strings: Uint8Array): void
  /**
   * Loads a C extension: a shared library exporting the seven
   * `kui_ext_*` entry points `crates/kui-ffi/include/kui.h` describes
   * (ADR 0014). `namespace` is the word that fronts every slot name it
   * fills — `<slot name="todos/panel"/>` for `addExtension('todos', …)`
   * — and an empty one takes the plugin's own `kui_ext_name`. Throws
   * with the reason if the library will not load, declares no
   * `kui_ext_abi` or one this build does not implement, has no
   * `kui_ext_view`, or wants a namespace another extension has.
   *
   * Load before the first frame: origins are positions in the list, so
   * one added later renumbers the ones after it. The context owns it and
   * unloads it when the context goes.
   *
   * **A plugin runs in this process**, on this thread, on this app's
   * frame — loading one is trusting it as much as linking it would be.
   * Only C shared libraries: there is no script-loads-script path here,
   * and a Lua extension is loaded by a Rust host or not at all.
   */
  addExtension(namespace: string, path: string): void
  /**
   * The namespaces of the loaded extensions, in origin order: index `i`
   * is origin `i + 1`, and origin 0 is the app's own nodes. What turns
   * the `origin` on an event into the name this app gave the plugin.
   */
  extensionNamespaces(): Array<string>
  /**
   * The frame clock for `transition` props: monotonic seconds, any
   * origin. Set before each frame; never setting it makes transitions
   * snap. A bare `Ctx` is the only place to call it: `createApp`'s loop
   * owns the clock, stamps it before every frame it draws, and replaces
   * this method on its surface with one that throws — `app.advance(ms)`
   * is what moves time there.
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
   * Files dragged in from the OS are over the window at (`x`, `y`) —
   * entering and moving alike (ADR 0031): the `onDrop` zone under the
   * point hears `{kind:"drop", phase:"enter"|"move", paths, x, y,
   * tag}`, a zone it left hears `leave`, a repeat at the same point is
   * nothing. `dropTarget()` afterwards is what a driver answers the OS
   * with.
   */
  dragFiles(paths: Array<string>, x: number, y: number): void
  /**
   * The dragged files released at (`x`, `y`): the zone there hears
   * `{kind:"drop", phase:"drop", paths, x, y, tag}` and no `leave`
   * after it; with no zone there, nothing but the lit zone's `leave`.
   */
  dropFiles(paths: Array<string>, x: number, y: number): void
  /**
   * The dragged files left the window, or the OS ended the drag
   * elsewhere: the lit zone hears its `leave`.
   */
  dragCancel(): void
  /**
   * A button press or release. `clicks`: 1 single, 2 double (word
   * select), 3 triple (line select) — the grain everywhere text can be
   * selected: an editor, a `selectable` scope, and a `cells` grid,
   * where it counts in cells. `button` defaults to "primary", and
   * only that one presses, drags, places the caret and clicks;
   * "secondary" asks the node under the pointer for a context menu and
   * moves nothing else, and nothing routes "middle" yet.
   */
  mouse(down: boolean, clicks?: number, button?: MouseButtonName): void
  scroll(dx: number, dy: number): void
  /** Committed text input (typing, paste); routed to the focused editor. */
  text(text: string): void
  /**
   * Text an IME committed at the end of a composition: a focused
   * `<edit>` takes it, otherwise the focused `onKey` sink hears it as
   * `{kind:"text", text, tag}` — the one committed text a `key` event
   * never carries. `type`/`press` stay the way to type.
   */
  commit(text: string): void
  /**
   * An in-progress IME composition: `text` is the uncommitted string
   * (empty ends the composition without a commit), `cursor` the byte
   * range inside it the IME's caret covers, or null. A focused `<edit>`
   * shows it inline; otherwise the focused `onKey` sink hears
   * `{kind:"preedit", text, cursor, tag}`.
   */
  preedit(text: string, cursor?: [number, number] | null): void
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
   * way; omit it and it is the position's US key: the lower-case letter
   * for a letter, `code` for everything else — the pair a window
   * reports for ⇧Z is `code: "Z", physical: "z"`, and so is this door's
   * (backlog F65). Passing both is how a driver reports a non-US
   * layout, and it is what makes the reported `code` portable: a
   * layout producing something outside ASCII would leave a Latin
   * keymap matching nothing, so the position's US letter stands in.
   */
  keyDown(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void
  /**
   * The release of a key, spelled the way `keyDown` spells it (`physical`
   * included): a sink that declared `keyUp` hears `{kind:"key",
   * phase:"up", ...}` with `text` null; one that did not hears nothing,
   * since presses only is the keymap default. A release whose press the
   * sink never got resolves nothing, and moving focus while a key is
   * held delivers the `up` first.
   */
  keyUp(code: string, mods?: KeySinkMods, physical?: string): void
  /**
   * A whole key going down, the way a window sends it: the raw press
   * to an `onKey` sink, and then what the core is asked to do with that
   * key — Escape dismisses a modal, Tab walks the focus ring, an arrow
   * nudges a focused slider, Space presses a focused control, a
   * printable character reaches the focused editor.
   *
   * **This is the one to reach for.** `keyDown` and `key` are its two
   * halves, kept for a test that means to drive one channel and not the
   * other; a test that means "the user pressed this key" wants both,
   * and `keyDown("escape")` leaving a modal open is what having to
   * choose used to cost (backlog F6).
   *
   * Spelled exactly as `keyDown`: a single character (layout-resolved,
   * e.g. "W" or "$") or a name ("left", "enter", "escape", "f5", ...),
   * with mods `{shift, ctrl, alt, super}`, `repeat` for an OS
   * auto-repeat, and `physical` for the US-QWERTY key at that position.
   * `release()` is the other end of the same key.
   *
   * Spell it the way the OS does, because nothing here re-spells it: a
   * shifted letter is the upper-case letter with `shift` set —
   * `press("Z", { shift: true, super: true })` is ⇧⌘Z — and
   * `press("z", { shift: true })` is a chord no keyboard produces, which
   * a handler switching on `"z"` hears headless and never from a user
   * (backlog F60: an app's redo was green for six releases over it).
   * Fold a one-character `code` to lower case under a chord if a keymap
   * binds letters.
   */
  press(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void
  /**
   * The same key coming up, spelled the way `press` spells it. One
   * channel, because only one has a second half: the editing keys act
   * on the way down, so this is `keyUp` under the name that pairs with
   * `press`. A sink that declared `keyUp` hears it; one that did not
   * hears nothing.
   */
  release(code: string, mods?: KeySinkMods, physical?: string): void
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
   * The same driver reports that a `stop` it drained landed on a
   * playback still running, `at` seconds in: a one-shot `<audio>` that
   * went away without `finish` is named in a `truncated-playback`
   * warning (`warnings()`); any other stop reports nothing.
   */
  audioTruncated(playback: number, at: number): void
  /**
   * The same driver reports that its device refused a `play` it
   * drained: a tagged playback becomes a `sound` event with phase
   * `refused` in `pollEvents`, and the node that asked is named in a
   * `playback-refused` warning either way.
   */
  audioRefused(playback: number): void
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
   * Whether the last frame asked for the window above every other
   * app's (a root `<box alwaysOnTop>`, backlog C30); false when it did
   * not. `runWindowed` applies it to the real window on change and
   * reports what the platform did as `env().window.alwaysOnTop`; a
   * bare `Ctx` hands the ask back so a test can assert on it.
   */
  alwaysOnTop(): boolean
  /**
   * A headless context is one window, the main: this answers whether
   * `window` names it (`"main"`, `0`, or left out) and addresses
   * nothing else — the same door `KuiWindow` has, so a loop or a test
   * can call it on either surface.
   */
  useWindow(window?: string | number): boolean
  /**
   * A headless context is one window, the main: this answers whether
   * `window` names it (`"main"`, `0`, or left out) and addresses
   * nothing else — the same door `KuiWindow` has, so a loop or a test
   * can call it on either surface.
   */
  useWindow(window?: string | number): boolean
  /**
   * Registers a w×h RGBA image (pixels copied); returns its id for
   * `<image src={id}>`. Stable until `removeImage`.
   */
  addImage(width: number, height: number, rgba: Buffer): string
  removeImage(id: string): void
  /**
   * Replaces an image's pixels in place (copied): the id is
   * unchanged, so every `<image src={id}>` shows the new pixels
   * next frame with no view change; `width`/`height` may differ
   * from the registration. From the first update on the image
   * is drawn from a texture of its own — a video frame, a
   * camera, a plot the app rasterised itself
   * (`docs/adr/0025-the-image-is-the-canvas.md`). A dead id warns
   * `foreign-resource` and changes nothing.
   */
  updateImage(id: string, width: number, height: number, rgba: Buffer): void
  /**
   * Registers a WGSL fragment function; returns its id for
   * `<fragment src={id}>`. Throws when the source does not
   * compile, with the compiler's message in the app's own line
   * numbers. Idempotent by source, so the same text gets the
   * same id without being validated twice.
   */
  addFragment(wgsl: string): string
  removeFragment(id: string): void
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
  /**
   * What the last frame left owed, by kind — `animating()`
   * taken apart: `{transition, cycle, depart, requested,
   * autoscroll}`. To the window they are one, and it redraws
   * for any of them; to a test they differ, since a keyframe
   * `repeat` cycle never ends and `settled()` never resolves
   * under one. `quiet()` on the loop waits on everything but
   * `cycle` (backlog F64).
   */
  owed(): Owed
  /**
   * Byte budget for the shaped-text cache: every text a frame
   * draws is shaped once and kept, and past this many
   * (estimated) bytes the least recently drawn entries go at
   * the start of the next frame — never what the last frame
   * drew. Default 64 MB; a terminal streaming new lines lowers
   * it, a viewer that wants every page it showed kept warm
   * raises it.
   */
  setTextCacheBudget(bytes: number): void
  /**
   * What the shaped-text cache holds, in the estimated bytes
   * the budget is charged against.
   */
  textCacheBytes(): number
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Raw quads for the finished frame, `quadStride()` bytes each,
   * laid out as kui-ffi's KuiQuad (see include/kui.h) and decoded
   * by `decodeQuads`. Copied into the Buffer. A window answers
   * with what its last pump drew, so a smoke test can read the
   * frame the shipping driver painted and not only a headless
   * one's (backlog F19). Drive that window with `access(key,
   * action)` — `click`, `type` and `key` are refused there,
   * because the OS is what drives a real window.
   */
  quads(): Buffer
  /**
   * The clips this frame's quads name through their `clip`
   * index, `clipStride()` bytes each, laid out as kui-ffi's
   * KuiClip (see include/kui.h) and decoded by `decodeClips`.
   * Entry zero clips nothing, so a quad always has one; the
   * list is empty only on a frame that drew nothing.
   */
  clips(): Buffer
  /**
   * This frame's fragment draws, in the order their quads index
   * them by `uv[0]`: twenty-four doubles each — the handle as
   * two 32-bit halves, the sixteen parameters, then where the
   * draw's `image` is (0 none, 1 the atlas, 2 a texture of its
   * own), the `textureDraws` index when it is 2, and the texel
   * rect `x, y, w, h` (backlog V1). The parameters ride a side
   * list rather than the quad, so `quads()` alone cannot show
   * them and a corpus adapter needs this to compare them
   * (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
   * Empty on a frame that draws no fragment.
   */
  fragmentDraws(): Array<number>
  /**
   * This frame's texture draws, in the order their quads index
   * them by `uv[0]`: nine doubles each — the image handle as two
   * 32-bit halves, the pixels' revision, width and height, and
   * the texel rect `x, y, w, h` in the image's own texels (the
   * whole image, or the crop a `fit="cover"` made). The side
   * list a `quads()` texture quad points at
   * (`docs/adr/0025-the-image-is-the-canvas.md`, decision 3).
   * Empty on a frame that draws no texture-backed image.
   */
  textureDraws(): Array<number>
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
   * The palette this window paints with: one `0xRRGGBBAA` number
   * per role, derived from `env().system` unless this app said
   * otherwise (`setAccent` / `setTheme`). A view reads it and
   * paints with it — `<box bg={theme.surface}>` — and the stock
   * widgets already do, so a JSX app that only uses `<button>`,
   * the context menu and `<text>` follows the OS's light and
   * dark without reading this at all.
   */
  theme(): Theme
  /**
   * Keep following the OS's light/dark, but paint this accent
   * instead of the OS's — an app with a brand colour. A
   * `0xRRGGBBAA` number or a `"#hex"` string, as any colour
   * prop takes; `null` goes back to the OS's own.
   */
  setAccent(accent: number | string | null): void
  /**
   * Pin the palette: an object of role overrides on top of the
   * base named by `appearance` (`"light"`, `"dark"`, or absent
   * for the OS's), each value a colour the way a prop takes
   * one. Follows nothing afterwards — `setAccent(null)` is how
   * an app goes back to following the OS.
   *
   * `{ appearance: "dark", accent: "#d2691e" }` is a dark app
   * with one colour changed; every role the object does not
   * name keeps the base's, and the ones derived from the accent
   * (its hover, its pressed shade, the label on it, the ring,
   * the selection tint) are recomputed unless named too.
   */
  setTheme(theme: ThemeOverrides): void
  /**
   * The sizes the stock widgets are built from — the palette's
   * other axis: one number per metric, logical px before the
   * scale factor. Read it so a control of your own agrees with
   * `<button>` on a radius and a padding: `<box radius={metrics.radius}>`.
   */
  metrics(): Metrics
  /**
   * Makes these the frame's metrics: overrides on top of the
   * set in effect, or on top of `base: "comfortable"` (the
   * stock set) / `"compact"` (a dense tool's), then `scale`
   * multiplying every length — a density slider. `null`
   * restores the stock set. Density is the app's to choose;
   * nothing in the OS is followed.
   */
  setMetrics(metrics: MetricsOverrides | null): void
  /**
   * Declare the app's named colours and lengths
   * (`docs/adr/0027-tokens-beside-the-theme.md`): `{ colors:
   * { peach: '#ffcc99', ink: { light, dark } }, lengths: {
   * sideW: 132 } }`. Replaces the table whole, so an app whose
   * lengths change with a viewport tier declares again on
   * `resize`. A name a theme or metrics role owns is dropped
   * with a `reserved-token` warning. A colour may be a
   * recipe over an earlier one (ADR 0028): `{ from: 'peach',
   * ops: [['lift', 0.3]] }`, dropped with `unknown-token` when
   * its source is not there. Reference one in a prop
   * as `'$peach'` — `defineTokens` types the names. The raw
   * addon door; `index.js` wraps it to keep the encoder's map
   * in step, so call `setTokens` and not this.
   */
  setTokensRaw(tokens: { colors: [string, unknown][], lengths: [string, number][] }): void
  /**
   * The tokens a view here sees this frame, resolved: colours as
   * `0xRRGGBBAA` for the appearance in effect, lengths in px,
   * under `colors` and `lengths`. Roles are not listed — read
   * them off `theme()` and `metrics()`.
   */
  tokens(): ResolvedTokens
  /**
   * `frame` / `setView` report the `$name` references the
   * encoder could not resolve — nothing declared, or the other
   * kind — through here, as `unknown-token`, once per name.
   */
  warnUnknownTokens(names: [string, string][]): void
  /**
   * `measureText`'s door (index.js adds `measureText` itself):
   * one `<text>` element as `encoder.encodeText` writes it, and
   * the answer is what that text lays out to — `{width, height,
   * lines}` in logical px at the context's scale, capped to
   * `maxWidth` when given. The metrics do not scale linearly:
   * `measured × zoom` is not `measure(size × zoom)`, because
   * shaping rounds per size, so anything that zooms measures at
   * the size it draws.
   */
  measureTextBinary(stream: Float64Array, strings: Uint8Array, maxWidth?: number | undefined | null): TextMetrics
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
   * Every warning the core has raised so far, drained or not,
   * oldest first — the log `warnings()` leaves behind, for a
   * reader that is not the driver (a devtools stream).
   */
  warningsRaised(): Warning[]
  /**
   * Turns the per-frame node snapshot behind `nodes()` on or off
   * (off unless a devtool asked: the copy is O(nodes) a frame).
   */
  setInspect(on: boolean): void
  /**
   * Turns the core's devtools panel on or off
   * (`docs/adr/0024`): the event stream, the runtime's facts and
   * the tree, drawn by the core beside the app's own tree in the
   * main window — or where `setDevtoolsDock` says — with its
   * controls and its `Ctrl+Shift+<letter>` chords handled inside
   * the core, so nothing of it reaches `update`. `KUI_DEVTOOLS=1`
   * in the environment is the same call made by nobody, for a
   * `KuiWindow`; a headless `Ctx` never reads it.
   */
  setDevtools(on: boolean): void
  /** Whether the devtools panel is on. */
  devtools(): boolean
  /**
   * Where the devtools panel sits: `"left"`, `"right"`,
   * `"bottom"`, `"window"` (one of its own, named
   * `kui-devtools`) or `"off"` (hidden, the chords still live);
   * `"side"` is the right. Throws on any other word.
   */
  setDevtoolsDock(dock: DevtoolsDock): void
  /** Where the devtools panel sits (see `setDevtoolsDock`). */
  devtoolsDock(): DevtoolsDock
  /**
   * Seeds the panel's theme override, what its `T` and `A`
   * chords cycle from: `base` is `"light"`, `"dark"` or `null`
   * for the app's own; `accent` an `#rrggbb` string or `null`.
   */
  setDevtoolsTheme(base: 'light' | 'dark' | null, accent: string | null): void
  /**
   * Respells the chord that moves the keyboard into the panel
   * and back out — and brings a hidden panel back — from its
   * default `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod`
   * is Command on macOS, Control elsewhere), `"⌥⌘I"`, any
   * spelling a menu item's `accel` takes. The panel's other
   * chords stay `Ctrl+Shift+<letter>`. With another chord set,
   * `Ctrl+Shift+I` reaches the app like any other press. Throws
   * on a spelling kui cannot name.
   */
  setDevtoolsKey(key: string): void
  /**
   * The chord `setDevtoolsKey` set, or the default, in its
   * portable spelling: `"ctrl+shift+i"`, `"f12"`,
   * `"super+alt+d"`.
   */
  devtoolsKey(): string
  /**
   * The declared devtools tab on show, by name, or `null` for
   * one of the panel's own, the panel off or popped out (ADR
   * 0032). What `frame` / `setView` read once before encoding,
   * so a `<devtoolsTab>`'s function child is called only for
   * that tab.
   */
  devtoolsShownTab(): string | null
  /**
   * The node the panel's tree tab has selected, as a hex key,
   * or `null` (ADR 0032, decision 4) — what an inspector in a
   * declared tab reads to say which node it is about.
   */
  devtoolsSelected(): string | null
  /** The tree row under the pointer, as a hex key, or `null`. */
  devtoolsHovered(): string | null
  /** The node the picker is over while picking, or `null`. */
  devtoolsPicked(): string | null
  /**
   * Raises the panel's picker from outside it — an inspector in
   * a `<devtoolsTab>` asking "which node?" — or puts it away
   * (ADR 0032, decision 4). Picking happens over the app in
   * the main window: `devtoolsPicked()` is the node under the
   * pointer while it is up, and the press lands it in
   * `devtoolsSelected()`. Raised while a declared tab is on
   * show, the pick leaves that tab up; raised otherwise it is
   * the `Ctrl+Shift+P` pick and shows the tree tab. A hidden
   * panel comes back docked.
   */
  setDevtoolsPick(on: boolean): void
  /** Whether the panel's picker is up. */
  devtoolsPicking(): boolean
  /**
   * Shows the panel's tab named `name` from the app's side —
   * what the strip's click and `Ctrl+Shift+N` do, for a command
   * that jumps to the app's own tab (ADR 0032). `name` is one
   * of the panel's own (`facts`, `events`, `tree`) or a
   * `<devtoolsTab>`'s. A declared name the panel does not list
   * yet is kept and shows once a frame declares it; the return
   * says whether the panel lists it now. A hidden panel comes
   * back docked; `setDevtools(true)` is still the app's to
   * call. Call it once, not every frame: it would pin the strip
   * against the user's own clicks.
   */
  setDevtoolsTab(name: DevtoolsTab | (string & {})): boolean
  /**
   * The tab the panel is on, by name: one of its own or a
   * `<devtoolsTab>`'s — the selection itself, panel on or off,
   * unlike `devtoolsShownTab()`, which is the encoder's reading
   * of a declared tab on show.
   */
  devtoolsCurrentTab(): DevtoolsTab | (string & {})
  /**
   * Selects a node in the panel's tree tab from outside it and
   * reveals it there, as the picker does; `null` clears. `key`
   * is a hex key an event carried or `keyOf` answered.
   */
  setDevtoolsSelected(key?: string | undefined | null): void
  /**
   * The key legend the panel's facts tab shows: `[keys, what]`
   * pairs.
   */
  setDevtoolsLegend(legend: [string, string][]): void
  /**
   * The last finished frame's nodes in tree order, each with what
   * it is, the label it was opened under, where layout put it
   * (in the app's viewport, like `layoutOf`; backlog AR36), and
   * the declarations that explain the rest — what a tree view
   * and a node inspector are built from. Empty until
   * `setInspect(true)` and a frame after it.
   */
  nodes(): NodeInfo[]
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
   * A request from assistive technology on a node: an `AccessAction`
   * name the node advertises, with `value` the new text for
   * `setValue`. `key` is either spelling of the node — the hex key
   * an event carried (16 digits), or the label its `key` prop
   * declared, resolved through the last frame (see `focus`).
   * Resolved like its pointer/keyboard equivalent, so the resulting
   * events come out of `pollEvents`. A real screen reader's
   * requests arrive through a window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame. `key` is either spelling,
   * as for `focus`: the label a `key` prop declared, or the hex
   * key an event carried. For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  /**
   * Press state as of the last frame; `key` is either spelling,
   * as for `isHovered`.
   */
  isPressed(key: string): boolean
  /**
   * Whether files dragged in from the OS are over `key` (ADR
   * 0031) — for drop-dependent layout; the colour swap is the
   * `dropBg` prop. `key` is either spelling, as for `isHovered`.
   */
  isDropTarget(key: string): boolean
  /**
   * The `onDrop` zone the dragged files are over, as the hex key
   * an event carries, or null — what a driver answers the OS
   * with after each `dragFiles`, and what a test reads to say a
   * zone was found.
   */
  dropTarget(): string | null
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
   * `onKey` sink, a button Tab landed on (see `focused`). `key` is
   * a hex key or a declared label, as for `focus`.
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab walk every control in tree order, Enter and Space
   * press the focused one, and the arrows nudge a focused
   * slider — `press("tab")`, `press(" ")`, `press("right")`,
   * which is what a keyboard sends. (`key("tab")` is the half
   * of that press the core acts on, for a test that means to
   * drive one channel; `keyDown` is the other half, the one an
   * `onKey` sink hears.)
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * The caret's blink phase — `true` draws it (backlog C35). A
   * custom editor reads it in `view` and skips its caret node
   * on the off phase, keeping the `caret` row on its `line`
   * either way; the window's clock sets it while a focused
   * `<edit>` or such a line has a caret, and parks it hidden
   * while the window has no keyboard. Always `true` headless.
   */
  caretVisible(): boolean
  /**
   * Whether there is a caret to blink: a focused `<edit>`'s, or
   * the `caret` a `line` under the focused sink declares — unless
   * the line declares it `caretSolid` (backlog F68), which
   * anchors and reads but arms no clock. What the window's
   * clock is armed on; headless, what a test reads to see that
   * an idle view asks for no frame.
   */
  hasCaret(): boolean
  /**
   * The driver's half of the blink: sets the phase. A window
   * runs its own clock; headless, a test drives it to see the
   * off phase drawn.
   */
  setCaretVisible(visible: boolean): void
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   *
   * `key` is either spelling of the node: the 16-digit hex key an
   * event carried, or the label its `key` prop declared —
   * `focus('note')` — resolved through the last frame, so a node
   * the user has never touched can be named. Labels are unique
   * among siblings, not across the tree: when two nodes declare
   * the same one, the first in tree order wins and an
   * `ambiguous-key` warning says so. A label no node declared
   * throws.
   */
  focus(key: string): void
  /**
   * The hex key of the node a label names — the label a `key`
   * prop declared, resolved through the frame being built so
   * far and then the last finished one — or null when no node
   * declared it. The door for holding a key across frames;
   * every call that takes a key takes the label too, so this
   * is for caching one, or for checking that a name reached
   * the view. Two nodes on one label under different parents
   * resolve to the first in tree order and raise
   * `ambiguous-key` — among the host's own first, and a
   * plugin filling a slot is answered from its own nodes only.
   */
  keyOf(label: string): string | null
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
   * Enters a focus region — a box declared `focusRegion`, named by
   * the label its `key` prop declares or by the hex key an event
   * carried — or the main ring for `null`
   * (`docs/adr/0022-focus-regions.md`). Focus lands on what that
   * ring last held if the node is still there, else its
   * `initialFocus`, else its first stop, and shows.
   *
   * Resolved when the next frame finishes, like `focusNext`, so
   * the `update` that toggles a dock on may enter it in the same
   * turn — which is why a label is taken as a name to hold rather
   * than resolved now: the node need not exist yet. A frame that
   * then declares no `focusRegion` under the name raises
   * `focus-region-without-node` and moves nothing.
   */
  focusRegion(key?: string | undefined | null): void
  /**
   * The focus region in effect — the hex key of the `focusRegion`
   * node whose ring Tab walks — or `null` for the main ring. What
   * a chord that toggles between a dock and the app reads to know
   * which way it is going.
   */
  region(): string | null
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins. `key` is a hex key or a declared label, as for
   * `focus` — a label resolves through the *last* frame, so a row
   * the coming frame declares for the first time is reachable by
   * its hex key only.
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
   * The rect the last frame laid `key` out at, `{x, y, w, h}`
   * in logical viewport px, for a node that declared `onLayout`
   * — the `layout` event's numbers, read back during the next
   * build with no event and no model field (backlog C26 step
   * 2); `null` for any other key. Read while building, it
   * describes the previous frame, like `scrollGeometry`.
   */
  layoutOf(key: string): { x: number, y: number, w: number, h: number } | null
  /**
   * Where a point lands in the text a keyed node drew: a byte
   * offset into its text and the visual row within that node —
   * counted across every run the key covers, so a `line` row of
   * inline runs is one row and a wrapped run as many as it
   * wrapped to; not the ordinal `line` node a pointer event's
   * `line` names (backlog AR30) — or null for a key that drew no
   * text. A `role="none"` subtree under the key (a gutter) is
   * not its text, as the access tree reads it. `x`/`y` are the logical viewport px a
   * `click` or `drag` event carries, so a custom editor turns the
   * event into a caret position with one call — no prefix
   * measuring, no cell-width arithmetic. A node holding several
   * text runs (a `line` row of token runs) answers across them
   * in order, the way the access tree reads the line. Answered
   * from the frame that finished: the layout the pointer was
   * over.
   */
  textHit(key: string, x: number, y: number): TextHit | null
  /**
   * Opens a context menu at `(x, y)` over the node `key`, with
   * `items` as plain objects: `{label, role, enabled, id,
   * accel}`, all but `label` optional. `role` is one of
   * `custom` (the default), `separator`, `cut`, `copy`,
   * `paste`, `selectAll` or `lookUp`; the standard ones take
   * their own wording when `label` is empty, and the core
   * performs the ones it can (`docs/adr/0017-selection-as-a-scope.md`).
   *
   * Choosing a row posts `{kind:"menu", role, item}` on `key`
   * and closes the menu; a press outside it or Escape closes it
   * with nothing posted. What an app answering its own
   * `onContextMenu` calls — and what the core calls itself for
   * a right-click nobody claimed, so the two menus are one
   * implementation.
   */
  openMenu(key: string, x: number, y: number, items: MenuItemInput[]): boolean
  /**
   * Drains what choosing a menu row left for the host: the
   * clipboard, which is the host's in this library. Each entry
   * is `{kind}` — `"setClipboard"` with `text` (and `html`
   * where there is formatting to carry), `"paste"` asking for
   * what is on the clipboard (deliver it back with `commit()`,
   * which a focused editor takes as typing and a focused
   * `onKey` sink hears as `{kind:"text"}`),
   * or `"lookUp"` with the `text` to show a definition panel
   * for at `x`, `y`.
   *
   * A windowed app never needs this — the driver drains it —
   * but a headless one does: nothing else empties the queue,
   * and a Copy nobody drains is a copy that never happened.
   */
  takeMenuActions(): MenuAction[]
  /**
   * Puts `text` on the system clipboard — the action a menu's
   * Copy queues, with a door on it for an `onKey` sink that
   * hears the raw `Ctrl-c` and had nowhere to bind it (backlog
   * C33). `html` is a second flavour beside the text for the
   * host to offer, never in place of it. A window applies it
   * at its next drain (after every input and every frame); a
   * headless `Ctx` hands it out through `takeMenuActions()`.
   */
  setClipboard(text: string, html?: string | undefined | null): void
  /**
   * Asks for what is on the clipboard — the action a menu's
   * Paste queues. A window reads the clipboard and hands the
   * text back as a commit: a focused `<edit>` takes it as
   * typing, and a focused `onKey` sink hears it as
   * `{kind:"text", text, tag}`, so an app that owns its text
   * inserts a paste the way it inserts a committed IME string
   * and never reads the clipboard itself. Headless, the
   * request comes out of `takeMenuActions()` as `{kind:"paste"}`
   * and the test answers it with `commit(...)`.
   */
  requestPaste(): void
  /**
   * Whether a paste asked for is still unanswered: one ask at a
   * time — a second `requestPaste` while one is out is dropped,
   * and the `commit` that answers it (an empty one for an empty
   * clipboard) lets the next through (backlog AR34).
   */
  awaitingPaste(): boolean
  /**
   * The menu this window has open, or null:
   * `{target, x, y, items}`. What a host rendering menus itself
   * reads after `setNativeMenus(true)` — the core then keeps
   * the menu as state and draws none of it — and answers with
   * `activateMenuItem` or `closeMenu`. A row reads exactly as a
   * `menuBar()` row does (`menu_item_json`).
   */
  menu(): OpenMenu | null
  /**
   * Tells the core this host shows menus itself. It then keeps
   * the open menu as state and draws none of it: read it with
   * `menu()`, show it, and report back with `activateMenuItem`
   * or `closeMenu`. Off by default, which is the menu this
   * library draws.
   */
  setNativeMenus(on: boolean): void
  /**
   * The application menu the frame declared, or null:
   * `{revision, menus: [{label, enabled, items}]}`
   * (`docs/adr/0018-a-menu-bar-the-app-declares.md`). What a
   * host with a menu bar of its own reads after
   * `setNativeMenuBar(true)`; `revision` changes only when the
   * declaration does, so a host rebuilds nothing until it moves.
   */
  menuBar(): MenuBarState | null
  /**
   * Tells the core the platform owns the menu bar, so
   * `<menuBar/>` draws nothing and this host is the one handing
   * the declaration over (`menuBar()`) and reporting what was
   * chosen (`activateMenuBarItem`). Off by default, which is
   * the bar this library draws.
   */
  setNativeMenuBar(on: boolean): void
  /**
   * Reports that the platform's menu bar chose row `item` of
   * menu `menu` — the same path a press on the drawn bar's row
   * takes. False for a row that is not there.
   */
  activateMenuBarItem(menu: number, item: number): boolean
  /**
   * Tells the core this host can show the platform's definition
   * panel. The standard Look Up row is then offered where it
   * means something, and a force click over text asks for one.
   */
  setLookupAvailable(on: boolean): void
  /**
   * Reports that the host's own menu chose row `index` — the
   * same path a press on the drawn menu's row takes. An index
   * past the end closes the menu and posts nothing. False when
   * no menu was open.
   */
  activateMenuItem(index: number): boolean
  /** Closes whatever menu is open; true when there was one. */
  closeMenu(): boolean
  /**
   * Asks for the selection as text: `{ text, asked }`.
   *
   * `text` is the selection when the core has all of it. When
   * the selection reaches rows a virtual list never built,
   * `asked` is true instead and a `{kind:"selectionrange",
   * from:{index, byte}, to:{index, byte}}` event is posted on
   * the scope — the rows behind that gap are the app's, so the
   * app answers with `answerSelectionRange`, and the answer is
   * what reaches the clipboard
   * (`docs/adr/0017-selection-as-a-scope.md`).
   */
  requestCopy(): { text: string | null, asked: boolean }
  /**
   * Answers a `selectionrange` ask with the text for the range
   * it named, whole. False when nothing asked — a late answer
   * cannot overwrite what has been copied since.
   */
  answerSelectionRange(text: string): boolean
  /**
   * The window's selected text: what a `selectable` scope has
   * selected, or the focused `<edit>`'s selection, whichever
   * the window holds — starting either clears the other, so
   * there is never a choice to make. Null with no selection,
   * `""` when a selection exists but covers nothing (a press
   * that placed both ends together). See
   * `docs/adr/0017-selection-as-a-scope.md`.
   */
  selectionText(): string | null
  /**
   * The text selection's two ends as the drag made them:
   * `{anchor: {index, byte}, focus: {index, byte}}`, `index` the
   * data index of the virtualised row the end is in (null
   * outside every virtualised row — the `index` a
   * `selectionrange` ask would name) and `byte` the offset in
   * that row's own text. Directed, so a Shift-click that kept
   * the anchor reads as one (ADR 0029). Null with no text
   * selection; a grid's is `cellSelection()`.
   */
  selectionEnds(): SelectionEnds | null
  /**
   * A `cells` grid's selection, the window's when it lives in
   * one: the grid's key, `anchor` and `focus` as the drag made
   * them — each an absolute `line` (`originLine` plus the row,
   * so a scroll does not move it) and a `col` — and `block`
   * for a rectangular one (ADR 0017, decision 4). Null when the
   * window's selection is not a grid's; a text selection's ends
   * are `selectionEnds()`.
   */
  cellSelection(): CellSelection | null
  /**
   * The selection as HTML, carrying the formatting the text
   * declared — bold, italic, a span's own colour — and *not*
   * the node's colour, which is the app's theme rather than
   * the text's (`docs/adr/0017-selection-as-a-scope.md`).
   * Null with no text selection. Meant as a second clipboard
   * flavour beside the plain text, never instead of it.
   */
  selectionHtml(): string | null
  /**
   * Selects every run inside the selection scope a keyed node
   * declared (`selectable`), first byte to last — Select All,
   * scoped. False when that node drew no text, or is not a
   * scope. Text the frame built but never drew is included:
   * the selection is over the scope's text, not over what fits
   * on screen.
   */
  selectAllIn(key: string): boolean
  /**
   * Drops the window's selection, whichever it is. True when
   * there was one to drop.
   */
  clearSelection(): boolean
  /**
   * The caret rect for a byte offset in the text a keyed node
   * drew: logical viewport px, zero wide, one line tall — where
   * a caret, a selection edge or an IME candidate window goes.
   * A byte past the text is the end; null for a key that drew
   * no text.
   */
  caretRect(key: string, byte: number): Rect | null
  /**
   * Where the OS candidate window goes while a composition is
   * under way: the focused `<edit>`'s caret, or a custom editor's
   * `line` carrying `caret`; null when nothing with a caret is
   * focused. The windowed driver applies it itself; headless,
   * it is what a test reads to see the anchor moved.
   */
  imeRect(): Rect | null
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
  /**
   * Replaces an editor's text, leaving the caret at the end.
   *
   * It reaches an editor that does not exist yet: the `update`
   * that opens a rename field runs a frame ahead of the view
   * that declares it, so the text is held for the frame that
   * declares this name and seeds the editor there, over
   * `initial`. Name it by the label its `key` prop declares —
   * the spelling that needs nothing to exist yet, since the
   * hex key comes from an event the editor has not fired.
   * Either spelling works, as for `focus`; a label the last
   * frame declared lands at once, and one it did not is held.
   * Held for that one frame — a name nothing declares on it
   * drops its text with an `edit-text-without-editor` warning,
   * so a name the view spells differently is a line rather
   * than a field that opens with the wrong text.
   *
   * A redraw is asked for only when the text reached an editor.
   * A held seed changed nothing on screen, and the frame that
   * will — the view that declares the editor — is the app's:
   * a redraw here re-lowered the *retained* tree, which declares
   * no editor, and that was the frame the hold expired on when
   * the call came from a `dispatch` outside the loop (backlog
   * F42; `runWindowed` draws that model before it pumps).
   */
  setEditText(key: string, text: string): void
  /**
   * Registers a w×h RGBA image (pixels copied); returns its id for
   * `<image src={id}>`. Stable until `removeImage`.
   */
  addImage(width: number, height: number, rgba: Buffer): string
  removeImage(id: string): void
  /**
   * Replaces an image's pixels in place (copied): the id is
   * unchanged, so every `<image src={id}>` shows the new pixels
   * next frame with no view change; `width`/`height` may differ
   * from the registration. From the first update on the image
   * is drawn from a texture of its own — a video frame, a
   * camera, a plot the app rasterised itself
   * (`docs/adr/0025-the-image-is-the-canvas.md`). A dead id warns
   * `foreign-resource` and changes nothing.
   */
  updateImage(id: string, width: number, height: number, rgba: Buffer): void
  /**
   * Registers a WGSL fragment function; returns its id for
   * `<fragment src={id}>`. Throws when the source does not
   * compile, with the compiler's message in the app's own line
   * numbers. Idempotent by source, so the same text gets the
   * same id without being validated twice.
   */
  addFragment(wgsl: string): string
  removeFragment(id: string): void
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
  /**
   * What the last frame left owed, by kind — `animating()`
   * taken apart: `{transition, cycle, depart, requested,
   * autoscroll}`. To the window they are one, and it redraws
   * for any of them; to a test they differ, since a keyframe
   * `repeat` cycle never ends and `settled()` never resolves
   * under one. `quiet()` on the loop waits on everything but
   * `cycle` (backlog F64).
   */
  owed(): Owed
  /**
   * Byte budget for the shaped-text cache: every text a frame
   * draws is shaped once and kept, and past this many
   * (estimated) bytes the least recently drawn entries go at
   * the start of the next frame — never what the last frame
   * drew. Default 64 MB; a terminal streaming new lines lowers
   * it, a viewer that wants every page it showed kept warm
   * raises it.
   */
  setTextCacheBudget(bytes: number): void
  /**
   * What the shaped-text cache holds, in the estimated bytes
   * the budget is charged against.
   */
  textCacheBytes(): number
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Raw quads for the finished frame, `quadStride()` bytes each,
   * laid out as kui-ffi's KuiQuad (see include/kui.h) and decoded
   * by `decodeQuads`. Copied into the Buffer. A window answers
   * with what its last pump drew, so a smoke test can read the
   * frame the shipping driver painted and not only a headless
   * one's (backlog F19). Drive that window with `access(key,
   * action)` — `click`, `type` and `key` are refused there,
   * because the OS is what drives a real window.
   */
  quads(): Buffer
  /**
   * The clips this frame's quads name through their `clip`
   * index, `clipStride()` bytes each, laid out as kui-ffi's
   * KuiClip (see include/kui.h) and decoded by `decodeClips`.
   * Entry zero clips nothing, so a quad always has one; the
   * list is empty only on a frame that drew nothing.
   */
  clips(): Buffer
  /**
   * This frame's fragment draws, in the order their quads index
   * them by `uv[0]`: twenty-four doubles each — the handle as
   * two 32-bit halves, the sixteen parameters, then where the
   * draw's `image` is (0 none, 1 the atlas, 2 a texture of its
   * own), the `textureDraws` index when it is 2, and the texel
   * rect `x, y, w, h` (backlog V1). The parameters ride a side
   * list rather than the quad, so `quads()` alone cannot show
   * them and a corpus adapter needs this to compare them
   * (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
   * Empty on a frame that draws no fragment.
   */
  fragmentDraws(): Array<number>
  /**
   * This frame's texture draws, in the order their quads index
   * them by `uv[0]`: nine doubles each — the image handle as two
   * 32-bit halves, the pixels' revision, width and height, and
   * the texel rect `x, y, w, h` in the image's own texels (the
   * whole image, or the crop a `fit="cover"` made). The side
   * list a `quads()` texture quad points at
   * (`docs/adr/0025-the-image-is-the-canvas.md`, decision 3).
   * Empty on a frame that draws no texture-backed image.
   */
  textureDraws(): Array<number>
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
   * The palette this window paints with: one `0xRRGGBBAA` number
   * per role, derived from `env().system` unless this app said
   * otherwise (`setAccent` / `setTheme`). A view reads it and
   * paints with it — `<box bg={theme.surface}>` — and the stock
   * widgets already do, so a JSX app that only uses `<button>`,
   * the context menu and `<text>` follows the OS's light and
   * dark without reading this at all.
   */
  theme(): Theme
  /**
   * Keep following the OS's light/dark, but paint this accent
   * instead of the OS's — an app with a brand colour. A
   * `0xRRGGBBAA` number or a `"#hex"` string, as any colour
   * prop takes; `null` goes back to the OS's own.
   */
  setAccent(accent: number | string | null): void
  /**
   * Pin the palette: an object of role overrides on top of the
   * base named by `appearance` (`"light"`, `"dark"`, or absent
   * for the OS's), each value a colour the way a prop takes
   * one. Follows nothing afterwards — `setAccent(null)` is how
   * an app goes back to following the OS.
   *
   * `{ appearance: "dark", accent: "#d2691e" }` is a dark app
   * with one colour changed; every role the object does not
   * name keeps the base's, and the ones derived from the accent
   * (its hover, its pressed shade, the label on it, the ring,
   * the selection tint) are recomputed unless named too.
   */
  setTheme(theme: ThemeOverrides): void
  /**
   * The sizes the stock widgets are built from — the palette's
   * other axis: one number per metric, logical px before the
   * scale factor. Read it so a control of your own agrees with
   * `<button>` on a radius and a padding: `<box radius={metrics.radius}>`.
   */
  metrics(): Metrics
  /**
   * Makes these the frame's metrics: overrides on top of the
   * set in effect, or on top of `base: "comfortable"` (the
   * stock set) / `"compact"` (a dense tool's), then `scale`
   * multiplying every length — a density slider. `null`
   * restores the stock set. Density is the app's to choose;
   * nothing in the OS is followed.
   */
  setMetrics(metrics: MetricsOverrides | null): void
  /**
   * Declare the app's named colours and lengths
   * (`docs/adr/0027-tokens-beside-the-theme.md`): `{ colors:
   * { peach: '#ffcc99', ink: { light, dark } }, lengths: {
   * sideW: 132 } }`. Replaces the table whole, so an app whose
   * lengths change with a viewport tier declares again on
   * `resize`. A name a theme or metrics role owns is dropped
   * with a `reserved-token` warning. A colour may be a
   * recipe over an earlier one (ADR 0028): `{ from: 'peach',
   * ops: [['lift', 0.3]] }`, dropped with `unknown-token` when
   * its source is not there. Reference one in a prop
   * as `'$peach'` — `defineTokens` types the names. The raw
   * addon door; `index.js` wraps it to keep the encoder's map
   * in step, so call `setTokens` and not this.
   */
  setTokensRaw(tokens: { colors: [string, unknown][], lengths: [string, number][] }): void
  /**
   * The tokens a view here sees this frame, resolved: colours as
   * `0xRRGGBBAA` for the appearance in effect, lengths in px,
   * under `colors` and `lengths`. Roles are not listed — read
   * them off `theme()` and `metrics()`.
   */
  tokens(): ResolvedTokens
  /**
   * `frame` / `setView` report the `$name` references the
   * encoder could not resolve — nothing declared, or the other
   * kind — through here, as `unknown-token`, once per name.
   */
  warnUnknownTokens(names: [string, string][]): void
  /**
   * `measureText`'s door (index.js adds `measureText` itself):
   * one `<text>` element as `encoder.encodeText` writes it, and
   * the answer is what that text lays out to — `{width, height,
   * lines}` in logical px at the context's scale, capped to
   * `maxWidth` when given. The metrics do not scale linearly:
   * `measured × zoom` is not `measure(size × zoom)`, because
   * shaping rounds per size, so anything that zooms measures at
   * the size it draws.
   */
  measureTextBinary(stream: Float64Array, strings: Uint8Array, maxWidth?: number | undefined | null): TextMetrics
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
   * Every warning the core has raised so far, drained or not,
   * oldest first — the log `warnings()` leaves behind, for a
   * reader that is not the driver (a devtools stream).
   */
  warningsRaised(): Warning[]
  /**
   * Turns the per-frame node snapshot behind `nodes()` on or off
   * (off unless a devtool asked: the copy is O(nodes) a frame).
   */
  setInspect(on: boolean): void
  /**
   * Turns the core's devtools panel on or off
   * (`docs/adr/0024`): the event stream, the runtime's facts and
   * the tree, drawn by the core beside the app's own tree in the
   * main window — or where `setDevtoolsDock` says — with its
   * controls and its `Ctrl+Shift+<letter>` chords handled inside
   * the core, so nothing of it reaches `update`. `KUI_DEVTOOLS=1`
   * in the environment is the same call made by nobody, for a
   * `KuiWindow`; a headless `Ctx` never reads it.
   */
  setDevtools(on: boolean): void
  /** Whether the devtools panel is on. */
  devtools(): boolean
  /**
   * Where the devtools panel sits: `"left"`, `"right"`,
   * `"bottom"`, `"window"` (one of its own, named
   * `kui-devtools`) or `"off"` (hidden, the chords still live);
   * `"side"` is the right. Throws on any other word.
   */
  setDevtoolsDock(dock: DevtoolsDock): void
  /** Where the devtools panel sits (see `setDevtoolsDock`). */
  devtoolsDock(): DevtoolsDock
  /**
   * Seeds the panel's theme override, what its `T` and `A`
   * chords cycle from: `base` is `"light"`, `"dark"` or `null`
   * for the app's own; `accent` an `#rrggbb` string or `null`.
   */
  setDevtoolsTheme(base: 'light' | 'dark' | null, accent: string | null): void
  /**
   * Respells the chord that moves the keyboard into the panel
   * and back out — and brings a hidden panel back — from its
   * default `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod`
   * is Command on macOS, Control elsewhere), `"⌥⌘I"`, any
   * spelling a menu item's `accel` takes. The panel's other
   * chords stay `Ctrl+Shift+<letter>`. With another chord set,
   * `Ctrl+Shift+I` reaches the app like any other press. Throws
   * on a spelling kui cannot name.
   */
  setDevtoolsKey(key: string): void
  /**
   * The chord `setDevtoolsKey` set, or the default, in its
   * portable spelling: `"ctrl+shift+i"`, `"f12"`,
   * `"super+alt+d"`.
   */
  devtoolsKey(): string
  /**
   * The declared devtools tab on show, by name, or `null` for
   * one of the panel's own, the panel off or popped out (ADR
   * 0032). What `frame` / `setView` read once before encoding,
   * so a `<devtoolsTab>`'s function child is called only for
   * that tab.
   */
  devtoolsShownTab(): string | null
  /**
   * The node the panel's tree tab has selected, as a hex key,
   * or `null` (ADR 0032, decision 4) — what an inspector in a
   * declared tab reads to say which node it is about.
   */
  devtoolsSelected(): string | null
  /** The tree row under the pointer, as a hex key, or `null`. */
  devtoolsHovered(): string | null
  /** The node the picker is over while picking, or `null`. */
  devtoolsPicked(): string | null
  /**
   * Raises the panel's picker from outside it — an inspector in
   * a `<devtoolsTab>` asking "which node?" — or puts it away
   * (ADR 0032, decision 4). Picking happens over the app in
   * the main window: `devtoolsPicked()` is the node under the
   * pointer while it is up, and the press lands it in
   * `devtoolsSelected()`. Raised while a declared tab is on
   * show, the pick leaves that tab up; raised otherwise it is
   * the `Ctrl+Shift+P` pick and shows the tree tab. A hidden
   * panel comes back docked.
   */
  setDevtoolsPick(on: boolean): void
  /** Whether the panel's picker is up. */
  devtoolsPicking(): boolean
  /**
   * Shows the panel's tab named `name` from the app's side —
   * what the strip's click and `Ctrl+Shift+N` do, for a command
   * that jumps to the app's own tab (ADR 0032). `name` is one
   * of the panel's own (`facts`, `events`, `tree`) or a
   * `<devtoolsTab>`'s. A declared name the panel does not list
   * yet is kept and shows once a frame declares it; the return
   * says whether the panel lists it now. A hidden panel comes
   * back docked; `setDevtools(true)` is still the app's to
   * call. Call it once, not every frame: it would pin the strip
   * against the user's own clicks.
   */
  setDevtoolsTab(name: DevtoolsTab | (string & {})): boolean
  /**
   * The tab the panel is on, by name: one of its own or a
   * `<devtoolsTab>`'s — the selection itself, panel on or off,
   * unlike `devtoolsShownTab()`, which is the encoder's reading
   * of a declared tab on show.
   */
  devtoolsCurrentTab(): DevtoolsTab | (string & {})
  /**
   * Selects a node in the panel's tree tab from outside it and
   * reveals it there, as the picker does; `null` clears. `key`
   * is a hex key an event carried or `keyOf` answered.
   */
  setDevtoolsSelected(key?: string | undefined | null): void
  /**
   * The key legend the panel's facts tab shows: `[keys, what]`
   * pairs.
   */
  setDevtoolsLegend(legend: [string, string][]): void
  /**
   * The last finished frame's nodes in tree order, each with what
   * it is, the label it was opened under, where layout put it
   * (in the app's viewport, like `layoutOf`; backlog AR36), and
   * the declarations that explain the rest — what a tree view
   * and a node inspector are built from. Empty until
   * `setInspect(true)` and a frame after it.
   */
  nodes(): NodeInfo[]
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
   * A request from assistive technology on a node: an `AccessAction`
   * name the node advertises, with `value` the new text for
   * `setValue`. `key` is either spelling of the node — the hex key
   * an event carried (16 digits), or the label its `key` prop
   * declared, resolved through the last frame (see `focus`).
   * Resolved like its pointer/keyboard equivalent, so the resulting
   * events come out of `pollEvents`. A real screen reader's
   * requests arrive through a window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame. `key` is either spelling,
   * as for `focus`: the label a `key` prop declared, or the hex
   * key an event carried. For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  /**
   * Press state as of the last frame; `key` is either spelling,
   * as for `isHovered`.
   */
  isPressed(key: string): boolean
  /**
   * Whether files dragged in from the OS are over `key` (ADR
   * 0031) — for drop-dependent layout; the colour swap is the
   * `dropBg` prop. `key` is either spelling, as for `isHovered`.
   */
  isDropTarget(key: string): boolean
  /**
   * The `onDrop` zone the dragged files are over, as the hex key
   * an event carries, or null — what a driver answers the OS
   * with after each `dragFiles`, and what a test reads to say a
   * zone was found.
   */
  dropTarget(): string | null
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
   * `onKey` sink, a button Tab landed on (see `focused`). `key` is
   * a hex key or a declared label, as for `focus`.
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab walk every control in tree order, Enter and Space
   * press the focused one, and the arrows nudge a focused
   * slider — `press("tab")`, `press(" ")`, `press("right")`,
   * which is what a keyboard sends. (`key("tab")` is the half
   * of that press the core acts on, for a test that means to
   * drive one channel; `keyDown` is the other half, the one an
   * `onKey` sink hears.)
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * The caret's blink phase — `true` draws it (backlog C35). A
   * custom editor reads it in `view` and skips its caret node
   * on the off phase, keeping the `caret` row on its `line`
   * either way; the window's clock sets it while a focused
   * `<edit>` or such a line has a caret, and parks it hidden
   * while the window has no keyboard. Always `true` headless.
   */
  caretVisible(): boolean
  /**
   * Whether there is a caret to blink: a focused `<edit>`'s, or
   * the `caret` a `line` under the focused sink declares — unless
   * the line declares it `caretSolid` (backlog F68), which
   * anchors and reads but arms no clock. What the window's
   * clock is armed on; headless, what a test reads to see that
   * an idle view asks for no frame.
   */
  hasCaret(): boolean
  /**
   * The driver's half of the blink: sets the phase. A window
   * runs its own clock; headless, a test drives it to see the
   * off phase drawn.
   */
  setCaretVisible(visible: boolean): void
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   *
   * `key` is either spelling of the node: the 16-digit hex key an
   * event carried, or the label its `key` prop declared —
   * `focus('note')` — resolved through the last frame, so a node
   * the user has never touched can be named. Labels are unique
   * among siblings, not across the tree: when two nodes declare
   * the same one, the first in tree order wins and an
   * `ambiguous-key` warning says so. A label no node declared
   * throws.
   */
  focus(key: string): void
  /**
   * The hex key of the node a label names — the label a `key`
   * prop declared, resolved through the frame being built so
   * far and then the last finished one — or null when no node
   * declared it. The door for holding a key across frames;
   * every call that takes a key takes the label too, so this
   * is for caching one, or for checking that a name reached
   * the view. Two nodes on one label under different parents
   * resolve to the first in tree order and raise
   * `ambiguous-key` — among the host's own first, and a
   * plugin filling a slot is answered from its own nodes only.
   */
  keyOf(label: string): string | null
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
   * Enters a focus region — a box declared `focusRegion`, named by
   * the label its `key` prop declares or by the hex key an event
   * carried — or the main ring for `null`
   * (`docs/adr/0022-focus-regions.md`). Focus lands on what that
   * ring last held if the node is still there, else its
   * `initialFocus`, else its first stop, and shows.
   *
   * Resolved when the next frame finishes, like `focusNext`, so
   * the `update` that toggles a dock on may enter it in the same
   * turn — which is why a label is taken as a name to hold rather
   * than resolved now: the node need not exist yet. A frame that
   * then declares no `focusRegion` under the name raises
   * `focus-region-without-node` and moves nothing.
   */
  focusRegion(key?: string | undefined | null): void
  /**
   * The focus region in effect — the hex key of the `focusRegion`
   * node whose ring Tab walks — or `null` for the main ring. What
   * a chord that toggles between a dock and the app reads to know
   * which way it is going.
   */
  region(): string | null
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins. `key` is a hex key or a declared label, as for
   * `focus` — a label resolves through the *last* frame, so a row
   * the coming frame declares for the first time is reachable by
   * its hex key only.
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
   * The rect the last frame laid `key` out at, `{x, y, w, h}`
   * in logical viewport px, for a node that declared `onLayout`
   * — the `layout` event's numbers, read back during the next
   * build with no event and no model field (backlog C26 step
   * 2); `null` for any other key. Read while building, it
   * describes the previous frame, like `scrollGeometry`.
   */
  layoutOf(key: string): { x: number, y: number, w: number, h: number } | null
  /**
   * Where a point lands in the text a keyed node drew: a byte
   * offset into its text and the visual row within that node —
   * counted across every run the key covers, so a `line` row of
   * inline runs is one row and a wrapped run as many as it
   * wrapped to; not the ordinal `line` node a pointer event's
   * `line` names (backlog AR30) — or null for a key that drew no
   * text. A `role="none"` subtree under the key (a gutter) is
   * not its text, as the access tree reads it. `x`/`y` are the logical viewport px a
   * `click` or `drag` event carries, so a custom editor turns the
   * event into a caret position with one call — no prefix
   * measuring, no cell-width arithmetic. A node holding several
   * text runs (a `line` row of token runs) answers across them
   * in order, the way the access tree reads the line. Answered
   * from the frame that finished: the layout the pointer was
   * over.
   */
  textHit(key: string, x: number, y: number): TextHit | null
  /**
   * Opens a context menu at `(x, y)` over the node `key`, with
   * `items` as plain objects: `{label, role, enabled, id,
   * accel}`, all but `label` optional. `role` is one of
   * `custom` (the default), `separator`, `cut`, `copy`,
   * `paste`, `selectAll` or `lookUp`; the standard ones take
   * their own wording when `label` is empty, and the core
   * performs the ones it can (`docs/adr/0017-selection-as-a-scope.md`).
   *
   * Choosing a row posts `{kind:"menu", role, item}` on `key`
   * and closes the menu; a press outside it or Escape closes it
   * with nothing posted. What an app answering its own
   * `onContextMenu` calls — and what the core calls itself for
   * a right-click nobody claimed, so the two menus are one
   * implementation.
   */
  openMenu(key: string, x: number, y: number, items: MenuItemInput[]): boolean
  /**
   * Drains what choosing a menu row left for the host: the
   * clipboard, which is the host's in this library. Each entry
   * is `{kind}` — `"setClipboard"` with `text` (and `html`
   * where there is formatting to carry), `"paste"` asking for
   * what is on the clipboard (deliver it back with `commit()`,
   * which a focused editor takes as typing and a focused
   * `onKey` sink hears as `{kind:"text"}`),
   * or `"lookUp"` with the `text` to show a definition panel
   * for at `x`, `y`.
   *
   * A windowed app never needs this — the driver drains it —
   * but a headless one does: nothing else empties the queue,
   * and a Copy nobody drains is a copy that never happened.
   */
  takeMenuActions(): MenuAction[]
  /**
   * Puts `text` on the system clipboard — the action a menu's
   * Copy queues, with a door on it for an `onKey` sink that
   * hears the raw `Ctrl-c` and had nowhere to bind it (backlog
   * C33). `html` is a second flavour beside the text for the
   * host to offer, never in place of it. A window applies it
   * at its next drain (after every input and every frame); a
   * headless `Ctx` hands it out through `takeMenuActions()`.
   */
  setClipboard(text: string, html?: string | undefined | null): void
  /**
   * Asks for what is on the clipboard — the action a menu's
   * Paste queues. A window reads the clipboard and hands the
   * text back as a commit: a focused `<edit>` takes it as
   * typing, and a focused `onKey` sink hears it as
   * `{kind:"text", text, tag}`, so an app that owns its text
   * inserts a paste the way it inserts a committed IME string
   * and never reads the clipboard itself. Headless, the
   * request comes out of `takeMenuActions()` as `{kind:"paste"}`
   * and the test answers it with `commit(...)`.
   */
  requestPaste(): void
  /**
   * Whether a paste asked for is still unanswered: one ask at a
   * time — a second `requestPaste` while one is out is dropped,
   * and the `commit` that answers it (an empty one for an empty
   * clipboard) lets the next through (backlog AR34).
   */
  awaitingPaste(): boolean
  /**
   * The menu this window has open, or null:
   * `{target, x, y, items}`. What a host rendering menus itself
   * reads after `setNativeMenus(true)` — the core then keeps
   * the menu as state and draws none of it — and answers with
   * `activateMenuItem` or `closeMenu`. A row reads exactly as a
   * `menuBar()` row does (`menu_item_json`).
   */
  menu(): OpenMenu | null
  /**
   * Tells the core this host shows menus itself. It then keeps
   * the open menu as state and draws none of it: read it with
   * `menu()`, show it, and report back with `activateMenuItem`
   * or `closeMenu`. Off by default, which is the menu this
   * library draws.
   */
  setNativeMenus(on: boolean): void
  /**
   * The application menu the frame declared, or null:
   * `{revision, menus: [{label, enabled, items}]}`
   * (`docs/adr/0018-a-menu-bar-the-app-declares.md`). What a
   * host with a menu bar of its own reads after
   * `setNativeMenuBar(true)`; `revision` changes only when the
   * declaration does, so a host rebuilds nothing until it moves.
   */
  menuBar(): MenuBarState | null
  /**
   * Tells the core the platform owns the menu bar, so
   * `<menuBar/>` draws nothing and this host is the one handing
   * the declaration over (`menuBar()`) and reporting what was
   * chosen (`activateMenuBarItem`). Off by default, which is
   * the bar this library draws.
   */
  setNativeMenuBar(on: boolean): void
  /**
   * Reports that the platform's menu bar chose row `item` of
   * menu `menu` — the same path a press on the drawn bar's row
   * takes. False for a row that is not there.
   */
  activateMenuBarItem(menu: number, item: number): boolean
  /**
   * Tells the core this host can show the platform's definition
   * panel. The standard Look Up row is then offered where it
   * means something, and a force click over text asks for one.
   */
  setLookupAvailable(on: boolean): void
  /**
   * Reports that the host's own menu chose row `index` — the
   * same path a press on the drawn menu's row takes. An index
   * past the end closes the menu and posts nothing. False when
   * no menu was open.
   */
  activateMenuItem(index: number): boolean
  /** Closes whatever menu is open; true when there was one. */
  closeMenu(): boolean
  /**
   * Asks for the selection as text: `{ text, asked }`.
   *
   * `text` is the selection when the core has all of it. When
   * the selection reaches rows a virtual list never built,
   * `asked` is true instead and a `{kind:"selectionrange",
   * from:{index, byte}, to:{index, byte}}` event is posted on
   * the scope — the rows behind that gap are the app's, so the
   * app answers with `answerSelectionRange`, and the answer is
   * what reaches the clipboard
   * (`docs/adr/0017-selection-as-a-scope.md`).
   */
  requestCopy(): { text: string | null, asked: boolean }
  /**
   * Answers a `selectionrange` ask with the text for the range
   * it named, whole. False when nothing asked — a late answer
   * cannot overwrite what has been copied since.
   */
  answerSelectionRange(text: string): boolean
  /**
   * The window's selected text: what a `selectable` scope has
   * selected, or the focused `<edit>`'s selection, whichever
   * the window holds — starting either clears the other, so
   * there is never a choice to make. Null with no selection,
   * `""` when a selection exists but covers nothing (a press
   * that placed both ends together). See
   * `docs/adr/0017-selection-as-a-scope.md`.
   */
  selectionText(): string | null
  /**
   * The text selection's two ends as the drag made them:
   * `{anchor: {index, byte}, focus: {index, byte}}`, `index` the
   * data index of the virtualised row the end is in (null
   * outside every virtualised row — the `index` a
   * `selectionrange` ask would name) and `byte` the offset in
   * that row's own text. Directed, so a Shift-click that kept
   * the anchor reads as one (ADR 0029). Null with no text
   * selection; a grid's is `cellSelection()`.
   */
  selectionEnds(): SelectionEnds | null
  /**
   * A `cells` grid's selection, the window's when it lives in
   * one: the grid's key, `anchor` and `focus` as the drag made
   * them — each an absolute `line` (`originLine` plus the row,
   * so a scroll does not move it) and a `col` — and `block`
   * for a rectangular one (ADR 0017, decision 4). Null when the
   * window's selection is not a grid's; a text selection's ends
   * are `selectionEnds()`.
   */
  cellSelection(): CellSelection | null
  /**
   * The selection as HTML, carrying the formatting the text
   * declared — bold, italic, a span's own colour — and *not*
   * the node's colour, which is the app's theme rather than
   * the text's (`docs/adr/0017-selection-as-a-scope.md`).
   * Null with no text selection. Meant as a second clipboard
   * flavour beside the plain text, never instead of it.
   */
  selectionHtml(): string | null
  /**
   * Selects every run inside the selection scope a keyed node
   * declared (`selectable`), first byte to last — Select All,
   * scoped. False when that node drew no text, or is not a
   * scope. Text the frame built but never drew is included:
   * the selection is over the scope's text, not over what fits
   * on screen.
   */
  selectAllIn(key: string): boolean
  /**
   * Drops the window's selection, whichever it is. True when
   * there was one to drop.
   */
  clearSelection(): boolean
  /**
   * The caret rect for a byte offset in the text a keyed node
   * drew: logical viewport px, zero wide, one line tall — where
   * a caret, a selection edge or an IME candidate window goes.
   * A byte past the text is the end; null for a key that drew
   * no text.
   */
  caretRect(key: string, byte: number): Rect | null
  /**
   * Where the OS candidate window goes while a composition is
   * under way: the focused `<edit>`'s caret, or a custom editor's
   * `line` carrying `caret`; null when nothing with a caret is
   * focused. The windowed driver applies it itself; headless,
   * it is what a test reads to see the anchor moved.
   */
  imeRect(): Rect | null
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
  /**
   * Replaces an editor's text, leaving the caret at the end.
   *
   * It reaches an editor that does not exist yet: the `update`
   * that opens a rename field runs a frame ahead of the view
   * that declares it, so the text is held for the frame that
   * declares this name and seeds the editor there, over
   * `initial`. Name it by the label its `key` prop declares —
   * the spelling that needs nothing to exist yet, since the
   * hex key comes from an event the editor has not fired.
   * Either spelling works, as for `focus`; a label the last
   * frame declared lands at once, and one it did not is held.
   * Held for that one frame — a name nothing declares on it
   * drops its text with an `edit-text-without-editor` warning,
   * so a name the view spells differently is a line rather
   * than a field that opens with the wrong text.
   *
   * A redraw is asked for only when the text reached an editor.
   * A held seed changed nothing on screen, and the frame that
   * will — the view that declares the editor — is the app's:
   * a redraw here re-lowered the *retained* tree, which declares
   * no editor, and that was the frame the hold expired on when
   * the call came from a `dispatch` outside the loop (backlog
   * F42; `runWindowed` draws that model before it pumps).
   */
  setEditText(key: string, text: string): void
}

/** Byte stride of one quad in the `quads()` buffer. */
export declare function quadStride(): number

/** Byte stride of one clip in the `clips()` buffer. */
export declare function clipStride(): number

/**
 * A real kui window (winit + wgpu) driven from Node. The event loop is
 * pumped, not run: call `pump()` between libuv turns so winit and libuv share
 * the main thread — or prefer `runWindowed`, which does that for you, unless
 * you are building your own loop. One event loop per process (winit event
 * loops are not recreatable on every platform), any number of windows on
 * it: a view whose root declares `windows` opens more, `windows()` lists
 * them, and `setView` takes the name of the one a tree is for.
 */
export declare class KuiWindow {
  /**
   * Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
   * chrome: "native" | "custom" | "borderless", textAa: "auto" | "gray"
   * | "subpixel", system}`. The min/max pairs bound what the user can
   * resize the window to; either half may stand alone. `system` pins part of `env.system` over what the OS
   * says, for the life of the window — `{motion: 'reduced'}` is what a
   * user who asked for less motion would get, on a machine whose owner
   * did not; see `WindowOptions`.
   */
  constructor(title: string, options?: WindowOptions)
  /**
   * Which window the per-window doors — `focus`, `editText`,
   * `setEditText`, `isHovered`, `scrollGeometry`, `openMenu`,
   * `selectionText`, `setTheme`, `setMetrics`, `setTokens`, the
   * devtools setters, input injection — address from here on: a name
   * from `windows()`, or the id an event carries in `window`; left out,
   * the main window. Returns whether that window is open now; until it
   * is, the doors address the main window. `runWindowed` aims the
   * surface at the window whose view it is calling and at the window
   * an event came from before handing the surface to `update`, so an
   * app that never calls this reads and writes the window it is being
   * asked about (backlog AR12). Resources, `windows()`, `pump` and
   * `pollEvents` are the session's and unaffected.
   */
  useWindow(window?: string | number): boolean
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
   * Registers what the window calls as it goes for good — its close
   * button, `close()`, Quit from the menu or the dock — once, from
   * inside the `pump()` that saw it and before that pump returns
   * (backlog RG1). On macOS a Quit ends the process inside that pump:
   * `pump()` never returns, `runWindowed` never resolves and nothing
   * after it runs, not even `process.on('exit')` — so this is the only
   * thing an app runs on ⌘Q. `runWindowed` registers its config's
   * `teardown(model)` here; a driver of its own does the same. The
   * window is gone by then: the callback saves and returns, and does
   * not call the window's doors. The last registration wins.
   */
  onTeardown(callback: () => void): void
  /**
   * `pump`, but parked for up to `timeoutMs` — returning the moment an
   * OS event or a `Waker` wake arrives, and otherwise when the time is
   * up. Returns false once the window has closed. `timeoutMs` is clamped
   * to an hour; anything not finite and positive parks not at all.
   *
   * **What this buys is latency, and it is not what `runWindowed` idles
   * on.** The wake is the event itself rather than the next tick after
   * it, so the period stops setting the response time — but two measured
   * things make it the wrong default, and both surprise:
   *
   * - **It does not save CPU; it costs.** winit 0.30's macOS pump stops
   *   on the *first* wake of any kind, so a park does not hold: a 200 ms
   *   park runs 101 ms on average. Against polling at the same period, a
   *   32 ms period is 49 pumps a second and 4.59% of a core parked,
   *   against 30 pumps and 1.80% polled.
   * - **A blocked main thread is a blocked libuv**, and Node cannot be
   *   asked whether that is safe — `process.getActiveResourcesInfo()`
   *   reports nothing for an in-flight `fs.readFile`. Behind a 50 ms
   *   park, 200 sequential `await readFile` went from 6 ms to 4 s.
   *
   * So reach for it only in a loop that owns its whole process and does
   * no async I/O, and wants the latency. Otherwise `pump()` on a timer
   * is both cheaper and safer — which is what `runWindowed` does.
   */
  pumpUntil(timeoutMs: number): boolean
  /**
   * How long until the window next needs a pump, in ms — `null` when
   * nothing the runner knows about is due.
   *
   * A driver on a fixed interval hits every deadline the shell has
   * already worked out a whole interval late: the caret blink, a
   * transition's next frame, the audio poll, a window's first-frame
   * retry. Ask after each pump and sleep to the answer instead, which is
   * what `runWindowed` does. It only ever asks for *sooner* — whether an
   * OS event is waiting is not something it can say, so a driver still
   * needs a ceiling of its own.
   */
  nextDeadlineMs(): number | null
  /**
   * The viewport the app lays out into, in logical px, plus the scale
   * factor: `{width, height, scale}` — the window's inner size, less
   * the devtools' dock while the panel is docked (`docs/adr/0024`).
   * Readable before the first frame (in `setup` and `init`, where
   * `env().viewport` is still 0×0), and re-reported as a
   * `{kind:"resize", width, height, scale}` event through `pollEvents`
   * — a `ResizeMsg` — whenever the window changes size, moves to a
   * display with another DPI, or the dock comes, goes or is dragged.
   * Backlog F43: this was the window's inner size, so an app that seeded
   * its tiers from it under `KUI_DEVTOOLS=1` drew for the whole window.
   */
  size(): WindowSize
  /**
   * Frame timing measured by the runner — what the latency HUD draws,
   * as data: `{frames, framesTotal, pumps, last: {inputMs, viewMs,
   * layoutMs, renderMs, waitMs, totalMs, workMs} | null, avgTotalMs,
   * maxTotalMs, avgWorkMs, maxWorkMs}`. The averages and maxima are
   * over the last 120 frames and `frames` is how many of those the
   * ring holds — it climbs to 120 in the first two seconds and stays
   * there. `framesTotal` and `pumps` are the monotonic counts of every
   * frame painted and every `pump()` taken (backlog F62), so two
   * readings a second apart are that second's frame and pump rates.
   * `waitMs` is vsync backpressure; `workMs` is everything else.
   */
  frameStats(): FrameTiming
  /** Asks the window to close; the next pump returns false. */
  close(): void
}

/** Byte stride of one quad in the `quads()` buffer. */
export declare function quadStride(): number

/** Byte stride of one clip in the `clips()` buffer. */
export declare function clipStride(): number

/**
 * A real kui window (winit + wgpu) driven from Node. The event loop is
 * pumped, not run: call `pump()` between libuv turns so winit and libuv share
 * the main thread — or prefer `runWindowed`, which does that for you, unless
 * you are building your own loop. One event loop per process (winit event
 * loops are not recreatable on every platform), any number of windows on
 * it: a view whose root declares `windows` opens more, `windows()` lists
 * them, and `setView` takes the name of the one a tree is for.
 */
export declare class KuiWindow {
  /**
   * Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
   * chrome: "native" | "custom" | "borderless", textAa: "auto" | "gray"
   * | "subpixel", system}`. The min/max pairs bound what the user can
   * resize the window to; either half may stand alone. `system` pins part of `env.system` over what the OS
   * says, for the life of the window — `{motion: 'reduced'}` is what a
   * user who asked for less motion would get, on a machine whose owner
   * did not; see `WindowOptions`.
   */
  constructor(title: string, options?: WindowOptions)
  /**
   * Which window the per-window doors — `focus`, `editText`,
   * `setEditText`, `isHovered`, `scrollGeometry`, `openMenu`,
   * `selectionText`, `setTheme`, `setMetrics`, `setTokens`, the
   * devtools setters, input injection — address from here on: a name
   * from `windows()`, or the id an event carries in `window`; left out,
   * the main window. Returns whether that window is open now; until it
   * is, the doors address the main window. `runWindowed` aims the
   * surface at the window whose view it is calling and at the window
   * an event came from before handing the surface to `update`, so an
   * app that never calls this reads and writes the window it is being
   * asked about (backlog AR12). Resources, `windows()`, `pump` and
   * `pollEvents` are the session's and unaffected.
   */
  useWindow(window?: string | number): boolean
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
   * Registers what the window calls as it goes for good — its close
   * button, `close()`, Quit from the menu or the dock — once, from
   * inside the `pump()` that saw it and before that pump returns
   * (backlog RG1). On macOS a Quit ends the process inside that pump:
   * `pump()` never returns, `runWindowed` never resolves and nothing
   * after it runs, not even `process.on('exit')` — so this is the only
   * thing an app runs on ⌘Q. `runWindowed` registers its config's
   * `teardown(model)` here; a driver of its own does the same. The
   * window is gone by then: the callback saves and returns, and does
   * not call the window's doors. The last registration wins.
   */
  onTeardown(callback: () => void): void
  /**
   * `pump`, but parked for up to `timeoutMs` — returning the moment an
   * OS event or a `Waker` wake arrives, and otherwise when the time is
   * up. Returns false once the window has closed. `timeoutMs` is clamped
   * to an hour; anything not finite and positive parks not at all.
   *
   * **What this buys is latency, and it is not what `runWindowed` idles
   * on.** The wake is the event itself rather than the next tick after
   * it, so the period stops setting the response time — but two measured
   * things make it the wrong default, and both surprise:
   *
   * - **It does not save CPU; it costs.** winit 0.30's macOS pump stops
   *   on the *first* wake of any kind, so a park does not hold: a 200 ms
   *   park runs 101 ms on average. Against polling at the same period, a
   *   32 ms period is 49 pumps a second and 4.59% of a core parked,
   *   against 30 pumps and 1.80% polled.
   * - **A blocked main thread is a blocked libuv**, and Node cannot be
   *   asked whether that is safe — `process.getActiveResourcesInfo()`
   *   reports nothing for an in-flight `fs.readFile`. Behind a 50 ms
   *   park, 200 sequential `await readFile` went from 6 ms to 4 s.
   *
   * So reach for it only in a loop that owns its whole process and does
   * no async I/O, and wants the latency. Otherwise `pump()` on a timer
   * is both cheaper and safer — which is what `runWindowed` does.
   */
  pumpUntil(timeoutMs: number): boolean
  /**
   * How long until the window next needs a pump, in ms — `null` when
   * nothing the runner knows about is due.
   *
   * A driver on a fixed interval hits every deadline the shell has
   * already worked out a whole interval late: the caret blink, a
   * transition's next frame, the audio poll, a window's first-frame
   * retry. Ask after each pump and sleep to the answer instead, which is
   * what `runWindowed` does. It only ever asks for *sooner* — whether an
   * OS event is waiting is not something it can say, so a driver still
   * needs a ceiling of its own.
   */
  nextDeadlineMs(): number | null
  /**
   * The viewport the app lays out into, in logical px, plus the scale
   * factor: `{width, height, scale}` — the window's inner size, less
   * the devtools' dock while the panel is docked (`docs/adr/0024`).
   * Readable before the first frame (in `setup` and `init`, where
   * `env().viewport` is still 0×0), and re-reported as a
   * `{kind:"resize", width, height, scale}` event through `pollEvents`
   * — a `ResizeMsg` — whenever the window changes size, moves to a
   * display with another DPI, or the dock comes, goes or is dragged.
   * Backlog F43: this was the window's inner size, so an app that seeded
   * its tiers from it under `KUI_DEVTOOLS=1` drew for the whole window.
   */
  size(): WindowSize
  /**
   * Frame timing measured by the runner — what the latency HUD draws,
   * as data: `{frames, framesTotal, pumps, last: {inputMs, viewMs,
   * layoutMs, renderMs, waitMs, totalMs, workMs} | null, avgTotalMs,
   * maxTotalMs, avgWorkMs, maxWorkMs}`. The averages and maxima are
   * over the last 120 frames and `frames` is how many of those the
   * ring holds — it climbs to 120 in the first two seconds and stays
   * there. `framesTotal` and `pumps` are the monotonic counts of every
   * frame painted and every `pump()` taken (backlog F62), so two
   * readings a second apart are that second's frame and pump rates.
   * `waitMs` is vsync backpressure; `workMs` is everything else.
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
   * Replaces an image's pixels in place (copied): the id is
   * unchanged, so every `<image src={id}>` shows the new pixels
   * next frame with no view change; `width`/`height` may differ
   * from the registration. From the first update on the image
   * is drawn from a texture of its own — a video frame, a
   * camera, a plot the app rasterised itself
   * (`docs/adr/0025-the-image-is-the-canvas.md`). A dead id warns
   * `foreign-resource` and changes nothing.
   */
  updateImage(id: string, width: number, height: number, rgba: Buffer): void
  /**
   * Registers a WGSL fragment function; returns its id for
   * `<fragment src={id}>`. Throws when the source does not
   * compile, with the compiler's message in the app's own line
   * numbers. Idempotent by source, so the same text gets the
   * same id without being validated twice.
   */
  addFragment(wgsl: string): string
  removeFragment(id: string): void
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
  /**
   * What the last frame left owed, by kind — `animating()`
   * taken apart: `{transition, cycle, depart, requested,
   * autoscroll}`. To the window they are one, and it redraws
   * for any of them; to a test they differ, since a keyframe
   * `repeat` cycle never ends and `settled()` never resolves
   * under one. `quiet()` on the loop waits on everything but
   * `cycle` (backlog F64).
   */
  owed(): Owed
  /**
   * Byte budget for the shaped-text cache: every text a frame
   * draws is shaped once and kept, and past this many
   * (estimated) bytes the least recently drawn entries go at
   * the start of the next frame — never what the last frame
   * drew. Default 64 MB; a terminal streaming new lines lowers
   * it, a viewer that wants every page it showed kept warm
   * raises it.
   */
  setTextCacheBudget(bytes: number): void
  /**
   * What the shaped-text cache holds, in the estimated bytes
   * the budget is charged against.
   */
  textCacheBytes(): number
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Raw quads for the finished frame, `quadStride()` bytes each,
   * laid out as kui-ffi's KuiQuad (see include/kui.h) and decoded
   * by `decodeQuads`. Copied into the Buffer. A window answers
   * with what its last pump drew, so a smoke test can read the
   * frame the shipping driver painted and not only a headless
   * one's (backlog F19). Drive that window with `access(key,
   * action)` — `click`, `type` and `key` are refused there,
   * because the OS is what drives a real window.
   */
  quads(): Buffer
  /**
   * The clips this frame's quads name through their `clip`
   * index, `clipStride()` bytes each, laid out as kui-ffi's
   * KuiClip (see include/kui.h) and decoded by `decodeClips`.
   * Entry zero clips nothing, so a quad always has one; the
   * list is empty only on a frame that drew nothing.
   */
  clips(): Buffer
  /**
   * This frame's fragment draws, in the order their quads index
   * them by `uv[0]`: twenty-four doubles each — the handle as
   * two 32-bit halves, the sixteen parameters, then where the
   * draw's `image` is (0 none, 1 the atlas, 2 a texture of its
   * own), the `textureDraws` index when it is 2, and the texel
   * rect `x, y, w, h` (backlog V1). The parameters ride a side
   * list rather than the quad, so `quads()` alone cannot show
   * them and a corpus adapter needs this to compare them
   * (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
   * Empty on a frame that draws no fragment.
   */
  fragmentDraws(): Array<number>
  /**
   * This frame's texture draws, in the order their quads index
   * them by `uv[0]`: nine doubles each — the image handle as two
   * 32-bit halves, the pixels' revision, width and height, and
   * the texel rect `x, y, w, h` in the image's own texels (the
   * whole image, or the crop a `fit="cover"` made). The side
   * list a `quads()` texture quad points at
   * (`docs/adr/0025-the-image-is-the-canvas.md`, decision 3).
   * Empty on a frame that draws no texture-backed image.
   */
  textureDraws(): Array<number>
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
   * The palette this window paints with: one `0xRRGGBBAA` number
   * per role, derived from `env().system` unless this app said
   * otherwise (`setAccent` / `setTheme`). A view reads it and
   * paints with it — `<box bg={theme.surface}>` — and the stock
   * widgets already do, so a JSX app that only uses `<button>`,
   * the context menu and `<text>` follows the OS's light and
   * dark without reading this at all.
   */
  theme(): Theme
  /**
   * Keep following the OS's light/dark, but paint this accent
   * instead of the OS's — an app with a brand colour. A
   * `0xRRGGBBAA` number or a `"#hex"` string, as any colour
   * prop takes; `null` goes back to the OS's own.
   */
  setAccent(accent: number | string | null): void
  /**
   * Pin the palette: an object of role overrides on top of the
   * base named by `appearance` (`"light"`, `"dark"`, or absent
   * for the OS's), each value a colour the way a prop takes
   * one. Follows nothing afterwards — `setAccent(null)` is how
   * an app goes back to following the OS.
   *
   * `{ appearance: "dark", accent: "#d2691e" }` is a dark app
   * with one colour changed; every role the object does not
   * name keeps the base's, and the ones derived from the accent
   * (its hover, its pressed shade, the label on it, the ring,
   * the selection tint) are recomputed unless named too.
   */
  setTheme(theme: ThemeOverrides): void
  /**
   * The sizes the stock widgets are built from — the palette's
   * other axis: one number per metric, logical px before the
   * scale factor. Read it so a control of your own agrees with
   * `<button>` on a radius and a padding: `<box radius={metrics.radius}>`.
   */
  metrics(): Metrics
  /**
   * Makes these the frame's metrics: overrides on top of the
   * set in effect, or on top of `base: "comfortable"` (the
   * stock set) / `"compact"` (a dense tool's), then `scale`
   * multiplying every length — a density slider. `null`
   * restores the stock set. Density is the app's to choose;
   * nothing in the OS is followed.
   */
  setMetrics(metrics: MetricsOverrides | null): void
  /**
   * Declare the app's named colours and lengths
   * (`docs/adr/0027-tokens-beside-the-theme.md`): `{ colors:
   * { peach: '#ffcc99', ink: { light, dark } }, lengths: {
   * sideW: 132 } }`. Replaces the table whole, so an app whose
   * lengths change with a viewport tier declares again on
   * `resize`. A name a theme or metrics role owns is dropped
   * with a `reserved-token` warning. A colour may be a
   * recipe over an earlier one (ADR 0028): `{ from: 'peach',
   * ops: [['lift', 0.3]] }`, dropped with `unknown-token` when
   * its source is not there. Reference one in a prop
   * as `'$peach'` — `defineTokens` types the names. The raw
   * addon door; `index.js` wraps it to keep the encoder's map
   * in step, so call `setTokens` and not this.
   */
  setTokensRaw(tokens: { colors: [string, unknown][], lengths: [string, number][] }): void
  /**
   * The tokens a view here sees this frame, resolved: colours as
   * `0xRRGGBBAA` for the appearance in effect, lengths in px,
   * under `colors` and `lengths`. Roles are not listed — read
   * them off `theme()` and `metrics()`.
   */
  tokens(): ResolvedTokens
  /**
   * `frame` / `setView` report the `$name` references the
   * encoder could not resolve — nothing declared, or the other
   * kind — through here, as `unknown-token`, once per name.
   */
  warnUnknownTokens(names: [string, string][]): void
  /**
   * `measureText`'s door (index.js adds `measureText` itself):
   * one `<text>` element as `encoder.encodeText` writes it, and
   * the answer is what that text lays out to — `{width, height,
   * lines}` in logical px at the context's scale, capped to
   * `maxWidth` when given. The metrics do not scale linearly:
   * `measured × zoom` is not `measure(size × zoom)`, because
   * shaping rounds per size, so anything that zooms measures at
   * the size it draws.
   */
  measureTextBinary(stream: Float64Array, strings: Uint8Array, maxWidth?: number | undefined | null): TextMetrics
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
   * Every warning the core has raised so far, drained or not,
   * oldest first — the log `warnings()` leaves behind, for a
   * reader that is not the driver (a devtools stream).
   */
  warningsRaised(): Warning[]
  /**
   * Turns the per-frame node snapshot behind `nodes()` on or off
   * (off unless a devtool asked: the copy is O(nodes) a frame).
   */
  setInspect(on: boolean): void
  /**
   * Turns the core's devtools panel on or off
   * (`docs/adr/0024`): the event stream, the runtime's facts and
   * the tree, drawn by the core beside the app's own tree in the
   * main window — or where `setDevtoolsDock` says — with its
   * controls and its `Ctrl+Shift+<letter>` chords handled inside
   * the core, so nothing of it reaches `update`. `KUI_DEVTOOLS=1`
   * in the environment is the same call made by nobody, for a
   * `KuiWindow`; a headless `Ctx` never reads it.
   */
  setDevtools(on: boolean): void
  /** Whether the devtools panel is on. */
  devtools(): boolean
  /**
   * Where the devtools panel sits: `"left"`, `"right"`,
   * `"bottom"`, `"window"` (one of its own, named
   * `kui-devtools`) or `"off"` (hidden, the chords still live);
   * `"side"` is the right. Throws on any other word.
   */
  setDevtoolsDock(dock: DevtoolsDock): void
  /** Where the devtools panel sits (see `setDevtoolsDock`). */
  devtoolsDock(): DevtoolsDock
  /**
   * Seeds the panel's theme override, what its `T` and `A`
   * chords cycle from: `base` is `"light"`, `"dark"` or `null`
   * for the app's own; `accent` an `#rrggbb` string or `null`.
   */
  setDevtoolsTheme(base: 'light' | 'dark' | null, accent: string | null): void
  /**
   * Respells the chord that moves the keyboard into the panel
   * and back out — and brings a hidden panel back — from its
   * default `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod`
   * is Command on macOS, Control elsewhere), `"⌥⌘I"`, any
   * spelling a menu item's `accel` takes. The panel's other
   * chords stay `Ctrl+Shift+<letter>`. With another chord set,
   * `Ctrl+Shift+I` reaches the app like any other press. Throws
   * on a spelling kui cannot name.
   */
  setDevtoolsKey(key: string): void
  /**
   * The chord `setDevtoolsKey` set, or the default, in its
   * portable spelling: `"ctrl+shift+i"`, `"f12"`,
   * `"super+alt+d"`.
   */
  devtoolsKey(): string
  /**
   * The declared devtools tab on show, by name, or `null` for
   * one of the panel's own, the panel off or popped out (ADR
   * 0032). What `frame` / `setView` read once before encoding,
   * so a `<devtoolsTab>`'s function child is called only for
   * that tab.
   */
  devtoolsShownTab(): string | null
  /**
   * The node the panel's tree tab has selected, as a hex key,
   * or `null` (ADR 0032, decision 4) — what an inspector in a
   * declared tab reads to say which node it is about.
   */
  devtoolsSelected(): string | null
  /** The tree row under the pointer, as a hex key, or `null`. */
  devtoolsHovered(): string | null
  /** The node the picker is over while picking, or `null`. */
  devtoolsPicked(): string | null
  /**
   * Raises the panel's picker from outside it — an inspector in
   * a `<devtoolsTab>` asking "which node?" — or puts it away
   * (ADR 0032, decision 4). Picking happens over the app in
   * the main window: `devtoolsPicked()` is the node under the
   * pointer while it is up, and the press lands it in
   * `devtoolsSelected()`. Raised while a declared tab is on
   * show, the pick leaves that tab up; raised otherwise it is
   * the `Ctrl+Shift+P` pick and shows the tree tab. A hidden
   * panel comes back docked.
   */
  setDevtoolsPick(on: boolean): void
  /** Whether the panel's picker is up. */
  devtoolsPicking(): boolean
  /**
   * Shows the panel's tab named `name` from the app's side —
   * what the strip's click and `Ctrl+Shift+N` do, for a command
   * that jumps to the app's own tab (ADR 0032). `name` is one
   * of the panel's own (`facts`, `events`, `tree`) or a
   * `<devtoolsTab>`'s. A declared name the panel does not list
   * yet is kept and shows once a frame declares it; the return
   * says whether the panel lists it now. A hidden panel comes
   * back docked; `setDevtools(true)` is still the app's to
   * call. Call it once, not every frame: it would pin the strip
   * against the user's own clicks.
   */
  setDevtoolsTab(name: DevtoolsTab | (string & {})): boolean
  /**
   * The tab the panel is on, by name: one of its own or a
   * `<devtoolsTab>`'s — the selection itself, panel on or off,
   * unlike `devtoolsShownTab()`, which is the encoder's reading
   * of a declared tab on show.
   */
  devtoolsCurrentTab(): DevtoolsTab | (string & {})
  /**
   * Selects a node in the panel's tree tab from outside it and
   * reveals it there, as the picker does; `null` clears. `key`
   * is a hex key an event carried or `keyOf` answered.
   */
  setDevtoolsSelected(key?: string | undefined | null): void
  /**
   * The key legend the panel's facts tab shows: `[keys, what]`
   * pairs.
   */
  setDevtoolsLegend(legend: [string, string][]): void
  /**
   * The last finished frame's nodes in tree order, each with what
   * it is, the label it was opened under, where layout put it
   * (in the app's viewport, like `layoutOf`; backlog AR36), and
   * the declarations that explain the rest — what a tree view
   * and a node inspector are built from. Empty until
   * `setInspect(true)` and a frame after it.
   */
  nodes(): NodeInfo[]
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
   * A request from assistive technology on a node: an `AccessAction`
   * name the node advertises, with `value` the new text for
   * `setValue`. `key` is either spelling of the node — the hex key
   * an event carried (16 digits), or the label its `key` prop
   * declared, resolved through the last frame (see `focus`).
   * Resolved like its pointer/keyboard equivalent, so the resulting
   * events come out of `pollEvents`. A real screen reader's
   * requests arrive through a window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame. `key` is either spelling,
   * as for `focus`: the label a `key` prop declared, or the hex
   * key an event carried. For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  /**
   * Press state as of the last frame; `key` is either spelling,
   * as for `isHovered`.
   */
  isPressed(key: string): boolean
  /**
   * Whether files dragged in from the OS are over `key` (ADR
   * 0031) — for drop-dependent layout; the colour swap is the
   * `dropBg` prop. `key` is either spelling, as for `isHovered`.
   */
  isDropTarget(key: string): boolean
  /**
   * The `onDrop` zone the dragged files are over, as the hex key
   * an event carries, or null — what a driver answers the OS
   * with after each `dragFiles`, and what a test reads to say a
   * zone was found.
   */
  dropTarget(): string | null
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
   * `onKey` sink, a button Tab landed on (see `focused`). `key` is
   * a hex key or a declared label, as for `focus`.
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab walk every control in tree order, Enter and Space
   * press the focused one, and the arrows nudge a focused
   * slider — `press("tab")`, `press(" ")`, `press("right")`,
   * which is what a keyboard sends. (`key("tab")` is the half
   * of that press the core acts on, for a test that means to
   * drive one channel; `keyDown` is the other half, the one an
   * `onKey` sink hears.)
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * The caret's blink phase — `true` draws it (backlog C35). A
   * custom editor reads it in `view` and skips its caret node
   * on the off phase, keeping the `caret` row on its `line`
   * either way; the window's clock sets it while a focused
   * `<edit>` or such a line has a caret, and parks it hidden
   * while the window has no keyboard. Always `true` headless.
   */
  caretVisible(): boolean
  /**
   * Whether there is a caret to blink: a focused `<edit>`'s, or
   * the `caret` a `line` under the focused sink declares — unless
   * the line declares it `caretSolid` (backlog F68), which
   * anchors and reads but arms no clock. What the window's
   * clock is armed on; headless, what a test reads to see that
   * an idle view asks for no frame.
   */
  hasCaret(): boolean
  /**
   * The driver's half of the blink: sets the phase. A window
   * runs its own clock; headless, a test drives it to see the
   * off phase drawn.
   */
  setCaretVisible(visible: boolean): void
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   *
   * `key` is either spelling of the node: the 16-digit hex key an
   * event carried, or the label its `key` prop declared —
   * `focus('note')` — resolved through the last frame, so a node
   * the user has never touched can be named. Labels are unique
   * among siblings, not across the tree: when two nodes declare
   * the same one, the first in tree order wins and an
   * `ambiguous-key` warning says so. A label no node declared
   * throws.
   */
  focus(key: string): void
  /**
   * The hex key of the node a label names — the label a `key`
   * prop declared, resolved through the frame being built so
   * far and then the last finished one — or null when no node
   * declared it. The door for holding a key across frames;
   * every call that takes a key takes the label too, so this
   * is for caching one, or for checking that a name reached
   * the view. Two nodes on one label under different parents
   * resolve to the first in tree order and raise
   * `ambiguous-key` — among the host's own first, and a
   * plugin filling a slot is answered from its own nodes only.
   */
  keyOf(label: string): string | null
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
   * Enters a focus region — a box declared `focusRegion`, named by
   * the label its `key` prop declares or by the hex key an event
   * carried — or the main ring for `null`
   * (`docs/adr/0022-focus-regions.md`). Focus lands on what that
   * ring last held if the node is still there, else its
   * `initialFocus`, else its first stop, and shows.
   *
   * Resolved when the next frame finishes, like `focusNext`, so
   * the `update` that toggles a dock on may enter it in the same
   * turn — which is why a label is taken as a name to hold rather
   * than resolved now: the node need not exist yet. A frame that
   * then declares no `focusRegion` under the name raises
   * `focus-region-without-node` and moves nothing.
   */
  focusRegion(key?: string | undefined | null): void
  /**
   * The focus region in effect — the hex key of the `focusRegion`
   * node whose ring Tab walks — or `null` for the main ring. What
   * a chord that toggles between a dock and the app reads to know
   * which way it is going.
   */
  region(): string | null
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins. `key` is a hex key or a declared label, as for
   * `focus` — a label resolves through the *last* frame, so a row
   * the coming frame declares for the first time is reachable by
   * its hex key only.
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
   * The rect the last frame laid `key` out at, `{x, y, w, h}`
   * in logical viewport px, for a node that declared `onLayout`
   * — the `layout` event's numbers, read back during the next
   * build with no event and no model field (backlog C26 step
   * 2); `null` for any other key. Read while building, it
   * describes the previous frame, like `scrollGeometry`.
   */
  layoutOf(key: string): { x: number, y: number, w: number, h: number } | null
  /**
   * Where a point lands in the text a keyed node drew: a byte
   * offset into its text and the visual row within that node —
   * counted across every run the key covers, so a `line` row of
   * inline runs is one row and a wrapped run as many as it
   * wrapped to; not the ordinal `line` node a pointer event's
   * `line` names (backlog AR30) — or null for a key that drew no
   * text. A `role="none"` subtree under the key (a gutter) is
   * not its text, as the access tree reads it. `x`/`y` are the logical viewport px a
   * `click` or `drag` event carries, so a custom editor turns the
   * event into a caret position with one call — no prefix
   * measuring, no cell-width arithmetic. A node holding several
   * text runs (a `line` row of token runs) answers across them
   * in order, the way the access tree reads the line. Answered
   * from the frame that finished: the layout the pointer was
   * over.
   */
  textHit(key: string, x: number, y: number): TextHit | null
  /**
   * Opens a context menu at `(x, y)` over the node `key`, with
   * `items` as plain objects: `{label, role, enabled, id,
   * accel}`, all but `label` optional. `role` is one of
   * `custom` (the default), `separator`, `cut`, `copy`,
   * `paste`, `selectAll` or `lookUp`; the standard ones take
   * their own wording when `label` is empty, and the core
   * performs the ones it can (`docs/adr/0017-selection-as-a-scope.md`).
   *
   * Choosing a row posts `{kind:"menu", role, item}` on `key`
   * and closes the menu; a press outside it or Escape closes it
   * with nothing posted. What an app answering its own
   * `onContextMenu` calls — and what the core calls itself for
   * a right-click nobody claimed, so the two menus are one
   * implementation.
   */
  openMenu(key: string, x: number, y: number, items: MenuItemInput[]): boolean
  /**
   * Drains what choosing a menu row left for the host: the
   * clipboard, which is the host's in this library. Each entry
   * is `{kind}` — `"setClipboard"` with `text` (and `html`
   * where there is formatting to carry), `"paste"` asking for
   * what is on the clipboard (deliver it back with `commit()`,
   * which a focused editor takes as typing and a focused
   * `onKey` sink hears as `{kind:"text"}`),
   * or `"lookUp"` with the `text` to show a definition panel
   * for at `x`, `y`.
   *
   * A windowed app never needs this — the driver drains it —
   * but a headless one does: nothing else empties the queue,
   * and a Copy nobody drains is a copy that never happened.
   */
  takeMenuActions(): MenuAction[]
  /**
   * Puts `text` on the system clipboard — the action a menu's
   * Copy queues, with a door on it for an `onKey` sink that
   * hears the raw `Ctrl-c` and had nowhere to bind it (backlog
   * C33). `html` is a second flavour beside the text for the
   * host to offer, never in place of it. A window applies it
   * at its next drain (after every input and every frame); a
   * headless `Ctx` hands it out through `takeMenuActions()`.
   */
  setClipboard(text: string, html?: string | undefined | null): void
  /**
   * Asks for what is on the clipboard — the action a menu's
   * Paste queues. A window reads the clipboard and hands the
   * text back as a commit: a focused `<edit>` takes it as
   * typing, and a focused `onKey` sink hears it as
   * `{kind:"text", text, tag}`, so an app that owns its text
   * inserts a paste the way it inserts a committed IME string
   * and never reads the clipboard itself. Headless, the
   * request comes out of `takeMenuActions()` as `{kind:"paste"}`
   * and the test answers it with `commit(...)`.
   */
  requestPaste(): void
  /**
   * Whether a paste asked for is still unanswered: one ask at a
   * time — a second `requestPaste` while one is out is dropped,
   * and the `commit` that answers it (an empty one for an empty
   * clipboard) lets the next through (backlog AR34).
   */
  awaitingPaste(): boolean
  /**
   * The menu this window has open, or null:
   * `{target, x, y, items}`. What a host rendering menus itself
   * reads after `setNativeMenus(true)` — the core then keeps
   * the menu as state and draws none of it — and answers with
   * `activateMenuItem` or `closeMenu`. A row reads exactly as a
   * `menuBar()` row does (`menu_item_json`).
   */
  menu(): OpenMenu | null
  /**
   * Tells the core this host shows menus itself. It then keeps
   * the open menu as state and draws none of it: read it with
   * `menu()`, show it, and report back with `activateMenuItem`
   * or `closeMenu`. Off by default, which is the menu this
   * library draws.
   */
  setNativeMenus(on: boolean): void
  /**
   * The application menu the frame declared, or null:
   * `{revision, menus: [{label, enabled, items}]}`
   * (`docs/adr/0018-a-menu-bar-the-app-declares.md`). What a
   * host with a menu bar of its own reads after
   * `setNativeMenuBar(true)`; `revision` changes only when the
   * declaration does, so a host rebuilds nothing until it moves.
   */
  menuBar(): MenuBarState | null
  /**
   * Tells the core the platform owns the menu bar, so
   * `<menuBar/>` draws nothing and this host is the one handing
   * the declaration over (`menuBar()`) and reporting what was
   * chosen (`activateMenuBarItem`). Off by default, which is
   * the bar this library draws.
   */
  setNativeMenuBar(on: boolean): void
  /**
   * Reports that the platform's menu bar chose row `item` of
   * menu `menu` — the same path a press on the drawn bar's row
   * takes. False for a row that is not there.
   */
  activateMenuBarItem(menu: number, item: number): boolean
  /**
   * Tells the core this host can show the platform's definition
   * panel. The standard Look Up row is then offered where it
   * means something, and a force click over text asks for one.
   */
  setLookupAvailable(on: boolean): void
  /**
   * Reports that the host's own menu chose row `index` — the
   * same path a press on the drawn menu's row takes. An index
   * past the end closes the menu and posts nothing. False when
   * no menu was open.
   */
  activateMenuItem(index: number): boolean
  /** Closes whatever menu is open; true when there was one. */
  closeMenu(): boolean
  /**
   * Asks for the selection as text: `{ text, asked }`.
   *
   * `text` is the selection when the core has all of it. When
   * the selection reaches rows a virtual list never built,
   * `asked` is true instead and a `{kind:"selectionrange",
   * from:{index, byte}, to:{index, byte}}` event is posted on
   * the scope — the rows behind that gap are the app's, so the
   * app answers with `answerSelectionRange`, and the answer is
   * what reaches the clipboard
   * (`docs/adr/0017-selection-as-a-scope.md`).
   */
  requestCopy(): { text: string | null, asked: boolean }
  /**
   * Answers a `selectionrange` ask with the text for the range
   * it named, whole. False when nothing asked — a late answer
   * cannot overwrite what has been copied since.
   */
  answerSelectionRange(text: string): boolean
  /**
   * The window's selected text: what a `selectable` scope has
   * selected, or the focused `<edit>`'s selection, whichever
   * the window holds — starting either clears the other, so
   * there is never a choice to make. Null with no selection,
   * `""` when a selection exists but covers nothing (a press
   * that placed both ends together). See
   * `docs/adr/0017-selection-as-a-scope.md`.
   */
  selectionText(): string | null
  /**
   * The text selection's two ends as the drag made them:
   * `{anchor: {index, byte}, focus: {index, byte}}`, `index` the
   * data index of the virtualised row the end is in (null
   * outside every virtualised row — the `index` a
   * `selectionrange` ask would name) and `byte` the offset in
   * that row's own text. Directed, so a Shift-click that kept
   * the anchor reads as one (ADR 0029). Null with no text
   * selection; a grid's is `cellSelection()`.
   */
  selectionEnds(): SelectionEnds | null
  /**
   * A `cells` grid's selection, the window's when it lives in
   * one: the grid's key, `anchor` and `focus` as the drag made
   * them — each an absolute `line` (`originLine` plus the row,
   * so a scroll does not move it) and a `col` — and `block`
   * for a rectangular one (ADR 0017, decision 4). Null when the
   * window's selection is not a grid's; a text selection's ends
   * are `selectionEnds()`.
   */
  cellSelection(): CellSelection | null
  /**
   * The selection as HTML, carrying the formatting the text
   * declared — bold, italic, a span's own colour — and *not*
   * the node's colour, which is the app's theme rather than
   * the text's (`docs/adr/0017-selection-as-a-scope.md`).
   * Null with no text selection. Meant as a second clipboard
   * flavour beside the plain text, never instead of it.
   */
  selectionHtml(): string | null
  /**
   * Selects every run inside the selection scope a keyed node
   * declared (`selectable`), first byte to last — Select All,
   * scoped. False when that node drew no text, or is not a
   * scope. Text the frame built but never drew is included:
   * the selection is over the scope's text, not over what fits
   * on screen.
   */
  selectAllIn(key: string): boolean
  /**
   * Drops the window's selection, whichever it is. True when
   * there was one to drop.
   */
  clearSelection(): boolean
  /**
   * The caret rect for a byte offset in the text a keyed node
   * drew: logical viewport px, zero wide, one line tall — where
   * a caret, a selection edge or an IME candidate window goes.
   * A byte past the text is the end; null for a key that drew
   * no text.
   */
  caretRect(key: string, byte: number): Rect | null
  /**
   * Where the OS candidate window goes while a composition is
   * under way: the focused `<edit>`'s caret, or a custom editor's
   * `line` carrying `caret`; null when nothing with a caret is
   * focused. The windowed driver applies it itself; headless,
   * it is what a test reads to see the anchor moved.
   */
  imeRect(): Rect | null
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
  /**
   * Replaces an editor's text, leaving the caret at the end.
   *
   * It reaches an editor that does not exist yet: the `update`
   * that opens a rename field runs a frame ahead of the view
   * that declares it, so the text is held for the frame that
   * declares this name and seeds the editor there, over
   * `initial`. Name it by the label its `key` prop declares —
   * the spelling that needs nothing to exist yet, since the
   * hex key comes from an event the editor has not fired.
   * Either spelling works, as for `focus`; a label the last
   * frame declared lands at once, and one it did not is held.
   * Held for that one frame — a name nothing declares on it
   * drops its text with an `edit-text-without-editor` warning,
   * so a name the view spells differently is a line rather
   * than a field that opens with the wrong text.
   *
   * A redraw is asked for only when the text reached an editor.
   * A held seed changed nothing on screen, and the frame that
   * will — the view that declares the editor — is the app's:
   * a redraw here re-lowered the *retained* tree, which declares
   * no editor, and that was the frame the hold expired on when
   * the call came from a `dispatch` outside the loop (backlog
   * F42; `runWindowed` draws that model before it pumps).
   */
  setEditText(key: string, text: string): void
  /**
   * Registers a w×h RGBA image (pixels copied); returns its id for
   * `<image src={id}>`. Stable until `removeImage`.
   */
  addImage(width: number, height: number, rgba: Buffer): string
  removeImage(id: string): void
  /**
   * Replaces an image's pixels in place (copied): the id is
   * unchanged, so every `<image src={id}>` shows the new pixels
   * next frame with no view change; `width`/`height` may differ
   * from the registration. From the first update on the image
   * is drawn from a texture of its own — a video frame, a
   * camera, a plot the app rasterised itself
   * (`docs/adr/0025-the-image-is-the-canvas.md`). A dead id warns
   * `foreign-resource` and changes nothing.
   */
  updateImage(id: string, width: number, height: number, rgba: Buffer): void
  /**
   * Registers a WGSL fragment function; returns its id for
   * `<fragment src={id}>`. Throws when the source does not
   * compile, with the compiler's message in the app's own line
   * numbers. Idempotent by source, so the same text gets the
   * same id without being validated twice.
   */
  addFragment(wgsl: string): string
  removeFragment(id: string): void
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
  /**
   * What the last frame left owed, by kind — `animating()`
   * taken apart: `{transition, cycle, depart, requested,
   * autoscroll}`. To the window they are one, and it redraws
   * for any of them; to a test they differ, since a keyframe
   * `repeat` cycle never ends and `settled()` never resolves
   * under one. `quiet()` on the loop waits on everything but
   * `cycle` (backlog F64).
   */
  owed(): Owed
  /**
   * Byte budget for the shaped-text cache: every text a frame
   * draws is shaped once and kept, and past this many
   * (estimated) bytes the least recently drawn entries go at
   * the start of the next frame — never what the last frame
   * drew. Default 64 MB; a terminal streaming new lines lowers
   * it, a viewer that wants every page it showed kept warm
   * raises it.
   */
  setTextCacheBudget(bytes: number): void
  /**
   * What the shaped-text cache holds, in the estimated bytes
   * the budget is charged against.
   */
  textCacheBytes(): number
  /** Summary of the last frame's display list. */
  stats(): FrameStats
  /**
   * Raw quads for the finished frame, `quadStride()` bytes each,
   * laid out as kui-ffi's KuiQuad (see include/kui.h) and decoded
   * by `decodeQuads`. Copied into the Buffer. A window answers
   * with what its last pump drew, so a smoke test can read the
   * frame the shipping driver painted and not only a headless
   * one's (backlog F19). Drive that window with `access(key,
   * action)` — `click`, `type` and `key` are refused there,
   * because the OS is what drives a real window.
   */
  quads(): Buffer
  /**
   * The clips this frame's quads name through their `clip`
   * index, `clipStride()` bytes each, laid out as kui-ffi's
   * KuiClip (see include/kui.h) and decoded by `decodeClips`.
   * Entry zero clips nothing, so a quad always has one; the
   * list is empty only on a frame that drew nothing.
   */
  clips(): Buffer
  /**
   * This frame's fragment draws, in the order their quads index
   * them by `uv[0]`: twenty-four doubles each — the handle as
   * two 32-bit halves, the sixteen parameters, then where the
   * draw's `image` is (0 none, 1 the atlas, 2 a texture of its
   * own), the `textureDraws` index when it is 2, and the texel
   * rect `x, y, w, h` (backlog V1). The parameters ride a side
   * list rather than the quad, so `quads()` alone cannot show
   * them and a corpus adapter needs this to compare them
   * (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
   * Empty on a frame that draws no fragment.
   */
  fragmentDraws(): Array<number>
  /**
   * This frame's texture draws, in the order their quads index
   * them by `uv[0]`: nine doubles each — the image handle as two
   * 32-bit halves, the pixels' revision, width and height, and
   * the texel rect `x, y, w, h` in the image's own texels (the
   * whole image, or the crop a `fit="cover"` made). The side
   * list a `quads()` texture quad points at
   * (`docs/adr/0025-the-image-is-the-canvas.md`, decision 3).
   * Empty on a frame that draws no texture-backed image.
   */
  textureDraws(): Array<number>
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
   * The palette this window paints with: one `0xRRGGBBAA` number
   * per role, derived from `env().system` unless this app said
   * otherwise (`setAccent` / `setTheme`). A view reads it and
   * paints with it — `<box bg={theme.surface}>` — and the stock
   * widgets already do, so a JSX app that only uses `<button>`,
   * the context menu and `<text>` follows the OS's light and
   * dark without reading this at all.
   */
  theme(): Theme
  /**
   * Keep following the OS's light/dark, but paint this accent
   * instead of the OS's — an app with a brand colour. A
   * `0xRRGGBBAA` number or a `"#hex"` string, as any colour
   * prop takes; `null` goes back to the OS's own.
   */
  setAccent(accent: number | string | null): void
  /**
   * Pin the palette: an object of role overrides on top of the
   * base named by `appearance` (`"light"`, `"dark"`, or absent
   * for the OS's), each value a colour the way a prop takes
   * one. Follows nothing afterwards — `setAccent(null)` is how
   * an app goes back to following the OS.
   *
   * `{ appearance: "dark", accent: "#d2691e" }` is a dark app
   * with one colour changed; every role the object does not
   * name keeps the base's, and the ones derived from the accent
   * (its hover, its pressed shade, the label on it, the ring,
   * the selection tint) are recomputed unless named too.
   */
  setTheme(theme: ThemeOverrides): void
  /**
   * The sizes the stock widgets are built from — the palette's
   * other axis: one number per metric, logical px before the
   * scale factor. Read it so a control of your own agrees with
   * `<button>` on a radius and a padding: `<box radius={metrics.radius}>`.
   */
  metrics(): Metrics
  /**
   * Makes these the frame's metrics: overrides on top of the
   * set in effect, or on top of `base: "comfortable"` (the
   * stock set) / `"compact"` (a dense tool's), then `scale`
   * multiplying every length — a density slider. `null`
   * restores the stock set. Density is the app's to choose;
   * nothing in the OS is followed.
   */
  setMetrics(metrics: MetricsOverrides | null): void
  /**
   * Declare the app's named colours and lengths
   * (`docs/adr/0027-tokens-beside-the-theme.md`): `{ colors:
   * { peach: '#ffcc99', ink: { light, dark } }, lengths: {
   * sideW: 132 } }`. Replaces the table whole, so an app whose
   * lengths change with a viewport tier declares again on
   * `resize`. A name a theme or metrics role owns is dropped
   * with a `reserved-token` warning. A colour may be a
   * recipe over an earlier one (ADR 0028): `{ from: 'peach',
   * ops: [['lift', 0.3]] }`, dropped with `unknown-token` when
   * its source is not there. Reference one in a prop
   * as `'$peach'` — `defineTokens` types the names. The raw
   * addon door; `index.js` wraps it to keep the encoder's map
   * in step, so call `setTokens` and not this.
   */
  setTokensRaw(tokens: { colors: [string, unknown][], lengths: [string, number][] }): void
  /**
   * The tokens a view here sees this frame, resolved: colours as
   * `0xRRGGBBAA` for the appearance in effect, lengths in px,
   * under `colors` and `lengths`. Roles are not listed — read
   * them off `theme()` and `metrics()`.
   */
  tokens(): ResolvedTokens
  /**
   * `frame` / `setView` report the `$name` references the
   * encoder could not resolve — nothing declared, or the other
   * kind — through here, as `unknown-token`, once per name.
   */
  warnUnknownTokens(names: [string, string][]): void
  /**
   * `measureText`'s door (index.js adds `measureText` itself):
   * one `<text>` element as `encoder.encodeText` writes it, and
   * the answer is what that text lays out to — `{width, height,
   * lines}` in logical px at the context's scale, capped to
   * `maxWidth` when given. The metrics do not scale linearly:
   * `measured × zoom` is not `measure(size × zoom)`, because
   * shaping rounds per size, so anything that zooms measures at
   * the size it draws.
   */
  measureTextBinary(stream: Float64Array, strings: Uint8Array, maxWidth?: number | undefined | null): TextMetrics
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
   * Every warning the core has raised so far, drained or not,
   * oldest first — the log `warnings()` leaves behind, for a
   * reader that is not the driver (a devtools stream).
   */
  warningsRaised(): Warning[]
  /**
   * Turns the per-frame node snapshot behind `nodes()` on or off
   * (off unless a devtool asked: the copy is O(nodes) a frame).
   */
  setInspect(on: boolean): void
  /**
   * Turns the core's devtools panel on or off
   * (`docs/adr/0024`): the event stream, the runtime's facts and
   * the tree, drawn by the core beside the app's own tree in the
   * main window — or where `setDevtoolsDock` says — with its
   * controls and its `Ctrl+Shift+<letter>` chords handled inside
   * the core, so nothing of it reaches `update`. `KUI_DEVTOOLS=1`
   * in the environment is the same call made by nobody, for a
   * `KuiWindow`; a headless `Ctx` never reads it.
   */
  setDevtools(on: boolean): void
  /** Whether the devtools panel is on. */
  devtools(): boolean
  /**
   * Where the devtools panel sits: `"left"`, `"right"`,
   * `"bottom"`, `"window"` (one of its own, named
   * `kui-devtools`) or `"off"` (hidden, the chords still live);
   * `"side"` is the right. Throws on any other word.
   */
  setDevtoolsDock(dock: DevtoolsDock): void
  /** Where the devtools panel sits (see `setDevtoolsDock`). */
  devtoolsDock(): DevtoolsDock
  /**
   * Seeds the panel's theme override, what its `T` and `A`
   * chords cycle from: `base` is `"light"`, `"dark"` or `null`
   * for the app's own; `accent` an `#rrggbb` string or `null`.
   */
  setDevtoolsTheme(base: 'light' | 'dark' | null, accent: string | null): void
  /**
   * Respells the chord that moves the keyboard into the panel
   * and back out — and brings a hidden panel back — from its
   * default `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod`
   * is Command on macOS, Control elsewhere), `"⌥⌘I"`, any
   * spelling a menu item's `accel` takes. The panel's other
   * chords stay `Ctrl+Shift+<letter>`. With another chord set,
   * `Ctrl+Shift+I` reaches the app like any other press. Throws
   * on a spelling kui cannot name.
   */
  setDevtoolsKey(key: string): void
  /**
   * The chord `setDevtoolsKey` set, or the default, in its
   * portable spelling: `"ctrl+shift+i"`, `"f12"`,
   * `"super+alt+d"`.
   */
  devtoolsKey(): string
  /**
   * The declared devtools tab on show, by name, or `null` for
   * one of the panel's own, the panel off or popped out (ADR
   * 0032). What `frame` / `setView` read once before encoding,
   * so a `<devtoolsTab>`'s function child is called only for
   * that tab.
   */
  devtoolsShownTab(): string | null
  /**
   * The node the panel's tree tab has selected, as a hex key,
   * or `null` (ADR 0032, decision 4) — what an inspector in a
   * declared tab reads to say which node it is about.
   */
  devtoolsSelected(): string | null
  /** The tree row under the pointer, as a hex key, or `null`. */
  devtoolsHovered(): string | null
  /** The node the picker is over while picking, or `null`. */
  devtoolsPicked(): string | null
  /**
   * Raises the panel's picker from outside it — an inspector in
   * a `<devtoolsTab>` asking "which node?" — or puts it away
   * (ADR 0032, decision 4). Picking happens over the app in
   * the main window: `devtoolsPicked()` is the node under the
   * pointer while it is up, and the press lands it in
   * `devtoolsSelected()`. Raised while a declared tab is on
   * show, the pick leaves that tab up; raised otherwise it is
   * the `Ctrl+Shift+P` pick and shows the tree tab. A hidden
   * panel comes back docked.
   */
  setDevtoolsPick(on: boolean): void
  /** Whether the panel's picker is up. */
  devtoolsPicking(): boolean
  /**
   * Shows the panel's tab named `name` from the app's side —
   * what the strip's click and `Ctrl+Shift+N` do, for a command
   * that jumps to the app's own tab (ADR 0032). `name` is one
   * of the panel's own (`facts`, `events`, `tree`) or a
   * `<devtoolsTab>`'s. A declared name the panel does not list
   * yet is kept and shows once a frame declares it; the return
   * says whether the panel lists it now. A hidden panel comes
   * back docked; `setDevtools(true)` is still the app's to
   * call. Call it once, not every frame: it would pin the strip
   * against the user's own clicks.
   */
  setDevtoolsTab(name: DevtoolsTab | (string & {})): boolean
  /**
   * The tab the panel is on, by name: one of its own or a
   * `<devtoolsTab>`'s — the selection itself, panel on or off,
   * unlike `devtoolsShownTab()`, which is the encoder's reading
   * of a declared tab on show.
   */
  devtoolsCurrentTab(): DevtoolsTab | (string & {})
  /**
   * Selects a node in the panel's tree tab from outside it and
   * reveals it there, as the picker does; `null` clears. `key`
   * is a hex key an event carried or `keyOf` answered.
   */
  setDevtoolsSelected(key?: string | undefined | null): void
  /**
   * The key legend the panel's facts tab shows: `[keys, what]`
   * pairs.
   */
  setDevtoolsLegend(legend: [string, string][]): void
  /**
   * The last finished frame's nodes in tree order, each with what
   * it is, the label it was opened under, where layout put it
   * (in the app's viewport, like `layoutOf`; backlog AR36), and
   * the declarations that explain the rest — what a tree view
   * and a node inspector are built from. Empty until
   * `setInspect(true)` and a frame after it.
   */
  nodes(): NodeInfo[]
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
   * A request from assistive technology on a node: an `AccessAction`
   * name the node advertises, with `value` the new text for
   * `setValue`. `key` is either spelling of the node — the hex key
   * an event carried (16 digits), or the label its `key` prop
   * declared, resolved through the last frame (see `focus`).
   * Resolved like its pointer/keyboard equivalent, so the resulting
   * events come out of `pollEvents`. A real screen reader's
   * requests arrive through a window on their own.
   */
  access(key: string, action: AccessAction, value?: string | AccessArg): void
  /**
   * Hover state as of the last frame. `key` is either spelling,
   * as for `focus`: the label a `key` prop declared, or the hex
   * key an event carried. For plain hover styling prefer the
   * `hoverBg` / `pressedBg` props — the core resolves those without
   * a round trip.
   */
  isHovered(key: string): boolean
  /**
   * Press state as of the last frame; `key` is either spelling,
   * as for `isHovered`.
   */
  isPressed(key: string): boolean
  /**
   * Whether files dragged in from the OS are over `key` (ADR
   * 0031) — for drop-dependent layout; the colour swap is the
   * `dropBg` prop. `key` is either spelling, as for `isHovered`.
   */
  isDropTarget(key: string): boolean
  /**
   * The `onDrop` zone the dragged files are over, as the hex key
   * an event carries, or null — what a driver answers the OS
   * with after each `dragFiles`, and what a test reads to say a
   * zone was found.
   */
  dropTarget(): string | null
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
   * `onKey` sink, a button Tab landed on (see `focused`). `key` is
   * a hex key or a declared label, as for `focus`.
   */
  isFocused(key: string): boolean
  /**
   * The node holding keyboard focus (hex key), or null. Tab /
   * Shift-Tab walk every control in tree order, Enter and Space
   * press the focused one, and the arrows nudge a focused
   * slider — `press("tab")`, `press(" ")`, `press("right")`,
   * which is what a keyboard sends. (`key("tab")` is the half
   * of that press the core acts on, for a test that means to
   * drive one channel; `keyDown` is the other half, the one an
   * `onKey` sink hears.)
   */
  focused(): string | null
  /**
   * Whether focus got where it is by keyboard or assistive
   * technology rather than a click — when it shows (the ring, or
   * `focusBg`).
   */
  focusVisible(): boolean
  /**
   * The caret's blink phase — `true` draws it (backlog C35). A
   * custom editor reads it in `view` and skips its caret node
   * on the off phase, keeping the `caret` row on its `line`
   * either way; the window's clock sets it while a focused
   * `<edit>` or such a line has a caret, and parks it hidden
   * while the window has no keyboard. Always `true` headless.
   */
  caretVisible(): boolean
  /**
   * Whether there is a caret to blink: a focused `<edit>`'s, or
   * the `caret` a `line` under the focused sink declares — unless
   * the line declares it `caretSolid` (backlog F68), which
   * anchors and reads but arms no clock. What the window's
   * clock is armed on; headless, what a test reads to see that
   * an idle view asks for no frame.
   */
  hasCaret(): boolean
  /**
   * The driver's half of the blink: sets the phase. A window
   * runs its own clock; headless, a test drives it to see the
   * off phase drawn.
   */
  setCaretVisible(visible: boolean): void
  /**
   * Moves keyboard focus to a node now (an editor, an `onKey` sink,
   * a control, a `focusable` box); `keyFocus` on a box is the
   * declarative, edge-triggered form.
   *
   * `key` is either spelling of the node: the 16-digit hex key an
   * event carried, or the label its `key` prop declared —
   * `focus('note')` — resolved through the last frame, so a node
   * the user has never touched can be named. Labels are unique
   * among siblings, not across the tree: when two nodes declare
   * the same one, the first in tree order wins and an
   * `ambiguous-key` warning says so. A label no node declared
   * throws.
   */
  focus(key: string): void
  /**
   * The hex key of the node a label names — the label a `key`
   * prop declared, resolved through the frame being built so
   * far and then the last finished one — or null when no node
   * declared it. The door for holding a key across frames;
   * every call that takes a key takes the label too, so this
   * is for caching one, or for checking that a name reached
   * the view. Two nodes on one label under different parents
   * resolve to the first in tree order and raise
   * `ambiguous-key` — among the host's own first, and a
   * plugin filling a slot is answered from its own nodes only.
   */
  keyOf(label: string): string | null
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
   * Enters a focus region — a box declared `focusRegion`, named by
   * the label its `key` prop declares or by the hex key an event
   * carried — or the main ring for `null`
   * (`docs/adr/0022-focus-regions.md`). Focus lands on what that
   * ring last held if the node is still there, else its
   * `initialFocus`, else its first stop, and shows.
   *
   * Resolved when the next frame finishes, like `focusNext`, so
   * the `update` that toggles a dock on may enter it in the same
   * turn — which is why a label is taken as a name to hold rather
   * than resolved now: the node need not exist yet. A frame that
   * then declares no `focusRegion` under the name raises
   * `focus-region-without-node` and moves nothing.
   */
  focusRegion(key?: string | undefined | null): void
  /**
   * The focus region in effect — the hex key of the `focusRegion`
   * node whose ring Tab walks — or `null` for the main ring. What
   * a chord that toggles between a dock and the app reads to know
   * which way it is going.
   */
  region(): string | null
  /**
   * Scrolls whatever contains a node so it shows — "scroll to the
   * selected row", which needs the container geometry only the core
   * has. The request resolves against the *next* frame's layout (one
   * is requested), so a row the view is about to declare for the
   * first time reveals fine. If that frame does not declare the key,
   * or nothing above it scrolls, it is a no-op and is not kept for a
   * later frame; two reveals before one frame are contradictory, so
   * the last wins. `key` is a hex key or a declared label, as for
   * `focus` — a label resolves through the *last* frame, so a row
   * the coming frame declares for the first time is reachable by
   * its hex key only.
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
   * The rect the last frame laid `key` out at, `{x, y, w, h}`
   * in logical viewport px, for a node that declared `onLayout`
   * — the `layout` event's numbers, read back during the next
   * build with no event and no model field (backlog C26 step
   * 2); `null` for any other key. Read while building, it
   * describes the previous frame, like `scrollGeometry`.
   */
  layoutOf(key: string): { x: number, y: number, w: number, h: number } | null
  /**
   * Where a point lands in the text a keyed node drew: a byte
   * offset into its text and the visual row within that node —
   * counted across every run the key covers, so a `line` row of
   * inline runs is one row and a wrapped run as many as it
   * wrapped to; not the ordinal `line` node a pointer event's
   * `line` names (backlog AR30) — or null for a key that drew no
   * text. A `role="none"` subtree under the key (a gutter) is
   * not its text, as the access tree reads it. `x`/`y` are the logical viewport px a
   * `click` or `drag` event carries, so a custom editor turns the
   * event into a caret position with one call — no prefix
   * measuring, no cell-width arithmetic. A node holding several
   * text runs (a `line` row of token runs) answers across them
   * in order, the way the access tree reads the line. Answered
   * from the frame that finished: the layout the pointer was
   * over.
   */
  textHit(key: string, x: number, y: number): TextHit | null
  /**
   * Opens a context menu at `(x, y)` over the node `key`, with
   * `items` as plain objects: `{label, role, enabled, id,
   * accel}`, all but `label` optional. `role` is one of
   * `custom` (the default), `separator`, `cut`, `copy`,
   * `paste`, `selectAll` or `lookUp`; the standard ones take
   * their own wording when `label` is empty, and the core
   * performs the ones it can (`docs/adr/0017-selection-as-a-scope.md`).
   *
   * Choosing a row posts `{kind:"menu", role, item}` on `key`
   * and closes the menu; a press outside it or Escape closes it
   * with nothing posted. What an app answering its own
   * `onContextMenu` calls — and what the core calls itself for
   * a right-click nobody claimed, so the two menus are one
   * implementation.
   */
  openMenu(key: string, x: number, y: number, items: MenuItemInput[]): boolean
  /**
   * Drains what choosing a menu row left for the host: the
   * clipboard, which is the host's in this library. Each entry
   * is `{kind}` — `"setClipboard"` with `text` (and `html`
   * where there is formatting to carry), `"paste"` asking for
   * what is on the clipboard (deliver it back with `commit()`,
   * which a focused editor takes as typing and a focused
   * `onKey` sink hears as `{kind:"text"}`),
   * or `"lookUp"` with the `text` to show a definition panel
   * for at `x`, `y`.
   *
   * A windowed app never needs this — the driver drains it —
   * but a headless one does: nothing else empties the queue,
   * and a Copy nobody drains is a copy that never happened.
   */
  takeMenuActions(): MenuAction[]
  /**
   * Puts `text` on the system clipboard — the action a menu's
   * Copy queues, with a door on it for an `onKey` sink that
   * hears the raw `Ctrl-c` and had nowhere to bind it (backlog
   * C33). `html` is a second flavour beside the text for the
   * host to offer, never in place of it. A window applies it
   * at its next drain (after every input and every frame); a
   * headless `Ctx` hands it out through `takeMenuActions()`.
   */
  setClipboard(text: string, html?: string | undefined | null): void
  /**
   * Asks for what is on the clipboard — the action a menu's
   * Paste queues. A window reads the clipboard and hands the
   * text back as a commit: a focused `<edit>` takes it as
   * typing, and a focused `onKey` sink hears it as
   * `{kind:"text", text, tag}`, so an app that owns its text
   * inserts a paste the way it inserts a committed IME string
   * and never reads the clipboard itself. Headless, the
   * request comes out of `takeMenuActions()` as `{kind:"paste"}`
   * and the test answers it with `commit(...)`.
   */
  requestPaste(): void
  /**
   * Whether a paste asked for is still unanswered: one ask at a
   * time — a second `requestPaste` while one is out is dropped,
   * and the `commit` that answers it (an empty one for an empty
   * clipboard) lets the next through (backlog AR34).
   */
  awaitingPaste(): boolean
  /**
   * The menu this window has open, or null:
   * `{target, x, y, items}`. What a host rendering menus itself
   * reads after `setNativeMenus(true)` — the core then keeps
   * the menu as state and draws none of it — and answers with
   * `activateMenuItem` or `closeMenu`. A row reads exactly as a
   * `menuBar()` row does (`menu_item_json`).
   */
  menu(): OpenMenu | null
  /**
   * Tells the core this host shows menus itself. It then keeps
   * the open menu as state and draws none of it: read it with
   * `menu()`, show it, and report back with `activateMenuItem`
   * or `closeMenu`. Off by default, which is the menu this
   * library draws.
   */
  setNativeMenus(on: boolean): void
  /**
   * The application menu the frame declared, or null:
   * `{revision, menus: [{label, enabled, items}]}`
   * (`docs/adr/0018-a-menu-bar-the-app-declares.md`). What a
   * host with a menu bar of its own reads after
   * `setNativeMenuBar(true)`; `revision` changes only when the
   * declaration does, so a host rebuilds nothing until it moves.
   */
  menuBar(): MenuBarState | null
  /**
   * Tells the core the platform owns the menu bar, so
   * `<menuBar/>` draws nothing and this host is the one handing
   * the declaration over (`menuBar()`) and reporting what was
   * chosen (`activateMenuBarItem`). Off by default, which is
   * the bar this library draws.
   */
  setNativeMenuBar(on: boolean): void
  /**
   * Reports that the platform's menu bar chose row `item` of
   * menu `menu` — the same path a press on the drawn bar's row
   * takes. False for a row that is not there.
   */
  activateMenuBarItem(menu: number, item: number): boolean
  /**
   * Tells the core this host can show the platform's definition
   * panel. The standard Look Up row is then offered where it
   * means something, and a force click over text asks for one.
   */
  setLookupAvailable(on: boolean): void
  /**
   * Reports that the host's own menu chose row `index` — the
   * same path a press on the drawn menu's row takes. An index
   * past the end closes the menu and posts nothing. False when
   * no menu was open.
   */
  activateMenuItem(index: number): boolean
  /** Closes whatever menu is open; true when there was one. */
  closeMenu(): boolean
  /**
   * Asks for the selection as text: `{ text, asked }`.
   *
   * `text` is the selection when the core has all of it. When
   * the selection reaches rows a virtual list never built,
   * `asked` is true instead and a `{kind:"selectionrange",
   * from:{index, byte}, to:{index, byte}}` event is posted on
   * the scope — the rows behind that gap are the app's, so the
   * app answers with `answerSelectionRange`, and the answer is
   * what reaches the clipboard
   * (`docs/adr/0017-selection-as-a-scope.md`).
   */
  requestCopy(): { text: string | null, asked: boolean }
  /**
   * Answers a `selectionrange` ask with the text for the range
   * it named, whole. False when nothing asked — a late answer
   * cannot overwrite what has been copied since.
   */
  answerSelectionRange(text: string): boolean
  /**
   * The window's selected text: what a `selectable` scope has
   * selected, or the focused `<edit>`'s selection, whichever
   * the window holds — starting either clears the other, so
   * there is never a choice to make. Null with no selection,
   * `""` when a selection exists but covers nothing (a press
   * that placed both ends together). See
   * `docs/adr/0017-selection-as-a-scope.md`.
   */
  selectionText(): string | null
  /**
   * The text selection's two ends as the drag made them:
   * `{anchor: {index, byte}, focus: {index, byte}}`, `index` the
   * data index of the virtualised row the end is in (null
   * outside every virtualised row — the `index` a
   * `selectionrange` ask would name) and `byte` the offset in
   * that row's own text. Directed, so a Shift-click that kept
   * the anchor reads as one (ADR 0029). Null with no text
   * selection; a grid's is `cellSelection()`.
   */
  selectionEnds(): SelectionEnds | null
  /**
   * A `cells` grid's selection, the window's when it lives in
   * one: the grid's key, `anchor` and `focus` as the drag made
   * them — each an absolute `line` (`originLine` plus the row,
   * so a scroll does not move it) and a `col` — and `block`
   * for a rectangular one (ADR 0017, decision 4). Null when the
   * window's selection is not a grid's; a text selection's ends
   * are `selectionEnds()`.
   */
  cellSelection(): CellSelection | null
  /**
   * The selection as HTML, carrying the formatting the text
   * declared — bold, italic, a span's own colour — and *not*
   * the node's colour, which is the app's theme rather than
   * the text's (`docs/adr/0017-selection-as-a-scope.md`).
   * Null with no text selection. Meant as a second clipboard
   * flavour beside the plain text, never instead of it.
   */
  selectionHtml(): string | null
  /**
   * Selects every run inside the selection scope a keyed node
   * declared (`selectable`), first byte to last — Select All,
   * scoped. False when that node drew no text, or is not a
   * scope. Text the frame built but never drew is included:
   * the selection is over the scope's text, not over what fits
   * on screen.
   */
  selectAllIn(key: string): boolean
  /**
   * Drops the window's selection, whichever it is. True when
   * there was one to drop.
   */
  clearSelection(): boolean
  /**
   * The caret rect for a byte offset in the text a keyed node
   * drew: logical viewport px, zero wide, one line tall — where
   * a caret, a selection edge or an IME candidate window goes.
   * A byte past the text is the end; null for a key that drew
   * no text.
   */
  caretRect(key: string, byte: number): Rect | null
  /**
   * Where the OS candidate window goes while a composition is
   * under way: the focused `<edit>`'s caret, or a custom editor's
   * `line` carrying `caret`; null when nothing with a caret is
   * focused. The windowed driver applies it itself; headless,
   * it is what a test reads to see the anchor moved.
   */
  imeRect(): Rect | null
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
  /**
   * Replaces an editor's text, leaving the caret at the end.
   *
   * It reaches an editor that does not exist yet: the `update`
   * that opens a rename field runs a frame ahead of the view
   * that declares it, so the text is held for the frame that
   * declares this name and seeds the editor there, over
   * `initial`. Name it by the label its `key` prop declares —
   * the spelling that needs nothing to exist yet, since the
   * hex key comes from an event the editor has not fired.
   * Either spelling works, as for `focus`; a label the last
   * frame declared lands at once, and one it did not is held.
   * Held for that one frame — a name nothing declares on it
   * drops its text with an `edit-text-without-editor` warning,
   * so a name the view spells differently is a line rather
   * than a field that opens with the wrong text.
   *
   * A redraw is asked for only when the text reached an editor.
   * A held seed changed nothing on screen, and the frame that
   * will — the view that declares the editor — is the app's:
   * a redraw here re-lowered the *retained* tree, which declares
   * no editor, and that was the frame the hold expired on when
   * the call came from a `dispatch` outside the loop (backlog
   * F42; `runWindowed` draws that model before it pumps).
   */
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
  /** Declare the app's named colours and lengths (ADR 0027): `{ colors:
   *  { peach: '#ffcc99', ink: { light, dark } }, lengths: { sideW: 132 } }`
   *  — the object `defineTokens` typed. Replaces the table whole, so an
   *  app whose lengths change with a viewport tier declares again on
   *  `resize`. Reference one in any colour or length prop as `T.peach`;
   *  a name nothing declared raises `unknown-token` and the slot keeps
   *  its default. */
  setTokens(tokens: TokenDeclaration): void;
  /** What `content` measures under `style` as one `<text>` would lay out —
   *  logical px, and the line count — capped to `maxWidth` when given. The
   *  text crosses encoded like a frame's, so measuring and drawing shape
   *  the same runs. A headless context answers at scale 1 before its first
   *  frame; a window at its own scale once a frame has run. */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics;
}

export interface KuiWindow {
  /** Stores the tree future redraws lower, and schedules one. Encodes it to
   *  the binary IR stream first. */
  /** Shows `tree` in one window: `'main'` when `window` is left out, else
   *  a name `windows()` lists. */
  setView(tree: KuiNode, window?: string): void;
  /** See `Ctx.measureText`. */
  measureText(content: KuiNode, style?: TextProps, maxWidth?: number): TextMetrics;
  /** See `Ctx.setTokens`. */
  setTokens(tokens: TokenDeclaration): void;
}

/** What both drivers take. `S` is the surface the loop drives, and the
 *  fourth argument `update` gets: the headless `Ctx` under `createApp`, the
 *  `KuiWindow` under `runWindowed`. `M` is the model, `A` every message
 *  `update` can see. */
/** What `update` (or a function `init`) returns to hand the loop effects
 *  beside the model — built by `withEffects`, never by hand: the brand is
 *  what tells it from a model that happens to be an array or to have a
 *  `model` key. `docs/adr/0013-effects-as-data.md`. */
export interface WithEffects<M, E> {
  readonly [EFFECTS]: true;
  /** The next model, or `undefined` to keep the current one. */
  model: M | undefined;
  effects: E[];
}
declare const EFFECTS: unique symbol;

/** `withEffects(model, ...effects)`: the next model — or `undefined` to
 *  keep the current one, as a bare `undefined` return does — and the
 *  effects `update` wants performed. An effect is the app's own value (`E`
 *  on `createApp` / `runWindowed`); kui defines no vocabulary for them,
 *  and its own effects stay where they are — a sound is `surface.play` or
 *  an `<audio>` node, a window is `windows`. The loop sets the model,
 *  queues the effects, and after the next frame hands each to the
 *  `effects` handler; headless, `app.effects()` drains them for a test
 *  whether or not a handler ran. */
export declare function withEffects<M, E>(model: M | undefined, ...effects: E[]): WithEffects<M, E>;

/** The one place an app's effects are performed: `createApp` /
 *  `runWindowed`'s `effects` option. Runs after the frame that followed
 *  the `update` which returned the effect, with `dispatch` for whatever
 *  the effect has to say back (a "done", a "failed", a result) — which
 *  goes through `update` on the next turn, not inside this one — and the
 *  surface, which shows the frame the effect's cause produced. A
 *  promise-shaped effect is the handler's business. */
export type EffectHandler<E, A, S> = (effect: E, dispatch: (msg: A) => void, surface: S) => void;

export interface LoopConfig<M, A, S, E = never> {
  /** The first model, or a function that builds it. The function form is
   *  handed the surface — after `setup` has run, so the fonts and images it
   *  registered are there to measure against — which is how a first model
   *  gets the real `size()` and its own `measureText` numbers instead of
   *  constants it corrects on the first `resize`. It may return
   *  `withEffects` too, for the effect an app starts with. */
  init: M | ((surface: S) => M | WithEffects<M, E>);
  /** Returns the next model; returning undefined keeps the current one.
   *  `withEffects(model, ...effects)` returns it with the effects the loop
   *  should perform after the frame (see `EffectHandler`). `surface` is the
   *  thing being driven — for `editText`, `focus`, `play`,
   *  `scrollGeometry` and the rest. */
  update: (model: M, msg: A, event: UiEvent<A>, surface: S) => M | WithEffects<M, E> | undefined | void;
  /** The tree for one window. Called once per open window per frame with
   *  its name — `'main'` for the one the app starts in, else a name
   *  `windows` declared — so a single-window app ignores the argument. The
   *  surface comes third, for the measurement a tree needs while it is being
   *  built: `surface.measureText(...)` to size a column to its widest label,
   *  `size()` to pick the tier that fits. Measure, do not mutate — a view
   *  runs every frame. */
  view: (model: M, window: string, surface: S) => KuiNode;
  /** Which windows exist besides `main`, by name (see `WindowDecl`). A
   *  window opens on the first frame that lists it and closes on the first
   *  that does not; the `WindowMsg` says when. Leave it out for one window. */
  windows?: (model: M) => WindowDecl[];
  /** A clock: every `every` ms the loop feeds `msg` (or `msg(now, every)`,
   *  with the clock's own reading — `Date.now()` under a window, the loop's
   *  own milliseconds headless — and the cadence this tick fired on, so a
   *  model can tell a 16 ms reading from a 1000 ms one and know how stale
   *  `now` may be by the next press) to `update`. Ticks are frequent, so unlike UI
   *  events they re-render only when `update` returns a new model — a
   *  countdown that returns undefined until the displayed second changes
   *  costs nothing in between. Read backwards, that is the trap: a tick
   *  handler that mutates the model in place and returns undefined never
   *  reaches the screen. Return the model (any non-undefined return renders)
   *  on the ticks that should draw.
   *
   *  A window fires these off its own timer; headless, `app.advance(ms)`
   *  fires every tick inside the span, so a ticking app is testable.
   *
   *  `every` may be a function of the model, for an app whose cadence
   *  depends on its state: `every: (m) => m.endsAt ? 16 : 1000` ticks at
   *  frame rate while a countdown runs and once a second while it is
   *  stopped — and since the windowed driver's idle gap is capped by the
   *  next tick, a stopped app then costs a pump a second rather than
   *  sixty. It is read after `init` and after every `update` (a tick's
   *  own included); when the answer changes the next tick moves to the
   *  last tick plus the new value, keeping the beat, rather than letting
   *  a tick already queued a second out stand — or, when that is already
   *  past, to the new value from the change, as a fresh loop counts from
   *  its start: a cadence that shortens after a long quiet owes one tick
   *  `every` from now, not a burst. `0` or less means no tick, as the
   *  number does. */
  tick?: { every: number | ((model: M) => number); msg: A | ((now: number, every: number) => A) };
  /** Runs once as the main window goes for good — its close button,
   *  `win.close()`, Quit from the menu or the dock — with the model as it
   *  stands, from inside the pump that saw it and before `runWindowed`
   *  resolves. On a Mac, ⌘Q ends the process inside that pump: the promise
   *  never resolves and nothing after `await runWindowed(...)` runs, not
   *  even `process.on('exit')` — so this is the only thing an app runs on
   *  ⌘Q, and the place to save a session, a draft, a position (backlog
   *  RG1). Nothing draws by then, and the window's doors are not for it.
   *  Headless, `app.teardown()` runs it, so a drive can assert on what
   *  the app would have kept. */
  teardown?: (model: M) => void;
}

/** `runWindowed`'s config: `update` also gets the window. */
export type WindowedConfig<M, A = AppMsg | CoreMsg, E = never> = LoopConfig<M, A, KuiWindow, E>;

/** Opens the main window and runs the Elm loop; resolves with the final
 *  model when that window closes. `config.windows` opens more. */
export declare function runWindowed<M, A = AppMsg | CoreMsg, E = never>(
  config: WindowedConfig<M, A, E>,
  opts?: WindowOptions & {
    title?: string;
    /** The gap between pumps while the app is being used, in ms
     *  (default 8) — and the floor the backoff never goes under. */
    pumpMs?: number;
    /** The longest gap between pumps, in ms (default 32), reached by
     *  doubling once the window has been quiet for `quietMs`. This is what
     *  an idle window costs: a pump is ~1.2 ms of CPU whether or not
     *  anything happened, so the bill is linear in the rate — 8 ms is
     *  8-15% of a core, 32 ms is ~3%, 250 ms is under 1%. What it buys back
     *  is the wait an OS event arriving into a deep idle sits through, up
     *  to this long: clicked with a posted `CGEvent`, a window answers in
     *  34 ms at 8 and ~55 ms at 32. Only the first event after `quietMs` of
     *  silence pays it. Floored at `pumpMs`. */
    idlePumpMs?: number;
    /** How long a window must have been quiet before the gap starts
     *  growing, in ms (default 500). Anything the user does — an event, a
     *  transition, a `dispatch` from outside the loop, an OS event the app
     *  never sees — resets both the gap and this. A tick does not, nor the
     *  frame it draws: the loop's own clock says nothing about whether
     *  anyone is there, and the next tick is a deadline the pump sleeps to
     *  regardless, so a stopped app drawing a clock digit once a second
     *  idles between digits. */
    quietMs?: number;
    /** The loop's time source, in milliseconds. A window fills it with
     *  `Date.now`: ticks fire off it and `tick.msg(now)` reads it (the
     *  frame clock behind `transition` is the runner's own and does not
     *  follow it). A test hands in `() => Date.now() + ahead` to move a
     *  real window's clock — its ticks run ahead of the wall by that much,
     *  which is how a countdown is watched in seconds rather than minutes.
     *  `createApp`'s `startTime` has no counterpart here: under a clock it
     *  is dead. */
    clock?: () => number;
    /** Runs after the window opens, before `init` and the first frame —
     *  register images, fonts and other resources here. `app` is the loop
     *  itself, so a test can hold on to it and drive a real window with the
     *  same helpers `createApp` gives (`settle`, `access`, ...). */
    setup?: (win: KuiWindow, app: WindowLoop<M, A, E>) => void;
    /** Performs the effects `update` returns with `withEffects`, after each
     *  frame. The same handler a headless `createApp` takes, so an effect
     *  a test asserted on is the effect the window performs. */
    effects?: EffectHandler<E, A, KuiWindow>;
    /** The window to drive. Default: a new `KuiWindow` opened with the
     *  options above. A test fills it with a stand-in — what a surface
     *  answers plus `pump`, `animating` and `nextDeadlineMs` — to run
     *  this driver, pump order and all, without a display; the mirror of
     *  `createApp`'s `surface`. */
    surface?: KuiWindow;
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
  /** Shadow quads (kind 5) only: the blur radius, which is also how far
   *  the rect is inflated past the shape being blurred. 0 otherwise. */
  blur: number;
  /** 0 solid, 1 mask glyph, 2 color glyph, 3 image, 4 subpixel glyph,
   *  5 shadow, 6 segment, 7 fragment, 8 texture — `QuadKind` in the core
   *  and `KUI_QUAD_*` in `include/kui.h`, in the same order. A fragment
   *  quad's `uv[0]` indexes `fragmentDraws()`, a texture quad's
   *  `textureDraws()`; on both image kinds `borderW` is the `sampling`
   *  flag (1 = nearest). */
  kind: number;
  uv: [number, number, number, number];
  /** Segment quads (kind 6) only: the stroke's endpoints `x0, y0, x1, y1`
   *  in physical px, with `borderW` the stroke width and `color` the
   *  stroke. A `<line>` is one of these per piece — a curve is flattened
   *  in the core, so a display list carries many. `null` on every other
   *  kind. */
  ends: [number, number, number, number] | null;
  /** Which entry of `decodeClips(ctx.clips())` clips this quad. An index
   *  rather than the clip itself since ABI 11: a clip is 32 bytes and a
   *  frame has a handful of them, so carrying one per quad was paid by
   *  every quad of every frame for a value nearly all of them share.
   *  Entry 0 clips nothing, so there is no null case. */
  clip: number;
}

/** One clip a frame's quads name, from `decodeClips`. Physical px. */
export interface Clip {
  rect: [number, number, number, number];
  /** Corner radii, clockwise from the top-left: a clipping node with a
   *  radius rounds what it clips. All zero = a plain rect clip. */
  radii: [number, number, number, number];
}

/** The container `virtualColumn` declares: every `<box>` prop, plus what it
 *  needs to slice by. `scrollY` and `gap` are the widget's own. */
export interface VirtualColumnProps extends Omit<BoxProps, 'children' | 'scrollY' | 'gap'> {
  /** Names the container. Required: its geometry is read back by this name,
   *  so two lists cannot share one (`ambiguous-key`). */
  key: string;
  /** How many rows the list has, built or not. */
  rows: number;
  /** One row's height in logical px — the whole stride. Put a row's
   *  spacing inside it (a row that pads itself) rather than in a `gap`. */
  rowH: number;
  /** Rows built past each edge of the window, covering the frame of lag on
   *  a resize or a wheel jump. Two by default. */
  overscan?: number;
}

/**
 * A vertically scrolling column of `rows` uniform rows that declares only
 * the visible ones, and the two spacers that hold the height of the rest —
 * so the frame costs a screenful however long the list is.
 *
 *     virtualColumn(ctx, { key: 'log', rows: lines.length, rowH: 28 }, (i) => (
 *       <box width="grow" height="grow" onClick={{ kind: 'pick', row: i }}>
 *         <text>{lines[i]}</text>
 *       </box>
 *     ))
 *
 * `row(i)` returns row `i`'s *contents*; the widget owns the row's own node,
 * `rowH` tall and keyed by the row's data `index`, so a row keeps its hover,
 * focus, edit buffer and tweens as the built range slides over it — and a
 * virtualised list and a full one agree on identity. A clickable row puts
 * its `onClick` on a `width="grow" height="grow"` child, as above.
 *
 * It also declares the zero-height node that makes the *loop* re-run `view`
 * when the container scrolls: the wheel raises no event and a window redraws
 * by re-lowering the tree it was handed, so nothing else would. That node's
 * events never reach `update`.
 *
 * `ctx.setScroll(key, 0, i * rowH)` puts row `i` at the top — the way to
 * reach a row that is not built, since `reveal` of an unbuilt row finds
 * nothing.
 */
export declare function virtualColumn(
  ctx: Pick<Ctx, 'scrollGeometry' | 'env'>,
  opts: VirtualColumnProps,
  row: (i: number) => KuiNode,
): KuiNode;

export declare function decodeQuads(buffer: Buffer): Quad[];

export declare function decodeClips(buffer: Buffer): Clip[];

/** `M` is the model, `A` every message `update` can see. Annotate `update`
 *  with the app's own union (`type Msg = MyMsg | CoreMsg`) and `A` is
 *  inferred from it; leave it and `A` is the registered `AppMsg` plus the
 *  core's messages. */
export type AppConfig<M, A = AppMsg | CoreMsg, E = never> = LoopConfig<M, A, Ctx, E>;

/** The loop both drivers run, over the surface it was handed. Everything
 *  here is written once and works against either — which is what lets the
 *  test helpers drive a real window when you want to watch one. */
export interface Loop<M, A, S, E = never> {
  /** The surface this loop drives. */
  readonly surface: S;
  readonly model: M;
  /** Every warning the core raised while rendering, in order; each is also
   *  printed unless created with `warnings: false`. A test asserts it is
   *  empty, or that a specific code showed up. */
  readonly warnings: Warning[];
  dispatch(msg: A, event?: UiEvent<A>): void;
  /** The config's `teardown(model)`, once; a second call is nothing. The
   *  window calls it as it goes (see `LoopConfig.teardown`); a headless
   *  drive calls it to end. */
  teardown(): void;
  /** What `update` (and `init`) returned besides the model since the last
   *  drain — every effect, whether or not an `effects` handler ran, the
   *  way `audioCommands()` answers without a device. Headless this is the
   *  assertion point; a window drains it every frame. */
  effects(): E[];
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
  /** One half of a key: what the *core* is asked to do with it (Escape
   *  dismisses a modal, Tab walks the ring, an arrow nudges a focused
   *  slider). `press` is the whole key and the one to reach for. */
  key(name: EditKeyName, mods?: KeyMods): void;
  /** A whole key going down, the way a window sends it: the raw press to an
   *  `onKey` sink, and then what the core is asked to do with that key —
   *  Escape dismisses a modal, Tab walks the focus ring, an arrow nudges a
   *  focused slider, Space presses a focused control, a printable character
   *  reaches the focused editor. Spelled as `ctx.keyDown` spells it: a
   *  character ("W", "$") or a name ("left", "escape", "f5", ...). */
  press(code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string): void;
  /** The same key coming up, spelled the way `press` spells it. One
   *  channel: the editing keys act on the way down, so this is the release
   *  a sink that declared `keyUp` hears. */
  release(code: string, mods?: KeySinkMods, physical?: string): void;
  /** What assistive technology sees of the last render. */
  accessTree(): AccessTree;
  /** Drives the app the way a screen reader would — `access(key, 'click')`
   *  activates a node, `access(key, 'setValue', text)` types into an
   *  editor — and settles the events that follow through `update`. Works
   *  against a real window too, and there it is not only the screen
   *  reader's path but the only synthetic input a window takes: a smoke
   *  test that reads `win.quads()` presses with this, because `click`,
   *  `type` and `key` are refused on a surface the OS drives. */
  access(key: string, action: AccessAction, value?: string | AccessArg): void;
}

export interface App<M, A = AppMsg | CoreMsg, E = never> extends Loop<M, A, Ctx, E> {
  /** The surface, under the name headless tests reach for. */
  ctx: Ctx;
  /** Moves the loop's own clock `ms` forward: every tick inside the span
   *  fires, the frame clock behind `transition` follows it (`ctx.setTime`),
   *  and the app re-renders. This is the window's timer by hand — what makes
   *  a ticking app drivable by a test. */
  advance(ms: number): void;
  /** Draws a frame, then `advance`s in frame steps (`stepMs`, default 16)
   *  until `animating()` is false — the settled frame a capture or an
   *  end-state assertion wants, by name. A frame that applies a change is
   *  frame 0 of its transitions, so a `render()` straight after a dispatch
   *  is the *start* of the motion; this runs it out. Returns the
   *  milliseconds advanced, and stops at `maxMs` (default 10 000) with
   *  `animating()` still true if something never settles — a looping
   *  keyframe, say. */
  runOut(maxMs?: number, stepMs?: number): number;
}

/** The loop `runWindowed` builds, handed to `setup`. It has no `advance`
 *  and no `runOut`: a window runs on the wall clock and ticks itself, so
 *  nothing here can move time — what it has instead is the two waits below,
 *  answered from inside the driver's pump. */
export interface WindowLoop<M, A = AppMsg | CoreMsg, E = never> extends Loop<M, A, KuiWindow, E> {
  /** Resolves the first time a pump — `win.pump()` and the `step()` after
   *  it — leaves `animating()` false and no effect unflushed, with the
   *  wall-clock milliseconds it waited. Stops at `maxMs` (default 10 000)
   *  and resolves anyway with `animating()` still true, which is what
   *  `runOut` returning its cap says headless.
   *
   *  This is `runOut` for a window, and it is a promise rather than a loop
   *  because a window's clock is the wall's: `advance` refuses it, so a
   *  test cannot step time forward itself and the driver's pump is the only
   *  thing that can say a frame has happened. The alternative it replaces
   *  is `setTimeout(400)` and hope (backlog F30). Rejects if the pump
   *  throws, or if the window closes while it waits. */
  settled(maxMs?: number): Promise<number>;
  /** `settled` with a keyframe cycle allowed: resolves the first time a
   *  pump leaves nothing owed but a `repeat` cycle (`owed()` with only
   *  `cycle` set, or nothing) and no effect unflushed, with the
   *  milliseconds it waited — and at `maxMs` anyway, as `settled` does.
   *  For the window whose view has a looping keyframe, where `settled()`
   *  can only ever hit its cap: the transitions have run out, and what is
   *  still moving is moving by design (backlog F64). Not an option on
   *  `settled`, since a wait that ignores something should say so in its
   *  name. */
  quiet(maxMs?: number): Promise<number>;
  /** Resolves after the next pump has painted — the cheap half of
   *  `settled`, for a test that only needs the window to have drawn, not to
   *  have stopped moving. A promise for the same reason: the pump runs on a
   *  timer a test cannot see into. */
  frame(): Promise<void>;
}

export declare function createApp<M, A = AppMsg | CoreMsg, E = never>(
  config: AppConfig<M, A, E>,
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
    setup?: (ctx: Ctx, app: App<M, A, E>) => void;
    /** Performs the effects `update` returns with `withEffects`, after each
     *  frame (see `EffectHandler`). Optional: without one the effects still
     *  reach `app.effects()`, which is what a test reads. */
    effects?: EffectHandler<E, A, Ctx>;
  },
): App<M, A, E>;
