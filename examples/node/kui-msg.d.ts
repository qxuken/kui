// Registers this program's own messages with the JSX types, so a typo in an
// `onClick` payload fails at the node that declares it. Each is registered
// as it arrives (ADR 0047): a message a handler declares carries that
// event's field — `DragMsg<{ kind: 'pan' }>` is `{ kind: 'pan', drag }` —
// and the handler prop (`onDrag`) takes the registered messages with that
// field, as declared without it. A message registered without the field
// fits no handler but `onClick`. A window's own events are declared the
// same way (ADR 0048) — `windowEvents={{ onResize: { kind: 'resize' } }}`
// takes a registered `ResizeMsg<{ kind: 'resize' }>` — so `update` sees
// these and nothing else.
import type {} from '@qxuken/kui/jsx-runtime';
import type {
  ContextMenuMsg,
  DismissMsg,
  DragMsg,
  DropMsg,
  EditChangeMsg,
  FilesMsg,
  HoverMsg,
  KeyMsg,
  KeySinkMsg,
  MenuMsg,
  ResizeMsg,
  SelectionRangeMsg,
  SliderChangeMsg,
  SystemMsg,
} from '@qxuken/kui';

type AppMessages =
  // The harness's own (devtools.tsx): an OS setting changing, declared on
  // the root it wraps every example in so the view re-runs for it.
  | SystemMsg<{ kind: '@harness'; what: 'system' }>
  // apps/counter.tsx and features/window.tsx.
  | { kind: 'add'; by: number }
  | { kind: 'reset' }
  | { kind: 'hum' }
  // features/window.tsx: the window's size, its root's `onResize`.
  | ResizeMsg<{ kind: 'resize' }>
  // The counter's right-click, which `onContextMenu` sends back, and the
  // menu's dismissal, which its `modal` does: one message per event.
  | ContextMenuMsg<{ kind: 'menu' }>
  | DismissMsg<{ kind: 'closeMenu' }>
  // tools/types.tsx: the key sink's message, the editor's and the
  // slider's; a bare-string click, registered as declared (it arrives as
  // `{ tag: 'save' }`), and a bare drag, registered as it arrives (the
  // prop takes `'nudge'` back).
  | KeyMsg<{ tool: string }>
  | 'save'
  | DragMsg<{ tag: 'nudge' }>
  | EditChangeMsg<{ kind: 'note' }>
  | SliderChangeMsg<{ kind: 'tally' }>
  // features/slide.tsx: the canvas's drag and a card's click.
  | DragMsg<{ kind: 'pan' }>
  | { kind: 'card'; id: string }
  // widgets/virtual_list.tsx: the row a click picked.
  | { kind: 'pick'; row: number }
  // features/clipboard.tsx: the register's key sink, and the log's ask
  // for rows it never built.
  | KeySinkMsg<{ kind: 'register' }>
  | SelectionRangeMsg<{ kind: 'log' }>
  // features/relaunch.tsx: the button that closes the window for the next.
  | { kind: 'reopen' }
  // features/drop.tsx: the zone's files, the buttons inside it, and the
  // Open dialog's answer.
  | DropMsg<{ kind: 'zone' }>
  | { kind: 'clear' }
  | { kind: 'open' }
  | FilesMsg<{ kind: 'added' }>
  // features/devtools_tab.tsx: a source token and an Inspector row, each
  // a hover message and a click message, the picker the tab raises, and
  // the page's button that jumps to the tab.
  | HoverMsg<{ kind: 'tok'; id: number }>
  | HoverMsg<{ kind: 'row'; id: number }>
  | { kind: 'tokClick'; id: number }
  | { kind: 'rowClick'; id: number }
  | { kind: 'inspect-pick' }
  | { kind: 'show-tab' }
  // widgets/table.tsx: the row a click selected, and the column a header's
  // click sorts by.
  | { kind: 'select'; row: number }
  | { kind: 'sort'; by: 'name' | 'size' | 'kind' }
  // widgets/controls.tsx: the toggles' clicks and the sliders' changes.
  | { kind: 'notify' }
  | { kind: 'all' }
  | { kind: 'channel'; i: number }
  | { kind: 'theme'; i: number }
  | SliderChangeMsg<{ kind: 'volume' }>
  | SliderChangeMsg<{ kind: 'gain' }>
  // widgets/select.tsx: each field's choice.
  | MenuMsg<{ kind: 'language' }>
  | MenuMsg<{ kind: 'size' }>;

declare module '@qxuken/kui/jsx-runtime' {
  interface KuiMsg {
    msg: AppMessages;
  }
}
