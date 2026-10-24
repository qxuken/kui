# Warnings and the devtools

At the end of this chapter, you can see what your app is doing.

## Turn it on

```rust,noplayground
kui_native::app("Todo").devtools(true).run(Todo::default())
```

Or set `KUI_DEVTOOLS=1` and run any app. A panel opens beside the app,
docked to the right. `Ctrl+Shift+D` moves it (left, right, bottom, its
own window, off); `Ctrl+Shift+I` moves the keyboard into it and back.

## Four tabs

**facts** — what the runtime believes right now: the theme and where
it came from, the window and its size, the focused node, the modifier
keys, the node count.

**events** — every event handed to your `on_event`, as it arrives: the
frame number, the event, the node it came from, and the message it
arrived as, as data. Click a row to unfold the message. This is the tab to keep open while you
learn. When a button does nothing, look here first: either the click
arrived and your handler ignored it, or it never arrived and the button
is not where you think it is.

**tree** — the last frame's nodes, foldable, with a filter. Hover a
row and its node lights up in the app the way a browser's devtools light
an element: the content blue, the padding green, the border yellow, and
a badge with its kind, name and size. Click a row to see its box model,
its layout, its paint, its handlers and its state; the app is left alone
while you read, unless the eye button keeps the selection lit. The
picker (`Ctrl+Shift+P`, or the crosshair button) selects a node by
pointing at it.

**perf** — what each frame costs: input, view, layout, render and the
vsync wait, against your frame budget. Press record, use the app, press
stop: every frame is kept with why it ran (a key, the pointer, a wake,
an animation still owed), who held it (the node whose transition was
mid-flight, the line that called `request_frame`) and whether it drew
anything new. Click a bar to open its frame; click a holder to find its
node in the tree. When your app won't go idle, this is the tab: the
frames marked "drew nothing new" are the ones you can stop asking for.
The camera saves a screenshot of the window and the film camera
records it as a GIF, to the desktop (or `KUI_CAPTURE_DIR`), the path
in the events tab.

## Warnings

A mistake that fails silently in most UI libraries comes back as data
in kui: a warning with a stable code, the node it is about, and a
sentence that says what to do. They show in the events tab, and a test
reads them with `d.warnings()`.

The ones you will meet first:

| code | what happened | what to do |
|---|---|---|
| `control-without-name` | a button, editor or checkbox with no text inside it and no `label` | give it `.label("..")` |
| `duplicate-key` | two nodes in one frame share a key | key rows by id, not by text that repeats |
| `transition-auto-key` | a node with a transition sits among siblings that change count, under a position key | give it `with_keyed` or `with_indexed` |
| `modal-behind-content` | a modal float declared before the content that paints over it | declare the modal last |
| `grow-weight-ignored` | `Sizing::Grow(2.0)` on the only grow child | it has nothing to split against; use `grow_width()` |
| `item-outside-container` | a `radio` with no `radio_group` above it | wrap the items |
| `exit-budget` | one frame removed more than 4096 nodes declaring `exit` | that removal did not animate; remove a parent instead of its children |

The full list is the Warnings table in
[`docs/props.md`](https://github.com/qxuken/kui/blob/main/docs/props.md#warnings). Each
`(code, node)` pair is reported once, so a warning in a view that runs
every frame does not flood the log.

## Where this is decided

- The panel is drawn by the core, not by the window runner, so a Node,
  Lua or C app gets the same one, and `KUI_DEVTOOLS=1` works for all of
  them.
- Your app can add a tab of its own:
  [`features/devtools_tab.rs`](examples/features/devtools_tab.md).
