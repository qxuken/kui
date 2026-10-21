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
A node row that names the message a node event arrives as: `onClick`,
`onDrag`, `onChange`, and the rest. A node that declares no handler
for an event hears none of it.
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

**Root event**:
An event with no node to declare a handler on, such as a resize or a
modifier change. It arrives with the core's own kind.
_Avoid_: global event, core message

## Editors

**Seed**:
The text a new editor starts from (`initial`). After the first frame
the editor owns its text.
_Avoid_: default value, initial value

**Value**:
The text a view declares for an editor on every frame. When it differs
from the editor's text, the editor is brought to it.
_Avoid_: controlled text, model text
