---
status: accepted
date: 2026-10-23
---

# An app asks for its window as a file

> **Accepted and built (2026-10-23); backlog C56.** From the question
> ADR 0054 left: `capture` reaches a headless core and a Node window
> between pumps, but not a running app's own window under the native
> runner, which is what a "copy screenshot" row, a bug-report button and
> a "record 5 seconds" toggle need. This revisits the capture-as-an-effect
> option ADR 0054 declined, with ADR 0013's rule beside it: the request
> is data, the runner performs it, and what comes back is a message with
> a file's path in it, not its pixels.

## Context

ADR 0054 built `kui_native::capture(&core)`: the core's finished frame
drawn again on a windowless device and read back as straight RGBA. It is
a call between frames, and who holds the core between frames decides who
can make it.

- **A headless core, Node's `Ctx` and `KuiWindow`** hold theirs, and call
  it.
- **A Rust `App`** holds its core in `view`, while the list is being
  rebuilt, and in `on_event_with` (ADR 0036), which the runner calls
  after the frame with the window's core lent. A still is possible there
  today: `capture(core)`, `encode_image`, `std::fs::write`. A recording
  is not: it wants every frame the window draws, and `on_event_with`
  runs only when there is an event.
- **A C host under `kui_run`** holds a context only inside `view`, and
  its `on_event` gets none. Odin's `kui.run` is C's. So neither can
  capture its own window at all.

And every one that can would write the same thing: a capture per frame
while a flag is on, the time between frames, an `AnimationEncoder`, a
file at the end, an error path. Four bindings, four recorders, each on
the frame path of the app's own code.

ADR 0054 declined capture as an effect because "a 30 MB payload does not
belong in an event, and Node's events cross as JSON". That reason is
about the pixels, not the shape. ADR 0013 says what the shape is for an
effect kui owns: a command type and a drain, a driver that performs it,
and a result that re-enters as a message. The three buttons do not want
pixels in the app either. They want a file: to attach to a report, to
open, to hand to the clipboard or a share sheet. So the payload is the
file's path.

What it costs, from ADR 0054's numbers on an M3 Pro at 3456×2234: a
capture is about 10 ms, an `add` to an animation 1.7 ms, a PNG still
about 12 ms to encode, an APNG's finish 15 ms and a GIF's 393 ms. At
800×600 the capture is 2.3 ms and the add 0.1 ms.

## Decision

1. **Three verbs on the core queue commands, and ask for a frame.**
   - `Core::capture_to(path, message)`: the next frame this window
     draws, written to `path` as one picture.
   - `Core::record_to(path, message)`: every frame this window draws
     from the next one on, written to `path` as an animation when the
     recording stops.
   - `Core::stop_recording()`: the recording stops with the next frame
     drawn, and its file is written.

   Each is on `Ui` too, for a view, and is data until a driver serves
   it: `Core::take_capture_commands()` drains the `CaptureCommand`s,
   so a headless test asserts that a button asked without a GPU or a
   file, as it asserts on `take_audio_commands()`. Called between
   frames, a verb asks for a frame (`request_frame`), so a capture asked
   from a handler does not wait for the next input; called from a view,
   it is served after the frame that view builds. "The next frame drawn"
   is that frame.

2. **The path's extension is the format.** A still is `.png`, `.jpg` /
   `.jpeg` (quality 90, alpha dropped), `.webp` (lossless) or `.gif`; a
   recording is `.png` / `.apng` (an animated PNG) or `.gif`, playing for
   ever. Any other extension fails the command, saying which ones it
   takes. One rule a reader already knows from every screenshot tool,
   with no format enum to keep in step across four bindings.

3. **A driver serves them after a drawn frame, with one struct.**
   `kui_native::Captures` holds what is in flight for one core (a
   recording's encoder, the frame it holds and when that frame was
   drawn), and `Captures::serve(&mut core)` does a turn: it drains the
   commands in order, captures the finished frame once if anything
   needs it, writes stills, adds the frame to the recording, and
   finishes one that stops. The runner keeps one per window and calls
   it after every frame the window draws (not an unseen one, backlog
   F169's, which draws nothing to capture). So the Rust `App`, C's
   `kui_run`, Odin's `kui.run` and Node's `runWindowed` / `KuiWindow`
   all have it with no code of their own. A headless driver calls it
   itself: Rust with a `Captures` of its own beside the core, C
   `kui_serve_captures(ctx)`, Odin `serve_captures`, Node
   `Ctx.serveCaptures()`. A context borrowed from `kui_run`'s view is
   the runner's to serve, and C's call does nothing there.

4. **A recording's time is the frame clock's.** A frame shows from the
   `Core::now` it was drawn at until the next frame drawn, and the last
   one until the stop; the frame drawn at the stop is not in the file,
   so a recording is the span from its first frame to its stop. The
   runner draws only when something changes, so an idle stretch is one
   frame held for as long as it lasted, which the encoder already folds.
   A headless recording moved by `advance` is exact. The first frame
   sets the size: a window resized while recording is cut, or padded
   with transparent pixels, at the right and bottom. A second
   `record_to` while one runs fails, naming the one that runs;
   `stop_recording` with none running does nothing. A window that closes
   while recording has its file written, and no message: its core went
   with it.

5. **The result is a `capture` event: the command's message, a path in
   it.** `capture: {phase, path, width, height, frames, seconds, error}`
   rides in the message the verb named, as `sound` rides in a playback's
   tag (ADR 0047): `phase` is `"saved"` or `"failed"`, `width` and
   `height` physical px, `frames` 1 for a still and the frames drawn
   while recording for a recording (the file folds one that changed
   nothing into the one before), `seconds` its length (0 for a still),
   and `error` the reason on a failure (a directory that is not there,
   no GPU, an extension the format rule refuses). With no message,
   nothing is heard; the file is still written, and a failure is said on
   stderr. The event is pending like a `sound` is, and the runner routes
   it with the frame's others. Rust reads it as `Event::Capture(Captured)`
   beside the app's message.

6. **The pixels stay out of the event.** What a view wants in its own
   memory — a thumbnail of the capture it just took, a comparison — is
   ADR 0054's call between frames, or the file read back through
   `decode_image`.

7. **Written on the thread that serves.** The runner serves on the main
   thread, so a still costs a frame its capture and encode (about 25 ms
   at 3456×2234, under 5 ms at 800×600), a recorded frame its capture
   and add, and a recording's end its finish: 15 ms for an APNG, about
   0.4 s for a retina GIF, whose 256-colour quantizing is the cost.
   APNG is the recording format to reach for; a GIF is for where only
   a GIF plays.

8. **Doors.** Rust `Core::capture_to` / `record_to` / `stop_recording`
   (and on `Ui`), `Core::take_capture_commands`, `Captures::serve`. C
   `kui_capture_to(ctx, path, message)`, `kui_record_to`,
   `kui_stop_recording` and `kui_serve_captures`, the message consumed
   as `kui_play`'s tag is. Odin `capture_to`, `record_to`,
   `stop_recording`, `serve_captures`. Node `captureTo(path, message?)`,
   `recordTo`, `stopRecording` on both classes and `Ctx.serveCaptures()`.
   Lua none (`GUEST`): a guest's frame is the host's to keep. New
   functions and a new event name, no struct moved, so the ABI stays.

## Considered options

- **An `App` hook with the core after each drawn frame** (`App::drawn(&mut
  self, core: &Core)`). The smallest change to the runner, and every
  capture door already exists to use from it. But it is a Rust door
  only, unless C's `kui_run` grows a callback and Node's pump another;
  and then each app writes the recorder this decision writes once, on
  its frame path, with the frame times and the error paths to get
  right. Declined; `on_event_with` stays the place for a Rust app that
  wants a capture's pixels in hand.
- **Pixels in the event, or a handle to them.** The event carrying the
  RGBA is ADR 0054's declined option and still declined: 30 MB through
  Node's JSON, and through every log that prints an event. A handle
  (`capture: {id}`, then `take_capture(id)`) keeps the pixels in the
  core until the app takes them, which is a store with a lifetime the
  app must drain or leak, and a C `on_event` that has no context to
  take them with. A file is what all three buttons wanted.
- **A declared recording** (`record` on the root, present = recording,
  as `<audio>` is present = playing). Its safety is real: a view that
  stops declaring stops the recording, and a forgotten stop cannot
  happen. But it is a root prop in four bindings' schemas and a diff in
  the core for one feature, and a window that closes already ends its
  recording. A still has to be a command anyway (ADR 0008: a one-shot
  spelled as a declaration must vary its key to fire twice). Declined
  while the commands are enough.
- **A format argument.** An enum in four bindings (with JPEG's
  quality), for what the extension says. Declined; a path whose
  extension the rule refuses fails loudly.
- **Writing on a thread.** The encode and the write could leave the
  main thread and the result come back through the loop's proxy. It
  would take the GIF's 0.4 s off a frame. But a headless serve would
  then have to wait for it, two code paths for one result, and APNG is
  15 ms. Not done until a recording's end shows up in a report.
- **A copy to the clipboard.** "Copy screenshot" wants the picture on the
  clipboard, and kui's clipboard is text. An image clipboard is a door
  of its own; with this one, the row writes a file and puts what the
  platform needs on the clipboard when that door exists.

## Consequences

- A screenshot row is one call in every binding: `core.capture_to(path,
  Msg::Saved)` from a handler, or `kui_capture_to(ctx, path, msg)` from
  a C view, and a `capture` message back with the path. A record toggle
  is `record_to` then `stop_recording`, and `request_frame_at(now + 5)`
  for "5 seconds".
- A capture is ADR 0054's: what the window shows, less what the OS draws
  behind a translucent window, over the theme's `bg`. The first one in a
  process opens the device (40–100 ms).
- A machine with no GPU fails each command with a `failed` message
  whose `error` starts "no GPU", so a test there still sees the request
  answered; the tests that write files skip.
- Measured by hand in the `capture` example's window (M3 Pro, a debug
  build, 1720×1200 with the devtools dock): a 5 s recording of a turning
  spinner held 131 frames, about 26 a second where the window draws 60
  unrecorded, and wrote a 2.0 MB APNG. A recording slows the window it
  records by its captures.
- kui-devtools' `Harness` forwards `on_event_with` to the example, which
  a windowed example never heard before; the `capture` example answers
  its buttons there.
- New: `EventKind::Capture` (`"capture"`), a `schema::EVENTS` row, the
  derive's field, `Event::Capture(Captured)`. Pinned by kui-core's
  `captures::tests` (the extension rule), kui-native's `tests/capture.rs`
  (a still written, answered and decoded back, a frame owed for an ask
  between frames, a recording's delays on the frame clock with the stop's
  frame left out, the refusals with and without a message, a second
  recording refused), the C and Odin counters' headless drives (a still
  and a half-second recording, both answered, or both `failed` with no
  GPU), Node's `captureTo and recordTo write the frames and answer with
  the path`, and the `capture` example's headless drive, which checks the
  asks as data with no GPU.

## Amendment: a recording is written on a thread (2026-10-23)

Decision 7 kept everything on the thread that serves, and the first
measurement made the cost plain: a debug build's window drew about 26
frames a second while it recorded, every frame waiting for the GPU's
readback and the copy out. A recording now runs on a thread of its own
in every window the runner opens:

- The serving thread draws each frame again and submits its readback,
  and waits for nothing (`kui_wgpu::Renderer::capture_begin`, about
  0.1 ms at 1720×1200). The pending frame goes through a queue of four
  to the recording's thread, which waits for it, copies it out, adds it
  to the encoder, and at the stop writes the file and wakes the loop;
  the `capture` answer goes out with the frame that wake brings.
- `Captures::write_on_thread(wake)` turns it on; the runner calls it
  with its `Waker`. A headless driver — Node's `Ctx.serveCaptures`, C's
  `kui_serve_captures`, a test — keeps the serving thread, so every
  answer is pending when `serve` returns, as before. Stills stay on the
  serving thread in both.
- The capture renderer no longer uploads the whole glyph atlas page for
  every capture: `GlyphAtlas::rev` moves with each change to its pixels,
  and the renderer keeps the page it uploaded while the atlas's id,
  `epoch` and `rev` are the ones it uploaded from. The id is what keeps a
  renderer shown several windows' atlases — the process's one capture
  renderer — from taking one for another.
- A window that goes mid-recording still leaves a whole file: dropping
  its `Captures` waits for the thread.

Measured (M3 Pro, release): recording a 1720×1200 frame at a 60 Hz pace,
`serve` takes 0.59 ms a frame (1.3 ms at worst) on the serving thread
where it took 6.5 ms (20–27 ms at worst); a Node window recording itself
for 3 s through `recordTo` kept 359 frames — 119 a second, its
display's refresh — and answered 45 ms after the stop. This replaces the
considered option "Writing on a thread" above, whose objection (a
headless serve would have to wait) is met by keeping headless drivers
on the serving thread.

## Amendment: a capture to the clipboard (2026-10-23)

"Copy screenshot" is the third ask beside a still and a recording, and
the clipboard is the host's here as it is for text (backlog C33): the
core works out what to copy and hands it over as a menu action.

- `MenuAction::SetClipboardImage { width, height, rgba }` is a picture
  for the clipboard, straight RGBA, shared rather than copied on its way
  out. `set_clipboard_image(width, height, rgba)` queues one from pixels
  the app holds — a capture, a decoded image — and refuses bytes that
  are not the size's.
- `capture_to_clipboard(message)` asks for the next frame drawn,
  served as a still is, its picture queued as that action, and answered
  with a `capture` event whose `path` is empty: an app reads the same
  answer whichever ask it made.
- The runner writes the picture with arboard's `set_image`, which on
  macOS hands AppKit a TIFF that the pasteboard offers as PNG, JPEG and
  the rest. A C host reads it from `KuiMenuAction`'s new `width`,
  `height` and `rgba`, borrowed until its next `kui_take_menu_action`
  (ABI 37); Node's `takeMenuActions()` reports the size and not the
  pixels, which a headless host already holds or captures. Lua gets no
  door: a script holds no pixels.
