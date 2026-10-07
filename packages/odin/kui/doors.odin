package kui

import c "c"

// The doors the generator cannot lower from the C signature alone, each
// because the header says something in prose that the types do not.

// The pixels behind an image handle, for a renderer meeting a texture quad:
// RGBA, w * h * 4 bytes, borrowed until the image is updated or removed.
// Rust: Core::image_pixels.
image_pixels :: proc(ui: ^Ui, id: Image) -> (w, h: u32, rgba: []u8, ok: bool) {
	p: [^]u8
	c.image_pixels(ui, u64(id), &w, &h, &p) or_return
	return w, h, p[:w * h * 4], true
}
