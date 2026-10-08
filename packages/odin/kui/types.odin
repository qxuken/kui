package kui

import c "c"

// The types generated.odin builds on: the ones with helpers, the ones that
// cross to C as they are (each pinned to its kui/c mirror below), and the
// lowering helpers Spec's generated spec_to_c calls. The enums are
// generated, from kui.h's constants.

// The tree builder's handle: the C context, under the name a view reads.
Ui :: c.Ctx

// 0xRRGGBBAA, as every colour in kui is. `rgb(0x3b82f6)` is opaque.
Color :: u32

rgb :: proc "contextless" (hex: u32) -> Color {return hex << 8 | 0xff}
rgba :: proc "contextless" (hex: u32) -> Color {return hex}
// The same colour at another alpha, 0..1.
alpha :: proc "contextless" (col: Color, a: f32) -> Color {
	return col &~ 0xff | Color(clamp(a, 0, 1) * 255 + 0.5)
}

// Handles minted by the registration doors (image_add, font_add,
// sound_add, fragment_add, play); distinct so one cannot be passed where
// another is meant.
Image :: distinct u64
Font :: distinct u64
Sound :: distinct u64
Fragment :: distinct u64
Playback :: distinct u64

// -- Sizes ------------------------------------------------------------------

// One axis of a node's size. The zero value is Fit: as big as the content.
Size :: struct {
	tag:   Sizing_Tag,
	value: f32,
}

FIT :: Size{}
GROW :: Size{.Grow, 1}

px :: proc(n: f32) -> Size {return transmute(Size)c.size_px(n)}
pct :: proc(n: f32) -> Size {return transmute(Size)c.size_pct(n)}
// Take the room left over, shared by weight with the other growers.
grow :: proc "contextless" (weight: f32 = 1) -> Size {return {.Grow, weight}}

// A size from its spelling: "fit", "grow", "120", "50%",
// "clamp(400px, 80%, 1000px)", nested min()/max(). Build one per layout
// rather than one per frame (kui.h, "Size expressions"); ok is false, and
// the size Fit, when the spelling is not a size.
size :: proc(spelling: string) -> (s: Size, ok: bool) #optional_ok {
	out: c.Sizing
	ok = c.size_parse(spelling, &out)
	return transmute(Size)out, ok
}

// CSS's min(), max() and clamp() over sizes.
size_min :: proc(args: ..Size) -> Size {
	return transmute(Size)c.size_min(([^]c.Sizing)(raw_data(args)), uint(len(args)))
}
size_max :: proc(args: ..Size) -> Size {
	return transmute(Size)c.size_max(([^]c.Sizing)(raw_data(args)), uint(len(args)))
}
clamp_size :: proc(lo, target, hi: Size) -> Size {
	return transmute(Size)c.size_clamp(transmute(c.Sizing)lo, transmute(c.Sizing)target, transmute(c.Sizing)hi)
}

// -- Layout -----------------------------------------------------------------

// Padding, one per side. `pad(16)` and `pad(16, 8)` build the usual ones.
Pad :: struct {
	l, r, t, b: f32,
}

pad :: proc {
	pad_all,
	pad_xy,
}
pad_all :: proc "contextless" (n: f32) -> Pad {return {n, n, n, n}}
pad_xy :: proc "contextless" (x, y: f32) -> Pad {return {x, x, y, y}}

// Out-of-flow placement: the node's (self_x, self_y) point is put on the
// anchor's (anchor_x, anchor_y) point, then moved by (dx, dy). `fit` flips
// or clamps it to stay on screen; `clip` keeps a Parent float inside its
// parent's clip. `float_preset("below")` fills one from a name.
Float :: struct {
	mode:               Float_Mode,
	anchor_x, anchor_y: Align,
	self_x, self_y:     Align,
	dx, dy:             f32,
	fit:                bool,
	clip:               bool,
}

// A preset placement by name, as kui_spec_float_preset knows them
// ("below", "above", "right", "center", ...); false for a name it does not.
float_preset :: proc(name: string) -> (f: Float, ok: bool) {
	s: c.Spec
	c.spec_float_preset(&s, name) or_return
	return {
			mode = Float_Mode(s.float_mode),
			anchor_x = Align(s.float_anchor_x),
			anchor_y = Align(s.float_anchor_y),
			self_x = Align(s.float_self_x),
			self_y = Align(s.float_self_y),
			dx = s.float_dx,
			dy = s.float_dy,
			fit = s.float_fit != 0,
			clip = s.float_clip != 0,
		},
		true
}

// -- Paint and motion -------------------------------------------------------

Gradient_Stop :: struct {
	color: Color,
	at:    f32, // 0..1; below 0 is spaced between its neighbours
}

Gradient :: struct {
	kind:  Gradient_Kind,
	angle: f32,
	at:    [2]f32, // a radial gradient's centre, 0..1 of the box
	stops: []Gradient_Stop,
}

Keyframe :: struct {
	set:           Keyframe_Slots,
	at:            f32,
	width, height: Size,
	bg:            Color,
	radius:        f32,
	opacity:       f32,
	rotate:        f32, // turns clockwise (ADR 0043)
	scale:         f32, // a uniform scale about the pivot
}

// Where an entering node starts from, or a leaving one ends at; `set` names
// the slots that move.
Enter :: struct {
	set:           Enter_Slots,
	dx, dy:        f32,
	width, height: Size,
	bg:            Color,
	radius:        f32,
	opacity:       f32,
	rotate:        f32, // turns clockwise (ADR 0043)
	scale:         f32, // a uniform scale; 0 grows the subtree in from nothing
}

// -- Text and cells ---------------------------------------------------------

// One run of a rich_text paragraph.
Span :: struct {
	text:            string,
	color:           Color, // 0: the paragraph's
	flags:           Span_Flags,
	bg:              Color,
	underline_color: Color,
	underline_style: Underline_Style,
	bg_radius:       f32,
	// The span's own face with .Family in flags (inline code in .Mono),
	// its own size in px (0 the paragraph's), and a registered font that
	// overrides family.
	family:          Font_Family,
	size:            f32,
	font:            Font,
}

// One cell of a `cells` grid: a character and its colours.
Cell :: struct {
	ch:    rune,
	fg:    Color,
	bg:    Color,
	flags: Cell_Flags,
	ul:    Color, // the underline's own colour; 0 = fg
}

// -- Input ------------------------------------------------------------------

// Which of a key's twins it was.
Key_Location :: enum u32 {
	Standard,
	Left,
	Right,
	Numpad,
}

// The word the key doors take as `kmods`: the held modifiers, the lock
// keys, whether the layout is not Latin, and which twin the key was.
Key_Mods :: bit_field u32 {
	shift:     bool         | 1,
	ctrl:      bool         | 1,
	alt:       bool         | 1,
	super:     bool         | 1,
	caps_lock: bool         | 1,
	num_lock:  bool         | 1,
	nonlatin:  bool         | 1,
	_reserved: u32          | 1,
	location:  Key_Location | 2,
}

#assert(c.KMOD_SHIFT == 1 << 0 && c.KMOD_CTRL == 1 << 1 && c.KMOD_ALT == 1 << 2 && c.KMOD_SUPER == 1 << 3)
#assert(c.KLOCK_CAPS == 1 << 4 && c.KLOCK_NUM == 1 << 5 && c.KLAYOUT_NONLATIN == 1 << 6)
#assert(c.KLOC_LEFT == 1 << 8 && c.KLOC_RIGHT == 2 << 8 && c.KLOC_NUMPAD == 3 << 8)

// The main window's id; a window the app declares gets its own.
WINDOW_MAIN :: u32(c.WINDOW_MAIN)

// -- Draw data --------------------------------------------------------------

// One quad of a frame's display list, as a renderer reads it: physical
// pixels, linear colours. kui.h's KuiQuad documents each field.
Quad :: struct {
	x, y, w, h:   f32,
	color:        [4]f32,
	border_color: [4]f32,
	radius:       [4]f32,
	border_w:     f32,
	blur:         f32,
	kind:         Quad_Kind,
	clip:         u32, // an index into Draw_Data.clips; 0 clips nothing
	uv:           [4]u32,
}

// -- Run --------------------------------------------------------------------

// The window kui.run opens; zero is 960x640 with the OS's chrome.
Run_Config :: struct {
	width, height: f32,
	min_w, min_h:  f32,
	max_w, max_h:  f32,
	chrome:        Chrome,
	text_aa:       Text_AA,
	diagnostics:   Diagnostics,
	frame_latency: u32,
	// What shows through the window's transparent pixels (backlog F126);
	// .Opaque, the zero, is none. What it got is ctx_backdrop, in the view.
	backdrop:      Backdrop,
}

// -- Lowering, for the generated spec_to_c -----------------------------------

// The message values a call borrows: encoded for it and freed after it,
// which is when the core has taken its copies.
Scratch :: struct {
	values: [dynamic]^c.Value,
}

borrow :: proc(s: ^Scratch, msg: any) -> ^c.Value {
	v := to_value(msg)
	if v != nil {
		if s.values == nil do s.values = make([dynamic]^c.Value, context.temp_allocator)
		append(&s.values, v)
	}
	return v
}

scratch_free :: proc(s: ^Scratch) {
	for v in s.values do c.value_free(v)
}

// A lower clamp: Fit is the node's fit size, a length or a size expression
// what it says, and a declared 0 kui.h's KUI_MIN_NONE.
lower_min :: proc(v: Maybe(Size), px: ^f32, expr: ^c.Sizing) {
	s, ok := v.?
	if !ok do return
	#partial switch s.tag {
	case .Fit:
		px^ = c.MIN_FIT
	case .Fixed:
		px^ = s.value if s.value != 0 else c.MIN_NONE
	case .Percent, .Calc:
		expr^ = transmute(c.Sizing)s
	}
}

// An upper clamp: a length or a size expression.
lower_max :: proc(v: Maybe(Size), px: ^f32, expr: ^c.Sizing) {
	s, ok := v.?
	if !ok do return
	#partial switch s.tag {
	case .Fixed:
		px^ = s.value
	case .Percent, .Calc:
		expr^ = transmute(c.Sizing)s
	}
}

// One corner's radius: once any corner is set, every corner is its own,
// and an unset one keeps the uniform radius.
lower_corner :: proc(v: Maybe(f32), radius: f32, dst: ^f32, per_corner: ^u32) {
	r, set := v.?
	dst^ = r if set else radius
	if set do per_corner^ = 1
}

lower_float :: proc(f: Float, out: ^c.Spec) {
	out.float_mode = u32(f.mode)
	out.float_anchor_x, out.float_anchor_y = u32(f.anchor_x), u32(f.anchor_y)
	out.float_self_x, out.float_self_y = u32(f.self_x), u32(f.self_y)
	out.float_dx, out.float_dy = f.dx, f.dy
	out.float_fit = 1 if f.fit else 0
	out.float_clip = 1 if f.clip else 0
}

// The gradient a spec points at, alive until the temp allocator is freed.
lower_gradient :: proc(g: Gradient) -> ^c.Gradient {
	out := new(c.Gradient, context.temp_allocator)
	out^ = {
		kind      = u32(g.kind),
		angle     = g.angle,
		at_x      = g.at.x,
		at_y      = g.at.y,
		stops     = ([^]c.GradientStop)(raw_data(g.stops)),
		stops_len = uint(len(g.stops)),
	}
	return out
}

// -- The twins are the C structs ----------------------------------------------
//
// Size, Keyframe, Enter, Gradient_Stop, Span, Cell, Quad and Run_Config
// cross to C by a cast, so each is pinned field by field to its generated mirror,
// which layout.odin pins to the C compiler.

#assert(size_of(Size) == size_of(c.Sizing))
#assert(offset_of(Size, value) == offset_of(c.Sizing, value))

#assert(size_of(Keyframe) == size_of(c.Keyframe))
#assert(offset_of(Keyframe, at) == offset_of(c.Keyframe, at))
#assert(offset_of(Keyframe, width) == offset_of(c.Keyframe, width))
#assert(offset_of(Keyframe, height) == offset_of(c.Keyframe, height))
#assert(offset_of(Keyframe, bg) == offset_of(c.Keyframe, bg))
#assert(offset_of(Keyframe, radius) == offset_of(c.Keyframe, radius))
#assert(offset_of(Keyframe, opacity) == offset_of(c.Keyframe, opacity))
#assert(offset_of(Keyframe, rotate) == offset_of(c.Keyframe, rotate))
#assert(offset_of(Keyframe, scale) == offset_of(c.Keyframe, scale))

#assert(size_of(Enter) == size_of(c.Enter))
#assert(offset_of(Enter, dx) == offset_of(c.Enter, dx))
#assert(offset_of(Enter, dy) == offset_of(c.Enter, dy))
#assert(offset_of(Enter, width) == offset_of(c.Enter, width))
#assert(offset_of(Enter, height) == offset_of(c.Enter, height))
#assert(offset_of(Enter, bg) == offset_of(c.Enter, bg))
#assert(offset_of(Enter, radius) == offset_of(c.Enter, radius))
#assert(offset_of(Enter, opacity) == offset_of(c.Enter, opacity))
#assert(offset_of(Enter, rotate) == offset_of(c.Enter, rotate))
#assert(offset_of(Enter, scale) == offset_of(c.Enter, scale))

#assert(size_of(Gradient_Stop) == size_of(c.GradientStop))
#assert(offset_of(Gradient_Stop, at) == offset_of(c.GradientStop, at))

#assert(size_of(Span) == size_of(c.Span))
#assert(offset_of(Span, color) == offset_of(c.Span, color))
#assert(offset_of(Span, flags) == offset_of(c.Span, flags))
#assert(offset_of(Span, bg) == offset_of(c.Span, bg))
#assert(offset_of(Span, underline_color) == offset_of(c.Span, underline_color))
#assert(offset_of(Span, underline_style) == offset_of(c.Span, underline_style))
#assert(offset_of(Span, bg_radius) == offset_of(c.Span, bg_radius))
#assert(offset_of(Span, family) == offset_of(c.Span, family))
#assert(offset_of(Span, size) == offset_of(c.Span, size))
#assert(offset_of(Span, font) == offset_of(c.Span, font))

#assert(size_of(Cell) == size_of(c.Cell))
#assert(offset_of(Cell, fg) == offset_of(c.Cell, fg))
#assert(offset_of(Cell, bg) == offset_of(c.Cell, bg))
#assert(offset_of(Cell, flags) == offset_of(c.Cell, flags))
#assert(offset_of(Cell, ul) == offset_of(c.Cell, ul))

#assert(size_of(Quad) == size_of(c.Quad))
#assert(offset_of(Quad, color) == offset_of(c.Quad, color))
#assert(offset_of(Quad, border_color) == offset_of(c.Quad, border_color))
#assert(offset_of(Quad, radius) == offset_of(c.Quad, radius))
#assert(offset_of(Quad, border_w) == offset_of(c.Quad, border_w))
#assert(offset_of(Quad, blur) == offset_of(c.Quad, blur))
#assert(offset_of(Quad, kind) == offset_of(c.Quad, kind))
#assert(offset_of(Quad, clip) == offset_of(c.Quad, clip))
#assert(offset_of(Quad, uv) == offset_of(c.Quad, uv))

#assert(size_of(Run_Config) == size_of(c.RunConfig))
#assert(offset_of(Run_Config, min_w) == offset_of(c.RunConfig, min_w))
#assert(offset_of(Run_Config, max_w) == offset_of(c.RunConfig, max_w))
#assert(offset_of(Run_Config, chrome) == offset_of(c.RunConfig, chrome))
#assert(offset_of(Run_Config, text_aa) == offset_of(c.RunConfig, text_aa))
#assert(offset_of(Run_Config, diagnostics) == offset_of(c.RunConfig, diagnostics))
#assert(offset_of(Run_Config, frame_latency) == offset_of(c.RunConfig, frame_latency))
#assert(offset_of(Run_Config, backdrop) == offset_of(c.RunConfig, backdrop))
