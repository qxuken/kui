---
status: accepted
date: 2026-10-24
---

# An app runs with no window

> **Accepted and built (2026-10-24); backlog C58.** From kawoosh's
> question after ADR 0055: with a frame drawn again on a device of its
> own, an app need not have a window to take pictures of itself, so
> could it run with none at all — on the real clock, woken by its
> threads, driven over a socket by an agent — and never take the user's
> screen or keyboard?

## Context

Two ways ran an `App` until now.

- **The windowed runner** (`Launcher::run`, `Launcher::open`): a winit
  event loop, the real clock, frames paced by wakes, times asked for, an
  animation and the display, and every effect of the frame done to the
  OS. It needs a display: winit cannot open its loop without one.
- **`testing::Drive`**: no window and no GPU, a clock that moves when the
  test says, a frame when the test asks. It keeps every event it hands
  out in a log, never serves a capture, and has no `Waker` to give
  `App::setup`.

Neither is an app *running* with no window. kawoosh — an editor whose
plugins, language servers, terminals and file watchers all wake the
frame from threads — wanted exactly that, for three things (kawoosh's
`docs/design/headless.md`):

1. **Agent and smoke runs that do not touch the desktop.** A test build
   is driven over its socket and photographed with `:screenshot`. With a
   window it comes up over the user's work, and the keys a socket
   command cannot send went in as synthetic OS events, which twice
   landed in the user's own copy of the app. A run with no window takes
   its keys from the process that drives it, and from no one else.
2. **Pictures rebuilt by a script**: the docs, the help, a theme
   gallery, a bug report replayed into an animation.
3. **A box with no display**: a Linux machine over ssh, a container.

What stopped an app doing it itself: `App::setup` takes a `Waker`, and a
`Waker` was a winit `EventLoopProxy` that nothing outside kui-native can
make. So an app could not run its own `setup` with no window without
splitting it in two, and then it would write the runner's pacing, its
capture serving and its clipboard again, in the one app that needed it.

## Decision

1. **`Launcher::open_headless(app, w, h, scale) -> HeadlessRunner<A>`.**
   The same launcher a window is opened from: its core, its setup steps,
   its diagnostics, its extensions, its chrome (which `env.window` says)
   and its pinned `env.system` apply; its title, icon, size bounds and
   backdrop have nothing to apply to. `App::setup` runs at open. Nothing
   of winit runs, so it needs no display. A GPU only for a capture.

2. **A `Waker` rings a window's loop or this runner.** The type keeps its
   name and methods, `wake` and `wake_at`; its field is private, so it
   is now an enum of the two targets with no change to any caller. A
   thread an app started in `setup` wakes a headless run as it wakes a
   window.

3. **The host turns it, or hands it the thread.** `turn()` handles what
   was sent in, draws the frame if one is owed, and does what the frame
   asked for, without waiting. `turn_until(deadline)` parks until there
   is something to turn for or the deadline comes. `run()` parks and
   turns until the run ends. Between turns `app_mut`, `core_mut`,
   `frames`, `size`, `next_deadline` and `idle` read the state, as
   `PumpRunner`'s readers do. `next_deadline` is read from the core each
   time it is asked, so an ask made through `core_mut` between turns — a
   capture, `request_frame` — is in it.

4. **A frame is owed** by input, a wake, a time the view or a `wake_at`
   asked for that has come, the events the last frame made reaching the
   app, or the core animating — which includes a frame asked for with
   `request_frame`, a capture's among them — drawn at 60 Hz
   (`env.refresh_hz` says so), since there is no display to pace it. The
   caret does not blink: a frame is drawn for something the app did, so
   two pictures of one state are one picture.

5. **Input comes in through `HeadlessInput`**, cloneable and `Send`:
   `send(InputEvent)`, `resize(w, h, scale)`, `focus(bool)`,
   `set_clipboard(text)`, `close()`. **Everything sent before a turn
   begins is handled before that turn's frame, in the order sent.** That
   is what makes a driver's "these keys, then this question" answer
   about the frame after the keys: a socket reader that sends a run of
   keys and then asks the app something reads the state all of them made.

6. **What the frame asks of the OS, the runner keeps to itself.**
   - The clipboard is the runner's own text: a copy is read back by
     `HeadlessRunner::clipboard()`, a paste gets what the run copied or
     what `set_clipboard` put there, and the user's clipboard is never
     written. A picture for the clipboard is dropped.
   - Sounds are dropped, as with the `audio` feature off.
   - A file dialog is answered at once with no files, as a cancel.
   - Windows a view declares are not opened; the main window's close —
     the chrome's close button, `WindowCommand::Close(MAIN)` — ends the
     run, and `App::teardown` runs once, as it does for a window, on the
     close, on `close()`, or when the runner is dropped first.
   - `env.system` is what the launcher pinned and nothing else. The OS is
     not asked, so a picture taken on one machine is the picture another
     takes. An app that wants the OS's reading reads it and pins it.
   - `KUI_DEVTOOLS` does not open the panel, as for a headless core: the
     pictures are the app's.

7. **Captures are served after every frame, as a window's are**, a
   recording on a thread of its own whose wake is this runner's (ADR
   0055's amendment), so recording a headless run costs its frames a
   draw and a submit.

## Considered options

- **The loop in the app.** kawoosh drives a `Drive` on the real clock
  with its own condvar, serves `Captures` itself, and splits `setup`
  into the waker half and the rest. It needs no kui change — and that is
  the case against it: the pacing, the capture serving, the clipboard
  and the dialogs written again beside the runner's, by the app with
  the least reason to know them. Declined.
- **`Waker::from_fn(wake, wake_at)`, the loop still the app's.** The
  smallest kui change, and it lifts the one thing an app cannot do. But
  it leaves every other part of the loop to each app that runs headless.
  Declined, though decision 2 is it, kept private.
- **`Drive` on the real clock.** `Drive` is a test's: it logs every
  event it hands out, which in a run of hours is memory with no end, and
  its frames are the test's to call. A runner and a test driver want
  different things of the same calls. Declined; `Drive` is unchanged.
- **The windowed runner with the window hidden.** A window never shown,
  under the same loop. It still needs a display, which a box over ssh has
  not; the OS still makes it a window (a Dock icon, a Mission Control
  entry, focus rules); and it would still read and write the user's
  clipboard. Declined.
- **The system clipboard, as a window has.** What a run copies would
  land on the user's clipboard while they work — exactly the reach into
  the desktop this is meant to remove. Declined; the runner's own
  clipboard is readable and writable from the host, which is what a
  test of copy and paste wants.
- **C, Odin and Node doors.** Their headless hosts already own their
  loop — Node's `Ctx` on its event loop, a C or Odin standalone context
  with `kui_serve_captures` — and what this adds to them is pacing their
  own loops do. Not built until one of them asks; Lua is a guest.

## Consequences

- An app runs with no window from the same `main` that opens one: a flag
  picks `open_headless` over `run`. kawoosh's `--headless` is that flag.
- A test that wants the real clock and the app's own threads — a socket,
  a file watcher, a process — has a runner to drive, where `Drive` is
  for the stepped clock. They are not interchangeable: a headless run's
  timings are the machine's.
- An idle run parks on a condvar: no CPU while nothing is owed. An
  animating one draws 60 frames a second whether or not anything reads
  them.
- What a window does that a run cannot see is not tested by it: the
  platform's input methods, drag and drop from the desktop, the menu
  bar, occlusion, presenting and its pacing. The smoke round's windows
  stay the test of those.
- Pinned by `crates/kui-native/tests/headless.rs`: the first turn draws
  and an idle one does not; three clicks sent from another thread before
  a turn are one frame that sees all three; a wake from a thread `setup`
  started brings a frame; a time the view asks for is the deadline and
  the frame comes then; a copy goes to the runner's clipboard and
  `set_clipboard` replaces it; a resize lays out the next frame and is
  heard; the main window's close ends the run and tears down once; `run`
  returns on `close()` from another thread; a capture asked between turns
  is written after the next frame and answered (or `failed`, "no GPU");
  a recording is written on its thread and answered on its wake.
