---
status: accepted
date: 2026-10-23
---

# A wheel notch glides

> **Accepted and built 2026-10-23; backlog F182.** What the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed), and the glide was retuned on
> 2026-10-24 ([amendment](#amended-2026-10-24-a-slower-hands-notches-run-into-each-other)),
> then made a setting with presets the same day
> ([amendment](#amended-2026-10-24-a-feel-the-app-chooses), backlog F193).
> Raised by the user from
> Windows: a mouse wheel there scrolls in hard 40 px steps, where a
> macOS trackpad scrolls smoothly. The steps are not Windows' alone. A
> notch is a step on every platform, a plain mouse on macOS included;
> macOS looks smooth only because a trackpad sends a stream of pixels,
> not notches. The proposal: a notch becomes an input of its own, which
> the core turns into a short glide toward a target that every further
> notch moves on. Pixel streams stay as they are. It is on by default,
> and an app turns it off with one switch.

## Context

### Where a notch goes today

The native runner turns winit's `MouseScrollDelta::LineDelta` into
pixels at once (`Core::lines_to_px`, 40 px a line, `SCROLL_LINE_PX`) and
sends it as `InputEvent::ScrollGesture { delta, begins }`, the same
event a trackpad's `PixelDelta` becomes (`crates/kui-native/src/lib.rs`,
the `MouseWheel` arm). The runner keeps the difference only to tell
where a gesture begins (`scroll_gesture::Kind`, ADR 0038). The core
routes the delta (`route_scroll`, `runtime/gesture.rs`), and
`scroll_into` adds it to the container's stored offset
(`ScrollStore::scroll_by`). The next layout draws it there. One notch
is one 40 px jump on the next frame.

How winit 0.30 reports a wheel, per backend (to be checked against the
pinned winit when this is built):

- **Windows**: every `WM_MOUSEWHEEL` is a `LineDelta` of `delta / 120`.
  A notch is 1.0. A precision touchpad and a high-resolution wheel
  send fractions of a line, many times a second.
- **X11**: notches as `LineDelta`, and XInput2's smooth-scroll
  valuators as fractional `LineDelta`s.
- **Wayland**: a wheel's notches as `LineDelta`, and a touchpad's
  continuous axis as `PixelDelta`.
- **macOS**: an event with precise scrolling deltas (a trackpad, a
  Magic Mouse, a smooth-scrolling mouse driver) is a `PixelDelta`,
  with phases and a momentum run. Any other mouse is a `LineDelta`.

So an unsmoothed `LineDelta` reaches the user on every platform
whenever the hand is on a plain wheel.

### What the core already eases, and why it does not fit

A scroll container that declares a `transition` eases a *programmatic*
offset change (a `set_scroll`, a `reveal`) as a leg in the store:
`smooth: Option<(from, start, transition)>`, with the target in
`offset` and `sample` drawing a curve between the two
(`crates/kui-core/src/scroll.rs`). `geometry` samples the leg at the
clock the coming layout reads, so a virtual list slices the rows that
frame will draw. Two things keep it from serving a wheel:

- **The hand's writes are never eased, on purpose.** `asked_smooth`
  stays false for the pointer, since easing a swipe or a thumb drag
  "would lag behind the finger". That is right for pixels. A notch
  carries no finger, and the hand is waiting on the step.
- **A leg is a curve from a start, not a chase.** `scroll_by` mid-leg
  ends the leg and moves on from the drawn place, and a new leg starts
  its curve again from rest. A wheel spun at ten notches a second would
  restart every 100 ms at zero speed and stutter. It would also need
  the container to declare a `transition`, whose duration is chosen
  for reveals (a few hundred ms), too slow for a wheel.

## Decision

1. **A notch is an input of its own.** `InputEvent::Wheel { lines:
   Vec2, begins: bool }`, beside `ScrollGesture`, in lines rather than
   pixels. The core converts with `lines_to_px` as before, routes it as
   `ScrollGesture` is routed (the same latch, ADR 0038), and alone
   knows it came from a notch. `Scroll` and `ScrollGesture` keep their
   meaning: pixels, applied as they arrive. The native runner sends
   `LineDelta` as `Wheel`. A driver that keeps sending notches as
   pixels keeps today's jump, so nothing an existing host does changes
   under it. Doors: C `kui_input_wheel(ctx, lx, ly, begins)` (a new
   function, no struct moves, so no ABI bump), Odin `input_wheel`, Node
   `ctx.wheel(lx, ly, begins)`. Lua is a guest and feeds no input.

2. **A whole notch glides, a fraction does not.** Per axis, a `Wheel`
   delta of at least one line is eased. A smaller one, from a precision
   touchpad on Windows, a high-resolution wheel or XInput2's smooth
   scrolling, is already a fine stream, so it is applied as it arrives.
   The only exception is a container that is mid-glide, where the
   fraction is added to the target so the content does not jump back.
   The threshold is per event, not per gesture, because a
   high-resolution wheel turned fast sends whole lines.

3. **A glide chases a target with momentum.** Each notch adds its
   pixels to the container's target (`offset`), clamped to the last
   layout's `max_offset`. A notch past the end therefore stores nothing
   that would have to be scrolled back before a reverse spin moves the
   content. The drawn offset chases the target on a critically damped
   spring, the `smooth` easing's integration (`anim.rs`), which keeps
   its velocity when the target moves. A spun wheel speeds up and a
   reversed one turns, with no restart from rest. The spring settles in
   about 150 ms. The number is a constant tuned when building, against
   browsers, not a prop. The glide sits in the store beside the
   existing leg as a third state of an entry: offset at rest, a leg
   toward a programmatic target, or a glide (drawn place, velocity,
   last clock). It is advanced at the clock the layout reads, as `sample`
   is, so `geometry` and the virtual lists see the drawn place
   unchanged. It ignores the container's `transition`: a container with
   none still glides, and one with a slow reveal does not make its
   wheel slow.

4. **Routing reads the target.** `ScrollStore::room`, what a gesture
   asks to learn whether a scroller can still move a way, answers from
   the target while a glide runs. That differs from a leg, where the
   delta ends the leg and the drawn place is the truth. So a list whose
   glide is headed for its end passes the next gesture to the scroller
   around it at once, not when the animation lands. A handler, meaning
   an `on_scroll` node such as a terminal's sink scrolling its rows,
   gets a notch's pixels as an event, as it does today, unsmoothed. It
   owns its content, and a fraction of a terminal row is not a place.

5. **What ends a glide.** These end it and go on from the drawn place,
   as `scroll_by` does with a leg today:
   - a pixel delta on the container (a trackpad taking over)
   - a thumb drag
   - the held-drag autoscroll (`follow.rs`)
   - an assistive technology's scroll action

   A `set_scroll` or `reveal` ends it and starts its own leg from the
   drawn place. A layout that shrinks `max_offset` clamps the target and
   the glide lands there. The glide snaps to its target once within
   half a pixel and nearly still, so a settled view is never left a
   subpixel off.

6. **It keeps frames coming the way a leg does.** A gliding entry is in
   `ScrollStore::animating` (the `owing` set), so the runner's pacer
   draws until it settles and idles after, as for a leg. The runner
   changes only in the event it sends.

7. **On by default, one switch to turn it off.** `Core::set_smooth_wheel
   (on)`, true by default, read back by `Core::smooth_wheel`. It is a
   `DOORS` row in the shape of `set_devtools_key`: C
   `kui_set_smooth_wheel` / `kui_smooth_wheel`, Odin `set_smooth_wheel`
   / `smooth_wheel`, Node `ctx.setSmoothWheel` / `ctx.smoothWheel`, and
   Lua `No(GUEST)`, since an extension does not set the app's input
   feel. Off, a notch jumps as it does today. The launcher's
   `smooth_wheel(on)` is sugar for `setup_core`, and the runner copies
   the main core's setting to every window it opens, so the switch is
   the app's and not one window's. Node's `runWindowed` takes the same
   as `smoothWheel: false`.

8. **Reduced motion jumps.** When the window's `env.system.motion`
   reads `reduced`, a notch jumps whatever the switch says. Up to now
   nothing in the core shortened an animation for that setting; a view
   honoured it where it declared one (`schema.rs`, the `system.motion`
   fact). A glide
   is the first animation the core starts on its own, which no view
   declared and so no view can honour. The fact's doc says so.

## Considered options

- **Smoothing in the runner.** The runner could spread a notch over a
  few frames as small `ScrollGesture` pixel deltas, with no core change.
  Rejected for four reasons:
  - A C, Odin or Node host driving its own window would not get it.
    That breaks the bindings-first rule: shared behaviour goes in the
    core, with a door per binding.
  - The runner would need a clock and frames of its own.
  - The core would see invented pixels it could not tell from a
    trackpad's.
  - Routing could not hand a gesture off by the target (decision 4),
    since the core would never learn the target.
- **Reuse the leg, `scroll_by_smooth`.** It restarts from rest on every
  notch (see Context), and it runs only in a container that declares a
  `transition`. Rejected.
- **Guess notches from pixel deltas.** For example, treat a
  `ScrollGesture` that is a multiple of 40 px as a notch. That is
  fragile: a host may send any pixel count, and a trackpad's delta can
  happen to be 40. The event that says what it is costs one variant and
  one door. Rejected.
- **Smooth pixel streams too.** That adds lag to trackpads, which the
  OS already smooths, with momentum. Rejected.
- **Opt-in.** Rejected by the user. Browsers glide a notch by default,
  and an option each app has to discover leaves most apps stepping.
- **A per-container switch** (a prop, or reading `transition`).
  Deferred. The app-level switch answers the ask, and a container that
  must stay instant has not been named yet. If one is, it is a prop in
  `scrollMods`' shape.

## Consequences

- **What breaks:** under the native runner, a notch now lands over
  about 150 ms instead of on the next frame, for every app that does
  not opt out. A view or a test that reads a container's offset in the
  frame after a notch now reads it mid-glide. The headless doors that
  feed pixels (`scroll`, `kui_input_scroll`, `scrollGesture`) are
  unchanged, so tests written against them do not move.
- **The changelog lists it under What breaks**, with
  `set_smooth_wheel(false)` as the way back, and a What-you-can-delete
  line: an `on_scroll` handler that intercepts the wheel only to call
  `set_scroll` on its own container with an easing.
- **Cost:** an entry at rest pays nothing. `animating` is the same set
  and the store's walk is the same walk. A glide costs one spring step
  per gliding container per frame. The frame benches have no wheel in
  them, and the guard should read the same.
- **Tests, at the core** (headless, with `set_time`):
  - a notch is part-way after one frame and at its target once settled
  - ten notches 100 ms apart move without a stall (the drawn offset
    keeps rising, never repeats)
  - a reverse spin turns without overshooting the start
  - a fraction is applied at once, or added to a running glide
  - the target clamps at the end, and the next gesture is handed off by
    the target
  - a pixel delta, a thumb drag, a `set_scroll` and a shrinking layout
    each end or clamp a glide as decision 5 says
  - the switch off, or `reduced` motion, jumps
  - a handler gets the notch's pixels unsmoothed
- **At the runner:** a `LineDelta` becomes `Wheel` and a `PixelDelta`
  stays `ScrollGesture`. Seen by hand on macOS with a plain mouse, on
  Windows, and on Linux under X11 and Wayland, since the per-backend
  list in Context is from reading, not from running.

### Not done here

- **Windows' "lines per notch" setting** (`SPI_GETWHEELSCROLLLINES`,
  3 by default) and macOS's scroll speed. A notch stays 40 px. Reading
  the user's setting is a separate question about how far a notch goes,
  not how it gets there.
- **Easing a key's scroll**, such as PageDown in a container. The core
  scrolls no container by key today, and the assistive-technology
  scroll actions stay jumps (decision 5).
- **A notch on a handler.** It is not smoothed, and the event does not
  say it was a notch. An app that wants to know asks for that
  separately.

## What the building changed

- **Each notch gives the spring its own speed** (decision 3). A
  critically damped spring started from rest moved 14% of a notch on
  the first frame: a soft start that read as lag. Each notch now also
  adds `ω` times the travel it added to the target to the velocity,
  which turns its own share into an exact ease-out, `1 - e^(-ωt)`,
  while a notch landing mid-glide still adds to the speed there is.
  The response is 0.21 s. Measured at 60 Hz: 37% of a notch on the
  first frame, nine tenths by 100 ms, and landed on the target by the
  eleventh frame, once within half a pixel and under 20 px/s.
- **The first step is one nominal frame** (1/60 s). The store's clock
  is whatever the last frame or input stamped, which may be long ago;
  stepping from it would land most of a notch on the first frame.
- **Per event, not per axis** (decision 2). An event glides when any
  axis it moves is a whole line, or a glide is already running on that
  container; otherwise it is applied as pixels are. The glide chases
  the target on both axes, so a fraction on the other axis rides it.
- **A glide needs a laid-out container and a clock.** A key no layout
  has resolved yet, or a store no driver gave a clock, takes a notch as
  `scroll_by` always did.
- **Node's `smoothWheel`** is applied by the loop to whichever surface
  it drives, a `Ctx` or a `KuiWindow`. A window opened after the main
  window copies the main core's switch (`windows.rs`). That covers the
  runner, `kui_run` and Node's window alike.
- **The C round calls every new door.** `abi_parity`'s
  every-entry-point check refused the three C functions until
  `examples/c/tools/surface.c` called them; it now turns the switch off
  and checks a notch's jump on the card.
- **Seen by hand on macOS** with line-unit wheel events posted through
  the HID tap (a plain mouse's kind, which winit reported as
  `LineDelta(0, ±n)` with no phase), recorded at 60 fps. A 3-line notch
  moved the `virtual_list` example's rows on every frame, fast and then
  settling, and landed 120 px down. Windows and Linux are not run here;
  the per-backend list in Context is still from reading.

## Amended 2026-10-24: a slower hand's notches run into each other

Raised by the user from Carnet: notches turned slowly were jumpy. At a
0.21 s response with a whole kick, a notch was landed by 180 ms. Its
speed fell from 15 px a frame to under one by the ninth frame, so
notches 150–250 ms apart each found the page stopped. Each one lurched
off at full speed and halted again.

- **Each notch gives `GLIDE_KICK` (0.6) of the pure ease-out's speed**,
  and the response is 0.30 s. A notch's share still eases out, its
  first frame the fastest. At 60 Hz that is 7.8 px of a 40 px notch on
  the first frame (was 14.8), nine tenths by 170 ms (was 100) and
  landed by about 280 ms (was 180). Two notches 150 ms apart go from
  1.3 px a frame to 8.8, where they went from 0.5 to 15.1. At 250 ms
  apart they go from 0.3 to 8.0, where the old glide had been still
  for four frames. A spin of ten notches a second trails its target by
  about 27 px, under a notch (was 13).
- No kick at all, a spring from rest, was the soft start the building
  rejected as lag. A whole kick on a slower response alone keeps the
  hard start and still stops before a 250 ms gap ends. The two numbers
  were picked by simulating the step at 60 Hz, not by eye on a device.
- `notches_turned_slowly_run_into_each_other` (`wheel_glide.rs`) holds
  the 150 ms case: the glide is still moving at least a pixel a frame
  when the second notch comes, and that notch's first frame is under
  eight times the last one before it.

## Amended 2026-10-24: a feel the app chooses

Raised by the user after trying the retuned glide on Windows: it could
be a touch faster, and they wanted settings and presets to choose from
rather than another retune (backlog F193). Decision 3's "a constant, not
a prop" and Not done here's "a notch stays 40 px" give way to one
setting, the app's like the switch.

- **`WheelFeel { line_px, response, kick }`**, set by
  `Core::set_wheel_feel` and read back by `Core::wheel_feel`: how far a
  line goes (the 40 px that was `SCROLL_LINE_PX`), the glide's response
  (`GLIDE_RESPONSE`) and its kick (`GLIDE_KICK`). The numbers are
  clamped (a line 1–1000 px, a response 0.05–2 s, a kick 0–1; not a
  number takes the standard one). The line applies whether the notch
  glides or jumps, to a fraction of a line, and to the pixels an
  `on_scroll` handler hears. `Core::lines_to_px` stays the standard 40,
  for a host converting on its own.
- **Four presets**, by name, slow to fast. `standard` is the default
  and is the amendment above unchanged, so nothing an app does moves
  under it.

  | preset     | line  | response | kick | first frame | nine tenths | landed |
  |------------|-------|----------|------|-------------|-------------|--------|
  | `gentle`   | 40 px | 0.34 s   | 0.55 | 6.5 px      | 183 ms      | 333 ms |
  | `standard` | 40 px | 0.30 s   | 0.6  | 7.8 px      | 167 ms      | 283 ms |
  | `brisk`    | 48 px | 0.27 s   | 0.6  | 10.2 px     | 150 ms      | 267 ms |
  | `swift`    | 64 px | 0.24 s   | 0.62 | 15.5 px     | 133 ms      | 267 ms |

  Simulated at 60 Hz as the amendment above was. Speed comes mostly from
  the line, not the spring: a shorter response alone stops each notch
  sooner and brings back the slow hand's lurch. Every preset is still
  moving 1.3–1.6 px a frame when a second notch comes 150 ms later.
  `brisk` is the "touch faster" asked for. A Windows browser goes
  further, about 100 px a notch at the system's three lines; reading
  that setting is still not done here.
- **Doors.** C `kui_set_wheel_feel(ctx, line_px, response, kick)`,
  `kui_wheel_feel(ctx, &line_px, &response, &kick)` and
  `kui_set_wheel_preset(ctx, name)` (new functions, no struct moves, no
  ABI bump); Odin `set_wheel_feel`, `wheel_feel`, `set_wheel_preset`;
  Node `setWheelFeel(name | { linePx, response, kick })`, partial
  objects keeping the rest, and `wheelFeel()`; `wheelFeel` to
  `runWindowed` and `createApp`. Lua `No(GUEST)`, as for the switch. The
  launcher's `wheel_feel(feel)` is sugar for `setup_core`, and a window
  opened after the main one copies the main core's feel.
- **`KUI_WHEEL`** in the environment, a preset's name or
  `line_px,response,kick` (trailing numbers may be left off), sets the
  main core's feel when the native runner opens it, over the launcher's,
  as `KUI_FRAME_LATENCY` is: to compare feels in an app without
  rebuilding it, and for a user to override the feel an app ships
  with. Every way an app sets its feel before the window opens is
  under it: the launcher's `wheel_feel` and `setup_core`, a core handed
  to `Launcher::core` (so C's `kui_set_wheel_feel` before `kui_run`),
  and Node's `wheelFeel`, which goes to the window as a window option
  as `textAa` does. Only a call while the app runs (a settings screen's
  `set_wheel_feel`) changes it after.

Tests: `wheel_glide.rs` (`presets_go_by_name`,
`a_notch_goes_the_feels_line`,
`every_preset_glides_and_keeps_a_slow_hand_moving`,
`a_feel_is_clamped`), kui-ffi's `wheel_headless`, the C surface round,
Node's wheel test, and kui-native's `kui_wheel_reads_a_preset_or_numbers`.
Not yet felt by hand: which preset the user keeps, and whether
`standard` should stay the default, is theirs to say after trying them.
