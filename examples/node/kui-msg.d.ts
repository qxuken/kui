// Registers this program's own messages with the JSX types, so a typo in an
// `onClick` payload fails at the node that declares it. Register only the
// messages you wrote: `CoreMsg` is typed in terms of this registration (its
// `tag` fields carry your messages), so naming `... | CoreMsg` here would
// make the alias circular. The full union stays where `update` needs it
// (`type Msg = CounterMsg | CoreMsg` in each example).
import type {} from '@qxuken/kui/jsx-runtime';

type AppMessages =
  | { kind: 'add'; by: number }
  | { kind: 'reset' }
  | { kind: 'hum' }
  // The key sink's tag in counter.tsx: tags are messages too.
  | { tool: string };

declare module '@qxuken/kui/jsx-runtime' {
  interface KuiMsg {
    msg: AppMessages;
  }
}
