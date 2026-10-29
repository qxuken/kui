package kui

import c "c"

// kui's own shader and instance packing, for a renderer of your own
// (backlog F234): renderer_wgsl is the shader, instance_stride and
// instance_attributes the vertex layout, pack_instances the frame. Build
// the pipeline from the layout read here and a later library's needs no
// change. kui.h's "Drawing it yourself" is the whole of it; on
// vendor:wgpu the pipeline around these is the kui/wgpu package (backlog
// F263), so a host with its own device draws kui in three calls.

// The shader that draws pack_instances' instances, WGSL: dual for a device
// with dual-source blending (LCD subpixel text), else the single variant.
// The library's own text, valid while it is loaded.
renderer_wgsl :: proc(dual := false) -> string {
	return c.renderer_wgsl(c.WGSL_DUAL if dual else c.WGSL_SINGLE)
}

// The bytes one packed instance takes: the vertex buffer's stride.
instance_stride :: proc() -> u32 {
	return c.instance_stride()
}

// One vertex attribute of the instance: its shader location, how many f32
// it is (2 or 4) and its byte offset.
Instance_Attribute :: struct {
	location: u32,
	floats:   u32,
	offset:   u32,
}

instance_attributes :: proc(allocator := context.temp_allocator) -> []Instance_Attribute {
	n := c.instance_attributes(nil, 0)
	out := make([]Instance_Attribute, n, allocator)
	c.instance_attributes((^u32)(raw_data(out)), n)
	return out
}

// The finished frame's quads as the shader's instances, instance_stride()
// bytes each, in `allocator`.
pack_instances :: proc(ui: ^Ui, allocator := context.temp_allocator) -> []u8 {
	n := c.pack_instances(ui, nil, 0)
	buf := make([]u8, n * uint(c.instance_stride()), allocator)
	c.pack_instances(ui, raw_data(buf), uint(len(buf)))
	return buf
}
