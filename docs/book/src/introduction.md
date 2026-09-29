# Introduction

kui is a UI library for Rust. You describe what is on screen as a tree
of boxes and text. kui lays it out, draws it, and turns what the user
does into plain data.

This book is the first path through it. Each chapter adds one idea and
ends with a program you can run. The programs live in the repository
under `examples/rust/tutorial/`, and the code you read here is pulled
from those files. It cannot go stale.

## One idea, held throughout

Everything in kui follows from one rule:

> A frame is a function of the tree you declare, the input so far, and
> the time.

Your app has a `view`. It runs once per frame and declares the whole
screen from scratch: every box, every text, every button. Nothing is
retained by you between frames. kui matches this frame's tree to the
last one by key and does the rest.

What the user does comes back as data, not callbacks. A click is a
value. A key press is a value. Your app has an `on_event` that receives
those values and changes its state. The next `view` shows the change.

The core that does this owns no window, no clock and no GPU. Those are
handed in from outside. That is why the same app runs in a window, in a
test with no window, and — with the same tree — from Node, Lua or C.

## Who this is for

Someone who knows Rust and has not used kui. You should be comfortable
with closures and enums. You do not need to know any other UI library.

If you already know what you want and need its name, the reference is a
better door: [`docs/howto.md`](../../howto.md) answers "how do I…"
questions and [`docs/props.md`](../../props.md) lists every prop and
event.

## How to read it

In order, the first time. Each chapter assumes the last. Later, the
sidebar is an index.

Run each chapter's program. Open the devtools while it runs (chapter
12 shows how; it is one line). Watch the events tab as you click.
kui is easier to believe when you can see the data.
