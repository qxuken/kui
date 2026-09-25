// Registers this program's own messages with the JSX types, so a typo in an
// `onClick` payload fails at the node that declares it. Register only the
// messages you wrote: `CoreMsg` is typed in terms of this registration (its
// `tag` fields carry your messages), so naming `... | CoreMsg` here would
// make the alias circular. The full union stays where `update` needs it
// (`type Msg = CounterMsg | CoreMsg` in each example).
import type {} from '@qxuken/kui/jsx-runtime';

type AppMessages =
  // The devtools' own (devtools.tsx): its dock buttons and its root sink.
  | { kind: '@harness'; what: string }
  // apps/counter.tsx and features/window.tsx.
  | { kind: 'add'; by: number }
  | { kind: 'reset' }
  | { kind: 'hum' }
  // The context menu's tag: what onContextMenu sends back, and what the
  // menu declares as its modal tag.
  | { kind: 'menu' }
  // The key sink's tag in tools/types.tsx: tags are messages too.
  | { tool: string }
  // features/slide.tsx: the canvas's drag tag and a card's click payload.
  | { kind: 'pan' }
  | { kind: 'card'; id: string }
  // widgets/virtual_list.tsx: the row a click picked.
  | { kind: 'pick'; row: number }
  // features/clipboard.tsx: the register's key-sink tag.
  | { kind: 'register' }
  // features/relaunch.tsx: the button that closes the window for the next.
  | { kind: 'reopen' }
  // features/drop.tsx: the zone's tag, the buttons inside it, and the
  // Open dialog's tag.
  | { kind: 'zone' }
  | { kind: 'clear' }
  | { kind: 'open' }
  // features/devtools_tab.tsx: a source token and an Inspector row (each
  // both a hover tag and a click payload), the picker the tab raises, and
  // the page's button that jumps to the tab.
  | { kind: 'tok'; id: number }
  | { kind: 'row'; id: number }
  | { kind: 'inspect-pick' }
  | { kind: 'show-tab' }
  // widgets/table.tsx: the row a click selected, and the column a header's
  // click sorts by.
  | { kind: 'select'; row: number }
  | { kind: 'sort'; by: 'name' | 'size' | 'kind' }
  // widgets/controls.tsx: the toggles' clicks and the sliders' change tags.
  | { kind: 'notify' }
  | { kind: 'all' }
  | { kind: 'channel'; i: number }
  | { kind: 'theme'; i: number }
  | { kind: 'volume' }
  | { kind: 'gain' };

declare module '@qxuken/kui/jsx-runtime' {
  interface KuiMsg {
    msg: AppMessages;
  }
}
