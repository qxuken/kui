// The scene corpus from Odin: examples/c/tools/conformance.c ported through
// the typed layer (package kui), so the Odin binding is held to the lowering
// the Rust, Lua, C and Node bindings are - every scene of kui_core's
// conformance corpus rebuilt through the doors and the Spec an Odin app
// would use, its steps replayed, and the report diffed byte for byte against
// the reference kui-core wrote. A tool, not an example.
//
//   cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt
//   nu scripts/odin.nu run conformance [target/conformance.txt]
//
// The reference path is the first argument, else KUI_CONFORMANCE, else
// target/conformance.txt. It is generated, never checked in: the quad
// digests cover real glyph geometry, so they hold only for the machine and
// fonts that produced them. Exit 1 when it cannot be read, so a round that
// lost the step producing it goes red rather than skipping.
//
// Where the C sets a KuiSpec field this sets the Spec field the prop schema
// names, and lets the generated spec_to_c lower it: that lowering is what
// the corpus is here to hold. (It found the polygon and polyline doors
// passing their float count where kui.h counts points; they take
// [][2]f32 now.)
package conformance

import "core:fmt"
import "core:os"
import "core:strconv"
import "core:strings"
import "core:unicode/utf8"
import kui "../../../packages/odin/kui"

// -- Messages -----------------------------------------------------------------

// The {kind = k} payload conformance.c's conf_kind builds: a map, so the
// kind can be any string the corpus names, not only an Odin type's name.
Kind_Msg :: map[string]string

msg :: proc(k: string) -> Kind_Msg {
	m := make(Kind_Msg, context.temp_allocator)
	m["kind"] = k
	return m
}

// -- Fixtures -----------------------------------------------------------------

// The corpus fixtures, registered in the order conformance::fixtures uses so
// the handles - and the atlas the image lands in - come out the same.
Fixtures :: struct {
	image:    kui.Image,
	// A copy of the image, updated in place to 8x2 grey before the first
	// frame, so it is texture-backed.
	stream:   kui.Image,
	sound:    kui.Sound,
	fragment: kui.Fragment,
	// The fragment that reads its image.
	sampler:  kui.Fragment,
	// An image registered and removed: the handle the dead-handle rule is
	// pinned on.
	dead:     kui.Image,
}

// conformance::FRAGMENT_WGSL, character for character.
FRAGMENT_WGSL ::
	"fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n" +
	"    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);\n" +
	"    let base = mix(params[0], params[1], t);\n" +
	"    let d = kui_sd_rounded_box(in.local - in.size * 0.5, in.size * 0.5, vec4<f32>(params[2].x));\n" +
	"    let ring = 1.0 - smoothstep(-KUI_AA, KUI_AA, abs(d) - params[2].y);\n" +
	"    return vec4<f32>(mix(base.rgb, params[3].rgb, ring), base.a);\n" +
	"}"

// conformance::FRAGMENT_IMAGE_WGSL, character for character.
FRAGMENT_IMAGE_WGSL ::
	"fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n" +
	"    let uv = in.local / max(in.size, vec2<f32>(1.0));\n" +
	"    let c = kui_sample(uv) * params[0];\n" +
	"    return vec4<f32>(c.rgb, c.a * step(1.0, in.image.z));\n" +
	"}"

// conformance::FRAGMENT_IMAGE_PARAMS.
fragment_image_params := [4]f32{1.0, 0.5, 0.25, 1.0}

// conformance::FRAGMENT_PARAMS and FRAGMENT_PARAMS_LONG.
fragment_params := [16]f32 {
	0.85, 0.30, 0.25, 1.0,
	0.20, 0.45, 0.90, 1.0,
	10.0, 2.0, 0.0, 0.0,
	1.0, 1.0, 1.0, 1.0,
}
fragment_params_long := [18]f32 {
	0.1, 0.2, 0.3, 1.0, 0.4, 0.5, 0.6, 1.0, 4.0,
	1.0, 0.0, 0.0, 0.9, 0.9, 0.2, 1.0, 7.0, 8.0,
}

fixtures :: proc(ui: ^kui.Ui) -> (f: Fixtures) {
	rgba: [4 * 4 * 4]u8
	for &b in rgba do b = 0xff
	f.image = kui.image_add(ui, 4, 4, rgba[:])
	f.stream = kui.image_add(ui, 4, 4, rgba[:])
	grey: [8 * 2 * 4]u8
	for &b, i in grey do b = 0xff if i % 4 == 3 else 0x80
	kui.image_update(ui, f.stream, 8, 2, grey[:])
	wav := "RIFF....WAVE"
	f.sound = kui.sound_add(ui, transmute([]u8)wav)
	f.fragment = kui.fragment_add(ui, FRAGMENT_WGSL)
	f.sampler = kui.fragment_add(ui, FRAGMENT_IMAGE_WGSL)
	f.dead = kui.image_add(ui, 4, 4, rgba[:])
	kui.image_remove(ui, f.dead)
	return
}

// -- The scenes, in Odin --------------------------------------------------------

scene_layout :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(8), gap = 6, bg = 0x14161eff}) {
		card := kui.Spec {
			key           = "card",
			dir           = .Row,
			pad           = {l = 12, r = 10, t = 6, b = 4},
			gap           = 4,
			bg            = 0x202030ff,
			border_w      = 2,
			border_color  = 0x2a2d3aff,
			radius        = 5,
			opacity       = 0.75,
			shadow_color  = 0x00000066,
			shadow_blur   = 8,
			shadow_y      = 3,
			shadow_spread = 1,
			width         = kui.px(180),
			height        = kui.px(40),
		}
		if kui.box(ui, card) {
			kui.text(ui, "ab", {size = 12})
			kui.text(ui, "cd", {size = 12})
		}
		// What PadShorthand{x:9, y:3, b:1} resolves to, as the C writes it.
		if kui.box(ui, {pad = {l = 9, r = 9, t = 3, b = 1}, bg = 0x2a2d3aff}) {}
		spans := []kui.Span {
			{text = "a "},
			{text = "b", color = 0x73d98cff, flags = {.Bold}},
			{text = " c", flags = {.Italic}},
			{text = " d()", flags = {.Family}, family = .Mono},
			{text = " E", size = 20},
		}
		kui.rich_text(ui, spans, {size = 13})
	}
}

// conformance::WRAP_BOXES: 92px of content and a 6px gap put 30 + 40 on the
// first line and 50 + 20 on the second.
scene_wrap :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	boxes := [4][2]f32{{30, 12}, {40, 16}, {50, 20}, {20, 24}}
	row := kui.Spec {
		dir           = .Row,
		wrap_children = true,
		pad           = kui.pad(4),
		gap           = 6,
		cross_gap     = 10,
		width         = kui.px(100),
		bg            = 0x101018ff,
	}
	if kui.box(ui, row) {
		for b in boxes {
			if kui.box(ui, {width = kui.px(b[0]), height = kui.px(b[1]), bg = 0x30344aff}) {}
		}
	}
}

// conformance::build_align: the three spreads (backlog C13), a baseline row
// of a 12 and a 20 px text and a box, and a ratio sizing each axis (C14).
align_square :: proc(ui: ^kui.Ui) {
	if kui.box(ui, {width = kui.px(10), height = kui.px(10), bg = 0x30344aff}) {}
}

scene_align :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	spreads := [3]kui.Align{.Space_Between, .Space_Around, .Space_Evenly}
	if kui.box(ui, {pad = kui.pad(4), gap = 6, width = kui.px(128), bg = 0x101018ff}) {
		for spread in spreads {
			if kui.row(ui, {width = kui.px(120), main_align = spread}) {
				for _ in 0 ..< 3 do align_square(ui)
			}
		}
		if kui.row(ui, {gap = 4, cross_align = .Baseline}) {
			kui.text(ui, "ab", {size = 12})
			kui.text(ui, "cd", {size = 20})
			align_square(ui)
		}
		if kui.box(ui, {width = kui.GROW, aspect_ratio = 4, bg = 0x3b5bd4ff}) {}
		if kui.box(ui, {height = kui.px(12), aspect_ratio = 2, bg = 0x73d98cff}) {}
	}
}

// conformance::build_stock_controls (docs/adr/0034): every toggle keyed by
// its text, as kui_checkbox keys it.
scene_stock_controls :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(8), gap = 8}) {
		kui.slider(ui, "Volume", 30, 0, 100, msg("vol"), {width = kui.px(216), value_step = 10})
		kui.checkbox(ui, "Mute", false, msg("mute"))
		kui.checkbox(ui, "Sync", true, msg("sync"))
		kui.checkbox(ui, "All", false, msg("all"), {mixed = true})
		if kui.radio_group(ui, "Theme") {
			kui.radio(ui, "Light", false, msg("light"))
			kui.radio(ui, "Dark", true, msg("dark"))
		}
		kui.toggle(ui, "Wi-Fi", true, msg("wifi"))
	}
}

// conformance::build_table (ADR 0033): a table column of a fit header row
// and three grow rows, each a bare text, a fixed box and a grow box.
scene_table :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	labels := [3]string{"abc", "abcde", "ab"}
	rows := [3][2]f32{{30, 10}, {50, 12}, {20, 8}}
	style := kui.Text_Style {
		size = 12,
	}
	table := kui.Spec {
		key        = "table",
		dir        = .Table,
		pad        = kui.pad(4),
		gap        = 2,
		width      = kui.px(200),
		bg         = 0x101018ff,
		rules      = 0x2b3350ff,
		rule_width = 1,
	}
	if kui.box(ui, table) {
		if kui.row(ui, {gap = 6}) {
			kui.text(ui, "name", style)
			kui.text(ui, "w", style)
		}
		for r, i in rows {
			if kui.row(ui, {gap = 6, width = kui.GROW}) {
				kui.text(ui, labels[i], style)
				if kui.box(ui, {width = kui.px(r[0]), height = kui.px(r[1]), bg = 0x30344aff}) {}
				if kui.box(ui, {width = kui.GROW, height = kui.px(r[1]), bg = 0x3b5bd4ff}) {}
			}
		}
	}
}

// An i3-style tab bar twice: grow tabs with their fit size as their floor,
// in a bar with room and in one without. Mirrors conformance::TAB_*.
scene_tabs :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	roomy := [2][2]f32{{30, 12}, {50, 8}}
	crowded := [4]f32{60, 70, 80, 90}
	if kui.box(ui, {pad = kui.pad(4), gap = 4}) {
		bar := kui.Spec {
			key    = "roomy",
			dir    = .Row,
			width  = kui.px(200),
			height = kui.px(20),
			bg     = 0x101018ff,
		}
		if kui.box(ui, bar) {
			for r in roomy {
				tab := kui.Spec {
					width      = kui.GROW,
					min_width  = kui.FIT,
					height     = kui.pct(50),
					min_height = kui.FIT,
					bg         = 0x30344aff,
				}
				if kui.box(ui, tab) {
					if kui.box(ui, {width = kui.px(r[0]), height = kui.px(r[1]), bg = 0x3b5bd4ff}) {}
				}
			}
		}
		bar.key = "crowded"
		bar.overflow = {.Scroll_X}
		if kui.box(ui, bar) {
			for w in crowded {
				if kui.box(ui, {width = kui.GROW, min_width = kui.FIT, height = kui.GROW, bg = 0x30344aff}) {
					if kui.box(ui, {width = kui.px(w), height = kui.px(12), bg = 0x3b5bd4ff}) {}
				}
			}
		}
	}
}

scene_overflow :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(4), overflow = {.Clip}}) {
		list := kui.Spec {
			key      = "list",
			width    = kui.px(120),
			height   = kui.px(60),
			gap      = 4,
			overflow = {.Scroll_Y},
			radius   = 8,
			bg       = 0x101018ff,
		}
		if kui.box(ui, list) {
			for i in 0 ..< 6 {
				item := kui.Spec {
					key    = fmt.tprintf("i%d", i),
					width  = kui.px(100),
					height = kui.px(20),
					bg     = 0x30344aff,
				}
				if kui.box(ui, item) {}
			}
		}
	}
}

// conformance::build_scrollbar: the four scrollbar rows on three scrollers
// of the same list - hidden, styled (an 8 px thumb in two colours), auto.
scrollbar_list :: proc(ui: ^kui.Ui, list: kui.Spec) {
	if kui.box(ui, list) {
		for i in 0 ..< 6 {
			item := kui.Spec {
				key    = fmt.tprintf("i%d", i),
				width  = kui.px(80),
				height = kui.px(20),
				bg     = 0x30344aff,
			}
			if kui.box(ui, item) {}
		}
	}
}

scene_scrollbar :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui, {pad = kui.pad(10), gap = 10}) {
		base := kui.Spec {
			width    = kui.px(90),
			height   = kui.px(60),
			overflow = {.Scroll_Y},
			bg       = 0x101018ff,
		}
		hidden := base
		hidden.key = "hidden"
		hidden.scrollbar = .Hidden
		scrollbar_list(ui, hidden)
		styled := base
		styled.key = "styled"
		styled.scrollbar_width = 8
		styled.scrollbar_color = 0x3b5bd4ff
		styled.scrollbar_active_color = 0xffcc00ff
		scrollbar_list(ui, styled)
		auto_bar := base
		auto_bar.key = "auto"
		auto_bar.scrollbar = .Auto
		scrollbar_list(ui, auto_bar)
	}
}

// conformance::build_tokens (ADR 0027): the table declared every build -
// with `surface` in it, refused as `reserved-token` - then read back by
// name, as the C does, since an Odin prop carries no reference either.
scene_tokens :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	colors := []kui.Color_Token {
		{name = "peach", light = 0xffcc99ff, dark = 0xffcc99ff},
		{name = "ink", light = 0x202020ff, dark = 0xe0e0e0ff},
		{name = "surface", light = 0xff0000ff, dark = 0xff0000ff},
	}
	lengths := []kui.Length_Token{{name = "side_w", value = 60}, {name = "gap", value = 8}, {name = "big", value = 16}}
	kui.tokens_set(ui, colors, lengths)
	// conformance::TOKEN_DERIVED (ADR 0028): `bad` dropped by the core with
	// `unknown-token`; `up` derives from `surface`, which the declaration
	// above lost to the role.
	derived := []kui.Derived_Token {
		{name = "lit", from = "peach", ops = {{op = .Lift, t = 0.3}}},
		{name = "dim", from = "ink", ops = {{op = .Mix, t = 0.5, other = "peach"}, {op = .Darken, t = 0.5}}},
		{name = "up", from = "surface", ops = {{op = .Raise, t = 0.25}}},
		{name = "deep", from = "lit", ops = {{op = .Alpha, t = 0.5}}},
		{name = "read", from = "peach", ops = {{op = .Readable, t = 4.5, other = "ink"}}},
		{name = "bad", from = "nothing", ops = {{op = .Lift, t = 0.1}}},
	}
	kui.tokens_derive(ui, derived)
	peach := kui.token_color(ui, "peach")
	ink := kui.token_color(ui, "ink")
	surface := kui.token_color(ui, "surface")
	nothing := kui.token_color(ui, "nothing")
	side_w := kui.token_length(ui, "side_w")
	gap := kui.token_length(ui, "gap")
	big := kui.token_length(ui, "big")
	radius := kui.token_length(ui, "radius")
	if kui.row(ui, {pad = {l = gap, r = 10, t = 10, b = 10}, gap = gap}) {
		cell := kui.Spec {
			width  = kui.px(side_w),
			height = kui.px(30),
		}
		cell.key = "peach"
		cell.bg = peach
		if kui.box(ui, cell) {}
		cell.key = "ink"
		cell.bg = ink
		cell.border_w = gap
		cell.border_color = peach
		if kui.box(ui, cell) {}
		cell.border_w = 0
		cell.border_color = 0
		cell.key = "role"
		cell.bg = surface
		cell.radius = radius
		if kui.box(ui, cell) {}
		cell.radius = 0
		cell.key = "missing"
		cell.bg = nothing
		if kui.box(ui, cell) {}
		for name in ([]string{"lit", "dim", "up", "deep", "read"}) {
			cell.key = name
			cell.bg = kui.token_color(ui, name)
			if kui.box(ui, cell) {}
		}
		spans := []kui.Span{{text = "tokens"}, {text = "x", color = ink}}
		kui.rich_text(ui, spans, {size = big, color = peach})
	}
}

// Scroll anchoring (backlog C26 step 3): two scrollers of the same rows, one
// with `anchor`; phase 1 prepends a taller row to both.
scene_anchor :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui, {pad = kui.pad(10), gap = 10}) {
		keys := [2]string{"anchored", "plain"}
		for key, n in keys {
			list := kui.Spec {
				key      = key,
				width    = kui.px(90),
				height   = kui.px(60),
				overflow = {.Scroll_Y},
				bg       = 0x101018ff,
				anchor   = n == 0,
			}
			if kui.box(ui, list) {
				if phase >= 1 {
					if kui.box(ui, {key = "new", width = kui.px(80), height = kui.px(30), bg = 0x30344aff}) {}
				}
				for i in 0 ..< 6 {
					item := kui.Spec {
						key    = fmt.tprintf("i%d", i),
						width  = kui.px(80),
						height = kui.px(20),
						bg     = 0x30344aff,
					}
					if kui.box(ui, item) {}
				}
			}
		}
	}
}

// The four sizing modes in a parent of known width: 30 fixed, 25% of 200 =
// 50, fit around a 20-wide child, and grow taking the remaining 100.
scene_sizing :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(14, 6)}) {
		if kui.row(ui, {key = "bar", width = kui.px(200), height = kui.px(40), bg = 0x101018ff}) {
			if kui.box(ui, {width = kui.px(30), height = kui.px(20), bg = 0x30344aff}) {}
			if kui.box(ui, {width = kui.pct(25), height = kui.px(20), bg = 0x3b5bd4ff}) {}
			if kui.box(ui, {width = kui.FIT, height = kui.px(20), bg = 0x73d98cff}) {
				if kui.box(ui, {width = kui.px(20), height = kui.px(10), bg = 0xff0000ff}) {}
			}
			if kui.box(ui, {width = kui.GROW, height = kui.px(20), bg = 0xffcc00ff}) {}
		}
	}
}

scene_float :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(20), gap = 4}) {
		if kui.box(ui, {key = "anchor", width = kui.px(80), height = kui.px(24), bg = 0x333333ff}) {
			// The same "below" the JSX and Lua props name, resolved by the
			// same function in kui-core rather than spelled out here.
			below, _ := kui.float_preset("below")
			if kui.box(ui, {width = kui.px(40), height = kui.px(12), bg = 0xff0000ff, float = below}) {}
		}
		if kui.box(ui, {key = "nudged", width = kui.px(60), height = kui.px(20), bg = 0x444444ff}) {
			// A preset as a starting point: dx moves it sideways, and the dy
			// the preset filled in stays, so the 6px gap survives.
			nudged, _ := kui.float_preset("below")
			nudged.dx = 6
			if kui.box(ui, {width = kui.px(30), height = kui.px(10), bg = 0x0000ffff, float = nudged}) {}
		}
		// Asymmetric in every axis, and attached at (-16, 254) so that `fit`
		// has to clamp it back on screen.
		corner := kui.Spec {
			float = {
				mode = .Viewport,
				anchor_x = .Start,
				anchor_y = .End,
				self_x = .End,
				self_y = .Start,
				dx = -6,
				dy = 14,
				fit = true,
			},
			width = kui.px(10),
			height = kui.px(10),
			bg = 0x00ff00ff,
		}
		if kui.box(ui, corner) {}
	}
}

// One of scene_clip_float's two nodes: a parent-anchored float at (dx, -20)
// on the canvas that posts `key` when clicked, cut by the canvas's clip when
// `clip` is set (float_clip, ABI 19).
clip_float_node :: proc(ui: ^kui.Ui, key: string, dx: f32, clip: bool, bg: kui.Color, label: string) {
	spec := kui.Spec {
		key      = key,
		float    = {mode = .Parent, dx = dx, dy = -20, clip = clip},
		width    = kui.px(80),
		height   = kui.px(40),
		bg       = bg,
		label    = label,
		on_click = msg(key),
	}
	if kui.box(ui, spec) {}
}

// conformance::build_clip_float (backlog F90): a toolbar over a clip canvas,
// and two nodes on the canvas panned half past its top edge.
scene_clip_float :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.GROW, height = kui.GROW}) {
		toolbar := kui.Spec {
			key      = "toolbar",
			dir      = .Row,
			width    = kui.GROW,
			height   = kui.px(40),
			bg       = 0x3a3f52ff,
			label    = "Toolbar",
			on_click = msg("toolbar"),
		}
		if kui.box(ui, toolbar) {}
		canvas := kui.Spec {
			key      = "canvas",
			width    = kui.GROW,
			height   = kui.GROW,
			overflow = {.Clip},
			bg       = 0x101018ff,
		}
		if kui.box(ui, canvas) {
			clip_float_node(ui, "node", 40, true, 0x3b5bd4ff, "Node")
			clip_float_node(ui, "free", 160, false, 0x73d98cff, "Free")
		}
	}
}

// conformance::build_pixel_snap: three boxes at 40.5 by 20.25, the first two
// painted on whole pixels and the first with a hard shadow.
scene_pixel_snap :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui) {
		first := kui.Spec {
			width        = kui.px(40.5),
			height       = kui.px(20.25),
			bg           = 0xd9738cff,
			pixel_snap   = true,
			shadow_color = 0x000000ff,
		}
		if kui.box(ui, first) {}
		if kui.box(ui, {width = kui.px(40.5), height = kui.px(20.25), bg = 0x73d98cff, pixel_snap = true}) {}
		if kui.box(ui, {width = kui.px(40.5), height = kui.px(20.25), bg = 0x3b5bd4ff}) {}
	}
}

// conformance::build_clip_access (backlog F93): access rects cut to the clip.
scene_clip_access :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.GROW, height = kui.GROW}) {
		toolbar := kui.Spec {
			key      = "toolbar",
			width    = kui.GROW,
			height   = kui.px(40),
			bg       = 0x3a3f52ff,
			label    = "Toolbar",
			on_click = msg("toolbar"),
		}
		if kui.box(ui, toolbar) {}
		if kui.row(ui, {width = kui.GROW, height = kui.GROW}) {
			canvas := kui.Spec {
				key      = "canvas",
				width    = kui.GROW,
				height   = kui.GROW,
				overflow = {.Clip},
				bg       = 0x101018ff,
			}
			if kui.box(ui, canvas) {
				Node :: struct {
					key, label: string,
					dx, dy:     f32,
					clip:       bool,
				}
				nodes := [3]Node{{"cut", "Cut", 20, -20, true}, {"past", "Past", 100, -60, true}, {"free", "Free", 140, -60, false}}
				for n in nodes {
					node := kui.Spec {
						key      = n.key,
						width    = kui.px(60),
						height   = kui.px(40),
						bg       = 0x3b5bd4ff,
						label    = n.label,
						float    = {mode = .Parent, dx = n.dx, dy = n.dy, clip = n.clip},
						on_click = msg(n.key),
					}
					if kui.box(ui, node) {}
				}
			}
			list := kui.Spec {
				key      = "list",
				width    = kui.px(100),
				height   = kui.px(50),
				overflow = {.Scroll_Y},
				bg       = 0x202030ff,
			}
			if kui.box(ui, list) {
				rows := [3][2]string{{"row0", "Row 0"}, {"row1", "Row 1"}, {"row2", "Row 2"}}
				for r in rows {
					row := kui.Spec {
						key      = r[0],
						width    = kui.px(100),
						height   = kui.px(30),
						bg       = 0x73d98cff,
						label    = r[1],
						on_click = msg(r[0]),
					}
					if kui.box(ui, row) {}
				}
			}
		}
	}
}

scene_tooltip :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10)}) {
		// The `tooltip` prop: hover tracking, the accessible description, and
		// the float the close hangs below while hovered.
		tip := kui.Spec {
			key     = "tip",
			dir     = .Row,
			width   = kui.px(100),
			height  = kui.px(40),
			bg      = 0x333333ff,
			role    = .Group,
			tooltip = "a hint",
		}
		if kui.box(ui, tip) {
			kui.text(ui, "badge", {size = 12})
		}
		// `description` is the same slot without the hover tracking or the
		// float: spoken, never drawn.
		saved := kui.Spec {
			dir         = .Row,
			width       = kui.px(100),
			height      = kui.px(20),
			role        = .Button,
			label       = "Save",
			description = "Nothing to save yet",
		}
		if kui.box(ui, saved) {}
	}
}

titlebar_body :: proc(_: ^Fixtures, ui: ^kui.Ui) {
	kui.text(ui, "app", {size = 12})
}

scene_chrome :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	kui.window_title(ui, "kui conformance")
	// The root declarations with no node: on top, secure keyboard entry
	// (F85), the left Option key as Alt (F113), the input method off (F125).
	kui.set_always_on_top(ui, true)
	kui.set_secure_input(ui, true)
	kui.set_option_as_alt(ui, .Left)
	kui.set_ime_off(ui, true)
	if kui.box(ui, {gap = 6}) {
		// titlebar_with appends its own cluster after the body; the second is
		// window_buttons called directly, in a strip laid out here.
		kui.titlebar_with(ui, f, titlebar_body)
		if kui.row(ui, {width = kui.GROW, keep_focus = true}) {
			kui.window_buttons(ui)
		}
		sink := kui.Spec {
			key       = "sink",
			width     = kui.px(40),
			height    = kui.px(16),
			bg        = 0x22242cff,
			focusable = true,
			label     = "Sink",
			on_focus  = msg("sink"),
			key_focus = true,
		}
		if kui.box(ui, sink) {}
	}
}

scene_controls :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	// The panel claims the middle button (backlog F105) and leaves the
	// secondary one to its menu.
	outer := kui.Spec {
		pad             = kui.pad(10),
		gap             = 6,
		on_context_menu = msg("menu"),
		on_button       = msg("panel"),
		buttons         = {.Middle},
	}
	if kui.box(ui, outer) {
		kui.button(ui, "go", msg("go"), {description = "Starts the run"})
		// Every row the stock button admits, on a button the steps never
		// touch.
		kui.button(ui, "stop", msg("stop"), {label = "Stop the run", disabled = true, tooltip = "Nothing is running"})
		// A field with the wrap row declared (backlog F44). The seed folds
		// onto two lines.
		kui.text_edit(ui, "note", "hello, on two lines in a narrow field", {.Wrap}, {size = 13}, {width = kui.px(160), label = "Note"})
		// A slider that names its own reading (backlog F8).
		focus := kui.Spec {
			key        = "focus",
			width      = kui.px(120),
			height     = kui.px(12),
			role       = .Slider,
			label      = "Focus length",
			value_now  = 25,
			value_min  = 5,
			value_max  = 60,
			value_text = "25 minutes",
		}
		if kui.box(ui, focus) {}
	}
}

scene_media :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(6), gap = 4}) {
		kui.image(ui, f.image, {width = kui.px(16), radius = 2})
		// ADR 0025: the icon as `contain` in a box twice its aspect, then the
		// stream fixture plain, `nearest`, and `cover` in a square box.
		kui.image_with(ui, f.image, .Linear, .Contain, {width = kui.px(32), height = kui.px(16), label = "Icon"})
		kui.image(ui, f.stream, {width = kui.px(16), label = "Stream"})
		kui.image_with(ui, f.stream, .Nearest, .Fill, {width = kui.px(16), label = "Crisp"})
		kui.image_with(ui, f.stream, .Linear, .Cover, {width = kui.px(12), height = kui.px(12), label = "Cropped"})
		kui.audio(ui, "music", {src = f.sound, volume = 0.5, looped = true})
		kui.latency_graph(ui)
		// The two phase 1 drops: `chime` asked to finish, so its removal
		// releases the playback and queues no stop; `blip` did not.
		if phase == 0 {
			kui.audio(ui, "chime", {src = f.sound, finish = true})
			kui.audio(ui, "blip", {src = f.sound})
		}
	}
}

// docs/adr/0025-the-image-is-the-canvas.md, decision 6: five fills in a
// 200x120 canvas. The vertices are conformance::POLYGON_* to the number.
scene_polygon :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.px(200), height = kui.px(120), bg = 0x14161eff}) {
		tri := [3][2]f32{{10, 10}, {60, 20}, {20, 50}}
		kui.polygon(ui, "", tri[:], {bg = 0x7f9cf5ff, label = "Triangle"}, msg("tri"))
		arrow := [4][2]f32{{80, 10}, {130, 30}, {80, 50}, {95, 30}}
		kui.polygon(ui, "", arrow[:], {bg = 0xd8863bff})
		star := [8][2]f32{{170, 10}, {176, 24}, {190, 30}, {176, 36}, {170, 50}, {164, 36}, {150, 30}, {164, 24}}
		kui.polygon(ui, "star", star[:], {bg = 0xf5d67fff})
		nine := [9][2]f32{{10, 70}, {30, 65}, {50, 70}, {70, 65}, {90, 70}, {90, 110}, {50, 100}, {10, 110}, {5, 90}}
		kui.polygon(ui, "", nine[:], {bg = 0x9ad9a0ff})
		quad := [4][2]f32{{110, 70}, {190, 70}, {180, 110}, {120, 110}}
		kui.polygon(ui, "", quad[:], {bg = 0xe07a8aff, opacity = 0.5})
	}
}

// docs/adr/0040-a-path-is-a-mask-in-the-atlas.md: seven paths in a 200x120
// canvas. Data is conformance::PATH_* to the character.
scene_path :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.px(200), height = kui.px(120), bg = 0x14161eff}) {
		kui.path_d(ui, "", "M60 60 L100 60 A40 40 0 0 1 60 100 Z", .Nonzero, 0, 0, 0, nil, nil, {bg = 0x7f9cf5ff, label = "Wedge"}, msg("wedge"))
		// A leaf's tooltip floats beside it, below its box (backlog RG113).
		kui.path_d(ui, "wedge2", "M60 60 L60 100 A40 40 0 0 1 20 60 Z", .Nonzero, 0, 0, 0, nil, nil, {bg = 0xd8863bff, tooltip = "The other quarter"})
		// path_parse answers with the whole form; the C's 64-float buffer
		// is the bound it draws within.
		ring := kui.path_parse("M120 10 H190 V80 H120 Z M140 30 H170 V60 H140 Z")
		if len(ring) > 0 && len(ring) <= 64 {
			kui.path(ui, "", ring, .Evenodd, 0, 0, 0, nil, nil, {bg = 0xf5d67fff})
		}
		// The curve's stroke is dashed: 8 px marks, 4 px gaps, 3 px in.
		dashes := [5]f32{8, 4, 8, 4, 3}
		kui.path_d(ui, "", "M110 90 C130 70 150 110 190 90", .Nonzero, 2, 0x9ad9a0ff, 0, nil, dashes)
		kui.path_d(ui, "", "M20 10 L50 10 L35 40 Z", .Nonzero, 1.5, 0xffffffff, 0, nil, nil, {bg = 0xe07a8aff, opacity = 0.5})
		pivot := [2]f32{40, 107}
		kui.path_d(ui, "", "M30 104 H50 V110 H30 Z", .Nonzero, 0, 0, 0.125, pivot, nil, {bg = 0x7fd6f5ff})
		kui.path_d(ui, "bad", "M10 10 L20", .Nonzero, 0, 0, 0, nil, nil, {bg = 0xffffffff})
	}
}

// docs/adr/0015: four fragments - plain, keyed with a child painted over it,
// a dead handle that draws nothing, and one with eighteen params so the
// warning fires - then the image input (backlog V1).
scene_fragments :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.px(200), height = kui.px(120), gap = 4, bg = 0x14161eff}) {
		kui.fragment(ui, f.fragment, fragment_params[:], {width = kui.px(80), height = kui.px(40)})
		card := kui.Spec {
			width   = kui.px(80),
			height  = kui.px(40),
			pad     = kui.pad(6),
			radius  = 8,
			opacity = 0.5,
		}
		if kui.fragment_open(ui, "card", f.fragment, fragment_params[:], card) {
			if kui.box(ui, {width = kui.px(20), height = kui.px(10), bg = 0x202030ff}) {}
		}
		kui.fragment(ui, 0, fragment_params[:], {width = kui.px(20), height = kui.px(10)})
		kui.fragment(ui, f.fragment, fragment_params_long[:], {width = kui.px(30), height = kui.px(12)})
		if kui.row(ui, {gap = 4}) {
			sq := kui.Spec {
				width  = kui.px(24),
				height = kui.px(24),
			}
			kui.fragment_with(ui, "", f.sampler, f.image, fragment_image_params[:], sq)
			kui.fragment_with(ui, "", f.sampler, f.stream, fragment_image_params[:], {width = kui.px(32), height = kui.px(8)})
			// An image handle live in no session: the fragment draws nothing.
			kui.fragment_with(ui, "", f.sampler, f.dead, fragment_image_params[:], sq)
		}
	}
}

// Backlog K4: a wave in red under a span, a green solid line through the
// style's fields, dots in their own colour, and an undercurl over three
// cells.
scene_underlines :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10), gap = 4}) {
		mono := kui.Text_Style {
			size        = 14,
			family      = .Mono,
			line_height = 20,
		}
		spans := []kui.Span{{text = "let "}, {text = "value", underline_color = 0xff0000ff, underline_style = .Wavy}}
		kui.rich_text(ui, spans, mono)
		green := mono
		green.underline_color = 0x00ff00ff
		kui.text(ui, "warn", green)
		dotted := mono
		dotted.underline_color = 0x7f9cf5ff
		dotted.underline_style = .Dotted
		kui.text(ui, "dots", dotted)
		screen: [3]kui.Cell
		chars := "abc"
		for &cell, i in screen {
			cell.ch = rune(chars[i])
			cell.fg = 0xd6d8e0ff
			cell.flags = {.Wavy}
			cell.ul = 0xff0000ff
		}
		kui.cells(ui, "term", 1, 3, screen[:], mono, {label = "term"})
	}
}

// conformance::build_joined_backgrounds (backlog F101).
scene_joined_backgrounds :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10)}) {
		mono := kui.Text_Style {
			size        = 14,
			family      = .Mono,
			line_height = 20,
		}
		sel: kui.Color = 0x3b5bd466
		kui.rich_text(ui, {{text = "let "}, {text = "a = 1;", bg = sel, bg_radius = 4}}, mono)
		kui.rich_text(ui, {{text = "let b = 22;", bg = sel, bg_radius = 4}}, mono)
		kui.rich_text(ui, {{text = "c", bg = sel, bg_radius = 4}, {text = " + d"}}, mono)
		kui.rich_text(ui, {{text = "find", bg = 0xd9738c66, bg_radius = 4}}, mono)
	}
}

// conformance::build_break_spaces (backlog F106).
scene_break_spaces :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10)}) {
		if kui.box(ui, {width = kui.px(4)}) {
			mono := kui.Text_Style {
				size        = 14,
				family      = .Mono,
				line_height = 20,
				wrap        = .Break_Spaces,
			}
			kui.rich_text(ui, {{text = "ab"}, {text = "  ", bg = 0x3b5bd4ff}, {text = "c"}}, mono)
		}
	}
}

// conformance::build_relative_shrink (backlog F110).
scene_relative_shrink :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {gap = 4}) {
		bar := kui.Spec {
			height = kui.px(10),
			bg     = 0x3b5bd4ff,
		}
		if kui.row(ui, {width = kui.px(200), gap = 20}) {
			half := bar
			half.width = kui.pct(50)
			if kui.box(ui, half) {}
			if kui.box(ui, half) {}
		}
		if kui.row(ui, {width = kui.px(300), gap = 20}) {
			clamped := bar
			clamped.width = kui.clamp_size(kui.px(100), kui.pct(60), kui.px(400))
			if kui.box(ui, clamped) {}
			if kui.box(ui, clamped) {}
		}
	}
}

// conformance::build_column_squeeze (backlog F114).
bar20 :: proc(ui: ^kui.Ui) {
	if kui.box(ui, {width = kui.px(100), height = kui.px(20), bg = 0x3b5bd4ff}) {}
}

scene_column_squeeze :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui, {gap = 20}) {
		column := kui.Spec {
			width  = kui.px(100),
			height = kui.px(50),
		}
		if kui.box(ui, column) {
			for _ in 0 ..< 3 {
				if kui.row(ui) {bar20(ui)}
			}
		}
		if kui.box(ui, column) {
			if kui.row(ui) {bar20(ui)}
			if kui.box(ui, {overflow = {.Clip}}) {
				bar20(ui)
				bar20(ui)
			}
		}
		if kui.box(ui, column) {
			if kui.row(ui, {width = kui.GROW, wrap_children = true, gap = 10, cross_gap = 5}) {
				for _ in 0 ..< 2 {
					if kui.box(ui, {width = kui.px(60), height = kui.px(20), bg = 0x73d98cff}) {}
				}
			}
			if kui.row(ui) {bar20(ui)}
		}
	}
}

// conformance::build_fit_across (backlog F116).
fit_card :: proc(ui: ^kui.Ui, min_width: Maybe(kui.Size)) {
	if kui.box(ui, {max_width = kui.px(100)}) {
		if kui.box(ui, {min_width = min_width, bg = 0x30344aff}) {
			if kui.row(ui, {wrap_children = true, gap = 10, cross_gap = 5}) {
				for _ in 0 ..< 3 {
					if kui.box(ui, {width = kui.px(40), height = kui.px(20), bg = 0x73d98cff}) {}
				}
			}
		}
	}
}

scene_fit_across :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui, {gap = 20}) {
		fit_card(ui, nil)
		fit_card(ui, kui.FIT)
	}
}

// conformance::build_gradients (ADR 0042).
scene_gradients :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10), gap = 6}) {
		fade_stops := []kui.Gradient_Stop{{0x7f9cf5ff, -1}, {0xe07a8aff, -1}}
		strip := kui.Spec {
			width    = kui.px(200),
			height   = kui.px(40),
			radius   = 8,
			gradient = kui.Gradient{kind = .Linear, angle = 0, stops = fade_stops},
		}
		if kui.box(ui, strip) {}
		glow_stops := []kui.Gradient_Stop{{0xf5d67fff, -1}, {0x00000000, 0.8}}
		square := kui.Spec {
			width        = kui.px(200),
			height       = kui.px(40),
			bg           = 0x14161eff,
			border_w     = 2,
			border_color = 0xffffffff,
			gradient     = kui.Gradient{kind = .Linear, angle = 0.125, stops = glow_stops},
		}
		if kui.box(ui, square) {}
		sun_stops := []kui.Gradient_Stop{{0x9ad9a0ff, -1}, {0x14161eff, -1}}
		radial := kui.Spec {
			width    = kui.px(200),
			height   = kui.px(40),
			gradient = kui.Gradient{kind = .Radial, at = {0.5, 0}, stops = sun_stops},
		}
		if kui.box(ui, radial) {}
	}
}

// conformance::build_size_expressions (backlog F109): four bars in a 400 px
// column sized by expressions, built from parts - nothing parsed but the
// clamp.
scene_size_expressions :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.px(400), gap = 4}) {
		bar := kui.Spec {
			height = kui.px(10),
			bg     = 0x3b5bd4ff,
		}
		clamped := bar
		clamped.width = kui.size("clamp(100px, 50%, 150px)")
		if kui.box(ui, clamped) {}
		smaller := bar
		smaller.width = kui.size_min(kui.pct(80), kui.px(300))
		if kui.box(ui, smaller) {}
		held := bar
		held.width = kui.px(900)
		held.max_width = kui.pct(25)
		if kui.box(ui, held) {}
		floored := bar
		floored.min_width = kui.size_max(kui.pct(40), kui.px(50))
		if kui.box(ui, floored) {}
	}
}

// docs/adr/0010-a-segment-primitive.md: three strokes and a box in a 200x120
// canvas; the elbow's on_click is the one a line ignores.
scene_lines :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.px(200), height = kui.px(120), bg = 0x14161eff}) {
		kui.line(ui, 10, 10, 90, 70, 2, 0x7f9cf5ff)
		// The elbow takes a click, and is hit by its stroke (ADR 0026).
		elbow := [3][2]f32{{100, 20}, {140, 20}, {140, 60}}
		kui.polyline(ui, "", elbow[:], 3, 0xd8863bff, false, nil, {label = "Elbow"}, msg("elbow"))
		curve := [4][2]f32{{20, 100}, {60, 80}, {100, 110}, {180, 90}}
		kui.polyline(ui, "curve", curve[:], 1.5, 0x9ad9a0ff, true, nil, {opacity = 0.5})
		// A dash-dot round a corner, 3 px into its pattern (backlog V2).
		corner := [3][2]f32{{150, 70}, {190, 70}, {190, 110}}
		dash_dot := [5]f32{10, 4, 2, 4, 3}
		kui.polyline(ui, "", corner[:], 2, 0xe07a8aff, false, dash_dot)
		if kui.box(ui, {width = kui.px(40), height = kui.px(20), bg = 0x202030ff}) {}
	}
}

// One button in the dialog: `kind` is the click payload, `label` the
// accessible name.
modal_button :: proc(ui: ^kui.Ui, key, kind, label: string) {
	spec := kui.Spec {
		key      = key,
		dir      = .Row,
		width    = kui.px(100),
		height   = kui.px(24),
		bg       = 0x3b5bd4ff,
		label    = label,
		on_click = msg(kind),
	}
	if kui.box(ui, spec) {}
}

// UTF-8 for one scalar value, for the steps that may carry any.
utf8_of :: proc(c: int, out: ^[4]u8) -> int {
	bytes, n := utf8.encode_rune(rune(c))
	out^ = bytes
	return n
}

// An IME against both kinds of editor (backlog C17): the custom one is a
// sink holding a line with a caret, the stock one text_edit.
scene_ime :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10), gap = 6}) {
		buf := kui.Spec {
			key    = "buffer",
			width  = kui.px(200),
			height = kui.px(24),
			bg     = 0x1b1d27ff,
			role   = .Multiline_Text_Input,
			label  = "Buffer",
			on_key = msg("ed"),
		}
		if kui.box(ui, buf) {
			if kui.row(ui, {key = "l0", height = kui.px(20), role = .Line, caret = 1}) {
				kui.text(ui, "ab", {size = 13, family = .Mono})
			}
		}
		kui.text_edit(ui, "note", "", {}, {size = 13}, {width = kui.px(200), label = "Note"})
	}
}

// A terminal's screen as one node (backlog C20): the click on the fourth
// cell names it.
scene_cells :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10)}) {
		txt := "hello world"
		screen: [11]kui.Cell
		for &cell, i in screen {
			cell.ch = rune(txt[i])
			cell.fg = 0xd6d8e0ff
			cell.bg = 0x1a1d27ff if i < 3 else 0
		}
		style := kui.Text_Style {
			size        = 13,
			family      = .Mono,
			line_height = 18,
		}
		kui.cells(ui, "term", 1, 11, screen[:], style, {label = "term"}, msg("hit"), nil, nil, 0, 3, .Block, 0x6a8bffff, 0)
	}
}

// Two key sinks: the first says only on_key and hears the press alone, the
// second sets key_up and hears both halves. An integer tag, so the report's
// event column shows the phase. Left open, so the third can hold a button.
keys_sink :: proc(ui: ^kui.Ui, name: string, key_up: bool) {
	spec := kui.Spec {
		key    = name,
		dir    = .Row,
		width  = kui.px(100),
		height = kui.px(24),
		bg     = 0x1b1d27ff,
		role   = .Group,
		label  = name,
		key_up = key_up,
		on_key = 1,
	}
	kui.open(ui, spec)
}

// Two sinks asking for releases, tagged by kind; the second asks for the
// modifier keys (backlog F108).
modkeys_sink :: proc(ui: ^kui.Ui, name: string, modifier_keys: bool) {
	spec := kui.Spec {
		key           = name,
		dir           = .Row,
		width         = kui.px(100),
		height        = kui.px(24),
		bg            = 0x1b1d27ff,
		role          = .Group,
		label         = name,
		key_up        = true,
		modifier_keys = modifier_keys,
		on_key        = msg(name),
	}
	if kui.box(ui, spec) {}
}

scene_modifier_keys :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10), gap = 6}) {
		modkeys_sink(ui, "plain", false)
		modkeys_sink(ui, "mods", true)
	}
}

scene_keys :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10), gap = 6}) {
		keys_sink(ui, "press", false)
		kui.close(ui)
		keys_sink(ui, "held", true)
		kui.close(ui)
		// A shell over a ring: the sink hears what the button inside it does
		// not claim, and the button holds focus from the first frame
		// (docs/adr/0011-keys-bubble-to-the-enclosing-sink.md).
		keys_sink(ui, "shell", true)
		go := kui.Spec {
			key       = "go",
			dir       = .Row,
			width     = kui.px(80),
			height    = kui.px(16),
			bg        = 0x3b5bd4ff,
			label     = "Go",
			on_click  = msg("go"),
			key_focus = true,
		}
		if kui.box(ui, go) {}
		kui.close(ui)
	}
}

// A floated modal over an app with window chrome (docs/adr/0003).
scene_modal :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {gap = 6, width = kui.GROW}) {
		kui.titlebar_with(ui, f, titlebar_body)
		// The app owns its keyboard while the dialog is shut and says so
		// every frame - an edge once, and no clobber after.
		open := kui.Spec {
			key       = "open",
			dir       = .Row,
			width     = kui.px(100),
			height    = kui.px(20),
			bg        = 0x30344aff,
			label     = "Open",
			on_click  = msg("open"),
			key_focus = phase == 0,
		}
		if kui.box(ui, open) {}
		// Phase 1 drops the dialog and declares the node it was renaming
		// focused instead (ADR 0003, decision 4).
		if phase != 0 {
			note := kui.Spec {
				key       = "note",
				dir       = .Row,
				width     = kui.px(100),
				height    = kui.px(20),
				bg        = 0x30344aff,
				label     = "Note",
				on_click  = msg("note"),
				key_focus = true,
			}
			if kui.box(ui, note) {}
			return
		}
		dialog := kui.Spec {
			key    = "dialog",
			width  = kui.px(120),
			height = kui.px(100),
			pad    = kui.pad(8),
			gap    = 6,
			bg     = 0x202030ff,
			float  = {mode = .Viewport, anchor_x = .End, anchor_y = .End, self_x = .End, self_y = .End},
			modal  = msg("dlg"),
			label  = "Settings",
		}
		if kui.box(ui, dialog) {
			modal_button(ui, "ok", "ok", "OK")
			modal_button(ui, "cancel", "cancel", "Cancel")
		}
	}
}

// conformance::EXIT_BULK_ROWS: with its own root, one node past
// kui_core::depart::MAX_NODES, so the whole subtree is refused.
EXIT_BULK_ROWS :: 4096
// conformance::EXIT_ROWS: more one-node subtrees than the budget, dropped in
// one frame and refused whole (docs/adr/0012-the-exit-budget.md).
EXIT_ROWS :: 4200

// One of the exit scene's fixed-size slots; the caller closes it.
exit_slot :: proc(ui: ^kui.Ui, key: string, h: f32) {
	kui.open(ui, {key = key, width = kui.px(140), height = kui.px(h), bg = 0x101018ff})
}

// A live Tab stop either side of the departing ones.
exit_keep :: proc(ui: ^kui.Ui, key, label: string) {
	keep := kui.Spec {
		key       = key,
		dir       = .Row,
		width     = kui.px(60),
		height    = kui.px(16),
		bg        = 0x22242cff,
		focusable = true,
		label     = label,
	}
	if kui.box(ui, keep) {}
}

// Exit transitions (docs/adr/0005-the-paint-vocabulary.md).
scene_exit :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	outer := kui.Spec {
		width  = kui.GROW,
		height = kui.GROW,
		pad    = kui.pad(8),
		gap    = 6,
		bg     = 0x14161eff,
	}
	if kui.box(ui, outer) {
		exit_keep(ui, "a", "A")

		exit_slot(ui, "slotFade", 40)
		if phase == 0 {
			// Focusable, clickable and labelled while it is live, so each of
			// those is a separate thing the ghost has to stop being.
			fade := kui.Spec {
				key        = "fade",
				width      = kui.px(100),
				height     = kui.px(24),
				bg         = 0x3b5bd4ff,
				transition = 400,
				exit       = {set = {.Offset, .Opacity}, dx = 40, opacity = 0},
				focusable  = true,
				label      = "Fade",
				on_click   = msg("hit"),
			}
			if kui.box(ui, fade) {
				kui.text(ui, "bye", {size = 12})
			}
		}
		kui.close(ui)

		exit_slot(ui, "slotBlink", 16)
		if phase == 0 {
			blink := kui.Spec {
				key        = "blink",
				width      = kui.px(100),
				height     = kui.px(12),
				bg         = 0x73d98cff,
				transition = 50,
				exit       = {set = {.Offset}, dx = 20},
			}
			if kui.box(ui, blink) {}
		}
		kui.close(ui)

		exit_slot(ui, "slotFlash", 16)
		if phase != 1 {
			flash := kui.Spec {
				key        = "flash",
				width      = kui.px(100),
				height     = kui.px(12),
				bg         = 0xffcc00ff,
				transition = 400,
				exit       = {set = {.Offset}, dx = -20},
			}
			if kui.box(ui, flash) {}
		}
		kui.close(ui)

		exit_keep(ui, "b", "B")

		// More one-node departures than the budget, each a solid quad half a
		// pixel wide, in a slot that keeps its size when they go.
		slot_rows := kui.Spec {
			key    = "slotRows",
			dir    = .Row,
			width  = kui.px(300),
			height = kui.px(4),
			bg     = 0x101018ff,
		}
		if kui.box(ui, slot_rows) {
			if phase < 4 {
				cell := kui.Spec {
					width      = kui.px(0.5),
					height     = kui.px(4),
					bg         = 0x8a8fa3ff,
					transition = 400,
					exit       = {set = {.Opacity}, opacity = 0},
				}
				for _ in 0 ..< EXIT_ROWS {
					if kui.box(ui, cell) {}
				}
			}
		}

		// Last, and sized by children that have no size: dropping it takes
		// only the trailing gap with it.
		if phase < 3 {
			if kui.box(ui, {key = "bulk", transition = 400, exit = {set = {.Opacity}, opacity = 0}}) {
				for _ in 0 ..< EXIT_BULK_ROWS {
					if kui.box(ui) {}
				}
			}
		}
	}
}

// One item of a composite: a click payload, its own text, and (for a row)
// `focusable`.
composite_item :: proc(ui: ^kui.Ui, name, kind: string, role: kui.Role, selected, focusable: bool, w, h: f32, bg: kui.Color) {
	spec := kui.Spec {
		key       = name,
		dir       = .Row,
		role      = role,
		selected  = selected,
		focusable = focusable,
		width     = kui.px(w),
		height    = kui.px(h),
		bg        = bg,
		on_click  = msg(kind),
	}
	if kui.box(ui, spec) {
		kui.text(ui, name, {size = 12})
	}
}

// A tab bar and a picker list, each one Tab stop with the arrows moving
// inside it (docs/adr/0007), and an ordinary button between them.
scene_composite :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {gap = 6, width = kui.GROW}) {
		if kui.row(ui, {key = "tabs", role = .Tab_List, gap = 4}) {
			composite_item(ui, "One", "one", .Tab, false, false, 60, 20, 0x30344aff)
			composite_item(ui, "Two", "two", .Tab, true, false, 60, 20, 0x30344aff)
			composite_item(ui, "Three", "three", .Tab, false, false, 60, 20, 0x30344aff)
		}
		add := kui.Spec {
			key      = "add",
			dir      = .Row,
			width    = kui.px(40),
			height   = kui.px(20),
			bg       = 0x3b5bd4ff,
			label    = "Add",
			on_click = msg("add"),
		}
		if kui.box(ui, add) {}
		if kui.box(ui, {key = "rows", role = .List, gap = 2}) {
			composite_item(ui, "Alpha", "alpha", .List_Item, false, true, 80, 18, 0x202030ff)
			composite_item(ui, "Bravo", "bravo", .List_Item, false, true, 80, 18, 0x202030ff)
		}
	}
}

// conformance::build_windows: the declaration comes and goes with the
// phase. Phase 0 declares `palette` twice, disagreeing about the size, so the
// first wins and the second warns.
scene_windows :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	cfg := kui.Window_Config {
		kind   = .Normal,
		width  = 400,
		height = 300,
	}
	if phase == 0 || phase == 2 do kui.window_declare(ui, "palette", cfg)
	if phase == 0 {
		cfg.width = 500
		cfg.height = 500
		kui.window_declare(ui, "palette", cfg)
	}
	if kui.box(ui, {pad = kui.pad(8), bg = 0x14161eff}) {
		kui.text(ui, "open" if phase == 0 || phase == 2 else "closed", {size = 12})
	}
}

// conformance::build_popup: one declaration, with the kind and the anchor a
// menu carries (ADR 0004 decision 9).
scene_popup :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if phase == 0 {
		cfg := kui.Window_Config {
			kind      = .Popup,
			activates = false,
			width     = 160,
			height    = 320,
			anchor_x  = 12,
			anchor_y  = 40,
			anchor_w  = 160,
			anchor_h  = 24,
		}
		kui.window_declare(ui, "menu", cfg)
	}
	if kui.box(ui, {pad = kui.pad(8), bg = 0x14161eff}) {
		kui.text(ui, "menu" if phase == 0 else "closed", {size = 12})
	}
}

// conformance::build_live: the `live` prop on a box that would otherwise be
// elided, and `announce` for the half with no node behind it.
scene_live :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if phase == 1 do kui.announce(ui, "Saved", .Assertive)
	if kui.box(ui, {pad = kui.pad(8), gap = 4, bg = 0x14161eff}) {
		if kui.box(ui, {key = "status", live = .Polite}) {
			kui.text(ui, "0 results" if phase == 0 else "3 results", {size = 12})
		}
		if kui.box(ui, {key = "empty", live = .Polite}) {}
	}
}

// conformance::build_drag: one keyed 80x40 handle whose drag deltas the event
// rows carry.
scene_drag :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	kui.open_draggable(ui, "handle", {width = kui.px(80), height = kui.px(40), bg = 0x30344aff}, nil, msg("split"))
	kui.close(ui)
}

// conformance::build_selection: a `selectable` card of three runs.
scene_selection :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	card := kui.Spec {
		key        = "card",
		width      = kui.px(200),
		pad        = kui.pad(8),
		gap        = 4,
		bg         = 0x14161eff,
		selectable = true,
	}
	if kui.box(ui, card) {
		kui.text(ui, "one", {size = 13})
		kui.text(ui, "two", {size = 13})
		kui.text(ui, "three", {size = 13})
	}
}

// conformance::build_selection_scroll: the same card, forty px tall and
// scrolling, over six runs (ADR 0029).
scene_selection_scroll :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	card := kui.Spec {
		key        = "card",
		width      = kui.px(200),
		height     = kui.px(40),
		pad        = kui.pad(8),
		gap        = 4,
		bg         = 0x14161eff,
		overflow   = {.Scroll_Y},
		selectable = true,
	}
	if kui.box(ui, card) {
		for line in ([]string{"one", "two", "three", "four", "five", "six"}) {
			kui.text(ui, line, {size = 13})
		}
	}
}

// conformance::build_scroll_handler_room (backlog F118): a strip (x) holding
// a code box that scrolls x and hears the wheel, then a spacer.
scene_scroll_handler_room :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(4)}) {
		strip := kui.Spec {
			key      = "strip",
			dir      = .Row,
			width    = kui.px(200),
			height   = kui.px(80),
			overflow = {.Scroll_X},
			bg       = 0x101018ff,
		}
		if kui.box(ui, strip) {
			code := kui.Spec {
				key       = "code",
				width     = kui.px(100),
				height    = kui.px(80),
				overflow  = {.Scroll_X},
				on_scroll = msg("code"),
				bg        = 0x161820ff,
			}
			if kui.box(ui, code) {
				if kui.box(ui, {width = kui.px(300), height = kui.px(80), bg = 0x3b5bd4ff}) {}
			}
			if kui.box(ui, {width = kui.px(200), height = kui.px(80), bg = 0x2a2d3aff}) {}
		}
	}
}

// conformance::build_scroll_gestures (backlog F107): a page (y) holding a
// strip (x) holding a list that contains its gestures and a wheel handler
// that takes only y, then a spacer each.
scene_scroll_gestures :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(4), on_scroll = msg("zoom"), scroll_mods = {.Ctrl}}) {
		page := kui.Spec {
			key      = "page",
			width    = kui.px(200),
			height   = kui.px(100),
			overflow = {.Scroll_Y},
			bg       = 0x101018ff,
		}
		if kui.box(ui, page) {
			if kui.row(ui, {key = "strip", width = kui.px(200), height = kui.px(80), overflow = {.Scroll_X}}) {
				list := kui.Spec {
					key        = "list",
					width      = kui.px(100),
					height     = kui.px(80),
					gap        = 4,
					overflow   = {.Scroll_Y},
					overscroll = .Contain,
					bg         = 0x161820ff,
				}
				if kui.box(ui, list) {
					for i in 0 ..< 6 {
						item := kui.Spec {
							key    = fmt.tprintf("i%d", i),
							width  = kui.px(90),
							height = kui.px(20),
							bg     = 0x30344aff,
						}
						if kui.box(ui, item) {}
					}
				}
				term := kui.Spec {
					key         = "term",
					width       = kui.px(100),
					height      = kui.px(80),
					bg          = 0x3b5bd4ff,
					on_scroll   = msg("term"),
					scroll_axes = .Y,
				}
				if kui.box(ui, term) {}
				if kui.box(ui, {width = kui.px(100), height = kui.px(80), bg = 0x2a2d3aff}) {}
			}
			if kui.box(ui, {width = kui.px(200), height = kui.px(60), bg = 0x22252fff}) {}
		}
	}
}

// conformance::build_cells_scroll: the `cells` screen three rows tall,
// `selectable` and hearing the wheel, row 0 at 100 plus the phase (ADR 0029).
scene_cells_scroll :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(10)}) {
		screen: [33]kui.Cell
		for &cell in screen {
			cell.ch = ' '
			cell.fg = 0xd6d8e0ff
		}
		rows := [3]string{"hello world", "brave", "bye"}
		for row, r in rows {
			for ch, col in transmute([]u8)row do screen[r * 11 + col].ch = rune(ch)
		}
		style := kui.Text_Style {
			size        = 13,
			family      = .Mono,
			line_height = 18,
		}
		spec := kui.Spec {
			label      = "term",
			selectable = true,
			on_scroll  = msg("term"),
		}
		kui.cells(ui, "term", 3, 11, screen[:], style, spec, origin_line = 100 + u64(phase))
	}
}

// A virtual list's three built rows, each opened at its data index between
// the two spacers that stand in for the rows nobody built.
scene_virtual :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	list := kui.Spec {
		key       = "list",
		width     = kui.px(120),
		height    = kui.px(60),
		overflow  = {.Scroll_Y},
		bg        = 0x101018ff,
		role      = .List,
		label     = "log",
		// conformance::VIRTUAL_ROW_COUNT: how many rows the list has.
		row_count = 109,
	}
	if kui.box(ui, list) {
		if kui.box(ui, {key = "lead", width = kui.GROW, height = kui.px(20)}) {}
		for i in u64(100) ..< 103 {
			row := kui.Spec {
				index  = i,
				width  = kui.GROW,
				height = kui.px(20),
				bg     = 0x30344aff,
				role   = .List_Item,
				label  = fmt.tprintf("row %d", i),
			}
			if kui.box(ui, row) {}
		}
		if kui.box(ui, {key = "tail", width = kui.GROW, height = kui.px(100)}) {}
	}
}

// conformance::LAYERS_ROWS: enough rows to overflow the viewport.
LAYERS_ROWS :: 16

// One of the two floats of scene_layers: a viewport float at (x, y) that
// posts `kind` when clicked.
layers_float :: proc(ui: ^kui.Ui, key: string, x, y: f32, bg: kui.Color, kind, label: string) {
	spec := kui.Spec {
		key      = key,
		float    = {mode = .Viewport, anchor_x = .Start, anchor_y = .Start, self_x = .Start, self_y = .Start, dx = x, dy = y},
		width    = kui.px(120),
		height   = kui.px(80),
		bg       = bg,
		label    = label,
		on_click = msg(kind),
	}
	if kui.box(ui, spec) {}
}

// conformance::build_layers: two floats over a scroller's bar (ADR 0023).
scene_layers :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {width = kui.GROW, height = kui.GROW}) {
		page := kui.Spec {
			key      = "page",
			width    = kui.GROW,
			height   = kui.GROW,
			overflow = {.Scroll_Y},
			bg       = 0x101018ff,
		}
		if kui.box(ui, page) {
			for i in 0 ..< LAYERS_ROWS {
				row := kui.Spec {
					key    = fmt.tprintf("row%d", i),
					dir    = .Row,
					width  = kui.GROW,
					height = kui.px(30),
					bg     = 0x22242cff if i % 2 == 0 else 0x30344aff,
				}
				if kui.box(ui, row) {}
			}
		}
		if phase != 1 do layers_float(ui, "popover", 200, 40, 0x3b5bd4ff, "popover", "Popover")
		layers_float(ui, "toast", 140, 60, 0x73d98cff, "toast", "Toast")
	}
}

// conformance::build_drop (ADR 0031): two zones, a button inside the first,
// and across the phases a hoverable float over the first zone that is no
// zone (phase 1) and a modal over it (phase 2).
scene_drop :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.row(ui, {width = kui.GROW, height = kui.GROW}) {
		zone := kui.Spec {
			key     = "files",
			width   = kui.px(200),
			height  = kui.GROW,
			pad     = kui.pad(10),
			bg      = 0x22242cff,
			drop_bg = 0x2b3350ff,
			on_drop = msg("files"),
		}
		if kui.box(ui, zone) {
			pick := kui.Spec {
				key      = "pick",
				dir      = .Row,
				width    = kui.px(60),
				height   = kui.px(40),
				bg       = 0x3b5bd4ff,
				label    = "Pick",
				on_click = msg("pick"),
			}
			if kui.box(ui, pick) {}
		}
		if kui.box(ui, {key = "other", width = kui.GROW, height = kui.GROW, bg = 0x30344aff, on_drop = msg("other")}) {}
		at_corner := kui.Float {
			mode     = .Viewport,
			anchor_x = .Start,
			anchor_y = .Start,
			self_x   = .Start,
			self_y   = .Start,
			dx       = 20,
			dy       = 20,
		}
		if phase == 1 {
			overlay := kui.Spec {
				key       = "overlay",
				float     = at_corner,
				width     = kui.px(160),
				height    = kui.px(160),
				hoverable = true,
			}
			if kui.box(ui, overlay) {}
		}
		if phase == 2 {
			confirm := kui.Spec {
				key    = "confirm",
				float  = at_corner,
				width  = kui.px(160),
				height = kui.px(160),
				bg     = 0x101018ff,
				modal  = msg("dismiss"),
			}
			if kui.box(ui, confirm) {}
		}
	}
}

// conformance::build_sampler (backlog AR47): the generic rows no other scene
// declares, on four nodes.
scene_sampler :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	if kui.box(ui, {pad = kui.pad(8), gap = 6}) {
		stops := []kui.Keyframe{{set = {.Bg}, bg = 0x1b1d27ff}, {set = {.At, .Bg, .Radius}, at = 1, bg = 0x3b5bd4ff, radius = 12}}
		card := kui.Spec {
			key            = "card",
			dir            = .Row,
			width          = kui.px(120),
			height         = kui.px(40),
			max_width      = kui.px(100),
			max_height     = kui.px(30),
			main_align     = .Center,
			cross_align    = .Center,
			bg             = 0x1b1d27ff,
			radius_tl      = 8,
			radius_tr      = 2,
			radius_br      = 8,
			radius_bl      = 2,
			shadow_color   = 0x00000080,
			shadow_x       = 3,
			shadow_y       = 2,
			shadow_blur    = 2,
			hoverable      = true,
			hover_bg       = 0x262a3aff,
			pressed_bg     = 0x30364aff,
			hover_group    = "cards",
			focusable      = true,
			focus_bg       = 0x2b3350ff,
			initial_focus  = true,
			accent         = true,
			cursor         = .Pointer,
			selected       = true,
			expanded       = .Expanded,
			on_layout      = msg("lay"),
			on_force_click = msg("force"),
			click_sound    = f.sound,
			hover_sound    = f.sound,
			animate        = true,
			transition     = 100,
			easing         = .In_Out,
			bounce         = 0.3,
			slide          = true,
			delay          = 20,
			repeat         = .Alternate,
			keyframes      = stops,
			enter          = {set = {.Offset, .Opacity}, dx = -12, opacity = 0},
			role           = .Tab,
			label          = "Card",
			on_click       = msg("card"),
			on_hover       = msg("hov"),
		}
		if kui.box(ui, card) {
			kui.text(ui, "ab", {size = 12})
		}
		if kui.row(ui, {key = "strip", width = kui.px(60), height = kui.px(10), bg = 0x3a3f52ff, window = .Drag}) {}
		dock := kui.Spec {
			key           = "dock",
			dir           = .Row,
			focus_region  = true,
			gap           = 4,
			height        = kui.px(30),
			main_align    = .Center,
			cross_align   = .End,
			backdrop_blur = 6,
		}
		if kui.box(ui, dock) {
			stop := kui.Spec {
				key       = "stop",
				dir       = .Row,
				width     = kui.px(20),
				height    = kui.px(20),
				bg        = 0x2a2d3aff,
				focusable = true,
				role      = .Button,
				label     = "Stop",
			}
			if kui.box(ui, stop) {}
		}
		if kui.box(ui, {width = kui.px(60)}) {
			cut := kui.Text_Style {
				size          = 12,
				max_lines     = 1,
				ellipsis      = true,
				features      = "liga=0",
				underline     = true,
				strikethrough = true,
			}
			kui.text(ui, "a long line that is cut short", cut)
		}
		line := kui.Spec {
			key              = "line",
			dir              = .Row,
			height           = kui.px(16),
			role             = .Line,
			caret            = 2,
			selection_anchor = 0,
			caret_solid      = true,
		}
		if kui.box(ui, line) {
			kui.text(ui, "sel", {size = 12})
		}
	}
}

// conformance::build_select: the stock select over four rows, the fourth
// posting an id and disabled, the second in force.
scene_select :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	langs := []kui.Menu_Item {
		{label = "English", role = .Custom},
		{label = "Deutsch", role = .Custom},
		{label = "Fran\u00e7ais", role = .Custom},
		{label = "Latin", role = .Custom, disabled = true, id = "la"},
	}
	if kui.box(ui, {pad = kui.pad(10), gap = 6}) {
		kui.select(ui, "language", langs, 1)
		kui.text(ui, "body", {size = 12})
	}
}

// conformance::build_menu_bar: the application menu bar (ADR 0018) - a
// declaration, and the widget that draws it.
scene_menu_bar :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int) {
	file := []kui.Menu_Item {
		{label = "New", role = .Custom, id = "file.new", accel = "mod+n"},
		// conformance.c's separator is a zeroed KuiMenuItem, so not enabled.
		{role = .Separator, disabled = true},
		{label = "Wrap", role = .Custom, id = "file.wrap", checked = true},
		{label = "Print", role = .Custom, disabled = true, id = "file.print"},
	}
	edit := []kui.Menu_Item{{role = .Copy}}
	menus := []kui.Bar_Menu{{label = "File", items = file}, {label = "Edit", items = edit}}
	if kui.box(ui, {gap = 6, width = kui.GROW}) {
		kui.menu_bar(ui, menus)
		kui.text(ui, "body", {size = 12})
	}
}

Build :: proc(ui: ^kui.Ui, f: ^Fixtures, phase: int)

Scene :: struct {
	name:  string,
	// `phase` is what the "step phase N" lines leave behind: the view's own
	// mind.
	build: Build,
}

// One entry per scene of conformance::SCENES; a scene in the reference with
// no entry here fails the run rather than being skipped.
SCENES := [?]Scene {
	{"layout", scene_layout},
	{"sizing", scene_sizing},
	{"wrap", scene_wrap},
	{"align", scene_align},
	{"stock-controls", scene_stock_controls},
	{"table", scene_table},
	{"tabs", scene_tabs},
	{"overflow", scene_overflow},
	{"float", scene_float},
	{"clip-float", scene_clip_float},
	{"pixel-snap", scene_pixel_snap},
	{"clip-access", scene_clip_access},
	{"tooltip", scene_tooltip},
	{"select", scene_select},
	{"chrome", scene_chrome},
	// Same builder: chrome-inset is the same tree under an env that also
	// reports the OS controls.
	{"chrome-inset", scene_chrome},
	{"controls", scene_controls},
	{"keys", scene_keys},
	{"modifier-keys", scene_modifier_keys},
	{"ime", scene_ime},
	// The paste scene's tree is the ime scene's (backlog F84).
	{"paste", scene_ime},
	{"cells", scene_cells},
	{"underlines", scene_underlines},
	{"joined-backgrounds", scene_joined_backgrounds},
	{"break-spaces", scene_break_spaces},
	{"relative-shrink", scene_relative_shrink},
	{"column-squeeze", scene_column_squeeze},
	{"fit-across", scene_fit_across},
	{"size-expressions", scene_size_expressions},
	{"gradients", scene_gradients},
	{"media", scene_media},
	{"lines", scene_lines},
	{"polygon", scene_polygon},
	{"path", scene_path},
	{"fragments", scene_fragments},
	{"modal", scene_modal},
	{"composite", scene_composite},
	{"exit", scene_exit},
	{"windows", scene_windows},
	{"popup", scene_popup},
	{"live", scene_live},
	{"drag", scene_drag},
	{"selection", scene_selection},
	{"selection-extend", scene_selection},
	{"selection-scroll", scene_selection_scroll},
	{"scroll-gestures", scene_scroll_gestures},
	{"scroll-handler-room", scene_scroll_handler_room},
	{"cells-scroll", scene_cells_scroll},
	// Same builder: `menu` is that tree under a secondary press.
	{"menu", scene_selection},
	{"menubar", scene_menu_bar},
	{"virtual", scene_virtual},
	{"layers", scene_layers},
	{"drop", scene_drop},
	{"anchor", scene_anchor},
	{"scrollbar", scene_scrollbar},
	{"tokens", scene_tokens},
	{"sampler", scene_sampler},
}

// -- Driving one scene ----------------------------------------------------------

// A replayed input, parsed back out of the reference report's step lines.
Step :: struct {
	kind:    string,
	a, b, c: int,
	// How many numbers followed the kind.
	args:    int,
}

// The window facts a scene is driven under, parsed back out of its `env`
// line: the five arguments env_set_window takes, in its order.
Env :: struct {
	set:                                                            bool,
	custom_chrome, maximized, fullscreen, controls_w, controls_h: int,
}

// KUI_ROLE_* back to the camelCase spelling every binding prints
// (kui_core::Role::name); index 0 is unused, the enum starts at 1.
ROLE_NAMES := [?]string {
	"?", "none", "button", "checkbox", "radio", "switch", "slider", "tab",
	"tabList", "link", "heading", "list", "listItem", "image", "dialog",
	"group", "window", "titleBar", "staticText", "textInput",
	"multilineTextInput", "scrollView", "line",
	"radioGroup", "menu", "menuItem", "terminal",
}

role_name :: proc(role: kui.Role) -> string {
	r := int(role)
	return ROLE_NAMES[r] if r < len(ROLE_NAMES) else "?"
}

// AccessAction names in bit order (kui_core::AccessAction::ALL).
ACTION_NAMES := [?]string {
	"click", "focus", "blur", "setValue", "increment", "decrement",
	"scrollIntoView", "scrollUp", "scrollDown", "scrollLeft", "scrollRight",
	"setTextSelection", "replaceSelectedText",
}

// FNV-1a over each quad's words 0..18 - the quad without its uv and clip
// index - plus the uv of a segment or texture quad, then the eight words of
// the clip that index names. Mirrors conformance::quad_digest.
#assert(size_of(kui.Quad) == 24 * size_of(u32))
#assert(size_of(kui.Clip) == 8 * size_of(u32))

digest_words :: proc(h: ^u64, words: []u32) {
	for word in words {
		v := word
		for _ in 0 ..< 4 {
			h^ ~= u64(v & 0xff)
			h^ *= 0x100000001b3
			v >>= 8
		}
	}
}

quad_digest :: proc(quads: []kui.Quad, clips: []kui.Clip) -> u64 {
	h: u64 = 0xcbf29ce484222325
	for q in quads {
		w := transmute([24]u32)q
		digest_words(&h, w[:19])
		if q.kind == .Segment || q.kind == .Texture do digest_words(&h, w[20:24])
		cw: [8]u32
		if int(q.clip) < len(clips) do cw = transmute([8]u32)clips[q.clip]
		digest_words(&h, cw[:])
	}
	return h
}

// Drains the window commands, one line each, the way
// conformance::write_command spells them.
drain_cmds :: proc(ui: ^kui.Ui, out: ^strings.Builder) {
	for c in kui.take_window_command(ui) {
		#partial switch c.kind {
		case .Start_Drag:
			fmt.sbprintf(out, "cmd drag %d\n", c.window)
		case .Close:
			fmt.sbprintf(out, "cmd close %d\n", c.window)
		case .Minimize:
			fmt.sbprintf(out, "cmd minimize %d\n", c.window)
		case .Toggle_Maximize:
			fmt.sbprintf(out, "cmd maximize %d\n", c.window)
		case .Open:
			cfg := c.config
			fmt.sbprintf(
				out,
				"cmd open %d %d %d %d %d %d %d %d %d %d %d\n",
				c.window,
				c.owner,
				c.origin,
				u32(cfg.kind),
				int(cfg.width),
				int(cfg.height),
				1 if (cfg.activates.? or_else false) else 0,
				int(cfg.anchor_x),
				int(cfg.anchor_y),
				int(cfg.anchor_w),
				int(cfg.anchor_h),
			)
		case:
			fmt.sbprintf(out, "cmd ? %d\n", c.window)
		}
	}
}

// Drains the audio commands, one line each, the way
// conformance::write_audio_command spells them.
drain_audio :: proc(ui: ^kui.Ui, out: ^strings.Builder) {
	for c in kui.take_audio_commands(ui) {
		#partial switch c.kind {
		case .Play:
			fmt.sbprintf(out, "audio play %d %d\n", u64(c.playback), 1 if c.looped else 0)
		case .Stop:
			fmt.sbprintf(out, "audio stop %d\n", u64(c.playback))
		case .Set_Volume:
			fmt.sbprintf(out, "audio volume %d\n", u64(c.playback))
		case .Pause:
			fmt.sbprintf(out, "audio pause %d\n", u64(c.playback))
		case .Resume:
			fmt.sbprintf(out, "audio resume %d\n", u64(c.playback))
		case .Master_Volume:
			fmt.sbprintf(out, "audio master\n")
		case .Unload:
			fmt.sbprintf(out, "audio unload\n")
		case:
			fmt.sbprintf(out, "audio ? %d\n", u64(c.playback))
		}
	}
}

// The paths a files step names: /drop/1.txt ... /drop/n.txt
// (conformance::drop_paths), at most eight.
drop_paths :: proc(n: int) -> []string {
	paths := make([]string, min(n, 8), context.temp_allocator)
	for &p, i in paths do p = fmt.tprintf("/drop/%d.txt", i + 1)
	return paths
}

// Keys that a "keyat" step names by index (conformance::STEP_KEYS).
STEP_KEYS := [?]string{"shift", "enter", "capslock"}

apply :: proc(ui: ^kui.Ui, s: Step) {
	switch s.kind {
	case "cursor":
		kui.input_cursor(ui, f32(s.a), f32(s.b))
	case "cursorleft":
		kui.input_cursor_left(ui)
	case "mousedown":
		kui.input_mouse(ui, true, 1)
	case "mouseup":
		kui.input_mouse(ui, false, 1)
	case "secondarydown":
		kui.input_mouse_button(ui, true, .Secondary, 1)
	case "secondaryup":
		kui.input_mouse_button(ui, false, .Secondary, 1)
	case "middledown":
		kui.input_mouse_button(ui, true, .Middle, 1)
	case "middleup":
		kui.input_mouse_button(ui, false, .Middle, 1)
	case "scroll":
		kui.input_scroll(ui, f32(s.a), f32(s.b))
	case "tab":
		kui.input_key(ui, .Tab, {})
	case "shifttab":
		kui.input_key(ui, .Tab, {.Shift})
	case "modifiers":
		// The modifier state as KUI_KMOD_* bits, the corpus's own spelling.
		kui.input_modifiers(ui, transmute(kui.Mods)u32(s.a))
	case "escape":
		kui.input_key(ui, .Escape, {})
	case "arrow":
		// conformance::ARROWS order: left, right, up, down - Edit_Key's.
		kui.input_key(ui, kui.Edit_Key(u32(kui.Edit_Key.Left) + u32(s.a)), {})
	case "home":
		kui.input_key(ui, .Home, {})
	case "end":
		kui.input_key(ui, .End, {})
	case "type":
		// A scalar value, so a step line carries only integers; the corpus
		// types ASCII, which is one UTF-8 byte.
		b := [1]u8{u8(s.a)}
		kui.input_text(ui, string(b[:]))
	case "keydown":
		b := [1]u8{u8(s.a)}
		kui.input_key_down(ui, string(b[:]), "", {}, "", false)
	case "keyup":
		b := [1]u8{u8(s.a)}
		kui.input_key_up(ui, string(b[:]), "", {})
	case "keyatdown":
		kui.input_key_down(ui, STEP_KEYS[s.a], "", {location = kui.Key_Location(s.b)}, "", false)
	case "keyatup":
		kui.input_key_up(ui, STEP_KEYS[s.a], "", {location = kui.Key_Location(s.b)})
	case "preedit":
		// One character composing with its caret at the end, or 0 for the
		// composition ending.
		buf: [4]u8
		n := utf8_of(s.a, &buf) if s.a != 0 else 0
		none := max(u32)
		kui.input_preedit(ui, string(buf[:n]), 0 if n > 0 else none, u32(n) if n > 0 else none)
	case "commit":
		buf: [4]u8
		n := utf8_of(s.a, &buf)
		kui.input_commit(ui, string(buf[:n]))
	case "paste":
		buf: [4]u8
		n := utf8_of(s.a, &buf)
		kui.input_paste(ui, string(buf[:n]), transmute(kui.Paste_Marks)u32(s.b))
	case "dragfiles":
		kui.input_drag_files(ui, drop_paths(s.a), f32(s.b), f32(s.c))
	case "dropfiles":
		kui.input_drop_files(ui, drop_paths(s.a), f32(s.b), f32(s.c))
	case "dragcancel":
		kui.input_drag_cancel(ui)
	case "open":
		kui.input_open(ui, drop_paths(s.a))
	case:
		fmt.eprintfln("conformance: unknown step '%s'", s.kind)
		os.exit(1)
	}
}

// Drains the event queue; payloads are borrowed until the next poll, so each
// is formatted before the next.
drain_events :: proc(ui: ^kui.Ui, out: ^strings.Builder) {
	for ev in kui.poll_event(ui) {
		p := ev.payload
		kind, tag := "-", "-"
		if p != nil {
			if s, ok := kui.as_string(kui.get(p, "kind")); ok do kind = s
			t := kui.get(p, "tag")
			tk: kui.Value
			if t != nil do tk = kui.get(t, "kind")
			// The tag column, or - for the window-level events, which have
			// none - the field that tells one from its siblings.
			if tk == nil do tk = kui.get(p, "phase")
			if tk == nil do tk = kui.get(p, "reason")
			if s, ok := kui.as_string(tk); ok do tag = s
		}
		fmt.sbprintf(out, "event %s %s", kind, tag)
		phase := "-"
		if p != nil {
			if s, ok := kui.as_string(kui.get(p, "phase")); ok do phase = s
		}
		switch kind {
		case "change":
			// A slider's phase and the value the core worked out.
			v, _ := kui.as_i64(kui.get(p, "value"))
			fmt.sbprintf(out, " %s %d", phase, v)
		case "drag":
			// dx/dy from the press point in every phase.
			dx, _ := kui.as_i64(kui.get(p, "dx"))
			dy, _ := kui.as_i64(kui.get(p, "dy"))
			fmt.sbprintf(out, " %s %d %d", phase, dx, dy)
		case "key":
			// A key from one of a key's twins says which (backlog F108).
			loc := "standard"
			if s, ok := kui.as_string(kui.get(p, "location")); ok do loc = s
			if loc != "standard" {
				code := "-"
				if s, ok := kui.as_string(kui.get(p, "code")); ok do code = s
				fmt.sbprintf(out, " %s %s %s", phase, code, loc)
			}
		case "button":
			// A held button's phase and which button (backlog F105).
			b := kui.get(p, "button")
			if name, ok := kui.as_string(b); ok {
				fmt.sbprintf(out, " %s %s", phase, name)
			} else if code, is_int := kui.as_i64(b); is_int {
				fmt.sbprintf(out, " %s %d", phase, code)
			} else {
				fmt.sbprintf(out, " %s -", phase)
			}
		case "scroll":
			// The whole lines a grid's notch covers, `-` off a grid.
			if n, ok := kui.as_i64(kui.get(p, "lines")); ok {
				fmt.sbprintf(out, " %d", n)
			} else {
				fmt.sbprintf(out, " -")
			}
		case "drop":
			l := kui.get(p, "paths")
			fmt.sbprintf(out, " %s %d", phase, kui.length(l) if l != nil else 0)
		case "open":
			l := kui.get(p, "paths")
			fmt.sbprintf(out, " %d", kui.length(l) if l != nil else 0)
		case "text":
			// A paste's markers, each only when it is set (backlog F84).
			for marker in ([]string{"concealed", "transient"}) {
				if on, ok := kui.as_bool(kui.get(p, marker)); ok && on do fmt.sbprintf(out, " %s", marker)
			}
		}
		strings.write_byte(out, '\n')
	}
}

// The protocol conformance::drive documents: the window facts, then a frame,
// then each step followed by another frame, then the last frame's output.
run_scene :: proc(scene: Scene, env: Env, steps: []Step, out: ^strings.Builder) {
	ui := kui.new_ui()
	defer kui.free_ui(ui)
	kui.set_diagnostics(ui, true)
	if env.set {
		kui.env_set_window(
			ui,
			kui.WINDOW_MAIN,
			env.custom_chrome != 0,
			env.maximized != 0,
			env.fullscreen != 0,
			f32(env.controls_w),
			f32(env.controls_h),
		)
	}
	f := fixtures(ui)

	events := strings.builder_make(context.temp_allocator)
	cmds := strings.builder_make(context.temp_allocator)
	audio := strings.builder_make(context.temp_allocator)
	phase := 0
	for i in 0 ..= len(steps) {
		if i > 0 {
			// Four steps are not input: the clock, the view changing its
			// mind, and the OS closing a window or dismissing a popup.
			s := steps[i - 1]
			switch s.kind {
			case "phase":
				phase = s.a
			case "time":
				kui.set_time(ui, f64(s.a) / 1000.0)
			case "windowclosed":
				kui.window_closed(ui, u32(s.a))
				drain_events(ui, &events)
				drain_cmds(ui, &cmds)
				drain_audio(ui, &audio)
			case "windowdismissed":
				kui.window_dismissed(ui, u32(s.a), kui.Dismiss_Reason(s.b))
				drain_events(ui, &events)
			case "appearance":
				// The OS appearance moving under the app; accent, motion and
				// locale stay at "cannot tell".
				kui.env_set_system(ui, kui.Appearance(s.a), 0, .Unknown, "")
				drain_events(ui, &events)
			case:
				apply(ui, s)
				drain_events(ui, &events)
				drain_cmds(ui, &cmds)
				drain_audio(ui, &audio)
			}
		}
		kui.frame_begin(ui, 320, 240, 1)
		scene.build(ui, &f, phase)
		kui.frame_finish(ui)
		drain_events(ui, &events)
		drain_cmds(ui, &cmds)
		drain_audio(ui, &audio)
	}

	fmt.sbprintf(out, "scene %s\n", scene.name)
	if env.set {
		fmt.sbprintf(out, "env %d %d %d %d %d\n", env.custom_chrome, env.maximized, env.fullscreen, env.controls_w, env.controls_h)
	}
	for s in steps {
		switch {
		case s.args >= 3:
			fmt.sbprintf(out, "step %s %d %d %d\n", s.kind, s.a, s.b, s.c)
		case s.args == 2:
			fmt.sbprintf(out, "step %s %d %d\n", s.kind, s.a, s.b)
		case s.args == 1:
			fmt.sbprintf(out, "step %s %d\n", s.kind, s.a)
		case:
			fmt.sbprintf(out, "step %s\n", s.kind)
		}
	}

	if title, ok := kui.window_title_get(ui); ok {
		fmt.sbprintf(out, "title %s\n", title)
	} else {
		fmt.sbprintf(out, "title -\n")
	}
	fmt.sbprintf(out, "always-on-top %d\n", 1 if kui.always_on_top_get(ui) else 0)
	fmt.sbprintf(out, "secure-input %d\n", 1 if kui.secure_input_get(ui) else 0)
	option_names := [4]string{"none", "left", "right", "both"}
	o := int(kui.option_as_alt_get(ui))
	fmt.sbprintf(out, "option-as-alt %s\n", option_names[o] if o < 4 else "none")
	fmt.sbprintf(out, "ime-off %d\n", 1 if kui.ime_off_get(ui) else 0)

	dd := kui.draw_data(ui)
	fmt.sbprintf(out, "quads %d %016x\n", len(dd.quads), quad_digest(dd.quads, dd.clips))
	kinds: [10]int
	for q in dd.quads {
		if u32(q.kind) < 10 do kinds[int(q.kind)] += 1
	}
	fmt.sbprintf(
		out,
		"kinds %d %d %d %d %d %d %d %d %d %d\n",
		kinds[0],
		kinds[1],
		kinds[2],
		kinds[3],
		kinds[4],
		kinds[5],
		kinds[6],
		kinds[7],
		kinds[8],
		kinds[9],
	)
	// A fragment's parameters ride a side list, not the quad, so the digest
	// cannot reach them; the report carries them as bits.
	for d, i in dd.fragments {
		fmt.sbprintf(out, "fragment %d", i)
		for p in d.params do fmt.sbprintf(out, " %08x", transmute(u32)p)
		strings.write_byte(out, '\n')
	}
	// Where a fragment's image is, for the draws that have one.
	for d, i in dd.fragments {
		if d.image_source == .None do continue
		fmt.sbprintf(out, "fragment-image %d %s ", i, "atlas" if d.image_source == .Atlas else "texture")
		if d.image_source == .Texture {
			fmt.sbprintf(out, "%d", d.image_texture)
		} else {
			strings.write_string(out, "-")
		}
		fmt.sbprintf(out, " %d %d %d %d\n", d.image_uv[0], d.image_uv[1], d.image_uv[2], d.image_uv[3])
	}
	// A texture quad's texel rect rides the side list the same way.
	for t, i in dd.textures {
		fmt.sbprintf(out, "texture %d %d %d %d %d\n", i, t.uv[0], t.uv[1], t.uv[2], t.uv[3])
	}

	nodes := kui.access_tree(ui)
	// Depth from the parent chain: the tree comes back in tree order, so a
	// node's parent is always already in the table.
	keys := make([]u64, len(nodes), context.temp_allocator)
	depths := make([]int, len(nodes), context.temp_allocator)
	for a, i in nodes {
		depth := 0
		for j in 0 ..< i {
			if keys[j] == a.parent {
				depth = depths[j] + 1
				break
			}
		}
		keys[i] = a.key
		depths[i] = depth
		actions := strings.builder_make(context.temp_allocator)
		for name, bit in ACTION_NAMES {
			if kui.Access_Action(bit) not_in a.actions do continue
			if strings.builder_len(actions) > 0 do strings.write_byte(&actions, ',')
			strings.write_string(&actions, name)
		}
		checked := "m" if .Mixed in a.flags else "-" if .Checked_Set not_in a.flags else "1" if .Checked in a.flags else "0"
		selected := "-" if .Selected_Set not_in a.flags else "1" if .Selected in a.flags else "0"
		orientation := "h" if a.orientation == .Horizontal else "v" if a.orientation == .Vertical else "-"
		live := "p" if .Live_Polite in a.flags else "a" if .Live_Assertive in a.flags else "-"
		action_list := strings.to_string(actions)
		// The rect as f32 bits, cut to the node's clip (backlog F93).
		fmt.sbprintf(
			out,
			"node %d %016x %s %d %d %s %s %s %s %d %08x %08x %08x %08x %s %s | %s | %s\n",
			depth,
			a.key,
			role_name(a.role),
			1 if .Focused in a.flags else 0,
			1 if .Disabled in a.flags else 0,
			checked,
			selected,
			orientation,
			live,
			1 if .Has_Scroll in a.flags else 0,
			transmute(u32)a.x,
			transmute(u32)a.y,
			transmute(u32)a.w,
			transmute(u32)a.h,
			action_list if len(action_list) > 0 else "-",
			a.name,
			a.description,
			a.value,
		)
	}

	strings.write_string(out, strings.to_string(events))
	strings.write_string(out, strings.to_string(cmds))
	strings.write_string(out, strings.to_string(audio))

	for said in kui.take_announcements(ui) {
		fmt.sbprintf(out, "announce %s %s\n", "assertive" if said.live == .Assertive else "polite", said.text)
	}
	for w in kui.take_warnings(ui) {
		fmt.sbprintf(out, "warn %s\n", w.code)
	}
	strings.write_string(out, "end\n")
}

// -- Reading the reference --------------------------------------------------------

find_scene :: proc(name: string) -> (Scene, bool) {
	for s in SCENES {
		if s.name == name do return s, true
	}
	return {}, false
}

// The leading integers of `fields`, as sscanf's %d would read them: up to
// three, stopping at the first that is not one.
leading_ints :: proc(fields: []string, out: []int) -> (n: int) {
	for field in fields {
		if n == len(out) do break
		v, ok := strconv.parse_int(field, 10)
		if !ok do break
		out[n] = v
		n += 1
	}
	return
}

// Prints the first line the two reports disagree on - a mismatched digest
// says "the geometry moved", a mismatched node line says where.
report_diff :: proc(want, got: string) {
	a, b := want, got
	line := 1
	for len(a) > 0 && len(b) > 0 {
		ae := strings.index_byte(a, '\n')
		be := strings.index_byte(b, '\n')
		al := a[:ae] if ae >= 0 else a
		bl := b[:be] if be >= 0 else b
		if al != bl {
			fmt.eprintf("  line %d:\n    reference: %s\n    Odin:      %s\n", line, al, bl)
			return
		}
		if ae < 0 || be < 0 do break
		a = a[ae + 1:]
		b = b[be + 1:]
		line += 1
	}
	fmt.eprintf("  the reports differ in length (reference %d bytes, Odin %d)\n", len(want), len(got))
}

// conformance.c's CONF_MAX_STEPS: a limit on one scene's replay.
MAX_STEPS :: 32

conformance :: proc(path: string) -> int {
	data, err := os.read_entire_file(path, context.allocator)
	if err != nil {
		fmt.eprintf(
			"conformance: cannot read %s\n  generate it with: cargo run -p kui-core --features conformance --example conformance-dump -- %s\n",
			path,
			path,
		)
		return 1
	}
	defer delete(data)

	scenes, bad := 0, 0
	rest := string(data)
	for {
		at := strings.index(rest, "scene ")
		if at < 0 do break
		rest = rest[at:]
		// A block runs from its "scene " line to the "end\n" that closes it.
		stop := strings.index(rest, "\nend\n")
		if stop < 0 do break
		block := rest[:stop + 5]
		rest = rest[stop + 5:]

		name := ""
		steps := make([dynamic]Step, context.temp_allocator)
		env: Env
		lines := block
		for line in strings.split_lines_iterator(&lines) {
			fields := strings.fields(line, context.temp_allocator)
			if len(fields) == 0 do continue
			switch fields[0] {
			case "scene":
				if name == "" && len(fields) > 1 do name = fields[1]
			case "env":
				env.set = true
				v: [5]int
				leading_ints(fields[1:], v[:])
				env.custom_chrome, env.maximized, env.fullscreen, env.controls_w, env.controls_h = v[0], v[1], v[2], v[3], v[4]
			case "step":
				// Dropping a step silently would replay a different scene and
				// blame the difference on the lowering.
				if len(steps) == MAX_STEPS {
					fmt.eprintfln("conformance: scene '%s' replays more than %d steps", name, MAX_STEPS)
					os.exit(1)
				}
				s := Step {
					kind = fields[1] if len(fields) > 1 else "",
				}
				v: [3]int
				if len(fields) > 2 do s.args = leading_ints(fields[2:], v[:])
				s.a, s.b, s.c = v[0], v[1], v[2]
				append(&steps, s)
			}
		}

		scene, found := find_scene(name)
		if !found {
			fmt.eprintfln("FAIL: no Odin scene for '%s' - every corpus scene needs one", name)
			bad += 1
		} else {
			got := strings.builder_make()
			run_scene(scene, env, steps[:], &got)
			if strings.to_string(got) != block {
				fmt.eprintfln("FAIL: scene '%s' lowers differently from Odin than from kui-core", name)
				report_diff(block, strings.to_string(got))
				bad += 1
			}
			strings.builder_destroy(&got)
		}
		scenes += 1
		free_all(context.temp_allocator)
	}

	if scenes == 0 {
		fmt.eprintfln("conformance: %s holds no scenes", path)
		return 1
	}
	if scenes != len(SCENES) {
		fmt.eprintfln("FAIL: the reference has %d scenes, Odin builds %d", scenes, len(SCENES))
		bad += 1
	}
	if bad > 0 {
		fmt.eprintfln("conformance: %d of %d scene(s) failed", bad, scenes)
		return 1
	}
	fmt.printfln("conformance: %d scenes match the reference", scenes)
	return 0
}

main :: proc() {
	if !kui.abi_ok() {
		fmt.eprintfln("conformance: libkui_ffi implements another ABI than kui.h %d", kui.ABI_VERSION)
		os.exit(1)
	}
	// --headless is what the test runner passes; there is no window either
	// way.
	path := ""
	for arg in os.args[1:] {
		if arg == "--headless" do continue
		path = arg
	}
	if path == "" {
		if v, found := os.lookup_env("KUI_CONFORMANCE", context.allocator); found && v != "" {
			path = v
		} else {
			path = "target/conformance.txt"
		}
	}
	os.exit(conformance(path))
}
