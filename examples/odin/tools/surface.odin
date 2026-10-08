// The header walk, from Odin: examples/c/tools/surface.c ported through the
// typed layer (package kui), so every C function of kui.h is called at
// runtime through the door an Odin app would use, and what comes back is
// checked. A self-test of the binding, not an example.
//
//   nu scripts/odin.nu run surface        exit 0 on a clean walk
//   nu scripts/odin.nu test               runs it with --headless, ignored
//
// It walks what surface.c walks, in its order: measurement, keyed nodes and
// animation, hover and keyboard focus, editors, accessibility, images,
// fonts, audio, window chrome, diagnostics, values, the env setters, the
// theme, menus, and the readers. The colours stay literal on purpose, as in
// the C: a fixture that read the theme would be asserting against the
// host's OS settings. What the typed layer does not reach - the size
// handshake its doors make themselves, a NULL out-param - says so where the
// C does it.
//
// surface.c's conformance corpus (the shared scene report) is out of scope
// here: it proves the bindings agree with each other, not that a door works.
package surface

import "core:fmt"
import "core:os"
import "core:strings"
import kui "../../../packages/odin/kui"

fails := 0

// One assertion: says what failed and counts it, so the walk reports every
// miss rather than the first.
check :: proc(ok: bool, what: string) {
	if !ok {
		fmt.eprintln("FAIL:", what)
		fails += 1
	}
}

// Body for the widgets that host custom content.
body_text :: proc(s: ^string, ui: ^kui.Ui) {
	kui.text(ui, s^, {size = 12})
}

Keys :: struct {
	image:                                                  kui.Image, // in: a registered image
	sound:                                                  kui.Sound, // in: a registered sound
	claim_focus:                                            bool, // in: declare the editor focused this frame
	card, slider, sink, editor, input, select, drag, child: u64, // out: node keys
	checkbox, gain:                                         u64, // out: the stock controls' keys
	hitline, region:                                        u64, // out: the selectable row, the focus region
}

surface_view :: proc(k: ^Keys, ui: ^kui.Ui) {
	kui.root(ui, {width = kui.GROW, height = kui.GROW, gap = 8})
	kui.window_title(ui, "surface")
	kui.set_always_on_top(ui, true)
	kui.set_secure_input(ui, true)
	kui.set_option_as_alt(ui, .Left)
	kui.set_ime_off(ui, true)

	// Window chrome, each piece in its own keyed slot: two titlebars as
	// siblings would share a key, and per-key state with it.
	if kui.box(ui, {key = "chrome-plain", dir = .Row, width = kui.GROW}) {
		kui.titlebar(ui, "surface")
	}
	if kui.box(ui, {key = "chrome-custom", dir = .Row, width = kui.GROW}) {
		strip := "tab strip"
		kui.titlebar_with(ui, &strip, body_text)
	}
	if kui.box(ui, {key = "chrome-buttons", dir = .Row, width = kui.GROW}) {
		kui.window_buttons(ui)
	}

	// One card carrying most of Spec at once: clamps, per-corner radii,
	// transition + keyframes + entrance, a hover group, semantics, focus,
	// and a drop zone (ADR 0031) with the colour it takes under a drag.
	stops := []kui.Keyframe {
		{set = {.At, .Bg}, at = 0, bg = 0x161820ff},
		{set = {.At, .Bg, .Radius}, at = 1, bg = 0x1b1e28ff, radius = 10},
	}
	card := kui.Spec {
		key          = "card",
		dir          = .Column,
		gap          = 8,
		cross_align  = .Start,
		pad          = kui.pad(12),
		min_width    = kui.px(240),
		max_width    = kui.px(600),
		min_height   = kui.px(40),
		max_height   = kui.px(500),
		bg           = 0x161820ff,
		border_color = 0x2a2d3aff,
		border_w     = 1,
		overflow     = {.Clip, .Scroll_Y},
		radius_tl    = 10,
		radius_tr    = 10,
		radius_br    = 2,
		radius_bl    = 2,
		transition   = 120,
		easing       = .In_Out,
		slide        = true,
		hover_bg     = 0x1b1e28ff,
		pressed_bg   = 0x22252fff,
		hover_group  = "card",
		repeat       = .Alternate,
		delay        = 20,
		keyframes    = stops,
		enter        = {set = {.Offset, .Bg}, dx = -12, bg = 0},
		on_layout    = "card",
		role         = .Group,
		label        = "surface card",
		focusable    = true,
		focus_bg     = 0x2b3350ff,
		on_drop      = "files",
		drop_bg      = 0x2b4a50ff,
	}
	k.card = kui.open(ui, card)
	{
		k.child = kui.child_key(ui, "slot")

		// A slider: declared role, value and range; the arrows nudge it.
		k.slider = kui.open(
			ui,
			{
				key = "slider",
				width = kui.px(160),
				height = kui.px(20),
				bg = 0x2a2d3aff,
				radius = 10,
				role = .Slider,
				label = "volume",
				value_now = 40,
				value_min = 0,
				value_max = 100,
			},
		)
		kui.close(ui)

		// A checked switch that is also disabled: inert, and says so.
		if kui.box(
			ui,
			{
				key = "switch",
				width = kui.px(40),
				height = kui.px(20),
				bg = 0x2a2d3aff,
				radius = 10,
				hoverable = true,
				role = .Switch,
				label = "mute",
				checked = true,
				disabled = true,
			},
		) {}

		// A key sink (on_key) that is also a hover sink (on_hover). Focusable,
		// so Tab reaches it; key_up, so it hears releases too.
		k.sink = kui.open(
			ui,
			{
				key = "sink",
				width = kui.px(120),
				height = kui.px(24),
				bg = 0x11131aff,
				focusable = true,
				role = .Group,
				label = "key sink",
				key_up = true,
				on_key = "sink",
				on_hover = "sink",
			},
		)
		kui.close(ui)

		// A draggable strip, floated into the card's top-right corner.
		strip := kui.Spec {
			width  = kui.px(80),
			height = kui.px(16),
			bg     = 0x3a3f52ff,
			float  = {mode = .Parent, anchor_x = .End, anchor_y = .Start, self_x = .End, self_y = .Start, dx = -4, dy = 4, fit = true},
		}
		k.drag = kui.open_draggable(ui, "strip", strip, nil, "strip")
		kui.close(ui)

		// Editors: the styled single-line widget and a raw multiline node.
		k.input = kui.text_input(ui, "name", "ada")
		// The stock select: the rows a context menu takes, the second in
		// force; the choice comes back as a menu event on this key.
		langs := []kui.Menu_Item{{label = "English", role = .Custom}, {label = "Deutsch", role = .Custom}}
		k.select = kui.select(ui, "language", langs, 1)
		// The stock controls (ADR 0034): a mixed checkbox, a radio group of
		// two, a switch, and a slider that asks the core for its changes.
		k.checkbox = kui.checkbox(ui, "All", false, "all", {mixed = true})
		if kui.radio_group(ui, "Theme") {
			kui.radio(ui, "Light", false, "light")
			kui.radio(ui, "Dark", true, "dark")
		}
		kui.toggle(ui, "Wi-Fi", true, "wifi")
		k.gain = kui.slider(ui, "Gain", 40, 0, 100, "gain", {value_step = 5})
		mono := kui.Text_Style {
			size   = 13,
			family = .Mono,
			wrap   = .Glyph,
		}
		editor := kui.Spec {
			width  = {.Percent, 0.5},
			height = kui.px(48),
			bg     = 0x11131aff,
			radius = 4,
			label  = "notes",
		}
		k.editor = kui.text_edit(ui, "notes", "line one", {.Multiline}, mono, editor)
		if k.claim_focus do kui.set_key_focus(ui, k.editor)

		// An image node, a tooltip gated on hover, and the latency panels.
		kui.image(ui, k.image, {width = kui.px(32), height = kui.FIT, radius = 4, role = .Image, label = "swatch"})
		if kui.is_hovered(ui, k.card) || kui.is_pressed(ui, k.card) {
			kui.tooltip(ui, "the card")
		} else {
			tip := "custom tip"
			kui.tooltip_with(ui, &tip, body_text)
		}
		// An audio node: a playback retained for as long as the frame declares
		// it. Without a device it just queues commands for the host.
		// Volume unsaid is KUI_AUDIO_INIT's 1, not the zero value's silence.
		kui.audio(ui, "bed", {src = k.sound, looped = true, paused = true}, "bed")

		// A line of text runs a custom editor would draw, for the two text
		// queries: the row's key answers for every run inside it. Ligatures
		// off through `features`.
		hit_mono := kui.Text_Style {
			size     = 16,
			family   = .Mono,
			features = "liga=0 calt=0",
		}
		k.hitline = kui.open(ui, {key = "hitline", dir = .Row, selectable = true})
		kui.text(ui, "let ", hit_mono)
		kui.text(ui, "value", hit_mono)
		kui.text(ui, " = 1;", hit_mono)
		kui.close(ui)

		// A terminal's screen as one node: cells travel as a Cell slice.
		screen: [2 * 6]kui.Cell
		top, bottom := "hello!", "world."
		for i in 0 ..< 6 {
			screen[i].ch = rune(top[i])
			screen[i].fg = 0xd6d8e0ff
			screen[6 + i].ch = rune(bottom[i])
			screen[6 + i].fg = 0x73d98cff
			screen[6 + i].bg = 0x1a1d27ff if i < 3 else 0
		}
		screen[0].flags = {.Bold}
		cell_style := kui.Text_Style {
			size        = 13,
			family      = .Mono,
			line_height = 18,
		}
		kui.cells(ui, "term", 2, 6, screen[:], cell_style, {}, nil, nil, nil, 1, 2, .Block, 0x6a8bffff, 0)

		// A focus region (ADR 0022): a ring of its own, entered by name.
		k.region = kui.open(ui, {key = "dock", dir = .Row, focus_region = true})
		if kui.box(
			ui,
			{
				key = "dock-button",
				width = kui.px(20),
				height = kui.px(20),
				bg = 0x2a2d3aff,
				focusable = true,
				role = .Button,
				label = "dock button",
			},
		) {}
		kui.close(ui)

		kui.latency_graph(ui)
		kui.latency_hud(ui, .End, .Start)
	}
	kui.close(ui)
}

surface :: proc() -> bool {
	ui := kui.new_ui()
	if ui == nil {
		fmt.eprintln("FAIL: new_ui (the library's ABI is not the one this binding was generated against)")
		return false
	}
	kui.set_diagnostics(ui, true)
	kui.env_set(ui, 120, true)
	kui.env_set_window(ui, kui.WINDOW_MAIN, true, false, false, 78, 28)
	// The audio row (ADR 0021, 6a): a fact the host pushes and a view reads;
	// a code this build has no name for reads back as closed.
	kui.env_set_audio(ui, .Open, 2)
	kui.env_set_audio(ui, kui.Audio_Device(99), 0)
	kui.set_subpixel_text(ui, false)

	// Measurement works before the first frame.
	body := kui.Text_Style {
		size = 16,
	}
	one, one_ok := kui.measure_text(ui, "measure me", body, 0)
	check(one_ok, "measure_text")
	check(one.width > 0 && one.height > 0 && one.lines == 1, "unwrapped is one line")
	wrapped, wrapped_ok := kui.measure_text(ui, "measure me measure me", body, one.width)
	check(wrapped_ok, "measure_text wrapped")
	check(wrapped.lines > 1 && wrapped.height > one.height, "a narrow width wraps")
	// A decorated span measures like a plain one, since decorations are paint.
	spans := []kui.Span {
		{text = "rich ", flags = {.Bold, .Underline}, bg = 0x3b5bd455},
		{text = "measure", color = 0x73d98cff, flags = {.Italic, .Strikethrough}},
	}
	rich, rich_ok := kui.measure_rich_text(ui, spans, body, 0)
	check(rich_ok, "measure_rich_text")
	check(rich.width > 0 && rich.lines == 1, "spans measure as one line")

	// Size expressions (backlog F109): built from parts or parsed, one entry
	// either way; a length reduces to Fixed, a lone percentage to Percent,
	// and a part that is no size makes the whole Fit.
	built := kui.clamp_size(kui.px(400), kui.pct(80), kui.px(1000))
	parsed, parsed_ok := kui.size("clamp(400px, 80%, 1000px)")
	check(parsed_ok, "size parses a clamp")
	check(built.tag == .Calc && parsed.tag == .Calc && built.value == parsed.value, "a clamp built and spelled is one calc")
	smaller := kui.size_min(kui.px(300), kui.px(400))
	check(smaller.tag == .Fixed && smaller.value == 300, "min of lengths is a length")
	check(kui.size_max(kui.pct(50), kui.px(300)).tag == .Calc, "max over a percentage is a calc")
	check(kui.clamp_size(kui.FIT, kui.pct(50), kui.px(9)).tag == .Fit, "fit is no size")
	_, three := kui.size("clamp(1, 2)")
	check(!three, "clamp takes three")

	// Resources. Unusable input is a 0 handle, not a crash.
	junk := [4]u8{0, 1, 2, 3}
	check(kui.font_add(ui, junk[:]) == 0, "garbage is not a font")
	check(kui.font_load_file(ui, "/nonexistent.ttf") == 0, "missing font file")
	check(kui.font_load_dir(ui, "/nonexistent") == 0, "missing font dir")
	check(kui.font_reload_system(ui) == 0, "a rescan with nothing installed since finds nothing")
	{
		fb := []kui.Font{kui.font_add_system(ui, "Menlo")}
		kui.font_set_fallback(ui, fb)
		kui.font_set_fallback(ui, nil) // the platform's list alone
	}
	kui.font_remove(ui, kui.font_add_system(ui, "Menlo")) // 0 is a no-op

	rgba := [2 * 2 * 4]u8{255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255}
	k: Keys
	k.image = kui.image_add(ui, 2, 2, rgba[:])
	check(k.image != 0, "image_add")
	k.sound = kui.sound_add(ui, junk[:])
	check(k.sound != 0, "sound_add")

	// Two frames on a clock: the second leaves the keyframes mid-flight.
	kui.set_time(ui, 0)
	kui.frame(ui, &k, surface_view, 800, 600, 2)
	kui.set_time(ui, 0.05)
	check(kui.now(ui) == 0.05, "now reads the clock set_time set")
	kui.request_frame_at(ui, 3)
	check(kui.next_frame_at(ui) == 3, "request_frame_at sets a deadline")
	kui.frame(ui, &k, surface_view, 800, 600, 2)

	check(k.checkbox != 0 && k.gain != 0, "checkbox and slider return their keys")
	check(
		k.card != 0 && k.slider != 0 && k.sink != 0 && k.editor != 0 && k.input != 0 && k.select != 0 && k.drag != 0 && k.child != 0,
		"every node got a key",
	)
	check(kui.animating(ui), "the keyframes keep animating")
	// And by kind (backlog F64): at 50 ms the card's 120 ms entrance is
	// mid-flight beside the cycle, each its own bit.
	check(kui.owed(ui) & {.Cycle, .Transition} == {.Cycle, .Transition}, "owed: the cycle and the entrance, each its own bit")
	title, title_ok := kui.window_title_get(ui)
	check(title_ok && strings.contains(title, "surface"), "window_title_get")
	// The level, secure input and Option-as-Alt: the frame asked, the host
	// says what it did, and the two are separate facts.
	check(kui.always_on_top_get(ui), "always_on_top_get")
	check(kui.secure_input_get(ui), "secure_input_get")
	check(kui.option_as_alt_get(ui) == .Left, "option_as_alt_get")
	check(kui.ime_off_get(ui), "ime_off_get")
	kui.env_set_always_on_top(ui, true)
	// What is behind the window, as the host got it (backlog F126), read back
	// as a view reads it.
	check(kui.ctx_backdrop(ui) == .Opaque, "a window is opaque until told")
	kui.env_set_backdrop(ui, .Blur)
	check(kui.ctx_backdrop(ui) == .Blur, "ctx_backdrop reads the blur back")

	dd, dd_ok := kui.draw_data(ui)
	check(dd_ok && len(dd.quads) > 0 && dd.scale == 2, "the surface frame drew at scale 2")
	check(dd.atlas_size > 0 && dd.atlas_pixels != nil, "the glyph atlas is there")
	images := 0
	for q in dd.quads {
		if q.kind == .Image do images += 1
	}
	check(images > 0, "the image node drew")
	quad_count := len(dd.quads)

	// Text queries against the frame that finished: a point becomes a byte
	// offset across the row's three runs, and a byte becomes a caret rect.
	hitline := kui.key_of(ui, "hitline")
	start, start_ok := kui.caret_rect(ui, hitline, 0)
	check(hitline != 0 && start_ok && start.h > 0 && start.w == 0, "caret_rect at the start")
	th, th_ok := kui.text_hit(ui, hitline, start.x + 1, start.y + 1)
	check(th_ok && th.byte == 0 && th.line == 0, "text_hit at the start")
	end, end_ok := kui.caret_rect(ui, hitline, 999)
	check(end_ok && end.x > start.x, "past the text is the end")
	th, th_ok = kui.text_hit(ui, hitline, end.x + 50, start.y + 1)
	check(th_ok && th.byte == 14, "far right of the line is its end, across the runs")
	th, th_ok = kui.text_hit(ui, hitline, (start.x + end.x) / 2, start.y + 1)
	check(th_ok && th.byte > 0 && th.byte < 14, "the middle is inside")
	_, nothing_ok := kui.caret_rect(ui, 12345, 0)
	check(!nothing_ok, "a key that drew no text answers false")
	_, ime_ok := kui.ime_rect(ui)
	check(!ime_ok, "nothing with a caret is focused: no candidate window")
	check(kui.key_of(ui, "term") != 0, "cells keyed its node")

	// Pointer state and scrolling.
	kui.input_cursor(ui, 400, 300)
	kui.input_scroll(ui, 0, -3)
	kui.input_cursor_left(ui)
	kui.input_modifiers(ui, {.Shift, .Ctrl})

	// Files dragged in from the OS (ADR 0031): over the card, which is a
	// zone, then released there. The events land below with the rest.
	{
		card_rect, card_ok := kui.layout_of(ui, k.card)
		check(card_ok, "the card has a rect")
		cx, cy := card_rect.x + 4, card_rect.y + 4
		paths := []string{"/drop/1.txt", "/drop/2.txt"}
		kui.input_drag_files(ui, paths, cx, cy)
		check(kui.drop_target(ui) == k.card, "the files are over the card")
		check(kui.is_drop_target(ui, k.card), "and the card says so")
		kui.input_drag_files(ui, paths, cx + 8, cy + 8)
		kui.input_drag_files(ui, paths, -50, -50)
		check(kui.drop_target(ui) == 0, "off every zone: nothing lit")
		kui.input_drag_files(ui, paths, cx, cy)
		kui.input_drop_files(ui, paths, cx, cy)
		check(kui.drop_target(ui) == 0, "a drop ends the hover")
		kui.input_drag_files(ui, paths, cx, cy)
		kui.input_drag_cancel(ui)
		check(kui.drop_target(ui) == 0, "a cancel ends it too")
	}

	// Keyboard focus: Tab walks the ring, and the ring is visible.
	kui.focus(ui, 0)
	check(kui.focused(ui) == 0, "focus(0) blurs")
	kui.focus_next(ui, true)
	first := kui.focused(ui)
	check(first != 0 && kui.focus_visible(ui), "Tab focuses something, visibly")
	kui.focus_next(ui, true)
	check(kui.focused(ui) != first, "Tab moves on")
	kui.focus_next(ui, false)
	check(kui.focused(ui) == first, "Shift-Tab comes back")

	// The key sink takes focus like anything else, and a host drives its
	// raw keys directly. A node the host never got an event from is named by
	// the label it was opened under.
	check(kui.key_of(ui, "sink") == k.sink, "key_of resolves a declared label")
	check(kui.key_of(ui, "no such node") == 0, "key_of is 0 for an undeclared one")
	kui.focus(ui, kui.key_of(ui, "sink"))
	check(kui.is_focused(ui, k.sink), "is_focused")
	// "" for `physical` means "the key I just named". A held key on a sink
	// with key_up is one {kind="key"} payload twice, "down" then "up".
	kui.input_key_down(ui, "w", "", {}, "", false)
	kui.input_key_down(ui, "w", "", {}, "", true) // OS auto-repeat
	kui.input_key_up(ui, "w", "", {})
	// A Russian layout puts "ц" on the key US-QWERTY prints W on: the sink
	// still hears code="w", with physical="w" beside it.
	kui.input_key_down(ui, "ц", "w", {nonlatin = true}, "", false)
	kui.input_key_up(ui, "ц", "w", {nonlatin = true})
	// Held over a focus change: the sink hears the release anyway.
	// The kmods word is a bit_field: the held modifiers, the locks, the twin.
	kui.input_key_down(ui, "a", "", {ctrl = true, caps_lock = true, location = .Left}, "", false)
	kui.release_held_keys(ui)
	kui.focus(ui, k.sink)
	kui.input_key(ui, .Right, {})
	kui.input_key(ui, .Tab, {})
	// Those are one half of a key each; input_press is the whole key, the
	// sink hearing the press before the core acts on it.
	kui.focus(ui, k.sink)
	kui.input_press(ui, "w", "", {}, "", false)
	kui.input_release(ui, "w", "", {})
	// An IME on a sink: the composition and its commit arrive as data.
	kui.input_preedit(ui, "日本", 0, 6)
	kui.input_commit(ui, "日本語")

	// Editors: type, compose, select, read back.
	kui.focus(ui, k.editor)
	kui.input_text(ui, "two ")
	kui.input_preedit(ui, "mid", 0, 3)
	kui.input_preedit(ui, "", 0, 0)
	kui.input_key(ui, .End, {.Doc})
	kui.input_key(ui, .Left, {.Shift, .Word})
	text, text_ok := kui.edit_text(ui, k.editor)
	check(text_ok, "edit_text")
	check(strings.contains(text, "two ") && strings.contains(text, "line one"), "the typing landed")
	kui.edit_set_text(ui, k.editor, "replaced")
	text, text_ok = kui.edit_text(ui, k.editor)
	check(text_ok && strings.contains(text, "replaced"), "edit_set_text")
	text, text_ok = kui.edit_text(ui, k.input)
	check(text_ok && len(text) == 3, "the text input kept \"ada\"")

	// A third frame: re-lays out the edited text, and picks up the focus the
	// view declares (set_key_focus is edge-triggered, so it lands once).
	k.claim_focus = true
	kui.set_time(ui, 0.2)
	kui.frame(ui, &k, surface_view, 800, 600, 2)
	check(kui.focused(ui) == k.editor, "the view declared the focus")

	// Accessibility: the tree, an editor's runs, and requests coming back.
	nodes := kui.access_tree(ui)
	total := len(nodes)
	check(total > 1, "the access tree has nodes")
	sliders, disabled, editors, mixed_boxes := 0, 0, 0, 0
	run_count := 0
	for &n in nodes {
		if n.role == .Slider {
			sliders += 1
			check(.Has_Number in n.flags, "the slider reports a value")
			check(n.value_now == 40 && n.value_max == 100, "and its range")
			check(.Increment in n.actions, "and accepts increments")
		}
		if .Disabled in n.flags do disabled += 1
		if .Mixed in n.flags do mixed_boxes += 1
		if n.key == k.editor {
			editors += 1
			check(strings.contains(n.value, "replaced"), "the editor's text is in the tree")
			check(.Focused in n.flags, "and it holds focus")
			runs := kui.access_runs(ui, n.key)
			run_count = len(runs)
			check(u32(run_count) == n.run_count, "access_runs matches run_count")
			if run_count > 0 {
				r := runs[0]
				check(len(r.text) > 0 && r.char_count > 0, "a run has characters")
				check(u32(len(r.char_widths)) == r.char_count, "char_widths is one width a character")
				check(len(r.char_positions) == len(r.char_widths) && len(r.char_lengths) == len(r.char_widths), "and so are the positions and lengths")
				check(u32(len(r.word_starts)) == r.word_start_count, "word_starts is word_start_count long")
				// The last character's width is all of it: positions add up.
				last := len(r.char_widths) - 1
				check(r.char_positions[last] + r.char_widths[last] <= r.w + 0.5, "the characters fit the run")
				kui.input_access_text(ui, n.key, .Set_Text_Selection, r.key, 0, r.key, 1, "")
			}
		}
	}
	check(sliders == 2, "two sliders in the tree, one of them stock")
	check(mixed_boxes == 1, "the mixed checkbox says so")
	check(disabled == 1, "the disabled switch says so")
	check(editors == 1, "the multiline editor is in the tree")
	kui.input_access(ui, k.slider, .Increment, "")
	kui.input_access(ui, k.card, .Focus, "")

	// Audio without a device: commands queue for the host to apply.
	// A Play that leaves volume unsaid gets KUI_PLAY_INIT's 1.
	playback := kui.play(ui, k.sound, kui.Play{fade_in_ms = 10}, "beep")
	check(playback != 0, "play")
	kui.set_volume(ui, playback, 0.5, 20)
	kui.pause(ui, playback, 5)
	kui.resume(ui, playback, 5)
	kui.set_master_volume(ui, 0.8, 0)
	kui.stop(ui, playback, 0)
	cmds := kui.take_audio_commands(ui)
	check(len(cmds) >= 6 && cmds[0].kind == .Play, "the audio commands queued")
	check(len(kui.take_audio_commands(ui)) == 0, "and drained")
	kui.audio_ended(ui, playback)
	// The other two answers a device owes (backlog F36): an imperative stop
	// cut short reports nothing, and a refusal names the node that asked.
	kui.audio_truncated(ui, playback, 0.25)
	kui.audio_refused(ui, playback)
	{
		refused := kui.take_warnings(ui)
		check(
			len(refused) == 1 && refused[0].code == "playback-refused",
			"a refused playback is one warning, and a truncation on an imperative stop none",
		)
	}
	kui.sound_remove(ui, k.sound)

	// Window chrome turns clicks into commands rather than events.
	for _ in kui.take_window_command(ui) {}

	// Size and focus are the app asking: queued the same way, drained in
	// order and once, carrying the window they name.
	{
		kui.set_window_size(ui, kui.WINDOW_MAIN, 640, 480)
		kui.focus_window(ui, kui.WINDOW_MAIN)
		cmd, ok := kui.take_window_command(ui)
		check(
			ok && cmd.kind == .Set_Size && cmd.window == kui.WINDOW_MAIN && cmd.width == 640 && cmd.height == 480,
			"a size request drains with the size it asked for",
		)
		cmd, ok = kui.take_window_command(ui)
		check(ok && cmd.kind == .Focus && cmd.window == kui.WINDOW_MAIN, "and the focus request behind it")
		_, ok = kui.take_window_command(ui)
		check(!ok, "both drained once")
	}

	// surface.c's two size-handshake blocks (a short reservation refused by
	// kui_poll_event / kui_draw_data, an ABI 3 prefix filled) are not
	// expressible here: the doors fill `size` themselves, which is the point
	// of them. What they also proved - the queue's first event is the card's
	// first on_layout - is kept.
	first_ev, first_ok := kui.poll_event(ui)
	check(first_ok && kui.kind(first_ev.payload) == "layout", "the first event is the first on_layout")

	// Everything above lands as data.
	events, layouts, access, downs, ups := 0, 0, 0, 0, 0
	latin, physical, preedits, commits := 0, 0, 0, 0
	drops, drop_paths := 0, 0
	for ev in kui.poll_event(ui) {
		events += 1
		check(ev.window == kui.WINDOW_MAIN, "one window, so every event is from it")
		if ev.payload == nil do continue
		switch kui.kind(ev.payload) {
		case "layout":
			layouts += 1
		case "access":
			access += 1
		case "preedit":
			preedits += 1
		case "text":
			commits += 1
		case "drop":
			drops += 1
			if kui.length(kui.get(ev.payload, "paths")) == 2 do drop_paths += 1
		case "key":
			phase, phase_ok := kui.as_string(kui.get(ev.payload, "phase"))
			if !phase_ok do continue
			if phase == "down" do downs += 1
			if phase == "up" do ups += 1
			// The Cyrillic press folded to its position's Latin letter, and
			// `physical` rode along beside it.
			code, code_ok := kui.as_string(kui.get(ev.payload, "code"))
			if code_ok && len(code) == 1 && code[0] < 0x80 do latin += 1
			if kui.get(ev.payload, "physical") != nil do physical += 1
		}
	}
	check(events > 0, "the inputs produced events")
	check(layouts > 0, "on_layout reported the card's rect")
	check(access > 0, "the slider nudge arrived as an access event")
	// Four presses through input_key_down (w, its repeat, the Cyrillic w,
	// ctrl-a) and a release for each of the three distinct holds, plus the
	// whole-key pair input_press and input_release send.
	check(downs == 5, "the sink took the presses, repeat included")
	check(ups == 4, "every held key came back up exactly once")
	check(latin == downs + ups, "every code is a Latin key, the Cyrillic press included")
	check(physical == downs + ups, "and every one carries its position")
	check(preedits == 1 && commits == 1, "the sink heard the composition and its commit as data")
	// enter, move, leave, enter, drop (no leave after it), enter, leave.
	check(drops == 7, "every phase of the file drag landed as data")
	check(drop_paths == drops, "each carrying both paths")

	// Values round-trip, including the ones the counter never builds. A
	// KuiValue is any Odin value; encode makes one, value_free lets it go.
	vals: [dynamic]kui.Value
	keep :: proc(vals: ^[dynamic]kui.Value, msg: any) -> kui.Value {
		v := kui.encode(msg)
		append(vals, v)
		return v
	}
	{
		// The C builds a map entry by entry; a map[string]any is that here.
		n_in, x_in, on_in := -7, 1.5, true
		src := make(map[string]any, context.temp_allocator)
		src["n"], src["x"], src["on"], src["nil"] = n_in, x_in, on_in, nil
		m := keep(&vals, src)
		n, n_ok := kui.as_i64(kui.get(m, "n"))
		check(n_ok && n == -7, "as_i64 reads a map built from map[string]any")
		x, x_ok := kui.as_f64(kui.get(m, "x"))
		on, on_ok := kui.as_bool(kui.get(m, "on"))
		check(x_ok && x == 1.5 && on_ok && on, "and its float and bool")
		check(kui.is_null(kui.get(m, "nil")), "and its explicit null")
		check(kui.get(m, "missing") == nil, "a missing key is nil")
		// A kind is the derive's snake_case: a run of capitals is a word.
		HTTPGet :: struct {}
		OpenURLNow :: struct {}
		Tab_New :: struct {}
		for pair in ([][2]any{{HTTPGet{}, "http_get"}, {OpenURLNow{}, "open_url_now"}, {Tab_New{}, "tab_new"}}) {
			kind, kind_ok := kui.as_string(kui.get(keep(&vals, pair[0]), "kind"))
			check(kind_ok && kind == pair[1].(string), fmt.tprintf("a kind is %s", pair[1]))
		}
		// The same map through a struct, which encode does lower.
		Fields :: struct {
			n:   int,
			x:   f64,
			on:  bool,
			nil_: ^int `kui:"nil"`,
		}
		s := keep(&vals, Fields{n = -7, x = 1.5, on = true})
		n, n_ok = kui.as_i64(kui.get(s, "n"))
		check(n_ok && n == -7, "as_i64")
		check(kui.get(s, "nil") != nil && kui.is_null(kui.get(s, "nil")), "a nil pointer is an explicit null")
	}

	// Diagnostics were on the whole way: a clean tree raises none.
	warned := kui.take_warnings(ui)
	for w in warned do fmt.eprintfln("  warning: %s: %s", w.code, w.message)
	check(len(warned) == 0, "the surface view raises no diagnostics")

	// Declared windows (ADR 0004): a frame that names one gets an Open back
	// from the same drain the chrome commands use, carrying its id and the
	// config it was declared with; the app hears about it as data.
	// Reporting the OS close keeps it closed while it is still declared.
	{
		check(kui.ctx_window(ui) == kui.WINDOW_MAIN, "this context draws the main window")
		name, name_ok := kui.ctx_window_name(ui)
		check(name_ok && name == "main", "and is named for it")
		// activates unsaid is KUI_WINDOW_CONFIG_INIT's yes.
		cfg := kui.Window_Config {
			kind   = .Normal,
			width  = 400,
			height = 300,
		}
		kui.frame_begin(ui, 320, 240, 1)
		kui.window_declare(ui, "palette", cfg)
		kui.frame_finish(ui)
		// (surface.c's short command reservation is the handshake again.)
		cmd, ok := kui.take_window_command(ui)
		check(
			ok && cmd.kind == .Open && cmd.window == 1 && cmd.origin == 0 && cmd.config.width == 400 && cmd.config.height == 300 && (cmd.config.activates.? or_else false),
			"the declared window opens with its config",
		)
		_, ok = kui.take_window_command(ui)
		check(!ok, "one command for one window")
		opened, closed := 0, 0
		for wev in kui.poll_event(ui) {
			if kui.kind(wev.payload) != "window" do continue
			phase, phase_ok := kui.as_string(kui.get(wev.payload, "phase"))
			if !phase_ok do continue
			if phase == "opened" do opened += 1
			if phase == "closed" do closed += 1
		}
		check(opened == 1 && closed == 0, "the app hears the window open")
		kui.window_closed(ui, 1)
		kui.frame_begin(ui, 320, 240, 1)
		kui.window_declare(ui, "palette", cfg)
		kui.frame_finish(ui)
		_, ok = kui.take_window_command(ui)
		check(!ok, "a closed window still declared does not reopen")
		for wev in kui.poll_event(ui) {
			phase, phase_ok := kui.as_string(kui.get(wev.payload, "phase"))
			if phase_ok && phase == "closed" do closed += 1
		}
		check(closed == 1, "and hears it close")
		trapped := 0
		for w in kui.take_warnings(ui) do if w.code == "window-declared-while-closed" do trapped += 1
		check(trapped == 1, "the still-declared window is a named diagnostic")
		// Lapse, then declare again, with no config (C's NULL): a new window,
		// a new id, the defaults.
		kui.frame_begin(ui, 320, 240, 1)
		kui.frame_finish(ui)
		kui.frame_begin(ui, 320, 240, 1)
		kui.window_declare(ui, "palette")
		kui.frame_finish(ui)
		cmd, ok = kui.take_window_command(ui)
		check(ok && cmd.kind == .Open && cmd.window == 2 && cmd.config.width == 640, "a declaration that starts again opens anew, at the defaults")
		for _ in kui.poll_event(ui) {}
		kui.frame_begin(ui, 320, 240, 1)
		kui.frame_finish(ui)
		cmd, ok = kui.take_window_command(ui)
		check(ok && cmd.kind == .Close && cmd.window == 2, "and closes when it stops")
		for _ in kui.poll_event(ui) {}
	}

	// The application menu bar (ADR 0018): declared in a frame, read back the
	// way a host with a bar of its own reads it, and one row chosen.
	{
		sort := []kui.Menu_Item{{label = "Name", role = .Custom}, {label = "Date", role = .Custom, checked = true}}
		file := []kui.Menu_Item {
			{label = "Save", role = .Custom, id = "file.save", accel = "mod+s"},
			{role = .Separator},
			{label = "Wrap", role = .Custom, checked = true},
			// A submenu (backlog F128), read and chosen by its path.
			{label = "Sort by", role = .Custom, submenu = sort},
		}
		menus := []kui.Bar_Menu{{label = "File", items = file}}
		kui.frame_begin(ui, 320, 240, 1)
		check(kui.menu_bar(ui, menus), "menu_bar declares and draws")
		kui.frame_finish(ui)

		count, rev := kui.menu_bar_menu_count(ui)
		check(count == 1 && rev > 0, "one menu, at a revision")
		items, label, on := kui.menu_bar_menu(ui, 0)
		check(items == 4 && strings.contains(label, "File") && on, "menu_bar_menu reads the title back")
		wrap, _, role, flags, ok := kui.menu_bar_item(ui, 0, 2)
		check(ok && strings.contains(wrap, "Wrap") && role == .Custom && .Checked in flags && .Enabled in flags, "menu_bar_item reads a checked row back")
		_, accel, _, _, save_ok := kui.menu_bar_item(ui, 0, 0)
		check(save_ok && len(accel) > 0, "and the accelerator, in this platform's spelling")
		_, _, _, _, ok = kui.menu_bar_item(ui, 9, 9)
		check(!ok, "a row that is not there is false")
		_, _, _, flags, ok = kui.menu_bar_item(ui, 0, 3)
		check(ok && .Submenu in flags, "a row with rows of its own says so")
		check(kui.menu_bar_submenu_count(ui, 0, {3}) == 2 && kui.menu_bar_submenu_count(ui, 0, {}) == 4, "menu_bar_submenu_count")
		date, _, _, date_flags, date_ok := kui.menu_bar_item_path(ui, 0, {3, 1})
		check(date_ok && strings.contains(date, "Date") && .Checked in date_flags, "menu_bar_item_path reads a row inside")
		check(!kui.activate_menu_bar_path(ui, 0, {3}), "the row that opens the submenu is not chosen")

		// The choice a native bar reports: the same event a press on the
		// drawn bar's row produces.
		check(kui.activate_menu_bar_item(ui, 0, 0), "activate_menu_bar_item")
		check(kui.activate_menu_bar_path(ui, 0, {3, 0}), "activate_menu_bar_path")
		menus_heard := 0
		for mev in kui.poll_event(ui) do if strings.contains(kui.kind(mev.payload), "menu") do menus_heard += 1
		check(menus_heard == 2, "and the app hears a menu event for each")
	}

	// -- The rest of the header, so that the walk is what it says it is.

	// Values: the list half, and the readers the counter never needs.
	{
		list := keep(&vals, []any{true, 2.5, nil})
		check(kui.length(list) == 3, "length counts a list")
		b, b_ok := kui.as_bool(kui.at(list, 0))
		check(b_ok && b, "at / as_bool")
		f, f_ok := kui.as_f64(kui.at(list, 1))
		check(f_ok && f == 2.5, "as_f64")
		check(kui.is_null(kui.at(list, 2)) && kui.is_null(kui.at(list, 3)), "an explicit null and a missing entry read the same")
		// The same list from a typed tuple, which encode lowers.
		Row :: struct {
			b: bool,
			f: f64,
			z: ^int,
		}
		row := keep(&vals, Row{true, 2.5, nil})
		b, b_ok = kui.as_bool(kui.get(row, "b"))
		check(b_ok && b, "get / as_bool")
		f, f_ok = kui.as_f64(kui.get(row, "f"))
		check(f_ok && f == 2.5, "as_f64 off a struct")
		_, b_ok = kui.as_bool(kui.get(row, "f"))
		check(!b_ok, "the readers do not coerce")
		keyed := make(map[string]int, context.temp_allocator)
		keyed["k"] = 1
		m := keep(&vals, keyed)
		key, v := kui.entry(m, 0)
		k1, k1_ok := kui.as_i64(v)
		check(key == "k" && k1_ok && k1 == 1, "entry walks a map whose keys you do not know")
		_, past := kui.entry(m, 1)
		null := keep(&vals, (^int)(nil))
		check(past == nil && kui.length(null) == 0, "past the end is nil, and a scalar has no entries")
	}
	for v in vals do kui.value_free(v)

	// The theme's two setters and the metrics: pin, read, restore.
	{
		theme, theme_ok := kui.theme(ui)
		check(theme_ok, "theme")
		kui.theme_set_accent(ui, 0xff8800ff)
		accented, accented_ok := kui.theme(ui)
		check(accented_ok && accented.accent == 0xff8800ff, "theme_set_accent")
		theme.bg = 0x102030ff
		kui.theme_set(ui, theme)
		pinned, pinned_ok := kui.theme(ui)
		check(pinned_ok && pinned.bg == 0x102030ff, "theme_set pins the palette")
		// No theme (C's NULL): back to deriving it from the OS.
		kui.theme_set(ui)
		derived, _ := kui.theme(ui)
		check(derived.bg != 0x102030ff, "theme_set with none derives again")
		metrics, metrics_ok := kui.metrics(ui)
		check(metrics_ok && metrics.control_text > 0, "metrics")
		stock := metrics.control_text
		metrics.control_text = 11
		kui.metrics_set(ui, metrics)
		compact, compact_ok := kui.metrics(ui)
		check(compact_ok && compact.control_text == 11, "metrics_set")
		kui.metrics_set(ui)
		restored, _ := kui.metrics(ui)
		check(restored.control_text == stock, "metrics_set with none restores the stock set")
	}

	// The text cache's budget and its reading; the font families.
	kui.set_text_cache_budget(ui, 4 << 20)
	check(kui.text_cache_bytes(ui) > 0, "text_cache_bytes: the frames above shaped text")
	{
		families := kui.font_families(ui)
		check(len(families) > 0 && len(families[0]) > 0, "font_families lists the stock set")
		fonts := kui.system_fonts(ui)
		check(
			len(fonts) == len(families) && fonts[0].family == families[0] && len(fonts[0].weights) > 0 && fonts[0].weights[0] > 0,
			"system_fonts: the same families, each with its weights",
		)
	}

	// The image's pixels read back, and a fragment's whole module.
	{
		w, h, px, ok := kui.image_pixels(ui, k.image)
		check(ok && w == 2 && h == 2 && len(px) == 16 && px[0] == 255 && px[4 + 1] == 255, "image_pixels reads the RGBA back")
		frag := kui.fragment_add(ui, "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n    return params[0];\n}\n")
		check(frag != 0, "fragment_add compiles a one-line fragment")
		wgsl, wgsl_ok := kui.fragment_source(ui, frag)
		check(wgsl_ok && strings.contains(wgsl, "params[0]"), "fragment_source is the module around it")
		params := []f32{1, 0, 0, 1}
		kui.frame_begin(ui, 320, 240, 1)
		kui.fragment_open_with(ui, "painted", frag, 0, params, {width = kui.px(20), height = kui.px(20)})
		kui.text(ui, "over it")
		kui.close(ui)
		kui.frame_finish(ui)
		fdd, fdd_ok := kui.draw_data(ui)
		check(fdd_ok && len(fdd.fragments) == 1, "fragment_open_with drew one")
		kui.fragment_remove(ui, frag)
		_, wgsl_ok = kui.fragment_source(ui, frag)
		check(!wgsl_ok, "fragment_remove: the handle is dead")
	}

	// The assistive fact, and the cursor the last hover derived.
	kui.env_set_assistive(ui, .Listening)
	kui.set_time(ui, 0.3)
	kui.frame(ui, &k, surface_view, 800, 600, 2)
	check(kui.cursor_shape(ui) != .Unset, "cursor_shape names a cursor once a frame has hovered")

	// The caret clock a host runs itself.
	kui.focus(ui, k.editor)
	check(kui.has_caret(ui) && kui.caret_visible(ui), "a focused editor has a caret, shown")
	stamp := kui.caret_stamp(ui)
	kui.set_caret_visible(ui, false)
	check(!kui.caret_visible(ui), "set_caret_visible parks it")
	kui.set_caret_visible(ui, true)
	kui.input_key(ui, .Left, {})
	check(kui.caret_stamp(ui) != stamp, "caret_stamp moves with the caret")
	kui.edit_set_text_label(ui, "notes", "by label")
	text, text_ok = kui.edit_text(ui, k.editor)
	check(text_ok && strings.contains(text, "by label"), "edit_set_text_label")

	// Layout and scrolling read back by key; a reveal resolves at the frame.
	{
		rect, rect_ok := kui.layout_of(ui, k.card)
		check(rect_ok && rect.w > 0 && rect.h > 0, "layout_of answers for the on_layout node")
		_, rect_ok = kui.layout_of(ui, k.sink)
		check(!rect_ok, "and for no other")
		geo, geo_ok := kui.scroll_geometry(ui, k.card)
		check(geo_ok && geo.h > 0, "scroll_geometry on the card")
		kui.shift_scroll(ui, k.card, 0, 0) // a zero shift moves nothing
		kui.set_scroll(ui, k.card, 0, 9999)
		kui.reveal(ui, k.slider)
		kui.frame(ui, &k, surface_view, 800, 600, 2)
		sx, sy := kui.scroll_offset(ui, k.card)
		check(sx == 0 && sy >= 0, "scroll_offset reads the clamped offset back")
		_, sy = kui.scroll_offset(ui, 12345)
		check(sy == 0, "a node that never scrolled is 0")

		// A scroll gesture (backlog F107): its first event picks the card
		// under the pointer, toward whichever end it has room for, and the
		// next goes on to it.
		at, at_ok := kui.layout_of(ui, k.card)
		check(at_ok, "the card has a rect")
		geo, geo_ok = kui.scroll_geometry(ui, k.card)
		check(geo_ok, "and a geometry")
		before := geo.offset_y
		way: f32 = 1 if before >= 5 else -1 // +y is toward the start
		kui.input_cursor(ui, at.x + 4, at.y + 4)
		kui.input_scroll_gesture(ui, 0, 3 * way, true)
		kui.input_scroll_gesture(ui, 0, 2 * way, false)
		kui.frame(ui, &k, surface_view, 800, 600, 2)
		_, sy = kui.scroll_offset(ui, k.card)
		check(geo.max_offset_y < 5 || sy == before - 5 * way, "input_scroll_gesture moves the card it began over")
	}

	// File dialogs (backlog C51): ask, drain as the host, answer, hear it.
	{
		dialog := kui.File_Dialog {
			mode     = .Open,
			multiple = true,
			title    = "Pick images",
			filters  = {{name = "Images", extensions = {"png", ".jpg"}}},
		}
		check(kui.request_files(ui, dialog, "pics"), "request_files asks")
		check(kui.awaiting_files(ui), "awaiting_files while it is out")
		check(!kui.request_files(ui, {}), "and a second ask is dropped")
		mode, multiple, dtitle, dir, _, filters, ok := kui.take_file_request(ui)
		check(ok && mode == .Open && multiple && filters == 1 && len(dtitle) == 11 && len(dir) == 0, "take_file_request hands the host the dialog")
		fname, fexts, f_ok := kui.file_request_filter(ui, 0)
		check(f_ok && len(fname) == 6 && fexts == "png;jpg", "file_request_filter reads a filter, dots dropped")
		_, _, f_ok = kui.file_request_filter(ui, 1)
		check(!f_ok, "and no filter past the last")
		kui.input_files(ui, {"/tmp/a.png"})
		check(!kui.awaiting_files(ui), "input_files spends the ask")
		files := 0
		for fev in kui.poll_event(ui) {
			got, got_ok := kui.message(fev, kui.Files_Event)
			tag, tag_ok := kui.as_string(got.tag)
			if got_ok && len(got.paths) == 1 && tag_ok && len(tag) == 4 do files += 1
		}
		check(files == 1, "the answer is one files event with the path and the tag")
	}

	// Documents the OS asked the app to open (backlog F124): nobody asked,
	// and the host hears them on the root; none is nothing.
	{
		kui.input_open(ui, {"/tmp/a.txt", "/tmp/b.md"})
		opened := 0
		for oev in kui.poll_event(ui) {
			got, ok := kui.message(oev, kui.Open_Event)
			if ok && len(got.paths) == 2 && got.paths[1] == "/tmp/b.md" do opened += 1
		}
		check(opened == 1, "input_open is one open event with both paths")
		kui.input_open(ui, nil)
		_, ok := kui.poll_event(ui)
		check(!ok, "and no documents is no event")
	}

	// Focus regions: entered by name, read back as the ring in effect.
	check(kui.region(ui) == 0, "the main ring to begin with")
	kui.focus_region(ui, k.region)
	kui.frame(ui, &k, surface_view, 800, 600, 2)
	check(kui.region(ui) == k.region, "focus_region entered the dock's ring")
	check(kui.focused(ui) == kui.key_of(ui, "dock-button"), "and landed on its stop")
	kui.focus_region(ui, 0)
	kui.frame(ui, &k, surface_view, 800, 600, 2)
	check(kui.region(ui) == 0, "and 0 is the main ring again")

	// Selection (ADR 0017): a scope selected whole, read three ways, copied,
	// cleared; a grid's the same by lines and columns.
	{
		check(kui.select_all_in(ui, k.hitline), "select_all_in on the selectable row")
		sel, sel_ok := kui.selection_text(ui)
		check(sel_ok && strings.contains(sel, "let value = 1;"), "selection_text")
		html, html_ok := kui.selection_html(ui)
		check(html_ok && len(html) > 0, "selection_html")
		ai, ab, fi, fb, ends_ok := kui.selection_ends(ui)
		check(ends_ok && ai == -1 && fi == -1 && ab == 0 && fb > 0, "selection_ends: outside every virtual row, from the start to the end")
		answer, copied := kui.request_copy(ui)
		check(answer == .Ready && strings.contains(copied, "value"), "request_copy is ready with the text")
		check(!kui.answer_selection_range(ui, "late"), "nothing asked, so no answer taken")
		check(kui.clear_selection(ui), "clear_selection")
		_, sel_ok = kui.selection_text(ui)
		check(!sel_ok, "and there is none")
		term := kui.key_of(ui, "term")
		check(kui.select_all_in(ui, term), "select_all_in on the grid")
		node, al, ac, fl, fc, block, cs_ok := kui.cell_selection(ui)
		check(cs_ok && node == term && al == 0 && ac == 0 && fl == 1 && fc == 6 && !block, "cell_selection: the whole screen, linewise")
		cleared := kui.clear_selection(ui)
		_, _, _, _, _, _, cs_ok = kui.cell_selection(ui)
		check(cleared && !cs_ok, "cleared, a grid's selection reads false")
	}

	// The clipboard the app owns, drained as menu actions.
	{
		kui.set_lookup_available(ui, true)
		kui.set_clipboard(ui, "plain", "<b>plain</b>")
		check(!kui.awaiting_paste(ui), "no paste asked yet")
		kui.request_paste(ui)
		kui.request_paste(ui) // one ask at a time: dropped (AR34)
		check(kui.awaiting_paste(ui), "awaiting_paste while one is out")
		act, ok := kui.take_menu_action(ui)
		check(
			ok && act.kind == .Set_Clipboard && strings.contains(act.text, "plain") && strings.contains(act.html, "<b>"),
			"set_clipboard queues both flavours",
		)
		act, ok = kui.take_menu_action(ui)
		check(ok && act.kind == .Paste, "request_paste queues the ask")
		_, ok = kui.take_menu_action(ui)
		check(!ok, "drained, and the second ask was dropped")
		check(kui.awaiting_paste(ui), "still out until answered")
		kui.input_commit(ui, "") // the clipboard held nothing
		check(!kui.awaiting_paste(ui), "an empty commit is an answer")
		// A secret goes out as its own kind; a marked paste is an answer too
		// (backlog F84).
		kui.set_clipboard_secret(ui, "hunter2")
		act, ok = kui.take_menu_action(ui)
		check(ok && act.kind == .Set_Clipboard_Secret && strings.contains(act.text, "hunter2") && len(act.html) == 0, "set_clipboard_secret queues the secret alone")
		kui.request_paste(ui)
		act, ok = kui.take_menu_action(ui)
		check(ok && act.kind == .Paste, "asked again")
		kui.input_paste(ui, "s3cret", {.Concealed, .Transient})
		check(!kui.awaiting_paste(ui), "a marked paste is an answer")
	}

	// A context menu the host shows itself: opened over a node, read row by
	// row, chosen, and one closed unchosen.
	{
		kui.set_native_menus(ui, true)
		kui.set_native_menu_bar(ui, true)
		rows := []kui.Menu_Item{{label = "Rename", role = .Custom}, {role = .Separator}, {role = .Copy}}
		check(kui.open_menu(ui, k.card, 10, 20, rows), "open_menu")
		count, target, mx, my := kui.menu_item_count(ui)
		check(count == 3 && target == k.card && mx == 10 && my == 20, "menu_item_count: the rows, the node and the point")
		label, _, role, flags, ok := kui.menu_item(ui, 0)
		check(ok && strings.contains(label, "Rename") && role == .Custom && .Enabled in flags, "menu_item reads a row back")
		label, _, role, _, ok = kui.menu_item(ui, 2)
		check(ok && role == .Copy && len(label) > 0, "a standard role carries its own label")
		_, _, _, _, ok = kui.menu_item(ui, 3)
		check(!ok, "past the end is false")
		check(kui.activate_menu_item(ui, 0), "activate_menu_item chooses the row")
		count, _, _, _ = kui.menu_item_count(ui)
		check(count == 0, "which closed the menu")
		chosen := 0
		for mev in kui.poll_event(ui) do if strings.contains(kui.kind(mev.payload), "menu") && mev.key == k.card do chosen += 1
		check(chosen == 1, "and the app heard it on the node")
		check(kui.open_menu(ui, k.card, 0, 0, rows) && kui.close_menu(ui), "close_menu")
		check(!kui.close_menu(ui), "false when nothing was open")
		check(!kui.open_menu(ui, 0, 0, 0, rows), "a key of 0 opens nothing")
		// A submenu, by its path (backlog F128).
		nested := []kui.Menu_Item {
			{label = "Open", role = .Custom},
			{label = "Sort by", role = .Custom, submenu = {{label = "Name", role = .Custom}, {label = "Date", role = .Custom}}},
		}
		check(kui.open_menu(ui, k.card, 0, 0, nested), "a menu with a submenu opens")
		check(kui.menu_submenu_count(ui, {1}) == 2 && kui.menu_submenu_count(ui, {}) == 2, "menu_submenu_count")
		name, _, _, _, name_ok := kui.menu_item_path(ui, {1, 0})
		check(name_ok && strings.contains(name, "Name"), "menu_item_path reads a row inside")
		check(!kui.activate_menu_path(ui, {1}) && kui.activate_menu_path(ui, {1, 0}), "activate_menu_path chooses the row inside")
		for _ in kui.poll_event(ui) {}
		kui.set_native_menus(ui, false)
		kui.set_native_menu_bar(ui, false)
	}

	// The devtools doors (ADR 0024): the panel, its dock, its theme, legend
	// and inspect chord, and the node snapshot a tree view reads.
	{
		check(!kui.devtools(ui), "the panel is off until asked")
		kui.set_devtools(ui, true)
		check(kui.devtools(ui), "set_devtools")
		check(kui.set_devtools_dock(ui, "left"), "set_devtools_dock")
		check(!kui.set_devtools_dock(ui, "sideways"), "a dock word this build lacks is false")
		dock, dock_ok := kui.devtools_dock(ui)
		check(dock_ok && strings.contains(dock, "left"), "devtools_dock reads it back")
		check(kui.set_devtools_theme(ui, "dark", 0x3b5bd4ff), "set_devtools_theme")
		check(!kui.set_devtools_theme(ui, "blue", 0), "a base that is not one is false")
		kui.set_devtools_legend(ui, {"Space"}, {"play"})
		chord, chord_ok := kui.devtools_key(ui)
		check(chord_ok && strings.contains(chord, "ctrl+shift+i"), "devtools_key: the default")
		check(kui.set_devtools_key(ui, "f12"), "set_devtools_key")
		check(!kui.set_devtools_key(ui, "f99"), "a chord kui cannot name is false")
		chord, chord_ok = kui.devtools_key(ui)
		check(chord_ok && strings.contains(chord, "f12"), "the respelled chord reads back")
		check(kui.devtools_selected(ui) == 0 && kui.devtools_hovered(ui) == 0 && kui.devtools_picked(ui) == 0, "nothing selected, hovered or picked yet")
		kui.set_devtools_selected(ui, 0)
		kui.set_devtools_pick(ui, true)
		check(kui.devtools_picking(ui), "set_devtools_pick raises the picker")
		kui.set_devtools_pick(ui, false)
		check(!kui.devtools_picking(ui), "and puts it away")
		tab, tab_ok := kui.devtools_current_tab(ui)
		check(tab_ok && strings.contains(tab, "tree"), "devtools_current_tab: the pick showed the tree")
		check(kui.set_devtools_tab(ui, "events"), "set_devtools_tab: one of the panel's own")
		tab, tab_ok = kui.devtools_current_tab(ui)
		check(tab_ok && strings.contains(tab, "events"), "and it reads back")
		check(!kui.set_devtools_tab(ui, "mine"), "a declared name is listed from the panel's first frame on, not before")
		kui.set_inspect(ui, true)
		kui.frame_begin(ui, 800, 600, 2)
		surface_view(&k, ui)
		// A declared tab of each form (ADR 0032): the extension form names a
		// slot nobody loaded (harmless); the host form's open answers false,
		// since "mine" is listed only from the panel's next frame. A second
		// declaration of a name is refused.
		check(kui.devtools_tab(ui, "plug", "Plugin", "ts/panel"), "devtools_tab")
		check(!kui.devtools_tab(ui, "plug", "Again", "ts/again"), "a name twice is refused")
		if kui.devtools_tab_open(ui, "mine", "Mine") {
			check(false, "devtools_tab_open: not on show, so nothing opens")
			kui.close(ui)
		} else {
			check(true, "devtools_tab_open answers false off show")
		}
		kui.frame_finish(ui)
		// Where the frame put the host: right of the left dock, the rest of
		// the 800x600 window (backlog F92). (surface.c's NULL out has no
		// spelling here.)
		host, host_ok := kui.host_rect(ui)
		check(host_ok && host.x > 0 && host.x + host.w == 800 && host.y == 0 && host.h == 600, "host_rect: the host area beside the dock")
		{
			dup := kui.take_warnings(ui)
			seen := 0
			for w in dup do if w.code == "duplicate-tab" do seen += 1
			check(seen == 1 && len(dup) == 1, "the second declaration is the duplicate-tab diagnostic, and nothing else")
		}
		nodes_list := kui.nodes(ui)
		check(nodes_list != nil && kui.length(nodes_list) > 1, "nodes: the frame's nodes, as data")
		first_node := kui.at(nodes_list, 0)
		check(first_node != nil && kui.get(first_node, "key") != nil && kui.get(first_node, "kind") != nil, "each with a key and a kind")
		kui.set_inspect(ui, false)
		kui.set_devtools(ui, false)
		check(!kui.devtools(ui), "and off again")
		for _ in kui.take_window_command(ui) {}
		for _ in kui.poll_event(ui) {}
	}

	// Why a frame runs (backlog F111), on a context of its own: the input
	// handed in and the host's note are the next frame's reasons, and, traced,
	// whether a frame drew what the one before drew.
	{
		why := kui.new_ui()
		kui.set_frame_trace(why, true)
		kui.frame_begin(why, 320, 240, 1)
		kui.frame_finish(why)
		check(kui.frame_unchanged(why) == -1, "frame_unchanged: nothing to compare the first frame with")
		kui.input_cursor(why, 5, 5)
		kui.note_frame_cause(why, {.Wake})
		kui.frame_begin(why, 320, 240, 1)
		check(kui.frame_cause(why) == {.Pointer_Move, .Wake}, "frame_cause: the pointer's move and the host's wake")
		kui.frame_finish(why)
		check(kui.frame_unchanged(why) == 1, "and it drew what the frame before drew")
		kui.free_ui(why)
	}

	// A standalone context is nobody's slot.
	{
		_, name_ok := kui.slot_name(ui)
		_, ns_ok := kui.slot_namespace(ui)
		check(!name_ok && !ns_ok, "slot_name / slot_namespace are false outside an extension")
	}

	// And none of that raised a diagnostic either.
	{
		late := kui.take_warnings(ui)
		for w in late do fmt.eprintfln("  warning: %s: %s", w.code, w.message)
		check(len(late) == 0, "the rest of the walk raises no diagnostics")
	}

	kui.image_remove(ui, k.image)
	kui.free_ui(ui)

	rest()

	fmt.printfln("surface: %d access nodes, %d editor runs, %d quads, %d events", total, run_count, quad_count, events)
	if fails > 0 {
		fmt.eprintfln("surface walk: %d check(s) failed", fails)
		return false
	}
	fmt.println("surface walk OK")
	return true
}

// -- What the C binding walks in its other programs ---------------------------
//
// surface.c leaves these to the programs beside it: the drawing elements,
// the tokens, announcements and the OS moving under the app to
// conformance.c, the extension contract to host.c and panel.c, the icon to
// counter.c (and so does counter.odin). Here each gets a check, in a
// context of its own so the diagnostics it means to raise stay its own.

FRAGMENT_WGSL :: `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);
    return mix(params[0], params[1], t);
}`

FRAGMENT_IMAGE_WGSL :: `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    return kui_sample(uv) * params[0];
}`

Rest :: struct {
	image:    kui.Image,
	fragment: kui.Fragment,
	sampler:  kui.Fragment,
	ring:     []f32,
	slot:     [2]bool,
}

rest_view :: proc(r: ^Rest, ui: ^kui.Ui) {
	kui.root(ui, {width = kui.GROW, height = kui.GROW, dir = .Row, gap = 4, wrap_children = true})
	kui.line(ui, 10, 10, 90, 70, 2, 0x7f9cf5ff)
	kui.polyline(ui, "elbow", {{0, 0}, {40, 20}, {80, 0}}, 3, 0xd8863bff, false, dash = [5]f32{6, 3, 6, 3, 0})
	kui.polygon(ui, "tri", {{0, 30}, {15, 0}, {30, 30}}, {width = kui.px(30), height = kui.px(30), bg = 0x3b82f6ff})
	kui.path(ui, "ring", r.ring, .Evenodd, 0, 0, 0, spec = {width = kui.px(80), height = kui.px(80), bg = 0xffd000ff})
	kui.path_d(ui, "wedge", "M60 60 L100 60 A40 40 0 0 1 60 100 Z", .Nonzero, 0, 0, 0, spec = {width = kui.px(100), height = kui.px(100), bg = 0x73d98cff})
	kui.image_with(ui, r.image, .Nearest, .Contain, {width = kui.px(16), height = kui.px(16)})
	params := [16]f32{1, 0, 0, 1, 0, 0, 1, 1, 4, 1, 0, 0, 1, 1, 1, 1}
	kui.fragment(ui, r.fragment, params[:], {width = kui.px(40), height = kui.px(40)})
	// An opener: the block's end closes it, as box's does.
	if kui.fragment_open(ui, "card", r.fragment, params[:], {width = kui.px(80), height = kui.px(30), pad = kui.pad(4)}) {
		kui.text(ui, "in a fragment", {size = 10})
	}
	kui.fragment_with(ui, "sampled", r.sampler, r.image, params[:4], {width = kui.px(16), height = kui.px(16)})
	// A slot with nothing loaded still takes its place in the tree; the same
	// name twice in a frame is refused, and the core says so.
	r.slot = {kui.slot(ui, "ns/panel", "params"), kui.slot(ui, "ns/panel")}
}

rest :: proc() {
	ui := kui.new_ui()
	defer kui.free_ui(ui)
	kui.set_diagnostics(ui, true)

	// Tokens (ADR 0027, 0028): declared, derived, read back.
	kui.tokens_set(ui, {{"peach", 0xffcc99ff, 0xffcc99ff}, {"ink", 0x202020ff, 0xe0e0e0ff}}, {{"gap", 8}})
	check(kui.tokens_derive(ui, {{name = "lit", from = "peach", ops = {{op = .Lift, t = 0.3}}}}), "tokens_derive")
	peach, peach_ok := kui.token_color(ui, "peach")
	check(peach_ok && peach == 0xffcc99ff, "token_color")
	lit, lit_ok := kui.token_color(ui, "lit")
	check(lit_ok && lit != peach, "a derived token is a colour of its own")
	gap, gap_ok := kui.token_length(ui, "gap")
	check(gap_ok && gap == 8, "token_length")
	_, nothing := kui.token_color(ui, "nothing")
	check(!nothing, "an undeclared token is false")

	// An announcement: said once, drained once.
	kui.announce(ui, "Saved", .Assertive)
	said := kui.take_announcements(ui)
	check(len(said) == 1 && said[0].text == "Saved" && said[0].live == .Assertive, "announce, and take_announcements drains it")
	check(len(kui.take_announcements(ui)) == 0, "once")

	// A drain past 64: the strings handed out are the door's until its
	// next call, and a drain that asked in chunks of 64 called it again
	// and freed the first chunk's; the announcements past 64 were dropped.
	// 300 duplicate keys fill the core's pending warnings (256), every
	// message whole; 70 announcements are all kept.
	{
		kui.frame_begin(ui, 320, 240, 1)
		kui.root(ui, {})
		for i in 0 ..< 300 {
			key := fmt.tprintf("dup%d", i)
			for _ in 0 ..< 2 do if kui.box(ui, {key = key}) {}
		}
		kui.frame_finish(ui)
		dups, whole := 0, 0
		for w in kui.take_warnings(ui) do if w.code == "duplicate-key" {
			dups += 1
			if strings.has_prefix(w.message, "two nodes share this key") do whole += 1
		}
		check(dups > 64 && whole == dups, "take_warnings past a chunk keeps every message")
		for i in 0 ..< 70 do kui.announce(ui, fmt.tprintf("said %d", i), .Polite)
		many := kui.take_announcements(ui)
		check(len(many) == 70 && many[0].text == "said 0" && many[69].text == "said 69", "take_announcements keeps all 70")
	}

	// Resources the drawing elements take: an image whose pixels change
	// under its id, two fragments, a path parsed once.
	r := Rest {
		image    = kui.image_add(ui, 2, 1, {255, 0, 0, 255, 0, 0, 255, 255}),
		fragment = kui.fragment_add(ui, FRAGMENT_WGSL),
		sampler  = kui.fragment_add(ui, FRAGMENT_IMAGE_WGSL),
		ring     = kui.path_parse("M120 10 H190 V80 H120 Z M140 30 H170 V60 H140 Z"),
	}
	check(r.fragment != 0 && r.sampler != 0, "fragment_add")
	kui.image_update(ui, r.image, 1, 1, {0, 255, 0, 255})
	w, h, px, px_ok := kui.image_pixels(ui, r.image)
	check(px_ok && w == 1 && h == 1 && px[1] == 255, "image_update replaced the pixels under the same id")
	check(len(r.ring) > 0, "path_parse")
	check(len(kui.path_parse("M10 10 L20")) == 0, "data that does not parse is no ops")

	kui.frame(ui, &r, rest_view, 600, 400, 1)
	check(r.slot[0] && !r.slot[1], "slot: declared once, refused the second time")
	check(kui.key_of(ui, "ns/panel") != 0, "and keyed though nothing fills it")
	dd, dd_ok := kui.draw_data(ui)
	segments, fragments, images := 0, 0, 0
	for q in dd.quads {
		#partial switch q.kind {
		case .Segment: segments += 1
		case .Fragment: fragments += 1
		case .Image, .Texture: images += 1
		}
	}
	check(dd_ok && segments > 0, "line and polyline drew segments")
	check(fragments >= 3 && len(dd.fragments) >= 3, "the fragments drew, each with its draw")
	check(images > 0, "image_with drew the image")
	for label in ([]string{"elbow", "tri", "ring", "wedge", "card", "sampled"}) {
		check(kui.key_of(ui, label) != 0, fmt.tprintf("the %s element is keyed by its label", label))
	}
	duplicate := false
	for wr in kui.take_warnings(ui) do if wr.code == "duplicate-slot" do duplicate = true
	check(duplicate, "the second is a duplicate-slot diagnostic")

	// A popup the host dismisses: the app hears it on the root, by name.
	{
		popup := kui.Window_Config {
			kind     = .Popup,
			width    = 120,
			height   = 80,
			anchor_x = 10,
			anchor_y = 10,
		}
		kui.frame_begin(ui, 320, 240, 1)
		kui.window_declare(ui, "menu", popup)
		kui.frame_finish(ui)
		cmd, ok := kui.take_window_command(ui)
		check(ok && cmd.kind == .Open && cmd.config.kind == .Popup, "the popup opens")
		// activates unsaid is KUI_WINDOW_POPUP_INIT's no, where it was a yes.
		check(!(cmd.config.activates.? or_else true), "a popup does not activate unless it says so")
		for _ in kui.poll_event(ui) {}
		kui.window_dismissed(ui, cmd.window, .Escape)
		heard := false
		for ev in kui.poll_event(ui) {
			if d, is := kui.message(ev, kui.Dismiss_Event); is {
				heard = d.reason == "escape" && d.name == "menu" && d.id == cmd.window
			}
		}
		check(heard, "window_dismissed: the app hears whose and why")
	}

	// The OS's appearance moving under the app.
	{
		kui.env_set_system(ui, .Dark, 0, .Reduced, "de-DE")
		kui.frame(ui, &r, rest_view, 600, 400, 1)
		dark := kui.theme(ui)
		kui.env_set_system(ui, .Light, 0, .Full, "en-US")
		kui.frame(ui, &r, rest_view, 600, 400, 1)
		light := kui.theme(ui)
		check(dark.appearance == .Dark && light.appearance == .Light && dark.bg != light.bg, "env_set_system: the palette follows the appearance")
		systems := 0
		for ev in kui.poll_event(ui) do if _, is := kui.message(ev, kui.System_Event); is do systems += 1
		check(systems > 0, "and the app hears the system change")
	}

	// Extensions, from the refusing side: there is no plugin at this path.
	{
		check(!kui.ctx_add_extension(ui, "ns", "target/no-such-plugin.so"), "ctx_add_extension refuses a missing library")
		why, why_ok := kui.ctx_extension_error(ui)
		check(why_ok && len(why) > 0, "ctx_extension_error says why")
		check(kui.ctx_extension_count(ui) == 0, "ctx_extension_count")
		_, ns_ok := kui.ctx_extension_namespace(ui, 1)
		check(!ns_ok, "ctx_extension_namespace: nothing at origin 1")
	}

	// A float preset by name, the same four JSX and Lua reach; a typo places
	// nothing rather than somewhere arbitrary.
	below, below_ok := kui.float_preset("below")
	check(below_ok && below.mode == .Parent && below.anchor_y == .End && below.self_y == .Start, "float_preset fills a placement")
	_, beneath := kui.float_preset("beneath")
	check(!beneath, "an unknown preset is false")

	// A plugin's own readers, outside a plugin: nothing to read, nobody to
	// reply to.
	check(kui.slot_params(ui) == nil, "slot_params outside an extension")
	check(!kui.reply(kui.Event{}, "pong"), "reply: an event with no sink takes none")
}

main :: proc() {
	// --headless is what the test runner passes; the walk has no window
	// either way.
	os.exit(0 if surface() else 1)
}
