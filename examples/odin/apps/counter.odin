// The counter, from Odin: the one example every binding has in the same
// shape (docs/adr/0021, decision 3). `view` rebuilds the tree from the
// state; a click arrives in `on_event` as a message and moves the state; a
// right-click asks for a menu the next frame declares as a modal float.
//
//   nu scripts/odin.nu run counter              opens a window
//   nu scripts/odin.nu run counter --headless   no window: clicks +1 by its
//       name, right-clicks the card for the menu, dismisses it with Escape,
//       and exits non-zero on a wrong answer - the Rosetta drive every
//       counter runs.
package counter

import "core:fmt"
import "core:os"
import kui "../../../packages/odin/kui"

State :: struct {
	count: int,
	// Where the context menu is open; nil while it is not. The core opens
	// nothing: the view declares the menu while this is set.
	menu:  Maybe([2]f32),
}

// The messages, as types: each is {kind = "<snake_case name>"} on the wire,
// which is what the Rust counter's #[derive(Message)] enum sends too. An
// event arrives as the message its handler declared, its data in a field
// named for it (ADR 0047): the card's right-click as {kind = "card",
// contextmenu = {x, y}}, which the embedded With_Context_Menu reads.
Inc :: struct {}
Dec :: struct {}
Add10 :: struct {}
Reset :: struct {}
Card :: struct {
	using _: kui.With_Context_Menu, // the card's context menu: where it was asked for
}
Menu :: struct {
	using _: kui.With_Dismiss, // the menu's dismiss: why
}
// What the headless drive asked the driver to keep, answered as these with
// capture = {phase, path, width, height, frames, seconds, error} in them.
Shot :: struct {
	using _: kui.With_Capture,
}
Rec :: struct {
	using _: kui.With_Capture,
}
Copy :: struct {
	using _: kui.With_Capture,
}

Msg :: union {
	Inc,
	Dec,
	Add10,
	Reset,
	Card,
	Menu,
}

view :: proc(s: ^State, ui: ^kui.Ui) {
	// Every colour is a role of the palette the core derived from the OS, so
	// the window follows light and dark with no branch of its own.
	t := kui.theme(ui)
	kui.root(ui, {width = kui.GROW, height = kui.GROW, main_align = .Center, cross_align = .Center, bg = t.bg})

	card := kui.Spec {
		gap             = 20,
		cross_align     = .Center,
		pad             = kui.pad(32),
		bg              = t.surface,
		radius          = 12,
		border_w        = 1,
		border_color    = t.border,
		on_context_menu = Card{}, // a right-click asks for the menu
	}
	if kui.column(ui, card) {
		kui.text(ui, "kui from Odin", {size = 14, color = t.muted})
		kui.text(ui, fmt.tprint(s.count), {size = 56})
		if kui.row(ui, {gap = 12}) {
			kui.button(ui, "-1", Dec{})
			kui.button(ui, "+1", Inc{})
		}
		kui.rich_text(
			ui,
			{
				{text = "same IR as Rust and "},
				{text = "Odin", color = t.success, flags = {.Bold}},
				{text = " — just typed", flags = {.Italic}},
			},
			{size = 13, color = t.faint},
		)
	}

	if at, open := s.menu.?; open do context_menu(ui, t, at)
}

// The menu the right-click asks for: a float at the press, modal so the
// Tab ring is its own and Escape or a press outside sends Dismiss.
context_menu :: proc(ui: ^kui.Ui, t: kui.Theme, at: [2]f32) {
	menu := kui.Spec {
		gap          = 4,
		width        = kui.px(120),
		pad          = kui.pad(4),
		bg           = t.raised,
		radius       = 6,
		border_w     = 1,
		border_color = t.border_strong,
		float        = {mode = .Viewport, dx = at.x, dy = at.y, fit = true},
		modal        = Menu{},
		label        = "Actions", // a modal is a dialog, named by its label
	}
	if kui.column(ui, menu) {
		kui.button(ui, "+10", Add10{})
		kui.button(ui, "Reset", Reset{}, {description = "Back to zero"})
	}
}

on_event :: proc(s: ^State, ev: kui.Event) {
	switch m in kui.message(ev, Msg) {
	case Inc:
		s.count += 1
	case Dec:
		s.count -= 1
	case Add10:
		s.count += 10
		s.menu = nil
	case Reset:
		s.count = 0
		s.menu = nil
	case Card:
		s.menu = [2]f32{m.contextmenu.x, m.contextmenu.y}
	case Menu:
		s.menu = nil
	}
}

teardown :: proc(s: ^State) {
	fmt.println("teardown at count", s.count)
}

// -- headless self-test: the whole loop without a window ---------------------

headless :: proc() -> bool {
	s: State
	ui := kui.new_ui()
	if ui == nil do return fail("libkui_ffi's ABI is not the one this binding was generated against")
	defer kui.free_ui(ui)

	// Frame 1 lays out, so the buttons have places to be clicked at.
	kui.frame(ui, &s, view)
	_, plus, found := kui.find(ui, "+1")
	if !found do return fail("no node named +1 in the access tree")
	// A primary press and release where it is, as a pointer sends them.
	kui.input_cursor(ui, kui.center(plus))
	kui.input_mouse(ui, true, 1)
	kui.input_mouse(ui, false, 1)
	got := drain(ui, &s)
	fmt.printfln("clicked +1: %d event(s), count = %d", got, s.count)
	if got != 1 || s.count != 1 do return fail("expected one click and a count of 1")

	// The secondary button over the card asks for the menu at the press, and
	// moves nothing else.
	x, y := kui.center(plus)
	kui.click(ui, x, y - 60, .Secondary)
	got = drain(ui, &s)
	at, open := s.menu.?
	fmt.printfln("right-clicked: %d event(s), menu at (%.0f, %.0f)", got, at.x, at.y)
	if got != 1 || !open || s.count != 1 do return fail("expected one contextmenu event and no click")

	// Declare it, then let Escape ask for it back.
	kui.frame(ui, &s, view)
	if _, _, ok := kui.find(ui, "Actions"); !ok do return fail("the menu was not declared")
	kui.press(ui, "escape")
	drain(ui, &s)
	if _, still := s.menu.?; still do return fail("escape did not dismiss the menu")
	fmt.println("escape dismissed the menu")

	// The last frame as pixels (backlog C55), drawn again on a GPU device of
	// the process's own; a machine with no GPU adapter has none, and says so
	// on stderr - a failure where KUI_REQUIRE_GPU is set, as CI sets it over
	// Mesa's software Vulkan.
	if cw, ch, shot, shot_ok := kui.capture(ui); shot_ok {
		defer delete(shot)
		if cw != 800 || ch != 600 || shot[3] != 255 do return fail("capture")
		fmt.printfln("captured the last frame: %dx%d", cw, ch)
	} else {
		if need, asked := os.lookup_env("KUI_REQUIRE_GPU", context.temp_allocator); asked && need != "" {
			return fail("KUI_REQUIRE_GPU is set, but capture drew nothing")
		}
		fmt.println("no GPU adapter: capture had nothing to draw on")
	}

	// What an app asks the driver to keep of its window (ADR 0055): a still,
	// a copy on the clipboard, and a recording that runs for half a second of
	// the frame clock, served after each frame the way run serves a window's
	// and answered as a capture event in the message each named. With no GPU
	// adapter all three answer "failed", the reason in error.
	{
		tmp, _ := os.temp_directory(context.temp_allocator)
		still := fmt.tprintf("%s/kui-odin-counter-shot.png", tmp)
		movie := fmt.tprintf("%s/kui-odin-counter-rec.apng", tmp)
		defer os.remove(still)
		defer os.remove(movie)
		if !kui.capture_to(ui, still, Shot{}) || !kui.capture_to_clipboard(ui, Copy{}) || !kui.record_to(ui, movie, Rec{}) || kui.capture_to(ui, "") {
			return fail("capture_to / capture_to_clipboard / record_to refused")
		}
		kui.set_time(ui, 1.0)
		kui.frame(ui, &s, view)
		kui.serve_captures(ui)
		kui.stop_recording(ui)
		kui.set_time(ui, 1.5)
		kui.frame(ui, &s, view)
		kui.serve_captures(ui)
		saved, failed := 0, 0
		for ev in kui.poll_event(ui) {
			got: kui.Capture_Event
			if shot, ok := kui.message(ev, Shot); ok {
				got = shot.capture
				if got.phase == "saved" && (got.width != 800 || got.frames != 1) do return fail("capture_to's still")
			} else if copy, copy_ok := kui.message(ev, Copy); copy_ok {
				got = copy.capture
				if got.phase == "saved" && (got.width != 800 || got.path != "") do return fail("capture_to_clipboard's copy")
			} else if rec, rec_ok := kui.message(ev, Rec); rec_ok {
				got = rec.capture
				if got.phase == "saved" && (got.width != 800 || got.frames != 1 || abs(got.seconds - 0.5) > 1e-9) {
					return fail("record_to's recording")
				}
			} else {
				on_event(&s, ev)
				continue
			}
			if got.phase == "saved" do saved += 1
			else do failed += 1
		}
		if saved + failed != 3 || (saved > 0 && failed > 0) do return fail("serve_captures answered wrong")
		// The copy is a menu action for the host, as a menu's Copy is: under
		// run the runner puts it on the clipboard.
		if saved > 0 {
			image := false
			for a in kui.take_menu_action(ui) {
				if a.kind == .Set_Clipboard_Image do image = a.width == 800 && a.height == 600 && a.rgba[3] == 255
			}
			if !image do return fail("capture_to_clipboard queued no picture")
			fmt.printfln("kept the window as %s and %s, and on the clipboard", still, movie)
		} else do fmt.println("no GPU adapter: serve_captures answered all three as failed")
	}

	// Pixels to a file's bytes and back through the runner's own codecs,
	// which a test keeping a capture as a file needs no library for: two
	// pixels as a PNG, then a three-frame GIF whose middle frame repeats the
	// first and folds into its time.
	two := []u8{255, 0, 0, 255, 0, 0, 255, 128}
	png, png_ok := kui.encode_image(.Png, 2, 1, two)
	defer delete(png)
	if !png_ok || png[1] != 'P' do return fail("encode_image")
	pw, ph, again, again_ok := kui.decode_image(png)
	defer delete(again)
	if !again_ok || pw != 2 || ph != 1 || again[7] != 128 || again[6] != 255 do return fail("encode_image round trip")
	// The file manager and default apps: a missing path is refused and
	// nothing is started.
	if kui.show_in_folder("/no/such/kui/file.png") || kui.open_external("/no/such/kui/file.png") do return fail("show_in_folder / open_external took a missing path")
	red := []u8{255, 0, 0, 255, 255, 0, 0, 255}
	blue := []u8{0, 0, 255, 255, 0, 0, 255, 255}
	enc, enc_ok := kui.animation_encoder_new(.Gif, 2, 1)
	if !enc_ok do return fail("animation_encoder_new")
	if !kui.animation_encoder_add(enc, red, 0.1) || !kui.animation_encoder_add(enc, red, 0.1) || !kui.animation_encoder_add(enc, blue, 0.3) {
		kui.animation_encoder_free(enc)
		return fail("animation_encoder_add")
	}
	gif_bytes, gif_ok := kui.animation_encoder_finish(enc)
	defer delete(gif_bytes)
	played, played_ok := kui.decode_animation(gif_bytes)
	defer kui.animation_free(played)
	if !gif_ok || !played_ok || len(played.frames) != 2 || abs(played.delays[0] - 0.2) > 1e-9 || played.frames[1][2] != 255 {
		return fail("animation_encoder round trip")
	}
	fmt.println("encoded a PNG and a GIF animation and read them back")

	// And a message with fields survives the round trip through the core.
	round_trip() or_return

	// The icon's refusals (backlog F86), which need no window: a size with
	// no pixels, and a resource id past 16 bits, keeping nothing.
	if kui.set_icon(nil, 32, 32, 0) || kui.set_icon(nil, 0, 0, 70000) || !kui.set_icon(nil, 0, 0, 0) {
		return fail("set_icon took what is not an icon")
	}
	fmt.println("set_icon refused a size with no pixels and a 17-bit resource")

	// The runner's decoder (backlog F138), which needs no window either: a
	// 2x2 GIF, red for 50 ms then blue for 100, played once.
	gif := []u8{
	0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x02, 0x00, 0x02, 0x00, 0x80, 0x00,
	0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0xff, 0x0b, 0x4e, 0x45,
	0x54, 0x53, 0x43, 0x41, 0x50, 0x45, 0x32, 0x2e, 0x30, 0x03, 0x01, 0x01,
	0x00, 0x00, 0x21, 0xf9, 0x04, 0x08, 0x05, 0x00, 0x00, 0x00, 0x2c, 0x00,
	0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x80, 0xff, 0x00, 0x00, 0x00,
	0x00, 0x00, 0x02, 0x02, 0x84, 0x51, 0x00, 0x21, 0xf9, 0x04, 0x08, 0x0a,
	0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00,
	0x80, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x02, 0x02, 0x84, 0x51, 0x00,
	0x3b,
	}
	w, h, rgba, ok := kui.decode_image(gif)
	defer delete(rgba)
	if !ok || w != 2 || h != 2 || rgba[0] != 255 do return fail("decode_image")
	anim, anim_ok := kui.decode_animation(gif)
	defer kui.animation_free(anim)
	if !anim_ok || len(anim.frames) != 2 || anim.loops != 1 || anim.delays[1] != 0.1 || anim.frames[1][2] != 255 {
		return fail("decode_animation")
	}
	if i, next := kui.animation_at(anim, 0.07); i != 1 || abs(next - 0.15) > 1e-9 do return fail("animation_at")
	fmt.println("decoded a GIF's first frame, both frames and their delays")
	fmt.println("headless self-test OK")
	return true
}

drain :: proc(ui: ^kui.Ui, s: ^State) -> (n: int) {
	for ev in kui.poll_event(ui) {
		on_event(s, ev)
		n += 1
	}
	return
}

// Pick :: {kind = "pick", id, name, tags}: an app message with fields,
// encoded by one button's click and decoded from the event it comes back as.
Pick :: struct {
	id:   int,
	name: string,
	tags: []string,
	mode: Mode,
}
Mode :: enum {
	Fast,
	Careful,
}

round_trip :: proc() -> bool {
	ui := kui.new_ui()
	defer kui.free_ui(ui)
	sent := Pick{7, "seven", {"a", "b"}, .Careful}
	pick_view :: proc(p: ^Pick, ui: ^kui.Ui) {
		kui.root(ui, {width = kui.GROW, height = kui.GROW})
		kui.button(ui, "pick", p^)
	}
	kui.frame(ui, &sent, pick_view)
	_, r, _ := kui.find(ui, "pick")
	kui.click(ui, kui.center(r))
	for ev in kui.poll_event(ui) {
		got, ok := kui.message(ev, Pick)
		fmt.printfln("round trip: kind %q -> %v", kui.kind(ev.payload), got)
		if !ok || got.id != 7 || got.name != "seven" || len(got.tags) != 2 || got.tags[1] != "b" || got.mode != .Careful {
			return fail("Pick did not come back as it was sent")
		}
		return true
	}
	return fail("the pick button sent nothing")
}

// Every window's icon (backlog F86): a disc in the buttons' blue, drawn
// here rather than read from a file. Windows shows it in the title bar,
// Alt-Tab and the taskbar, X11 in the window manager's; a Mac has no window
// icon, so it is the app's in the Dock (backlog F181).
set_icon :: proc() -> bool {
	N :: 64
	@(static) px: [N * N * 4]u8
	for y in 0 ..< N {
		for x in 0 ..< N {
			dx, dy, r := f32(x) + 0.5 - N / 2, f32(y) + 0.5 - N / 2, f32(N) / 2 - 2
			// r - d, near the edge, is (r² - d²) / 2r: one pixel of antialiasing.
			a := clamp((r * r - dx * dx - dy * dy) / (2 * r), 0, 1)
			i := (y * N + x) * 4
			px[i], px[i + 1], px[i + 2], px[i + 3] = 0x3b, 0x82, 0xf6, u8(a * 255)
		}
	}
	return kui.set_icon(px[:], N, N, 0)
}

fail :: proc(what: string) -> bool {
	fmt.eprintln("FAIL:", what)
	return false
}

main :: proc() {
	if len(os.args) > 1 && os.args[1] == "--headless" {
		os.exit(0 if headless() else 1)
	}
	state: State
	if !set_icon() do os.exit(1)
	// The Linux desktop's name for the window (backlog F172): Wayland's
	// app_id, X11's WM_CLASS, matched to a kui-odin-counter.desktop if one
	// is installed.
	kui.set_app_id("kui-odin-counter")
	ok := kui.run("kui — Odin counter", &state, view, on_event, teardown = teardown)
	os.exit(0 if ok else 1)
}
