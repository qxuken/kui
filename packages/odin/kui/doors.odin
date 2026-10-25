package kui

import "core:slice"
import c "c"

// The doors the generator cannot lower from the C signature alone, each
// because the header says something in prose that the types do not.

// The pixels behind an image handle, for a renderer meeting a texture quad:
// RGBA, w * h * 4 bytes, borrowed until the next image_pixels call, as
// kui.h says (it promised them until the image changed; backlog RG145).
// Rust: Core::image_pixels.
image_pixels :: proc(ui: ^Ui, id: Image) -> (w, h: u32, rgba: []u8, ok: bool) {
	p: [^]u8
	c.image_pixels(ui, u64(id), &w, &h, &p) or_return
	return w, h, p[:w * h * 4], true
}

// A family token's pick as the two Text_Style fields it fills (ADR 0049):
// a stock pick as its Font_Family and no handle, an installed or loaded one
// as its Font handle (family Sans). ok is false, with unknown-token raised,
// for a name nothing declared or one of another kind.
// Rust: Ui::token_family.
token_family :: proc(ui: ^Ui, name: string) -> (family: Font_Family, font: Font, ok: bool) {
	f: u32
	h: u64
	c.token_family(ui, name, &f, &h) or_return
	return Font_Family(f), Font(h), true
}

// Names the exit a node leaves by if this frame stops declaring it, over the
// one it declared: a throw a button aims. Read during the call; it aims a
// node that declares an exit, with that node's transition.
// Rust: Ui::exit_with.
exit_with :: proc(ui: ^Ui, key: u64, exit: Enter) {
	e := transmute(c.Enter)exit
	c.exit_with(ui, key, &e)
}

// Decodes PNG, JPEG, WebP or GIF bytes (an animated file's first frame) with
// the runner's decoder to straight RGBA, copied into the context allocator:
// delete(rgba) when done. ok is false, the reason on stderr, for bytes that
// are not an image.
// Rust: kui_native::decode_image.
decode_image :: proc(bytes: []u8) -> (w, h: u32, rgba: []u8, ok: bool) {
	p := c.decode_image(raw_data(bytes), uint(len(bytes)), &w, &h)
	if p == nil do return 0, 0, nil, false
	defer c.pixels_free(p)
	return w, h, slice.clone(p[:int(w) * int(h) * 4]), true
}

// The last finished frame as pixels (backlog C55): drawn again on a GPU
// device of the process's own, with no window, and read back as straight
// RGBA in physical px, copied into the context allocator: delete(rgba) when
// done. Between frames. ok is false, the reason on stderr, before any
// frame, or on a machine with no GPU adapter.
// Rust: kui_native::capture.
capture :: proc(ui: ^Ui) -> (w, h: u32, rgba: []u8, ok: bool) {
	p := c.capture(ui, &w, &h)
	if p == nil do return 0, 0, nil, false
	defer c.pixels_free(p)
	return w, h, slice.clone(p[:int(w) * int(h) * 4]), true
}

// What encode_image writes; Png and Gif are what an Animation_Encoder writes
// (an animated PNG for Png). The values are kui.h's KUI_ENCODE_*.
Image_Format :: enum u32 {
	Png  = c.ENCODE_PNG,
	Jpeg = c.ENCODE_JPEG,
	Webp = c.ENCODE_WEBP,
	Gif  = c.ENCODE_GIF,
}

// Straight RGBA, w * h * 4 bytes (what capture and decode_image hand out),
// as a file's bytes with the runner's encoders, copied into the context
// allocator: delete(bytes) when done. PNG lossless; JPEG at quality 1-100
// (0 is 90), alpha dropped; WebP lossless; GIF in 256 colours. ok is false,
// the reason on stderr, for a size of zero or pixels that are not that size.
// Rust: kui_native::encode_image.
encode_image :: proc(format: Image_Format, w, h: u32, rgba: []u8, quality: u32 = 0) -> (bytes: []u8, ok: bool) {
	if len(rgba) != int(w) * int(h) * 4 do return nil, false
	n: uint
	p := c.encode_image(u32(format), raw_data(rgba), w, h, quality, &n)
	if p == nil do return nil, false
	defer c.pixels_free(p)
	return slice.clone(p[:n]), true
}

// An animated PNG or GIF written a frame at a time, each frame the whole
// w x h canvas; it keeps the rectangle each frame changed (ADR 0054).
Animation_Encoder :: struct {
	raw:   ^c.AnimationEncoder,
	frame: int, // bytes a frame
}

// An encoder for .Png (an animated PNG) or .Gif, played loops times, 0 for
// ever. ok is false, the reason on stderr, for another format or a size it
// cannot hold.
// Rust: AnimationEncoder::new.
animation_encoder_new :: proc(format: Image_Format, w, h: u32, loops: u32 = 0) -> (enc: Animation_Encoder, ok: bool) {
	raw := c.animation_encoder_new(u32(format), w, h, loops)
	if raw == nil do return {}, false
	return {raw, int(w) * int(h) * 4}, true
}

// Adds a frame that shows for seconds. False for pixels that are not a
// frame's size, or a time that is negative or not finite.
// Rust: AnimationEncoder::add.
animation_encoder_add :: proc(enc: Animation_Encoder, rgba: []u8, seconds: f64) -> bool {
	if enc.raw == nil || len(rgba) != enc.frame do return false
	return c.animation_encoder_add(enc.raw, raw_data(rgba), seconds)
}

// The file's bytes, copied into the context allocator: delete(bytes) when
// done. The encoder is spent either way; ok is false with no frame added.
// Rust: AnimationEncoder::finish.
animation_encoder_finish :: proc(enc: Animation_Encoder) -> (bytes: []u8, ok: bool) {
	n: uint
	p := c.animation_encoder_finish(enc.raw, &n)
	if p == nil do return nil, false
	defer c.pixels_free(p)
	return slice.clone(p[:n]), true
}

// Drops an encoder without writing it.
animation_encoder_free :: proc(enc: Animation_Encoder) {
	c.animation_encoder_free(enc.raw)
}

// Every frame of an animated GIF, PNG (APNG) or WebP, each the whole canvas,
// with the seconds each shows; loops is how many times it plays, 0 for ever.
// Copied into the context allocator: animation_free when done.
Animation :: struct {
	w, h:   u32,
	frames: [][]u8,
	delays: []f64,
	loops:  u32,
}

// Rust: kui_native::decode_animation.
decode_animation :: proc(bytes: []u8) -> (a: Animation, ok: bool) {
	count: u32
	delays: [^]f64
	p := c.decode_animation(raw_data(bytes), uint(len(bytes)), &a.w, &a.h, &count, &a.loops, &delays)
	if p == nil do return {}, false
	defer c.pixels_free(p)
	size := int(a.w) * int(a.h) * 4
	a.frames = make([][]u8, int(count))
	for &f, i in a.frames do f = slice.clone(p[i * size:(i + 1) * size])
	a.delays = slice.clone(delays[:int(count)])
	return a, true
}

animation_free :: proc(a: Animation) {
	for f in a.frames do delete(f)
	delete(a.frames)
	delete(a.delays)
}

// Which frame shows elapsed seconds after the animation started, and the
// seconds after the start the next is due (infinity once played out).
// Rust: Animation::at.
animation_at :: proc(a: Animation, elapsed: f64) -> (index: u32, next: f64) {
	index = c.animation_at(raw_data(a.delays), u32(len(a.delays)), a.loops, elapsed, &next)
	return
}

// What is kept of the recording - the last one, or the frames the Perf tab
// shows - as kui-trace bytes, a .kuitrace file's (ADR 0058). Borrowed until
// the next perf_trace call, as kui.h says; slice.clone to keep them.
// Rust: Core::perf_trace.
perf_trace :: proc(ui: ^Ui) -> []u8 {
	n: uint
	p := c.perf_trace(ui, &n)
	if p == nil do return nil
	return p[:n]
}

// The runner's waker (ADR 0058, decision 14): what a thread that changed
// what the view shows holds to get a frame drawn. Any thread may use it, at
// any rate; free each with waker_free.
Waker :: struct {
	raw: ^c.Waker,
}

// The runner's waker, from a run's view context. ok is false on any other -
// a standalone context's host wakes its own loop, and names it with
// note_wake.
// Rust: Waker::named.
waker :: proc(ui: ^Ui) -> (w: Waker, ok: bool) {
	raw := c.waker(ui)
	return {raw}, raw != nil
}

// Asks every window for a frame; wakes coalesce into the loop's next turn.
waker_wake :: proc(w: Waker) {
	c.waker_wake(w.raw)
}

// Asks every window for a frame ms from now, the earliest asked for winning:
// 0 or less is now, infinity never.
waker_wake_at :: proc(w: Waker, ms: f64) {
	c.waker_wake_at(w.raw, ms)
}

// waker_wake under a name: the next frame's causes read "wake: name".
waker_wake_named :: proc(w: Waker, name: string) {
	c.waker_wake_named(w.raw, name)
}

// Another handle on the same loop, for another thread.
waker_clone :: proc(w: Waker) -> Waker {
	return {c.waker_clone(w.raw)}
}

waker_free :: proc(w: Waker) {
	c.waker_free(w.raw)
}
