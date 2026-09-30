# Layout

At the end of this chapter, the window has a header, a sidebar beside a
content area, and a footer — and resizing the window keeps them in
place.

## Boxes

A frame is a tree of boxes. A box is a `NodeSpec`: a row or a column,
with a size, padding, a gap between its children, and paint.

You open a box with `ui.with(spec, |ui| { ... })`. The closure declares
its children. When the closure returns, the box is closed.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/02_layout.rs:view}}
```

## Sizes are three words

Every box has a width and a height, and each is one of three things:

| word | meaning | spelled |
|---|---|---|
| **fit** | as big as its children | the default |
| **fixed** | this many logical pixels | `.width(160.0)` |
| **grow** | share the parent's leftover space | `.grow_width()` |

`.fill()` is grow on both axes. Two growers split the leftover evenly.
A grower with no siblings that grow takes all of it.

The sidebar above is fixed at 160. The content area grows, so it takes
the rest. When the window is resized, the sidebar stays and the content
moves.

## Rows and columns

A **row** lays its children left to right. A **column** lays them top
to bottom. That axis is the *main* axis; the other is the *cross* axis.

`.main_align(..)` says where leftover space goes along the main axis:
`Start`, `Center`, `End`, or `SpaceBetween` to push children apart.
`.cross_align(..)` aligns children on the other axis. `.center()` is
both at once.

The header is a row with `SpaceBetween`, so its title sits at the left
and its badge at the right, whatever the width.

## Padding, gap, paint

`.pad(16.0)` is space inside the box, on all four sides. `.pad_xy(x, y)`
sets the horizontal and vertical padding separately. `.gap(8.0)` is
space between children.

`.bg(colour)`, `.radius(r)` and `.border(width, colour)` paint the box.
A box with no paint is invisible and only takes up room.

`ui.text_in(spec, text, style)` is a box with one text child. It is how
you give a text a background or a size of its own.

## Try this

- Make the sidebar grow too. What happens to the split? (Two growers:
  half each.)
- Change the body's `row()` to `column()`. The sidebar is now above the
  content, and its fixed width becomes a fixed *height*? No — width is
  still width. Give it `.height(80.0)` and see.
- Remove `.fill()` from the root. The tree shrinks to fit its content.

## Where this is decided

- Every layout prop, with its Node, Lua and C spelling:
  `docs/props.md`, container props.
- The layout solver is described in the README under
  *Layout*.
