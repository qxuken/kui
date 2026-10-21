# kui

A UI core with bindings for Rust, C, Odin, Node and Lua. A view declares
a tree of nodes on every frame, and what the user does comes back as
data.

## Nodes and names

**Key**:
The identity a node keeps across frames, derived from its path in the
tree. Events name their node by it.
_Avoid_: id, hash

**Key label**:
The name a view opens a node under, unique among its siblings. `keyOf`
turns it into a key and `labelOf` turns a key back into it.
_Avoid_: label (that is the accessible name), id

**Label**:
The accessible name a reader hears for a node. It is not a key label.
_Avoid_: name, title

## Events and messages

**Event**:
Something that happened, delivered with its event kind, the key of the
node it happened to, and its payload.
_Avoid_: message (that is the app's value inside it)

**Event kind**:
Which event it was: a drag, a click, a submit. It rides beside the
payload, not inside it.
_Avoid_: type, kind (that belongs to the app's message)

**Message**:
A value the app defines, declares on a node (or on a playback or a file
dialog), and receives back in `update`. What a handler names is "the
handler's message".
_Avoid_: action, command, tag

**Handler**:
A row that names the message an event arrives as: on a node, `onClick`,
`onDrag`, `onChange` and the rest; on a window, a key of its
`windowEvents` group or of its `windows` entry; on the menu bar,
`onMenu`. Nothing that declares no handler for an event hears it.
_Avoid_: listener, callback, subscription

**Payload**:
Everything an event carries: the handler's message plus the event
field, if the event has one.
_Avoid_: using it for a part of the payload

**Event field**:
The one field the core writes into a handler's message, named after the
event's kind (`drag`, `change`, `hover`). It holds that event's data.
Its type is spelled `With<Event>`: `WithDrag`, `WithEditChange`.
_Avoid_: payload, event data, tag data

**Window event**:
An event about a window rather than a node: a resize, its keyboard
focus, a system setting, the fonts, the modifiers, the OS's documents.
It arrives as the message the window's root declared in its
`windowEvents` group. A window's opening, closing and a popup's
dismissal arrive as the messages on its `windows` entry.
_Avoid_: root event (there are none since ADR 0048), global event, core
message

## Editors

**Seed**:
The text a new editor starts from (`initial`). After the first frame
the editor owns its text.
_Avoid_: default value, initial value

**Value**:
The text a view declares for an editor on every frame. When it differs
from the editor's text, the editor is brought to it.
_Avoid_: controlled text, model text
