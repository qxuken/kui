---
status: accepted
date: 2026-10-23
---

# A frame is drawn again to be read

> **Accepted and built (2026-10-23); backlog C55.** From a question
> about offscreen rendering: how cheap it is, and what kui would use it
> for. The answer was: pixels for tests, and kui drawn into a texture
> its host owns. This puts both in kui-wgpu, and a `capture` door in
> four bindings on top of the first.

## Context

`Renderer::render` took the window's swapchain image itself and drew
into it. The surface was configured `RENDER_ATTACHMENT` only, so nothing
could read a frame kui drew. The one offscreen target was the backdrop
blur's (F129), private to the frames that blur.

That left two holes.

- **Nothing checked the shipping renderer's pixels.** F19 put
  `capture()` off "until a smoke test needs pixels". Since then the
  regression passes have checked paint with `screencapture` of a window
  id, CGEvent probes and a state gallery screenshotted by hand. The GPU
  tests in kui-wgpu are CPU transcriptions of the WGSL, plus the
  backdrop's one readback.
- **A host with its own wgpu device could not have kui draw into its
  texture.** A game engine, a 3D panel or a runner compositing kui with
  its own content had to give kui a window.

The bench C55 added (`crates/kui-wgpu/benches/offscreen.rs`, M3 Pro,
Metal) says what the pieces cost at 3456×2234. A whole frame drawn
offscreen and blitted is +0.12–0.24 ms of GPU, and 29.5 MB. Reading one
back is about 1.1 ms of copy and wait, plus 0.6 ms to unpad the rows.
Making a target is 30–45 µs, with the first use 0.35–2 ms. A device with
no window opens in 12–28 ms. None of it is a reason not to.

## Decision

1. **kui-wgpu draws without a window.** `Gpu::windowless()` opens a
   device with no surface to be compatible with. `Gpu::from_parts(instance,
   adapter, device, queue)` wraps a device the host opened, reading
   dual-source blending and mapped frames off what the device has
   enabled, and sets neither an error handler nor a lost callback, which
   are the host's. `Renderer::windowless(&gpu, format)` builds the same
   pipelines with no surface. `Renderer::render_to(list, atlas,
   &texture)` draws a frame into a texture of that format with
   `RENDER_ATTACHMENT` usage and submits it. `render` on a windowless
   renderer draws nothing and returns `RenderError::Skip`.
2. **One draw, three targets.** `render` is split into the CPU half
   (instances, blurs, globals, bind groups, pipelines), the passes into
   a view, and the submit. The surface's image, a host's texture and a
   capture's texture are all a view to the same passes, the backdrop
   blur's included.
3. **A capture draws the frame again.** `Renderer::capture(list,
   &atlas)` draws into a texture of its own, the list's viewport in
   physical px, reads it back, and returns `Capture { width, height,
   rgba }`. It does not read the swapchain (see *Considered options*).
   It takes the atlas without clearing `dirty` and uploads it whole each
   time, so a renderer that is not the core's own leaves the core's
   renderer the changes it has not drawn.
4. **Straight RGBA out.** The target holds premultiplied colour. A
   capture divides it back out (an opaque frame skips that) and swizzles
   a BGRA target. Straight RGBA, row by row from the top left, is what
   `add_image`, `decode_image` and a window's icon take, so a capture can
   be shown again as an image.
5. **`kui_native::capture(&core)` is the door.** It reads the core's
   finished frame through `Core::drawn()`, a reader that marks nothing
   read and lends the atlas immutably, and draws it on a windowless
   renderer the process opens on the first capture and keeps. It reopens
   that renderer after a lost device. It clears to the theme's `bg`, as
   the runner clears an opaque window. The same call serves a window's
   core between pumps and a headless core after a frame.
6. **Doors.** Rust `kui_native::capture`. C `kui_capture(ctx, &w, &h)`,
   freed with `kui_pixels_free` like the decoder's pixels. Odin
   `kui.capture(ui)`, copied into the context allocator. Node
   `capture()` on both `Ctx` and `KuiWindow`, returning `{width, height,
   scale, rgba}`. Lua has none, for the reason every reading-back row
   gives (`GUEST`). It is a new function in C, with no struct moved, so
   the ABI does not change.
7. **An encoder beside the decoder, for stills and recordings.** With
   the crates the runner already links for `decode_image` (`image`, and
   the `png` and `gif` it builds), so a test or a probe keeps what it saw
   as a file with no image library of its own; Node has none built in.
   - `kui_native::encode_image(format, width, height, &rgba)`: PNG
     (lossless, the fast setting: about 12 ms and 1.2 MB for a
     text-heavy 3456×2234 capture), JPEG (a quality, alpha dropped),
     WebP (lossless: there is no lossy encoder in the tree) or GIF (256
     colours).
   - `kui_native::AnimationEncoder`: an animated PNG or GIF written a
     frame at a time, `add(rgba, seconds)` then `finish()`. A frame is
     the whole canvas, but what is kept is the rectangle it changed over
     the frame kept before (both formats draw a frame over the last at an
     offset), a frame that changed nothing adds its time to that one,
     and a frame shorter than the format shows (1 ms for PNG, 20 ms for
     GIF, which browsers slow below) gives its time to the next. So a
     recording holds two whole frames and the changes, where a list of
     frames handed over at the end would be 30 MB a retina frame; and
     APNG must know its frame count before it writes, which holding the
     changes until `finish` gives it. Animated WebP has no encoder in
     the tree.
   - Doors: `kui_encode_image` with `KUI_ENCODE_*` and the four
     `kui_animation_encoder_*` (an opaque handle) in C, the pixels freed
     with `kui_pixels_free`; `encode_image` and `animation_encoder_*` in
     Odin; `kui.encodeImage` and the class `kui.AnimationEncoder` in Node;
     `GUEST` in Lua. New functions, no struct moved, so no ABI change.

## Considered options

- **Read the window's swapchain.** Configure the surface `COPY_SRC` and
  copy the presented image out. It would be the window's exact pixels,
  the OS's material aside. But the capture would cost every frame:
  Metal's `framebufferOnly` drawable is the fast one, and a readable one
  is not. It would also reach only a window, so a headless core — the
  one a test drives — would have nothing. And the request would have to
  ride across the pump to the runner and back, which the Rust `App` and
  C's `kui_run` have no door for. Declined.
- **Capture on the window's own renderer and device.** No second device,
  and the caches shared. But the window's renderer lives inside the
  runner, out of reach of every door but Node's `KuiWindow`, and
  capturing through it would take its atlas's `dirty` flag. The
  process's own windowless renderer reaches every core the same way. It
  costs a second device's pipelines and an atlas copy (4–64 MB), plus a
  viewport-sized texture and buffer during each capture, freed after.
  Declined while that memory does not matter to an app.
- **Capture as an effect.** `ui.capture()` from a view, with the pixels
  arriving as an event. A 30 MB payload does not belong in an event,
  and Node's events cross as JSON. A test reads between frames, which
  is a call, like `quads()`. Declined.
- **Premultiplied out.** It is what the GPU holds, and it skips a pass
  over translucent pixels. But every other pixel door in kui is
  straight, and a capture fed back to `add_image` would darken.
  Declined.
- **PNG out of `capture`.** A screenshot wants a file. But the encode is
  milliseconds a pixel comparison does not need, so the capture stays
  pixels and the encode is a door of its own (decision 7).
- **Layers for seam-free group opacity, and cached layers.** Measured by
  the same bench and not built. A layer drawn before the main pass costs
  about 5–12 µs of GPU and 15 µs of encoding each. A cached
  full-viewport layer saves about 0.4 ms of GPU an animated frame. Both
  wait for a view that asks.

## Consequences

- A capture is what the window shows, less what the OS draws behind a
  translucent window (a material, the desktop) and the wallpaper ground
  kui draws for one. A window over a backdrop clears transparent, and
  its capture clears to the theme's `bg`.
- The first capture in a process opens the device and builds the
  pipelines: about 40–100 ms on an M3 Pro. A 3456×2234 capture after
  that takes about 10 ms in Rust: about 0.5 ms of CPU to prepare, the
  wait for an idle GPU, and the 30 MB copied into fresh memory.
- A machine with no GPU adapter has no capture. Each door fails saying
  so: Rust and Node with a message starting "no GPU", and C and Odin
  with NULL / `ok = false`, and the tests skip, saying so. CI's docker
  runner has no GPU, so its `check` job installs Mesa's software Vulkan
  (lavapipe, `mesa-vulkan-drivers`) and sets `KUI_REQUIRE_GPU=llvmpipe`,
  under which a missing adapter, or one of another name, fails the
  tests and the C and Odin counters rather than skipping them.
- Subpixel text follows the device. A headless core draws grayscale, and
  a window's follows its device unless `textAa` pins it. So a test
  comparing images across machines pins it.
- Between frames only: while a frame builds, the list is the one
  `begin_frame` emptied. A C host under `kui_run` holds its context only
  inside `view`, so it captures a standalone context.
- Pinned by `crates/kui-wgpu/tests/capture.rs` (a capture in physical
  px from both formats, a translucent frame back straight, the atlas
  left dirty, `render_to` into a host's texture and the format refused,
  no viewport refused, a host's own device), by
  `crates/kui-native/tests/capture.rs` (a headless core over the
  theme's ground, and none before a frame), by kui-ffi's
  `a_capture_is_the_finished_frame_as_pixels`, by Node's `capture reads
  the finished frame back as RGBA in physical px`, and by the C and
  Odin counters' headless drives, which also hold `kui_capture` to
  `every_entry_point_is_called`. The encoders by kui-native's
  `encode::tests` (each still format back through the decoder, the
  refusals, an animation keeping its rectangle and folding a repeat in
  both formats, a GIF at 60 fps showing every other frame for the same
  total, one frame a still PNG), Node's `encodeImage` and
  `AnimationEncoder` tests (the second a headless recording), and the C
  and Odin counters, which encode a PNG and a GIF animation and decode
  them back.
- Recording costs little beside the capture. Thirty frames of a card
  sliding over a page of text, 800×600: 2.3 ms a frame to capture, 0.1
  ms to add, 2 ms to finish a 230 KB APNG or 38 ms a 99 KB GIF. At
  3456×2234: 10 ms to capture, 1.7 ms to add, 15 ms to finish an 804 KB
  APNG or 393 ms a 283 KB GIF, the GIF's being its 256-colour
  quantizing.
